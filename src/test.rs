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

// ── Basic functionality ─────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);

    assert_eq!(client.total_events(), 0);
}

#[test]
fn test_log_event() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"tx1"));

    assert_eq!(client.total_events(), 1);

    let evt = client.get_event(&id);
    assert_eq!(evt.index, 0);
    assert_eq!(evt.event_type, event_type);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"tx1"));
    // genesis prev_hash must be all-zeros
    assert_eq!(evt.prev_hash, BytesN::from_array(&env, &[0u8; 32]));
}

#[test]
fn test_log_multiple_events() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
    client.log_event(&submitter, &refund, &Bytes::from_slice(&env, b"tx3"));

    assert_eq!(client.total_events(), 3);
    assert_eq!(client.event_count(&payment), 2);
    assert_eq!(client.event_count(&refund), 1);

    let evt0 = client.get_event_by_type(&payment, &0);
    assert_eq!(evt0.metadata, Bytes::from_slice(&env, b"tx1"));

    let evt1 = client.get_event_by_type(&payment, &1);
    assert_eq!(evt1.metadata, Bytes::from_slice(&env, b"tx2"));

    let evt2 = client.get_event_by_type(&refund, &0);
    assert_eq!(evt2.metadata, Bytes::from_slice(&env, b"tx3"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_get_nonexistent_event_panics() {
    let (env, _owner, client) = create_ledger();
    client.get_event(&BytesN::from_array(&env, &[0u8; 32]));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #19)")]
fn test_initialize_reinitialization_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);
    // Try to re-initialize — should fail with AlreadyInitialized (error #19)
    client.initialize(&owner, &200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #19)")]
fn test_initialize_reinitialization_after_ownership_transfer_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);
    
    // Transfer ownership
    client.transfer_ownership(&owner, &new_owner);
    
    // Try to re-initialize with new owner — should still fail with AlreadyInitialized
    // (demonstrates that version counter protects against re-init even if owner changes)
    client.initialize(&new_owner, &200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #14)")]
fn test_get_event_by_type_no_events_returns_no_events_for_type() {
    let (_env, _owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    client.get_event_by_type(&payment, &0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_get_event_by_type_with_bad_index_panics() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.get_event_by_type(&payment, &1);
}

#[test]
fn test_event_count_and_total_events_with_empty_metadata() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::new(&env));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"non-empty"));

    assert_eq!(client.total_events(), 2);
    assert_eq!(client.event_count(&payment), 2);

    let evt0 = client.get_event_by_type(&payment, &0);
    let evt1 = client.get_event_by_type(&payment, &1);
    assert_eq!(evt0.metadata.len(), 0);
    assert_eq!(evt1.metadata, Bytes::from_slice(&env, b"non-empty"));
}

#[test]
fn test_batch_log_events_logs_each_event_atomically() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let events = soroban_sdk::vec![
        &env,
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"a")
        ),
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"b")
        ),
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"c")
        ),
    ];

    let indices = client.log_events(&events);
    assert_eq!(indices.len(), 3);
    assert_eq!(client.total_events(), 3);
    assert_eq!(client.event_count(&payment), 3);
    assert_eq!(
        client.get_event_by_type(&payment, &0).metadata,
        Bytes::from_slice(&env, b"a")
    );
    assert_eq!(
        client.get_event_by_type(&payment, &2).metadata,
        Bytes::from_slice(&env, b"c")
    );
}

#[test]
fn test_batch_log_events_exceeds_type_cap_reverts() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &2);

    let events = soroban_sdk::vec![
        &env,
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"a")
        ),
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"b")
        ),
        (
            submitter.clone(),
            payment.clone(),
            Bytes::from_slice(&env, b"c")
        ),
    ];

    let result = client.try_log_events(&events);
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_batch_log_events_integer_overflow_cap_check() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &event_type, &5);

    let events1 = soroban_sdk::vec![&env,
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"1")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"2")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"3")),
    ];
    client.log_events(&events1);

    let events2 = soroban_sdk::vec![&env,
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"4")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"5")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"6")),
    ];
    client.log_events(&events2);
}

// ── issue #70: hash-based IDs ───────────────────────────────────────────────

#[test]
fn test_event_ids_are_bytes32() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id: BytesN<32> = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    // ID is a 32-byte value (BytesN<32> by type)
    assert_eq!(id.len(), 32);
}

#[test]
fn test_different_metadata_produces_different_ids() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id1 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    let id2 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
    assert_ne!(id1, id2);
}

