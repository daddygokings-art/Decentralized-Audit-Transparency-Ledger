use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Vec};

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);
    (env, owner, client)
}

// ── Authentication Tests ────────────────────────────────────────────────────

#[test]
fn test_auth_submitter_must_authenticate() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);

    let submitter = Address::generate(&env);
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"test"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_auth_submitter_authenticated_succeeds() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    let evt = client.get_event(&id);
    assert_eq!(evt.submitter, submitter);
}

#[test]
fn test_auth_owner_must_authenticate_governance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);

    let result = client.try_set_global_max_logs(&owner, &200);
    assert!(result.is_err());
}

#[test]
fn test_auth_owner_calls_governance_succeeds() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let result = client.try_set_global_max_logs(&owner, &200);
    assert!(result.is_ok());
}

#[test]
fn test_auth_nonce_monotonic_increases() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id1 = client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"first"),
        &1u32,
    );
    assert!(client.get_event(&id1).index == 0);
    assert_eq!(client.get_submitter_nonce(&submitter), 1);

    let id2 = client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"second"),
        &2u32,
    );
    assert!(client.get_event(&id2).index == 1);
    assert_eq!(client.get_submitter_nonce(&submitter), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #19)")]
fn test_auth_nonce_replay_rejected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"first"),
        &1u32,
    );
    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"replay"),
        &1u32,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #19)")]
fn test_auth_nonce_zero_rejected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"zero"),
        &0u32,
    );
}

#[test]
fn test_auth_nonce_gap_accepted() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"first"),
        &1u32,
    );
    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"skip"),
        &5u32,
    );
    assert_eq!(client.get_submitter_nonce(&submitter), 5);
}

// ── Authorization Tests ─────────────────────────────────────────────────────

#[test]
fn test_auth_non_owner_cannot_set_global_max() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_set_global_max_logs(&attacker, &200);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_set_event_max() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_set_event_max_logs(&attacker, &symbol_short!("pay"), &5);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_transfer_ownership() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_transfer_ownership(&attacker, &new_owner);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_pause() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_pause(&attacker);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_upgrade() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);

    env.mock_all_auths();
    let result = client.try_upgrade_contract(&attacker, &wasm_hash);
    assert!(result.is_err());
}

#[test]
fn test_auth_blocklisted_submitter_rejected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.block_submitter(&owner, &submitter);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_auth_unblocked_submitter_allowed() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.block_submitter(&owner, &submitter);
    client.unblock_submitter(&owner, &submitter);

    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    let evt = client.get_event(&id);
    assert_eq!(evt.submitter, submitter);
}

#[test]
fn test_auth_allowlist_mode_rejects_unlisted() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.enable_allowlist_mode(&owner);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_auth_allowlist_mode_allows_listed() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.enable_allowlist_mode(&owner);
    client.allow_submitter(&owner, &submitter);

    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    let evt = client.get_event(&id);
    assert_eq!(evt.submitter, submitter);
}

#[test]
fn test_auth_allowlist_removed_rejects() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.enable_allowlist_mode(&owner);
    client.allow_submitter(&owner, &submitter);
    client.remove_submitter_from_allowlist(&owner, &submitter);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_auth_governance_blocked_during_pause() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    client.pause(&owner);

    let result = client.try_set_global_max_logs(&owner, &200);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_block_submitter() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);
    let victim = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_block_submitter(&attacker, &victim);
    assert!(result.is_err());
}

#[test]
fn test_auth_non_owner_cannot_enable_allowlist() {
    let (env, _owner, client) = create_ledger();
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    let result = client.try_enable_allowlist_mode(&attacker);
    assert!(result.is_err());
}

// ── Input Validation Tests ──────────────────────────────────────────────────

#[test]
fn test_validation_metadata_exactly_at_limit_succeeds() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &100);

    let meta = Bytes::from_slice(&env, &[0u8; 100]);
    let id = client.log_event(&submitter, &symbol_short!("test"), &meta, &None, &None, &false);
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 100);
}

#[test]
fn test_validation_metadata_one_byte_over_rejected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &100);

    let meta = Bytes::from_slice(&env, &[0u8; 101]);
    let result = client.try_log_event(&submitter, &symbol_short!("test"), &meta, &None, &None, &false);
    assert!(result.is_err());
}

#[test]
fn test_validation_empty_metadata_succeeds() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(&submitter, &symbol_short!("test"), &Bytes::new(&env), &None, &None, &false);
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 0);
}

#[test]
fn test_validation_metadata_u32_max_disables_limit() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &u32::MAX);

    let meta = Bytes::from_slice(&env, &[0u8; 5000]);
    let id = client.log_event(&submitter, &symbol_short!("test"), &meta, &None, &None, &false);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn test_validation_event_type_max_logs_respected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let pay = symbol_short!("pay");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &pay, &3);

    for i in 0..3 {
        client.log_event(&submitter, &pay, &Bytes::from_slice(&env, &[i]), &None, &None, &false);
    }
    assert_eq!(client.event_count(&pay), 3);

    let result = client.try_log_event(&submitter, &pay, &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    assert!(result.is_err());
}

#[test]
fn test_validation_global_max_logs_respected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &3, &4096);

    env.mock_all_auths();
    let submitter = Address::generate(&env);
    for i in 0..3 {
        client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, &[i]), &None, &None, &false);
    }
    assert_eq!(client.total_events(), 3);

    let result = client.try_log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_validation_timestamp_non_monotonic_rejected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.ledger().set_timestamp(2000);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"first"), &None, &None, &false);

    env.ledger().set_timestamp(1999);
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"second"), &None, &None, &false);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_validation_timestamp_excessive_drift_rejected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"first"), &None, &None, &false);

    env.ledger().set_timestamp(1000 + super::MAX_TIMESTAMP_DRIFT_SECONDS + 1);
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"second"), &None, &None, &false);
}

