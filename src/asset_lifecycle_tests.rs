//! Tokenized asset lifecycle tests

#[cfg(test)]
mod asset_lifecycle_tests {
    use crate::asset_lifecycle::*;
    use crate::asset_lifecycle_impl::*;
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env};

    fn setup() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_issue_asset() {
        let env = setup();
        let issuer = Address::generate(&env);

        let asset_id = AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer,
            Bytes::from_slice(&env, b"Corporate Bond"),
            Bytes::from_slice(&env, b"BOND"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32, // 5% coupon
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue");

        assert_ne!(asset_id.to_array(), [0u8; 32]);

        let asset = AssetLifecycleContract::get_asset(env, asset_id)
            .expect("Failed to get asset");
        assert_eq!(asset.coupon_rate_bp, 500u32);
    }

    #[test]
    fn test_register_investor() {
        let env = setup();
        let investor = Address::generate(&env);

        AssetLifecycleContract::register_investor(
            env.clone(),
            investor.clone(),
            Bytes::from_slice(&env, b"John Doe"),
        ).expect("Failed to register");

        let profile = AssetLifecycleContract::get_investor(env, investor)
            .expect("Failed to get investor");
        assert!(!profile.kyc_verified);
    }

    #[test]
    fn test_verify_kyc() {
        let env = setup();
        let investor = Address::generate(&env);

        AssetLifecycleContract::register_investor(
            env.clone(),
            investor.clone(),
            Bytes::from_slice(&env, b"John Doe"),
        ).expect("Failed to register");

        AssetLifecycleContract::verify_investor_kyc(env.clone(), investor.clone())
            .expect("Failed to verify");

        let profile = AssetLifecycleContract::get_investor(env, investor)
            .expect("Failed to get investor");
        assert!(profile.kyc_verified);
    }

    #[test]
    fn test_compliance_check() {
        let env = setup();
        let issuer = Address::generate(&env);
        let investor = Address::generate(&env);

        let asset_id = AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer,
            Bytes::from_slice(&env, b"Bond"),
            Bytes::from_slice(&env, b"BND"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue");

        AssetLifecycleContract::register_investor(
            env.clone(),
            investor.clone(),
            Bytes::from_slice(&env, b"Investor"),
        ).expect("Failed to register");

        AssetLifecycleContract::verify_investor_kyc(env.clone(), investor.clone())
            .expect("Failed to verify KYC");

        let compliant = AssetLifecycleContract::check_compliance(
            env,
            asset_id,
            investor,
            1000u128,
        ).expect("Failed to check compliance");

        assert!(compliant);
    }

    #[test]
    fn test_add_compliance_rule() {
        let env = setup();
        let issuer = Address::generate(&env);

        let asset_id = AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer,
            Bytes::from_slice(&env, b"Bond"),
            Bytes::from_slice(&env, b"BND"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue");

        let rule_id = AssetLifecycleContract::add_compliance_rule(
            env,
            asset_id,
            ComplianceRuleType::Accredited,
            30u32,
            100u32,
            false,
            true,
        ).expect("Failed to add rule");

        assert_ne!(rule_id.to_array(), [0u8; 32]);
    }

    #[test]
    fn test_declare_corporate_action() {
        let env = setup();
        let issuer = Address::generate(&env);

        let asset_id = AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer,
            Bytes::from_slice(&env, b"Bond"),
            Bytes::from_slice(&env, b"BND"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue");

        let action_id = AssetLifecycleContract::declare_corporate_action(
            env.clone(),
            asset_id,
            0u32, // dividend
            env.ledger().timestamp() + 86400u64 * 30u64,
            env.ledger().timestamp() + 86400u64 * 25u64,
            env.ledger().timestamp() + 86400u64 * 35u64,
            50u128,
        ).expect("Failed to declare");

        assert_ne!(action_id.to_array(), [0u8; 32]);

        let action = AssetLifecycleContract::get_corporate_action(env, action_id)
            .expect("Failed to get action");
        assert_eq!(action.dividend_amount, 50u128);
    }

    #[test]
    fn test_request_redemption() {
        let env = setup();
        let issuer = Address::generate(&env);
        let investor = Address::generate(&env);

        let asset_id = AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer,
            Bytes::from_slice(&env, b"Bond"),
            Bytes::from_slice(&env, b"BND"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue");

        // This would need balance setup in full implementation
        let result = AssetLifecycleContract::request_redemption(
            env,
            asset_id,
            investor,
            100u128,
        );

        // Will fail due to insufficient balance in simplified test
        assert!(result.is_err());
    }

    #[test]
    fn test_asset_counts() {
        let env = setup();
        let issuer1 = Address::generate(&env);
        let issuer2 = Address::generate(&env);

        AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer1,
            Bytes::from_slice(&env, b"Bond1"),
            Bytes::from_slice(&env, b"BD1"),
            1_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            1000u128,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to issue 1");

        AssetLifecycleContract::issue_asset(
            env.clone(),
            issuer2,
            Bytes::from_slice(&env, b"Bond2"),
            Bytes::from_slice(&env, b"BD2"),
            2_000_000u128,
            18u32,
            env.ledger().timestamp() + 86400u64 * 365u64,
            500u32,
            2000u128,
            BytesN::<32>::from_array(&[2u8; 32]),
        ).expect("Failed to issue 2");

        let count = AssetLifecycleContract::total_asset_count(env);
        assert_eq!(count, 2u32);
    }
}
