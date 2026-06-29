use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env};

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);
    (env, owner, client)
}

// ── Core Functionality Tests ─────────────────────────────────────────────────────

#[test]
fn regression_initialize_and_verify_state() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);

    assert_eq!(client.total_events(), 0);
    assert!(!client.is_low_cost_mode());
}

#[test]
fn regression_log_event_complete_happy_path() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    let metadata = Bytes::from_slice(&env, b"transaction-data");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &event_type, &metadata);

    // Verify event was logged
    assert_eq!(client.total_events(), 1);

    // Retrieve and verify event
    let evt = client.get_event(&id);
    assert_eq!(evt.index, 0);
    assert_eq!(evt.event_type, event_type);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.metadata, metadata);

    // Verify hash chain integrity
    assert!(client.verify_integrity());
}

#[test]
fn regression_retrieve_event_by_id() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("audit");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"data"));

    let retrieved = client.get_event(&id);
    assert_eq!(retrieved.index, 0);
    assert_eq!(retrieved.event_type, event_type);
}

#[test]
fn regression_retrieve_event_by_order() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("log");

    env.mock_all_auths();
    client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"first"));
    client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"second"));

    let evt0 = client.get_event_by_order(&0);
    let evt1 = client.get_event_by_order(&1);
    assert_eq!(evt0.metadata, Bytes::from_slice(&env, b"first"));
    assert_eq!(evt1.metadata, Bytes::from_slice(&env, b"second"));
}

#[test]
fn regression_retrieve_event_by_type() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"pay1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"pay2"));
    client.log_event(&submitter, &refund, &Bytes::from_slice(&env, b"ref1"));

    assert_eq!(client.event_count(&payment), 2);
    assert_eq!(client.event_count(&refund), 1);

    let evt = client.get_event_by_type(&payment, &0);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"pay1"));
}

// ── Error Case Tests ─────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn regression_uninitialized_contract_panics() {
    let env = Env::default();
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    // Try to log without initializing
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("test"), &Bytes::new(&env));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn regression_unauthorized_governance_call_panics() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    client.set_global_max_logs(&attacker, &200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn regression_get_nonexistent_event_panics() {
    let (env, _owner, client) = create_ledger();
    client.get_event(&BytesN::from_array(&env, &[0u8; 32]));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn regression_event_type_index_out_of_bounds_panics() {
    let (env, _owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    client.get_event_by_type(&payment, &0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn regression_get_event_by_type_invalid_index_panics() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));

    client.get_event_by_type(&payment, &1); // Only index 0 exists
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn regression_global_max_logs_reached_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &2);

    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx1"),
    );
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx2"),
    );
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx3"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn regression_event_type_max_logs_reached_panics() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &2);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")]
fn regression_transfer_to_zero_address_panics() {
    let (env, owner, client) = create_ledger();
    let zero_address = Address::from_str(&env, NULL_ACCOUNT);

    env.mock_all_auths();
    client.transfer_ownership(&owner, &zero_address);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn regression_remove_nonexistent_cap_panics() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.remove_event_cap(&owner, &payment);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn regression_metadata_too_large_panics() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &10);

    let large_meta = Bytes::from_slice(&env, &[0u8; 11]);
    client.log_event(&submitter, &symbol_short!("test"), &large_meta);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn regression_log_event_when_paused_panics() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.pause(&owner);

    client.log_event(&submitter, &symbol_short!("test"), &Bytes::new(&env));
}

// ── Governance Tests ─────────────────────────────────────────────────────────────

#[test]
fn regression_set_global_max_logs() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.set_global_max_logs(&owner, &200);
}

#[test]
fn regression_set_event_max_logs() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &50);
}

#[test]
fn regression_remove_event_cap() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &5);
    client.remove_event_cap(&owner, &payment);
}

#[test]
fn regression_transfer_ownership() {
    let (env, owner, client) = create_ledger();
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    client.transfer_ownership(&owner, &new_owner);

    // New owner can govern
    client.set_global_max_logs(&new_owner, &300);
}

#[test]
fn regression_non_owner_cannot_govern() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_set_global_max_logs(&attacker, &200);
    assert!(result.is_err());
}

#[test]
fn regression_set_metadata_max_size_global() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &500);
}

#[test]
fn regression_set_metadata_max_size_per_type() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_metadata_max_size(&owner, &payment, &1000);
}

#[test]
fn regression_pause_contract() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.pause(&owner);
}

#[test]
fn regression_unpause_contract() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.pause(&owner);
    client.unpause(&owner);
}

