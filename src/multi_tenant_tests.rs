//! Comprehensive tests for multi-tenant isolation (Issue #394).
//!
//! Tests cover:
//! - Tenant lifecycle: create → suspend → resume → archive → delete
//! - Tenant event logging and querying within a namespace
//! - Complete namespace isolation (no cross-tenant data leakage)
//! - Per-tenant governance: admins, caps, config updates
//! - Per-tenant resource quotas and rate limits
//! - Edge cases: empty tenants, boundary values, concurrent tenants

#[cfg(test)]
mod tests {
    use crate::multi_tenant::{
        MultiTenantLedger, MultiTenantLedgerClient, TenantError, TenantStatus,
    };
    use soroban_sdk::{
        symbol_short,
        testutils::Address as _,
        Address, Bytes, Env, Symbol, Vec,
    };

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn create_env() -> Env {
        Env::default()
    }

    fn register_client(env: &Env) -> MultiTenantLedgerClient<'static> {
        let contract_id = env.register(MultiTenantLedger, ());
        MultiTenantLedgerClient::new(env, &contract_id)
    }

    fn default_description(env: &Env) -> Bytes {
        Bytes::from_slice(env, b"test tenant")
    }

    fn metadata(env: &Env, s: &[u8]) -> Bytes {
        Bytes::from_slice(env, s)
    }

    // ── 1. Tenant lifecycle tests ─────────────────────────────────────────────

    #[test]
    fn test_initialize_tenant_basic() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("acme");

        env.mock_all_auths();
        let config = client.initialize_tenant(
            &admin,
            &tid,
            &1000,
            &4096,
            &0,
            &default_description(&env),
        );

        assert_eq!(config.tenant_id, tid);
        assert_eq!(config.creator, admin);
        assert_eq!(config.status, TenantStatus::Active);
        assert_eq!(config.max_events, 1000);
        assert_eq!(config.max_metadata_bytes, 4096);
        assert_eq!(config.total_events, 0);
    }

    #[test]
    fn test_initialize_tenant_zero_metadata_defaults_to_1024() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("acme");

        env.mock_all_auths();
        let config = client.initialize_tenant(
            &admin,
            &tid,
            &100,
            &0, // should default to 1024
            &0,
            &default_description(&env),
        );

        assert_eq!(config.max_metadata_bytes, 1024);
    }

    #[test]
    #[should_panic]
    fn test_initialize_tenant_duplicate_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("dup");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        // Second call with same ID must panic
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
    }

    #[test]
    fn test_suspend_and_resume_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("alpha");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        // Suspend
        client.suspend_tenant(&admin, &tid);
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Suspended);

        // Resume
        client.resume_tenant(&admin, &tid);
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Active);
    }

    #[test]
    fn test_suspend_is_idempotent() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("idem");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.suspend_tenant(&admin, &tid);
        client.suspend_tenant(&admin, &tid); // second call should not panic
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Suspended);
    }

    #[test]
    fn test_resume_is_idempotent() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("idem2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.resume_tenant(&admin, &tid); // already active, should not panic
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Active);
    }

    #[test]
    fn test_archive_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("arc");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Archived);
    }

    #[test]
    fn test_archive_is_idempotent() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("arc2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);
        client.archive_tenant(&admin, &tid); // idempotent
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Archived);
    }

    #[test]
    fn test_delete_empty_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("del");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);
        client.delete_tenant(&admin, &tid, &false);
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Deleted);
    }

    #[test]
    #[should_panic]
    fn test_delete_active_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("del2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        // Cannot delete without archiving first
        client.delete_tenant(&admin, &tid, &false);
    }

    #[test]
    #[should_panic]
    fn test_delete_tenant_with_events_no_force_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("del3");
        let submitter = Address::generate(&env);

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"tx1"),
            &None,
        );
        client.archive_tenant(&admin, &tid);
        // force=false should panic because there are events
        client.delete_tenant(&admin, &tid, &false);
    }

    #[test]
    fn test_delete_tenant_with_events_force_succeeds() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("del4");
        let submitter = Address::generate(&env);

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"tx1"),
            &None,
        );
        client.archive_tenant(&admin, &tid);
        client.delete_tenant(&admin, &tid, &true); // force=true
        let config = client.get_tenant_config(&tid);
        assert_eq!(config.status, TenantStatus::Deleted);
    }

    // ── 2. Event logging tests ────────────────────────────────────────────────

    #[test]
    fn test_log_single_event() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("t1");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let event_id = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("payment"),
            &metadata(&env, b"tx-data"),
            &None,
        );

        let event = client.get_tenant_event(&tid, &event_id);
        assert_eq!(event.tenant_id, tid);
        assert_eq!(event.index, 0);
        assert_eq!(event.submitter, submitter);
        assert_eq!(event.metadata, metadata(&env, b"tx-data"));
        assert_eq!(event.event_type, symbol_short!("payment"));
    }

    #[test]
    fn test_log_multiple_events_sequential_index() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("t2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let id0 = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"a"),
            &None,
        );
        let id1 = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"b"),
            &None,
        );
        let id2 = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("refund"),
            &metadata(&env, b"c"),
            &None,
        );

        assert_eq!(client.tenant_total_events(&tid), 3);
        assert_eq!(client.get_tenant_event(&tid, &id0).index, 0);
        assert_eq!(client.get_tenant_event(&tid, &id1).index, 1);
        assert_eq!(client.get_tenant_event(&tid, &id2).index, 2);
    }

    #[test]
    fn test_get_tenant_event_by_index() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("t3");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"first"),
            &None,
        );
        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"second"),
            &None,
        );

        let ev0 = client.get_tenant_event_by_index(&tid, &0);
        let ev1 = client.get_tenant_event_by_index(&tid, &1);

        assert_eq!(ev0.metadata, metadata(&env, b"first"));
        assert_eq!(ev1.metadata, metadata(&env, b"second"));
    }

    #[test]
    #[should_panic]
    fn test_get_tenant_event_by_index_out_of_bounds() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("t4");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        // No events logged yet, index 0 should panic
        client.get_tenant_event_by_index(&tid, &0);
    }

    #[test]
    fn test_get_tenant_event_by_type() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("t5");
        let pay = symbol_short!("pay");
        let refund = symbol_short!("refund");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"p1"), &None);
        client.log_tenant_event(&submitter, &tid, &refund, &metadata(&env, b"r1"), &None);
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"p2"), &None);

        assert_eq!(client.tenant_event_count_by_type(&tid, &pay), 2);
        assert_eq!(client.tenant_event_count_by_type(&tid, &refund), 1);

        let ev_pay0 = client.get_tenant_event_by_type(&tid, &pay, &0);
        let ev_pay1 = client.get_tenant_event_by_type(&tid, &pay, &1);
        let ev_refund0 = client.get_tenant_event_by_type(&tid, &refund, &0);

        assert_eq!(ev_pay0.metadata, metadata(&env, b"p1"));
        assert_eq!(ev_pay1.metadata, metadata(&env, b"p2"));
        assert_eq!(ev_refund0.metadata, metadata(&env, b"r1"));
    }

    #[test]
    #[should_panic]
    fn test_get_tenant_event_by_type_out_of_bounds() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("t6");
        let pay = symbol_short!("pay");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"p1"), &None);
        // Only 1 event, index 1 should panic
        client.get_tenant_event_by_type(&tid, &pay, &1);
    }

    // ── 3. Namespace isolation tests ──────────────────────────────────────────

    #[test]
    fn test_cross_tenant_event_isolation() {
        let env = create_env();
        let client = register_client(&env);
        let admin_a = Address::generate(&env);
        let admin_b = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid_a = symbol_short!("tenantA");
        let tid_b = symbol_short!("tenantB");

        env.mock_all_auths();
        client.initialize_tenant(&admin_a, &tid_a, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin_b, &tid_b, &100, &1024, &0, &default_description(&env));

        // Log events in tenant A
        let id_a = client.log_tenant_event(
            &submitter,
            &tid_a,
            &symbol_short!("pay"),
            &metadata(&env, b"tenant_a_event"),
            &None,
        );

        // Log events in tenant B
        let id_b = client.log_tenant_event(
            &submitter,
            &tid_b,
            &symbol_short!("pay"),
            &metadata(&env, b"tenant_b_event"),
            &None,
        );

        // IDs must be different (different namespaces → different content-addressed IDs)
        assert_ne!(id_a, id_b);

        // Tenant A event counts must be 1, not 2
        assert_eq!(client.tenant_total_events(&tid_a), 1);
        assert_eq!(client.tenant_total_events(&tid_b), 1);

        // Tenant A can retrieve its own event
        let ev_a = client.get_tenant_event(&tid_a, &id_a);
        assert_eq!(ev_a.metadata, metadata(&env, b"tenant_a_event"));
        assert_eq!(ev_a.tenant_id, tid_a);

        // Tenant B can retrieve its own event
        let ev_b = client.get_tenant_event(&tid_b, &id_b);
        assert_eq!(ev_b.metadata, metadata(&env, b"tenant_b_event"));
        assert_eq!(ev_b.tenant_id, tid_b);
    }

    #[test]
    #[should_panic]
    fn test_cross_tenant_event_id_not_accessible_in_other_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin_a = Address::generate(&env);
        let admin_b = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid_a = symbol_short!("tenantA");
        let tid_b = symbol_short!("tenantB");

        env.mock_all_auths();
        client.initialize_tenant(&admin_a, &tid_a, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin_b, &tid_b, &100, &1024, &0, &default_description(&env));

        let id_a = client.log_tenant_event(
            &submitter,
            &tid_a,
            &symbol_short!("pay"),
            &metadata(&env, b"secret"),
            &None,
        );

        // Attempting to read tenant A's event under tenant B must fail
        client.get_tenant_event(&tid_b, &id_a);
    }

    #[test]
    fn test_identical_payload_different_tenants_produce_different_ids() {
        let env = create_env();
        let client = register_client(&env);
        let admin_a = Address::generate(&env);
        let admin_b = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid_a = symbol_short!("tenantA");
        let tid_b = symbol_short!("tenantB");
        let same_meta = metadata(&env, b"identical payload");
        let same_type = symbol_short!("pay");

        env.mock_all_auths();
        client.initialize_tenant(&admin_a, &tid_a, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin_b, &tid_b, &100, &1024, &0, &default_description(&env));

        let id_a = client.log_tenant_event(&submitter, &tid_a, &same_type, &same_meta, &None);
        let id_b = client.log_tenant_event(&submitter, &tid_b, &same_type, &same_meta, &None);

        // Even identical payloads produce different IDs because tenant_id is baked into the hash
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn test_multiple_tenants_independent_counters() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid_a = symbol_short!("tA");
        let tid_b = symbol_short!("tB");
        let tid_c = symbol_short!("tC");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid_a, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin, &tid_b, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin, &tid_c, &100, &1024, &0, &default_description(&env));

        let pay = symbol_short!("pay");

        // Log 3 events in A, 1 in B, 0 in C
        for _ in 0..3 {
            client.log_tenant_event(&submitter, &tid_a, &pay, &metadata(&env, b"x"), &None);
        }
        client.log_tenant_event(&submitter, &tid_b, &pay, &metadata(&env, b"x"), &None);

        assert_eq!(client.tenant_total_events(&tid_a), 3);
        assert_eq!(client.tenant_total_events(&tid_b), 1);
        assert_eq!(client.tenant_total_events(&tid_c), 0);
    }

    // ── 4. Quota and cap tests ─────────────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_tenant_event_quota_exceeded() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("quota");

        env.mock_all_auths();
        // max_events = 2
        client.initialize_tenant(&admin, &tid, &2, &1024, &0, &default_description(&env));

        let pay = symbol_short!("pay");
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"1"), &None);
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"2"), &None);
        // Third event should panic with TenantQuotaExceeded
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"3"), &None);
    }

    #[test]
    fn test_tenant_unlimited_quota_zero_means_unlimited() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("unlim");

        env.mock_all_auths();
        // max_events = 0 means unlimited
        client.initialize_tenant(&admin, &tid, &0, &1024, &0, &default_description(&env));

        let pay = symbol_short!("pay");
        // Log 10 events without hitting quota
        for _ in 0..10 {
            client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"x"), &None);
        }
        assert_eq!(client.tenant_total_events(&tid), 10);
    }

    #[test]
    #[should_panic]
    fn test_tenant_per_type_cap_exceeded() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("typecap");
        let pay = symbol_short!("pay");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        // Set type cap to 1 for "pay"
        client.set_tenant_type_cap(&admin, &tid, &pay, &1);

        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"1"), &None);
        // Second "pay" event should panic with TenantTypeCapExceeded
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"2"), &None);
    }

    #[test]
    fn test_tenant_per_type_cap_only_affects_that_type() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("typecap2");
        let pay = symbol_short!("pay");
        let refund = symbol_short!("refund");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        // Cap "pay" at 1 but not "refund"
        client.set_tenant_type_cap(&admin, &tid, &pay, &1);

        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"1"), &None);
        // "refund" should still work
        client.log_tenant_event(&submitter, &tid, &refund, &metadata(&env, b"2"), &None);
        client.log_tenant_event(&submitter, &tid, &refund, &metadata(&env, b"3"), &None);

        assert_eq!(client.tenant_event_count_by_type(&tid, &pay), 1);
        assert_eq!(client.tenant_event_count_by_type(&tid, &refund), 2);
    }

    #[test]
    fn test_remove_type_cap_via_set_to_zero() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("remcap");
        let pay = symbol_short!("pay");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.set_tenant_type_cap(&admin, &tid, &pay, &1);
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"1"), &None);

        // Remove the cap by setting to 0
        client.set_tenant_type_cap(&admin, &tid, &pay, &0);

        // Should now be able to log more "pay" events
        client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"2"), &None);
        assert_eq!(client.tenant_event_count_by_type(&tid, &pay), 2);
    }

    #[test]
    #[should_panic]
    fn test_metadata_too_large_for_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("meta");

        env.mock_all_auths();
        // max_metadata_bytes = 10
        client.initialize_tenant(&admin, &tid, &100, &10, &0, &default_description(&env));

        // 11-byte metadata should panic
        let large_meta = Bytes::from_slice(&env, b"12345678901");
        client.log_tenant_event(&submitter, &tid, &symbol_short!("pay"), &large_meta, &None);
    }

    #[test]
    fn test_metadata_exactly_at_limit_succeeds() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("meta2");

        env.mock_all_auths();
        // max_metadata_bytes = 5
        client.initialize_tenant(&admin, &tid, &100, &5, &0, &default_description(&env));

        // Exactly 5 bytes should succeed
        let exact_meta = Bytes::from_slice(&env, b"12345");
        client.log_tenant_event(&submitter, &tid, &symbol_short!("pay"), &exact_meta, &None);
        assert_eq!(client.tenant_total_events(&tid), 1);
    }

    // ── 5. Governance tests ───────────────────────────────────────────────────

    #[test]
    fn test_add_and_verify_tenant_admin() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        let tid = symbol_short!("gov");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        assert!(client.is_tenant_admin(&tid, &admin));
        assert!(!client.is_tenant_admin(&tid, &new_admin));

        client.add_tenant_admin(&admin, &tid, &new_admin);
        assert!(client.is_tenant_admin(&tid, &new_admin));
    }

    #[test]
    fn test_remove_tenant_admin() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let second_admin = Address::generate(&env);
        let tid = symbol_short!("gov2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.add_tenant_admin(&admin, &tid, &second_admin);
        assert!(client.is_tenant_admin(&tid, &second_admin));

        client.remove_tenant_admin(&admin, &tid, &second_admin);
        assert!(!client.is_tenant_admin(&tid, &second_admin));
    }

    #[test]
    #[should_panic]
    fn test_non_admin_cannot_add_admin() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let not_admin = Address::generate(&env);
        let new_addr = Address::generate(&env);
        let tid = symbol_short!("gov3");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        // not_admin should not be able to add an admin
        client.add_tenant_admin(&not_admin, &tid, &new_addr);
    }

    #[test]
    fn test_update_tenant_config() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("upd");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let new_desc = Bytes::from_slice(&env, b"updated description");
        client.update_tenant_config(&admin, &tid, &500, &2048, &10, &new_desc);

        let config = client.get_tenant_config(&tid);
        assert_eq!(config.max_events, 500);
        assert_eq!(config.max_metadata_bytes, 2048);
        assert_eq!(config.rate_limit, 10);
        assert_eq!(config.description, new_desc);
    }

    #[test]
    #[should_panic]
    fn test_non_admin_cannot_update_config() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let not_admin = Address::generate(&env);
        let tid = symbol_short!("upd2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        client.update_tenant_config(
            &not_admin,
            &tid,
            &500,
            &2048,
            &0,
            &default_description(&env),
        );
    }

    // ── 6. Write-blocked states ───────────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_log_event_in_suspended_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("sus");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.suspend_tenant(&admin, &tid);

        // Event logging on a suspended tenant must fail
        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"x"),
            &None,
        );
    }

    #[test]
    #[should_panic]
    fn test_log_event_in_archived_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("arc3");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);

        client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"x"),
            &None,
        );
    }

    #[test]
    #[should_panic]
    fn test_get_event_in_deleted_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("del5");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);
        client.delete_tenant(&admin, &tid, &false);

        // Any get on a deleted tenant must fail
        client.tenant_total_events(&tid);
    }

    // ── 7. Tenant registry and accessibility ──────────────────────────────────

    #[test]
    fn test_list_tenants() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let t1 = symbol_short!("tL1");
        let t2 = symbol_short!("tL2");
        let t3 = symbol_short!("tL3");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &t1, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin, &t2, &100, &1024, &0, &default_description(&env));
        client.initialize_tenant(&admin, &t3, &100, &1024, &0, &default_description(&env));

        let ids = client.list_tenants();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&t1));
        assert!(ids.contains(&t2));
        assert!(ids.contains(&t3));
    }

    #[test]
    fn test_tenant_accessible_active() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("acc");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        assert!(client.tenant_accessible(&tid));
    }

    #[test]
    fn test_tenant_accessible_suspended_is_accessible() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("acc2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.suspend_tenant(&admin, &tid);
        // Suspended tenants are still "accessible" (readable, just no writes)
        assert!(client.tenant_accessible(&tid));
    }

    #[test]
    fn test_tenant_accessible_archived_is_not() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("acc3");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        client.archive_tenant(&admin, &tid);
        assert!(!client.tenant_accessible(&tid));
    }

    #[test]
    fn test_tenant_accessible_nonexistent_is_false() {
        let env = create_env();
        let client = register_client(&env);
        let nonexistent = symbol_short!("ghost");

        env.mock_all_auths();
        assert!(!client.tenant_accessible(&nonexistent));
    }

    // ── 8. Hash-chain integrity ───────────────────────────────────────────────

    #[test]
    fn test_hash_chain_genesis_is_all_zeros() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("chain");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let id = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"genesis"),
            &None,
        );
        let event = client.get_tenant_event(&tid, &id);

        // Genesis event must have all-zero prev_hash
        let expected_zero = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        assert_eq!(event.prev_hash, expected_zero);
    }

    #[test]
    fn test_hash_chain_subsequent_events() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("chain2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let id0 = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"first"),
            &None,
        );
        let id1 = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"second"),
            &None,
        );

        let ev1 = client.get_tenant_event(&tid, &id1);
        // Second event's prev_hash must equal first event's ID
        assert_eq!(ev1.prev_hash, id0);
    }

    // ── 9. Sub-event-type tests ───────────────────────────────────────────────

    #[test]
    fn test_sub_event_type_stored_correctly() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("sub");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let sub_type = symbol_short!("card");
        let id = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"tx"),
            &Some(sub_type.clone()),
        );
        let event = client.get_tenant_event(&tid, &id);
        assert_eq!(event.sub_event_type, Some(sub_type));
    }

    #[test]
    fn test_no_sub_event_type_is_none() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("sub2");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let id = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &metadata(&env, b"tx"),
            &None,
        );
        let event = client.get_tenant_event(&tid, &id);
        assert_eq!(event.sub_event_type, None);
    }

    // ── 10. Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn test_empty_metadata_allowed() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("emp");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));

        let id = client.log_tenant_event(
            &submitter,
            &tid,
            &symbol_short!("pay"),
            &Bytes::new(&env),
            &None,
        );
        let event = client.get_tenant_event(&tid, &id);
        assert_eq!(event.metadata, Bytes::new(&env));
    }

    #[test]
    #[should_panic]
    fn test_get_event_from_nonexistent_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let ghost = symbol_short!("ghost");

        env.mock_all_auths();
        let fake_id = soroban_sdk::BytesN::from_array(&env, &[1u8; 32]);
        client.get_tenant_event(&ghost, &fake_id);
    }

    #[test]
    fn test_tenant_event_total_zero_for_new_tenant() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("zero");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        assert_eq!(client.tenant_total_events(&tid), 0);
    }

    #[test]
    fn test_tenant_type_count_zero_for_unknown_type() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let tid = symbol_short!("tc");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &tid, &100, &1024, &0, &default_description(&env));
        assert_eq!(
            client.tenant_event_count_by_type(&tid, &symbol_short!("nope")),
            0
        );
    }

    // ── 11. Rate limit tests ──────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_zero_means_no_limit() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let tid = symbol_short!("rate");

        env.mock_all_auths();
        // rate_limit = 0 means no rate limiting
        client.initialize_tenant(&admin, &tid, &0, &1024, &0, &default_description(&env));

        let pay = symbol_short!("pay");
        for _ in 0..5 {
            client.log_tenant_event(&submitter, &tid, &pay, &metadata(&env, b"x"), &None);
        }
        assert_eq!(client.tenant_total_events(&tid), 5);
    }

    // ── 12. Config get/list isolation ─────────────────────────────────────────

    #[test]
    fn test_get_config_for_specific_tenant_not_other() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);
        let ta = symbol_short!("cfgA");
        let tb = symbol_short!("cfgB");

        env.mock_all_auths();
        client.initialize_tenant(&admin, &ta, &111, &512, &5, &metadata(&env, b"A"));
        client.initialize_tenant(&admin, &tb, &222, &2048, &10, &metadata(&env, b"B"));

        let ca = client.get_tenant_config(&ta);
        let cb = client.get_tenant_config(&tb);

        assert_eq!(ca.max_events, 111);
        assert_eq!(ca.max_metadata_bytes, 512);
        assert_eq!(ca.rate_limit, 5);

        assert_eq!(cb.max_events, 222);
        assert_eq!(cb.max_metadata_bytes, 2048);
        assert_eq!(cb.rate_limit, 10);
    }

    // ── 13. Admin operations on non-existent/deleted tenants ─────────────────

    #[test]
    #[should_panic]
    fn test_suspend_nonexistent_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);

        env.mock_all_auths();
        client.suspend_tenant(&admin, &symbol_short!("ghost"));
    }

    #[test]
    #[should_panic]
    fn test_resume_nonexistent_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);

        env.mock_all_auths();
        client.resume_tenant(&admin, &symbol_short!("ghost"));
    }

    #[test]
    #[should_panic]
    fn test_archive_nonexistent_tenant_fails() {
        let env = create_env();
        let client = register_client(&env);
        let admin = Address::generate(&env);

        env.mock_all_auths();
        client.archive_tenant(&admin, &symbol_short!("ghost"));
    }
}
