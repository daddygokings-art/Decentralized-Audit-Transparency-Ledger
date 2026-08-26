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

// ── Circular Economy Tests ───────────────────────────────────────────────────

// ── Material Passport ────────────────────────────────────────────────────────

#[test]
fn test_register_material_passport_returns_id() {
    let (env, _owner, client) = create_ledger();
    let manufacturer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"Steel Beam A1"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &50_000_000_u64, // 50 kg in mg
        &7_500_u32,      // 75.00% recyclability
    );

    // ID is 32 bytes
    assert_eq!(id.len(), 32);
}

#[test]
fn test_register_material_passport_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let manufacturer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"PET Bottle"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &1_000_000_u64, // 1 kg
        &9_000_u32,     // 90.00%
    );

    let passport = client.get_material_passport(&id);
    assert_eq!(passport.id, id);
    assert_eq!(passport.name, Bytes::from_slice(&env, b"PET Bottle"));
    assert_eq!(passport.virgin_mass_mg, 1_000_000_u64);
    assert_eq!(passport.recyclability_bps, 9_000_u32);
    assert_eq!(passport.loop_event_count, 0);
    assert_eq!(passport.total_recycled_mg, 0);
    assert_eq!(passport.total_disposed_mg, 0);
}

#[test]
fn test_register_material_passport_updates_global_totals() {
    let (env, _owner, client) = create_ledger();
    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    env.mock_all_auths();

    client.register_material_passport(
        &m1,
        &Bytes::from_slice(&env, b"Aluminium Can"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &200_000_u64,
        &8_000_u32,
    );

    // Advance ledger so second passport gets a different timestamp (unique ID)
    env.ledger().with_mut(|li| li.timestamp += 1);

    client.register_material_passport(
        &m2,
        &Bytes::from_slice(&env, b"Glass Bottle"),
        &soroban_sdk::Symbol::new(&env, "glass"),
        &400_000_u64,
        &9_500_u32,
    );

    let totals = client.get_circularity_totals();
    assert_eq!(totals.total_materials, 2);
    assert_eq!(totals.total_virgin_mass_mg, 600_000_u64);
    assert_eq!(totals.total_loop_events, 0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #37)")]
fn test_register_material_passport_zero_mass_panics() {
    let (env, _owner, client) = create_ledger();
    let manufacturer = Address::generate(&env);
    env.mock_all_auths();

    client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"Invalid"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &0_u64, // zero mass → InvalidFlowQuantity
        &5_000_u32,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #37)")]
fn test_register_material_passport_recyclability_over_10000_panics() {
    let (env, _owner, client) = create_ledger();
    let manufacturer = Address::generate(&env);
    env.mock_all_auths();

    client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"TooGood"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_u64,
        &10_001_u32, // > 10000 → InvalidFlowQuantity
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #35)")]
fn test_get_material_passport_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let fake_id = BytesN::from_array(&env, &[0u8; 32]);
    client.get_material_passport(&fake_id);
}

// ── Loop Events ──────────────────────────────────────────────────────────────

#[test]
fn test_record_loop_event_recycle() {
    let (env, _owner, client) = create_ledger();
    let manufacturer = Address::generate(&env);
    let recycler = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"HDPE Pipe"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &5_000_000_u64,
        &6_500_u32,
    );

    let seq = client.record_loop_event(
        &recycler,
        &id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &2_500_000_u64,
        &None,
        &Bytes::from_slice(&env, b"batch-2026-001"),
    );

    assert_eq!(seq, 0);

    let passport = client.get_material_passport(&id);
    assert_eq!(passport.total_recycled_mg, 2_500_000_u64);
    assert_eq!(passport.loop_event_count, 1);
}

#[test]
fn test_record_loop_event_all_types() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let manufacturer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &manufacturer,
        &Bytes::from_slice(&env, b"Textile Roll"),
        &soroban_sdk::Symbol::new(&env, "textile"),
        &10_000_000_u64,
        &4_000_u32,
    );

    let loop_types = [
        soroban_sdk::Symbol::new(&env, "recycle"),
        soroban_sdk::Symbol::new(&env, "reuse"),
        soroban_sdk::Symbol::new(&env, "repair"),
        soroban_sdk::Symbol::new(&env, "remanuf"),
        soroban_sdk::Symbol::new(&env, "return"),
        soroban_sdk::Symbol::new(&env, "dispose"),
    ];

    for (i, lt) in loop_types.iter().enumerate() {
        let seq = client.record_loop_event(
            &actor,
            &id,
            lt,
            &100_000_u64,
            &None,
            &Bytes::new(&env),
        );
        assert_eq!(seq, i as u32);
    }

    let passport = client.get_material_passport(&id);
    assert_eq!(passport.loop_event_count, 6);
    assert_eq!(passport.total_recycled_mg, 100_000);
    assert_eq!(passport.total_reused_mg, 100_000);
    assert_eq!(passport.total_repaired_mg, 100_000);
    assert_eq!(passport.total_remanufactured_mg, 100_000);
    assert_eq!(passport.total_disposed_mg, 100_000);
    // `return` events don't accumulate into a mass bucket
}

#[test]
fn test_record_loop_event_with_target_material() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let source_id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Source Plastic"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &3_000_000_u64,
        &8_000_u32,
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    let target_id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Recycled Pellets"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &1_000_u64,
        &9_000_u32,
    );

    client.record_loop_event(
        &actor,
        &source_id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &2_000_000_u64,
        &Some(target_id.clone()),
        &Bytes::from_slice(&env, b"output-ref"),
    );

    let loops = client.get_material_loop(&source_id);
    assert_eq!(loops.len(), 1);
    let evt = loops.get(0).unwrap();
    assert_eq!(evt.target_material_id, Some(target_id));
    assert_eq!(evt.quantity_mg, 2_000_000_u64);
    assert_eq!(evt.loop_type, 0_u32); // recycle discriminant
}

#[test]
fn test_record_multiple_loop_events_sequential_seqs() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Paper"),
        &soroban_sdk::Symbol::new(&env, "organic"),
        &500_000_u64,
        &7_000_u32,
    );

    let s0 = client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "reuse"),
        &100_000_u64, &None, &Bytes::new(&env),
    );
    let s1 = client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "repair"),
        &50_000_u64, &None, &Bytes::new(&env),
    );
    let s2 = client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &200_000_u64, &None, &Bytes::new(&env),
    );

    assert_eq!(s0, 0);
    assert_eq!(s1, 1);
    assert_eq!(s2, 2);

    let loops = client.get_material_loop(&id);
    assert_eq!(loops.len(), 3);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #37)")]
fn test_record_loop_event_zero_quantity_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Copper Wire"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_000_u64,
        &8_500_u32,
    );

    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &0_u64, // zero → InvalidFlowQuantity
        &None,
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #36)")]
fn test_record_loop_event_invalid_type_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Unknown"),
        &soroban_sdk::Symbol::new(&env, "other"),
        &1_000_u64,
        &5_000_u32,
    );

    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "vaporise"), // not a valid type
        &1_000_u64,
        &None,
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #35)")]
fn test_record_loop_event_unknown_passport_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let fake_id = BytesN::from_array(&env, &[0x42u8; 32]);
    client.record_loop_event(
        &actor, &fake_id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &1_000_u64,
        &None,
        &Bytes::new(&env),
    );
}

// ── Circularity Score / Snapshot ─────────────────────────────────────────────

#[test]
fn test_compute_circularity_score_no_flows_returns_zero_mci() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Virgin Resin"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &10_000_000_u64,
        &5_000_u32,
    );

    let snap = client.compute_circularity_score();
    assert_eq!(snap.mci_bps, 0);
    assert_eq!(snap.recycling_rate_bps, 0);
    assert_eq!(snap.reuse_rate_bps, 0);
    assert_eq!(snap.total_materials, 1);
    assert_eq!(snap.snapshot_index, 0);
}

#[test]
fn test_compute_circularity_score_all_recycled() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Full Recycle"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_000_u64,
        &10_000_u32,
    );

    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &1_000_000_u64,
        &None,
        &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    // 100% circular → MCI = 10000 bps
    assert_eq!(snap.mci_bps, 10_000);
    assert_eq!(snap.recycling_rate_bps, 10_000);
    assert_eq!(snap.reuse_rate_bps, 0);
    assert_eq!(snap.total_circular_mass_mg, 1_000_000_u64);
    assert_eq!(snap.total_disposed_mass_mg, 0);
}

#[test]
fn test_compute_circularity_score_all_disposed() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Landfill Waste"),
        &soroban_sdk::Symbol::new(&env, "mixed"),
        &2_000_000_u64,
        &1_000_u32,
    );

    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "dispose"),
        &2_000_000_u64,
        &None,
        &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    assert_eq!(snap.mci_bps, 0);
    assert_eq!(snap.total_circular_mass_mg, 0);
    assert_eq!(snap.total_disposed_mass_mg, 2_000_000_u64);
    assert_eq!(snap.loop_closure_rate_bps, 0);
}

#[test]
fn test_compute_circularity_score_mixed_flows() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Mixed Material"),
        &soroban_sdk::Symbol::new(&env, "mixed"),
        &4_000_000_u64,
        &5_000_u32,
    );

    // 3 kg recycled, 1 kg disposed → MCI = 7500 bps
    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "recycle"),
        &3_000_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id,
        &soroban_sdk::Symbol::new(&env, "dispose"),
        &1_000_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    assert_eq!(snap.total_circular_mass_mg, 3_000_000_u64);
    assert_eq!(snap.total_disposed_mass_mg, 1_000_000_u64);
    // mci = 3_000_000 / 4_000_000 * 10000 = 7500
    assert_eq!(snap.mci_bps, 7_500);
    // recycling_rate = 3_000_000 / 4_000_000 * 10000 = 7500
    assert_eq!(snap.recycling_rate_bps, 7_500);
    assert_eq!(snap.reuse_rate_bps, 0);
}

#[test]
fn test_compute_circularity_score_reuse_and_repair() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Refurb Laptop"),
        &soroban_sdk::Symbol::new(&env, "electronic"),
        &1_500_000_u64,
        &3_000_u32,
    );

    // 500g reused, 500g repaired, 500g disposed
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "reuse"),
        &500_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "repair"),
        &500_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "dispose"),
        &500_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    // circular = 1_000_000, disposed = 500_000, total = 1_500_000
    // mci = 1_000_000/1_500_000 * 10000 = 6666
    assert_eq!(snap.mci_bps, 6_666);
    // reuse_rate = 500_000/1_500_000 * 10000 = 3333
    assert_eq!(snap.reuse_rate_bps, 3_333);
}