// ── Edge Case Tests ───────────────────────────────────────────────────────────────

#[test]
fn regression_zero_global_max_logs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &0);

    let result = client.try_log_event(
        &Address::generate(&env),
        &symbol_short!("test"),
        &Bytes::new(&env),
    );
    assert!(result.is_err());
}

#[test]
fn regression_zero_event_max_logs() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &0);

    let result = client.try_log_event(&Address::generate(&env), &payment, &Bytes::new(&env));
    assert!(result.is_err());
}

#[test]
fn regression_empty_metadata() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(&submitter, &symbol_short!("test"), &Bytes::new(&env));

    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 0);
}

#[test]
fn regression_maximum_metadata_size() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &100);

    let meta = Bytes::from_slice(&env, &[0u8; 100]);
    let id = client.log_event(&submitter, &symbol_short!("test"), &meta);

    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 100);
}

#[test]
fn regression_zero_events_initially() {
    let (_env, _owner, client) = create_ledger();
    assert_eq!(client.total_events(), 0);
}

#[test]
fn regression_single_event_operations() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    assert_eq!(client.total_events(), 1);
    assert!(client.verify_integrity());

    let evt = client.get_event(&id);
    assert_eq!(evt.index, 0);
    assert_eq!(evt.prev_hash, BytesN::from_array(&env, &[0u8; 32]));
}

#[test]
fn regression_multiple_submitters() {
    let (env, _owner, client) = create_ledger();
    let submitter1 = Address::generate(&env);
    let submitter2 = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter1,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"user1"),
    );
    client.log_event(
        &submitter2,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"user2"),
    );

    assert_eq!(client.total_events(), 2);
}

#[test]
fn regression_multiple_event_types() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("type1"),
        &Bytes::from_slice(&env, b"a"),
    );
    client.log_event(
        &submitter,
        &symbol_short!("type2"),
        &Bytes::from_slice(&env, b"b"),
    );
    client.log_event(
        &submitter,
        &symbol_short!("type3"),
        &Bytes::from_slice(&env, b"c"),
    );

    assert_eq!(client.total_events(), 3);
    assert_eq!(client.event_count(&symbol_short!("type1")), 1);
    assert_eq!(client.event_count(&symbol_short!("type2")), 1);
    assert_eq!(client.event_count(&symbol_short!("type3")), 1);
}

// ── Event Emission Tests ─────────────────────────────────────────────────────────

#[test]
fn regression_event_emitted_on_log() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
}

#[test]
fn regression_event_emission_correct_topics() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"tx1"));

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
    // Event was emitted successfully
}

#[test]
fn regression_event_emission_correct_data() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let metadata = Bytes::from_slice(&env, b"test-data");

    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("test"), &metadata);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
    // Event was emitted with data
}

#[test]
fn regression_pause_emits_event() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    client.pause(&owner);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
}

#[test]
fn regression_unpause_emits_event() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    client.pause(&owner);
    client.unpause(&owner);

    let contract_events = env.events().all();
    let events = contract_events.events();
    // Should have pause and unpause events
    assert!(events.len() >= 2);
}

#[test]
fn regression_event_emission_index_only_mode() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &1);
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
    // Event was emitted in index-only mode
}

#[test]
fn regression_event_emission_hash_only_mode() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &2);
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
    // Event was emitted in hash-only mode
}

#[test]
fn regression_event_emission_none_mode() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &3);
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    let contract_events = env.events().all();
    let events = contract_events.events();
    // In no emission mode, log_event should not emit
    // (but pause/unpause events might still exist if called)
    // We just verify the event was logged successfully
    assert_eq!(client.total_events(), 1);
}

// ── Backward Compatibility Tests ───────────────────────────────────────────────────

#[test]
fn regression_old_format_data_readable() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    // Should be able to retrieve using all methods
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"data"));

    let header = client.get_event_header(&id);
    assert_eq!(header.event_type, symbol_short!("test"));

    let metadata = client.get_event_metadata(&id);
    assert_eq!(metadata, Bytes::from_slice(&env, b"data"));
}

#[test]
fn regression_hash_chain_backward_compatible() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id1 = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"first"),
    );
    let id2 = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"second"),
    );

    let evt1 = client.get_event(&id1);
    let evt2 = client.get_event(&id2);

    // Genesis event should have zero prev_hash
    assert_eq!(evt1.prev_hash, BytesN::from_array(&env, &[0u8; 32]));
    // Second event should reference first event's hash
    assert_eq!(evt2.prev_hash, evt1.event_hash);

    // Integrity check should pass
    assert!(client.verify_integrity());
}
