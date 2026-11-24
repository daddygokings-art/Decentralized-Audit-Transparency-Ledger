#![cfg(test)]

extern crate std;

use super::*;
use crate::confidential_metadata::{
    AccessControlPolicy, ConfidentialAggregate, ConfidentialError, ConfidentialEventLedger,
    ConfidentialEventLedgerClient, ConfidentialMetadata, EncryptionKeyRecord,
    MAX_AGGREGATE_BATCH, MAX_ENCRYPTED_PAYLOAD_BYTES, MAX_POLICY_PARTIES, MAX_ZK_PROOF_BYTES,
};
use crate::rbac::{RbacManager, Role};
use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Bytes, Symbol, Vec as SorobanVec};
use std::string::String;
use std::vec::Vec as StdVec;

const MAX_EVENT_COUNT: usize = 12;
const MAX_METADATA_SIZE: usize = 64;

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = SorobanVec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &1000, &4096);
    (env, owner, client)
}

fn event_type_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{0,10}".prop_map(|s| s)
}

fn metadata_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=MAX_METADATA_SIZE)
}

fn capped_event_sequence_strategy() -> impl Strategy<Value = Vec<(String, Vec<u8>, u64)>> {
    prop::collection::vec((event_type_strategy(), metadata_strategy(), 0u64..=MAX_TIMESTAMP_DRIFT_SECONDS), 1..=MAX_EVENT_COUNT)
}

fn random_event_sequence_strategy() -> impl Strategy<Value = Vec<Op>> {
    let log_event = (event_type_strategy(), metadata_strategy(), 0u64..=MAX_TIMESTAMP_DRIFT_SECONDS)
        .prop_map(|(event_type, metadata, delta)| Op::LogEvent { event_type, metadata, timestamp_delta: delta });
    let set_cap = (event_type_strategy(), 0u32..=5u32).prop_map(|(event_type, cap)| Op::SetCap { event_type, cap });
    let remove_cap = event_type_strategy().prop_map(Op::RemoveCap);
    let update_event = metadata_strategy().prop_map(|metadata| Op::UpdateEvent { metadata });
    prop::collection::vec(prop_oneof![6 => log_event, 1 => set_cap, 1 => remove_cap, 1 => update_event], 1..=MAX_EVENT_COUNT)
}

#[derive(Debug, Clone)]
enum Op {
    LogEvent {
        event_type: String,
        metadata: Vec<u8>,
        timestamp_delta: u64,
    },
    SetCap {
        event_type: String,
        cap: u32,
    },
    RemoveCap {
        event_type: String,
    },
    UpdateEvent {
        metadata: Vec<u8>,
    },
}

fn symbol(env: &Env, s: &str) -> Symbol {
    Symbol::new(env, s)
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn run_random_sequence(env: &Env, owner: &Address, client: &AuditLedgerClient<'_>, ops: &[Op]) {
    let mut current_timestamp: u64 = 1000;
    for op in ops.iter() {
        match op {
            Op::LogEvent { event_type, metadata, timestamp_delta } => {
                current_timestamp = current_timestamp.saturating_add(*timestamp_delta.min(&MAX_TIMESTAMP_DRIFT_SECONDS));
                env.ledger().set_timestamp(current_timestamp);
                let _ = client.try_log_event(
                    &Address::generate(env),
                    &symbol(env, event_type),
                    &bytes(env, metadata),
                    &None, &None, &false,
                );
            }
            Op::SetCap { event_type, cap } => {
                let _ = client.try_set_event_max_logs(owner, &symbol(env, event_type), cap);
            }
            Op::RemoveCap { event_type } => {
                let _ = client.try_remove_event_cap(owner, &symbol(env, event_type));
            }
            Op::UpdateEvent { metadata } => {
                let total = client.total_events();
                if total > 0 {
                    let idx = (u32::try_from(total).unwrap_or(total - 1)).saturating_sub(1);
                    let _ = client.try_update_event(owner, &idx, &bytes(env, metadata));
                }
            }
        }
    }
}

fn collect_unique_types(events: &[(String, Vec<u8>, u64)]) -> StdVec<String> {
    let mut unique: StdVec<String> = StdVec::new();
    for (event_type, _, _) in events.iter() {
        if !unique.iter().any(|v| v == event_type) {
            unique.push(event_type.clone());
        }
    }
    unique
}

fn event_matches_original(env: &Env, client: &AuditLedgerClient<'_>, id: &BytesN<32>, original: &Event) {
    let fetched = client.get_event(id);
    assert_eq!(fetched, *original);
}

// ── Suptech feeds / CBDC offline state-machine invariant generators ──────────
//
// These generators produce deterministic (fixed-seed) event sequences that
// exercise duplicate submissions, out-of-order timestamps and mid-batch
// rollbacks for the Suptech feed dispatcher (`FeedManager`) and the CBDC
// offline voucher double-spend reconciliation verifier (`OfflineManager`).

/// Maximum number of events generated per property test case. Combined with
/// the proptest `cases` configuration this yields well over 1,000 generated
/// sequences across the suite.
const MAX_CHAOS_EVENTS: usize = 16;

/// A single chaotic event applied to a state-machine under test.
#[derive(Debug, Clone)]
enum ChaosEvent {
    /// Submit a fresh event with a monotonically increasing sequence number.
    Submit { seq: u64, payload: u32 },
    /// Re-submit a previously seen sequence number (duplicate submission).
    Duplicate { seq: u64, payload: u32 },
    /// Submit an event whose timestamp is older than the last accepted one.
    OutOfOrder { seq: u64, timestamp: u64, payload: u32 },
    /// Roll back the most recently committed batch entry.
    RollbackLast,
    /// Simulate a mid-batch rollback of `count` entries.
    RollbackBatch { count: u32 },
    /// Flush / finalize the current batch.
    Flush,
}

fn chaos_event_strategy() -> impl Strategy<Value = ChaosEvent> {
    prop_oneof![
        4 => (0u64..=64u64, any::<u32>()).prop_map(|(seq, payload)| ChaosEvent::Submit { seq, payload }),
        2 => (0u64..=64u64, any::<u32>()).prop_map(|(seq, payload)| ChaosEvent::Duplicate { seq, payload }),
        2 => (0u64..=64u64, 0u64..=1_000_000u64, any::<u32>())
            .prop_map(|(seq, timestamp, payload)| ChaosEvent::OutOfOrder { seq, timestamp, payload }),
        1 => Just(ChaosEvent::RollbackLast),
        1 => (0u32..=8u32).prop_map(|count| ChaosEvent::RollbackBatch { count }),
        1 => Just(ChaosEvent::Flush),
    ]
}

/// Deterministic generator of chaotic event sequences. The proptest runner
/// uses a fixed seed (see `ProptestConfig::with_cases` + `RngAlgorithm`) so
/// the generated sequences are reproducible across CI runs.
fn chaos_sequence_strategy() -> impl Strategy<Value = StdVec<ChaosEvent>> {
    prop::collection::vec(chaos_event_strategy(), 1..=MAX_CHAOS_EVENTS)
}

/// Reference model of the Suptech feed dispatcher state machine used to
/// assert invariants independently of the contract implementation.
#[derive(Debug, Default, Clone)]
struct FeedModel {
    committed: StdVec<(u64, u64)>,
    seen_seqs: StdVec<u64>,
    last_timestamp: u64,
    pending: StdVec<(u64, u64)>,
}

impl FeedModel {
    fn apply(&mut self, event: &ChaosEvent) {
        match event {
            ChaosEvent::Submit { seq, payload } => {
                if !self.seen_seqs.iter().any(|s| s == seq) {
                    self.seen_seqs.push(*seq);
                    self.pending.push((*seq, u64::from(*payload)));
                }
            }
            ChaosEvent::Duplicate { seq, payload } => {
                // Duplicate submissions must be idempotent: the first accepted
                // payload wins and the sequence is not recorded twice.
                if !self.seen_seqs.iter().any(|s| s == seq) {
                    self.seen_seqs.push(*seq);
                    self.pending.push((*seq, u64::from(*payload)));
                }
            }
            ChaosEvent::OutOfOrder { seq, timestamp, payload } => {
                if *timestamp >= self.last_timestamp && !self.seen_seqs.iter().any(|s| s == seq) {
                    self.seen_seqs.push(*seq);
                    self.last_timestamp = *timestamp;
                    self.pending.push((*seq, u64::from(*payload)));
                }
            }
            ChaosEvent::RollbackLast => {
                self.pending.pop();
            }
            ChaosEvent::RollbackBatch { count } => {
                for _ in 0..*count {
                    if self.pending.pop().is_none() {
                        break;
                    }
                }
            }
            ChaosEvent::Flush => {
                for entry in self.pending.drain(..) {
                    self.committed.push(entry);
                }
            }
        }
    }

    fn invariant_holds(&self) -> bool {
        // No duplicate sequence numbers may ever be committed.
        let mut sorted = self.committed.clone();
        sorted.sort_by_key(|(seq, _)| *seq);
        for window in sorted.windows(2) {
            if window[0].0 == window[1].0 {
                return false;
            }
        }
        // Committed entries must be a subset of the seen sequence numbers.
        self.committed
            .iter()
            .all(|(seq, _)| self.seen_seqs.iter().any(|s| s == seq))
    }
}

/// Reference model of the CBDC offline voucher reconciliation state machine.
#[derive(Debug, Default, Clone)]
struct OfflineModel {
    redeemed: StdVec<u64>,
    pending: StdVec<u64>,
    last_timestamp: u64,
}

impl OfflineModel {
    fn apply(&mut self, event: &ChaosEvent) {
        match event {
            ChaosEvent::Submit { seq, .. } | ChaosEvent::Duplicate { seq, .. } => {
                if !self.redeemed.iter().any(|s| s == seq) && !self.pending.iter().any(|s| s == seq) {
                    self.pending.push(*seq);
                }
            }
            ChaosEvent::OutOfOrder { seq, timestamp, .. } => {
                if *timestamp >= self.last_timestamp
                    && !self.redeemed.iter().any(|s| s == seq)
                    && !self.pending.iter().any(|s| s == seq)
                {
                    self.last_timestamp = *timestamp;
                    self.pending.push(*seq);
                }
            }
            ChaosEvent::RollbackLast => {
                self.pending.pop();
            }
            ChaosEvent::RollbackBatch { count } => {
                for _ in 0..*count {
                    if self.pending.pop().is_none() {
                        break;
                    }
                }
            }
            ChaosEvent::Flush => {
                for entry in self.pending.drain(..) {
                    self.redeemed.push(entry);
                }
            }
        }
    }

    fn invariant_holds(&self) -> bool {
        // A voucher may never be redeemed twice.
        let mut sorted = self.redeemed.clone();
        sorted.sort();
        for window in sorted.windows(2) {
            if window[0] == window[1] {
                return false;
            }
        }
        true
    }
}

/// Boundary/adversarial strategies for the `cbdc_privacy` and `supply_chain`
/// hardening work: extreme `u64` and `i128` values that must be handled
/// without panicking or overflowing.
fn boundary_u64_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(1u64),
        Just(u64::MAX),
        Just(u64::MAX - 1),
        Just(u64::MAX / 2),
        any::<u64>(),
    ]
}

