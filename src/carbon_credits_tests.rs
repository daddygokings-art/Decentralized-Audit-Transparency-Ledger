#[cfg(test)]
mod tests {
    use crate::carbon_credits::*;
    use soroban_sdk::{bytes, vec, Address, BytesN, Env, Symbol};

    /// Helper to create renewable energy source
    fn create_test_renewable(env: &Env, energy_mwh: u32) -> RenewableEnergySource {
        RenewableEnergySource {
            source_type: RenewableEnergyType::Solar,
            facility_id: bytes!(env, b"SOL-001"),
            location: bytes!(env, b"California"),
            capacity_mw: 50,
            energy_generated_mwh: energy_mwh,
            verification_date: env.ledger().timestamp(),
            certifications: vec![env],
        }
    }

    /// Helper to create offset
    fn create_test_offset(env: &Env) -> Offset {
        let now = env.ledger().timestamp();
        Offset {
            offset_type: Symbol::new(env, "reforestation"),
            project_id: bytes!(env, b"REFOR-001"),
            tonnes_co2e: 100,
            project_location: bytes!(env, b"Brazil"),
            verification_body: Address::random(env),
            verification_date: now,
            expiration_date: now + (10 * 365 * 86400),
        }
    }

    /// Helper to create registry entry
    fn create_test_registry(env: &Env, verifier: Address) -> RegistryEntry {
        RegistryEntry {
            registry_id: bytes!(env, b"REG-001"),
            registry_name: bytes!(env, b"Verified Carbon Standard"),
            registry_url: bytes!(env, b"https://vcs.org"),
            issuance_date: env.ledger().timestamp(),
            verified_by: verifier,
            compliance_standard: ComplianceStandard::Vcs,
        }
    }

    /// Test 1: Issue carbon credit
    #[test]
    fn test_issue_carbon_credit() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer.clone(),
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        assert!(!credit_id.to_vec().is_empty());

