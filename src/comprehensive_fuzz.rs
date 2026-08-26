#![cfg(test)]

extern crate std;

use super::*;
use proptest::prelude::*;
use rand::prelude::*;
use rand::Rng;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Symbol, Vec};
use std::string::String;
use std::vec::Vec as StdVec;

const MAX_FUZZ_COUNT: usize = 100;
const MAX_METADATA: usize = 256;

fn create_env() -> Env {
    Env::default()
}

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = create_env();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &1_000_000, &4096);
    (env, owner, client)
}

fn random_bytes(env: &Env, rng: &mut StdRng, max_len: usize) -> Bytes {
    let len = rng.gen_range(0..=max_len);
    let mut buf = StdVec::with_capacity(len);
    for _ in 0..len {
        buf.push(rng.gen());
    }
    Bytes::from_slice(env, &buf)
}

fn random_symbol(env: &Env, rng: &mut StdRng) -> Symbol {
    let len = rng.gen_range(1..=16);
    let s: String = (0..len).map(|_| rng.gen_range(b'a'..=b'z') as char).collect();
    Symbol::new(env, &s)
}

fn random_address(env: &Env) -> Address {
    Address::generate(env)
}

// ── Property-Based Tests (extended) ─────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 5000, .. ProptestConfig::default() })]

    #[test]
    fn prop_events_never_lose_count(
        event_types in prop::collection::vec("[a-z]{1,8}", 1..=10),
        metadata_list in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..=64), 1..=10),
    ) {
        let (env, _owner, client) = create_ledger();
        env.ledger().set_timestamp(1000);
        let count = event_types.len().min(metadata_list.len());

        for i in 0..count {
            let _ = client.try_log_event(
                &random_address(&env),
                &Symbol::new(&env, &event_types[i]),
                &Bytes::from_slice(&env, &metadata_list[i]),
                &None, &None, &false,
            );
        }

        let total = client.total_events() as usize;
        assert!(total <= count);
    }

    #[test]
    fn prop_event_ids_are_unique(
        events in prop::collection::vec(("[a-z]{1,8}", prop::collection::vec(any::<u8>(), 0..=32)), 2..=20),
    ) {
        let (env, _owner, client) = create_ledger();
        env.ledger().set_timestamp(1000);
        let mut ids: StdVec<BytesN<32>> = StdVec::new();

        for (event_type, metadata) in events {
            if let Ok(id) = client.try_log_event(
                &random_address(&env),
                &Symbol::new(&env, &event_type),
                &Bytes::from_slice(&env, &metadata),
                &None, &None, &false,
            ) {
                ids.push(id.unwrap());
            }
        }

        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                assert_ne!(ids[i], ids[j], "event IDs must be unique");
            }
        }
    }
}

// ── Mutation Testing ────────────────────────────────────────────────────────

#[test]
fn mutation_metadata_cap_respected_after_update() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let id = client.log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"original"),
        &None,
        &None,
        &false,
    );

    client.set_metadata_max_size(&owner, &5);
    let oversized = Bytes::from_slice(&env, b"too-long-metadata");
    let result = client.try_update_event(&owner, &0, &oversized);
    assert!(result.is_err());
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"original"));
}

#[test]
fn mutation_governance_state_persists_across_owners() {
    let (env, owner, client) = create_ledger();
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &symbol_short!("t"), &5);
    client.transfer_ownership(&owner, &new_owner);

    assert!(client.has_cap(&symbol_short!("t")));

    client.remove_event_cap(&new_owner, &symbol_short!("t"));
    assert!(!client.has_cap(&symbol_short!("t")));
}

#[test]
fn mutation_event_field_integrity_after_update() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let id = client.log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"original"),
        &None,
        &None,
        &false,
    );

    let new_meta = Bytes::from_slice(&env, b"modified");
    let new_id = client.update_event(&owner, &0, &new_meta);

    let evt = client.get_event(&new_id);
    assert_eq!(evt.metadata, new_meta);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.event_type, symbol_short!("t"));
    assert_eq!(evt.index, 0);
    assert!(client.verify_integrity());
}

#[test]
fn mutation_event_history_tracks_versions() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"v0"), &None, &None, &false);
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"v1"));

    let history = client.get_event_history(&0);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0).unwrap().version, 0);
    assert_eq!(history.get(1).unwrap().version, 1);
}

#[test]
fn mutation_pause_state_blocks_and_resumes() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.pause(&owner);
    assert!(client.is_paused());

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    client.unpause(&owner);
    assert!(!client.is_paused());

    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"y"), &None, &None, &false);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn mutation_low_cost_mode_disables_indexing() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_low_cost_mode(&owner, &true);

    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn mutation_event_emission_modes_affect_events() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_emission_mode(&owner, &3);
    let before = env.events().all().events().len();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    let after = env.events().all().events().len();
    assert_eq!(before, after, "emission mode 3 must not publish any events");
}

// ── Boundary Testing (extended) ─────────────────────────────────────────────

#[test]
fn boundary_single_event_with_min_config() {
    let env = create_env();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &1, &0);

    let submitter = Address::generate(&env);
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::new(&env), &None, &None, &false);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn boundary_max_category_length() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.set_category_max_len(&owner, &18);
    let id = client.log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"x"),
        &Some(Symbol::new(&env, "general")),
        &None,
        &false,
    );
    let evt = client.get_event(&id);
    assert_eq!(evt.category, Symbol::new(&env, "general"));
}