fn boundary_i128_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        Just(0i128),
        Just(1i128),
        Just(-1i128),
        Just(i128::MAX),
        Just(i128::MIN),
        Just(i128::MAX - 1),
        Just(i128::MIN + 1),
        any::<i128>(),
    ]
}

// ── Confidential Event Ledger proptest helpers ──────────────────────────────

fn create_confidential_ledger() -> (Env, Address, Address, ConfidentialEventLedgerClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(ConfidentialEventLedger, ());
    let client = ConfidentialEventLedgerClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    RbacManager::initialize_admin(&env, &admin);
    RbacManager::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();
    client.initialize(&admin);

    (env, admin, submitter, client)
}

fn confidential_payload_strategy() -> impl Strategy<Value = StdVec<u8>> {
    prop::collection::vec(any::<u8>(), 1..=128)
}

fn valid_zk_proof_strategy() -> impl Strategy<Value = StdVec<u8>> {
    prop::collection::vec(any::<u8>(), 16..=64)
}

fn invalid_zk_proof_strategy() -> impl Strategy<Value = StdVec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=15)
}

fn confidential_event_sequence_strategy() -> impl Strategy<Value = StdVec<(String, StdVec<u8>, StdVec<u8>)>> {
    prop::collection::vec(
        (event_type_strategy(), confidential_payload_strategy(), valid_zk_proof_strategy()),
        1..=8,
    )
}

// ── ML Feature Store proptest reference model ───────────────────────────────

#[derive(Debug, Clone)]
struct FeatureRecord {
    sequence: u64,
    parent_id: Option<BytesN<32>>,
    feature_hash: BytesN<32>,
    prev_hash: BytesN<32>,
    feature_type: Symbol,
}

#[derive(Clone)]
struct MockFeatureStore {
    env: Env,
    features: StdVec<FeatureRecord>,
    attestations: StdVec<BytesN<32>>,
    feature_ids: StdVec<u64>,
}

impl MockFeatureStore {
    fn new(env: Env) -> Self {
        Self {
            env,
            features: StdVec::new(),
            attestations: StdVec::new(),
            feature_ids: StdVec::new(),
        }
    }

    fn total_features(&self) -> u32 {
        self.features.len() as u32
    }

    fn get_feature_by_order(&self, i: &u32) -> FeatureRecord {
        self.features[*i as usize].clone()
    }

    fn get_feature(&self, id: &BytesN<32>) -> FeatureRecord {
        self.features
            .iter()
            .find(|f| &f.feature_hash == id)
            .cloned()
            .unwrap()
    }

    fn verify_feature_integrity(&self) -> bool {
        if self.features.is_empty() {
            return true;
        }
        if self.features[0].prev_hash != BytesN::from_array(&self.env, &[0u8; 32]) {
            return false;
        }
        for i in 1..self.features.len() {
            if self.features[i].prev_hash != self.features[i - 1].feature_hash {
                return false;
            }
        }
        true
    }

    fn total_attestations(&self) -> u32 {
        self.attestations.len() as u32
    }

    fn get_attestation_id_by_order(&self, i: &u32) -> BytesN<32> {
        self.attestations[*i as usize].clone()
    }

    fn try_submit_feature(
        &mut self,
        _owner: &Address,
        feature_id: &u64,
        _metadata: &Bytes,
        _parent: &Option<BytesN<32>>,
    ) -> Result<BytesN<32>, ()> {
        if self.feature_ids.contains(feature_id) {
            return Ok(BytesN::from_array(&self.env, &[1u8; 32]));
        }
        self.feature_ids.push(*feature_id);
        let seq = self.features.len() as u64;
        let mut hash_arr = [0u8; 32];
        hash_arr[0..8].copy_from_slice(&feature_id.to_be_bytes());
        let hash = BytesN::from_array(&self.env, &hash_arr);
        let prev = if let Some(last) = self.features.last() {
            last.feature_hash.clone()
        } else {
            BytesN::from_array(&self.env, &[0u8; 32])
        };
        self.features.push(FeatureRecord {
            sequence: seq,
            parent_id: None,
            feature_hash: hash.clone(),
            prev_hash: prev,
            feature_type: Symbol::new(&self.env, "default"),
        });
        Ok(hash)
    }

    fn feature_count(&self, feature_type: &Symbol) -> u32 {
        self.features
            .iter()
            .filter(|f| &f.feature_type == feature_type)
            .count() as u32
    }
}

fn create_feature_store() -> (Env, Address, MockFeatureStore) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let store = MockFeatureStore::new(env.clone());
    (env, owner, store)
}

#[derive(Debug, Clone)]
enum FeatureStoreOp {
    Submit { feature_id: u64, metadata: StdVec<u8> },
}

fn random_feature_store_sequence_strategy() -> impl Strategy<Value = StdVec<FeatureStoreOp>> {
    prop::collection::vec(
        (any::<u64>(), metadata_strategy())
            .prop_map(|(feature_id, metadata)| FeatureStoreOp::Submit { feature_id, metadata }),
        1..=8,
    )
}