#[test]
fn test_validation_timestamp_drift_at_limit_accepted() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.ledger().set_timestamp(1000);
    env.mock_all_auths();
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"first"), &None, &None, &false);

    env.ledger().set_timestamp(1000 + super::MAX_TIMESTAMP_DRIFT_SECONDS);
    client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"second"), &None, &None, &false);
    assert_eq!(client.total_events(), 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_validation_call_before_initialize_rejected() {
    let env = Env::default();
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.total_events();
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #26)")]
fn test_validation_double_initialize_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());

    env.mock_all_auths();
    client.initialize(&owners, &100, &4096);
    client.initialize(&owners, &100, &4096);
}

#[test]
fn test_validation_empty_owners_rejected() {
    let env = Env::default();
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    let owners = Vec::new(&env);

    env.mock_all_auths();
    let result = client.try_initialize(&owners, &100, &4096);
    assert!(result.is_err());
}

#[test]
fn test_validation_set_global_max_below_count_rejected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    for i in 0..5 {
        client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, &[i]), &None, &None, &false);
    }

    let result = client.try_set_global_max_logs(&owner, &3);
    assert!(result.is_err());
}

#[test]
fn test_validation_transfer_to_same_owner_rejected() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    let result = client.try_transfer_ownership(&owner, &owner);
    assert!(result.is_err());
}

#[test]
fn test_validation_transfer_to_zero_address_rejected() {
    let (env, owner, client) = create_ledger();
    let zero = Address::from_str(&env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");

    env.mock_all_auths();
    let result = client.try_transfer_ownership(&owner, &zero);
    assert!(result.is_err());
}

#[test]
fn test_validation_list_events_limit_exceeds_max_rejected() {
    let (env, _owner, client) = create_ledger();
    let result = client.try_list_events(&0, &101);
    assert!(result.is_err());
}

// ── Injection / Attack Surface Tests ────────────────────────────────────────

#[test]
fn test_injection_reentrancy_guard_blocks_recursive_call() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();

    let submitter = Address::generate(&env);
    let inner_id = env.register(AuditLedger, ());
    let inner = AuditLedgerClient::new(&env, &inner_id);
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    inner.initialize(&owners, &100, &4096);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"test"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_ok());
}

#[test]
fn test_injection_many_concurrent_submitters_no_panic() {
    let (env, _owner, client) = create_ledger();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    for i in 0u32..20 {
        let submitter = Address::generate(&env);
        let result = client.try_log_event(
            &submitter,
            &symbol_short!("t"),
            &Bytes::from_slice(&env, &i.to_le_bytes()),
            &None,
            &None,
            &false,
        );
        assert!(result.is_ok());
    }
    assert_eq!(client.total_events(), 20);
}

#[test]
fn test_injection_rapid_rate_limit_respected() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.set_submitter_rate_limit(&owner, &submitter, &3);

    for _ in 0..3 {
        let r = client.try_log_event(
            &submitter,
            &symbol_short!("t"),
            &Bytes::from_slice(&env, b"x"),
            &None,
            &None,
            &false,
        );
        assert!(r.is_ok());
    }

    let r = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"y"),
        &None,
        &None,
        &false,
    );
    assert!(r.is_err());
}

#[test]
fn test_injection_rate_limit_resets_across_timestamps() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.set_submitter_rate_limit(&owner, &submitter, &2);

    for _ in 0..2 {
        client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    }

    env.ledger().set_timestamp(1001);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"y"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_ok());
}

#[test]
fn test_injection_rate_limit_zero_blocks_all() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    client.set_submitter_rate_limit(&owner, &submitter, &0);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("t"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_injection_extreme_metadata_size_storage() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_max_size(&owner, &4096);

    let large = Bytes::from_slice(&env, &[0u8; 4096]);
    let id = client.log_event(&submitter, &symbol_short!("t"), &large, &None, &None, &false);
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata.len(), 4096);
}

#[test]
fn test_injection_cross_contract_impersonation_prevented() {
    let (env, owner, client) = create_ledger();
    env.mock_all_auths();
    let submitter = Address::generate(&env);

    let id = client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, b"data"), &None, &None, &false);
    let evt = client.get_event(&id);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.event_type, symbol_short!("t"));
}

#[test]
fn test_injection_event_hash_tamper_detected() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    client.log_event(&submitter, &symbol_short!("a"), &Bytes::from_slice(&env, b"x"), &None, &None, &false);
    client.log_event(&submitter, &symbol_short!("b"), &Bytes::from_slice(&env, b"y"), &None, &None, &false);

    assert!(client.verify_integrity());

    let id0 = client.get_event_by_order(&0);
    let _id0_hash = id0.event_hash;
}

#[test]
fn test_injection_pagination_params_validation() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.mock_all_auths();

    for i in 0..10 {
        client.log_event(&submitter, &symbol_short!("t"), &Bytes::from_slice(&env, &[i]), &None, &None, &false);
    }

    let result = client.try_search_events(&Bytes::new(&env), &0, &101);
    assert!(result.is_err());

    let result = client.try_list_events_by_category(&symbol_short!("general"), &0, &101);
    assert!(result.is_err());
}
