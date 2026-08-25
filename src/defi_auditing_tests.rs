//! Comprehensive tests for DeFi protocol auditing system
//!
//! Tests cover TVL tracking, oracle verification, liquidation monitoring,
//! governance tracking, risk metrics, and audit report generation.

#[cfg(test)]
mod defi_auditing_tests {
    use crate::defi_auditing::*;
    use crate::defi_auditing_impl::*;
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Symbol, Vec};

    fn setup_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    fn create_test_address(id: u8) -> Address {
        Address::generate(&setup_env())
    }

    // ==================== PROTOCOL MANAGEMENT TESTS ====================

    #[test]
    fn test_register_protocol() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        let result = DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "uniswap"),
            ProtocolType::AMM,
            Symbol::new(&env, "ethereum"),
            None,
        );

        assert!(result.is_ok());

        // Verify registration
        let registry = DeFiAuditingContract::get_protocol(env, protocol)
            .expect("Failed to get protocol");
        assert_eq!(registry.protocol, protocol);
    }

    #[test]
    fn test_protocol_count() {
        let env = setup_env();
        let protocol1 = Address::generate(&env);
        let protocol2 = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol1,
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register first protocol");

        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol2,
            Symbol::new(&env, "compound"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register second protocol");

        let count = DeFiAuditingContract::total_protocol_count(env);
        assert_eq!(count, 2u32);
    }

    // ==================== TVL TRACKING TESTS ====================

    #[test]
    fn test_update_pool_tvl() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let pool_id = BytesN::<32>::from_array(&[1u8; 32]);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "uniswap"),
            ProtocolType::AMM,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let result = DeFiAuditingContract::update_pool_tvl(
            env.clone(),
            pool_id,
            protocol.clone(),
            Symbol::new(&env, "USDC-ETH"),
            1_000_000_000u128,
            500_000u128,
            100u32,
        );

        assert!(result.is_ok());

        // Verify pool TVL was stored
        let pool_tvl = DeFiAuditingContract::get_pool_tvl(env, pool_id)
            .expect("Failed to get pool TVL");
        assert_eq!(pool_tvl.tvl_usd, 1_000_000_000u128);
    }

    #[test]
    fn test_protocol_tvl_aggregation() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "uniswap"),
            ProtocolType::AMM,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        DeFiAuditingContract::update_pool_tvl(
            env.clone(),
            BytesN::<32>::from_array(&[1u8; 32]),
            protocol.clone(),
            Symbol::new(&env, "pool-1"),
            500_000_000u128,
            250_000u128,
            50u32,
        ).expect("Failed to update pool 1");

        let tvl = DeFiAuditingContract::get_protocol_tvl(env, protocol)
            .expect("Failed to get protocol TVL");
        assert_eq!(tvl, 500_000_000u128);
    }

    // ==================== ORACLE VERIFICATION TESTS ====================

    #[test]
    fn test_record_oracle_price() {
        let env = setup_env();
        let oracle_id = BytesN::<32>::from_array(&[2u8; 32]);
        let asset = Address::generate(&env);
        
        let result = DeFiAuditingContract::record_oracle_price(
            env.clone(),
            oracle_id,
            asset.clone(),
            2000u128, // $2000 per unit
            Symbol::new(&env, "chainlink"),
            100u32, // 1% confidence
            3600u64, // hourly updates
        );

        assert!(result.is_ok());

        // Verify oracle price was stored
        let oracle_price = DeFiAuditingContract::get_oracle_price(env, oracle_id)
            .expect("Failed to get oracle price");
        assert_eq!(oracle_price.price_usd, 2000u128);
    }

    #[test]
    fn test_price_anomaly_detection() {
        let env = setup_env();
        let asset = Address::generate(&env);
        
        // Record initial price
        let oracle_id1 = BytesN::<32>::from_array(&[2u8; 32]);
        DeFiAuditingContract::record_oracle_price(
            env.clone(),
            oracle_id1,
            asset.clone(),
            100u128,
            Symbol::new(&env, "chainlink"),
            100u32,
            3600u64,
        ).expect("Failed to record first price");

        // Record price with significant change (should be anomaly if > 5%)
        let oracle_id2 = BytesN::<32>::from_array(&[3u8; 32]);
        let result = DeFiAuditingContract::record_oracle_price(
            env.clone(),
            oracle_id2,
            asset.clone(),
            110u128, // 10% increase
            Symbol::new(&env, "chainlink"),
            100u32,
            3600u64,
        );

        assert!(result.is_ok());

        // Verify anomaly detection
        let is_anomaly = DeFiAuditingContract::verify_price_anomaly(
            env,
            asset,
            150u128, // 50% increase from current
            500u32, // 5% threshold
        ).expect("Failed to verify price anomaly");
        
        assert!(is_anomaly);
    }

    // ==================== LIQUIDATION MONITORING TESTS ====================

    #[test]
    fn test_record_liquidation() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let position = Address::generate(&env);
        let liquidator = Address::generate(&env);
        let collateral = Address::generate(&env);
        let debt_asset = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let result = DeFiAuditingContract::record_liquidation(
            env.clone(),
            protocol.clone(),
            position.clone(),
            liquidator,
            collateral,
            debt_asset,
            50_000_000u128,
            100_000_000u128,
            2000u128,
        );

        assert!(result.is_ok());

        // Verify liquidation was recorded
        let count = DeFiAuditingContract::get_protocol_liquidations(env, protocol)
            .expect("Failed to get liquidation count");
        assert_eq!(count, 1u32);
    }

    #[test]
    fn test_at_risk_position_tracking() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let owner = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let result = DeFiAuditingContract::add_at_risk_position(
            env.clone(),
            protocol.clone(),
            owner,
            100_000_000u128, // collateral
            50_000_000u128,  // debt
            10500u128,       // health factor < 1.1 (at risk)
        );

        assert!(result.is_ok());

        // Verify at-risk position count
        let count = DeFiAuditingContract::get_at_risk_positions(env, protocol)
            .expect("Failed to get at-risk count");
        assert_eq!(count, 1u32);
    }

    // ==================== GOVERNANCE TRACKING TESTS ====================

    #[test]
    fn test_create_governance_proposal() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let proposer = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let title = Bytes::from_slice(&env, b"Increase Reserve Factor");
        let description = Bytes::from_slice(&env, b"Proposal to increase reserve factor to 20%");

        let result = DeFiAuditingContract::create_proposal(
            env.clone(),
            protocol.clone(),
            title,
            description,
            proposer,
            env.ledger().timestamp(),
            env.ledger().timestamp() + 86400u64,
        );

        assert!(result.is_ok());

        // Verify proposal count
        let count = DeFiAuditingContract::protocol_proposal_count(env, protocol);
        assert_eq!(count, 1u32);
    }

    #[test]
    fn test_record_governance_votes() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let proposer = Address::generate(&env);
        let voter1 = Address::generate(&env);
        let voter2 = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let title = Bytes::from_slice(&env, b"Proposal");
        let description = Bytes::from_slice(&env, b"Test proposal");

        let proposal_id = DeFiAuditingContract::create_proposal(
            env.clone(),
            protocol.clone(),
            title,
            description,
            proposer,
            env.ledger().timestamp(),
            env.ledger().timestamp() + 86400u64,
        ).expect("Failed to create proposal");

        // Record votes
        DeFiAuditingContract::record_vote(
            env.clone(),
            proposal_id,
            voter1,
            1u32, // for
            1000_000u128,
        ).expect("Failed to record vote 1");

        DeFiAuditingContract::record_vote(
            env.clone(),
            proposal_id,
            voter2,
            0u32, // against
            500_000u128,
        ).expect("Failed to record vote 2");

        // Verify votes
        let (for_votes, against_votes, abstain_votes) = DeFiAuditingContract::get_proposal_votes(
            env,
            proposal_id,
        ).expect("Failed to get votes");

        assert_eq!(for_votes, 1000_000u128);
        assert_eq!(against_votes, 500_000u128);
        assert_eq!(abstain_votes, 0u128);
    }

    #[test]
    fn test_update_proposal_status() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let proposer = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let title = Bytes::from_slice(&env, b"Proposal");
        let description = Bytes::from_slice(&env, b"Test");

        let proposal_id = DeFiAuditingContract::create_proposal(
            env.clone(),
            protocol,
            title,
            description,
            proposer,
            env.ledger().timestamp(),
            env.ledger().timestamp() + 86400u64,
        ).expect("Failed to create proposal");

        // Update status to passed
        let result = DeFiAuditingContract::update_proposal_status(
            env.clone(),
            proposal_id,
            2u32, // passed
        );

        assert!(result.is_ok());

        // Verify status
        let proposal = DeFiAuditingContract::get_proposal(env, proposal_id)
            .expect("Failed to get proposal");
        assert_eq!(proposal.status, 2u32);
    }

    // ==================== RISK METRICS TESTS ====================

    #[test]
    fn test_calculate_risk_metrics() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let result = DeFiAuditingContract::calculate_risk_metrics(env.clone(), protocol.clone());

        assert!(result.is_ok());

        // Verify metrics were stored
        let metrics_id = result.unwrap();
        let metrics = DeFiAuditingContract::get_risk_metrics(env, metrics_id)
            .expect("Failed to get risk metrics");
        
        assert!(metrics.protocol_health <= 100u32);
    }

    #[test]
    fn test_protocol_health_score() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let health = DeFiAuditingContract::get_protocol_health_score(env, protocol)
            .expect("Failed to get health score");
        
        assert!(health <= 100u32);
    }

    // ==================== AUDIT REPORT TESTS ====================

    #[test]
    fn test_generate_audit_report() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let now = env.ledger().timestamp();
        let findings = BytesN::<32>::from_array(&[4u8; 32]);

        let result = DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64 * 30, // 30 days ago
            now,
            findings,
        );

        assert!(result.is_ok());

        // Verify report was stored
        let report_id = result.unwrap();
        let report = DeFiAuditingContract::get_audit_report(env, report_id)
            .expect("Failed to get audit report");
        
        assert_eq!(report.protocol, protocol);
    }

    #[test]
    fn test_get_latest_audit_report() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let now = env.ledger().timestamp();

        DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64 * 60,
            now - 86400u64 * 30,
            BytesN::<32>::from_array(&[5u8; 32]),
        ).expect("Failed to generate first report");

        let report_id = DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64 * 30,
            now,
            BytesN::<32>::from_array(&[6u8; 32]),
        ).expect("Failed to generate second report");

        let latest = DeFiAuditingContract::get_latest_audit_report(env, protocol)
            .expect("Failed to get latest report");
        
        assert_eq!(latest.report_id, report_id);
    }

    #[test]
    fn test_report_count() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register protocol");

        let now = env.ledger().timestamp();

        DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64 * 60,
            now - 86400u64 * 30,
            BytesN::<32>::from_array(&[5u8; 32]),
        ).expect("Failed to generate first report");

        DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64 * 30,
            now,
            BytesN::<32>::from_array(&[6u8; 32]),
        ).expect("Failed to generate second report");

        let count = DeFiAuditingContract::protocol_report_count(env, protocol);
        assert_eq!(count, 2u32);
    }

    // ==================== QUERY TESTS ====================

    #[test]
    fn test_full_workflow() {
        let env = setup_env();
        let protocol = Address::generate(&env);
        let pool_id = BytesN::<32>::from_array(&[7u8; 32]);
        let asset = Address::generate(&env);
        let oracle_id = BytesN::<32>::from_array(&[8u8; 32]);
        let owner = Address::generate(&env);
        
        // 1. Register protocol
        DeFiAuditingContract::register_protocol(
            env.clone(),
            protocol.clone(),
            Symbol::new(&env, "aave"),
            ProtocolType::Lending,
            Symbol::new(&env, "ethereum"),
            None,
        ).expect("Failed to register");

        // 2. Update pool TVL
        DeFiAuditingContract::update_pool_tvl(
            env.clone(),
            pool_id,
            protocol.clone(),
            Symbol::new(&env, "DAI"),
            5_000_000_000u128,
            1_000_000_000u128,
            500u32,
        ).expect("Failed to update TVL");

        // 3. Record oracle price
        DeFiAuditingContract::record_oracle_price(
            env.clone(),
            oracle_id,
            asset,
            1u128,
            Symbol::new(&env, "chainlink"),
            100u32,
            3600u64,
        ).expect("Failed to record price");

        // 4. Add at-risk position
        DeFiAuditingContract::add_at_risk_position(
            env.clone(),
            protocol.clone(),
            owner,
            200_000_000u128,
            100_000_000u128,
            15000u128,
        ).expect("Failed to add position");

        // 5. Generate audit report
        let now = env.ledger().timestamp();
        DeFiAuditingContract::generate_audit_report(
            env.clone(),
            protocol.clone(),
            now - 86400u64,
            now,
            BytesN::<32>::from_array(&[9u8; 32]),
        ).expect("Failed to generate report");

        // 6. Verify queries
        assert_eq!(DeFiAuditingContract::total_protocol_count(env.clone()), 1u32);
        assert_eq!(DeFiAuditingContract::protocol_pool_count(env.clone(), protocol.clone()), 1u32);
        assert_eq!(DeFiAuditingContract::protocol_at_risk_count(env.clone(), protocol.clone()), 1u32);
        assert_eq!(DeFiAuditingContract::protocol_report_count(env, protocol), 1u32);
    }
}
