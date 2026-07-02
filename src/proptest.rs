#![cfg(test)]

extern crate std;

use super::*;
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
}