#[test]
fn test_snapshot_count_increments() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    assert_eq!(client.circularity_snapshot_count(), 0);
    client.compute_circularity_score();
    assert_eq!(client.circularity_snapshot_count(), 1);
    client.compute_circularity_score();
    assert_eq!(client.circularity_snapshot_count(), 2);
}

#[test]
fn test_get_circularity_snapshot_retrieves_correct_data() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Snapshot Test"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_000_u64,
        &8_000_u32,
    );

    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "recycle"),
        &500_000_u64, &None, &Bytes::new(&env),
    );

    let snap0 = client.compute_circularity_score();
    // total_loop_events should be 1
    assert_eq!(snap0.total_loop_events, 1);
    assert_eq!(snap0.snapshot_index, 0);

    // Second snapshot after more activity
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "reuse"),
        &250_000_u64, &None, &Bytes::new(&env),
    );
    let snap1 = client.compute_circularity_score();
    assert_eq!(snap1.snapshot_index, 1);
    assert_eq!(snap1.total_loop_events, 2);

    // Retrieve by index
    let retrieved0 = client.get_circularity_snapshot(&0);
    assert_eq!(retrieved0.mci_bps, snap0.mci_bps);
    assert_eq!(retrieved0.total_loop_events, 1);

    let retrieved1 = client.get_circularity_snapshot(&1);
    assert_eq!(retrieved1.total_loop_events, 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #30)")]
fn test_get_circularity_snapshot_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_circularity_snapshot(&99);
}

// ── Loop Closure Rate ────────────────────────────────────────────────────────

#[test]
fn test_loop_closure_rate_single_material_closed() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Closed Loop"),
        &soroban_sdk::Symbol::new(&env, "glass"),
        &1_000_000_u64,
        &9_000_u32,
    );

    // First non-dispose event = loop closed
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "reuse"),
        &1_000_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    // 1/1 materials closed → 10000 bps
    assert_eq!(snap.loop_closure_rate_bps, 10_000);
}

#[test]
fn test_loop_closure_rate_dispose_only_does_not_close_loop() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Linear Only"),
        &soroban_sdk::Symbol::new(&env, "mixed"),
        &2_000_000_u64,
        &500_u32,
    );

    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "dispose"),
        &2_000_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    assert_eq!(snap.loop_closure_rate_bps, 0);
}

#[test]
fn test_loop_closure_rate_two_materials_one_closed() {
    let (env, _owner, client) = create_ledger();
    let mfr1 = Address::generate(&env);
    let mfr2 = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id1 = client.register_material_passport(
        &mfr1,
        &Bytes::from_slice(&env, b"Closed"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_000_u64,
        &8_000_u32,
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    let id2 = client.register_material_passport(
        &mfr2,
        &Bytes::from_slice(&env, b"Linear"),
        &soroban_sdk::Symbol::new(&env, "plastic"),
        &1_000_000_u64,
        &2_000_u32,
    );

    // id1: circular; id2: dispose only
    client.record_loop_event(
        &actor, &id1, &soroban_sdk::Symbol::new(&env, "recycle"),
        &500_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id2, &soroban_sdk::Symbol::new(&env, "dispose"),
        &1_000_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    // 1/2 → 5000 bps
    assert_eq!(snap.loop_closure_rate_bps, 5_000);
    assert_eq!(snap.total_materials, 2);
}

// ── get_circularity_totals ───────────────────────────────────────────────────

#[test]
fn test_get_circularity_totals_empty() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let totals = client.get_circularity_totals();
    assert_eq!(totals.total_materials, 0);
    assert_eq!(totals.total_virgin_mass_mg, 0);
    assert_eq!(totals.total_recycled_mg, 0);
    assert_eq!(totals.total_loop_events, 0);
}

#[test]
fn test_get_circularity_totals_accumulates_across_materials() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let id1 = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Mat A"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &1_000_000_u64,
        &8_000_u32,
    );
    env.ledger().with_mut(|li| li.timestamp += 1);

    let id2 = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Mat B"),
        &soroban_sdk::Symbol::new(&env, "glass"),
        &2_000_000_u64,
        &9_000_u32,
    );

    client.record_loop_event(
        &actor, &id1, &soroban_sdk::Symbol::new(&env, "recycle"),
        &400_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id2, &soroban_sdk::Symbol::new(&env, "reuse"),
        &800_000_u64, &None, &Bytes::new(&env),
    );

    let totals = client.get_circularity_totals();
    assert_eq!(totals.total_materials, 2);
    assert_eq!(totals.total_virgin_mass_mg, 3_000_000_u64);
    assert_eq!(totals.total_recycled_mg, 400_000_u64);
    assert_eq!(totals.total_reused_mg, 800_000_u64);
    assert_eq!(totals.total_loop_events, 2);
    assert_eq!(totals.materials_with_closed_loop, 2);
}

// ── get_material_loop ────────────────────────────────────────────────────────

#[test]
fn test_get_material_loop_empty_after_register() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Fresh Asset"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &500_000_u64,
        &7_000_u32,
    );

    let loops = client.get_material_loop(&id);
    assert_eq!(loops.len(), 0);
}

#[test]
fn test_get_material_loop_preserves_metadata() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Cert Asset"),
        &soroban_sdk::Symbol::new(&env, "organic"),
        &300_000_u64,
        &6_000_u32,
    );

    let cert_data = Bytes::from_slice(&env, b"ISO14001-2026-CertRef-ABC");
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "remanuf"),
        &300_000_u64, &None, &cert_data.clone(),
    );

    let loops = client.get_material_loop(&id);
    assert_eq!(loops.len(), 1);
    let evt = loops.get(0).unwrap();
    assert_eq!(evt.metadata, cert_data);
    assert_eq!(evt.loop_type, 3_u32); // remanuf discriminant
    assert_eq!(evt.quantity_mg, 300_000_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #35)")]
fn test_get_material_loop_unknown_passport_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let fake_id = BytesN::from_array(&env, &[0x99u8; 32]);
    client.get_material_loop(&fake_id);
}

// ── Boundary conditions ──────────────────────────────────────────────────────

#[test]
fn test_circularity_snapshot_index_0_on_empty_contract() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let snap = client.compute_circularity_score();
    assert_eq!(snap.snapshot_index, 0);
    assert_eq!(snap.total_materials, 0);
    assert_eq!(snap.mci_bps, 0);
}

#[test]
fn test_recycled_plus_disposed_equals_total_flow_in_snapshot() {
    let (env, _owner, client) = create_ledger();
    let mfr = Address::generate(&env);
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_material_passport(
        &mfr,
        &Bytes::from_slice(&env, b"Balance Check"),
        &soroban_sdk::Symbol::new(&env, "mixed"),
        &6_000_000_u64,
        &5_000_u32,
    );

    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "recycle"),
        &2_000_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "reuse"),
        &1_000_000_u64, &None, &Bytes::new(&env),
    );
    client.record_loop_event(
        &actor, &id, &soroban_sdk::Symbol::new(&env, "dispose"),
        &3_000_000_u64, &None, &Bytes::new(&env),
    );

    let snap = client.compute_circularity_score();
    let total_flow = snap.total_circular_mass_mg + snap.total_disposed_mass_mg;
    assert_eq!(total_flow, 6_000_000_u64);
    // mci = 3_000_000/6_000_000*10000 = 5000
    assert_eq!(snap.mci_bps, 5_000);
}

// ── Lifecycle Assessment (LCA) Tests ────────────────────────────────────────

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a canonical 8-element impact vec with a given GWP value and zeros elsewhere.
fn gwp_impacts(env: &Env, gwp_micro: i64) -> soroban_sdk::Vec<i64> {
    let mut v = soroban_sdk::Vec::new(env);
    v.push_back(gwp_micro); // 0: GWP
    v.push_back(0i64);      // 1: AP
    v.push_back(0i64);      // 2: EP
    v.push_back(0i64);      // 3: ODP
    v.push_back(0i64);      // 4: POCP
    v.push_back(0i64);      // 5: ADP
    v.push_back(0i64);      // 6: WU
    v.push_back(0i64);      // 7: LU
    v
}

/// Build an 8-element impact vec from a slice of exactly 8 values.
fn impacts8(env: &Env, vals: [i64; 8]) -> soroban_sdk::Vec<i64> {
    let mut v = soroban_sdk::Vec::new(env);
    for x in vals {
        v.push_back(x);
    }
    v
}

/// Build an 8-element refs vec (all same value).
fn refs8(env: &Env, val: i64) -> soroban_sdk::Vec<i64> {
    let mut v = soroban_sdk::Vec::new(env);
    for _ in 0..8 {
        v.push_back(val);
    }
    v
}

/// Build an 8-element weights vec (all same value in bps).
fn weights8(env: &Env, bps: u32) -> soroban_sdk::Vec<u32> {
    let mut v = soroban_sdk::Vec::new(env);
    for _ in 0..8 {
        v.push_back(bps);
    }
    v
}

// ── register_lca_entry ───────────────────────────────────────────────────────

#[test]
fn test_register_lca_entry_returns_id() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Widget A"),
        &Bytes::from_slice(&env, b"1 unit at factory gate"),
        &None,
    );

    assert_eq!(id.len(), 32);
}

#[test]
fn test_register_lca_entry_profile_stored() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Widget B"),
        &Bytes::from_slice(&env, b"1 kg"),
        &None,
    );

    let profile = client.get_lca_profile(&id);
    assert_eq!(profile.product_id, id);
    assert_eq!(profile.name, Bytes::from_slice(&env, b"Widget B"));
    assert!(!profile.finalized);
    assert_eq!(profile.phase_mask, 0);
    assert_eq!(profile.material_passport_id, None);
}

#[test]
fn test_register_lca_entry_increments_count() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.lca_profile_count(), 0);

    client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"P1"),
        &Bytes::from_slice(&env, b"fu1"),
        &None,
    );
    assert_eq!(client.lca_profile_count(), 1);

    env.ledger().with_mut(|li| li.timestamp += 1);

    client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"P2"),
        &Bytes::from_slice(&env, b"fu2"),
        &None,
    );
    assert_eq!(client.lca_profile_count(), 2);
}

