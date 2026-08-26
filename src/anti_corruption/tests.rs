#[cfg(test)]
mod tests {
    use crate::anti_corruption::*;
    use soroban_sdk::{vec, Address, Bytes, Env};

    fn create_test_env() -> Env {
        Env::default()
    }

    fn sample_address(env: &Env, id: u32) -> Address {
        Address::generate(env)
    }

    // ── Initialization Tests ─────────────────────────────────────────────

    #[test]
    fn test_initialize() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());
        // Verify initialization successful
    }

    // ── Policy Management Tests ──────────────────────────────────────────

    #[test]
    fn test_publish_policy() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let policy_id = AntiCorruption::publish_policy(
            env.clone(),
            compliance_officer.clone(),
            1, // AntiBriberyCorruption
            Bytes::from_slice(&env, b"Anti-Bribery Policy"),
            Bytes::from_slice(&env, b"Policy description"),
            Bytes::from_slice(&env, b"policy content"),
        );

        let policy = AntiCorruption::get_policy(env.clone(), policy_id.clone());
        assert_eq!(policy.policy_type, 1);
        assert!(policy.active);
    }

    #[test]
    fn test_update_policy() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let policy_id = AntiCorruption::publish_policy(
            env.clone(),
            compliance_officer.clone(),
            1,
            Bytes::from_slice(&env, b"Anti-Bribery Policy"),
            Bytes::from_slice(&env, b"description"),
            Bytes::from_slice(&env, b"content v1"),
        );

        AntiCorruption::update_policy(
            env.clone(),
            compliance_officer.clone(),
            policy_id.clone(),
            Bytes::from_slice(&env, b"content v2"),
        );

        let updated_policy = AntiCorruption::get_policy(env.clone(), policy_id.clone());
        assert_eq!(updated_policy.version, 2);
    }

    // ── Risk Assessment Tests ────────────────────────────────────────────

    #[test]
    fn test_assess_risk() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let subject = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let assessment_id = AntiCorruption::assess_risk(
            env.clone(),
            compliance_officer.clone(),
            subject.clone(),
            1, // Low risk
            vec![&env, Bytes::from_slice(&env, b"factor1")],
            vec![&env, Bytes::from_slice(&env, b"mitigation1")],
            30, // 30 days
        );

        let assessment = AntiCorruption::get_risk_assessment(env.clone(), subject.clone());
        assert_eq!(assessment.risk_level, 1);
    }

    // ── Training Management Tests ────────────────────────────────────────

    #[test]
    fn test_create_and_complete_training() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let employee = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let now = env.ledger().timestamp();
        let training_id = AntiCorruption::create_training(
            env.clone(),
            compliance_officer.clone(),
            employee.clone(),
            1, // AntiBriberyCorruption training
            now + 86400 * 30, // Due in 30 days
        );

        let training = AntiCorruption::get_training_record(env.clone(), training_id.clone());
        assert_eq!(training.status, 0); // NotStarted

        AntiCorruption::complete_training(env.clone(), employee.clone(), training_id.clone(), 95);

        let completed_training =
            AntiCorruption::get_training_record(env.clone(), training_id.clone());
        assert_eq!(completed_training.status, 2); // Completed
        assert_eq!(completed_training.score, 95);
    }

    // ── Third-Party Risk Management Tests ─────────────────────────────────

    #[test]
    fn test_assess_third_party_low_risk() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let third_party = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let risk_id = AntiCorruption::assess_third_party(
            env.clone(),
            compliance_officer.clone(),
            third_party.clone(),
            Bytes::from_slice(&env, b"Vendor Inc"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"Technology"),
            false, // Not PEP
            false, // No sanctions match
        );

        let profile = AntiCorruption::get_third_party_risk(env.clone(), third_party.clone());
        assert_eq!(profile.risk_level, 1); // Low
        assert!(!profile.is_pep);
        assert!(!profile.sanctions_match);
    }

    #[test]
    fn test_assess_third_party_pep() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let pep_party = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let _risk_id = AntiCorruption::assess_third_party(
            env.clone(),
            compliance_officer.clone(),
            pep_party.clone(),
            Bytes::from_slice(&env, b"Government Official"),
            Bytes::from_slice(&env, b"RU"),
            Bytes::from_slice(&env, b"Government"),
            true, // PEP
            false,
        );

        let profile = AntiCorruption::get_third_party_risk(env.clone(), pep_party.clone());
        assert_eq!(profile.risk_level, 4); // Critical
        assert!(profile.is_pep);
    }

    #[test]
    fn test_complete_due_diligence() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let third_party = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let risk_id = AntiCorruption::assess_third_party(
            env.clone(),
            compliance_officer.clone(),
            third_party.clone(),
            Bytes::from_slice(&env, b"Vendor Inc"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"Technology"),
            false,
            false,
        );

        AntiCorruption::complete_due_diligence(
            env.clone(),
            compliance_officer.clone(),
            risk_id.clone(),
            true, // Beneficial owners disclosed
        );

        let profile = AntiCorruption::get_third_party_risk(env.clone(), third_party.clone());
        assert!(profile.due_diligence_completed);
        assert!(profile.beneficial_owners_disclosed);
    }

    // ── Transaction Monitoring Tests ─────────────────────────────────────

    #[test]
    fn test_monitor_normal_transaction() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let from = sample_address(&env, 3);
        let to = sample_address(&env, 4);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let tx_id = AntiCorruption::monitor_transaction(
            env.clone(),
            from.clone(),
            from.clone(),
            to.clone(),
            1, // GovernmentPayment
            5000u64,
            Bytes::from_slice(&env, b"USD"),
            Bytes::from_slice(&env, b"Service payment"),
        );

        let transaction = AntiCorruption::get_transaction(env.clone(), tx_id.clone());
        assert_eq!(transaction.status, 1); // Approved (no issues)
        assert_eq!(transaction.amount, 5000);
    }

    #[test]
    fn test_monitor_gift_exceeding_limit() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let from = sample_address(&env, 3);
        let to = sample_address(&env, 4);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let tx_id = AntiCorruption::monitor_transaction(
            env.clone(),
            from.clone(),
            from.clone(),
            to.clone(),
            2, // GiftEntertainment
            1000u64, // Exceeds $500 limit
            Bytes::from_slice(&env, b"USD"),
            Bytes::from_slice(&env, b"Gift"),
        );

        let transaction = AntiCorruption::get_transaction(env.clone(), tx_id.clone());
        assert_eq!(transaction.status, 0); // Pending review
    }

    #[test]
    fn test_approve_transaction() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let from = sample_address(&env, 3);
        let to = sample_address(&env, 4);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let tx_id = AntiCorruption::monitor_transaction(
            env.clone(),
            from.clone(),
            from.clone(),
            to.clone(),
            2,
            1000u64,
            Bytes::from_slice(&env, b"USD"),
            Bytes::from_slice(&env, b"Gift"),
        );

        AntiCorruption::approve_transaction(env.clone(), compliance_officer.clone(), tx_id.clone());

        let transaction = AntiCorruption::get_transaction(env.clone(), tx_id.clone());
        assert_eq!(transaction.status, 1); // Approved
    }

    // ── Whistleblower Tests ──────────────────────────────────────────────

    #[test]
    fn test_submit_whistleblower_report() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let reporter = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let report_id = AntiCorruption::submit_whistleblower_report(
            env.clone(),
            reporter.clone(),
            Bytes::from_slice(&env, b"Suspected Bribery"),
            Bytes::from_slice(&env, b"encrypted description"),
            Bytes::from_slice(&env, b"encrypted contact"),
            2, // Restricted confidentiality
        );

        // Verify report was created
        let report =
            AntiCorruption::get_whistleblower_report(env.clone(), compliance_officer.clone(), report_id.clone());
        assert_eq!(report.status, 1); // Acknowledged
    }

    #[test]
    fn test_assign_investigator_and_complete() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let reporter = sample_address(&env, 3);
        let investigator = sample_address(&env, 4);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let report_id = AntiCorruption::submit_whistleblower_report(
            env.clone(),
            reporter.clone(),
            Bytes::from_slice(&env, b"Suspected Bribery"),
            Bytes::from_slice(&env, b"encrypted description"),
            Bytes::from_slice(&env, b"encrypted contact"),
            2,
        );

        AntiCorruption::assign_investigator(
            env.clone(),
            compliance_officer.clone(),
            report_id.clone(),
            investigator.clone(),
        );

        AntiCorruption::complete_investigation(
            env.clone(),
            compliance_officer.clone(),
            report_id.clone(),
            Bytes::from_slice(&env, b"encrypted findings"),
            Bytes::from_slice(&env, b"corrective actions"),
        );

        let report =
            AntiCorruption::get_whistleblower_report(env.clone(), compliance_officer.clone(), report_id.clone());
        assert_eq!(report.status, 4); // Concluded
    }

    // ── Incident Reporting Tests ─────────────────────────────────────────

    #[test]
    fn test_report_compliance_incident() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let reporter = sample_address(&env, 3);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        let incident_id = AntiCorruption::report_incident(
            env.clone(),
            reporter.clone(),
            Bytes::from_slice(&env, b"Gift Limit Violation"),
            Bytes::from_slice(&env, b"Employee gave excessive gift"),
            3, // High severity
            Bytes::from_slice(&env, b"Root cause"),
            Bytes::from_slice(&env, b"Corrective actions"),
            30, // 30 days to remediate
        );

        let incident = AntiCorruption::get_incident(env.clone(), incident_id.clone());
        assert_eq!(incident.severity, 3);
        assert_eq!(incident.status, 0); // Reported
    }

    // ── High-Risk Jurisdiction Tests ─────────────────────────────────────

    #[test]
    fn test_add_high_risk_jurisdiction() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        AntiCorruption::add_high_risk_jurisdiction(
            env.clone(),
            compliance_officer.clone(),
            Bytes::from_slice(&env, b"KP"),
            Bytes::from_slice(&env, b"North Korea"),
            vec![&env, Bytes::from_slice(&env, b"UN Sanctions")],
            2, // Prohibition
        );

        let is_high_risk = AntiCorruption::is_high_risk_jurisdiction_check(
            env.clone(),
            Bytes::from_slice(&env, b"KP"),
        );
        assert!(is_high_risk);
    }

    // ── Statistics Tests ─────────────────────────────────────────────────

    #[test]
    fn test_get_compliance_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let subject = sample_address(&env, 3);
        let reporter = sample_address(&env, 4);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        // Create some activities
        AntiCorruption::assess_risk(
            env.clone(),
            compliance_officer.clone(),
            subject.clone(),
            1,
            vec![&env],
            vec![&env],
            30,
        );

        AntiCorruption::report_incident(
            env.clone(),
            reporter.clone(),
            Bytes::from_slice(&env, b"Test Incident"),
            Bytes::from_slice(&env, b"Description"),
            2,
            Bytes::from_slice(&env, b"Root cause"),
            Bytes::from_slice(&env, b"Actions"),
            30,
        );

        let (assessments, training, violations, incidents) =
            AntiCorruption::get_compliance_stats(env.clone());

        assert_eq!(assessments, 1);
        assert_eq!(incidents, 1);
    }

    // ── Integration Tests ────────────────────────────────────────────────

    #[test]
    fn test_full_compliance_workflow() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let compliance_officer = sample_address(&env, 2);
        let employee = sample_address(&env, 3);
        let vendor = sample_address(&env, 4);
        let reporter = sample_address(&env, 5);

        AntiCorruption::initialize(env.clone(), owner.clone(), compliance_officer.clone());

        // 1. Publish policies
        let _policy_id = AntiCorruption::publish_policy(
            env.clone(),
            compliance_officer.clone(),
            1,
            Bytes::from_slice(&env, b"Anti-Bribery Policy"),
            Bytes::from_slice(&env, b"description"),
            Bytes::from_slice(&env, b"content"),
        );

        // 2. Assess risks
        let _risk_id = AntiCorruption::assess_risk(
            env.clone(),
            compliance_officer.clone(),
            vendor.clone(),
            2, // Medium risk
            vec![&env, Bytes::from_slice(&env, b"Operates in high-risk jurisdiction")],
            vec![&env, Bytes::from_slice(&env, b"Quarterly audits")],
            90,
        );

        // 3. Assign training
        let now = env.ledger().timestamp();
        let training_id = AntiCorruption::create_training(
            env.clone(),
            compliance_officer.clone(),
            employee.clone(),
            1,
            now + 86400 * 30,
        );

        // 4. Complete training
        AntiCorruption::complete_training(env.clone(), employee.clone(), training_id.clone(), 100);

        // 5. Assess third party
        let _third_party_id = AntiCorruption::assess_third_party(
            env.clone(),
            compliance_officer.clone(),
            vendor.clone(),
            Bytes::from_slice(&env, b"Vendor Corp"),
            Bytes::from_slice(&env, b"CN"),
            Bytes::from_slice(&env, b"Manufacturing"),
            false,
            false,
        );

        // 6. Monitor transaction
        let tx_id = AntiCorruption::monitor_transaction(
            env.clone(),
            employee.clone(),
            employee.clone(),
            vendor.clone(),
            3, // ThirdPartyPayment
            10000u64,
            Bytes::from_slice(&env, b"USD"),
            Bytes::from_slice(&env, b"Service contract"),
        );

        // 7. Submit whistleblower report (hypothetical violation)
        let report_id = AntiCorruption::submit_whistleblower_report(
            env.clone(),
            reporter.clone(),
            Bytes::from_slice(&env, b"Suspected improper payment"),
            Bytes::from_slice(&env, b"encrypted description"),
            Bytes::from_slice(&env, b"encrypted contact"),
            1,
        );

        // Verify workflow completed
        let (assessments, training_count, _violations, _incidents) =
            AntiCorruption::get_compliance_stats(env.clone());
        assert_eq!(assessments, 1);
        assert_eq!(training_count, 1);
    }
}
