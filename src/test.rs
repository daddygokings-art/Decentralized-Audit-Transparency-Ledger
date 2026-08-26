use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Vec};

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

// ── Basic functionality ─────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);

    assert_eq!(client.total_events(), 0);
}

#[test]
fn test_log_event() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &event_type,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &refund,
        &Bytes::from_slice(&env, b"tx3"),
        &None,
        &None,
        &false,
    );

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
#[should_panic(expected = "HostError: Error(Contract, #26)")]
fn test_initialize_reinitialization_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);
    // Try to re-initialize — should fail with AlreadyInitialized (error #19)
    client.initialize(&owners, &200, &4096);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #26)")]
fn test_initialize_reinitialization_after_ownership_transfer_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);
    
    // Transfer ownership
    client.transfer_ownership(&owner, &new_owner);

    // Try to re-initialize with new owner — should still fail with AlreadyInitialized
    // (demonstrates that version counter protects against re-init even if owner changes)
    client.initialize(&owners, &200, &4096);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #20)")]
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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.get_event_by_type(&payment, &1);
}

#[test]
fn test_event_count_and_total_events_with_empty_metadata() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::new(&env), &None, &None, &false);
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"non-empty"),
        &None,
        &None,
        &false,
    );

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
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"a")),
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"b")),
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"c")),
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
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"a")),
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"b")),
        (submitter.clone(), payment.clone(), Bytes::from_slice(&env, b"c")),
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

    let events1 = soroban_sdk::vec![
        &env,
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"1")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"2")),
        (submitter.clone(), event_type.clone(), Bytes::from_slice(&env, b"3")),
    ];
    client.log_events(&events1);

    let events2 = soroban_sdk::vec![
        &env,
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
    let id: BytesN<32> = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    // ID is a 32-byte value (BytesN<32> by type)
    assert_eq!(id.len(), 32);
}

#[test]
fn test_different_metadata_produces_different_ids() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id1 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    let id2 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert_ne!(id1, id2);
}

#[test]
fn test_get_event_by_order() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id0 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"first"),
        &None,
        &None,
        &false,
    );
    let id1 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"second"),
        &None,
        &None,
        &false,
    );

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
        &None,
        &None,
        &false,
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
        client.log_event(
            &submitter,
            &payment,
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
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
        client.log_event(
            &submitter,
            &payment,
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
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
    let id0 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"a"),
        &None,
        &None,
        &false,
    );
    let id1 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"b"),
        &None,
        &None,
        &false,
    );

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

    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.event_count(&payment), 2);

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx3"),
        &None,
        &None,
        &false,
    );
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
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &2, &4096);

    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &refund,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx3"),
        &None,
        &None,
        &false,
    );
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
        &None,
        &None,
        &false,
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
        &None,
        &None,
        &false,
    );
    client.set_global_max_logs(&owner, &1);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
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
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &0, &4096);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("p"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
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
        &None,
        &None,
        &false,
    );
    // Setting max below current count should fail
    let result = client.try_set_global_max_logs(&owner, &0);
    assert!(result.is_err());
}

#[test]
fn test_zero_event_max_logs() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &0);

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_set_event_max_equal_to_current_count() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );

    client.set_event_max_logs(&owner, &payment, &2);

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx3"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_event_was_emitted() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta = Bytes::from_slice(&env, b"emit-test");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &meta, &None, &None, &false);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(!events.is_empty());
}

#[test]
fn test_log_event_metadata_limit_at_max() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let max_meta = Bytes::from_slice(&env, &[0u8; 4096]);

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &max_meta, &None, &None, &false);
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 4096);
}

#[test]
fn test_log_event_metadata_too_large_reverts() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta_vec = [0u8; 4097];
    let metadata = Bytes::from_slice(&env, &meta_vec);

    env.mock_all_auths();
    let result = client.try_log_event(&submitter, &payment, &metadata, &None, &None, &false);
    assert!(result.is_err());
}

#[test]
fn test_log_events_metadata_too_large_reverts() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let meta_vec = [0u8; 4097];
    let metadata = Bytes::from_slice(&env, &meta_vec);
    let events = soroban_sdk::vec![
        &env,
        (
            submitter.clone(),
            payment.clone(),
            metadata.clone(),
        ),
    ];

    env.mock_all_auths();
    let result = client.try_log_events(&events);
    assert!(result.is_err());
}

#[test]
fn test_log_event_with_empty_metadata_respects_limit() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id = client.log_event(&submitter, &payment, &Bytes::new(&env), &None, &None, &false);
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

    client.log_event(
        &submitter,
        &type_a,
        &Bytes::from_slice(&env, b"a1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_b,
        &Bytes::from_slice(&env, b"b1"),
        &None,
        &None,
        &false,
    );

    let result = client.try_log_event(
        &submitter,
        &type_a,
        &Bytes::from_slice(&env, b"a2"),
        &None,
        &None,
        &false,
    );
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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);
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
        &None,
        &None,
        &false,
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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    env.ledger().set_timestamp(999);
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_log_event_rejects_future_timestamp() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    env.ledger()
        .set_timestamp(1000 + super::MAX_TIMESTAMP_DRIFT_SECONDS + 1);
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
}

#[test]
fn test_log_event_accepts_normal_timestamp_progression() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    env.ledger().set_timestamp(1001);
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );

    assert_eq!(client.total_events(), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn test_log_event_rejects_total_events_overflow() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &u32::MAX, &4096);
    env.storage()
        .instance()
        .set(&super::DataKey::TotalEvents, &u32::MAX);

    let submitter = Address::generate(&env);
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
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
    client.log_event(
        &submitter_a,
        &payment,
        &Bytes::from_slice(&env, b"t1"),
        &None,
        &None,
        &false,
    );
    env.ledger().set_timestamp(1001);
    client.log_event(
        &submitter_b,
        &refund,
        &Bytes::from_slice(&env, b"t2"),
        &None,
        &None,
        &false,
    );
    env.ledger().set_timestamp(1002);
    client.log_event(
        &submitter_a,
        &payment,
        &Bytes::from_slice(&env, b"t3"),
        &None,
        &None,
        &false,
    );

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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );

    client.set_global_max_logs(&owner, &2);

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx3"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_remove_cap_then_unlimited() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &0);

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"blocked"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    client.remove_event_cap(&owner, &payment);

    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"now-unblocked"),
        &None,
        &None,
        &false,
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
    let _id = client.log_event(&submitter, &symbol_short!("p"), &meta, &None, &None, &false);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_metadata_size_cap_rejects_oversized_default() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    // 1025 > 1024 default → rejected
    let meta = Bytes::from_slice(&env, &[0u8; 1025]);
    let result = client.try_log_event(&submitter, &symbol_short!("p"), &meta, &None, &None, &false);
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
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
    // 51 bytes → rejected
    let r2 = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, &[0u8; 51]),
        &None,
        &None,
        &false,
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
    let _id = client.log_event(
        &submitter,
        &lett,
        &Bytes::from_slice(&env, &[0u8; 50]),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
    // type "z" uses global cap of 10 → 11 fails
    let r2 = client.try_log_event(
        &submitter,
        &symbol_short!("z"),
        &Bytes::from_slice(&env, &[0u8; 11]),
        &None,
        &None,
        &false,
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
#[should_panic(expected = "HostError: Error(Contract, #12)")]
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
        &None,
        &None,
        &false,
    );
    let stored = client.get_event_signature(&id);
    assert!(stored.is_none());
}