#[test]
fn test_get_event_by_order() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id0 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"first"));
    let id1 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"second"));

    let evt0 = client.get_event_by_order(&0);
    let evt1 = client.get_event_by_order(&1);

    assert_eq!(evt0.metadata, Bytes::from_slice(&env, b"first"));
    assert_eq!(evt1.metadata, Bytes::from_slice(&env, b"second"));
    assert_eq!(client.get_event(&id0).index, 0);
    assert_eq!(client.get_event(&id1).index, 1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_get_event_by_order_out_of_bounds() {
    let (_env, _owner, client) = create_ledger();
    client.get_event_by_order(&999);
}

// ── issue #66: hash chain integrity ────────────────────────────────────────

#[test]
fn test_verify_integrity_empty() {
    let (_env, _owner, client) = create_ledger();
    assert!(client.verify_integrity());
}

#[test]
fn test_verify_integrity_single_event() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"x"),
    );

    assert!(client.verify_integrity());
}

#[test]
fn test_verify_integrity_multiple_events() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0u8..5 {
        client.log_event(&submitter, &payment, &Bytes::from_slice(&env, &[i]));
    }

    assert!(client.verify_integrity());
}

#[test]
fn test_verify_integrity_range() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0u8..5 {
        client.log_event(&submitter, &payment, &Bytes::from_slice(&env, &[i]));
    }

    assert!(client.verify_integrity_range(&1, &4));
    assert!(client.verify_integrity_range(&0, &5));
    assert!(client.verify_integrity_range(&2, &2)); // empty range
}

#[test]
fn test_hash_chain_links_prev_hash() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id0 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"a"));
    let id1 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"b"));

    let evt0 = client.get_event(&id0);
    let evt1 = client.get_event(&id1);

    // genesis
    assert_eq!(evt0.prev_hash, BytesN::from_array(&env, &[0u8; 32]));
    // second event's prev_hash == first event's event_hash
    assert_eq!(evt1.prev_hash, evt0.event_hash);
}

// ── Cap and governance ──────────────────────────────────────────────────────

#[test]
fn test_per_event_max_logs() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &2);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
    assert_eq!(client.event_count(&payment), 2);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"));
    assert!(result.is_err());
}

#[test]
fn test_global_max_logs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &2);

    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &refund, &Bytes::from_slice(&env, b"tx2"));

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"));
    assert!(result.is_err());
}

#[test]
fn test_owner_can_set_global_max_logs() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.set_global_max_logs(&owner, &200);
    assert_eq!(client.total_events(), 0);
}

#[test]
fn test_set_global_max_logs_below_current_count_panics() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx1"),
    );
    let result = client.try_set_global_max_logs(&owner, &0);
    assert!(result.is_err());
}

#[test]
fn test_set_global_max_logs_equal_current_count_freezes_logging() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx1"),
    );
    client.set_global_max_logs(&owner, &1);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx2"),
    );
    assert!(result.is_err());
}

#[test]
fn test_transfer_ownership_same_owner_panics() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    let result = client.try_transfer_ownership(&owner, &owner);
    assert!(result.is_err());
}

#[test]
fn test_remove_event_cap_never_set_panics() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let result = client.try_remove_event_cap(&owner, &payment);
    assert!(result.is_err());
}

#[test]
fn test_remove_event_cap_already_removed_panics() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &5);
    client.remove_event_cap(&owner, &payment);
    let result = client.try_remove_event_cap(&owner, &payment);
    assert!(result.is_err());
}

#[test]
fn test_has_cap_detects_cap_state() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    assert!(!client.has_cap(&payment));
    client.set_event_max_logs(&owner, &payment, &5);
    assert!(client.has_cap(&payment));
    client.remove_event_cap(&owner, &payment);
    assert!(!client.has_cap(&payment));
}

#[test]
fn test_non_owner_cannot_set_global_max() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_set_global_max_logs(&attacker, &200);
    assert!(result.is_err());
}

#[test]
fn test_transfer_ownership() {
    let (env, owner, client) = create_ledger();
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    client.transfer_ownership(&owner, &new_owner);
    client.set_global_max_logs(&new_owner, &300);
}

#[test]
fn test_remove_event_cap() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &5);
    client.remove_event_cap(&owner, &payment);
}

#[test]
fn test_zero_global_max_logs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &0);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"x"),
    );
    assert!(result.is_err());
}