        let credit = get_credit_details(&env, credit_id);
        assert_eq!(credit.carbon_tonnes, 100);
        assert_eq!(credit.status, CreditStatus::Issued);
    }

    /// Test 2: Verify renewable energy
    #[test]
    fn test_verify_renewable_energy() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier.clone()),
            ComplianceStandard::Vcs,
        );

        let verified = verify_renewable_energy(&env, credit_id.clone(), verifier, 200);
        assert!(verified);

        let credit = get_credit_details(&env, credit_id);
        assert_eq!(credit.status, CreditStatus::Active);
    }

    /// Test 3: Calculate offset
    #[test]
    fn test_calculate_offset() {
        let env = Env::default();

        let offset = calculate_offset(&env, 1000);
        assert_eq!(offset, 500); // 1000 MWh * 0.5 = 500 tonnes
    }

    /// Test 4: Tokenize credit
    #[test]
    fn test_tokenize_credit() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let owner = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let token_id = tokenize_credit(&env, credit_id.clone(), owner, 1000, 15);
        assert!(!token_id.to_vec().is_empty());

        let credit = get_credit_details(&env, credit_id);
        assert!(credit.tokenization.is_some());
    }

    /// Test 5: Retire credit
    #[test]
    fn test_retire_credit() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let retired = retire_credit(&env, credit_id.clone(), bytes!(env, b"Used for sustainability"));
        assert!(retired);

        let status = get_credit_status(&env, credit_id);
        assert_eq!(status, CreditStatus::Retired);
    }

    /// Test 6: Check retirement status
    #[test]
    fn test_check_retirement_status() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        retire_credit(&env, credit_id.clone(), bytes!(env, b"Retirement"));

        let is_retired = check_retirement_status(&env, credit_id);
        assert!(is_retired);
    }

    /// Test 7: Transfer credit
    #[test]
    fn test_transfer_credit() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let new_holder = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer.clone(),
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let transferred = transfer_credit(&env, credit_id.clone(), issuer, new_holder.clone());
        assert!(transferred);

        let holder_credits = get_holder_credits(&env, new_holder);
        assert_eq!(holder_credits.len(), 1);
    }

    /// Test 8: Verify sustainability claim
    #[test]
    fn test_verify_sustainability_claim() {
        let env = Env::default();
        env.mock_all_auths();

        let verifier = Address::random(&env);

        let mut evidence = vec![&env];
        evidence.push_back(bytes!(env, b"https://evidence.org/report1"));

        let claim = SustainabilityClaim {
            claim_id: bytes!(env, b"CLAIM-001"),
            claimant: Address::random(&env),
            claim_type: Symbol::new(&env, "carbon_neutral"),
            claim_description: bytes!(env, b"Achieved carbon neutrality through offsets"),
            claimed_reduction: 5000,
            supporting_evidence: evidence,
            claim_date: env.ledger().timestamp(),
        };

        let verified = verify_sustainability_claim(&env, claim, verifier);
        assert!(verified);
    }

    /// Test 9: Audit renewable usage
    #[test]
    fn test_audit_renewable_usage() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let auditor = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let record = audit_renewable_usage(&env, credit_id, auditor, 200);
        assert!(record.approved);
    }

    /// Test 10: Verify offset authenticity
    #[test]
    fn test_verify_offset_authenticity() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier_addr = Address::random(&env);

        let mut offset = create_test_offset(&env);
        offset.verification_body = verifier_addr.clone();

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            offset,
            create_test_registry(&env, verifier_addr.clone()),
            ComplianceStandard::Vcs,
        );

        let verified = verify_offset_authenticity(&env, credit_id, verifier_addr);
        assert!(verified);
    }

    /// Test 11: Register credit
    #[test]
    fn test_register_credit() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let registered = register_credit(&env, credit_id, bytes!(env, b"REG-001"));
        assert!(registered);
    }

    /// Test 12: Update registry
    #[test]
    fn test_update_registry() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier1 = Address::random(&env);
        let verifier2 = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier1),
            ComplianceStandard::Vcs,
        );

        register_credit(&env, credit_id, bytes!(env, b"REG-001"));
        let updated = update_registry(&env, bytes!(env, b"REG-001"), verifier2);
        assert!(updated);
    }

    /// Test 13: Link to standard
    #[test]
    fn test_link_to_standard() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let linked = link_to_standard(&env, credit_id, ComplianceStandard::Gold);
        assert!(linked);
    }

    /// Test 14: Verify registry compliance
    #[test]
    fn test_verify_registry_compliance() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        verify_renewable_energy(&env, credit_id.clone(), verifier, 200);
        register_credit(&env, credit_id.clone(), bytes!(env, b"REG-001"));

        let compliant = verify_registry_compliance(&env, credit_id);
        assert!(compliant);
    }

    /// Test 15: Calculate carbon reduction
    #[test]
    fn test_calculate_carbon_reduction() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            250,
            create_test_renewable(&env, 500),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let reduction = calculate_carbon_reduction(&env, credit_id);
        assert_eq!(reduction, 250);
    }

    /// Test 16: Generate offset report
    #[test]
    fn test_generate_offset_report() {
        let env = Env::default();

        let now = env.ledger().timestamp();
        let report = generate_offset_report(&env, now - 86400, now);

        assert!(report.reporting_period.0 < report.reporting_period.1);
    }

    /// Test 17: Get portfolio status
    #[test]
    fn test_get_portfolio_status() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        issue_carbon_credit(
            &env,
            issuer.clone(),
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier.clone()),
            ComplianceStandard::Vcs,
        );

        issue_carbon_credit(
            &env,
            issuer.clone(),
            50,
            create_test_renewable(&env, 100),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let portfolio = get_portfolio_status(&env, issuer);
        assert_eq!(portfolio.total_credits, 2);
    }

    /// Test 18: Validate claim
    #[test]
    fn test_validate_claim() {
        let env = Env::default();

        let mut evidence = vec![&env];
        evidence.push_back(bytes!(env, b"https://evidence.org"));

        let valid_claim = SustainabilityClaim {
            claim_id: bytes!(env, b"CLAIM-001"),
            claimant: Address::random(&env),
            claim_type: Symbol::new(&env, "carbon_neutral"),
            claim_description: bytes!(env, b"Carbon neutral operations"),
            claimed_reduction: 5000,
            supporting_evidence: evidence,
            claim_date: env.ledger().timestamp(),
        };

        let is_valid = validate_claim(&env, valid_claim);
        assert!(is_valid);
    }

    /// Test 19: Verify standard compliance
    #[test]
    fn test_verify_standard_compliance() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let compliant = verify_standard_compliance(&env, credit_id, ComplianceStandard::Vcs);
        assert!(compliant);
    }

    /// Test 20: Check data integrity
    #[test]
    fn test_check_data_integrity() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let valid = check_data_integrity(&env, credit_id);
        assert!(valid);
    }

    /// Test 21: Get total retired CO2e
    #[test]
    fn test_get_total_retired_co2e() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        retire_credit(&env, credit_id, bytes!(env, b"Retirement"));

        let total = get_total_retired_co2e(&env);
        assert_eq!(total, 100);
    }

    /// Test 22: Get credit status
    #[test]
    fn test_get_credit_status() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let status = get_credit_status(&env, credit_id);
        assert_eq!(status, CreditStatus::Issued);
    }

    /// Test 23: Get issuer credits
    #[test]
    fn test_get_issuer_credits() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        issue_carbon_credit(
            &env,
            issuer.clone(),
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier.clone()),
            ComplianceStandard::Vcs,
        );

        issue_carbon_credit(
            &env,
            issuer.clone(),
            50,
            create_test_renewable(&env, 100),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        let credits = get_issuer_credits(&env, issuer);
        assert_eq!(credits.len(), 2);
    }

    /// Test 24: Get total credits issued
    #[test]
    fn test_get_total_credits_issued() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        for _ in 0..3 {
            issue_carbon_credit(
                &env,
                issuer.clone(),
                100,
                create_test_renewable(&env, 200),
                create_test_offset(&env),
                create_test_registry(&env, verifier.clone()),
                ComplianceStandard::Vcs,
            );
        }

        let total = get_total_credits_issued(&env);
        assert_eq!(total, 3);
    }

    /// Test 25: Full carbon credit lifecycle
    #[test]
    fn test_full_carbon_credit_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let new_holder = Address::random(&env);
        let verifier = Address::random(&env);

        // 1. Issue credit
        let credit_id = issue_carbon_credit(
            &env,
            issuer.clone(),
            500,
            create_test_renewable(&env, 1000),
            create_test_offset(&env),
            create_test_registry(&env, verifier.clone()),
            ComplianceStandard::Vcs,
        );

        // 2. Verify renewable energy
        verify_renewable_energy(&env, credit_id.clone(), verifier.clone(), 1000);

        // 3. Register in registry
        register_credit(&env, credit_id.clone(), bytes!(env, b"REG-VCS-001"));

        // 4. Tokenize
        let token_id = tokenize_credit(&env, credit_id.clone(), issuer.clone(), 5000, 15);
        assert!(!token_id.to_vec().is_empty());

        // 5. Transfer to new holder
        transfer_credit(&env, credit_id.clone(), issuer, new_holder.clone());

        // 6. Get portfolio
        let portfolio = get_portfolio_status(&env, new_holder);
        assert_eq!(portfolio.total_credits, 1);

        // 7. Retire
        retire_credit(&env, credit_id.clone(), bytes!(env, b"Used for offset"));

        // 8. Verify retired
        let is_retired = check_retirement_status(&env, credit_id.clone());
        assert!(is_retired);

        // 9. Check total retired CO2e
        let total_retired = get_total_retired_co2e(&env);
        assert_eq!(total_retired, 500);
    }

    /// Test 26: Multiple renewable types
    #[test]
    fn test_multiple_renewable_types() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        // Solar credit
        let solar_source = RenewableEnergySource {
            source_type: RenewableEnergyType::Solar,
            facility_id: bytes!(env, b"SOL-001"),
            location: bytes!(env, b"California"),
            capacity_mw: 50,
            energy_generated_mwh: 1000,
            verification_date: env.ledger().timestamp(),
            certifications: vec![env],
        };

        issue_carbon_credit(
            &env,
            issuer.clone(),
            500,
            solar_source,
            create_test_offset(&env),
            create_test_registry(&env, verifier.clone()),
            ComplianceStandard::Vcs,
        );

        // Wind credit
        let wind_source = RenewableEnergySource {
            source_type: RenewableEnergyType::Wind,
            facility_id: bytes!(env, b"WIND-001"),
            location: bytes!(env, b"Texas"),
            capacity_mw: 100,
            energy_generated_mwh: 2000,
            verification_date: env.ledger().timestamp(),
            certifications: vec![env],
        };

        issue_carbon_credit(
            &env,
            issuer.clone(),
            1000,
            wind_source,
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Gold,
        );

        let issuer_credits = get_issuer_credits(&env, issuer);
        assert_eq!(issuer_credits.len(), 2);
    }

    /// Test 27: Invalid claim validation
    #[test]
    fn test_invalid_claim_validation() {
        let env = Env::default();

        let claim = SustainabilityClaim {
            claim_id: bytes!(env, b"INVALID-CLAIM"),
            claimant: Address::random(&env),
            claim_type: Symbol::new(&env, "carbon_neutral"),
            claim_description: bytes!(env, b""),  // Empty description
            claimed_reduction: 5000,
            supporting_evidence: vec![env],  // No evidence
            claim_date: env.ledger().timestamp(),
        };

        let is_valid = validate_claim(&env, claim);
        assert!(!is_valid);
    }

    /// Test 28: Large-scale credit issuance
    #[test]
    fn test_large_scale_credit_issuance() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        // Issue 10 credits
        for i in 0..10 {
            issue_carbon_credit(
                &env,
                issuer.clone(),
                (i + 1) * 100,
                create_test_renewable(&env, (i + 1) * 200),
                create_test_offset(&env),
                create_test_registry(&env, verifier.clone()),
                ComplianceStandard::Vcs,
            );
        }

        let total = get_total_credits_issued(&env);
        assert_eq!(total, 10);

        let portfolio = get_portfolio_status(&env, issuer);
        assert_eq!(portfolio.total_credits, 10);
    }

    /// Test 29: Credit transfer and retirement
    #[test]
    fn test_credit_transfer_and_retirement() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let holder1 = Address::random(&env);
        let holder2 = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer.clone(),
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        // Transfer issuer -> holder1
        transfer_credit(&env, credit_id.clone(), issuer, holder1.clone());
        let h1_credits = get_holder_credits(&env, holder1.clone());
        assert_eq!(h1_credits.len(), 1);

        // Try to retire from holder1
        retire_credit(&env, credit_id, bytes!(env, b"Retirement"));

        let portfolio = get_portfolio_status(&env, holder1);
        assert_eq!(portfolio.retired_credits, 1);
    }

    /// Test 30: Multi-standard compliance
    #[test]
    fn test_multi_standard_compliance() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let verifier = Address::random(&env);

        let credit_id = issue_carbon_credit(
            &env,
            issuer,
            100,
            create_test_renewable(&env, 200),
            create_test_offset(&env),
            create_test_registry(&env, verifier),
            ComplianceStandard::Vcs,
        );

        // Check initial standard
        let vcs_compliant = verify_standard_compliance(&env, credit_id.clone(), ComplianceStandard::Vcs);
        assert!(vcs_compliant);

        // Link to different standard
        link_to_standard(&env, credit_id.clone(), ComplianceStandard::Gold);

        // Check new standard
        let gold_compliant = verify_standard_compliance(&env, credit_id, ComplianceStandard::Gold);
        assert!(gold_compliant);
    }
}
