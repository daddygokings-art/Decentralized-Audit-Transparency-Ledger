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
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_get_event_by_type_out_of_bounds_panics() {
    let (_env, _owner, client) = create_ledger();
    let payment = symbol_short!("payment");
    client.get_event_by_type(&payment, &0);
}

// ── issue #70: hash-based IDs ───────────────────────────────────────────────

#[test]
fn test_event_ids_are_bytes32() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    let id: BytesN<32> =
        client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"));
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
    client.log_event(&submitter, &symbol_short!("p"), &Bytes::from_slice(&env, b"x"));

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
    let id = client.log_event(&submitter, &symbol_short!("p"), &meta);
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
    let id = client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, &[0u8; 50]));
    assert_eq!(client.total_events(), 1);
    // 51 bytes → rejected
    let r2 = client.try_log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, &[0u8; 51]));
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
    let id = client.log_event(&submitter, &lett, &Bytes::from_slice(&env, &[0u8; 50]));
    assert_eq!(client.total_events(), 1);
    // type "z" uses global cap of 10 → 11 fails
    let r2 = client.try_log_event(&submitter, &symbol_short!("z"), &Bytes::from_slice(&env, &[0u8; 11]));
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
    let id = client.log_event(&submitter, &symbol_short!("p"), &Bytes::from_slice(&env, b"x"));
    let stored = client.get_event_signature(&id);
    assert!(stored.is_none());
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