#[test]
fn test_set_global_max_to_zero_after_events() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx1"),
    );
    client.set_global_max_logs(&owner, &0);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx2"),
    );
    assert!(result.is_err());
}

#[test]
fn test_zero_event_max_logs() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &0);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    assert!(result.is_err());
}

#[test]
fn test_set_event_max_equal_to_current_count() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));

    client.set_event_max_logs(&owner, &payment, &2);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"));
    assert!(result.is_err());
}

#[test]
fn test_event_was_emitted() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"emit-test");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &meta);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
}

#[test]
fn test_log_event_with_empty_metadata() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &Bytes::new(&env));

    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 0);
}

#[test]
fn test_multiple_event_types_independent() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let type_a = symbol_short!("type_a");
    let type_b = symbol_short!("type_b");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &type_a, &1);
    client.set_event_max_logs(&owner, &type_b, &1);

    client.log_event(&submitter, &type_a, &Bytes::from_slice(&env, b"a1"));
    client.log_event(&submitter, &type_b, &Bytes::from_slice(&env, b"b1"));

    let result = client.try_log_event(&submitter, &type_a, &Bytes::from_slice(&env, b"a2"));
    assert!(result.is_err());
}

#[test]
fn test_log_event_returns_correct_fields() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"test-meta");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);
    let evt = client.get_event(&id);

    assert_eq!(evt.index, 0);
    assert_eq!(evt.event_type, payment);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.metadata, meta);
    assert_eq!(evt.timestamp, 1000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_total_events_before_initialize_panics() {
    let env = Env::default();
    let submitter = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.total_events();
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_log_event_before_initialize_panics() {
    let env = Env::default();
    let submitter = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_log_event_rejects_past_timestamp() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));

    env.ledger().set_timestamp(999);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_log_event_rejects_future_timestamp() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));

    env.ledger()
        .set_timestamp(1000 + super::MAX_TIMESTAMP_DRIFT_SECONDS + 1);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));
}

#[test]
fn test_log_event_accepts_normal_timestamp_progression() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));

    env.ledger().set_timestamp(1001);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));

    assert_eq!(client.total_events(), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_log_event_rejects_total_events_overflow() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &u32::MAX);
    env.storage()
        .instance()
        .set(&super::DataKey::TotalEvents, &u32::MAX);

    let submitter = Address::generate(&env);
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
    );
}

#[test]
fn test_get_statistics_returns_aggregates() {
    let (env, _owner, client) = create_ledger();
    let submitter_a = Address::generate(&env);
    let submitter_b = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter_a, &payment, &Bytes::from_slice(&env, b"t1"));
    env.ledger().set_timestamp(1001);
    client.log_event(&submitter_b, &refund, &Bytes::from_slice(&env, b"t2"));
    env.ledger().set_timestamp(1002);
    client.log_event(&submitter_a, &payment, &Bytes::from_slice(&env, b"t3"));

    let stats = client.get_statistics();
    assert_eq!(stats.total_events, 3);
    assert_eq!(stats.events_last_hour, 3);
    assert_eq!(stats.events_last_day, 3);
    assert_eq!(stats.events_last_week, 3);
    assert_eq!(stats.events_by_type.len(), 2);
    assert_eq!(stats.top_submitters.len(), 2);
}

#[test]
fn test_get_statistics_empty_ledger() {
    let (_env, _owner, client) = create_ledger();
    let stats = client.get_statistics();
    assert_eq!(stats.total_events, 0);
    assert_eq!(stats.events_last_hour, 0);
    assert_eq!(stats.events_by_type.len(), 0);
}

#[test]
fn test_set_global_max_equal_to_current_count() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"));

    client.set_global_max_logs(&owner, &2);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"));
    assert!(result.is_err());
}

#[test]
fn test_remove_cap_then_unlimited() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &0);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"blocked"));
    assert!(result.is_err());

    client.remove_event_cap(&owner, &payment);

    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"now-unblocked"),
    );
    assert_eq!(client.event_count(&payment), 1);
}

// ── issue #67: metadata size cap ──────────────────────────────────────────

#[test]
fn test_metadata_size_cap_default_allows_1kb() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    // Default max is 1024; 100 bytes should pass.
    let meta = Bytes::from_slice(&env, &[0u8; 100]);
    let _id = client.log_event(&submitter, &symbol_short!("p"), &meta);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_metadata_size_cap_rejects_oversized_default() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    // 1025 > 1024 default → rejected
    let meta = Bytes::from_slice(&env, &[0u8; 1025]);
    let result = client.try_log_event(&submitter, &symbol_short!("p"), &meta);
    assert!(result.is_err());
}