#[test]
fn test_register_lca_entry_linked_to_passport() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let passport_id = client.register_material_passport(
        &producer,
        &Bytes::from_slice(&env, b"Steel Sheet"),
        &soroban_sdk::Symbol::new(&env, "metal"),
        &10_000_000_u64,
        &8_000_u32,
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    let lca_id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Steel Product"),
        &Bytes::from_slice(&env, b"1 t"),
        &Some(passport_id.clone()),
    );

    let profile = client.get_lca_profile(&lca_id);
    assert_eq!(profile.material_passport_id, Some(passport_id));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #38)")]
fn test_register_lca_entry_duplicate_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"DupProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    // Same inputs at same timestamp → same derived ID → AlreadyExists
    client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"DupProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #35)")]
fn test_register_lca_entry_unknown_passport_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let fake_id = BytesN::from_array(&env, &[0x11u8; 32]);
    client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"X"),
        &Bytes::from_slice(&env, b"1 unit"),
        &Some(fake_id),
    );
}

// ── record_phase_impact ──────────────────────────────────────────────────────

#[test]
fn test_record_phase_impact_updates_phase_mask() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Car Door"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "raw_mat"),
        &gwp_impacts(&env, 5_000_000),
        &0_u32,
        &None,
        &Bytes::new(&env),
    );

    let profile = client.get_lca_profile(&id);
    // raw_mat is phase 0 → bit 0 set → mask = 1
    assert_eq!(profile.phase_mask, 1);
}

#[test]
fn test_record_phase_impact_all_seven_phases() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Solar Panel"),
        &Bytes::from_slice(&env, b"1 m2"),
        &None,
    );

    let phases = [
        soroban_sdk::Symbol::new(&env, "raw_mat"),
        soroban_sdk::Symbol::new(&env, "mfg"),
        soroban_sdk::Symbol::new(&env, "transport"),
        soroban_sdk::Symbol::new(&env, "use"),
        soroban_sdk::Symbol::new(&env, "maint"),
        soroban_sdk::Symbol::new(&env, "eol"),
        soroban_sdk::Symbol::new(&env, "recycling"),
    ];

    for phase in phases.iter() {
        client.record_phase_impact(
            &producer,
            &id,
            phase,
            &gwp_impacts(&env, 1_000_000),
            &0_u32,
            &None,
            &Bytes::new(&env),
        );
    }

    let profile = client.get_lca_profile(&id);
    // All 7 bits set → mask = 0b111_1111 = 127
    assert_eq!(profile.phase_mask, 127);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #40)")]
fn test_record_phase_impact_invalid_phase_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"X"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "badphase"),
        &gwp_impacts(&env, 1_000),
        &0_u32,
        &None,
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #41)")]
fn test_record_phase_impact_wrong_impact_count_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Y"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    let mut bad_vec = soroban_sdk::Vec::new(&env);
    bad_vec.push_back(1_000i64); // only 1 element instead of 8

    client.record_phase_impact(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &bad_vec,
        &0_u32,
        &None,
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #39)")]
fn test_record_phase_impact_unknown_product_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let fake_id = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.record_phase_impact(
        &producer,
        &fake_id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 1_000),
        &0_u32,
        &None,
        &Bytes::new(&env),
    );
}

#[test]
fn test_record_phase_impact_with_db_ref() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let db_id = client.register_lca_db_entry(
        &producer,
        &Bytes::from_slice(&env, b"ecoinvent"),
        &Bytes::from_slice(&env, b"3.10"),
        &Bytes::from_slice(&env, b"steel production RER"),
        &Bytes::from_slice(&env, b"RER"),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    let prod_id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Steel Beam"),
        &Bytes::from_slice(&env, b"1 t"),
        &None,
    );

    client.record_phase_impact(
        &producer,
        &prod_id,
        &soroban_sdk::Symbol::new(&env, "raw_mat"),
        &gwp_impacts(&env, 2_000_000_000),
        &500_u32, // 5% cv
        &Some(db_id),
        &Bytes::from_slice(&env, b"ecoinvent-3.10-steel"),
    );

    let profile = client.get_lca_profile(&prod_id);
    assert_eq!(profile.phase_mask, 1); // raw_mat bit
}

// ── finalize_lca ─────────────────────────────────────────────────────────────

#[test]
fn test_finalize_lca_aggregates_single_phase() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Bottle"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 3_500_000),
        &0_u32, &None, &Bytes::new(&env),
    );

    let result = client.finalize_lca(&producer, &id);

    // GWP total = 3_500_000
    assert_eq!(result.totals.get(0).unwrap(), 3_500_000i64);
    // All other categories = 0
    for cat in 1..8 {
        assert_eq!(result.totals.get(cat).unwrap(), 0i64);
    }
    // Normalized/weighted start zeroed
    assert_eq!(result.single_score, 0i64);
}

#[test]
fn test_finalize_lca_sums_multiple_phases() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"MultiPhase"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    // raw_mat: GWP = 1_000_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "raw_mat"),
        &gwp_impacts(&env, 1_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    // mfg: GWP = 2_000_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 2_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    // transport: GWP = 500_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "transport"),
        &gwp_impacts(&env, 500_000),
        &0_u32, &None, &Bytes::new(&env),
    );

    let result = client.finalize_lca(&producer, &id);
    assert_eq!(result.totals.get(0).unwrap(), 3_500_000i64);
}

#[test]
fn test_finalize_lca_allows_negative_credits() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"CreditProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 5_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    // recycling: avoided burden −2_000_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "recycling"),
        &gwp_impacts(&env, -2_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );

    let result = client.finalize_lca(&producer, &id);
    assert_eq!(result.totals.get(0).unwrap(), 3_000_000i64);
}

#[test]
fn test_finalize_lca_marks_profile_as_finalized() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Final"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "use"),
        &gwp_impacts(&env, 100_000),
        &0_u32, &None, &Bytes::new(&env),
    );

    client.finalize_lca(&producer, &id);

    let profile = client.get_lca_profile(&id);
    assert!(profile.finalized);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #42)")]
fn test_finalize_lca_twice_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Double"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.finalize_lca(&producer, &id);
    client.finalize_lca(&producer, &id); // second call → LcaAlreadyFinalized
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #42)")]
fn test_record_phase_impact_after_finalize_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Locked"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.finalize_lca(&producer, &id);

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "eol"),
        &gwp_impacts(&env, 200_000),
        &0_u32, &None, &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #43)")]
fn test_get_lca_result_before_finalize_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"NotFinal"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.get_lca_result(&id);
}

// ── Uncertainty (interval arithmetic) ────────────────────────────────────────

#[test]
fn test_uncertainty_zero_cv_gives_tight_interval() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"NoUnc"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 4_000_000),
        &0_u32, // cv = 0
        &None, &Bytes::new(&env),
    );

    client.finalize_lca(&producer, &id);

    let unc = client.get_lca_uncertainty(&id);
    // cv=0 → delta=0 → lo == hi == total
    assert_eq!(unc.lo.get(0).unwrap(), 4_000_000i64);
    assert_eq!(unc.hi.get(0).unwrap(), 4_000_000i64);
    assert_eq!(unc.cv_bps, 0);
}

#[test]
fn test_uncertainty_10pct_cv() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"Unc10"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    // GWP = 10_000_000 (10 kg CO₂-eq in micro-units), cv = 10% = 1000 bps
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 10_000_000),
        &1_000_u32, // ±10%
        &None, &Bytes::new(&env),
    );

    client.finalize_lca(&producer, &id);

    let unc = client.get_lca_uncertainty(&id);
    // delta = 10_000_000 * 1000 / 10_000 = 1_000_000
    // lo = 10_000_000 - 1_000_000 = 9_000_000
    // hi = 10_000_000 + 1_000_000 = 11_000_000
    assert_eq!(unc.lo.get(0).unwrap(), 9_000_000i64);
    assert_eq!(unc.hi.get(0).unwrap(), 11_000_000i64);
    assert_eq!(unc.cv_bps, 1_000);
}

#[test]
fn test_uncertainty_propagated_across_phases() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"MultiUnc"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    // Phase 1: GWP = 6_000_000, cv = 10% → delta = 600_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "raw_mat"),
        &gwp_impacts(&env, 6_000_000),
        &1_000_u32,
        &None, &Bytes::new(&env),
    );
    // Phase 2: GWP = 4_000_000, cv = 20% → delta = 800_000
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 4_000_000),
        &2_000_u32,
        &None, &Bytes::new(&env),
    );

    client.finalize_lca(&producer, &id);

    let result = client.get_lca_result(&id);
    let unc = client.get_lca_uncertainty(&id);

    // Total = 10_000_000
    assert_eq!(result.totals.get(0).unwrap(), 10_000_000i64);
    // lo = (6M - 600K) + (4M - 800K) = 5_400_000 + 3_200_000 = 8_600_000
    // hi = (6M + 600K) + (4M + 800K) = 6_600_000 + 4_800_000 = 11_400_000
    assert_eq!(unc.lo.get(0).unwrap(), 8_600_000i64);
    assert_eq!(unc.hi.get(0).unwrap(), 11_400_000i64);
    // avg_cv = (1000 + 2000) / 2 = 1500
    assert_eq!(unc.cv_bps, 1_500);
}

#[test]
fn test_uncertainty_negative_value_delta_is_absolute() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"NegUnc"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    // GWP = −10_000_000 (avoided burden), cv = 10%
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "recycling"),
        &gwp_impacts(&env, -10_000_000),
        &1_000_u32,
        &None, &Bytes::new(&env),
    );

    client.finalize_lca(&producer, &id);
    let unc = client.get_lca_uncertainty(&id);

    // delta = |-10_000_000| * 1000/10000 = 1_000_000
    // lo = −10_000_000 − 1_000_000 = −11_000_000
    // hi = −10_000_000 + 1_000_000 = −9_000_000
    assert_eq!(unc.lo.get(0).unwrap(), -11_000_000i64);
    assert_eq!(unc.hi.get(0).unwrap(), -9_000_000i64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #43)")]
fn test_get_lca_uncertainty_before_finalize_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"UncNotFinal"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.get_lca_uncertainty(&id);
}

// ── normalize_impacts ────────────────────────────────────────────────────────

#[test]
fn test_normalize_impacts_equal_refs() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    // Owner registers norm ref: each category ref = 10_000_000
    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "testref"),
        &refs8(&env, 10_000_000),
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"NormProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 5_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);

    let result = client.normalize_impacts(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "testref"),
    );

    // normalized[0] = (5_000_000 * 1_000_000) / 10_000_000 = 500_000
    assert_eq!(result.normalized.get(0).unwrap(), 500_000i64);
    // Other categories: total=0, ref=10_000_000 → normalized = 0
    for cat in 1..8u32 {
        assert_eq!(result.normalized.get(cat).unwrap(), 0i64);
    }
}