// ── issue #343: additional boundary and regression tests ─────────────────

#[test]
fn test_transfer_ownership_to_zero_panics() {
    let (env, owner, client) = create_ledger();
    let zero = Address::from_str(&env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");

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
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &symbol_short!("b"),
        &Bytes::from_slice(&env, b"y"),
        &None,
        &None,
        &false,
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
    let _id = client.log_event(&submitter, &symbol_short!("p"), &large_meta, &None, &None, &false);
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
        client.log_event(&submitter, &t, &Bytes::from_slice(&env, &[i]), &None, &None, &false);
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
        &None,
        &None,
        &false,
    );
    let id1 = client.log_event(
        &submitter,
        &symbol_short!("b"),
        &Bytes::from_slice(&env, b"second"),
        &None,
        &None,
        &false,
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
    let _id0 = client.log_event(
        &submitter,
        &payments,
        &Bytes::from_slice(&env, b"a"),
        &None,
        &None,
        &false,
    );
    let _id1 = client.log_event(
        &submitter,
        &payments,
        &Bytes::from_slice(&env, b"b"),
        &None,
        &None,
        &false,
    );
    let _id2 = client.log_event(
        &submitter,
        &payments,
        &Bytes::from_slice(&env, b"c"),
        &None,
        &None,
        &false,
    );

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
    let id = client.log_event(&submitter, &symbol_short!("p"), &meta, &None, &None, &false);

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
        client.log_event(&submitter, &t, &Bytes::from_slice(&env, &[i]), &None, &None, &false);
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
        client.log_event(
            &submitter,
            &type_a,
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
        client.log_event(
            &submitter,
            &type_b,
            &Bytes::from_slice(&env, &[i + 100]),
            &None,
            &None,
            &false,
        );
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

    client.log_event(
        &submitter,
        &type_a,
        &Bytes::from_slice(&env, b"a1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_a,
        &Bytes::from_slice(&env, b"a2"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_b,
        &Bytes::from_slice(&env, b"b1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_b,
        &Bytes::from_slice(&env, b"b2"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_b,
        &Bytes::from_slice(&env, b"b3"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &type_c,
        &Bytes::from_slice(&env, b"c1"),
        &None,
        &None,
        &false,
    );

    assert_eq!(client.total_events(), 6);
    assert_eq!(client.event_count(&type_a), 2);
    assert_eq!(client.event_count(&type_b), 3);
    assert_eq!(client.event_count(&type_c), 1);

    let result = client.try_log_event(
        &submitter,
        &type_a,
        &Bytes::from_slice(&env, b"a3"),
        &None,
        &None,
        &false,
    );
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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id = client.log_event(&submitter, &payment, &meta, &None, &None, &false);

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
    let id0 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"p1"),
        &None,
        &None,
        &false,
    );
    let _rid = client.log_event(
        &submitter,
        &refund,
        &Bytes::from_slice(&env, b"r1"),
        &None,
        &None,
        &false,
    );
    let id1 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"p2"),
        &None,
        &None,
        &false,
    );

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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"a"),
        &None,
        &None,
        &false,
    );

    // Second event at same timestamp is rejected
    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"b"),
        &None,
        &None,
        &false,
    );
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

    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"a"),
        &None,
        &None,
        &false,
    );

    // Advance timestamp — count resets
    env.ledger().set_timestamp(1001);
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"b"),
        &None,
        &None,
        &false,
    );
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

    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"blocked"),
        &None,
        &None,
        &false,
    );
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
    let r1 = client.try_log_event(&s1, &payment, &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    assert!(r1.is_err());

    // s2 is unaffected
    client.log_event(&s2, &payment, &Bytes::from_slice(&env, b"y"), &None, &None, &false);
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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"p1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &refund,
        &Bytes::from_slice(&env, b"r1"),
        &None,
        &None,
        &false,
    );

    // Remove only refund cap
    client.remove_event_cap(&owner, &refund);

    // Compact only refund
    client.compact_storage(&owner, &soroban_sdk::vec![&env, refund]);

    // payment cap still works
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"p2"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.event_count(&payment), 2);
}

#[test]
fn test_list_events_pagination() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0..50u8 {
        client.log_event(
            &submitter,
            &payment,
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
    }

    let page = client.list_events(&10, &10);
    assert_eq!(page.len(), 10);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[10]));

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
        client.log_event(&submitter, ty, &Bytes::from_slice(&env, &[i]), &None, &None, &false);
    }

    let page = client.list_events_by_type(&payment, &1, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[2]));
}

#[test]
fn test_get_events_by_time_range() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    for i in 0..5u64 {
        env.ledger().set_timestamp(1000 + i);
        client.log_event(
            &submitter,
            &payment,
            &Bytes::from_slice(&env, &[i as u8]),
            &None,
            &None,
            &false,
        );
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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"alpha"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"beta"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"alphabet"),
        &None,
        &None,
        &false,
    );

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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"original"),
        &None,
        &None,
        &false,
    );
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
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"original"),
        &None,
        &None,
        &false,
    );
    client.update_event(&attacker, &0, &Bytes::from_slice(&env, b"updated"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_update_event_nonexistent_panics() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"updated"));
}

// ── issue #204: event versioning with rollback ────────────────────────────────

#[test]
fn test_rollback_event_to_original() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id0 = client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"original"),
        &None,
        &None,
        &false,
    );
    let new_id = client.update_event(&owner, &0, &Bytes::from_slice(&env, b"updated"));
    assert_ne!(id0, new_id);

    let rolled_id = client.rollback_event(&owner, &0, &0);
    assert_eq!(rolled_id, id0);

    let evt = client.get_event(&id0);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"original"));

    let history = client.get_event_history(&0);
    assert_eq!(history.len(), 3);
    assert_eq!(history.get(2).unwrap().data.metadata, Bytes::from_slice(&env, b"original"));
}

