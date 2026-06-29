#![cfg(test)]

extern crate std;

use super::*;
use rand::prelude::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Bytes, Symbol, BytesN};

const FUZZ_ITERATIONS: usize = 10_000;
const MAX_METADATA_LEN: usize = 1024;
const MAX_EVENT_TYPE_LEN: usize = 32;

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &1000);
    (env, owner, client)
}

fn random_symbol(env: &Env, rng: &mut StdRng) -> Symbol {
    let len = rng.gen_range(0..=MAX_EVENT_TYPE_LEN);
    let symbol_text: String = (0..len)
        .map(|_| match rng.gen_range(0..=5) {
            0 => rng.gen_range(b'a'..=b'z') as char,
            1 => rng.gen_range(b'A'..=b'Z') as char,
            2 => rng.gen_range(b'0'..=b'9') as char,
            3 => '-'.into(),
            4 => '_'.into(),
            _ => ' ',
        })
        .collect();
    Symbol::new(env, &symbol_text)
}

fn random_bytes(env: &Env, rng: &mut StdRng, max_len: usize) -> Bytes {
    let len = rng.gen_range(0..=max_len);
    let mut buf = Vec::with_capacity(len);
    for _ in 0..len {
        buf.push(rng.gen());
    }
    Bytes::from_slice(env, &buf)
}

fn random_event_id(env: &Env, rng: &mut StdRng) -> BytesN<32> {
    let mut raw = [0u8; 32];
    rng.fill_bytes(&mut raw);
    BytesN::from_array(env, &raw)
}

fn random_address(env: &Env, rng: &mut StdRng) -> Address {
    Address::generate(env)
}

#[test]
fn fuzz_log_event_random_inputs() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(1_000);
    let mut rng = StdRng::seed_from_u64(0xfeed_beef);

    for _ in 0..FUZZ_ITERATIONS {
        let event_type = random_symbol(&env, &mut rng);
        let metadata = random_bytes(&env, &mut rng, MAX_METADATA_LEN);
        let submitter = random_address(&env, &mut rng);

        let result = client.try_log_event(&submitter, &event_type, &metadata);
        if let Ok(id) = result {
            let fetched = client.get_event(&id);
            assert_eq!(fetched.event_type, event_type);
            assert_eq!(fetched.submitter, submitter);
            assert_eq!(fetched.metadata, metadata);
        }
    }
}

#[test]
fn fuzz_get_event_random_indices() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(1_000);
    let mut rng = StdRng::seed_from_u64(0xcafe_f00d);

    let submitter = random_address(&env, &mut rng);
    let event_type = random_symbol(&env, &mut rng);
    client.try_log_event(&submitter, &event_type, &random_bytes(&env, &mut rng, 16)).ok();

    for _ in 0..FUZZ_ITERATIONS {
        let id = random_event_id(&env, &mut rng);
        let _ = client.try_get_event(&id);
    }
}

#[test]
fn fuzz_get_event_by_type_random_inputs() {
    let (env, _owner, client) = create_ledger();
    env.ledger().set_timestamp(1_000);
    let mut rng = StdRng::seed_from_u64(0x1234_5678);

    let event_type = random_symbol(&env, &mut rng);
    let submitter = random_address(&env, &mut rng);
    client.try_log_event(&submitter, &event_type, &random_bytes(&env, &mut rng, 16)).ok();

    for _ in 0..FUZZ_ITERATIONS {
        let symbol = random_symbol(&env, &mut rng);
        let index = rng.gen_range(0..=10);
        let _ = client.try_get_event_by_type(&symbol, &index);
    }
}

#[test]
fn fuzz_governance_random_addresses_and_values() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1_000);
    let mut rng = StdRng::seed_from_u64(0xdead_beef);

    for _ in 0..FUZZ_ITERATIONS {
        let choice = rng.gen_range(0..4);
        let event_type = random_symbol(&env, &mut rng);
        let cap = rng.gen_range(0..=10);
        let candidate_owner = random_address(&env, &mut rng);
        match choice {
            0 => {
                let _ = client.try_set_global_max_logs(&owner, &cap);
            }
            1 => {
                let _ = client.try_set_event_max_logs(&owner, &event_type, &cap);
            }
            2 => {
                let _ = client.try_remove_event_cap(&owner, &event_type);
            }
            _ => {
                if candidate_owner != owner {
                    let _ = client.try_transfer_ownership(&owner, &candidate_owner);
                }
            }
        }
    }
}