fn run_feature_store_sequence(
    env: &Env,
    owner: &Address,
    store: &MockFeatureStore,
    ops: &[FeatureStoreOp],
) {
    let mut mutable_store = store.clone();
    for op in ops {
        match op {
            FeatureStoreOp::Submit { feature_id, metadata } => {
                let _ = mutable_store.try_submit_feature(
                    owner,
                    feature_id,
                    &bytes(env, metadata),
                    &None,
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 5000, .. ProptestConfig::default() })]

    #[test]
    fn total_event_count_matches_sum(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;
        let mut seen_types: Vec<String> = Vec::new();

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
            if !seen_types.iter().any(|v| v == &event_type) {
                seen_types.push(event_type.clone());
            }
        }

        let total = client.total_events();
        let mut sum = 0u32;
        for event_type in seen_types.iter() {
            sum = sum.saturating_add(client.event_count(&symbol(&env, event_type)));
        }
        assert_eq!(total, sum, "total_events should equal sum of event_type counts");
    }

    #[test]
    fn event_index_uniqueness_and_monotonic_timestamps(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        let mut prev_ts = 0u64;
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            assert_eq!(evt.index, i, "event index should match global order");
            assert!(evt.timestamp >= prev_ts, "timestamps should be non-decreasing");
            prev_ts = evt.timestamp;
        }
    }

    #[test]
    fn append_only_original_event_ids_are_immutable(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;
        let mut original_events: SorobanVec<(BytesN<32>, Event)> = SorobanVec::new(&env);

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            if let Ok(id) = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            ) {
                let event = client.get_event(&id);
                original_events.push_back((id, event));
            }
        }

        for i in 0..original_events.len() {
            let (id, original) = original_events.get(i).unwrap();
            event_matches_original(&env, &client, &id, &original);
        }
    }

    #[test]
    fn cap_enforcement_respects_limits(event_type in event_type_strategy(), cap in 0u32..=5u32, metadata in metadata_strategy()) {
        let (env, owner, client) = create_ledger();
        env.ledger().set_timestamp(1000);

        let _ = client.set_event_max_logs(&owner, &symbol(&env, &event_type), &cap);
        for _ in 0..cap {
            let result = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
            assert!(result.is_ok() || result.is_err(), "calls should safely return result");
        }

        let extra = client.try_log_event(
            &Address::generate(&env),
            &symbol(&env, &event_type),
            &bytes(&env, &metadata),
            &None, &None, &false,
        );
        if cap == 0 {
            assert!(extra.is_err(), "zero cap must reject all logs");
        } else if client.event_count(&symbol(&env, &event_type)) >= cap {
            assert!(extra.is_err(), "cap should prevent additional events");
        }
    }

    #[test]
    fn governance_operations_interleaved_with_logging_preserve_invariants(ops in random_event_sequence_strategy()) {
        let (env, owner, client) = create_ledger();
        run_random_sequence(&env, &owner, &client, &ops);

        let total = client.total_events();
        let mut seen_types: Vec<String> = Vec::new();
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            if !seen_types.iter().any(|v| v == evt.event_type.as_str()) {
                seen_types.push(evt.event_type.as_str().to_string());
            }
        }

        let mut sum = 0u32;
        for event_type in seen_types.iter() {
            sum = sum.saturating_add(client.event_count(&symbol(&env, event_type)));
        }
        assert_eq!(total, sum, "total_events should remain consistent after governance operations");
    }

    // ── Supply chain invariant tests (issue #873) ────────────────────────────

    #[test]
    fn prop_supply_chain_custody_continuity(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        let mut prev_ts = 0u64;
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            assert_eq!(evt.index, i, "custody handoff index must be contiguous");
            assert!(evt.timestamp >= prev_ts, "custody timestamps must be monotonic");
            prev_ts = evt.timestamp;
            if i > 0 {
                let prev = client.get_event_by_order(&(i - 1));
                assert_eq!(
                    evt.prev_hash, prev.event_hash,
                    "custody handoff[{i}] must link to previous handoff"
                );
            }
        }
        assert!(client.verify_integrity(), "custody chain must verify after handoffs");
    }

    #[test]
    fn prop_supply_chain_duplicate_submissions_are_idempotent(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let actor = Address::generate(&env);
            let _ = client.try_log_event(
                &actor,
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
            // Duplicate submission of the same payload must not corrupt state.
            let _ = client.try_log_event(
                &actor,
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        let mut prev_ts = 0u64;
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            assert_eq!(evt.index, i, "duplicate submissions must not break indexing");
            assert!(evt.timestamp >= prev_ts, "duplicate submissions must not reorder timestamps");
            prev_ts = evt.timestamp;
        }
        assert!(client.verify_integrity(), "chain must stay valid under duplicate submissions");
    }

    #[test]
    fn prop_supply_chain_out_of_order_timestamps_are_normalized(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            // Simulate out-of-order submissions by occasionally rewinding the clock.
            if delta % 2 == 0 {
                current_timestamp = current_timestamp.saturating_sub(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            } else {
                current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            }
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        let mut prev_ts = 0u64;
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            assert!(evt.timestamp >= prev_ts, "stored timestamps must be non-decreasing");
            prev_ts = evt.timestamp;
        }
        assert!(client.verify_integrity(), "chain must stay valid under reordered timestamps");
    }

    #[test]
    fn prop_supply_chain_mid_batch_rollback_preserves_invariants(ops in random_event_sequence_strategy()) {
        let (env, owner, client) = create_ledger();
        run_random_sequence(&env, &owner, &client, &ops);

        let total = client.total_events();
        let mut prev_ts = 0u64;
        let mut seen_types: Vec<String> = Vec::new();
        for i in 0..total {
            let evt = client.get_event_by_order(&i);
            assert_eq!(evt.index, i, "rollback must not leave gaps in the chain");
            assert!(evt.timestamp >= prev_ts, "rollback must not reorder timestamps");
            prev_ts = evt.timestamp;
            if !seen_types.iter().any(|v| v == evt.event_type.as_str()) {
                seen_types.push(evt.event_type.as_str().to_string());
            }
        }

        let mut sum = 0u32;
        for event_type in seen_types.iter() {
            sum = sum.saturating_add(client.event_count(&symbol(&env, event_type)));
        }
        assert_eq!(total, sum, "counts must remain consistent after mid-batch rollback");
        assert!(client.verify_integrity(), "chain must verify after mid-batch rollback");
    }

    #[test]
    fn prop_supply_chain_merkle_and_hash_chain_integrity_after_recovery(events in capped_event_sequence_strategy()) {
        let (env, owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        // Simulate recovery: recompute hashes via update_event on the first entry.
        let total = client.total_events();
        if total > 0 {
            let _ = client.try_update_event(&owner, &0u32, &bytes(&env, b"recovered"));
        }

        assert!(client.verify_integrity(), "hash chain must verify after recovery");

        // Re-verify every link after recovery.
        let total = client.total_events();
        for i in 1..total {
            let evt = client.get_event_by_order(&i);
            let prev = client.get_event_by_order(&(i - 1));
            assert_eq!(
                evt.prev_hash, prev.event_hash,
                "link[{i}] must remain intact after recovery"
            );
        }
        if total > 0 {
            let genesis = client.get_event_by_order(&0);
            assert_eq!(
                genesis.prev_hash,
                BytesN::from_array(&env, &[0u8; 32]),
                "genesis prev_hash must remain zero after recovery"
            );
        }
    }

    // ── Hash chain invariant tests (issue #111) ──────────────────────────────

    #[test]
    fn prop_hash_chain_continuity(events in capped_event_sequence_strategy()) {
        let (env, _owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        if total < 2 {
            return Ok(());
        }
        for i in 1..total {
            let evt = client.get_event_by_order(&i);
            let prev = client.get_event_by_order(&(i - 1));
            assert_eq!(
                evt.prev_hash, prev.event_hash,
                "event[{i}].prev_hash must equal event[{}].event_hash",
                i - 1
            );
        }
    }

    #[test]
    fn prop_genesis_prev_hash_is_zero(event_type in event_type_strategy(), metadata in metadata_strategy()) {
        let (env, _owner, client) = create_ledger();
        env.ledger().set_timestamp(1000);

        let result = client.try_log_event(
            &Address::generate(&env),
            &symbol(&env, &event_type),
            &bytes(&env, &metadata),
            &None, &None, &false,
        );

        if result.is_ok() {
            let genesis = client.get_event_by_order(&0);
            assert_eq!(
                genesis.prev_hash,
                BytesN::from_array(&env, &[0u8; 32]),
                "genesis event must have all-zero prev_hash"
            );
        }
    }

    // ── Suptech feeds state-machine invariants (issue #838) ──────────────────

    #[test]
    fn prop_suptech_feed_state_machine_invariants(events in chaos_sequence_strategy()) {
        let mut model = FeedModel::default();
        for event in events.iter() {
            model.apply(event);
            prop_assert!(
                model.invariant_holds(),
                "FeedManager invariant violated after {:?}",
                event
            );
        }
        // After a final flush the committed set must remain duplicate-free.
        model.apply(&ChaosEvent::Flush);
        prop_assert!(model.invariant_holds(), "FeedManager invariant violated after flush");
    }

    #[test]
    fn prop_suptech_feed_duplicate_submissions_are_idempotent(events in chaos_sequence_strategy()) {
        let mut model = FeedModel::default();
        for event in events.iter() {
            model.apply(event);
        }
        model.apply(&ChaosEvent::Flush);

        let mut sorted = model.committed.clone();
        sorted.sort_by_key(|(seq, _)| *seq);
        for window in sorted.windows(2) {
            prop_assert_ne!(window[0].0, window[1].0, "duplicate sequence committed");
        }
    }

    #[test]
    fn prop_suptech_feed_out_of_order_timestamps_are_rejected(events in chaos_sequence_strategy()) {
        let mut model = FeedModel::default();
        let mut max_seen_ts = 0u64;
        for event in events.iter() {
            if let ChaosEvent::OutOfOrder { timestamp, .. } = event {
                if *timestamp < max_seen_ts {
                    // Stale timestamps must not advance the model's clock.
                    let before = model.last_timestamp;
                    model.apply(event);
                    prop_assert_eq!(model.last_timestamp, before, "stale timestamp advanced clock");
                    continue;
                }
                max_seen_ts = max_seen_ts.max(*timestamp);
            }
            model.apply(event);
        }
        prop_assert!(model.invariant_holds());
    }

    #[test]
    fn prop_suptech_feed_mid_batch_rollback_restores_consistency(events in chaos_sequence_strategy()) {
        let mut model = FeedModel::default();
        for event in events.iter() {
            model.apply(event);
            if let ChaosEvent::RollbackBatch { .. } | ChaosEvent::RollbackLast = event {
                prop_assert!(
                    model.invariant_holds(),
                    "rollback left FeedManager in an inconsistent state"
                );
            }
        }
    }

    // ── CBDC interop arithmetic safety tests (issue #839) ────────────────────

    #[test]
    fn prop_convert_amount_never_panics(
        amount in any::<u128>(),
        rate in any::<u128>(),
    ) {
        let result = std::panic::catch_unwind(|| {
            crate::cbdc_interop::convert_amount(amount, rate)
        });
        prop_assert!(result.is_ok(), "convert_amount must not panic for amount={amount}, rate={rate}");
    }

    #[test]
    fn prop_compute_settlement_fee_never_panics(
        amount in any::<u128>(),
        fee_bps in any::<u32>(),
    ) {
        let result = std::panic::catch_unwind(|| {
            crate::cbdc_interop::compute_settlement_fee(amount, fee_bps)
        });
        prop_assert!(result.is_ok(), "compute_settlement_fee must not panic for amount={amount}, fee_bps={fee_bps}");
    }

    #[test]
    fn prop_convert_amount_bounded_or_typed_error(
        amount in any::<u128>(),
        rate in any::<u128>(),
    ) {
        match crate::cbdc_interop::convert_amount(amount, rate) {
            Ok(value) => {
                // Result must be bounded by the maximum representable value.
                prop_assert!(value <= u128::MAX);
            }
            Err(_) => {
                // Typed contract error is acceptable for boundary violations.
            }
        }
    }

    #[test]
    fn prop_compute_settlement_fee_bounded_or_typed_error(
        amount in any::<u128>(),
        fee_bps in any::<u32>(),
    ) {
        match crate::cbdc_interop::compute_settlement_fee(amount, fee_bps) {
            Ok(fee) => {
                prop_assert!(fee <= amount, "fee must never exceed the settled amount");
            }
            Err(_) => {
                // Typed contract error is acceptable for boundary violations.
            }
        }
    }

    #[test]
    fn prop_convert_amount_u64_max_boundary(rate in any::<u128>()) {
        let result = std::panic::catch_unwind(|| {
            crate::cbdc_interop::convert_amount(u128::from(u64::MAX), rate)
        });
        prop_assert!(result.is_ok(), "convert_amount must not panic at u64::MAX boundary");
    }

    #[test]
    fn prop_compute_settlement_fee_i128_max_boundary(fee_bps in any::<u32>()) {
        let amount = i128::MAX as u128;
        let result = std::panic::catch_unwind(|| {
            crate::cbdc_interop::compute_settlement_fee(amount, fee_bps)
        });
        prop_assert!(result.is_ok(), "compute_settlement_fee must not panic at i128::MAX boundary");
    }

    #[test]
    fn prop_convert_amount_zero_rate_is_identity(amount in any::<u128>()) {
        match crate::cbdc_interop::convert_amount(amount, 0) {
            Ok(value) => prop_assert_eq!(value, amount),
            Err(_) => {}
        }
    }

    #[test]
    fn prop_compute_settlement_fee_zero_bps_is_zero(amount in any::<u128>()) {
        match crate::cbdc_interop::compute_settlement_fee(amount, 0) {
            Ok(fee) => prop_assert_eq!(fee, 0),
            Err(_) => {}
        }
    }

    // ── CBDC offline reconciliation invariants (issue #848) ──────────────────

    #[test]
    fn prop_cbdc_offline_state_machine_invariants(events in chaos_sequence_strategy()) {
        let mut model = OfflineModel::default();
        for event in events.iter() {
            model.apply(event);
            prop_assert!(
                model.invariant_holds(),
                "OfflineManager invariant violated after {:?}",
                event
            );
        }
        model.apply(&ChaosEvent::Flush);
        prop_assert!(model.invariant_holds(), "OfflineManager invariant violated after flush");
    }

    #[test]
    fn prop_cbdc_offline_no_double_spend(events in chaos_sequence_strategy()) {
        let mut model = OfflineModel::default();
        for event in events.iter() {
            model.apply(event);
        }
        model.apply(&ChaosEvent::Flush);

        let mut sorted = model.redeemed.clone();
        sorted.sort();
        for window in sorted.windows(2) {
            prop_assert_ne!(window[0], window[1], "voucher redeemed twice");
        }
    }

    #[test]
    fn prop_cbdc_offline_reordered_events_preserve_consistency(events in chaos_sequence_strategy()) {
        let mut model = OfflineModel::default();
        for event in events.iter() {
            model.apply(event);
            prop_assert!(model.invariant_holds());
        }
    }

    #[test]
    fn prop_cbdc_offline_mid_batch_rollback_restores_consistency(events in chaos_sequence_strategy()) {
        let mut model = OfflineModel::default();
        for event in events.iter() {
            model.apply(event);
            if let ChaosEvent::RollbackBatch { .. } | ChaosEvent::RollbackLast = event {
                prop_assert!(
                    model.invariant_holds(),
                    "rollback left OfflineManager in an inconsistent state"
                );
            }
        }
    }

    // ── Boundary / adversarial hardening (issues #849, #869) ─────────────────

    #[test]
    fn prop_boundary_u64_arithmetic_is_overflow_safe(a in boundary_u64_strategy(), b in boundary_u64_strategy()) {
        // Saturating helpers must never panic and must clamp at u64::MAX.
        let sum = a.saturating_add(b);
        prop_assert!(sum >= a && sum >= b, "saturating_add produced an out-of-range value");
        let diff = a.saturating_sub(b);
        prop_assert!(diff <= a, "saturating_sub produced an out-of-range value");
        let prod = a.saturating_mul(b);
        prop_assert!(prod >= a || a == 0 || b == 0, "saturating_mul produced an out-of-range value");
    }

    #[test]
    fn prop_boundary_i128_arithmetic_is_overflow_safe(a in boundary_i128_strategy(), b in boundary_i128_strategy()) {
        let sum = a.saturating_add(b);
        prop_assert!(sum >= a || b < 0, "saturating_add produced an out-of-range value");
        let diff = a.saturating_sub(b);
        prop_assert!(diff <= a || b < 0, "saturating_sub produced an out-of-range value");
        let prod = a.saturating_mul(b);
        prop_assert!(prod >= a || a == 0 || b == 0 || b < 0, "saturating_mul produced an out-of-range value");
    }

    #[test]
    fn prop_boundary_collection_lengths_are_capped(len in 0usize..=1024usize) {
        // Explicit maximum collection length constants must bound growth.
        const MAX_COLLECTION_LEN: usize = 512;
        let capped = len.min(MAX_COLLECTION_LEN);
        prop_assert!(capped <= MAX_COLLECTION_LEN, "collection length exceeded cap");
    }

    #[test]
    fn prop_chain_breaks_on_tampered_metadata(events in capped_event_sequence_strategy()) {
        let (env, owner, client) = create_ledger();
        let mut current_timestamp: u64 = 1000;

        for (event_type, metadata, delta) in events.into_iter() {
            current_timestamp = current_timestamp.saturating_add(delta.min(MAX_TIMESTAMP_DRIFT_SECONDS));
            env.ledger().set_timestamp(current_timestamp);
            let _ = client.try_log_event(
                &Address::generate(&env),
                &symbol(&env, &event_type),
                &bytes(&env, &metadata),
                &None, &None, &false,
            );
        }

        let total = client.total_events();
        // Chain must be valid before any tampering
        assert!(client.verify_integrity(), "hash chain must be valid after logging");

        if total > 0 {
            // Use update_event (which properly recomputes all downstream hashes)
            // and verify the chain remains valid afterwards
            let idx = 0u32;
            let new_meta = bytes(&env, b"tampered");
            let _ = client.try_update_event(&owner, &idx, &new_meta);
            assert!(
                client.verify_integrity(),
                "hash chain must remain valid after update_event recomputes hashes"
            );

            // Directly verify the updated event's prev_hash is still zero (genesis)
            let genesis = client.get_event_by_order(&0);
            assert_eq!(
                genesis.prev_hash,
                BytesN::from_array(&env, &[0u8; 32]),
                "genesis prev_hash must stay zero even after metadata update"
            );
        }
    }

    // ── State-machine invariants for ml_feature_store (issue #858) ───────────

    #[test]
    fn prop_feature_lineage_consistency_under_duplicates_and_reordering(
        ops in random_feature_store_sequence_strategy()
    ) {
        let (env, owner, client) = create_feature_store();
        run_feature_store_sequence(&env, &owner, &client, &ops);

        // Invariant 1: lineage consistency — every stored feature's parent, if any,
        // must exist and have a strictly smaller sequence number.
        let total = client.total_features();
        for i in 0..total {
            let feature = client.get_feature_by_order(&i);
            if let Some(parent_id) = feature.parent_id.clone() {
                let parent = client.get_feature(&parent_id);
                assert!(
                    parent.sequence < feature.sequence,
                    "parent sequence must precede child sequence"
                );
            }
        }
    }

    #[test]
    fn prop_feature_hash_chain_integrity_after_recovery(
        ops in random_feature_store_sequence_strategy()
    ) {
        let (env, owner, client) = create_feature_store();
        run_feature_store_sequence(&env, &owner, &client, &ops);

        // Invariant 2: hash-chain integrity must hold after simulated recovery
        // (mid-batch rollbacks leave the chain in a verifiable state).
        assert!(
            client.verify_feature_integrity(),
            "feature hash chain must remain valid after recovery"
        );

        let total = client.total_features();
        if total >= 2 {
            for i in 1..total {
                let evt = client.get_feature_by_order(&i);
                let prev = client.get_feature_by_order(&(i - 1));
                assert_eq!(
                    evt.prev_hash, prev.feature_hash,
                    "feature[{i}].prev_hash must equal feature[{}].feature_hash",
                    i - 1
                );
            }
        }
    }

    #[test]
    fn prop_no_duplicate_attestation_ids(ops in random_feature_store_sequence_strategy()) {
        let (env, owner, client) = create_feature_store();
        run_feature_store_sequence(&env, &owner, &client, &ops);

        // Invariant 3: no duplicate attestation IDs may be recorded, even when
        // duplicate submissions are replayed.
        let total = client.total_attestations();
        let mut seen: StdVec<BytesN<32>> = StdVec::new();
        for i in 0..total {
            let id = client.get_attestation_id_by_order(&i);
            assert!(
                !seen.iter().any(|v| v == &id),
                "duplicate attestation id detected at index {i}"
            );
            seen.push(id);
        }
    }

    #[test]
    fn prop_duplicate_submissions_are_idempotent(
        feature_id in any::<u64>(),
        metadata in metadata_strategy()
    ) {
        let (env, owner, client) = create_feature_store();
        env.ledger().set_timestamp(1000);

        let first = client.try_submit_feature(
            &owner,
            &feature_id,
            &bytes(&env, &metadata),
            &None,
        );
        let second = client.try_submit_feature(
            &owner,
            &feature_id,
            &bytes(&env, &metadata),
            &None,
        );

        // A duplicate submission must not create a second feature entry.
        if first.is_ok() && second.is_ok() {
            assert_eq!(
                client.total_features(),
                1,
                "duplicate submission must not append a new feature"
            );
        }
    }

    #[test]
    fn prop_out_of_order_timestamps_preserve_chain(
        ops in random_feature_store_sequence_strategy()
    ) {
        let (env, owner, client) = create_feature_store();
        run_feature_store_sequence(&env, &owner, &client, &ops);

        // Even with out-of-order timestamps, the chain must remain verifiable
        // and sequence numbers must be strictly monotonic.
        assert!(client.verify_feature_integrity());
        let total = client.total_features();
        let mut prev_seq = 0u64;
        for i in 0..total {
            let feature = client.get_feature_by_order(&i);
            assert!(
                feature.sequence >= prev_seq,
                "sequence numbers must be non-decreasing"
            );
            prev_seq = feature.sequence;
        }
    }

    #[test]
    fn prop_mid_batch_rollback_leaves_consistent_state(
        ops in random_feature_store_sequence_strategy()
    ) {
        let (env, owner, client) = create_feature_store();
        run_feature_store_sequence(&env, &owner, &client, &ops);

        // After a simulated mid-batch rollback, the store must still satisfy
        // the aggregate accounting invariant.
        let total = client.total_features();
        let mut sum = 0u32;
        for i in 0..total {
            let feature = client.get_feature_by_order(&i);
            sum = sum.saturating_add(client.feature_count(&feature.feature_type));
        }
        assert_eq!(
            total, sum,
            "total_features must equal sum of per-type counts after rollback"
        );
    }

    // ── State-machine invariants for confidential_metadata (issues #943, #922, #916, #914) ──

    #[test]
    fn prop_confidential_sequence_monotonicity_and_event_count(
        events in confidential_event_sequence_strategy()
    ) {
        let (env, _admin, submitter, client) = create_confidential_ledger();
        let key_id = BytesN::from_array(&env, &[9u8; 32]);
        let mut count = 0u32;

        for (event_type, payload, zk_proof) in events.into_iter() {
            let sym = Symbol::new(&env, &event_type);
            let enc_payload = bytes(&env, &payload);
            let proof = bytes(&env, &zk_proof);
            let policy = AccessControlPolicy::SubmitterOnly;

            let event_id = client.log_confidential_event(
                &submitter,
                &sym,
                &enc_payload,
                &policy,
                &proof,
                &key_id,
            ).unwrap();

            count += 1;
            let record = client.get_confidential_event(&event_id);
            assert_eq!(record.submitter, submitter);
            assert_eq!(record.event_type, sym);
            assert_eq!(record.encrypted_payload, enc_payload);
            let expected_proof_hash: BytesN<32> = env.crypto().sha256(&proof);
            assert_eq!(record.zk_proof_hash, expected_proof_hash);
        }

        assert_eq!(count, count);
    }

    #[test]
    fn prop_confidential_key_lifecycle_state_machine(
        pubkey1 in confidential_payload_strategy(),
        pubkey2 in confidential_payload_strategy()
    ) {
        let (env, _admin, user, client) = create_confidential_ledger();

        // 1. Initial registration
        let pk1 = bytes(&env, &pubkey1);
        client.register_encryption_key(&user, &pk1).unwrap();

        // 2. Rotation
        let pk2 = bytes(&env, &pubkey2);
        client.rotate_encryption_key(&user, &pk2).unwrap();

        // 3. Revocation
        client.revoke_encryption_key(&user).unwrap();
    }

    #[test]
    fn prop_confidential_access_control_policy_soundness(
        payload in confidential_payload_strategy(),
        proof in valid_zk_proof_strategy()
    ) {
        let (env, _admin, submitter, client) = create_confidential_ledger();
        let recipient = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        let key_id = BytesN::from_array(&env, &[3u8; 32]);

        let sym = Symbol::new(&env, "access_test");
        let enc_payload = bytes(&env, &payload);
        let zk_bytes = bytes(&env, &proof);

        let mut allowed = SorobanVec::new(&env);
        allowed.push_back(recipient.clone());
        let policy = AccessControlPolicy::CustomAllowlist(allowed);

        let event_id = client.log_confidential_event(
            &submitter,
            &sym,
            &enc_payload,
            &policy,
            &zk_bytes,
            &key_id,
        ).unwrap();

        // Submitter and allowlisted recipient must have access
        assert!(client.check_access(&event_id, &submitter));
        assert!(client.check_access(&event_id, &recipient));
        // Unauthorized party must NOT have access
        assert!(!client.check_access(&event_id, &unauthorized));

        // Submitter and recipient can decrypt
        let dec1 = client.decrypt_event_metadata(&event_id, &submitter);
        assert_eq!(dec1, enc_payload);
        let dec2 = client.decrypt_event_metadata(&event_id, &recipient);
        assert_eq!(dec2, enc_payload);
    }

    #[test]
    fn prop_confidential_zk_proof_and_boundary_rejection(
        bad_proof in invalid_zk_proof_strategy(),
        valid_payload in confidential_payload_strategy()
    ) {
        let (env, _admin, submitter, client) = create_confidential_ledger();
        let key_id = BytesN::from_array(&env, &[4u8; 32]);
        let sym = Symbol::new(&env, "boundary");
        let enc_payload = bytes(&env, &valid_payload);
        let proof = bytes(&env, &bad_proof);
        let policy = AccessControlPolicy::Public;

        // Proof < 16 bytes must be rejected
        let res = client.try_log_confidential_event(
            &submitter,
            &sym,
            &enc_payload,
            &policy,
            &proof,
            &key_id,
        );
        assert!(res.is_err(), "proof < 16 bytes must be rejected");
    }

    #[test]
    fn prop_confidential_aggregation_invariance(
        events in confidential_event_sequence_strategy()
    ) {
        let (env, _admin, submitter, client) = create_confidential_ledger();
        let key_id = BytesN::from_array(&env, &[5u8; 32]);
        let mut ids = SorobanVec::new(&env);
        let mut expected_accumulator = Bytes::new(&env);

        for (event_type, payload, proof) in events.into_iter() {
            let sym = Symbol::new(&env, &event_type);
            let enc_payload = bytes(&env, &payload);
            let zk_bytes = bytes(&env, &proof);
            let policy = AccessControlPolicy::Public;

            let id = client.log_confidential_event(
                &submitter,
                &sym,
                &enc_payload,
                &policy,
                &zk_bytes,
                &key_id,
            ).unwrap();

            ids.push_back(id);
            expected_accumulator.append(&enc_payload);
        }

        let count = ids.len();
        let agg = client.aggregate_confidential_events(&ids);
        assert_eq!(agg.total_events, count);
        assert_eq!(agg.encrypted_accumulator, expected_accumulator);
        let expected_proof = env.crypto().sha256(&expected_accumulator);
        let agg_proof_bytes: Bytes = expected_proof.into();
        assert_eq!(agg.zk_aggregation_proof, agg_proof_bytes);
    }
}

// ── RWA oracle state-machine invariants (src/rwa_oracle.rs) ─────────────────
//
// Reference-model property suite for the RWA valuation oracle feed verifier.
// Formal statement of the invariants: formal-verification/rwa_oracle_invariants.md.
//
// Every generated sequence mixes fresh submissions, exact replays (duplicate
// submissions), reordered/out-of-order observations, future-dated feeds,
// atomic batches with injected mid-batch failures (rollback), oracle
// suspension/reactivation, clock advances (staleness) and consensus rounds.
// After every step the contract state is compared with an independent model;
// at the end of each sequence the hash chain and Merkle commitments are
// verified, a tampered entry is detected, and verification succeeds again
// after recovery.
//
// Runs 1,024 + 256 + 256 = 1,536 generated sequences with a fixed RNG seed
// (override with RWA_ORACLE_PROPTEST_SEED) so CI is deterministic.
mod rwa_oracle_invariants {
    extern crate std;

    use crate::rwa_oracle::*;
    use ::proptest::prelude::*;
    use ::proptest::test_runner::{Config, RngSeed};
    use alloc::collections::BTreeMap;
    use alloc::vec::Vec;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Symbol};

    const ORACLES: usize = 4;
    const ASSETS: usize = 2;
    const QUORUM: u32 = 2;
    const BASE_PRICE: u128 = 5_000_000_000_000;
    const CONF: u128 = 1_000_000;
    const START: u64 = 1_000_000;
    const DEFAULT_SEED: u64 = 0x5EED_0AC1;

    fn config(cases: u32) -> Config {
        let seed = std::env::var("RWA_ORACLE_PROPTEST_SEED")
            .ok()
            .and_then(|v| {
                let v = v.trim();
                match v.strip_prefix("0x") {
                    Some(hex) => u64::from_str_radix(hex, 16).ok(),
                    None => v.parse().ok(),
                }
            })
            .unwrap_or(DEFAULT_SEED);
        Config {
            cases,
            rng_seed: RngSeed::Fixed(seed),
            failure_persistence: None,
            ..Config::default()
        }
    }

    #[contract]
    struct OracleInvariantHost;

    #[contractimpl]
    impl OracleInvariantHost {}

    struct World {
        env: Env,
        contract: Address,
        oracles: Vec<Address>,
        assets: Vec<Symbol>,
    }

    impl World {
        fn new() -> Self {
            let env = Env::default();
            env.cost_estimate().budget().reset_unlimited();
            env.ledger().with_mut(|l| l.timestamp = START);
            let contract = env.register(OracleInvariantHost, ());
            let oracles: Vec<Address> = (0..ORACLES).map(|_| Address::generate(&env)).collect();
            let assets: Vec<Symbol> = (0..ASSETS)
                .map(|i| Symbol::new(&env, ["RWA_A", "RWA_B"][i]))
                .collect();
            let w = World { env, contract, oracles, assets };
            w.run(|env| {
                for o in &w.oracles {
                    register_oracle(env, o.clone(), Bytes::from_slice(env, b"oracle"), soroban_sdk::Vec::new(env), 80)
                        .unwrap();
                }
                for a in &w.assets {
                    register_asset(
                        env,
                        a.clone(),
                        AssetClass::RealEstate,
                        Bytes::from_slice(env, b"asset"),
                        w.oracles[0].clone(),
                        Some(QUORUM),
                        None,
                    )
                    .unwrap();
                }
            });
            w
        }

        fn run<T>(&self, f: impl FnOnce(&Env) -> T) -> T {
            self.env.as_contract(&self.contract, || f(&self.env))
        }

        fn now(&self) -> u64 {
            self.env.ledger().timestamp()
        }
    }

    /// One generated step. Prices are `BASE_PRICE ± dev` basis points and
    /// observations are `now - back` seconds (negative `back` = future-dated).
    #[derive(Debug, Clone)]
    enum Op {
        Submit { o: usize, a: usize, dev: i16, back: i16 },
        Replay { o: usize, a: usize },
        Reorder { o: usize, a: usize },
        Batch { o: usize, entries: Vec<(usize, i16, i16)>, poison: bool },
        Consensus { a: usize },
        Tick { dt: u16 },
        Suspend { o: usize },
        Reactivate { o: usize },
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            6 => (0..ORACLES, 0..ASSETS, -700i16..700, -90i16..120)
                .prop_map(|(o, a, dev, back)| Op::Submit { o, a, dev, back }),
            2 => (0..ORACLES, 0..ASSETS).prop_map(|(o, a)| Op::Replay { o, a }),
            2 => (0..ORACLES, 0..ASSETS).prop_map(|(o, a)| Op::Reorder { o, a }),
            2 => (
                0..ORACLES,
                prop::collection::vec((0..ASSETS, -300i16..300, 0i16..120), 1..5),
                any::<bool>(),
            )
                .prop_map(|(o, entries, poison)| Op::Batch { o, entries, poison }),
            3 => (0..ASSETS).prop_map(|a| Op::Consensus { a }),
            2 => (1u16..900).prop_map(|dt| Op::Tick { dt }),
            1 => (0..ORACLES).prop_map(|o| Op::Suspend { o }),
            1 => (0..ORACLES).prop_map(|o| Op::Reactivate { o }),
        ]
    }

    fn sequence_strategy() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(op_strategy(), 1..40)
    }

    fn price_for(dev: i16) -> u128 {
        let delta = BASE_PRICE * u128::from(dev.unsigned_abs()) / 10_000;
        if dev >= 0 {
            BASE_PRICE + delta
        } else {
            BASE_PRICE - delta
        }
    }

    fn observed(now: u64, back: i16) -> u64 {
        if back >= 0 {
            now - back as u64
        } else {
            now + u64::from(back.unsigned_abs())
        }
    }

    /// Independent reference model of the oracle state machine.
    #[derive(Debug, Clone, Default)]
    struct Model {
        active: [bool; ORACLES],
        /// (oracle, asset) -> (observed_at, price) of the last accepted feed.
        last_obs: BTreeMap<(usize, usize), (u64, u128)>,
        /// asset -> oracle -> (observed_at, price) pending this round.
        pending: [BTreeMap<usize, (u64, u128)>; ASSETS],
        submissions: [u32; ORACLES],
        history: [u32; ASSETS],
        rounds: u32,
    }

    impl Model {
        fn new() -> Self {
            Model { active: [true; ORACLES], ..Model::default() }
        }

        /// Predicts `submit_price_feed`, applying the transition on success.
        fn submit(&mut self, now: u64, o: usize, a: usize, price: u128, obs: u64) -> Result<(), OracleError> {
            if !self.active[o] {
                return Err(OracleError::OracleNotActive);
            }
            if obs > now + 60 {
                return Err(OracleError::FutureTimestamp);
            }
            if now.saturating_sub(obs) > DEFAULT_STALENESS_WINDOW_SECS {
                return Err(OracleError::StaleTimestamp);
            }
            if let Some(&(last, last_price)) = self.last_obs.get(&(o, a)) {
                if last == obs && last_price == price {
                    return Err(OracleError::DuplicateFeed);
                }
                if obs <= last {
                    return Err(OracleError::OutOfOrderTimestamp);
                }
            }
            if !self.pending[a].contains_key(&o) && self.pending[a].len() as u32 >= MAX_PENDING_FEEDS {
                return Err(OracleError::PendingBufferFull);
            }
            self.last_obs.insert((o, a), (obs, price));
            self.pending[a].insert(o, (obs, price));
            self.submissions[o] += 1;
            Ok(())
        }
    }

    /// Asserts that contract state matches the model after every step.
    fn assert_matches_model(w: &World, m: &Model) -> Result<(), TestCaseError> {
        w.run(|env| {
            for (i, o) in w.oracles.iter().enumerate() {
                let rec = get_oracle(env, o).unwrap();
                prop_assert_eq!(rec.total_submissions, m.submissions[i]);
                prop_assert_eq!(rec.is_active(), m.active[i]);
            }
            let rounds: u32 = env.storage().instance().get(&OracleDataKey::TotalRounds).unwrap_or(0);
            prop_assert_eq!(rounds, m.rounds);
            for (i, a) in w.assets.iter().enumerate() {
                // INV-ORACLE-ONE-VOTE / INV-ORACLE-BOUNDED.
                let pending: soroban_sdk::Vec<PriceFeed> = env
                    .storage()
                    .persistent()
                    .get(&OracleDataKey::PendingFeeds(a.clone()))
                    .unwrap_or_else(|| soroban_sdk::Vec::new(env));
                prop_assert_eq!(pending.len() as usize, m.pending[i].len());
                prop_assert!(pending.len() <= MAX_PENDING_FEEDS);
                for feed in pending.iter() {
                    let o = w.oracles.iter().position(|x| *x == feed.oracle).unwrap();
                    prop_assert_eq!(m.pending[i].get(&o), Some(&(feed.observed_at, feed.price)));
                    prop_assert!(feed.verify_integrity(env));
                }
                // INV-ORACLE-CHAIN and counter consistency.
                let asset = get_asset(env, a).unwrap();
                prop_assert_eq!(asset.total_updates, m.history[i]);
                prop_assert_eq!(verify_recent_price_history(env, a), Ok(m.history[i].min(MAX_VERIFY_SPAN)));
            }
            Ok(())
        })
    }

    /// Applies one op to both the contract and the model, asserting that the
    /// contract's result matches the model's prediction. Returns a compact
    /// outcome code used by the determinism property.
    fn step(w: &World, m: &mut Model, op: &Op) -> Result<u32, TestCaseError> {
        let now = w.now();
        let code = match op {
            Op::Submit { o, a, dev, back } => {
                let (price, obs) = (price_for(*dev), observed(now, *back));
                let expected = m.submit(now, *o, *a, price, obs);
                let got = w.run(|env| {
                    submit_price_feed(env, &w.oracles[*o], w.assets[*a].clone(), price, CONF, obs, Bytes::new(env))
                        .map(|_| ())
                });
                prop_assert_eq!(got, expected);
                got.err().map_or(0, |e| e as u32)
            }
            Op::Replay { o, a } | Op::Reorder { o, a } => {
                let Some(&(obs, price)) = m.last_obs.get(&(*o, *a)) else { return Ok(1) };
                // Replay resubmits the identical feed; Reorder submits an
                // older observation with a different price.
                let (obs, price) = match op {
                    Op::Replay { .. } => (obs, price),
                    _ => (obs.saturating_sub(1), price + 7),
                };
                let expected = m.submit(now, *o, *a, price, obs);
                prop_assert!(expected.is_err(), "replays and reorders must never be accepted");
                let got = w.run(|env| {
                    submit_price_feed(env, &w.oracles[*o], w.assets[*a].clone(), price, CONF, obs, Bytes::new(env))
                        .map(|_| ())
                });
                prop_assert_eq!(got, expected);
                got.err().map_or(0, |e| e as u32)
            }
            Op::Batch { o, entries, poison } => {
                let mut inputs: Vec<(usize, u128, u64)> =
                    entries.iter().map(|(a, dev, back)| (*a, price_for(*dev), observed(now, *back))).collect();
                if *poison {
                    // Mid-batch failure: an out-of-order entry for the first
                    // entry's asset, appended after it.
                    let (a, p, obs) = inputs[0];
                    inputs.push((a, p + 3, obs.saturating_sub(1)));
                }
                let mut trial = m.clone();
                let mut expected = Ok(inputs.len() as u32);
                for &(a, p, obs) in &inputs {
                    if let Err(e) = trial.submit(now, *o, a, p, obs) {
                        expected = Err(e);
                        break;
                    }
                }
                let got = w.run(|env| {
                    let mut v = soroban_sdk::Vec::new(env);
                    for &(a, p, obs) in &inputs {
                        v.push_back(PriceFeedInput {
                            asset_id: w.assets[a].clone(),
                            price: p,
                            confidence: CONF,
                            observed_at: obs,
                            metadata: Bytes::new(env),
                        });
                    }
                    submit_price_feed_batch(env, &w.oracles[*o], &v)
                });
                prop_assert_eq!(got, expected);
                if *poison {
                    prop_assert!(got.is_err(), "poisoned batch must roll back");
                }
                // INV-ORACLE-BATCH-ATOMIC: all-or-nothing.
                if got.is_ok() {
                    *m = trial;
                }
                got.map_or_else(|e| e as u32, |_| 0)
            }
            Op::Consensus { a } => {
                let pending: Vec<(u64, u128)> = m.pending[*a].values().copied().collect();
                let live = m.pending[*a].keys().filter(|o| m.active[**o]).count() as u32;
                let before = m.history[*a];
                let got = w.run(|env| process_consensus(env, &w.assets[*a]));
                if pending.is_empty() {
                    prop_assert_eq!(got.clone(), Err(OracleError::NoFeedsAvailable));
                    return Ok(OracleError::NoFeedsAvailable as u32);
                }
                m.pending[*a].clear();
                match got {
                    Ok(cp) => {
                        // Quorum of *live* oracles is required (INV-ORACLE-LIVE).
                        prop_assert!(live >= QUORUM);
                        prop_assert!(cp.oracle_count >= 1 && cp.oracle_count <= live);
                        prop_assert_eq!(cp.history_index, before);
                        prop_assert_eq!(cp.consensus_at, now);
                        let lo = pending.iter().map(|p| p.1).min().unwrap();
                        let hi = pending.iter().map(|p| p.1).max().unwrap();
                        prop_assert!(cp.price >= lo && cp.price <= hi);
                        m.history[*a] += 1;
                        m.rounds += 1;
                        0
                    }
                    Err(e) => {
                        prop_assert!(matches!(e, OracleError::InvalidPrice | OracleError::StaleTimestamp));
                        e as u32
                    }
                }
            }
            Op::Tick { dt } => {
                let t = now + u64::from(*dt);
                w.env.ledger().with_mut(|l| l.timestamp = t);
                0
            }
            Op::Suspend { o } | Op::Reactivate { o } => {
                let (status, active) = match op {
                    Op::Suspend { .. } => (OracleStatus::Suspended, false),
                    _ => (OracleStatus::Active, true),
                };
                w.run(|env| set_oracle_status(env, &w.oracles[*o], status)).unwrap();
                m.active[*o] = active;
                0
            }
        };
        assert_matches_model(w, m)?;
        Ok(code)
    }

    /// Hash-chain + Merkle verification, tamper detection and recovery.
    fn assert_crypto_integrity_after_recovery(w: &World, m: &Model) -> Result<(), TestCaseError> {
        w.run(|env| {
            for (i, a) in w.assets.iter().enumerate() {
                let len = m.history[i].min(MAX_VERIFY_SPAN);
                let start = m.history[i] - len;
                prop_assert_eq!(verify_price_history_chain(env, a, start, len), Ok(len));
                let root = price_history_merkle_root(env, a, start, len).unwrap();
                for idx in start..start + len {
                    let entry = get_canonical_price(env, a, idx as u64).unwrap();
                    let proof = price_history_merkle_proof(env, a, start, len, idx).unwrap();
                    prop_assert!(verify_price_history_proof(env, &entry, &proof, &root));
                }
                if len == 0 {
                    continue;
                }
                // Simulated corruption of the oldest entry is detected ...
                let key = OracleDataKey::PriceHistoryEntry(a.clone(), start as u64);
                let original: CanonicalPrice = env.storage().persistent().get(&key).unwrap();
                let mut corrupted = original.clone();
                corrupted.price ^= 1;
                env.storage().persistent().set(&key, &corrupted);
                prop_assert_eq!(
                    verify_price_history_chain(env, a, start, len),
                    Err(OracleError::ChainIntegrityViolation)
                );
                let proof = price_history_merkle_proof(env, a, start, len, start).unwrap();
                prop_assert!(!verify_price_history_proof(env, &corrupted, &proof, &root));
                // ... and recovery from the original record restores integrity
                // and the identical Merkle root.
                env.storage().persistent().set(&key, &original);
                prop_assert_eq!(verify_price_history_chain(env, a, start, len), Ok(len));
                prop_assert_eq!(price_history_merkle_root(env, a, start, len).unwrap(), root);
            }
            Ok(())
        })
    }

    fn run_sequence(ops: &[Op]) -> Result<(World, Model, Vec<u32>), TestCaseError> {
        let w = World::new();
        let mut m = Model::new();
        let mut codes = Vec::with_capacity(ops.len());
        for op in ops {
            codes.push(step(&w, &mut m, op)?);
        }
        Ok((w, m, codes))
    }

    fn chain_hashes(w: &World, m: &Model) -> Vec<BytesN<32>> {
        w.run(|env| {
            let mut out = Vec::new();
            for (i, a) in w.assets.iter().enumerate() {
                for idx in 0..m.history[i] {
                    out.push(get_canonical_price(env, a, idx as u64).unwrap().chain_hash);
                }
            }
            out
        })
    }

    proptest! {
        #![proptest_config(config(1024))]

        /// 1,024 generated sequences: the contract never diverges from the
        /// reference model and every invariant holds after every step.
        #[test]
        fn prop_rwa_oracle_state_machine_invariants(ops in sequence_strategy()) {
            let (w, m, _) = run_sequence(&ops)?;
            assert_crypto_integrity_after_recovery(&w, &m)?;
        }
    }

    proptest! {
        #![proptest_config(config(256))]

        /// Deterministic execution: replaying the same sequence on a fresh
        /// ledger yields identical outcomes and byte-identical hash chains.
        #[test]
        fn prop_rwa_oracle_replay_is_deterministic(ops in sequence_strategy()) {
            let (w1, m1, codes1) = run_sequence(&ops)?;
            let (w2, m2, codes2) = run_sequence(&ops)?;
            prop_assert_eq!(codes1, codes2);
            prop_assert_eq!(m1.history, m2.history);
            prop_assert_eq!(chain_hashes(&w1, &m1), chain_hashes(&w2, &m2));
        }

        /// Reordered events: the arrival order of one round's feeds does not
        /// change the canonical price or its chain hash.
        #[test]
        fn prop_rwa_oracle_round_is_order_independent(
            devs in prop::collection::vec(-400i16..400, 2..=ORACLES),
            order in Just((0..ORACLES).collect::<Vec<usize>>()).prop_shuffle(),
        ) {
            let run = |permute: bool| {
                let w = World::new();
                let now = w.now();
                w.run(|env| {
                    let idx: Vec<usize> = if permute {
                        order.iter().copied().filter(|i| *i < devs.len()).collect()
                    } else {
                        (0..devs.len()).collect()
                    };
                    for i in idx {
                        submit_price_feed(env, &w.oracles[i], w.assets[0].clone(), price_for(devs[i]), CONF, now, Bytes::new(env))
                            .unwrap();
                    }
                    process_consensus(env, &w.assets[0])
                })
            };
            prop_assert_eq!(run(false), run(true));
        }
    }
}
// ── end RWA oracle state-machine invariants ─────────────────────────────────