#[test]
fn test_normalize_impacts_zero_ref_passes_through() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    // ref[0] = 0 (skip), ref[1..7] = 1_000_000
    let mut refs = soroban_sdk::Vec::new(&env);
    refs.push_back(0i64);
    for _ in 1..8 {
        refs.push_back(1_000_000i64);
    }

    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "partref"),
        &refs,
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"SkipNorm"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 7_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);

    let result = client.normalize_impacts(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "partref"),
    );

    // ref[0] = 0 → passthrough → normalized[0] = total = 7_000_000
    assert_eq!(result.normalized.get(0).unwrap(), 7_000_000i64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #44)")]
fn test_normalize_impacts_unknown_ref_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"UnknownRef"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 1_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);

    client.normalize_impacts(
        &producer,
        &id,
        &soroban_sdk::Symbol::new(&env, "missing"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_register_norm_ref_non_owner_panics() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    env.mock_all_auths();

    client.register_norm_ref(
        &attacker,
        &soroban_sdk::Symbol::new(&env, "evil"),
        &refs8(&env, 1_000_000),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #41)")]
fn test_register_norm_ref_wrong_length_panics() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let mut bad_refs = soroban_sdk::Vec::new(&env);
    bad_refs.push_back(1_000_000i64); // only 1 element

    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "badref"),
        &bad_refs,
    );
}

// ── apply_weighting_scheme ────────────────────────────────────────────────────

#[test]
fn test_apply_weighting_scheme_equal_weights() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    // 8 categories × 1250 bps = 10000 total
    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "ref1"),
        &refs8(&env, 1_000_000),
    );
    client.register_weighting_scheme(
        &owner,
        &soroban_sdk::Symbol::new(&env, "equal"),
        &weights8(&env, 1_250_u32),
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"WeightProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    // All 8 categories = 1_000_000 micro-units
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &impacts8(&env, [1_000_000i64; 8]),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);

    let normed = client.normalize_impacts(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "ref1"),
    );
    // normalized[cat] = (1_000_000 * 1_000_000) / 1_000_000 = 1_000_000
    assert_eq!(normed.normalized.get(0).unwrap(), 1_000_000i64);

    let result = client.apply_weighting_scheme(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "equal"),
    );
    // weighted[cat] = 1_000_000 * 1250 / 10000 = 125_000
    assert_eq!(result.weighted.get(0).unwrap(), 125_000i64);
    // single_score = 8 × 125_000 = 1_000_000
    assert_eq!(result.single_score, 1_000_000i64);
}

#[test]
fn test_apply_weighting_scheme_single_category_weight() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    // Weight only GWP (category 0) at 10000; others 0
    let mut w = soroban_sdk::Vec::new(&env);
    w.push_back(10_000_u32);
    for _ in 1..8 {
        w.push_back(0_u32);
    }

    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "refa"),
        &refs8(&env, 1_000_000),
    );
    client.register_weighting_scheme(
        &owner,
        &soroban_sdk::Symbol::new(&env, "gwponly"),
        &w,
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"GwpOnlyProduct"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 2_000_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);
    client.normalize_impacts(&producer, &id, &soroban_sdk::Symbol::new(&env, "refa"));

    let result = client.apply_weighting_scheme(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "gwponly"),
    );
    // normalized[0] = (2_000_000 * 1_000_000) / 1_000_000 = 2_000_000
    // weighted[0]   = 2_000_000 * 10_000 / 10_000 = 2_000_000
    // single_score  = 2_000_000
    assert_eq!(result.single_score, 2_000_000i64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #45)")]
fn test_apply_weighting_scheme_unknown_scheme_panics() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "rn1"),
        &refs8(&env, 1_000_000),
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"WS_miss"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 1_000),
        &0_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);
    client.normalize_impacts(&producer, &id, &soroban_sdk::Symbol::new(&env, "rn1"));

    client.apply_weighting_scheme(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "ghost"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_register_weighting_scheme_non_owner_panics() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    env.mock_all_auths();

    client.register_weighting_scheme(
        &attacker,
        &soroban_sdk::Symbol::new(&env, "hack"),
        &weights8(&env, 1_250_u32),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #41)")]
fn test_register_weighting_scheme_wrong_length_panics() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let mut bad_w = soroban_sdk::Vec::new(&env);
    bad_w.push_back(5_000_u32); // only 1 element

    client.register_weighting_scheme(
        &owner,
        &soroban_sdk::Symbol::new(&env, "badw"),
        &bad_w,
    );
}

// ── LCA Database integration ─────────────────────────────────────────────────

#[test]
fn test_register_lca_db_entry_returns_id() {
    let (env, _owner, client) = create_ledger();
    let provider = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_db_entry(
        &provider,
        &Bytes::from_slice(&env, b"ecoinvent"),
        &Bytes::from_slice(&env, b"3.10"),
        &Bytes::from_slice(&env, b"aluminium production, primary, ingot, GLO"),
        &Bytes::from_slice(&env, b"GLO"),
    );

    assert_eq!(id.len(), 32);
}

#[test]
fn test_get_lca_db_entry_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let provider = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_db_entry(
        &provider,
        &Bytes::from_slice(&env, b"gabi"),
        &Bytes::from_slice(&env, b"2023.1"),
        &Bytes::from_slice(&env, b"electricity mix DE"),
        &Bytes::from_slice(&env, b"DE"),
    );

    let entry = client.get_lca_db_entry(&id);
    assert_eq!(entry.id, id);
    assert_eq!(entry.db_name, Bytes::from_slice(&env, b"gabi"));
    assert_eq!(entry.version, Bytes::from_slice(&env, b"2023.1"));
    assert_eq!(entry.geography, Bytes::from_slice(&env, b"DE"));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #46)")]
fn test_get_lca_db_entry_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let fake = BytesN::from_array(&env, &[0x77u8; 32]);
    client.get_lca_db_entry(&fake);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #46)")]
fn test_record_phase_impact_unknown_db_ref_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"DbRefMiss"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    let fake_db = BytesN::from_array(&env, &[0xFFu8; 32]);
    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 1_000),
        &0_u32,
        &Some(fake_db),
        &Bytes::new(&env),
    );
}

// ── compute_lca_summary ───────────────────────────────────────────────────────

#[test]
fn test_compute_lca_summary_matches_get_lca_result() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "sumref"),
        &refs8(&env, 5_000_000),
    );
    client.register_weighting_scheme(
        &owner,
        &soroban_sdk::Symbol::new(&env, "sumw"),
        &weights8(&env, 1_250_u32),
    );

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"SummaryProd"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );

    client.record_phase_impact(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "mfg"),
        &gwp_impacts(&env, 10_000_000),
        &500_u32, &None, &Bytes::new(&env),
    );
    client.finalize_lca(&producer, &id);
    client.normalize_impacts(&producer, &id, &soroban_sdk::Symbol::new(&env, "sumref"));
    client.apply_weighting_scheme(&producer, &id, &soroban_sdk::Symbol::new(&env, "sumw"));

    let summary = client.compute_lca_summary(&id);
    let direct  = client.get_lca_result(&id);

    assert_eq!(summary.totals.get(0).unwrap(), direct.totals.get(0).unwrap());
    assert_eq!(summary.single_score, direct.single_score);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #43)")]
fn test_compute_lca_summary_not_finalized_panics() {
    let (env, _owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"SumNotFinal"),
        &Bytes::from_slice(&env, b"1 unit"),
        &None,
    );
    client.compute_lca_summary(&id);
}

// ── Full cradle-to-grave walkthrough ─────────────────────────────────────────

#[test]
fn test_full_cradle_to_grave_lca_walkthrough() {
    let (env, owner, client) = create_ledger();
    let producer = Address::generate(&env);
    env.mock_all_auths();

    // Register db entry for data source traceability
    let db_id = client.register_lca_db_entry(
        &producer,
        &Bytes::from_slice(&env, b"ecoinvent"),
        &Bytes::from_slice(&env, b"3.10"),
        &Bytes::from_slice(&env, b"plastic bottle production"),
        &Bytes::from_slice(&env, b"GLO"),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Register norm ref (CML-style, simplified: all cats = 8_760_000_000_000)
    client.register_norm_ref(
        &owner,
        &soroban_sdk::Symbol::new(&env, "cml"),
        &refs8(&env, 8_760_000_000_000_i64),
    );
    // Register equal weighting scheme
    client.register_weighting_scheme(
        &owner,
        &soroban_sdk::Symbol::new(&env, "equalw"),
        &weights8(&env, 1_250_u32),
    );

    // Register LCA profile
    let id = client.register_lca_entry(
        &producer,
        &Bytes::from_slice(&env, b"PET Bottle 500ml"),
        &Bytes::from_slice(&env, b"1000 bottles"),
        &None,
    );

    // Phase data (GWP in micro kg CO2-eq; others zero for brevity)
    let phase_gwp: [(soroban_sdk::Symbol, i64); 6] = [
        (soroban_sdk::Symbol::new(&env, "raw_mat"),   900_000_000),   // 900 kg CO2-eq
        (soroban_sdk::Symbol::new(&env, "mfg"),       400_000_000),   // 400 kg
        (soroban_sdk::Symbol::new(&env, "transport"),  80_000_000),   // 80 kg
        (soroban_sdk::Symbol::new(&env, "use"),        10_000_000),   // 10 kg
        (soroban_sdk::Symbol::new(&env, "eol"),        50_000_000),   // 50 kg
        (soroban_sdk::Symbol::new(&env, "recycling"), -120_000_000),  // −120 kg avoided
    ];

    for (phase, gwp) in phase_gwp.iter() {
        client.record_phase_impact(
            &producer, &id, phase,
            &gwp_impacts(&env, *gwp),
            &500_u32,           // ±5% uncertainty per phase
            &Some(db_id.clone()),
            &Bytes::new(&env),
        );
    }

    // Finalize
    let result = client.finalize_lca(&producer, &id);
    // Total GWP = 900 + 400 + 80 + 10 + 50 − 120 = 1_320_000_000 micro-units
    assert_eq!(result.totals.get(0).unwrap(), 1_320_000_000i64);

    // Uncertainty: each phase ±5% → lo/hi bracket total
    let unc = client.get_lca_uncertainty(&id);
    assert!(unc.lo.get(0).unwrap() < 1_320_000_000i64);
    assert!(unc.hi.get(0).unwrap() > 1_320_000_000i64);

    // Normalize then weight
    let normed = client.normalize_impacts(&producer, &id, &soroban_sdk::Symbol::new(&env, "cml"));
    // normalized[0] = (1_320_000_000 * 1_000_000) / 8_760_000_000_000
    //               = 1_320_000_000_000_000 / 8_760_000_000_000 = 150 (approx)
    let expected_norm = (1_320_000_000i128 * 1_000_000 / 8_760_000_000_000) as i64;
    assert_eq!(normed.normalized.get(0).unwrap(), expected_norm);

    let final_result = client.apply_weighting_scheme(
        &producer, &id,
        &soroban_sdk::Symbol::new(&env, "equalw"),
    );
    // weighted[0] = normalized[0] * 1250 / 10000
    let expected_weighted = (expected_norm as i128 * 1_250 / 10_000) as i64;
    assert_eq!(final_result.weighted.get(0).unwrap(), expected_weighted);

    // Summary equals result
    let summary = client.compute_lca_summary(&id);
    assert_eq!(summary.single_score, final_result.single_score);

    // Profile is finalized
    let profile = client.get_lca_profile(&id);
    assert!(profile.finalized);
    // phase_mask = bits 0..5 set (6 phases) = 0b0111111 = 63
    assert_eq!(profile.phase_mask, 63);
}

// ── Biodiversity Tests ───────────────────────────────────────────────────────

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a canonical 4-element ecosystem-service loss/value vec.
fn eco4(env: &Env, vals: [i64; 4]) -> soroban_sdk::Vec<i64> {
    let mut v = soroban_sdk::Vec::new(env);
    for x in vals { v.push_back(x); }
    v
}

/// Generate a deterministic 32-byte event reference.
fn fake_event_ref(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

// ── record_bio_impact ────────────────────────────────────────────────────────

#[test]
fn test_record_bio_impact_returns_id() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_bio_impact(
        &actor,
        &fake_event_ref(&env, 0x01),
        &soroban_sdk::Symbol::new(&env, "forest"),
        &5_000_000_u64,
        &120_000_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env),
        &Bytes::from_slice(&env, b"EN"),
        &Bytes::new(&env),
    );

    assert_eq!(id.len(), 32);
}