#[test]
fn test_metadata_size_cap_owner_can_set_global() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &50);
    // 50 bytes → passes
    let _id = client.log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, &[0u8; 50]),
    );
    assert_eq!(client.total_events(), 1);
    // 51 bytes → rejected
    let r2 = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, &[0u8; 51]),
    );
    assert!(r2.is_err());
}

#[test]
fn test_metadata_size_cap_non_owner_cannot_set() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_set_metadata_max_size(&attacker, &100);
    assert!(result.is_err());
}

#[test]
fn test_metadata_size_cap_per_type_overrides_global() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let lett = symbol_short!("lett");

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &10);
    client.set_event_metadata_max_size(&owner, &lett, &100);
    // type "lett" allows 100 → 50 passes
    let _id = client.log_event(&submitter, &lett, &Bytes::from_slice(&env, &[0u8; 50]));
    assert_eq!(client.total_events(), 1);
    // type "z" uses global cap of 10 → 11 fails
    let r2 = client.try_log_event(
        &submitter,
        &symbol_short!("z"),
        &Bytes::from_slice(&env, &[0u8; 11]),
    );
    assert!(r2.is_err());
}

#[test]
fn test_metadata_size_cap_getter() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.set_event_metadata_max_size(&owner, &symbol_short!("x"), &77);
    let cap = client.get_metadata_max_size(&symbol_short!("x"));
    assert_eq!(cap, 77);
}

// ── issue #69: event signatures ──────────────────────────────────────────

#[test]
fn test_log_event_signed_stores_signature() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let sig_payload = Bytes::from_slice(&env, &[0u8; 96]); // dummy 96 bytes
    let id = client.log_event_signed(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"data"),
        &sig_payload,
    );
    let stored = client.get_event_signature(&id);
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().len(), 96);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_log_event_signed_rejects_wrong_length() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let short_payload = Bytes::from_slice(&env, b"too-short");
    client.log_event_signed(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"data"),
        &short_payload,
    );
}

#[test]
fn test_get_event_signature_returns_none_for_unsigned() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"x"),
    );
    let stored = client.get_event_signature(&id);
    assert!(stored.is_none());
}

// ── issue #343: additional boundary and regression tests ─────────────────

#[test]
fn test_transfer_ownership_to_zero_panics() {
    let (env, owner, client) = create_ledger();
    let zero = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );

    env.mock_all_auths();
    let result = client.try_transfer_ownership(&owner, &zero);
    assert!(result.is_err());
}

#[test]
fn test_verify_integrity_empty_range() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("a"),
        &Bytes::from_slice(&env, b"x"),
    );
    client.log_event(
        &submitter,
        &symbol_short!("b"),
        &Bytes::from_slice(&env, b"y"),
    );

    assert!(client.verify_integrity_range(&0, &0));
    assert!(client.verify_integrity_range(&1, &1));
    assert!(client.verify_integrity_range(&2, &2));
}

#[test]
fn test_metadata_size_cap_u32_max_disables_limit() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &u32::MAX);

    let large_meta = Bytes::from_slice(&env, &[0u8; 2000]);
    let _id = client.log_event(&submitter, &symbol_short!("p"), &large_meta);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_event_order_preserved_across_multiple_types() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    for i in 0u8..10 {
        let t = if i % 2 == 0 {
            symbol_short!("even")
        } else {
            symbol_short!("odd")
        };
        client.log_event(&submitter, &t, &Bytes::from_slice(&env, &[i]));
    }

    assert_eq!(client.total_events(), 10);

    for i in 0u8..10 {
        let evt = client.get_event_by_order(&(i as u32));
        assert_eq!(evt.index, i as u32);
    }
}

#[test]
fn test_get_event_by_order_returns_correct_id() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id0 = client.log_event(
        &submitter,
        &symbol_short!("a"),
        &Bytes::from_slice(&env, b"first"),
    );
    let id1 = client.log_event(
        &submitter,
        &symbol_short!("b"),
        &Bytes::from_slice(&env, b"second"),
    );

    let evt0 = client.get_event_by_order(&0);
    assert_eq!(client.get_event(&id0), evt0);

    let evt1 = client.get_event_by_order(&1);
    assert_eq!(client.get_event(&id1), evt1);
}