#[test]
fn test_rollback_event_to_specific_version() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"v0"),
        &None,
        &None,
        &false,
    );
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"v1"));
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"v2"));

    let history = client.get_event_history(&0);
    assert_eq!(history.len(), 4);

    client.rollback_event(&owner, &0, &1);
    let evt = client.get_event_by_order(&0);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"v1"));

    let new_history = client.get_event_history(&0);
    assert_eq!(new_history.len(), 5);
    assert_eq!(new_history.get(4).unwrap().data.metadata, Bytes::from_slice(&env, b"v1"));
}

#[test]
fn test_rollback_preserves_hash_chain() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("a"), &Bytes::from_slice(&env, b"1"), &None, &None, &false);
    client.log_event(&submitter, &symbol_short!("b"), &Bytes::from_slice(&env, b"2"), &None, &None, &false);
    client.log_event(&submitter, &symbol_short!("c"), &Bytes::from_slice(&env, b"3"), &None, &None, &false);

    assert!(client.verify_integrity());

    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"1-updated"));
    assert!(client.verify_integrity());

    client.rollback_event(&owner, &0, &0);
    assert!(client.verify_integrity());
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #33)")]
fn test_rollback_invalid_version_panics() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    client.rollback_event(&owner, &0, &5);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_rollback_non_owner_panics() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    env.mock_all_auths();
    client.rollback_event(&attacker, &0, &0);
}

#[test]
fn test_get_event_version_count() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    assert_eq!(client.get_event_version_count(&0), 1);

    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"y"));
    assert_eq!(client.get_event_version_count(&0), 2);

    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"z"));
    assert_eq!(client.get_event_version_count(&0), 3);
}

#[test]
fn test_compare_event_versions() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"short"), &None, &None, &false);
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"much longer metadata"));

    assert_eq!(client.compare_event_versions(&0, &0, &0), 0);
    assert_eq!(client.compare_event_versions(&0, &1, &1), 0);
    assert!(client.compare_event_versions(&0, &0, &1) < 0);
    assert!(client.compare_event_versions(&0, &1, &0) > 0);
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
    client.log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
        &None,
        &None,
        &false,
    );

    assert!(client.verify_integrity_range(&0, &1));
}

#[test]
fn test_verify_chain_multiple_events_sequential() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    for i in 0u8..10 {
        client.log_event(
            &submitter,
            &symbol_short!("evt"),
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
    }

    assert!(client.verify_integrity_range(&0, &10));
}

#[test]
fn test_verify_chain_partial_ranges() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    for i in 0u8..5 {
        client.log_event(
            &submitter,
            &symbol_short!("evt"),
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
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
        client.log_event(
            &submitter,
            &symbol_short!("evt"),
            &Bytes::from_slice(&env, &[i]),
            &None,
            &None,
            &false,
        );
    }

    // Full chain must be valid
    assert!(client.verify_integrity());
}

#[test]
fn test_verify_chain_prev_hash_consistency() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    let id0 = client.log_event(
        &submitter,
        &symbol_short!("evt"),
        &Bytes::from_slice(&env, b"e0"),
        &None,
        &None,
        &false,
    );
    let id1 = client.log_event(
        &submitter,
        &symbol_short!("evt"),
        &Bytes::from_slice(&env, b"e1"),
        &None,
        &None,
        &false,
    );
    let id2 = client.log_event(
        &submitter,
        &symbol_short!("evt"),
        &Bytes::from_slice(&env, b"e2"),
        &None,
        &None,
        &false,
    );

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

    client.log_event(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &symbol_short!("ref"),
        &Bytes::from_slice(&env, b"2"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"3"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &symbol_short!("del"),
        &Bytes::from_slice(&env, b"4"),
        &None,
        &None,
        &false,
    );

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
        &false,
    );
    assert_eq!(client.total_events(), 1);

    // Block the submitter
    client.block_submitter(&owner, &submitter);

    // Attempt to submit after blocking should fail
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"blocked"),
        &None,
        &None,
        &false,
    );
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
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"blocked"),
        &None,
        &None,
        &false,
    );
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
        &false,
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
    let result = client.try_log_event(
        &whitelisted,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
        &None,
        &None,
        &false,
    );
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
        &false,
    );
    assert_eq!(client.total_events(), 1);

    // Non-whitelisted should still fail
    let result = client.try_log_event(
        &non_whitelisted,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"not_allowed"),
        &None,
        &None,
        &false,
    );
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
        &false,
    );
    assert_eq!(client.total_events(), 1);

    // Remove from allowlist
    client.remove_submitter_from_allowlist(&owner, &submitter);

    // Should fail now
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"removed"),
        &None,
        &None,
        &false,
    );
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
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
        &None,
        &None,
        &false,
    );
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
        &false,
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
        &false,
    );
    assert_eq!(client.total_events(), 1);

    // Block the submitter (blocklist takes precedence over allowlist)
    client.block_submitter(&owner, &submitter);

    // Should fail now
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"blocked"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_set_global_max_logs_emits_event() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    client.set_global_max_logs(&owner, &200);
    // Verify the event was published by checking total (non-zero events list)
    let evts = env.events().all();
    // At least one event should exist after governance call
    assert!(!evts.events().is_empty());
}

#[test]
fn test_transfer_ownership_emits_event() {
    let (env, owner, client) = create_ledger();
    let new_owner = Address::generate(&env);
    env.mock_all_auths();
    client.transfer_ownership(&owner, &new_owner);
    let evts = env.events().all();
    assert!(!evts.events().is_empty());
}

#[test]
fn test_remove_event_cap_emits_event() {
    let (env, owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &50);
    let before = env.events().all().events().len();
    client.remove_event_cap(&owner, &payment);
    let after = env.events().all().events().len();
    // A new event should have been published
    assert!(after > before);
}

// ── TTL storage (#121) ───────────────────────────────────────────────────────

#[test]
fn test_set_event_ttl_and_get() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    assert_eq!(client.get_event_ttl(), 0);
    client.set_event_ttl(&owner, &1000);
    assert_eq!(client.get_event_ttl(), 1000);
}

#[test]
fn test_set_event_ttl_emits_governance_event() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let before = env.events().all().events().len();
    client.set_event_ttl(&owner, &500);
    let after = env.events().all().events().len();
    assert!(after > before, "set_event_ttl should emit a governance event");
}

#[test]
fn test_log_event_with_ttl_enabled() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    client.set_event_ttl(&owner, &1000);
    // Logging should succeed with TTL enabled; event stored in both instance + persistent
    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"ttl_test"),
        &None,
        &None,
        &false,
    );
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"ttl_test"));
}

// ── get_events_by_type pagination ──────────────────────────────────────────

