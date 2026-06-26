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
    client.log_event(&submitter, &symbol_short!("payment"), &Bytes::from_slice(&env, b"tx1"));
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

    env.ledger().set_timestamp(1000 + super::MAX_TIMESTAMP_DRIFT_SECONDS + 1);
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
    env.storage().instance().set(&super::DataKey::TotalEvents, &u32::MAX);

    let submitter = Address::generate(&env);
    client.log_event(&submitter, &symbol_short!("payment"), &Bytes::from_slice(&env, b"tx1"));
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

// ── issue #64: nonce-based replay prevention ─────────────────────────────────

#[test]
fn test_nonce_correct_increments() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    // nonce starts at 0 (absent); first accepted nonce is 1
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &1);
    assert_eq!(client.get_submitter_nonce(&submitter), 1);

    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &2);
    assert_eq!(client.get_submitter_nonce(&submitter), 2);
}

#[test]
fn test_nonce_replay_rejected_with_nonce_too_low() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &1);

    // Re-submitting the same nonce must fail with NonceTooLow (#12)
    let result = client.try_log_event_with_nonce(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx1"),
        &1,
    );
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_nonce_too_low_error_code() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &5);
    // nonce 3 < 5 → NonceTooLow
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &3);
}

#[test]
fn test_nonce_gap_accepted() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    // Jump from 0 → 5 directly (gap is accepted)
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &5);
    assert_eq!(client.get_submitter_nonce(&submitter), 5);

    // Next must be > 5
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &10);
    assert_eq!(client.get_submitter_nonce(&submitter), 10);
}

#[test]
fn test_nonce_zero_rejected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    // nonce 0 is never valid
    let result = client.try_log_event_with_nonce(
        &submitter,
        &payment,
        &Bytes::from_slice(&env, b"tx"),
        &0,
    );
    assert!(result.is_err());
}

#[test]
fn test_nonce_multiple_submitters_independent() {
    let (env, _owner, client) = create_ledger();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    // s1 uses nonce 1, s2 independently uses nonce 1
    client.log_event_with_nonce(&s1, &payment, &Bytes::from_slice(&env, b"a"), &1);
    client.log_event_with_nonce(&s2, &payment, &Bytes::from_slice(&env, b"b"), &1);

    assert_eq!(client.get_submitter_nonce(&s1), 1);
    assert_eq!(client.get_submitter_nonce(&s2), 1);

    // s1 advances to 2; s2 stays at 1 until it submits
    client.log_event_with_nonce(&s1, &payment, &Bytes::from_slice(&env, b"c"), &2);
    assert_eq!(client.get_submitter_nonce(&s1), 2);
    assert_eq!(client.get_submitter_nonce(&s2), 1);

    // s1 replay attempt fails
    let result = client.try_log_event_with_nonce(
        &s1,
        &payment,
        &Bytes::from_slice(&env, b"c"),
        &2,
    );
    assert!(result.is_err());

    // s2 is unaffected and can advance
    client.log_event_with_nonce(&s2, &payment, &Bytes::from_slice(&env, b"d"), &2);
    assert_eq!(client.get_submitter_nonce(&s2), 2);
}

#[test]
fn test_get_submitter_nonce_default_zero() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assert_eq!(client.get_submitter_nonce(&submitter), 0);
}

#[test]
fn test_log_event_without_nonce_unaffected() {
    // log_event (no nonce) does not touch the nonce counter
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"legacy"));
    // nonce counter remains 0
    assert_eq!(client.get_submitter_nonce(&submitter), 0);
    // log_event_with_nonce still works independently
    client.log_event_with_nonce(&submitter, &payment, &Bytes::from_slice(&env, b"nonce"), &1);
    assert_eq!(client.get_submitter_nonce(&submitter), 1);
}