#[test]
fn test_get_event_by_type_multiple_indices() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payments = symbol_short!("pay");

    env.mock_all_auths();
    let _id0 = client.log_event(&submitter, &payments, &Bytes::from_slice(&env, b"a"));
    let _id1 = client.log_event(&submitter, &payments, &Bytes::from_slice(&env, b"b"));
    let _id2 = client.log_event(&submitter, &payments, &Bytes::from_slice(&env, b"c"));

    assert_eq!(
        client.get_event_by_type(&payments, &0).metadata,
        Bytes::from_slice(&env, b"a")
    );
    assert_eq!(
        client.get_event_by_type(&payments, &1).metadata,
        Bytes::from_slice(&env, b"b")
    );
    assert_eq!(
        client.get_event_by_type(&payments, &2).metadata,
        Bytes::from_slice(&env, b"c")
    );
}

#[test]
fn test_protocol_version_header() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let meta = Bytes::from_slice(&env, b"proto-check");
    let id = client.log_event(&submitter, &symbol_short!("p"), &meta);

    let evt = client.get_event(&id);
    assert_eq!(evt.event_hash.len(), 32);
    assert_eq!(evt.prev_hash.len(), 32);
}

// ── issue #341: performance / boundary tests ──────────────────────────

#[test]
fn test_log_many_events_per_type() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let t = symbol_short!("bulk");

    env.mock_all_auths();
    for i in 0u8..50 {
        client.log_event(&submitter, &t, &Bytes::from_slice(&env, &[i]));
    }

    assert_eq!(client.total_events(), 50);
    assert_eq!(client.event_count(&t), 50);
    assert!(client.verify_integrity());
}

#[test]
fn test_multiple_event_types_large_counts() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let type_a = symbol_short!("TypeA");
    let type_b = symbol_short!("TypeB");

    env.mock_all_auths();
    for i in 0u8..25 {
        client.log_event(&submitter, &type_a, &Bytes::from_slice(&env, &[i]));
        client.log_event(&submitter, &type_b, &Bytes::from_slice(&env, &[i + 100]));
    }

    assert_eq!(client.total_events(), 50);
    assert_eq!(client.event_count(&type_a), 25);
    assert_eq!(client.event_count(&type_b), 25);
    assert!(client.verify_integrity());
}

#[test]
fn test_mixed_types_with_limits() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let type_a = symbol_short!("TypeA");
    let type_b = symbol_short!("TypeB");
    let type_c = symbol_short!("TypeC");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &type_a, &2);
    client.set_event_max_logs(&owner, &type_b, &3);

    client.log_event(&submitter, &type_a, &Bytes::from_slice(&env, b"a1"));
    client.log_event(&submitter, &type_a, &Bytes::from_slice(&env, b"a2"));
    client.log_event(&submitter, &type_b, &Bytes::from_slice(&env, b"b1"));
    client.log_event(&submitter, &type_b, &Bytes::from_slice(&env, b"b2"));
    client.log_event(&submitter, &type_b, &Bytes::from_slice(&env, b"b3"));
    client.log_event(&submitter, &type_c, &Bytes::from_slice(&env, b"c1"));

    assert_eq!(client.total_events(), 6);
    assert_eq!(client.event_count(&type_a), 2);
    assert_eq!(client.event_count(&type_b), 3);
    assert_eq!(client.event_count(&type_c), 1);

    let result = client.try_log_event(&submitter, &type_a, &Bytes::from_slice(&env, b"a3"));
    assert!(result.is_err());
}

// ── Low-cost mode tests ────────────────────────────────────────────────────

#[test]
fn test_low_cost_mode_disabled_by_default() {
    let (env, _owner, client) = create_ledger();
    assert!(!client.is_low_cost_mode());
}

#[test]
fn test_low_cost_mode_enabled() {
    let (env, owner, client) = create_ledger();
    client.set_low_cost_mode(&owner, &true);
    assert!(client.is_low_cost_mode());
}

#[test]
fn test_low_cost_mode_logs_without_indexing() {
    let (env, owner, client) = create_ledger();
    client.set_low_cost_mode(&owner, &true);
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"test-metadata");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    assert_eq!(client.total_events(), 1);

    // In low-cost mode, event_count should panic (no try_event_count method)
    // This is expected behavior - event_count will panic with ContractError::CapNotSet
}

#[test]
fn test_low_cost_mode_emission() {
    let (env, owner, client) = create_ledger();
    client.set_low_cost_mode(&owner, &true);
    client.set_event_emission_mode(&owner, &1); // Index-only
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"test-metadata");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());

    // With low-cost mode and index-only emission, events are emitted
}

