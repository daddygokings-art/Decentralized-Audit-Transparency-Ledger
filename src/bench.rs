#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Bytes, Symbol};

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100_000);
    (env, owner, client)
}

fn random_bytes(env: &Env, seed: u8, len: usize) -> Bytes {
    let data: Vec<u8> = (0..len).map(|i| seed.wrapping_add(i as u8)).collect();
    Bytes::from_slice(env, &data)
}

fn random_symbol(env: &Env, seed: u8) -> Symbol {
    let mut s = String::new();
    for i in 0..10 {
        let c = match (seed.wrapping_add(i) % 3) {
            0 => 'a',
            1 => 'b',
            _ => 'c',
        };
        s.push(c);
    }
    Symbol::new(env, &s)
}

#[test]
fn benchmark_sequential_logging_10000() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(1_000);
    let submitter = Address::generate(&env);
    let event_type = random_symbol(&env, 1);

    for i in 0..10_000u32 {
        let metadata = random_bytes(&env, (i % 255) as u8, 16);
        let result = client.try_log_event(&submitter, &event_type, &metadata);
        assert!(result.is_ok());
        env.ledger().set_timestamp(1_000 + i as u64);
    }
    assert_eq!(client.total_events(), 10_000);
}

#[test]
fn benchmark_multi_type_logging_10000() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(2_000);
    let submitter = Address::generate(&env);

    for type_idx in 0..10u8 {
        let event_type = random_symbol(&env, type_idx);
        for i in 0..1_000u32 {
            let metadata = random_bytes(&env, (type_idx.wrapping_add((i % 255) as u8)), 32);
            let result = client.try_log_event(&submitter, &event_type, &metadata);
            assert!(result.is_ok());
            env.ledger().set_timestamp(2_000 + (type_idx as u64 * 1_000) + i as u64);
        }
    }
    assert_eq!(client.total_events(), 10_000);
}

#[test]
fn benchmark_mixed_metadata_sizes() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(3_000);
    let submitter = Address::generate(&env);
    let event_type = random_symbol(&env, 99);
    let sizes = [10, 100, 1024];

    for &size in sizes.iter() {
        for i in 0..1_000u32 {
            let metadata = random_bytes(&env, (size as u8).wrapping_add(i as u8), size);
            let result = client.try_log_event(&submitter, &event_type, &metadata);
            assert!(result.is_ok());
            env.ledger().set_timestamp(3_000 + i as u64);
        }
    }
    assert_eq!(client.total_events(), 3_000);
}

#[test]
fn benchmark_concurrent_submitters() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(4_000);
    let event_type = random_symbol(&env, 50);

    for submitter_idx in 0..100u8 {
        let submitter = Address::generate(&env);
        for i in 0..100u32 {
            let metadata = random_bytes(&env, submitter_idx, 16);
            let result = client.try_log_event(&submitter, &event_type, &metadata);
            assert!(result.is_ok());
            env.ledger().set_timestamp(4_000 + submitter_idx as u64 + i as u64);
        }
    }
    assert_eq!(client.total_events(), 10_000);
}

#[test]
fn benchmark_near_capacity_logging() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(5_000);
    let submitter = Address::generate(&env);
    let event_type = random_symbol(&env, 77);

    for i in 0..9_900u32 {
        let metadata = random_bytes(&env, 1, 8);
        let result = client.try_log_event(&submitter, &event_type, &metadata);
        assert!(result.is_ok());
        env.ledger().set_timestamp(5_000 + i as u64);
    }

    for i in 9_900..9_999u32 {
        let metadata = random_bytes(&env, 2, 8);
        let result = client.try_log_event(&submitter, &event_type, &metadata);
        assert!(result.is_ok());
        env.ledger().set_timestamp(5_000 + i as u64);
    }

    assert_eq!(client.total_events(), 9_999);
}
