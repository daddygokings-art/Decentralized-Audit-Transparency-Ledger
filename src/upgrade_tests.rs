use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Vec};

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);
    (env, owner, client)
}

fn wasm_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn log_sample_events(
    env: &Env,
    client: &AuditLedgerClient,
    submitter: &Address,
) -> Vec<BytesN<32>> {
    let mut ids = Vec::new(env);
    ids.push_back(client.log_event(
        submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(env, b"{\"reference\":\"INV-001\"}"),
    ));
    ids.push_back(client.log_event(
        submitter,
        &symbol_short!("refund"),
        &Bytes::from_slice(env, b"{\"reference\":\"RF-001\"}"),
    ));
    ids.push_back(client.log_event(
        submitter,
        &symbol_short!("audit"),
        &Bytes::from_slice(env, b"{\"reference\":\"AUD-001\"}"),
    ));
    ids
}

fn event_hashes(env: &Env, client: &AuditLedgerClient, ids: &Vec<BytesN<32>>) -> Vec<BytesN<32>> {
    let mut hashes = Vec::new(env);
    for i in 0..ids.len() {
        hashes.push_back(client.get_event(&ids.get(i).unwrap()).event_hash);
    }
    hashes
}

fn assert_events_preserved(
    client: &AuditLedgerClient,
    ids: &Vec<BytesN<32>>,
    before_hashes: &Vec<BytesN<32>>,
) {
    assert_eq!(client.total_events(), ids.len());

    for i in 0..ids.len() {
        let event = client.get_event(&ids.get(i).unwrap());
        assert_eq!(event.index, i);
        assert_eq!(event.event_hash, before_hashes.get(i).unwrap());
        assert_eq!(client.get_event_by_order(&i).event_hash, before_hashes.get(i).unwrap());
    }
}

#[test]
fn successful_upgrade_preserves_events_and_allows_new_behavior() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);

    client.upgrade_contract(&owner, &wasm_hash(&env, 7));

    assert_events_preserved(&client, &ids, &hashes);

    client.set_global_max_logs(&owner, &150);
    let new_id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"{\"reference\":\"INV-002\"}"),
    );
    let new_event = client.get_event(&new_id);
    assert_eq!(new_event.index, 3);
    assert_eq!(client.total_events(), 4);
    assert_eq!(client.event_count(&symbol_short!("payment")), 2);
}

#[test]
fn upgrade_with_invalid_wasm_hash_fails_without_corrupting_data() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);

    let result = client.try_upgrade_contract(&owner, &BytesN::from_array(&env, &[0u8; 32]));
    assert!(result.is_err());

    assert_events_preserved(&client, &ids, &hashes);
}

#[test]
fn upgrade_by_non_owner_fails_with_caller_not_owner() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);

    let result = client.try_upgrade_contract(&attacker, &wasm_hash(&env, 7));
    assert!(result.is_err());

    assert_events_preserved(&client, &ids, &hashes);
}

#[test]
fn upgrade_on_uninitialized_contract_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let result = client.try_upgrade_contract(&owner, &wasm_hash(&env, 7));
    assert!(result.is_err());
}

#[test]
fn event_hashes_are_stable_across_upgrade() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);

    client.upgrade_contract(&owner, &wasm_hash(&env, 8));

    let after_hashes = event_hashes(&env, &client, &ids);
    assert_eq!(after_hashes, hashes);
    assert_events_preserved(&client, &ids, &hashes);
}

#[test]
fn storage_key_compatibility_preserves_order_and_type_indexes() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);

    client.upgrade_contract(&owner, &wasm_hash(&env, 9));

    assert_eq!(client.get_event_by_order(&0).event_hash, hashes.get(0).unwrap());
    assert_eq!(client.get_event_by_type(&symbol_short!("payment"), &0).event_hash, hashes.get(0).unwrap());
    assert_eq!(client.get_event_by_type(&symbol_short!("refund"), &0).event_hash, hashes.get(1).unwrap());
    assert_eq!(client.get_event_by_type(&symbol_short!("audit"), &0).event_hash, hashes.get(2).unwrap());

    assert_events_preserved(&client, &ids, &hashes);
}

#[test]
fn rollback_upgrade_preserves_existing_data() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let ids = log_sample_events(&env, &client, &submitter);
    let hashes = event_hashes(&env, &client, &ids);
    let upgraded_hash = wasm_hash(&env, 10);
    let original_hash = wasm_hash(&env, 11);

    client.upgrade_contract(&owner, &upgraded_hash);
    client.upgrade_contract(&owner, &original_hash);

    assert_events_preserved(&client, &ids, &hashes);

    let post_rollback_id = client.log_event(
        &submitter,
        &symbol_short!("audit"),
        &Bytes::from_slice(&env, b"{\"reference\":\"AUD-002\"}"),
    );
    assert_eq!(client.get_event(&post_rollback_id).index, 3);
    assert_eq!(client.total_events(), 4);
}