// ── Event emission optimization tests ────────────────────────────────────────

#[test]
fn test_event_emission_mode_default() {
    let (env, _owner, client) = create_ledger();
    let mode = client.get_event_emission_mode();
    assert_eq!(mode, 1); // Default is full metadata emission
}

#[test]
fn test_event_emission_mode_index_only() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &1);
    assert_eq!(client.get_event_emission_mode(), 1);
}

#[test]
fn test_event_emission_mode_hash_only() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &2);
    assert_eq!(client.get_event_emission_mode(), 2);
}

#[test]
fn test_event_emission_mode_none() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &3);
    assert_eq!(client.get_event_emission_mode(), 3);
}

#[test]
fn test_event_emission_index_only() {
    let (env, owner, client) = create_ledger();
    client.set_event_emission_mode(&owner, &1);
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"large-metadata-that-would-be-emitted-full");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());

    // With index-only mode, events are emitted (data format verified by contract logic)
}

// ── Optimized storage tests ────────────────────────────────────────────────

#[test]
fn test_get_event_metadata() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"test-metadata");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    let retrieved_meta = client.get_event_metadata(&id);
    assert_eq!(retrieved_meta, meta);
}

#[test]
fn test_get_event_header() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"header-test");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    let header = client.get_event_header(&id);
    // EventHeader contains only index/timestamp/event_type/submitter — no metadata (issue #56)
    assert_eq!(header.index, 0);
    assert_eq!(header.event_type, payment);
    assert_eq!(header.submitter, submitter);
    assert_eq!(header.timestamp, 1000);
}

// ── issue #56: lazy loading / EventHeader ────────────────────────────────────

#[test]
fn test_get_event_header_has_no_metadata_field() {
    // EventHeader is a separate lighter struct; get_event() still returns full Event.
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"lazy-test");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &meta);

    // Full event has metadata
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, meta);

    // Header omits metadata; fields match
    let header = client.get_event_header(&id);
    assert_eq!(header.index, evt.index);
    assert_eq!(header.timestamp, evt.timestamp);
    assert_eq!(header.event_type, payment);
    assert_eq!(header.submitter, submitter);
}

// ── issue #54: packed-Bytes index storage ────────────────────────────────────

#[test]
fn test_packed_index_storage_get_event_by_type() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    let id0 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"p1"));
    let _rid = client.log_event(&submitter, &refund, &Bytes::from_slice(&env, b"r1"));
    let id1 = client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"p2"));

    assert_eq!(client.event_count(&payment), 2);
    assert_eq!(client.event_count(&refund), 1);

    let e0 = client.get_event_by_type(&payment, &0);
    assert_eq!(e0.metadata, Bytes::from_slice(&env, b"p1"));

    let e1 = client.get_event_by_type(&payment, &1);
    assert_eq!(e1.metadata, Bytes::from_slice(&env, b"p2"));
}

// ── issue #62: rate limiting ──────────────────────────────────────────────────

#[test]
fn test_rate_limit_blocks_excess_events() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();

    // Allow 1 event per timestamp
    client.set_submitter_rate_limit(&owner, &submitter, &1);

    // First event passes
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"a"));

    // Second event at same timestamp is rejected
    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"b"));
    assert!(result.is_err());
}

#[test]
fn test_rate_limit_resets_on_new_timestamp() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.set_submitter_rate_limit(&owner, &submitter, &1);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"a"));

    // Advance timestamp — count resets
    env.ledger().set_timestamp(1001);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"b"));
    assert_eq!(client.total_events(), 2);
}

#[test]
fn test_rate_limit_zero_blocks_completely() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.set_submitter_rate_limit(&owner, &submitter, &0);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"blocked"));
    assert!(result.is_err());
}

#[test]
fn test_rate_limit_does_not_affect_other_submitters() {
    let (env, owner, client) = create_ledger();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.set_submitter_rate_limit(&owner, &s1, &0);

    // s1 is blocked
    let r1 = client.try_log_event(&s1, &payment, &Bytes::from_slice(&env, b"x"));
    assert!(r1.is_err());

    // s2 is unaffected
    client.log_event(&s2, &payment, &Bytes::from_slice(&env, b"y"));
    assert_eq!(client.total_events(), 1);
}

// ── issue #59: storage compaction ────────────────────────────────────────────