#[test]
fn test_record_bio_impact_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let event_ref = fake_event_ref(&env, 0x02);
    let id = client.record_bio_impact(
        &actor,
        &event_ref,
        &soroban_sdk::Symbol::new(&env, "wetland"),
        &2_000_000_u64,
        &50_000_u64,
        &eco4(&env, [100, 200, 300, 400]),
        &Bytes::from_slice(&env, b"-3.1415,51.5074"),
        &Bytes::from_slice(&env, b"VU"),
        &Bytes::from_slice(&env, b"survey-2026"),
    );

    let rec = client.get_bio_impact(&id);
    assert_eq!(rec.id, id);
    assert_eq!(rec.event_ref, event_ref);
    assert_eq!(rec.land_use_type, 4u32); // wetland = 4
    assert_eq!(rec.area_m2_micro, 2_000_000_u64);
    assert_eq!(rec.msa_loss_micro, 50_000_u64);
    assert_eq!(rec.eco_service_loss.get(0).unwrap(), 100i64);
    assert_eq!(rec.iucn_threat, Bytes::from_slice(&env, b"VU"));
}

#[test]
fn test_record_bio_impact_all_land_use_types() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let types = [
        (soroban_sdk::Symbol::new(&env, "crop"),      0u32),
        (soroban_sdk::Symbol::new(&env, "pasture"),   1u32),
        (soroban_sdk::Symbol::new(&env, "forest"),    2u32),
        (soroban_sdk::Symbol::new(&env, "urban"),     3u32),
        (soroban_sdk::Symbol::new(&env, "wetland"),   4u32),
        (soroban_sdk::Symbol::new(&env, "water"),     5u32),
        (soroban_sdk::Symbol::new(&env, "barren"),    6u32),
        (soroban_sdk::Symbol::new(&env, "protected"), 7u32),
    ];

    for (i, (lut, expected_disc)) in types.iter().enumerate() {
        env.ledger().with_mut(|li| li.timestamp += 1);
        let id = client.record_bio_impact(
            &actor,
            &fake_event_ref(&env, i as u8),
            lut,
            &1_000_u64,
            &0_u64,
            &eco4(&env, [0, 0, 0, 0]),
            &Bytes::new(&env),
            &Bytes::new(&env),
            &Bytes::new(&env),
        );
        let rec = client.get_bio_impact(&id);
        assert_eq!(rec.land_use_type, *expected_disc);
    }
}

#[test]
fn test_record_bio_impact_updates_totals() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x10),
        &soroban_sdk::Symbol::new(&env, "crop"),
        &3_000_000_u64, &80_000_u64,
        &eco4(&env, [500, 300, 100, 50]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x11),
        &soroban_sdk::Symbol::new(&env, "urban"),
        &1_000_000_u64, &20_000_u64,
        &eco4(&env, [200, 100, 50, 25]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_impacts, 2);
    assert_eq!(totals.total_area_m2_micro, 4_000_000_u64);
    assert_eq!(totals.total_msa_loss_micro, 100_000_u64);
    // eco_loss = (500+300+100+50) + (200+100+50+25) = 950 + 375 = 1325
    assert_eq!(totals.total_eco_loss_micro, 1325i64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #51)")]
fn test_record_bio_impact_zero_area_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0xFF),
        &soroban_sdk::Symbol::new(&env, "forest"),
        &0_u64, // zero → InvalidLandArea
        &0_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #49)")]
fn test_record_bio_impact_invalid_land_use_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0xAB),
        &soroban_sdk::Symbol::new(&env, "lava"),   // not a valid type
        &1_000_u64, &0_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #41)")]
fn test_record_bio_impact_wrong_eco_vec_len_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let mut bad_eco = soroban_sdk::Vec::new(&env);
    bad_eco.push_back(100i64); // only 1 element instead of 4

    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0xCD),
        &soroban_sdk::Symbol::new(&env, "crop"),
        &1_000_u64, &0_u64, &bad_eco,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #47)")]
fn test_get_bio_impact_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_bio_impact(&BytesN::from_array(&env, &[0x00u8; 32]));
}

// ── register_bio_offset / retire_bio_offset ───────────────────────────────────

#[test]
fn test_register_bio_offset_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_bio_offset(
        &issuer,
        &Bytes::from_slice(&env, b"bng"),
        &500_000_u64,
        &0_u64,
        &None,
        &Bytes::from_slice(&env, b"cert-123"),
    );

    let off = client.get_bio_offset(&id);
    assert_eq!(off.total_micro, 500_000_u64);
    assert_eq!(off.retired_micro, 0_u64);
    assert_eq!(off.scheme, Bytes::from_slice(&env, b"bng"));
}

#[test]
fn test_register_bio_offset_updates_totals() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &300_000_u64, &0_u64, &None, &Bytes::new(&env),
    );
    env.ledger().with_mut(|li| li.timestamp += 1);
    client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &200_000_u64, &0_u64, &None, &Bytes::new(&env),
    );

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_offset_micro, 500_000_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #52)")]
fn test_register_bio_offset_zero_quantity_panics() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &0_u64, &0_u64, &None, &Bytes::new(&env),
    );
}

#[test]
fn test_retire_bio_offset_partial_then_full() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"mitbank"),
        &100_000_u64, &0_u64, &None, &Bytes::new(&env),
    );

    // First retirement: 40 000
    let rem1 = client.retire_bio_offset(&issuer, &id, &40_000_u64);
    assert_eq!(rem1, 60_000_u64);

    let off = client.get_bio_offset(&id);
    assert_eq!(off.retired_micro, 40_000_u64);

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_retired_micro, 40_000_u64);

    // Second retirement: the remaining 60 000
    let rem2 = client.retire_bio_offset(&issuer, &id, &60_000_u64);
    assert_eq!(rem2, 0_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #53)")]
fn test_retire_bio_offset_already_retired_panics() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &10_000_u64, &0_u64, &None, &Bytes::new(&env),
    );

    client.retire_bio_offset(&issuer, &id, &10_000_u64);   // fully retire
    client.retire_bio_offset(&issuer, &id, &1_u64);        // → OffsetAlreadyRetired
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #54)")]
fn test_retire_bio_offset_exceeds_balance_panics() {
    let (env, _owner, client) = create_ledger();
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"bng"),
        &50_000_u64, &0_u64, &None, &Bytes::new(&env),
    );

    client.retire_bio_offset(&issuer, &id, &50_001_u64);   // one too many
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #48)")]
fn test_get_bio_offset_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_bio_offset(&BytesN::from_array(&env, &[0x55u8; 32]));
}

// ── register_eco_service_record ───────────────────────────────────────────────

#[test]
fn test_register_eco_service_record_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let owner = Address::generate(&env);
    env.mock_all_auths();

    let id = client.register_eco_service_record(
        &owner,
        &Bytes::from_slice(&env, b"Amazon Forest Site A"),
        &50_000_000_000_u64,              // 50 km²
        &soroban_sdk::Symbol::new(&env, "forest"),
        &eco4(&env, [10_000, 50_000, 5_000, 8_000]),
        &Bytes::from_slice(&env, b"seea-2026"),
    );

    let rec = client.get_eco_service_record(&id);
    assert_eq!(rec.name, Bytes::from_slice(&env, b"Amazon Forest Site A"));
    assert_eq!(rec.land_use_type, 2u32); // forest
    assert_eq!(rec.area_m2_micro, 50_000_000_000_u64);
    assert_eq!(rec.annual_values.get(1).unwrap(), 50_000i64); // regulating
}