/// Helper: log `count` events of `event_type` with sequential single-byte metadata (0,1,2,…).
fn log_n_events(
    env: &Env,
    client: &AuditLedgerClient,
    submitter: &Address,
    event_type: &soroban_sdk::Symbol,
    count: u32,
) {
    for i in 0..count {
        // Use the index as a single-byte payload so each event is distinct.
        // Tests that inspect metadata values use the same byte encoding.
        let meta = Bytes::from_slice(env, &[i as u8]);
        client.log_event(submitter, event_type, &meta, &None, &None, &false);
    }
}

/// Normal case: fetch second page (start=2, limit=2) from 5 events.
#[test]
fn test_get_events_by_type_normal_page() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 5);

    // page: events at type-indices 2 and 3
    let page = client.get_events_by_type(&payment, &2, &2);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[2u8]));
    assert_eq!(page.get(1).unwrap().metadata, Bytes::from_slice(&env, &[3u8]));
}

/// Partial last page: start=4, limit=5 with only 5 events → should return 1 event.
#[test]
fn test_get_events_by_type_partial_last_page() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 5);

    let page = client.get_events_by_type(&payment, &4, &5);
    assert_eq!(page.len(), 1);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[4u8]));
}

/// First page: start=0, limit=3.
#[test]
fn test_get_events_by_type_first_page() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 5);

    let page = client.get_events_by_type(&payment, &0, &3);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[0u8]));
    assert_eq!(page.get(1).unwrap().metadata, Bytes::from_slice(&env, &[1u8]));
    assert_eq!(page.get(2).unwrap().metadata, Bytes::from_slice(&env, &[2u8]));
}

/// Exact page: limit equals the total number of events.
#[test]
fn test_get_events_by_type_exact_page() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    let page = client.get_events_by_type(&payment, &0, &3);
    assert_eq!(page.len(), 3);
}

/// Empty case: event type has no events at all → empty vec.
#[test]
fn test_get_events_by_type_empty_type_returns_empty() {
    let (_env, _owner, client) = create_ledger();
    let payment = symbol_short!("payment");

    // No events logged for this type
    let page = client.get_events_by_type(&payment, &0, &10);
    assert_eq!(page.len(), 0);
}

/// Boundary: start equals total count (one past end) → empty vec.
#[test]
fn test_get_events_by_type_start_at_boundary_returns_empty() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    // start == count (3) — out of range
    let page = client.get_events_by_type(&payment, &3, &10);
    assert_eq!(page.len(), 0);
}

/// Boundary: start beyond total count → empty vec.
#[test]
fn test_get_events_by_type_start_beyond_range_returns_empty() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    let page = client.get_events_by_type(&payment, &100, &10);
    assert_eq!(page.len(), 0);
}

/// Boundary: limit=0 → empty vec (no panic).
#[test]
fn test_get_events_by_type_zero_limit_returns_empty() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    let page = client.get_events_by_type(&payment, &0, &0);
    assert_eq!(page.len(), 0);
}

/// Boundary: limit=1 fetches exactly one event.
#[test]
fn test_get_events_by_type_limit_one() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    let page = client.get_events_by_type(&payment, &1, &1);
    assert_eq!(page.len(), 1);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, &[1u8]));
}

/// Boundary: limit=100 (max allowed) works fine.
#[test]
fn test_get_events_by_type_max_limit() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 5);

    // limit=100 but only 5 events exist → should return 5
    let page = client.get_events_by_type(&payment, &0, &100);
    assert_eq!(page.len(), 5);
}

/// Exceeding max limit panics with InvalidPaginationParams (error #21).
#[test]
#[should_panic(expected = "HostError: Error(Contract, #21)")]
fn test_get_events_by_type_limit_over_100_panics() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);

    // limit=101 should panic
    client.get_events_by_type(&payment, &0, &101);
}

/// Mixed types: pagination for one type is independent of another type's events.
#[test]
fn test_get_events_by_type_independent_of_other_types() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");

    env.mock_all_auths();
    log_n_events(&env, &client, &submitter, &payment, 3);
    log_n_events(&env, &client, &submitter, &refund, 2);

    // payment has 3 events
    let payment_page = client.get_events_by_type(&payment, &0, &10);
    assert_eq!(payment_page.len(), 3);

    // refund has 2 events, independent
    let refund_page = client.get_events_by_type(&refund, &0, &10);
    assert_eq!(refund_page.len(), 2);

    // each page contains the right event_type
    for i in 0..payment_page.len() {
        assert_eq!(payment_page.get(i).unwrap().event_type, payment);
    }
    for i in 0..refund_page.len() {
        assert_eq!(refund_page.get(i).unwrap().event_type, refund);
    }
}

/// Verify ordering: events are returned in insertion order by type.
#[test]
fn test_get_events_by_type_preserves_insertion_order() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    // Log interleaved events of different types; payment events should retain their own order
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"first"), &None, &None, &false);
    client.log_event(&submitter, &symbol_short!("other"), &Bytes::from_slice(&env, b"noise"), &None, &None, &false);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"second"), &None, &None, &false);
    client.log_event(&submitter, &symbol_short!("other"), &Bytes::from_slice(&env, b"noise2"), &None, &None, &false);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"third"), &None, &None, &false);

    let page = client.get_events_by_type(&payment, &0, &10);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap().metadata, Bytes::from_slice(&env, b"first"));
    assert_eq!(page.get(1).unwrap().metadata, Bytes::from_slice(&env, b"second"));
    assert_eq!(page.get(2).unwrap().metadata, Bytes::from_slice(&env, b"third"));
}

// ── issue #202: metadata schema validation ────────────────────────────────────

#[test]
fn test_set_metadata_schema_and_get() {
    let (env, owner, client) = create_ledger();
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    // schema: min_len = 5
    let schema = Bytes::from_slice(&env, &[5u8, 0, 0, 0]);
    client.set_metadata_schema(&owner, &event_type, &schema);
    let retrieved = client.get_metadata_schema(&event_type);
    assert_eq!(retrieved, schema);
}

#[test]
fn test_get_metadata_schema_returns_empty_when_not_set() {
    let (env, _owner, client) = create_ledger();
    let event_type = symbol_short!("payment");
    let schema = client.get_metadata_schema(&event_type);
    assert_eq!(schema.len(), 0);
}

