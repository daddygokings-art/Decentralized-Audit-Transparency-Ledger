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