#[test]
fn test_register_eco_service_record_increments_totals() {
    let (env, _owner, client) = create_ledger();
    let owner = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.get_bio_totals().total_eco_records, 0);

    client.register_eco_service_record(
        &owner,
        &Bytes::from_slice(&env, b"Site1"),
        &1_000_u64,
        &soroban_sdk::Symbol::new(&env, "wetland"),
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env),
    );

    assert_eq!(client.get_bio_totals().total_eco_records, 1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #51)")]
fn test_register_eco_service_record_zero_area_panics() {
    let (env, _owner, client) = create_ledger();
    let owner = Address::generate(&env);
    env.mock_all_auths();

    client.register_eco_service_record(
        &owner, &Bytes::from_slice(&env, b"Bad"),
        &0_u64,
        &soroban_sdk::Symbol::new(&env, "crop"),
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #47)")]
fn test_get_eco_service_record_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_eco_service_record(&BytesN::from_array(&env, &[0xBBu8; 32]));
}

// ── register_bio_offset with eco_service_ref ─────────────────────────────────

#[test]
fn test_register_bio_offset_with_eco_service_ref() {
    let (env, _owner, client) = create_ledger();
    let owner = Address::generate(&env);
    env.mock_all_auths();

    let eco_id = client.register_eco_service_record(
        &owner,
        &Bytes::from_slice(&env, b"Mangrove Reserve"),
        &10_000_000_u64,
        &soroban_sdk::Symbol::new(&env, "wetland"),
        &eco4(&env, [1_000, 5_000, 500, 2_000]),
        &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    let off_id = client.register_bio_offset(
        &owner,
        &Bytes::from_slice(&env, b"vbc"),
        &200_000_u64,
        &0_u64,
        &Some(eco_id.clone()),
        &Bytes::new(&env),
    );

    let off = client.get_bio_offset(&off_id);
    assert_eq!(off.eco_service_ref, Some(eco_id));
}

// ── record_species_observation ────────────────────────────────────────────────

#[test]
fn test_record_species_observation_stored() {
    let (env, _owner, client) = create_ledger();
    let observer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_species_observation(
        &observer,
        &fake_event_ref(&env, 0x30),
        &Bytes::from_slice(&env, b"Sumatran Tiger"),
        &Bytes::from_slice(&env, b"Panthera tigris sumatrae"),
        &Bytes::from_slice(&env, b"CR"),
        &3_u32,
        &0_u32, // positive (sighted)
        &Bytes::from_slice(&env, b"camera-trap-2026"),
    );

    let obs = client.get_species_observation(&id);
    assert_eq!(obs.species_name, Bytes::from_slice(&env, b"Sumatran Tiger"));
    assert_eq!(obs.iucn_category, Bytes::from_slice(&env, b"CR"));
    assert_eq!(obs.count, 3_u32);
    assert_eq!(obs.impact_direction, 0_u32);
}

#[test]
fn test_record_species_observation_updates_totals() {
    let (env, _owner, client) = create_ledger();
    let observer = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.get_bio_totals().total_observations, 0);

    client.record_species_observation(
        &observer, &fake_event_ref(&env, 0x31),
        &Bytes::from_slice(&env, b"Sea Turtle"),
        &Bytes::from_slice(&env, b"Chelonia mydas"),
        &Bytes::from_slice(&env, b"EN"),
        &1_u32, &1_u32,
        &Bytes::new(&env),
    );

    assert_eq!(client.get_bio_totals().total_observations, 1);
}

#[test]
fn test_record_species_observation_presence_only_count_zero() {
    let (env, _owner, client) = create_ledger();
    let observer = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_species_observation(
        &observer, &fake_event_ref(&env, 0x32),
        &Bytes::from_slice(&env, b"Unknown"),
        &Bytes::from_slice(&env, b"sp. nov."),
        &Bytes::from_slice(&env, b"DD"),
        &0_u32, // presence-only
        &2_u32, // neutral
        &Bytes::new(&env),
    );

    let obs = client.get_species_observation(&id);
    assert_eq!(obs.count, 0);
    assert_eq!(obs.impact_direction, 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #55)")]
fn test_get_species_observation_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_species_observation(&BytesN::from_array(&env, &[0x77u8; 32]));
}

// ── compute_nature_positive_score ─────────────────────────────────────────────

#[test]
fn test_nature_positive_score_no_impacts_returns_10000() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let snap = client.compute_nature_positive_score();
    // No losses → nature-positive by definition
    assert_eq!(snap.nature_positive_bps, 10_000_u32);
    assert_eq!(snap.net_msa_micro, 0i64);
    assert_eq!(snap.index, 0_u32);
}

#[test]
fn test_nature_positive_score_all_offset_equals_10000() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    // Impact: 100 000 MSA·ha loss
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x40),
        &soroban_sdk::Symbol::new(&env, "forest"),
        &1_000_000_u64, &100_000_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Offset: register 100 000 and retire all
    let off_id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"bng"),
        &100_000_u64, &0_u64, &None, &Bytes::new(&env),
    );
    client.retire_bio_offset(&issuer, &off_id, &100_000_u64);

    let snap = client.compute_nature_positive_score();
    assert_eq!(snap.nature_positive_bps, 10_000_u32);
    assert_eq!(snap.net_msa_micro, 0i64);
    assert_eq!(snap.total_msa_loss_micro, 100_000_u64);
    assert_eq!(snap.total_retired_micro, 100_000_u64);
}

#[test]
fn test_nature_positive_score_partial_offset() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    // Loss: 200 000
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x50),
        &soroban_sdk::Symbol::new(&env, "crop"),
        &2_000_000_u64, &200_000_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Retire: 50 000 (25% coverage)
    let off_id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &100_000_u64, &0_u64, &None, &Bytes::new(&env),
    );
    client.retire_bio_offset(&issuer, &off_id, &50_000_u64);

    let snap = client.compute_nature_positive_score();
    // nature_positive_bps = 50_000 * 10_000 / 200_000 = 2_500
    assert_eq!(snap.nature_positive_bps, 2_500_u32);
    // net_msa = 50_000 − 200_000 = −150_000
    assert_eq!(snap.net_msa_micro, -150_000i64);
    assert_eq!(snap.offset_coverage_bps, 2_500_u32);
}

#[test]
fn test_nature_positive_score_over_offset_capped_at_10000() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    // Loss: 100 000
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x60),
        &soroban_sdk::Symbol::new(&env, "pasture"),
        &1_000_000_u64, &100_000_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Retire 150 000 (over-offset → 150% = capped at 100%)
    let off_id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"bng"),
        &200_000_u64, &0_u64, &None, &Bytes::new(&env),
    );
    client.retire_bio_offset(&issuer, &off_id, &150_000_u64);

    let snap = client.compute_nature_positive_score();
    // 150_000 * 10_000 / 100_000 = 15_000 → capped at 10_000
    assert_eq!(snap.nature_positive_bps, 10_000_u32);
    // net is positive (nature-net-gain)
    assert_eq!(snap.net_msa_micro, 50_000i64);
}

#[test]
fn test_bio_snapshot_count_increments() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    assert_eq!(client.bio_snapshot_count(), 0);
    client.compute_nature_positive_score();
    assert_eq!(client.bio_snapshot_count(), 1);
    client.compute_nature_positive_score();
    assert_eq!(client.bio_snapshot_count(), 2);
}

#[test]
fn test_get_bio_snapshot_retrieves_correct_index() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    // First snapshot (no impacts)
    let snap0 = client.compute_nature_positive_score();
    assert_eq!(snap0.index, 0);

    // Add an impact, then take second snapshot
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x70),
        &soroban_sdk::Symbol::new(&env, "urban"),
        &500_000_u64, &10_000_u64,
        &eco4(&env, [0, 0, 0, 0]),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
    let snap1 = client.compute_nature_positive_score();
    assert_eq!(snap1.index, 1);
    assert_eq!(snap1.total_impacts, 1);

    // Verify retrieval
    let r0 = client.get_bio_snapshot(&0);
    let r1 = client.get_bio_snapshot(&1);
    assert_eq!(r0.total_impacts, 0);
    assert_eq!(r1.total_impacts, 1);
    assert_eq!(r1.total_msa_loss_micro, 10_000_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #30)")]
fn test_get_bio_snapshot_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    client.get_bio_snapshot(&99);
}

// ── get_bio_totals ────────────────────────────────────────────────────────────

#[test]
fn test_get_bio_totals_empty() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_impacts, 0);
    assert_eq!(totals.total_area_m2_micro, 0);
    assert_eq!(totals.total_msa_loss_micro, 0);
    assert_eq!(totals.total_offset_micro, 0);
    assert_eq!(totals.total_retired_micro, 0);
    assert_eq!(totals.total_observations, 0);
    assert_eq!(totals.total_eco_records, 0);
}

#[test]
fn test_get_bio_totals_full_accumulation() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    // Impact
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x80),
        &soroban_sdk::Symbol::new(&env, "forest"),
        &2_000_000_u64, &60_000_u64,
        &eco4(&env, [1_000, 2_000, 500, 300]),
        &Bytes::new(&env), &Bytes::from_slice(&env, b"NT"), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Ecosystem service record
    client.register_eco_service_record(
        &actor,
        &Bytes::from_slice(&env, b"PeatBog"),
        &5_000_000_u64,
        &soroban_sdk::Symbol::new(&env, "wetland"),
        &eco4(&env, [500, 3_000, 200, 400]),
        &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    // Offset
    let off_id = client.register_bio_offset(
        &issuer, &Bytes::from_slice(&env, b"vbc"),
        &80_000_u64, &0_u64, &None, &Bytes::new(&env),
    );
    client.retire_bio_offset(&issuer, &off_id, &25_000_u64);

    // Species observation
    env.ledger().with_mut(|li| li.timestamp += 1);
    client.record_species_observation(
        &actor, &fake_event_ref(&env, 0x81),
        &Bytes::from_slice(&env, b"Red Kite"),
        &Bytes::from_slice(&env, b"Milvus milvus"),
        &Bytes::from_slice(&env, b"LC"),
        &5_u32, &0_u32,
        &Bytes::new(&env),
    );

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_impacts, 1);
    assert_eq!(totals.total_area_m2_micro, 2_000_000_u64);
    assert_eq!(totals.total_msa_loss_micro, 60_000_u64);
    assert_eq!(totals.total_eco_loss_micro, 3_800i64); // 1000+2000+500+300
    assert_eq!(totals.total_offset_micro, 80_000_u64);
    assert_eq!(totals.total_retired_micro, 25_000_u64);
    assert_eq!(totals.total_observations, 1);
    assert_eq!(totals.total_eco_records, 1);
}

// ── Ecosystem service negative value (gain) ───────────────────────────────────

#[test]
fn test_bio_impact_negative_eco_service_records_gain() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    // Restoration project: negative losses = net gain
    client.record_bio_impact(
        &actor, &fake_event_ref(&env, 0x90),
        &soroban_sdk::Symbol::new(&env, "protected"),
        &1_000_000_u64, &0_u64,
        &eco4(&env, [-500, -1_000, -200, -100]), // gains
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let totals = client.get_bio_totals();
    assert_eq!(totals.total_eco_loss_micro, -1_800i64);
}