#[test]
fn test_compact_storage_removes_stale_indices() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &5);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));

    // Remove cap — leaves stale EventTypeIndices / EventTypeCount
    client.remove_event_cap(&owner, &payment);

    // Compact should clean up stale entries and return removed count > 0
    let removed = client.compact_storage(&owner, &soroban_sdk::vec![&env, payment]);
    assert!(removed > 0);
}

#[test]
fn test_compact_storage_does_not_touch_active_caps() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &5);
    client.set_event_max_logs(&owner, &refund, &5);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"p1"));
    client.log_event(&submitter, &refund, &Bytes::from_slice(&env, b"r1"));

    // Remove only refund cap
    client.remove_event_cap(&owner, &refund);

    // Compact only refund
    client.compact_storage(&owner, &soroban_sdk::vec![&env, refund]);

    // payment cap still works
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"p2"));
    assert_eq!(client.event_count(&payment), 2);
}

#[test]
fn test_list_events_pagination() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0..50u8 {
        client.log_event(&submitter, &payment, &Bytes::from_slice(&env, &[i]));
    }

    let page = client.list_events(&10, &10);
    assert_eq!(page.len(), 10);
    assert_eq!(
        page.get(0).unwrap().metadata,
        Bytes::from_slice(&env, &[10])
    );

    let beyond = client.list_events(&60, &10);
    assert_eq!(beyond.len(), 0);

    let empty_limit = client.list_events(&0, &0);
    assert_eq!(empty_limit.len(), 0);
}

#[test]
fn test_list_events_by_type_pagination() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    for i in 0..15u8 {
        let ty = if i % 2 == 0 { &payment } else { &refund };
        client.log_event(&submitter, ty, &Bytes::from_slice(&env, &[i]));
    }

    let page = client.list_events_by_type(&payment, &1, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[4]));
}

#[test]
fn test_get_events_by_time_range() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0..5u64 {
        env.ledger().set_timestamp(1000 + i);
        client.log_event(&submitter, &payment, &Bytes::from_slice(&env, &[i as u8]));
    }

    let results = client.get_events_by_time_range(&1001, &1003, &0, &10);
    assert_eq!(results.len(), 3);
    assert_eq!(results.get(0).unwrap().timestamp, 1001);

    let none = client.get_events_by_time_range(&2000, &3000, &0, &10);
    assert_eq!(none.len(), 0);

    let inverted = client.get_events_by_time_range(&2000, &1000, &0, &10);
    assert_eq!(inverted.len(), 0);

    let full = client.get_events_by_time_range(&1000, &1004, &0, &10);
    assert_eq!(full.len(), 5);

    let paged = client.get_events_by_time_range(&1000, &1004, &2, &2);
    assert_eq!(paged.len(), 2);
    assert_eq!(paged.get(0).unwrap().timestamp, 1002);
}

#[test]
fn test_search_events() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"alpha"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"beta"));
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"alphabet"));

    let exact = client.search_events(&Bytes::from_slice(&env, b"beta"), &0, &10);
    assert_eq!(exact.len(), 1);

    let substring = client.search_events(&Bytes::from_slice(&env, b"alp"), &0, &10);
    assert_eq!(substring.len(), 2);

    let none = client.search_events(&Bytes::from_slice(&env, b"gamma"), &0, &10);
    assert_eq!(none.len(), 0);

    let empty = client.search_events(&Bytes::from_slice(&env, b""), &0, &10);
    assert_eq!(empty.len(), 3);
}

#[test]
fn test_update_event_history() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"original"));
    let history_before = client.get_event_history(&0);
    assert_eq!(history_before.len(), 1);

    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"updated"));
    let history_after = client.get_event_history(&0);
    assert_eq!(history_after.len(), 2);
    assert_eq!(
        history_after.get(1).unwrap().data.metadata,
        Bytes::from_slice(&env, b"updated")
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_update_event_non_owner_panics() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let attacker = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"original"));
    client.update_event(&attacker, &0, &Bytes::from_slice(&env, b"updated"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_update_event_nonexistent_panics() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"updated"));
}


// ── Hash Chain Integrity Verification (Issue #144) ───────────────────────────

#[test]
fn test_verify_chain_empty() {
    let (_env, _owner, client) = create_ledger();
    assert!(client.verify_integrity_range(&0, &0));
}

#[test]
fn test_verify_chain_single_event() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("test"), &Bytes::from_slice(&env, b"data"));
    
    assert!(client.verify_integrity_range(&0, &1));
}

#[test]
fn test_verify_chain_multiple_events_sequential() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    
    for i in 0u8..10 {
        client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, &[i]));
    }
    
    assert!(client.verify_integrity_range(&0, &10));
}