#[test]
fn test_metadata_schema_passes_when_met() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    // schema: min_len = 5
    let schema = Bytes::from_slice(&env, &[5u8, 0, 0, 0]);
    client.set_metadata_schema(&owner, &event_type, &schema);
    // 5 bytes passes
    let id = client.log_event(
        &submitter,
        &event_type,
        &Bytes::from_slice(&env, b"12345"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
    // 10 bytes also passes
    client.log_event(
        &submitter,
        &event_type,
        &Bytes::from_slice(&env, b"0123456789"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #32)")]
fn test_metadata_schema_fails_when_too_short() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    // schema: min_len = 10
    let schema = Bytes::from_slice(&env, &[10u8, 0, 0, 0]);
    client.set_metadata_schema(&owner, &event_type, &schema);
    // 5 bytes is too short
    client.log_event(
        &submitter,
        &event_type,
        &Bytes::from_slice(&env, b"12345"),
        &None,
        &None,
        &false,
    );
}

#[test]
fn test_metadata_schema_empty_passes_any() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    // empty schema = no constraint
    client.set_metadata_schema(&owner, &event_type, &Bytes::new(&env));
    client.log_event(
        &submitter,
        &event_type,
        &Bytes::new(&env),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_metadata_schema_non_owner_cannot_set() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    let schema = Bytes::from_slice(&env, &[5u8, 0, 0, 0]);
    let result = client.try_set_metadata_schema(&attacker, &event_type, &schema);
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #32)")]
fn test_metadata_schema_enforced_on_update() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    env.mock_all_auths();
    // schema: min_len = 10
    let schema = Bytes::from_slice(&env, &[10u8, 0, 0, 0]);
    client.set_metadata_schema(&owner, &event_type, &schema);
    // log a valid event first
    let id = client.log_event(
        &submitter,
        &event_type,
        &Bytes::from_slice(&env, b"0123456789"),
        &None,
        &None,
        &false,
    );
    // attempt to update with too-short metadata
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"short"));
}

#[test]
fn test_metadata_schema_per_type_isolation() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");
    let refund = symbol_short!("refund");
    env.mock_all_auths();
    // payment requires min 8 bytes
    let schema_payment = Bytes::from_slice(&env, &[8u8, 0, 0, 0]);
    client.set_metadata_schema(&owner, &payment, &schema_payment);
    // refund has no schema
    // refund short metadata passes
    client.log_event(
        &submitter,
        &refund,
        &Bytes::from_slice(&env, b"1"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
    // payment short metadata fails
    let result = client.try_log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"1"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
    // payment long metadata passes
    client.log_event(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"01234567"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 2);
}

// ── TTL auto-cleanup (#200) ───────────────────────────────────────────────────

/// cleanup_expired_events returns 0 when TTL is disabled (ttl = 0).
#[test]
fn test_cleanup_expired_events_returns_zero_when_ttl_disabled() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.log_event(&submitter, &symbol_short!("pay"), &Bytes::from_slice(&env, b"a"), &None, &None, &false);
    // TTL not set — cleanup should be a no-op
    let removed = client.cleanup_expired_events(&owner, &0, &100);
    assert_eq!(removed, 0);
}

/// cleanup_expired_events with TTL set and no expired entries returns 0.
#[test]
fn test_cleanup_expired_events_no_expiry_returns_zero() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.set_event_ttl(&owner, &10000);
    client.log_event(&submitter, &symbol_short!("pay"), &Bytes::from_slice(&env, b"a"), &None, &None, &false);
    // Persistent entry is still alive, so nothing should be counted as expired.
    let removed = client.cleanup_expired_events(&owner, &0, &100);
    assert_eq!(removed, 0);
}

/// cleanup_expired_events increments the run counter each time.
#[test]
fn test_cleanup_stats_run_counter_increments() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);

    let before = client.get_cleanup_stats();
    assert_eq!(before.runs, 0);

    client.cleanup_expired_events(&owner, &0, &100);
    let after1 = client.get_cleanup_stats();
    assert_eq!(after1.runs, 1);

    client.cleanup_expired_events(&owner, &0, &100);
    let after2 = client.get_cleanup_stats();
    assert_eq!(after2.runs, 2);
}

/// cleanup_expired_events records the ledger sequence of the last run.
#[test]
fn test_cleanup_stats_last_run_ledger_updated() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);

    let stats_before = client.get_cleanup_stats();
    assert_eq!(stats_before.last_run_ledger, 0);

    client.cleanup_expired_events(&owner, &0, &100);

    let stats_after = client.get_cleanup_stats();
    assert!(stats_after.last_run_ledger > 0, "last_run_ledger should be set after cleanup");
}

/// get_cleanup_stats returns zero-valued struct when no cleanup has run.
#[test]
fn test_get_cleanup_stats_default_zero() {
    let (_env, _owner, client) = create_ledger();
    let stats = client.get_cleanup_stats();
    assert_eq!(stats.runs, 0);
    assert_eq!(stats.cleaned, 0);
    assert_eq!(stats.ttl_extensions, 0);
    assert_eq!(stats.last_run_ledger, 0);
}

/// Non-owner cannot call cleanup_expired_events.
#[test]
fn test_cleanup_expired_events_non_owner_rejected() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    env.mock_all_auths();

    client.set_event_ttl(&_owner, &1000);
    let result = client.try_cleanup_expired_events(&attacker, &0, &100);
    assert!(result.is_err());
}

/// cleanup_expired_events emits a monitoring event.
#[test]
fn test_cleanup_expired_events_emits_event() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);
    let before = env.events().all().events().len();
    client.cleanup_expired_events(&owner, &0, &100);
    let after = env.events().all().events().len();
    assert!(after > before, "cleanup_expired_events should emit a monitoring event");
}

/// TTL extension on read: get_event extends the persistent TTL.
#[test]
fn test_get_event_extends_ttl_on_read() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);
    let id = client.log_event(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"extend_test"),
        &None,
        &None,
        &false,
    );

    let stats_before = client.get_cleanup_stats();
    assert_eq!(stats_before.ttl_extensions, 0);

    // Reading the event should extend TTL and increment the counter.
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"extend_test"));

    let stats_after = client.get_cleanup_stats();
    assert_eq!(stats_after.ttl_extensions, 1, "TTL extension counter should increment on read");
}

/// TTL extension is skipped when TTL is disabled.
#[test]
fn test_get_event_no_extension_when_ttl_disabled() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    // No set_event_ttl call — TTL disabled.
    let id = client.log_event(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"no_ext"),
        &None,
        &None,
        &false,
    );

    client.get_event(&id);

    let stats = client.get_cleanup_stats();
    assert_eq!(stats.ttl_extensions, 0, "TTL extensions should stay 0 when TTL is disabled");
}

/// Multiple reads accumulate the ttl_extensions counter.
#[test]
fn test_get_event_ttl_extension_counter_accumulates() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);
    let id = client.log_event(
        &submitter,
        &symbol_short!("pay"),
        &Bytes::from_slice(&env, b"multi_read"),
        &None,
        &None,
        &false,
    );

    client.get_event(&id);
    client.get_event(&id);
    client.get_event(&id);

    let stats = client.get_cleanup_stats();
    assert_eq!(stats.ttl_extensions, 3, "Each read should increment ttl_extensions");
}