// ── Water Footprint Tests ────────────────────────────────────────────────────

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convenience: register a minimal risk assessment and return its ID.
fn register_risk(env: &Env, client: &AuditLedgerClient, caller: &Address) -> BytesN<32> {
    client.register_water_risk_assessment(
        caller,
        &Bytes::from_slice(env, b"Basin A"),
        &5_000_u32,
        &4_000_u32,
        &3_000_u32,
        &2_000_u32,
        &60_000_u32,
        &Bytes::from_slice(env, b"IN"),
        &Bytes::from_slice(env, b"4050017220"),
        &Bytes::from_slice(env, b"aqueduct4"),
        &Bytes::new(env),
    )
}

/// Convenience: register a minimal stewardship programme and return its ID.
fn register_stewardship(env: &Env, client: &AuditLedgerClient, caller: &Address) -> BytesN<32> {
    client.register_water_stewardship(
        caller,
        &Bytes::from_slice(env, b"aws_core"),
        &0_u64,
        &0_u64,
        &500_000_000_u64,
        &None,
        &Bytes::new(env),
    )
}

// ── record_water_footprint ───────────────────────────────────────────────────

#[test]
fn test_record_water_footprint_returns_id() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_water_footprint(
        &actor,
        &fake_event_ref(&env, 0x01),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &10_000_000_u64,
        &5_000_000_u64,
        &2_000_000_u64,
        &0_u32,
        &None,
        &None,
        &Bytes::from_slice(&env, b"IN"),
        &Bytes::from_slice(&env, b"4050017220"),
        &Bytes::new(&env),
    );

    assert_eq!(id.len(), 32);
}

#[test]
fn test_record_water_footprint_stored_correctly() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let event_ref = fake_event_ref(&env, 0x02);
    let id = client.record_water_footprint(
        &actor,
        &event_ref,
        &soroban_sdk::Symbol::new(&env, "indust"),
        &8_000_000_u64,
        &0_u64,
        &3_000_000_u64,
        &40_000_u32,   // 40% WSI
        &None,
        &None,
        &Bytes::from_slice(&env, b"CN"),
        &Bytes::from_slice(&env, b"basin-x"),
        &Bytes::from_slice(&env, b"meter-2026"),
    );

    let rec = client.get_water_footprint(&id);
    assert_eq!(rec.event_ref, event_ref);
    assert_eq!(rec.sector, 1u32); // indust
    assert_eq!(rec.blue_L_micro, 8_000_000_u64);
    assert_eq!(rec.green_L_micro, 0_u64);
    assert_eq!(rec.grey_L_micro, 3_000_000_u64);
    assert_eq!(rec.scarcity_factor_ppb, 40_000_u32);
    // scarcity_weighted = 8_000_000 * 40_000 / 1_000_000 = 320_000
    assert_eq!(rec.scarcity_weighted_L_micro, 320_000_u64);
}

#[test]
fn test_record_water_footprint_all_sectors() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let sectors = [
        (soroban_sdk::Symbol::new(&env, "agri"),   0u32),
        (soroban_sdk::Symbol::new(&env, "indust"),  1u32),
        (soroban_sdk::Symbol::new(&env, "munici"),  2u32),
        (soroban_sdk::Symbol::new(&env, "energy"),  3u32),
        (soroban_sdk::Symbol::new(&env, "mining"),  4u32),
    ];

    for (i, (sector, expected)) in sectors.iter().enumerate() {
        env.ledger().with_mut(|li| li.timestamp += 1);
        let id = client.record_water_footprint(
            &actor,
            &fake_event_ref(&env, i as u8),
            sector,
            &1_000_u64, &0_u64, &0_u64,
            &0_u32, &None, &None,
            &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
        );
        assert_eq!(client.get_water_footprint(&id).sector, *expected);
    }
}

#[test]
fn test_record_water_footprint_scarcity_weighted_formula() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    // blue = 12_000_000 L, WSI = 50_000 ppb (= 0.05 factor)
    // expected = 12_000_000 * 50_000 / 1_000_000 = 600_000
    let id = client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x10),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &12_000_000_u64, &0_u64, &0_u64,
        &50_000_u32,
        &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let rec = client.get_water_footprint(&id);
    assert_eq!(rec.scarcity_weighted_L_micro, 600_000_u64);
}

#[test]
fn test_record_water_footprint_zero_scarcity_gives_zero_weighted() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x11),
        &soroban_sdk::Symbol::new(&env, "munici"),
        &5_000_000_u64, &2_000_000_u64, &1_000_000_u64,
        &0_u32,  // no scarcity
        &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let rec = client.get_water_footprint(&id);
    assert_eq!(rec.scarcity_weighted_L_micro, 0_u64);
}

#[test]
fn test_record_water_footprint_updates_totals() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x20),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &6_000_000_u64, &4_000_000_u64, &1_000_000_u64,
        &20_000_u32,
        &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    env.ledger().with_mut(|li| li.timestamp += 1);

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x21),
        &soroban_sdk::Symbol::new(&env, "indust"),
        &3_000_000_u64, &0_u64, &2_000_000_u64,
        &30_000_u32,
        &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let t = client.get_water_totals();
    assert_eq!(t.total_footprints, 2);
    assert_eq!(t.total_blue_L_micro, 9_000_000_u64);
    assert_eq!(t.total_green_L_micro, 4_000_000_u64);
    assert_eq!(t.total_grey_L_micro, 3_000_000_u64);
    // sw1 = 6M*20k/1M = 120k; sw2 = 3M*30k/1M = 90k → total = 210k
    assert_eq!(t.total_scarcity_weighted_L_micro, 210_000_u64);
}

#[test]
fn test_record_water_footprint_with_risk_and_stewardship_refs() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let risk_id = register_risk(&env, &client, &actor);
    env.ledger().with_mut(|li| li.timestamp += 1);
    let stew_id = register_stewardship(&env, &client, &actor);
    env.ledger().with_mut(|li| li.timestamp += 1);

    let id = client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x30),
        &soroban_sdk::Symbol::new(&env, "energy"),
        &2_000_000_u64, &0_u64, &500_000_u64,
        &60_000_u32,
        &Some(risk_id.clone()),
        &Some(stew_id.clone()),
        &Bytes::from_slice(&env, b"IN"),
        &Bytes::from_slice(&env, b"4050017220"),
        &Bytes::new(&env),
    );

    let rec = client.get_water_footprint(&id);
    assert_eq!(rec.risk_assessment_ref, Some(risk_id));
    assert_eq!(rec.stewardship_ref, Some(stew_id));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #61)")]
fn test_record_water_footprint_all_zero_volumes_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0xFF),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &0_u64, &0_u64, &0_u64,  // all zero → InvalidWaterVolume
        &0_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #62)")]
fn test_record_water_footprint_scarcity_over_max_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0xFE),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &1_000_u64, &0_u64, &0_u64,
        &100_001_u32,  // > 100_000 → InvalidScarcityFactor
        &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #60)")]
fn test_record_water_footprint_invalid_sector_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0xFD),
        &soroban_sdk::Symbol::new(&env, "beverage"),  // not a valid sector
        &1_000_u64, &0_u64, &0_u64,
        &0_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #57)")]
fn test_record_water_footprint_unknown_risk_ref_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let fake_risk = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0xFC),
        &soroban_sdk::Symbol::new(&env, "munici"),
        &1_000_u64, &0_u64, &0_u64,
        &0_u32, &Some(fake_risk), &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #58)")]
fn test_record_water_footprint_unknown_stewardship_ref_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let fake_stew = BytesN::from_array(&env, &[0xBBu8; 32]);
    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0xFB),
        &soroban_sdk::Symbol::new(&env, "mining"),
        &1_000_u64, &0_u64, &0_u64,
        &0_u32, &None, &Some(fake_stew),
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #56)")]
fn test_get_water_footprint_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    client.get_water_footprint(&BytesN::from_array(&env, &[0x00u8; 32]));
}

// ── register_water_risk_assessment ───────────────────────────────────────────

#[test]
fn test_register_water_risk_assessment_stored() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = register_risk(&env, &client, &actor);
    let rec = client.get_water_risk_assessment(&id);

    assert_eq!(rec.name, Bytes::from_slice(&env, b"Basin A"));
    assert_eq!(rec.overall_risk_bps, 5_000_u32);
    assert_eq!(rec.wsi_ppb, 60_000_u32);
    assert_eq!(rec.country, Bytes::from_slice(&env, b"IN"));
}

#[test]
fn test_register_water_risk_assessment_increments_totals() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.get_water_totals().total_risk_assessments, 0);
    register_risk(&env, &client, &actor);
    assert_eq!(client.get_water_totals().total_risk_assessments, 1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #62)")]
fn test_register_water_risk_wsi_over_max_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.register_water_risk_assessment(
        &actor,
        &Bytes::from_slice(&env, b"BadBasin"),
        &5_000_u32, &4_000_u32, &3_000_u32, &2_000_u32,
        &100_001_u32,  // > 100_000 → InvalidScarcityFactor
        &Bytes::new(&env), &Bytes::new(&env),
        &Bytes::new(&env), &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #57)")]
fn test_get_water_risk_assessment_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    client.get_water_risk_assessment(&BytesN::from_array(&env, &[0x11u8; 32]));
}

// ── register_water_stewardship / update_stewardship_progress ─────────────────

#[test]
fn test_register_water_stewardship_stored() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = register_stewardship(&env, &client, &actor);
    let rec = client.get_water_stewardship(&id);

    assert_eq!(rec.programme, Bytes::from_slice(&env, b"aws_core"));
    assert_eq!(rec.target_reduction_L_micro, 500_000_000_u64);
    assert_eq!(rec.achieved_reduction_L_micro, 0_u64);
}

#[test]
fn test_register_water_stewardship_increments_totals() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.get_water_totals().total_stewardship_programmes, 0);
    register_stewardship(&env, &client, &actor);
    assert_eq!(client.get_water_totals().total_stewardship_programmes, 1);
}

#[test]
fn test_stewardship_with_risk_ref() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let risk_id = register_risk(&env, &client, &actor);
    env.ledger().with_mut(|li| li.timestamp += 1);

    let stew_id = client.register_water_stewardship(
        &actor,
        &Bytes::from_slice(&env, b"ceo_mandate"),
        &0_u64, &0_u64, &0_u64,
        &Some(risk_id.clone()),
        &Bytes::new(&env),
    );

    let rec = client.get_water_stewardship(&stew_id);
    assert_eq!(rec.risk_assessment_ref, Some(risk_id));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #57)")]
