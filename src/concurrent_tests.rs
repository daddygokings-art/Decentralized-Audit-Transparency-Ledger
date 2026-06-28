use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Bytes, Env, Vec};

const SUBMITTERS: u32 = 5;

fn create_ledger(global_max_logs: u32) -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &global_max_logs);
    (env, owner, client)
}

fn submitters(env: &Env) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..SUBMITTERS {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn event_type_for(submitter_index: u32) -> Symbol {
    match submitter_index {
        0 => symbol_short!("payment"),
        1 => symbol_short!("refund"),
        2 => symbol_short!("audit"),
        3 => symbol_short!("check"),
        _ => symbol_short!("report"),
    }
}

fn metadata_for(env: &Env, submitter_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            submitter_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
        ],
    )
}

fn log_interleaved_round(
    env: &Env,
    client: &AuditLedgerClient,
    submitters: &Vec<Address>,
    sequence: u32,
) {
    for submitter_index in 0..SUBMITTERS {
        let submitter = submitters.get(submitter_index).unwrap();
        client.log_event(
            &submitter,
            &event_type_for(submitter_index),
            &metadata_for(env, submitter_index, sequence),
        );
    }
}

fn assert_interleaved_integrity(
    env: &Env,
    client: &AuditLedgerClient,
    submitters: &Vec<Address>,
    rounds: u32,
) {
    let expected_total = SUBMITTERS * rounds;
    assert_eq!(client.total_events(), expected_total);

    for index in 0..expected_total {
        let submitter_index = index % SUBMITTERS;
        let sequence = index / SUBMITTERS;
        let event = client.get_event_by_order(&index);

        assert_eq!(event.index, index);
        assert_eq!(event.submitter, submitters.get(submitter_index).unwrap());
        assert_eq!(event.event_type, event_type_for(submitter_index));
        assert_eq!(event.metadata, metadata_for(env, submitter_index, sequence));
    }

    for submitter_index in 0..SUBMITTERS {
        assert_eq!(client.event_count(&event_type_for(submitter_index)), rounds);
    }
}

#[test]
fn concurrent_logging_five_submitters_interleaved() {
    let (env, _owner, client) = create_ledger(600);
    let submitters = submitters(&env);

    env.mock_all_auths();
    for sequence in 0..100 {
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 100);
}

#[test]
fn concurrent_governance_cap_changes_and_logging() {
    let (env, owner, client) = create_ledger(120);
    let submitters = submitters(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &symbol_short!("payment"), &40);

    for sequence in 0..20 {
        if sequence == 5 {
            client.set_global_max_logs(&owner, &200);
        }
        if sequence == 10 {
            client.set_event_max_logs(&owner, &symbol_short!("payment"), &75);
        }
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 20);
}

#[test]
fn concurrent_ownership_transfer_and_logging() {
    let (env, owner, client) = create_ledger(150);
    let submitters = submitters(&env);
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    for sequence in 0..20 {
        if sequence == 8 {
            client.transfer_ownership(&owner, &new_owner);
        }
        if sequence == 12 {
            client.set_global_max_logs(&new_owner, &200);
        }
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 20);
}

#[test]
fn concurrent_cap_removal_and_logging_same_type() {
    let (env, owner, client) = create_ledger(150);
    let submitters = submitters(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &event_type, &12);

    for sequence in 0..20 {
        if sequence == 6 {
            client.remove_event_cap(&owner, &event_type);
        }

        let submitter_index = sequence % SUBMITTERS;
        let submitter = submitters.get(submitter_index).unwrap();
        client.log_event(
            &submitter,
            &event_type,
            &metadata_for(&env, submitter_index, sequence),
        );
    }

    assert_eq!(client.total_events(), 20);
    assert_eq!(client.event_count(&event_type), 20);
    assert!(!client.has_cap(&event_type));

    for index in 0..20 {
        let submitter_index = index % SUBMITTERS;
        let event = client.get_event_by_order(&index);
        assert_eq!(event.index, index);
        assert_eq!(event.event_type, event_type);
        assert_eq!(event.submitter, submitters.get(submitter_index).unwrap());
        assert_eq!(event.metadata, metadata_for(&env, submitter_index, index));
    }
}