/// batch_size limits the scan range of cleanup_expired_events.
#[test]
fn test_cleanup_expired_events_batch_size_respected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);
    // Log 10 events
    for i in 0..10u32 {
        client.log_event(
            &submitter,
            &symbol_short!("pay"),
            &Bytes::from_slice(&env, &[i as u8]),
            &None,
            &None,
            &false,
        );
    }

    // Run cleanup with batch_size=5, starting at 0 — should process indices 0..5 only.
    client.cleanup_expired_events(&owner, &0, &5);
    let stats = client.get_cleanup_stats();
    assert_eq!(stats.runs, 1);
}

/// cleanup_expired_events with start_index beyond total is a no-op (no panic).
#[test]
fn test_cleanup_expired_events_start_beyond_total_is_noop() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    client.set_event_ttl(&owner, &1000);
    // No events — start at 999
    let removed = client.cleanup_expired_events(&owner, &999, &100);
    assert_eq!(removed, 0);
    let stats = client.get_cleanup_stats();
    assert_eq!(stats.runs, 1);  // run was recorded even if nothing to clean
}

// ── Social impact ────────────────────────────────────────────────────────────

/// Build a minimal valid SocialImpactMetrics for use in tests.
fn make_metrics(env: &Env, period: soroban_sdk::Symbol, submitter: Address) -> SocialImpactMetrics {
    SocialImpactMetrics {
        period,
        recorded_at: env.ledger().timestamp(),
        submitter,
        jobs_created: 50,
        training_positions: 10,
        diversity_women_bps: 4500,
        diversity_underrepresented_bps: 3000,
        community_investment: 100_000,
        community_beneficiaries: 500,
        human_rights_assessment_done: true,
        labour_violations_remediated: 2,
        collective_bargaining_agreements: 3,
        total_investment: 200_000,
        total_social_value: 700_000,
    }
}

#[test]
fn test_record_social_impact_basic() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let period = symbol_short!("2026_Q1");
    let metrics = make_metrics(&env, period.clone(), owner.clone());

    let idx = client.record_social_impact(&owner, &metrics);
    assert_eq!(idx, 0);
    assert_eq!(client.social_impact_count(), 1);

    let retrieved = client.get_social_impact(&period);
    assert_eq!(retrieved.jobs_created, 50);
    assert_eq!(retrieved.total_investment, 200_000);
    assert_eq!(retrieved.total_social_value, 700_000);
    assert_eq!(retrieved.diversity_women_bps, 4500);
    assert!(retrieved.human_rights_assessment_done);
}

#[test]
fn test_record_social_impact_increments_count() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    assert_eq!(client.social_impact_count(), 0);

    let m1 = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    let m2 = make_metrics(&env, symbol_short!("2026_Q2"), owner.clone());

    client.record_social_impact(&owner, &m1);
    client.record_social_impact(&owner, &m2);

    assert_eq!(client.social_impact_count(), 2);
}

#[test]
fn test_record_social_impact_duplicate_period_fails() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let period = symbol_short!("2026_Q1");
    let m1 = make_metrics(&env, period.clone(), owner.clone());
    let m2 = make_metrics(&env, period.clone(), owner.clone());

    client.record_social_impact(&owner, &m1);
    let result = client.try_record_social_impact(&owner, &m2);
    assert!(result.is_err());
}

#[test]
fn test_record_social_impact_owner_only() {
    let (env, _owner, client) = create_ledger();
    let non_owner = Address::generate(&env);
    env.mock_all_auths();

    let metrics = make_metrics(&env, symbol_short!("2026_Q1"), non_owner.clone());
    let result = client.try_record_social_impact(&non_owner, &metrics);
    assert!(result.is_err());
}

#[test]
fn test_get_social_impact_not_found() {
    let (_env, _owner, client) = create_ledger();
    let result = client.try_get_social_impact(&symbol_short!("missing"));
    assert!(result.is_err());
}

#[test]
fn test_calculate_sroi_single_period() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    // investment = 200_000, social_value = 700_000 → SROI = 3.5 → bps = 35000
    let metrics = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    client.record_social_impact(&owner, &metrics);

    let mut periods = Vec::new(&env);
    periods.push_back(symbol_short!("2026_Q1"));

    let sroi = client.calculate_sroi(&periods);
    assert_eq!(sroi, 35_000u64);
}

#[test]
fn test_calculate_sroi_multiple_periods() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    // Q1: inv=200_000, val=700_000
    // Q2: inv=100_000, val=250_000
    // Total: inv=300_000, val=950_000 → SROI = 950_000/300_000*10_000 = 31666 bps
    let m1 = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    let mut m2 = make_metrics(&env, symbol_short!("2026_Q2"), owner.clone());
    m2.total_investment = 100_000;
    m2.total_social_value = 250_000;

    client.record_social_impact(&owner, &m1);
    client.record_social_impact(&owner, &m2);

    let mut periods = Vec::new(&env);
    periods.push_back(symbol_short!("2026_Q1"));
    periods.push_back(symbol_short!("2026_Q2"));

    let sroi = client.calculate_sroi(&periods);
    // 950_000 * 10_000 / 300_000 = 31_666
    assert_eq!(sroi, 31_666u64);
}

#[test]
fn test_calculate_sroi_zero_investment_fails() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let mut metrics = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    metrics.total_investment = 0;
    client.record_social_impact(&owner, &metrics);

    let mut periods = Vec::new(&env);
    periods.push_back(symbol_short!("2026_Q1"));

    let result = client.try_calculate_sroi(&periods);
    assert!(result.is_err());
}

#[test]
fn test_add_and_get_stakeholder() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let addr = Address::generate(&env);
    let stakeholder = Stakeholder {
        address: addr.clone(),
        name: Bytes::from_slice(&env, b"Community Group A"),
        category: symbol_short!("community"),
        weight_bps: 3000,
        registered_at: env.ledger().timestamp(),
    };

    let idx = client.add_stakeholder(&owner, &stakeholder);
    assert_eq!(idx, 0);
    assert_eq!(client.stakeholder_count(), 1);

    let retrieved = client.get_stakeholder(&addr);
    assert_eq!(retrieved.weight_bps, 3000);
    assert_eq!(retrieved.category, symbol_short!("community"));
}

#[test]
fn test_add_stakeholder_duplicate_fails() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let addr = Address::generate(&env);
    let s = Stakeholder {
        address: addr.clone(),
        name: Bytes::from_slice(&env, b"Worker Org"),
        category: symbol_short!("worker"),
        weight_bps: 5000,
        registered_at: env.ledger().timestamp(),
    };

    client.add_stakeholder(&owner, &s.clone());
    let result = client.try_add_stakeholder(&owner, &s);
    assert!(result.is_err());
}