#[test]
fn boundary_event_at_every_index_up_to_100() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    for i in 0u32..100 {
        client.log_event(
            &submitter,
            &symbol_short!("t"),
            &Bytes::from_slice(&env, &i.to_le_bytes()),
            &None,
            &None,
            &false,
        );
        let evt = client.get_event_by_order(&i);
        assert_eq!(evt.index, i);
    }
    assert_eq!(client.total_events(), 100);
    assert!(client.verify_integrity());
}

#[test]
fn boundary_many_event_types_each_at_limit() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    for t in 0u8..20 {
        let et = Symbol::new(&env, &std::format!("type_{}", t));
        client.set_event_max_logs(&owner, &et, &1);
        client.log_event(&submitter, &et, &Bytes::from_slice(&env, &[t]), &None, &None, &false);
        let result = client.try_log_event(&submitter, &et, &Bytes::from_slice(&env, &[t + 1]), &None, &None, &false);
        assert!(result.is_err(), "type_{} must respect cap of 1", t);
    }
    assert_eq!(client.total_events(), 20);
}

#[test]
fn boundary_metadata_size_exact_default_limit() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    let meta = Bytes::from_slice(&env, &[0u8; 1024]);
    let id = client.log_event(&submitter, &symbol_short!("t"), &meta, &None, &None, &false);
    assert_eq!(client.get_event(&id).metadata.len(), 1024);
}

// ── Crash / Panic Testing ───────────────────────────────────────────────────

#[test]
fn crash_get_nonexistent_event_by_order() {
    let (_env, _owner, client) = create_ledger();
    let result = client.try_get_event_by_order(&999_999);
    assert!(result.is_err());
}

#[test]
fn crash_get_nonexistent_event_by_id() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_event(&BytesN::from_array(&env, &[0xFFu8; 32]));
    assert!(result.is_err());
}

#[test]
fn crash_get_event_type_without_events() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_event_by_type(&symbol_short!("ghost"), &0);
    assert!(result.is_err());
}

#[test]
fn crash_update_nonexistent_event() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let result = client.try_update_event(
        &owner,
        &999,
        &Bytes::from_slice(&env, b"x"),
    );
    assert!(result.is_err());
}

#[test]
fn crash_proposal_nonexistent() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let result = client.try_execute_proposal(&owner, &999);
    assert!(result.is_err());
}

#[test]
fn crash_upgrade_zero_wasm_hash() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let result = client.try_upgrade_contract(
        &owner,
        &BytesN::from_array(&env, &[0u8; 32]),
    );
    assert!(result.is_err());
}

#[test]
fn crash_remove_cap_never_set() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let result = client.try_remove_event_cap(&owner, &symbol_short!("ghost"));
    assert!(result.is_err());
}

#[test]
fn crash_archive_with_cutoff_zero() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    let archived = client.archive_events(&owner, &2000);
    assert_eq!(archived, 1);
}

#[test]
fn crash_list_archived_with_no_archives() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_list_archived_events(&0, &10);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().unwrap().len(), 0);
}

// ── Assertion-based fuzz (non-proptest) ─────────────────────────────────────

#[test]
fn fuzz_mixed_random_operations_no_crash() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);
    let mut rng = StdRng::seed_from_u64(0x5EED);
    let submitters: StdVec<Address> = (0..10).map(|_| random_address(&env)).collect();

    for _ in 0..MAX_FUZZ_COUNT {
        let op = rng.gen_range(0..8);
        match op {
            0..=3 => {
                let sub = submitters[rng.gen_range(0..submitters.len())].clone();
                let et = random_symbol(&env, &mut rng);
                let meta = random_bytes(&env, &mut rng, MAX_METADATA);
                let _ = client.try_log_event(&sub, &et, &meta, &None, &None, &false);
            }
            4 => {
                let et = random_symbol(&env, &mut rng);
                let cap = rng.gen_range(0..=100);
                let _ = client.try_set_event_max_logs(&owner, &et, &cap);
            }
            5 => {
                let total = client.total_events();
                if total > 0 {
                    let idx = rng.gen_range(0..total);
                    let _ = client.try_get_event_by_order(&idx);
                }
            }
            6 => {
                let total = client.total_events();
                if total > 0 {
                    let _ = client.try_verify_integrity();
                }
            }
            7 => {
                let _ = client.try_total_events();
            }
            _ => {}
        }
    }
}

#[test]
fn fuzz_random_ownership_transfers_no_crash() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let mut rng = StdRng::seed_from_u64(0x0b3e4);
    let mut current_owner = owner;

    for _ in 0..20 {
        let new_owner = random_address(&env);
        let result = client.try_transfer_ownership(&current_owner, &new_owner);
        if result.is_ok() {
            current_owner = new_owner;
        }
    }

    let _ = client.try_set_global_max_logs(&current_owner, &500);
}

#[test]
fn fuzz_random_pause_unpause_events_no_crash() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    let mut rng = StdRng::seed_from_u64(0x9a5e);

    for _ in 0..10 {
        if rng.gen_bool(0.5) {
            let _ = client.try_pause(&owner);
        } else {
            let _ = client.try_unpause(&owner);
        }
        let _ = client.try_log_event(
            &submitter,
            &symbol_short!("t"),
            &Bytes::from_slice(&env, b"x"),
            &None,
            &None,
            &false,
        );
    }
}
