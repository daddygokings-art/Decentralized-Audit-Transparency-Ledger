#[cfg(test)]
mod tests {
    use crate::dpia::*;
    use soroban_sdk::{vec, Address, Bytes, BytesN, Env};

    fn create_test_env() -> Env {
        Env::default()
    }

    fn sample_address(env: &Env, _id: u32) -> Address {
        Address::generate(env)
    }

    #[test]
    fn test_initialize() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        DPIAManager::initialize(env.clone(), owner.clone());
    }

    #[test]
    fn test_create_assessment() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Employee Monitoring System"),
            Bytes::from_slice(&env, b"Systematic monitoring of employee communications for compliance"),
            vec![&env, ProcessingType::SystematicMonitoring as u32],
            vec![&env, DataCategory::CommunicationData as u32, DataCategory::LocationData as u32],
            500,
            vec![&env, Bytes::from_slice(&env, b"EU"), Bytes::from_slice(&env, b"US")],
            Bytes::from_slice(&env, b"Prevent data exfiltration"),
            Bytes::from_slice(&env, b"WP248 rev.01"),
        );

        let dpia = DPIAManager::get_dpia(env.clone(), dpia_id);
        assert_eq!(dpia.status, DPIAStatus::Draft as u32);
        assert_eq!(dpia.data_subjects_estimated, 500);
        assert_eq!(dpia.processing_types.len(), 1);
    }

    #[test]
    fn test_assess_risk() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Public CCTV Analytics"),
            Bytes::from_slice(&env, b"Facial recognition in public spaces"),
            vec![&env, ProcessingType::PublicMonitoring as u32],
            vec![&env, DataCategory::BiometricData as u32],
            100000,
            vec![&env, Bytes::from_slice(&env, b"UK")],
            Bytes::from_slice(&env, b"Public safety"),
            Bytes::from_slice(&env, b"WP248 rev.01"),
        );

        let assessed = DPIAManager::assess_risk(
            env.clone(),
            owner,
            dpia_id,
            8,
            9,
            vec![&env, Bytes::from_slice(&env, b"mass_surveillance"), Bytes::from_slice(&env, b"biometric")],
            vec![&env, 1, 2, 3, 4, 5, 6],
        );

        assert_eq!(assessed.risk_assessment.overall_risk, 72);
        assert_eq!(assessed.status, DPIAStatus::ConsultationRequired as u32);
    }

    #[test]
    fn test_add_stakeholder() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Health Data Processing"),
            Bytes::from_slice(&env, b"Processing patient health records"),
            vec![&env, ProcessingType::HealthData as u32],
            vec![&env, DataCategory::HealthData as u32],
            10000,
            vec![&env, Bytes::from_slice(&env, b"DE")],
            Bytes::from_slice(&env, b"Healthcare delivery"),
            Bytes::from_slice(&env, b"ISO29134"),
        );

        let stakeholder = DPIAManager::add_stakeholder(
            env.clone(),
            owner.clone(),
            dpia_id,
            StakeholderRole::DPO as u32,
            dpo,
            Bytes::from_slice(&env, b"Chief DPO"),
        );

        assert_eq!(stakeholder.stakeholders.len(), 1);
        assert_eq!(stakeholder.stakeholders[0].role, StakeholderRole::DPO as u32);
    }

    #[test]
    fn test_record_stakeholder_feedback() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Genetic Data Research"),
            Bytes::from_slice(&env, b"Genomic analysis for research"),
            vec![&env, ProcessingType::GeneticData as u32],
            vec![&env, DataCategory::SpecialCategory as u32],
            5000,
            vec![&env, Bytes::from_slice(&env, b"EU")],
            Bytes::from_slice(&env, b"Medical research"),
            Bytes::from_slice(&env, b"ISO29134"),
        );

        DPIAManager::add_stakeholder(
            env.clone(),
            owner.clone(),
            dpia_id,
            StakeholderRole::SupervisoryAuthority as u32,
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"Regional DPA"),
        );

        let updated = DPIAManager::record_stakeholder_feedback(
            env.clone(),
            owner,
            dpia_id,
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"Concerns noted, additional safeguards required"),
            true,
        );

        assert!(updated.stakeholders[0].consent_given);
        assert!(updated.stakeholders[0].consulted_at > 0);
    }

    #[test]
    fn test_add_mitigation() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Large Scale Profiling"),
            Bytes::from_slice(&env, b"Automated profiling of loan applicants"),
            vec![&env, ProcessingType::AutomatedDecisionMaking as u32],
            vec![&env, DataCategory::FinancialData as u32],
            1000000,
            vec![&env, Bytes::from_slice(&env, b"EU")],
            Bytes::from_slice(&env, b"Credit risk assessment"),
            Bytes::from_slice(&env, b"WP248 rev.01"),
        );

        let measure_id = DPIAManager::add_mitigation(
            env.clone(),
            owner.clone(),
            dpia_id,
            Bytes::from_slice(&env, b"technical"),
            Bytes::from_slice(&env, b"Pseudonymization and encryption"),
            80,
            60,
            sample_address(&env, 4),
            1740000000,
        );

        assert!(measure_id != BytesN::from_array(&env, &[0u8; 32]));
    }

    #[test]
    fn test_notify_authority() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Biometric Time Tracking"),
            Bytes::from_slice(&env, b"Facial recognition for employee time tracking"),
            vec![&env, ProcessingType::BiometricIdentification as u32],
            vec![&env, DataCategory::BiometricData as u32],
            5000,
            vec![&env, Bytes::from_slice(&env, b"DE")],
            Bytes::from_slice(&env, b"Attendance management"),
            Bytes::from_slice(&env, b"ISO29134"),
        );

        let notified = DPIAManager::notify_authority(
            env.clone(),
            owner,
            dpia_id,
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"Prior consultation documentation"),
        );

        assert!(notified.authority_notified);
        assert_eq!(notified.status, DPIAStatus::ConsultationRequired as u32);
    }

    #[test]
    fn test_record_authority_response() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        let dpia_id = DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Health App Processing"),
            Bytes::from_slice(&env, b"Processing health data from mobile app"),
            vec![&env, ProcessingType::HealthData as u32],
            vec![&env, DataCategory::HealthData as u32],
            50000,
            vec![&env, Bytes::from_slice(&env, b"EU")],
            Bytes::from_slice(&env, b"Health monitoring"),
            Bytes::from_slice(&env, b"ISO29134"),
        );

        DPIAManager::notify_authority(
            env.clone(),
            owner.clone(),
            dpia_id,
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"Consultation doc"),
        );

        let responded = DPIAManager::record_authority_response(
            env.clone(),
            owner,
            dpia_id,
            Bytes::from_slice(&env, b"Approved with additional safeguards"),
            true,
        );

        assert_eq!(responded.status, DPIAStatus::ApprovedWithConditions as u32);
        assert!(responded.completed_at > 0);
    }

    #[test]
    fn test_get_dpia_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        DPIAManager::initialize(env.clone(), owner.clone());

        DPIAManager::create_assessment(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Customer Analytics"),
            Bytes::from_slice(&env, b"Customer behavior analytics"),
            vec![&env, ProcessingType::SystematicMonitoring as u32],
            vec![&env, DataCategory::CommunicationData as u32],
            100000,
            vec![&env, Bytes::from_slice(&env, b"EU")],
            Bytes::from_slice(&env, b"Business intelligence"),
            Bytes::from_slice(&env, b"WP248 rev.01"),
        );

        let (assessments, measures) = DPIAManager::get_dpia_stats(env.clone());
        assert_eq!(assessments, 1);
        assert_eq!(measures, 0);
    }
}