#[test]
fn test_add_stakeholder_owner_only() {
    let (env, _owner, client) = create_ledger();
    let non_owner = Address::generate(&env);
    env.mock_all_auths();

    let s = Stakeholder {
        address: non_owner.clone(),
        name: Bytes::from_slice(&env, b"NGO"),
        category: symbol_short!("ngo"),
        weight_bps: 2000,
        registered_at: env.ledger().timestamp(),
    };

    let result = client.try_add_stakeholder(&non_owner, &s);
    assert!(result.is_err());
}

#[test]
fn test_get_stakeholder_not_found() {
    let (env, _owner, client) = create_ledger();
    let addr = Address::generate(&env);
    let result = client.try_get_stakeholder(&addr);
    assert!(result.is_err());
}

#[test]
fn test_generate_impact_report() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    // Register a stakeholder
    let stk_addr = Address::generate(&env);
    let stk = Stakeholder {
        address: stk_addr.clone(),
        name: Bytes::from_slice(&env, b"Regulator"),
        category: symbol_short!("regulatr"),
        weight_bps: 1000,
        registered_at: env.ledger().timestamp(),
    };
    client.add_stakeholder(&owner, &stk);

    // Record two periods
    let m1 = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    let mut m2 = make_metrics(&env, symbol_short!("2026_Q2"), owner.clone());
    m2.jobs_created = 30;
    m2.total_investment = 100_000;
    m2.total_social_value = 300_000;
    m2.diversity_women_bps = 5000;

    client.record_social_impact(&owner, &m1);
    client.record_social_impact(&owner, &m2);

    let mut periods = Vec::new(&env);
    periods.push_back(symbol_short!("2026_Q1"));
    periods.push_back(symbol_short!("2026_Q2"));

    let report = client.generate_impact_report(&owner, &periods);

    assert_eq!(report.periods_included, 2);
    assert_eq!(report.total_jobs_created, 80); // 50 + 30
    assert_eq!(report.total_community_investment, 200_000);
    assert_eq!(report.total_investment, 300_000);
    assert_eq!(report.total_social_value, 1_000_000);
    // SROI = 1_000_000 * 10_000 / 300_000 = 33_333
    assert_eq!(report.sroi_bps, 33_333u64);
    // avg diversity = (4500 + 5000) / 2 = 4750
    assert_eq!(report.avg_diversity_women_bps, 4750);
    assert_eq!(report.stakeholder_count, 1);

    // Should be persisted
    let stored = client.get_impact_report();
    assert!(stored.is_some());
    let stored_report = stored.unwrap();
    assert_eq!(stored_report.sroi_bps, 33_333u64);
}

#[test]
fn test_generate_impact_report_owner_only() {
    let (env, owner, client) = create_ledger();
    let non_owner = Address::generate(&env);
    env.mock_all_auths();

    let m1 = make_metrics(&env, symbol_short!("2026_Q1"), owner.clone());
    client.record_social_impact(&owner, &m1);

    let mut periods = Vec::new(&env);
    periods.push_back(symbol_short!("2026_Q1"));

    let result = client.try_generate_impact_report(&non_owner, &periods);
    assert!(result.is_err());
}

#[test]
fn test_get_impact_report_none_before_generation() {
    let (_env, _owner, client) = create_ledger();
    let report = client.get_impact_report();
    assert!(report.is_none());
}

#[test]
fn test_social_impact_count_initial_zero() {
    let (_env, _owner, client) = create_ledger();
    assert_eq!(client.social_impact_count(), 0);
}

#[test]
fn test_stakeholder_count_initial_zero() {
    let (_env, _owner, client) = create_ledger();
    assert_eq!(client.stakeholder_count(), 0);
}


// ── Modern slavery act compliance ────────────────────────────────────────

fn make_risk_assessment(
    env: &Env,
    assessment_id: soroban_sdk::Symbol,
    submitter: Address,
) -> RiskAssessment {
    RiskAssessment {
        assessment_id,
        recorded_at: env.ledger().timestamp(),
        submitter,
        scope: symbol_short!("global"),
        risk_level: 1,
        high_risk_areas: 3,
        key_risks: Bytes::from_slice(&env, b"supply chain concentration in asia"),
        planned_remediations: 2,
        stakeholder_consultation_done: true,
    }
}

fn make_supply_chain_node(
    env: &Env,
    supplier_id: soroban_sdk::Symbol,
) -> SupplyChainNode {
    SupplyChainNode {
        supplier_id,
        name: Bytes::from_slice(&env, b"Supplier Inc"),
        country: symbol_short!("CN"),
        risk_level: 2,
        audited: true,
        last_audit_date: env.ledger().timestamp(),
        registered_at: env.ledger().timestamp(),
    }
}

fn make_training_record(
    env: &Env,
    training_id: soroban_sdk::Symbol,
) -> TrainingRecord {
    TrainingRecord {
        training_id,
        delivered_at: env.ledger().timestamp(),
        topic: symbol_short!("msa_aware"),
        attendees: 150,
        risk_assessment_covered: true,
        due_diligence_covered: true,
        reporting_covered: true,
        content_summary: Bytes::from_slice(&env, b"comprehensive msa framework training"),
    }
}

fn make_due_diligence_record(
    env: &Env,
    record_id: soroban_sdk::Symbol,
) -> DueDiligenceRecord {
    DueDiligenceRecord {
        record_id,
        completed_at: env.ledger().timestamp(),
        subject: symbol_short!("supplier1"),
        scope: symbol_short!("labour_prc"),
        findings: Bytes::from_slice(&env, b"no critical issues found, minor training gaps identified"),
        risk_level: 1,
        corrective_actions_required: 2,
        corrective_actions_completed_pct: 50,
    }
}

fn make_msa_policy(
    env: &Env,
    policy_id: soroban_sdk::Symbol,
) -> MSAPolicy {
    MSAPolicy {
        policy_id,
        adopted_at: env.ledger().timestamp(),
        last_updated_at: env.ledger().timestamp(),
        version: 1,
        scope: symbol_short!("global"),
        content_summary: Bytes::from_slice(&env, b"comprehensive modern slavery prevention policy covering all operations and supply chain"),
        stakeholder_input_included: true,
    }
}

#[test]
fn test_record_and_get_risk_assessment() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let assessment = make_risk_assessment(&env, symbol_short!("2026_q1"), owner.clone());
    let idx = client.record_risk_assessment(&owner, &assessment);
    assert_eq!(idx, 0);
    assert_eq!(client.msa_risk_assessment_count(), 1);

    let retrieved = client.get_risk_assessment(&assessment.assessment_id);
    assert_eq!(retrieved.risk_level, 1);
    assert_eq!(retrieved.high_risk_areas, 3);
    assert!(retrieved.stakeholder_consultation_done);
}

