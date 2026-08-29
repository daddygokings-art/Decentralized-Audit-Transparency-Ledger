#[cfg(test)]
mod tests {
    use crate::finops::*;
    use soroban_sdk::{bytes, vec, Address, BytesN, Env, Symbol};

    fn create_test_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    fn sample_name(env: &Env) -> Bytes {
        bytes!(env, b"TestCostCenter")
    }

    fn sample_period(env: &Env) -> Bytes {
        bytes!(env, b"2025-01")
    }

    fn sample_team(env: &Env) -> Bytes {
        bytes!(env, b"platform-team")
    }

    fn sample_region(env: &Env) -> Bytes {
        bytes!(env, b"us-east-1")
    }

    // ── Cost Center Tests ──────────────────────────────────────────────

    #[test]
    fn test_register_cost_center() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let id = register_cost_center(
            env.clone(),
            caller,
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        assert!(!id.to_vec().is_empty());

        let center = get_cost_center(env.clone(), id);
        assert_eq!(center.budget, 100000);
        assert_eq!(center.currency, Symbol::new(&env, "USD"));
        assert!(center.active);
    }

    #[test]
    fn test_duplicate_cost_center_fails() {
        let env = create_test_env();
        let caller = Address::random(&env);

        register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            register_cost_center(
                env,
                caller,
                sample_name(&env),
                200000,
                Symbol::new(&env, "USD"),
            );
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_update_cost_center_budget() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        update_cost_center_budget(env.clone(), caller, id, 200000);

        let center = get_cost_center(env, id);
        assert_eq!(center.budget, 200000);
    }

    #[test]
    fn test_deactivate_cost_center() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        deactivate_cost_center(env.clone(), caller, id);