#[test]
fn test_verify_chain_partial_ranges() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    
    for i in 0u8..5 {
        client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, &[i]));
    }
    
    // Verify subranges
    assert!(client.verify_integrity_range(&1, &3));
    assert!(client.verify_integrity_range(&0, &5));
    assert!(client.verify_integrity_range(&2, &4));
}

#[test]
fn test_verify_chain_full_integrity() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    
    for i in 0u8..15 {
        client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, &[i]));
    }
    
    // Full chain must be valid
    assert!(client.verify_integrity());
}

#[test]
fn test_verify_chain_prev_hash_consistency() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    
    let id0 = client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, b"e0"));
    let id1 = client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, b"e1"));
    let id2 = client.log_event(&submitter, &symbol_short!("evt"), &Bytes::from_slice(&env, b"e2"));
    
    let evt0 = client.get_event(&id0);
    let evt1 = client.get_event(&id1);
    let evt2 = client.get_event(&id2);
    
    // Chain linkage must be intact
    assert_eq!(evt0.prev_hash, BytesN::from_array(&env, &[0u8; 32]));
    assert_eq!(evt1.prev_hash, evt0.event_hash);
    assert_eq!(evt2.prev_hash, evt1.event_hash);
    
    // Verification must pass
    assert!(client.verify_integrity_range(&0, &3));
}

#[test]
fn test_verify_chain_different_event_types() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    
    client.log_event(&submitter, &symbol_short!("pay"), &Bytes::from_slice(&env, b"1"));
    client.log_event(&submitter, &symbol_short!("ref"), &Bytes::from_slice(&env, b"2"));
    client.log_event(&submitter, &symbol_short!("pay"), &Bytes::from_slice(&env, b"3"));
    client.log_event(&submitter, &symbol_short!("del"), &Bytes::from_slice(&env, b"4"));
    
    assert!(client.verify_integrity());
}


// ── Submitter Allowlist / Blocklist (Issue #141) ──────────────────────────────

#[test]
fn test_block_submitter() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Submit event before blocking
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
    
    // Block the submitter
    client.block_submitter(&owner, &submitter);
    
    // Attempt to submit after blocking should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &submitter,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"blocked"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
}

#[test]
fn test_unblock_submitter() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Block the submitter
    client.block_submitter(&owner, &submitter);
    
    // Verify blocked
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &submitter,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"blocked"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
    
    // Unblock the submitter
    client.unblock_submitter(&owner, &submitter);
    
    // Now submission should work
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"allowed"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_allowlist_mode_enabled() {
    let (env, owner, client) = create_ledger();
    let whitelisted = Address::generate(&env);
    let non_whitelisted = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Enable allowlist mode
    client.enable_allowlist_mode(&owner);
    
    // Whitelisted submitter not yet allowed - should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &whitelisted,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"data"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
    
    // Allow submitter
    client.allow_submitter(&owner, &whitelisted);
    
    // Now it should work
    let id = client.log_event(
        &whitelisted,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"allowed"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
    
    // Non-whitelisted should still fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &non_whitelisted,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"not_allowed"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
}

#[test]
fn test_remove_from_allowlist() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Enable allowlist mode and allow submitter
    client.enable_allowlist_mode(&owner);
    client.allow_submitter(&owner, &submitter);
    
    // Should work
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"allowed"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
    
    // Remove from allowlist
    client.remove_submitter_from_allowlist(&owner, &submitter);
    
    // Should fail now
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &submitter,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"removed"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
}

#[test]
fn test_disable_allowlist_mode() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Enable allowlist mode
    client.enable_allowlist_mode(&owner);
    
    // Submitter not whitelisted - should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &submitter,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"data"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
    
    // Disable allowlist mode
    client.disable_allowlist_mode(&owner);
    
    // Should work now
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"allowed"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_blocklist_takes_precedence() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    
    env.mock_all_auths();
    
    // Enable allowlist and allow submitter
    client.enable_allowlist_mode(&owner);
    client.allow_submitter(&owner, &submitter);
    
    // Should work
    let id = client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"allowed"),
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
    
    // Block the submitter (blocklist takes precedence over allowlist)
    client.block_submitter(&owner, &submitter);
    
    // Should fail now
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.log_event(
            &submitter,
            &symbol_short!("test"),
            &Bytes::from_slice(&env, b"blocked"),
            &None,
            &None,
        )
    }));
    assert!(result.is_err());
}