#[test]
fn test_risk_assessment_duplicate_fails() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let a1 = make_risk_assessment(&env, symbol_short!("assess1"), owner.clone());
    let a2 = make_risk_assessment(&env, symbol_short!("assess1"), owner.clone());

    client.record_risk_assessment(&owner, &a1);
    let result = client.try_record_risk_assessment(&owner, &a2);
    assert!(result.is_err());
}

#[test]
fn test_record_supply_chain_node() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let node = make_supply_chain_node(&env, symbol_short!("supplier1"));
    let idx = client.record_supply_chain_node(&owner, &node);
    assert_eq!(idx, 0);
    assert_eq!(client.msa_supply_chain_node_count(), 1);

    let retrieved = client.get_supply_chain_node(&node.supplier_id);
    assert_eq!(retrieved.risk_level, 2);
    assert!(retrieved.audited);
}

#[test]
fn test_record_training() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let training = make_training_record(&env, symbol_short!("train001"));
    let idx = client.record_training(&owner, &training);
    assert_eq!(idx, 0);
    assert_eq!(client.msa_training_record_count(), 1);

    let retrieved = client.get_training_record(&training.training_id);
    assert_eq!(retrieved.attendees, 150);
    assert!(retrieved.risk_assessment_covered);
    assert!(retrieved.due_diligence_covered);
}

#[test]
fn test_submit_due_diligence() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let dd = make_due_diligence_record(&env, symbol_short!("dd_2026_001"));
    let idx = client.submit_due_diligence(&owner, &dd);
    assert_eq!(idx, 0);
    assert_eq!(client.msa_due_diligence_count(), 1);

    let retrieved = client.get_due_diligence_record(&dd.record_id);
    assert_eq!(retrieved.risk_level, 1);
    assert_eq!(retrieved.corrective_actions_completed_pct, 50);
}

#[test]
fn test_record_msa_policy() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let policy = make_msa_policy(&env, symbol_short!("policy01"));
    let idx = client.record_msa_policy(&owner, &policy);
    assert_eq!(idx, 0);
    assert_eq!(client.msa_policy_count(), 1);

    let retrieved = client.get_msa_policy(&policy.policy_id);
    assert_eq!(retrieved.version, 1);
    assert!(retrieved.stakeholder_input_included);
}

#[test]
fn test_build_msa_report() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    // Record one of each component
    let assess = make_risk_assessment(&env, symbol_short!("assess1"), owner.clone());
    let node = make_supply_chain_node(&env, symbol_short!("supp01"));
    let train = make_training_record(&env, symbol_short!("train01"));
    let dd = make_due_diligence_record(&env, symbol_short!("dd01"));
    let policy = make_msa_policy(&env, symbol_short!("pol01"));

    client.record_risk_assessment(&owner, &assess);
    client.record_supply_chain_node(&owner, &node);
    client.record_training(&owner, &train);
    client.submit_due_diligence(&owner, &dd);
    client.record_msa_policy(&owner, &policy);

    let report = client.build_msa_report(&owner);
    assert_eq!(report.assessments_count, 1);
    assert_eq!(report.supply_chain_nodes, 1);
    assert_eq!(report.due_diligence_investigations, 1);
    assert_eq!(report.active_policies, 1);
}

#[test]
fn test_get_msa_report() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    // Initially None
    assert!(client.get_msa_report().is_none());

    // Generate report
    client.build_msa_report(&owner);

    // Now should be Some
    let report = client.get_msa_report();
    assert!(report.is_some());
    assert_eq!(report.unwrap().assessments_count, 0);
}

#[test]
fn test_msa_counts_increment() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    assert_eq!(client.msa_risk_assessment_count(), 0);
    assert_eq!(client.msa_supply_chain_node_count(), 0);
    assert_eq!(client.msa_training_record_count(), 0);
    assert_eq!(client.msa_due_diligence_count(), 0);
    assert_eq!(client.msa_policy_count(), 0);

    // Record one of each
    let a = make_risk_assessment(&env, symbol_short!("a1"), owner.clone());
    let n = make_supply_chain_node(&env, symbol_short!("n1"));
    let t = make_training_record(&env, symbol_short!("t1"));
    let d = make_due_diligence_record(&env, symbol_short!("d1"));
    let p = make_msa_policy(&env, symbol_short!("p1"));

    client.record_risk_assessment(&owner, &a);
    client.record_supply_chain_node(&owner, &n);
    client.record_training(&owner, &t);
    client.submit_due_diligence(&owner, &d);
    client.record_msa_policy(&owner, &p);

    // All should be 1
    assert_eq!(client.msa_risk_assessment_count(), 1);
    assert_eq!(client.msa_supply_chain_node_count(), 1);
    assert_eq!(client.msa_training_record_count(), 1);
    assert_eq!(client.msa_due_diligence_count(), 1);
    assert_eq!(client.msa_policy_count(), 1);

    // Record second of each
    let a2 = make_risk_assessment(&env, symbol_short!("a2"), owner.clone());
    let n2 = make_supply_chain_node(&env, symbol_short!("n2"));
    let t2 = make_training_record(&env, symbol_short!("t2"));
    let d2 = make_due_diligence_record(&env, symbol_short!("d2"));
    let p2 = make_msa_policy(&env, symbol_short!("p2"));

    client.record_risk_assessment(&owner, &a2);
    client.record_supply_chain_node(&owner, &n2);
    client.record_training(&owner, &t2);
    client.submit_due_diligence(&owner, &d2);
    client.record_msa_policy(&owner, &p2);

    // All should be 2
    assert_eq!(client.msa_risk_assessment_count(), 2);
    assert_eq!(client.msa_supply_chain_node_count(), 2);
    assert_eq!(client.msa_training_record_count(), 2);
    assert_eq!(client.msa_due_diligence_count(), 2);
    assert_eq!(client.msa_policy_count(), 2);
}

#[test]
fn test_msa_owner_only_access() {
    let (env, owner, client) = create_ledger();
    let non_owner = Address::generate(&env);
    env.mock_all_auths();

    let a = make_risk_assessment(&env, symbol_short!("a1"), non_owner.clone());
    let result = client.try_record_risk_assessment(&non_owner, &a);
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_risk_assessment_fails() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_risk_assessment(&symbol_short!("missing"));
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_supply_chain_node_fails() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_supply_chain_node(&symbol_short!("missing"));
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_training_fails() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_training_record(&symbol_short!("missing"));
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_due_diligence_fails() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_due_diligence_record(&symbol_short!("missing"));
    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_policy_fails() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_get_msa_policy(&symbol_short!("missing"));
    assert!(result.is_err());
}