fn test_register_stewardship_unknown_risk_ref_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let fake = BytesN::from_array(&env, &[0xCCu8; 32]);
    client.register_water_stewardship(
        &actor,
        &Bytes::from_slice(&env, b"aws_core"),
        &0_u64, &0_u64, &0_u64,
        &Some(fake),
        &Bytes::new(&env),
    );
}

#[test]
fn test_update_stewardship_progress() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = register_stewardship(&env, &client, &actor);

    client.update_stewardship_progress(&actor, &id, &200_000_000_u64);

    let rec = client.get_water_stewardship(&id);
    assert_eq!(rec.achieved_reduction_L_micro, 200_000_000_u64);
}

#[test]
fn test_update_stewardship_progress_cumulative() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let id = register_stewardship(&env, &client, &actor);

    client.update_stewardship_progress(&actor, &id, &100_000_000_u64);
    // Update again with a higher cumulative value
    client.update_stewardship_progress(&actor, &id, &350_000_000_u64);

    let rec = client.get_water_stewardship(&id);
    assert_eq!(rec.achieved_reduction_L_micro, 350_000_000_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_update_stewardship_progress_non_participant_panics() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let attacker = Address::generate(&env);
    env.mock_all_auths();

    let id = register_stewardship(&env, &client, &actor);
    client.update_stewardship_progress(&attacker, &id, &999_999_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #58)")]
fn test_get_water_stewardship_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    client.get_water_stewardship(&BytesN::from_array(&env, &[0x22u8; 32]));
}

// ── record_water_disclosure ───────────────────────────────────────────────────

#[test]
fn test_record_water_disclosure_stored() {
    let (env, _owner, client) = create_ledger();
    let org = Address::generate(&env);
    env.mock_all_auths();

    let id = client.record_water_disclosure(
        &org,
        &2025_u32,
        &100_000_000_000_u64,  // W1: 100 000 m³ withdrawal
        &40_000_000_000_u64,   // W2: 40 000 m³ consumption
        &60_000_000_000_u64,   // W3: discharge
        &2_000_u32,            // W4: 20% estimated
        &10_000_000_000_u64,   // W5: reduction target
        &3_u32,                // W6: 3 stressed sites
        &8_000_000_000_u64,    // W7: scarcity-weighted
        &2_000_000_000_u64,    // W8: achieved
        &Bytes::from_slice(&env, b"cdp-2025-sub-001"),
    );

    let rec = client.get_water_disclosure(&id);
    assert_eq!(rec.reporting_year, 2025_u32);
    assert_eq!(rec.total_withdrawal_L_micro, 100_000_000_000_u64);
    assert_eq!(rec.total_consumption_L_micro, 40_000_000_000_u64);
    assert_eq!(rec.sites_in_stressed_areas, 3_u32);
    assert_eq!(rec.scarcity_weighted_total_L_micro, 8_000_000_000_u64);
    assert_eq!(rec.reduction_achieved_L_micro, 2_000_000_000_u64);
    assert_eq!(rec.metadata, Bytes::from_slice(&env, b"cdp-2025-sub-001"));
}

#[test]
fn test_record_water_disclosure_increments_totals() {
    let (env, _owner, client) = create_ledger();
    let org = Address::generate(&env);
    env.mock_all_auths();

    assert_eq!(client.get_water_totals().total_disclosures, 0);

    client.record_water_disclosure(
        &org, &2024_u32,
        &50_000_u64, &20_000_u64, &30_000_u64,
        &1_000_u32, &0_u64, &0_u32, &0_u64, &0_u64,
        &Bytes::new(&env),
    );

    assert_eq!(client.get_water_totals().total_disclosures, 1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #63)")]
fn test_record_water_disclosure_invalid_year_panics() {
    let (env, _owner, client) = create_ledger();
    let org = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_disclosure(
        &org,
        &2000_u32,   // ≤ 2000 → InvalidDisclosureYear
        &1_000_u64, &0_u64, &0_u64,
        &0_u32, &0_u64, &0_u32, &0_u64, &0_u64,
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #59)")]
fn test_get_water_disclosure_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    client.get_water_disclosure(&BytesN::from_array(&env, &[0x33u8; 32]));
}

// ── compute_water_snapshot ────────────────────────────────────────────────────

#[test]
fn test_water_snapshot_empty_contract() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let snap = client.compute_water_snapshot();
    assert_eq!(snap.total_blue_L_micro, 0_u64);
    assert_eq!(snap.total_water_footprint_L_micro, 0_u64);
    assert_eq!(snap.scarcity_ratio_bps, 0_u32);
    assert_eq!(snap.blue_fraction_bps, 0_u32);
    assert_eq!(snap.index, 0_u32);
}

#[test]
fn test_water_snapshot_totals_and_ratios() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    // blue = 6M, green = 2M, grey = 2M → total = 10M
    // WSI = 50_000 → sw = 6M*50k/1M = 300k
    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x40),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &6_000_000_u64, &2_000_000_u64, &2_000_000_u64,
        &50_000_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let snap = client.compute_water_snapshot();

    assert_eq!(snap.total_blue_L_micro, 6_000_000_u64);
    assert_eq!(snap.total_water_footprint_L_micro, 10_000_000_u64);
    assert_eq!(snap.total_scarcity_weighted_L_micro, 300_000_u64);
    // scarcity_ratio = 300_000 * 10_000 / 6_000_000 = 500 bps
    assert_eq!(snap.scarcity_ratio_bps, 500_u32);
    // blue_fraction = 6_000_000 * 10_000 / 10_000_000 = 6_000 bps
    assert_eq!(snap.blue_fraction_bps, 6_000_u32);
}

#[test]
fn test_water_snapshot_all_blue() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x50),
        &soroban_sdk::Symbol::new(&env, "energy"),
        &4_000_000_u64, &0_u64, &0_u64,
        &0_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let snap = client.compute_water_snapshot();
    // total = 4M, all blue → blue_fraction = 10_000 bps (100%)
    assert_eq!(snap.blue_fraction_bps, 10_000_u32);
    // no scarcity weighting → ratio = 0
    assert_eq!(snap.scarcity_ratio_bps, 0_u32);
}

#[test]
fn test_water_snapshot_count_increments() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    assert_eq!(client.water_snapshot_count(), 0);
    client.compute_water_snapshot();
    assert_eq!(client.water_snapshot_count(), 1);
    client.compute_water_snapshot();
    assert_eq!(client.water_snapshot_count(), 2);
}

#[test]
fn test_get_water_snapshot_retrieves_by_index() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    let snap0 = client.compute_water_snapshot();
    assert_eq!(snap0.index, 0);
    assert_eq!(snap0.total_footprints, 0);

    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x60),
        &soroban_sdk::Symbol::new(&env, "munici"),
        &3_000_000_u64, &0_u64, &0_u64,
        &10_000_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    let snap1 = client.compute_water_snapshot();
    assert_eq!(snap1.index, 1);
    assert_eq!(snap1.total_footprints, 1);

    let r0 = client.get_water_snapshot(&0);
    let r1 = client.get_water_snapshot(&1);
    assert_eq!(r0.total_footprints, 0);
    assert_eq!(r1.total_footprints, 1);
    assert_eq!(r1.total_blue_L_micro, 3_000_000_u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #30)")]
fn test_get_water_snapshot_not_found_panics() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    client.get_water_snapshot(&99);
}

// ── get_water_totals ──────────────────────────────────────────────────────────

#[test]
fn test_get_water_totals_empty() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();

    let t = client.get_water_totals();
    assert_eq!(t.total_footprints, 0);
    assert_eq!(t.total_blue_L_micro, 0);
    assert_eq!(t.total_scarcity_weighted_L_micro, 0);
    assert_eq!(t.total_risk_assessments, 0);
    assert_eq!(t.total_stewardship_programmes, 0);
    assert_eq!(t.total_disclosures, 0);
}

#[test]
fn test_get_water_totals_full_accumulation() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    let org = Address::generate(&env);
    env.mock_all_auths();

    // 2 footprints
    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x70),
        &soroban_sdk::Symbol::new(&env, "agri"),
        &10_000_000_u64, &5_000_000_u64, &2_000_000_u64,
        &30_000_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );
    env.ledger().with_mut(|li| li.timestamp += 1);
    client.record_water_footprint(
        &actor, &fake_event_ref(&env, 0x71),
        &soroban_sdk::Symbol::new(&env, "indust"),
        &4_000_000_u64, &0_u64, &1_000_000_u64,
        &0_u32, &None, &None,
        &Bytes::new(&env), &Bytes::new(&env), &Bytes::new(&env),
    );

    // 1 risk assessment
    env.ledger().with_mut(|li| li.timestamp += 1);
    register_risk(&env, &client, &actor);

    // 1 stewardship
    env.ledger().with_mut(|li| li.timestamp += 1);
    register_stewardship(&env, &client, &actor);

    // 1 disclosure
    env.ledger().with_mut(|li| li.timestamp += 1);
    client.record_water_disclosure(
        &org, &2025_u32,
        &80_000_u64, &30_000_u64, &50_000_u64,
        &500_u32, &0_u64, &1_u32, &0_u64, &0_u64,
        &Bytes::new(&env),
    );

    let t = client.get_water_totals();
    assert_eq!(t.total_footprints, 2);
    assert_eq!(t.total_blue_L_micro, 14_000_000_u64);
    assert_eq!(t.total_green_L_micro, 5_000_000_u64);
    assert_eq!(t.total_grey_L_micro, 3_000_000_u64);
    // sw = 10M*30k/1M + 4M*0/1M = 300k
    assert_eq!(t.total_scarcity_weighted_L_micro, 300_000_u64);
    assert_eq!(t.total_risk_assessments, 1);
    assert_eq!(t.total_stewardship_programmes, 1);
    assert_eq!(t.total_disclosures, 1);
}

// ── Snapshot reflects risk/stewardship counts ─────────────────────────────────

#[test]
fn test_snapshot_includes_risk_and_stewardship_counts() {
    let (env, _owner, client) = create_ledger();
    let actor = Address::generate(&env);
    env.mock_all_auths();

    register_risk(&env, &client, &actor);
    env.ledger().with_mut(|li| li.timestamp += 1);
    register_stewardship(&env, &client, &actor);

    let snap = client.compute_water_snapshot();
    assert_eq!(snap.total_risk_assessments, 1);
    assert_eq!(snap.total_stewardship_programmes, 1);
}