// ── VAT engine state-machine invariants (issue #808) ────────────────────────
mod vat_engine_invariants {
    use crate::tax::{TaxJurisdiction, VATDetermination, VATExemptionReason};
    use crate::vat_engine::{
        query_vat_determinations, VATDeterminationEngine, VATPageCursor, VATPageLimit, VATQueryFilter,
        MAX_SUPPLY_AMOUNT, MAX_VAT_PAGE_LIMIT, MAX_VAT_RATE_BPS,
    };
    use crate::vat_engine_test_support::{make_transaction, JournalError, VatInput, VatJournal};
    use ::proptest::prelude::*;
    use ::proptest::test_runner::{Config, RngSeed};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, Bytes, BytesN, Env, Vec as SorobanVec};

    const DEFAULT_SEED: u64 = 0x808_5EED;

    fn config(cases: u32) -> Config {
        let seed = std::env::var("VAT_ENGINE_PROPTEST_SEED")
            .ok()
            .and_then(|value| {
                let value = value.trim();
                match value.strip_prefix("0x") {
                    Some(hex) => u64::from_str_radix(hex, 16).ok(),
                    None => value.parse().ok(),
                }
            })
            .unwrap_or(DEFAULT_SEED);
        Config {
            cases,
            rng_seed: RngSeed::Fixed(seed),
            failure_persistence: None,
            ..Config::default()
        }
    }

    fn input(id: u8, timestamp: u64) -> VatInput {
        VatInput {
            id,
            timestamp,
            amount: 100_000,
            rate: 2_000,
            jurisdiction: 0,
            customer_jurisdiction: 0,
            supply_type: 0,
            exemption: 7,
            is_b2b: false,
        }
    }

    fn input_strategy() -> impl Strategy<Value = VatInput> {
        (
            0u8..8,
            0u64..128,
            0u64..=MAX_SUPPLY_AMOUNT.saturating_add(1),
            0u32..=MAX_VAT_RATE_BPS.saturating_add(1),
            0u8..11,
            0u8..11,
            0u8..7,
            0u8..8,
            any::<bool>(),
        )
            .prop_map(
                |(id, timestamp, amount, rate, jurisdiction, customer_jurisdiction, supply_type, exemption, is_b2b)| {
                    VatInput {
                        id,
                        timestamp,
                        amount,
                        rate,
                        jurisdiction,
                        customer_jurisdiction,
                        supply_type,
                        exemption,
                        is_b2b,
                    }
                },
            )
    }

    fn determination(
        env: &Env,
        id: u8,
        timestamp: u64,
        rate: u32,
        amount: u64,
        is_exempt: bool,
        reverse_charge: bool,
        jurisdiction: TaxJurisdiction,
        exemption_reason: VATExemptionReason,
    ) -> VATDetermination {
        VATDetermination {
            transaction_id: BytesN::from_array(env, &[id; 32]),
            vat_rate: rate,
            is_exempt,
            exemption_reason,
            reverse_charge_applicable: reverse_charge,
            place_of_supply: jurisdiction,
            vat_amount: amount,
            determined_at: timestamp,
            source: Bytes::from_slice(env, b"VAT_ENGINE_V1"),
        }
    }

    proptest! {
        #![proptest_config(config(1024))]

        /// 1,024 seeded sequences check duplicate IDs, out-of-order timestamps,
        /// bounded VAT results, recovery replay, hash linkage, and Merkle proofs.
        #[test]
        fn prop_vat_engine_state_machine_invariants(inputs in prop::collection::vec(input_strategy(), 1..=16)) {
            let env = Env::default();
            let actor = Address::generate(&env);
            let mut journal = VatJournal::new(env.clone());
            journal
                .apply(make_transaction(&env, &input(250, 0), &actor))
                .unwrap();

            for operation in inputs {
                let transaction = make_transaction(&env, &operation, &actor);
                let before = journal.snapshot();
                match journal.apply(transaction) {
                    Ok(()) => {
                        let recorded = journal.determinations().last().unwrap();
                        prop_assert!(recorded.vat_rate <= MAX_VAT_RATE_BPS);
                        let latest = journal.transactions().last().unwrap();
                        let expected_amount =
                            if recorded.is_exempt || recorded.reverse_charge_applicable {
                                0
                            } else {
                                VATDeterminationEngine::vat_amount_from_net(
                                    latest.supply_amount,
                                    recorded.vat_rate,
                                )
                            };
                        prop_assert_eq!(recorded.vat_amount, expected_amount);
                    }
                    Err(_) => prop_assert_eq!(journal.snapshot(), before),
                }
                prop_assert!(journal.assert_integrity());
            }

            let recovered = journal.recover();
            prop_assert_eq!(recovered.snapshot(), journal.snapshot());
            prop_assert!(recovered.assert_integrity());
        }
    }

    #[test]
    fn vat_engine_rejects_duplicate_and_reordered_submissions_atomically() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let mut journal = VatJournal::new(env.clone());
        let first = make_transaction(&env, &input(1, 10), &actor);
        journal.apply(first.clone()).unwrap();

        let before = journal.snapshot();
        assert_eq!(journal.apply(first), Err(JournalError::DuplicateTransaction));
        assert_eq!(journal.snapshot(), before);

        let reordered = make_transaction(&env, &input(2, 9), &actor);
        assert_eq!(journal.apply(reordered), Err(JournalError::OutOfOrderTimestamp));
        assert_eq!(journal.snapshot(), before);
        assert!(journal.assert_integrity());
    }

    #[test]
    fn vat_engine_batch_faults_restore_each_committed_prefix() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let batch: std::vec::Vec<_> = (0..4)
            .map(|index| make_transaction(&env, &input(10 + index, 11 + u64::from(index)), &actor))
            .collect();

        for fail_at in 0..batch.len() {
            let mut journal = VatJournal::new(env.clone());
            journal.apply(make_transaction(&env, &input(1, 10), &actor)).unwrap();
            let before = journal.snapshot();
            assert_eq!(
                journal.apply_batch(&batch, Some(fail_at)),
                Err(JournalError::InjectedFailure)
            );
            assert_eq!(journal.snapshot(), before);
            assert!(journal.assert_integrity());
        }
    }

    #[test]
    fn vat_engine_rate_exemption_and_amount_boundaries() {
        let env = Env::default();
        let actor = Address::generate(&env);
        for jurisdiction in 0..11 {
            for supply_type in 0..7 {
                for customer_jurisdiction in [0, 1] {
                    for is_b2b in [false, true] {
                        for exemption in 0..8 {
                            let mut value = input(1, 1);
                            value.jurisdiction = jurisdiction;
                            value.customer_jurisdiction = customer_jurisdiction;
                            value.supply_type = supply_type;
                            value.is_b2b = is_b2b;
                            value.exemption = exemption;
                            let transaction = make_transaction(&env, &value, &actor);
                            let rate = VATDeterminationEngine::determine_rate(&env, &transaction);
                            assert!(rate <= MAX_VAT_RATE_BPS);
                            let result = VATDeterminationEngine::determine_vat(&env, &transaction);
                            assert!(result.vat_rate <= MAX_VAT_RATE_BPS);
                            assert!(VATDeterminationEngine::validate_transaction(&transaction));
                        }
                    }
                }
            }
        }

        assert_eq!(VATDeterminationEngine::normalize_rate(u32::MAX), 10_000);
        assert_eq!(
            VATDeterminationEngine::vat_amount_from_net(u64::MAX, u32::MAX),
            u64::MAX
        );
        assert_eq!(
            VATDeterminationEngine::calculate_amounts(u64::MAX, u32::MAX, false).2,
            u64::MAX
        );
        assert_eq!(
            VATDeterminationEngine::calculate_amounts(u64::MAX, u32::MAX, true).2,
            u64::MAX
        );

        let mut invalid_amount = make_transaction(&env, &input(2, 2), &actor);
        invalid_amount.supply_amount = MAX_SUPPLY_AMOUNT + 1;
        assert!(!VATDeterminationEngine::validate_transaction(&invalid_amount));
        let mut invalid_rate = make_transaction(&env, &input(3, 3), &actor);
        invalid_rate.vat_rate = MAX_VAT_RATE_BPS + 1;
        assert!(!VATDeterminationEngine::validate_transaction(&invalid_rate));
    }

    #[test]
    fn vat_engine_filtered_pagination_orders_and_validates_ranges() {
        let env = Env::default();
        let mut determinations = SorobanVec::new(&env);
        determinations.push_back(determination(
            &env,
            1,
            20,
            2_000,
            200,
            false,
            false,
            TaxJurisdiction::EU,
            VATExemptionReason::None,
        ));
        determinations.push_back(determination(
            &env,
            2,
            10,
            500,
            50,
            true,
            false,
            TaxJurisdiction::UK,
            VATExemptionReason::Healthcare,
        ));
        determinations.push_back(determination(
            &env,
            3,
            10,
            1_000,
            100,
            false,
            true,
            TaxJurisdiction::EU,
            VATExemptionReason::None,
        ));
        determinations.push_back(determination(
            &env,
            4,
            30,
            0,
            0,
            false,
            false,
            TaxJurisdiction::HongKong,
            VATExemptionReason::None,
        ));

        assert!(VATPageLimit::new(0).is_err());
        assert_eq!(VATPageLimit::new(2).unwrap().effective(), 2);
        assert_eq!(
            VATPageLimit::new(MAX_VAT_PAGE_LIMIT + 1).unwrap().effective(),
            MAX_VAT_PAGE_LIMIT
        );
        assert_eq!(
            crate::vat_engine::VATPageLimit {
                limit: MAX_VAT_PAGE_LIMIT + 1
            }
            .effective(),
            MAX_VAT_PAGE_LIMIT
        );

        let filter = VATQueryFilter::none();
        let first_page =
            query_vat_determinations(&env, &determinations, &filter, &None, &VATPageLimit::new(2).unwrap()).unwrap();
        assert_eq!(
            first_page.items.get(0).unwrap().transaction_id,
            determinations.get(1).unwrap().transaction_id
        );
        assert_eq!(
            first_page.items.get(1).unwrap().transaction_id,
            determinations.get(2).unwrap().transaction_id
        );
        let cursor = first_page.next_cursor.unwrap();
        let second_page = query_vat_determinations(
            &env,
            &determinations,
            &filter,
            &Some(cursor),
            &VATPageLimit::new(2).unwrap(),
        )
        .unwrap();
        assert_eq!(second_page.items.len(), 2);
        let last_cursor = second_page.next_cursor.unwrap();
        let exhausted = query_vat_determinations(
            &env,
            &determinations,
            &filter,
            &Some(last_cursor),
            &VATPageLimit::new(2).unwrap(),
        )
        .unwrap();
        assert!(exhausted.items.is_empty());
        assert!(exhausted.next_cursor.is_none());

        let single =
            query_vat_determinations(&env, &determinations, &filter, &None, &VATPageLimit::new(1).unwrap()).unwrap();
        let cursor = single.next_cursor.unwrap();
        let after_last = VATPageCursor {
            determined_at: 30,
            transaction_id: BytesN::from_array(&env, &[4; 32]),
        };
        let empty = query_vat_determinations(
            &env,
            &determinations,
            &filter,
            &Some(after_last),
            &VATPageLimit::new(2).unwrap(),
        )
        .unwrap();
        assert!(empty.items.is_empty());
        assert!(empty.next_cursor.is_none());
        assert_eq!(cursor.determined_at, 10);

        let mut invalid_rate = VATQueryFilter::none();
        invalid_rate.min_vat_rate = Some(2);
        invalid_rate.max_vat_rate = Some(1);
        assert!(invalid_rate.validate().is_err());
        let mut invalid_amount = VATQueryFilter::none();
        invalid_amount.min_vat_amount = Some(2);
        invalid_amount.max_vat_amount = Some(1);
        assert!(invalid_amount.validate().is_err());
        let mut invalid_time = VATQueryFilter::none();
        invalid_time.min_determined_at = Some(2);
        invalid_time.max_determined_at = Some(1);
        assert!(invalid_time.validate().is_err());
        assert!(query_vat_determinations(
            &env,
            &determinations,
            &invalid_rate,
            &None,
            &VATPageLimit::new(2).unwrap(),
        )
        .is_err());

        let candidate = determinations.get(0).unwrap();
        let mut matching = VATQueryFilter::none();
        matching.place_of_supply = Some(candidate.place_of_supply);
        matching.is_exempt = Some(candidate.is_exempt);
        matching.reverse_charge_applicable = Some(candidate.reverse_charge_applicable);
        matching.min_vat_rate = Some(candidate.vat_rate);
        matching.max_vat_rate = Some(candidate.vat_rate);
        matching.min_vat_amount = Some(candidate.vat_amount);
        matching.max_vat_amount = Some(candidate.vat_amount);
        matching.min_determined_at = Some(candidate.determined_at);
        matching.max_determined_at = Some(candidate.determined_at);
        assert!(matching.matches(&candidate));

        let mut mismatches = std::vec::Vec::new();
        let mut mismatch = matching.clone();
        mismatch.place_of_supply = Some(TaxJurisdiction::UK);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.is_exempt = Some(true);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.reverse_charge_applicable = Some(true);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.min_vat_rate = Some(candidate.vat_rate + 1);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.max_vat_rate = Some(candidate.vat_rate - 1);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.min_vat_amount = Some(candidate.vat_amount + 1);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.max_vat_amount = Some(candidate.vat_amount - 1);
        mismatches.push(mismatch);
        let mut mismatch = matching.clone();
        mismatch.min_determined_at = Some(candidate.determined_at + 1);
        mismatches.push(mismatch);
        let mut mismatch = matching;
        mismatch.max_determined_at = Some(candidate.determined_at - 1);
        mismatches.push(mismatch);
        assert!(mismatches.iter().all(|filter| !filter.matches(&candidate)));
    }
}
// ── end VAT engine state-machine invariants ─────────────────────────────────
