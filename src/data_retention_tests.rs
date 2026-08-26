use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Bytes, Env, Vec};

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);
    (env, owner, client)
}

#[test]
fn expired_events_are_redacted_and_recorded() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let category = Symbol::new(&env, "finance");
    let metadata = Bytes::from_slice(&env, b"account=42");

    env.ledger().set_timestamp(1_000);
    client.log_event(&submitter, &Symbol::new(&env, "transfer"), &metadata, &Some(category.clone()), &None, &false);
    client.set_retention_policy(&owner, &category, &1, &Symbol::new(&env, "gdpr"));

    env.ledger().set_timestamp(1_000 + 86_401);
    assert_eq!(client.run_retention_sweep(&owner, &0, &1), 1);

    let event = client.get_event_by_order(&0);
    assert_eq!(event.metadata, Bytes::new(&env));
    assert!(client.is_event_erased(&0));
    assert_eq!(client.get_erasure_record(&0).unwrap().original_metadata_hash, env.crypto().sha256(&metadata).into());
}

#[test]
fn legal_hold_blocks_expired_event() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let category = Symbol::new(&env, "finance");

    env.ledger().set_timestamp(1_000);
    client.log_event(&submitter, &Symbol::new(&env, "transfer"), &Bytes::from_slice(&env, b"held"), &Some(category.clone()), &None, &false);
    client.set_retention_policy(&owner, &category, &1, &Symbol::new(&env, "gdpr"));
    client.place_legal_hold(&owner, &0, &Bytes::from_slice(&env, b"investigation"));

    env.ledger().set_timestamp(1_000 + 86_401);
    assert_eq!(client.run_retention_sweep(&owner, &0, &1), 0);
    assert_eq!(client.verify_retention_compliance(&0, &1).overdue_blocked_hold, 1);
    assert!(!client.is_event_erased(&0));
}

#[test]
fn operational_events_are_not_erasable() {
    let (env, owner, client) = create_ledger();
    let recorder = Address::generate(&env);

    client.add_ops_recorder(&owner, &recorder);
    let event_id = client.log_operational_action(
        &recorder,
        &Symbol::new(&env, "ops_deploy"),
        &Bytes::from_slice(&env, b"release-1"),
    );

    let event = client.get_event(&event_id);
    assert_eq!(event.category, Symbol::new(&env, "operational"));
    assert!(client.is_ops_recorder(&recorder));
}