        let center = get_cost_center(env, id);
        assert!(!center.active);
    }

    // ── Cost Allocation Tests ──────────────────────────────────────────

    #[test]
    fn test_allocate_cost() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let alloc_id = allocate_cost(
            env.clone(),
            caller,
            cc_id,
            Symbol::new(&env, "compute"),
            5000,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            bytes!(env, b"monthly-compute"),
        );

        assert!(!alloc_id.to_vec().is_empty());

        let alloc = get_allocation(env, alloc_id);
        assert_eq!(alloc.amount, 5000);
        assert_eq!(alloc.resource_type, Symbol::new(&env, "compute"));
    }

    // ── Chargeback / Showback Tests ────────────────────────────────────

    #[test]
    fn test_create_chargeback() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let cb_id = create_chargeback(
            env.clone(),
            caller,
            sample_team(&env),
            2500,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            cc_id,
        );

        assert!(!cb_id.to_vec().is_empty());
    }

    #[test]
    fn test_create_showback() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let sb_id = create_showback(
            env.clone(),
            caller,
            sample_team(&env),
            2500,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            cc_id,
        );

        assert!(!sb_id.to_vec().is_empty());
    }

    #[test]
    fn test_approve_chargeback() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let cb_id = create_chargeback(
            env.clone(),
            caller,
            sample_team(&env),
            2500,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            cc_id,
        );

        approve_chargeback(env.clone(), caller, cb_id);

        let key = DataKey::Chargeback(cb_id);
        if let Some(record) = env.storage().persistent().get::<_, ChargebackRecord>(&key) {
            assert_eq!(record.status, 2);
        }
    }

    // ── Rightsizing Tests ──────────────────────────────────────────────

    #[test]
    fn test_record_resource() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let resource_id = record_resource(
            env.clone(),
            caller,
            Symbol::new(&env, "ec2"),
            8,
            800,
            sample_region(&env),
        );

        assert!(!resource_id.to_vec().is_empty());

        let resource = env
            .storage()
            .persistent()
            .get(&DataKey::Resource(resource_id))
            .unwrap();
        assert_eq!(resource.current_size, 8);
        assert_eq!(resource.monthly_cost, 800);
    }

    #[test]
    fn test_generate_rightsizing() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let resource_id = record_resource(
            env.clone(),
            caller.clone(),
            Symbol::new(&env, "ec2"),
            8,
            800,
            sample_region(&env),
        );

        let rec_id = generate_rightsizing(env.clone(), caller, resource_id);

        let rec = get_rightsizing(env, rec_id);
        assert_eq!(rec.current_size, 8);
        assert_eq!(rec.recommended_size, 4);
        assert_eq!(rec.monthly_savings, 400);
        assert_eq!(rec.confidence, 75);
    }

    // ── Anomaly Detection Tests ────────────────────────────────────────

    #[test]
    fn test_record_anomaly() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let resource_id = BytesN::from_slice(&env, b"resource-anomaly-test");
        let anomaly_id = record_anomaly(
            env.clone(),
            caller,
            resource_id,
            1000,
            1500,
            bytes!(env, b"spike-due-to-outage"),
        );

        assert!(!anomaly_id.to_vec().is_empty());

        let anomaly = env
            .storage()
            .persistent()
            .get(&DataKey::CostAnomaly(anomaly_id))
            .unwrap();
        assert_eq!(anomaly.actual_cost, 1500);
        assert_eq!(anomaly.expected_cost, 1000);
        assert!(!anomaly.resolved);
    }

    #[test]
    fn test_detect_anomalies() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let resource_id = BytesN::from_slice(&env, b"resource-anomaly-test-2");
        record_anomaly(
            env.clone(),
            caller.clone(),
            resource_id.clone(),
            1000,
            2000,
            bytes!(env, b"high-deviation"),
        );

        let anomalies = detect_anomalies(env, caller, resource_id);
        assert_eq!(anomalies.len(), 1);
    }

    #[test]
    fn test_resolve_anomaly() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let resource_id = BytesN::from_slice(&env, b"resource-anomaly-test-3");
        let anomaly_id = record_anomaly(
            env.clone(),
            caller.clone(),
            resource_id,
            1000,
            1500,
            bytes!(env, b"resolve-test"),
        );

        resolve_anomaly(env.clone(), caller, anomaly_id.clone());

        let anomaly = env
            .storage()
            .persistent()
            .get(&DataKey::CostAnomaly(anomaly_id))
            .unwrap();
        assert!(anomaly.resolved);
    }

    // ── Budget Tests ───────────────────────────────────────────────────

    #[test]
    fn test_create_budget() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let thresholds = vec![&env, 50, 80, 95];
        let budget_id = create_budget(
            env.clone(),
            caller,
            bytes!(env, b"monthly-budget"),
            cc_id,
            50000,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            thresholds,
        );

        assert!(!budget_id.to_vec().is_empty());

        let budget = get_budget(env, budget_id);
        assert_eq!(budget.amount, 50000);
        assert!(budget.active);
    }

    #[test]
    fn test_check_budget_alert() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        let thresholds = vec![&env, 50, 80];
        let budget_id = create_budget(
            env.clone(),
            caller.clone(),
            bytes!(env, b"monthly-budget"),
            cc_id,
            100000,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            thresholds,
        );

        let alert_id = check_budget(env.clone(), caller, budget_id, 85000);
        assert!(!alert_id.to_vec().is_empty());
    }

    // ── Dashboard Tests ────────────────────────────────────────────────

    #[test]
    fn test_generate_dashboard() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        allocate_cost(
            env.clone(),
            caller.clone(),
            cc_id,
            Symbol::new(&env, "compute"),
            5000,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            bytes!(env, b"monthly-compute"),
        );

        record_resource(
            env.clone(),
            caller,
            Symbol::new(&env, "ec2"),
            8,
            800,
            sample_region(&env),
        );

        let dashboard = generate_dashboard(env);
        assert!(dashboard.timestamp > 0);
    }

    #[test]
    fn test_get_cost_summary() {
        let env = create_test_env();
        let caller = Address::random(&env);

        let cc_id = register_cost_center(
            env.clone(),
            caller.clone(),
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        allocate_cost(
            env,
            caller,
            cc_id,
            Symbol::new(&env, "compute"),
            5000,
            Symbol::new(&env, "USD"),
            sample_period(&env),
            bytes!(env, b"monthly-compute"),
        );

        let summary = get_cost_summary(env, sample_period(&env));
        assert_eq!(summary.total_cost, 5000);
    }

    // ── Query Helper Tests ─────────────────────────────────────────────

    #[test]
    fn test_cost_center_count() {
        let env = create_test_env();
        let caller = Address::random(&env);

        assert_eq!(get_cost_center_count(env.clone()), 0);

        register_cost_center(
            env.clone(),
            caller,
            sample_name(&env),
            100000,
            Symbol::new(&env, "USD"),
        );

        assert_eq!(get_cost_center_count(env), 1);
    }

    #[test]
    fn test_active_anomaly_count() {
        let env = create_test_env();
        let caller = Address::random(&env);

        assert_eq!(get_active_anomaly_count(env.clone()), 0);

        let resource_id = BytesN::from_slice(&env, b"resource-anomaly-count");
        record_anomaly(
            env.clone(),
            caller,
            resource_id,
            1000,
            1500,
            bytes!(env, b"count-test"),
        );

        assert_eq!(get_active_anomaly_count(env), 1);
    }
}
