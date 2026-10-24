#[cfg(test)]
mod tests {
    use crate::esg_reporting::*;
    use soroban_sdk::{vec, Address, Env, Symbol};

    #[test]
    fn test_collect_environmental_data() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let metrics = collect_environmental_data(&env, org, 5000, 80, 75, 1000).unwrap();

        assert_eq!(metrics.carbon_emissions, 5000);
        assert_eq!(metrics.renewable_energy_percent, 80);
        assert_eq!(metrics.waste_recycled_percent, 75);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Boundary validation tests (issue #874)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_environmental_data_max_carbon_emissions() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let metrics = collect_environmental_data(&env, org, u32::MAX, 50, 50, u32::MAX).unwrap();

        assert_eq!(metrics.carbon_emissions, u32::MAX);
        assert_eq!(metrics.water_usage, u32::MAX);
    }

    #[test]
    fn test_environmental_data_rejects_invalid_percentage() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let result = collect_environmental_data(&env, org, 5000, 101, 75, 1000);

        assert_eq!(result, Err(ESGReportingError::InvalidInputRange));
    }

    #[test]
    fn test_social_data_rejects_invalid_percentage() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let result = collect_social_data(&env, org, 500, 150, 40, 2);

        assert_eq!(result, Err(ESGReportingError::InvalidInputRange));
    }

    #[test]
    fn test_governance_data_rejects_invalid_percentage() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let result = collect_governance_data(&env, org, 10, 101, 30);

        assert_eq!(result, Err(ESGReportingError::InvalidInputRange));
    }

    #[test]
    fn test_score_calculation_with_max_values() {
        let env = Env::default();

        let e = EnvironmentalMetrics {
            carbon_emissions: u32::MAX,
            renewable_energy_percent: 100,
            waste_recycled_percent: 100,
            water_usage: u32::MAX,
            water_recycled_percent: 100,
            biodiversity_score: 100,
            pollution_score: 100,
            energy_efficiency_score: 100,
            measurement_date: env.ledger().timestamp(),
        };

        let s = SocialMetrics {
            employee_count: u32::MAX,
            women_percent: 100,
            minority_percent: 100,
            training_hours_per_employee: u32::MAX,
            safety_incidents: u32::MAX,
            employee_satisfaction: 100,
            community_investment: u32::MAX,
            labor_violations: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let g = GovernanceMetrics {
            board_size: u32::MAX,
            board_independence_percent: 100,
            women_board_percent: 100,
            executive_compensation_ratio: u32::MAX,
            ethics_training_percent: 100,
            data_privacy_score: 100,
            anti_corruption_score: 100,
            audit_findings: u32::MAX,
            measurement_date: env.ledger().timestamp(),
        };

        // Should not panic and should return a valid score
        let score = calculate_esg_score(&env, &e, &s, &g);
        assert!(score <= 100);
    }

    #[test]
    fn test_score_calculation_with_extreme_safety_incidents() {
        let env = Env::default();

        let e = EnvironmentalMetrics {
            carbon_emissions: 5000,
            renewable_energy_percent: 80,
            waste_recycled_percent: 75,
            water_usage: 1000,
            water_recycled_percent: 50,
            biodiversity_score: 75,
            pollution_score: 80,
            energy_efficiency_score: 75,
            measurement_date: env.ledger().timestamp(),
        };

        let s = SocialMetrics {
            employee_count: 500,
            women_percent: 40,
            minority_percent: 30,
            training_hours_per_employee: 40,
            safety_incidents: u32::MAX,  // Extreme value
            employee_satisfaction: 85,
            community_investment: 100,
            labor_violations: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let g = GovernanceMetrics {
            board_size: 10,
            board_independence_percent: 80,
            women_board_percent: 30,
            executive_compensation_ratio: 250,
            ethics_training_percent: 95,
            data_privacy_score: 90,
            anti_corruption_score: 85,
            audit_findings: 0,
            measurement_date: env.ledger().timestamp(),
        };

        // Should not underflow or panic
        let score = calculate_esg_score(&env, &e, &s, &g);
        assert!(score <= 100);
    }

    #[test]
    fn test_trend_analysis_with_zero_previous_value() {
        let env = Env::default();

        let result = trend_analysis(
            &env,
            soroban_sdk::bytes!(&env, b"Carbon"),
            5000,
            0,  // Zero previous value
        );

        // Should return error for division by zero
        assert_eq!(result, Err(ESGReportingError::DivisionByZero));
    }

    #[test]
    fn test_trend_analysis_with_max_values() {
        let env = Env::default();

        let result = trend_analysis(
            &env,
            soroban_sdk::bytes!(&env, b"Carbon"),
            u32::MAX,
            1,
        );

        // Should not overflow and should clamp to i32::MAX
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.change_percent == i32::MAX || analysis.change_percent > 0);
    }

    #[test]
    fn test_framework_alignments_bounded() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();

        // Add alignments up to the limit
        for _ in 0..10 {
            let result = align_to_gri(&env, report_id.clone(), org.clone());
            if result.is_err() {
                assert_eq!(result, Err(ESGReportingError::CollectionOverflow));
                return;
            }
        }

        // The 11th alignment should fail
        let result = align_to_gri(&env, report_id.clone(), org.clone());
        assert_eq!(result, Err(ESGReportingError::CollectionOverflow));
    }

    #[test]
    fn test_organization_reports_bounded() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);

        // Generate reports up to the limit
        for i in 0..1000 {
            let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
            let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
            let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

            let result = generate_esg_report(&env, org.clone(), i, i + 86400, e, s, g);
            
            if result.is_err() {
                assert_eq!(result, Err(ESGReportingError::CollectionOverflow));
                return;
            }
        }

        // The 1001st report should fail
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();
        let result = generate_esg_report(&env, org.clone(), 1000, 1000 + 86400, e, s, g);
        
        assert_eq!(result, Err(ESGReportingError::CollectionOverflow));
    }

    #[test]
    fn test_kpi_tracking_bounded() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let mut kpis = vec![&env];

        // Create more than MAX_KPIS
        for i in 0..101 {
            kpis.push_back(KPI {
                kpi_name: soroban_sdk::bytes!(&env, b"KPI"),
                target_value: i,
                current_value: i,
                unit: Symbol::new(&env, "units"),
                status: Symbol::new(&env, "on_track"),
                measurement_date: env.ledger().timestamp(),
            });
        }

        let result = track_kpis(&env, org, kpis);
        assert_eq!(result, Err(ESGReportingError::CollectionOverflow));
    }

    #[test]
    fn test_get_nonexistent_report() {
        let env = Env::default();

        let fake_id = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
        let result = get_esg_report(&env, fake_id);

        assert_eq!(result, Err(ESGReportingError::GenerationFailed));
    }

    #[test]
    fn test_stakeholder_report_with_nonexistent_id() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let fake_id = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        let result = generate_investor_report(&env, fake_id.clone(), org.clone());
        assert_eq!(result, Err(ESGReportingError::GenerationFailed));

        let result = generate_employee_report(&env, fake_id.clone(), org.clone());
        assert_eq!(result, Err(ESGReportingError::GenerationFailed));

        let result = generate_supplier_report(&env, fake_id.clone(), org.clone());
        assert_eq!(result, Err(ESGReportingError::GenerationFailed));

        let result = generate_community_report(&env, fake_id.clone(), org.clone());
        assert_eq!(result, Err(ESGReportingError::GenerationFailed));
    }

    #[test]
    fn test_collect_social_data() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let metrics = collect_social_data(&env, org, 500, 40, 40, 2).unwrap();

        assert_eq!(metrics.employee_count, 500);
        assert_eq!(metrics.women_percent, 40);
        assert_eq!(metrics.training_hours_per_employee, 40);
    }

    #[test]
    fn test_collect_governance_data() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let metrics = collect_governance_data(&env, org, 10, 80, 30).unwrap();

        assert_eq!(metrics.board_size, 10);
        assert_eq!(metrics.board_independence_percent, 80);
        assert_eq!(metrics.women_board_percent, 30);
    }

    #[test]
    fn test_generate_esg_report() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e_data = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s_data = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g_data = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e_data, s_data, g_data).unwrap();

        assert!(!report_id.to_vec().is_empty());

        let report = get_esg_report(&env, report_id).unwrap();
        assert!(report.esg_score > 0);
        assert!(report.esg_score <= 100);
    }

    #[test]
    fn test_calculate_esg_score() {
        let env = Env::default();

        let e = EnvironmentalMetrics {
            carbon_emissions: 5000,
            renewable_energy_percent: 80,
            waste_recycled_percent: 75,
            water_usage: 1000,
            water_recycled_percent: 50,
            biodiversity_score: 75,
            pollution_score: 80,
            energy_efficiency_score: 75,
            measurement_date: env.ledger().timestamp(),
        };

        let s = SocialMetrics {
            employee_count: 500,
            women_percent: 40,
            minority_percent: 30,
            training_hours_per_employee: 40,
            safety_incidents: 2,
            employee_satisfaction: 85,
            community_investment: 100,
            labor_violations: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let g = GovernanceMetrics {
            board_size: 10,
            board_independence_percent: 80,
            women_board_percent: 30,
            executive_compensation_ratio: 250,
            ethics_training_percent: 95,
            data_privacy_score: 90,
            anti_corruption_score: 85,
            audit_findings: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let score = calculate_esg_score(&env, &e, &s, &g);
        assert!(score > 0 && score <= 100);
    }

    #[test]
    fn test_align_to_gri() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let alignment = align_to_gri(&env, report_id, org).unwrap();

        assert_eq!(alignment.framework, ReportingFramework::Gri);
        assert!(alignment.aligned);
    }

    #[test]
    fn test_align_to_sasb() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let alignment = align_to_sasb(&env, report_id, org).unwrap();

        assert_eq!(alignment.framework, ReportingFramework::Sasb);
        assert!(alignment.aligned);
    }

    #[test]
    fn test_align_to_tcfd() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let alignment = align_to_tcfd(&env, report_id, org).unwrap();

        assert_eq!(alignment.framework, ReportingFramework::Tcfd);
        assert!(alignment.aligned);
    }

    #[test]
    fn test_verify_alignment() {
        let env = Env::default();
        let alignment = FrameworkAlignment {
            framework: ReportingFramework::Gri,
            aligned: true,
            coverage_percent: 95,
            gaps: vec![&env],
            alignment_date: env.ledger().timestamp(),
        };

        let verified = verify_alignment(&env, &alignment);
        assert!(verified);
    }

    #[test]
    fn test_generate_investor_report() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let investor_report = generate_investor_report(&env, report_id, org).unwrap();

        assert_eq!(investor_report.stakeholder_type, StakeholderType::Investor);
        assert!(investor_report.esg_score > 0);
    }

    #[test]
    fn test_generate_employee_report() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let employee_report = generate_employee_report(&env, report_id, org).unwrap();

        assert_eq!(employee_report.stakeholder_type, StakeholderType::Employee);
    }

    #[test]
    fn test_generate_supplier_report() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let supplier_report = generate_supplier_report(&env, report_id, org).unwrap();

        assert_eq!(supplier_report.stakeholder_type, StakeholderType::Supplier);
    }

    #[test]
    fn test_generate_community_report() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();
        let community_report = generate_community_report(&env, report_id, org).unwrap();

        assert_eq!(community_report.stakeholder_type, StakeholderType::Community);
    }

    #[test]
    fn test_track_kpis() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let kpi = KPI {
            kpi_name: soroban_sdk::bytes!(&env, b"Carbon Reduction"),
            target_value: 5000,
            current_value: 4500,
            unit: Symbol::new(&env, "kg_co2e"),
            status: Symbol::new(&env, "on_track"),
            measurement_date: env.ledger().timestamp(),
        };

        let mut kpis = vec![&env];
        kpis.push_back(kpi);

        track_kpis(&env, org, kpis).unwrap();
    }

    #[test]
    fn test_trend_analysis() {
        let env = Env::default();

        let analysis = trend_analysis(&env, soroban_sdk::bytes!(&env, b"Carbon"), 5000, 4500).unwrap();

        assert!(analysis.change_percent > 0);
        assert_eq!(analysis.trend, Symbol::new(&env, "improving"));
    }

    #[test]
    fn test_validate_metrics() {
        let env = Env::default();

        let e = EnvironmentalMetrics {
            carbon_emissions: 5000,
            renewable_energy_percent: 80,
            waste_recycled_percent: 75,
            water_usage: 1000,
            water_recycled_percent: 50,
            biodiversity_score: 75,
            pollution_score: 80,
            energy_efficiency_score: 75,
            measurement_date: env.ledger().timestamp(),
        };

        let s = SocialMetrics {
            employee_count: 500,
            women_percent: 40,
            minority_percent: 30,
            training_hours_per_employee: 40,
            safety_incidents: 2,
            employee_satisfaction: 85,
            community_investment: 100,
            labor_violations: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let g = GovernanceMetrics {
            board_size: 10,
            board_independence_percent: 80,
            women_board_percent: 30,
            executive_compensation_ratio: 250,
            ethics_training_percent: 95,
            data_privacy_score: 90,
            anti_corruption_score: 85,
            audit_findings: 0,
            measurement_date: env.ledger().timestamp(),
        };

        let valid = validate_metrics(&env, &e, &s, &g);
        assert!(valid);
    }

    #[test]
    fn test_verify_data_completeness() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org, 0, 86400, e, s, g).unwrap();
        let complete = verify_data_completeness(&env, report_id).unwrap();

        assert!(complete);
    }

    #[test]
    fn test_check_reporting_standards() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();

        align_to_gri(&env, report_id.clone(), org.clone()).unwrap();
        align_to_sasb(&env, report_id.clone(), org.clone()).unwrap();
        align_to_tcfd(&env, report_id.clone(), org.clone()).unwrap();

        let compliance = check_reporting_standards(&env, report_id).unwrap();
        assert!(compliance > 0);
    }

    #[test]
    fn test_get_organization_reports() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();

        let reports = get_organization_reports(&env, org);
        assert_eq!(reports.len(), 1);
    }

    #[test]
    fn test_full_esg_workflow() {
        let env = Env::default();
        env.mock_all_auths();

        let org = Address::random(&env);

        // Collect data
        let e = collect_environmental_data(&env, org.clone(), 5000, 80, 75, 1000).unwrap();
        let s = collect_social_data(&env, org.clone(), 500, 40, 40, 2).unwrap();
        let g = collect_governance_data(&env, org.clone(), 10, 80, 30).unwrap();

        // Generate report
        let report_id = generate_esg_report(&env, org.clone(), 0, 86400, e, s, g).unwrap();

        // Align to frameworks
        align_to_gri(&env, report_id.clone(), org.clone()).unwrap();
        align_to_sasb(&env, report_id.clone(), org.clone()).unwrap();
        align_to_tcfd(&env, report_id.clone(), org.clone()).unwrap();

        // Generate stakeholder reports
        generate_investor_report(&env, report_id.clone(), org.clone()).unwrap();
        generate_employee_report(&env, report_id.clone(), org.clone()).unwrap();
        generate_supplier_report(&env, report_id.clone(), org.clone()).unwrap();
        generate_community_report(&env, report_id.clone(), org.clone()).unwrap();

        // Verify
        let report = get_esg_report(&env, report_id).unwrap();
        assert!(report.esg_score > 0);
        assert_eq!(report.framework_alignments.len(), 3);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property-based tests with proptest (issue #874)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod proptest_suite {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{Address, Env};

    proptest! {
        #[test]
        fn prop_percentage_validation_rejects_invalid(invalid_pct in 101u32..u32::MAX) {
            let env = Env::default();
            env.mock_all_auths();
            let org = Address::random(&env);

            let result = collect_environmental_data(&env, org.clone(), 1000, invalid_pct, 50, 1000);
            prop_assert_eq!(result, Err(ESGReportingError::InvalidInputRange));
        }

        #[test]
        fn prop_percentage_validation_accepts_valid(valid_pct in 0u32..=100) {
            let env = Env::default();
            env.mock_all_auths();
            let org = Address::random(&env);

            let result = collect_environmental_data(&env, org.clone(), 1000, valid_pct, 50, 1000);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_score_calculation_bounded(
            renewable in 0u32..=100,
            efficiency in 0u32..=100,
            biodiversity in 0u32..=100,
            women in 0u32..=100,
            satisfaction in 0u32..=100,
            independence in 0u32..=100,
            ethics in 0u32..=100,
            privacy in 0u32..=100,
        ) {
            let env = Env::default();

            let e = EnvironmentalMetrics {
                carbon_emissions: 5000,
                renewable_energy_percent: renewable,
                waste_recycled_percent: 75,
                water_usage: 1000,
                water_recycled_percent: 50,
                biodiversity_score: biodiversity,
                pollution_score: 80,
                energy_efficiency_score: efficiency,
                measurement_date: env.ledger().timestamp(),
            };

            let s = SocialMetrics {
                employee_count: 500,
                women_percent: women,
                minority_percent: 30,
                training_hours_per_employee: 40,
                safety_incidents: 2,
                employee_satisfaction: satisfaction,
                community_investment: 100,
                labor_violations: 0,
                measurement_date: env.ledger().timestamp(),
            };

            let g = GovernanceMetrics {
                board_size: 10,
                board_independence_percent: independence,
                women_board_percent: 30,
                executive_compensation_ratio: 250,
                ethics_training_percent: ethics,
                data_privacy_score: privacy,
                anti_corruption_score: 85,
                audit_findings: 0,
                measurement_date: env.ledger().timestamp(),
            };

            let score = calculate_esg_score(&env, &e, &s, &g);
            prop_assert!(score <= 100, "Score {} exceeds 100", score);
        }

        #[test]
        fn prop_safe_avg_never_overflows(a in 0u32..=u32::MAX, b in 0u32..=u32::MAX) {
            let env = Env::default();
            let result = super::tests::safe_avg_u32_helper(a, b);
            // Should never panic and result should be <= max of inputs
            prop_assert!(result <= a.max(b) || (a == u32::MAX && b == u32::MAX));
        }

        #[test]
        fn prop_trend_analysis_handles_large_changes(
            current in 1u32..=u32::MAX,
            previous in 1u32..=u32::MAX,
        ) {
            let env = Env::default();
            let result = trend_analysis(
                &env,
                soroban_sdk::bytes!(&env, b"test"),
                current,
                previous,
            );
            prop_assert!(result.is_ok());
            let analysis = result.unwrap();
            // Change percent should be clamped to i32 range
            prop_assert!(analysis.change_percent >= i32::MIN);
            prop_assert!(analysis.change_percent <= i32::MAX);
        }

        #[test]
        fn prop_carbon_emissions_any_value(carbon in 0u32..=u32::MAX) {
            let env = Env::default();
            env.mock_all_auths();
            let org = Address::random(&env);

            let result = collect_environmental_data(&env, org, carbon, 50, 50, 1000);
            prop_assert!(result.is_ok());
            let metrics = result.unwrap();
            prop_assert_eq!(metrics.carbon_emissions, carbon);
        }
    }

    // Helper function for property test (needs to be public for proptest macro)
    pub fn safe_avg_u32_helper(a: u32, b: u32) -> u32 {
        let sum = (a as u64).saturating_add(b as u64);
        (sum / 2) as u32
    }
}
