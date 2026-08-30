#[cfg(test)]
mod tests {
    use crate::ropa::*;
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
        ROPAManager::initialize(env.clone(), owner.clone());
    }

    #[test]
    fn test_create_record() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        ROPAManager::initialize(env.clone(), owner.clone());

        let recipients = vec![
            &env,
            RecipientInfo {
                recipient_type: RecipientType::Processor as u32,
                name: Bytes::from_slice(&env, b"CloudProvider"),
                address: sample_address(&env, 4),
                country: Bytes::from_slice(&env, b"DE"),
                purpose: Bytes::from_slice(&env, b"hosting"),
                safeguards: Bytes::from_slice(&env, b"iso27001"),
            },
        ];

        let transfers = vec![&env, TransferInfo {
            country: Bytes::from_slice(&env, b"US"),
            mechanism: TransferMechanism::SCCs as u32,
            data_categories: vec![&env, DataCategory::IdentityData as u32],
            frequency: Bytes::from_slice(&env, b"continuous"),
            documentation: Bytes::from_slice(&env, b"TIA-2026-001"),
            transfer_hash: BytesN::from_array(&env, &[0u8; 32]),
        }];

        let record_id = ROPAManager::create_record(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Customer Support Processing"),
            vec![&env, ProcessingPurpose::ContractPerformance as u32],
            vec![&env, DataCategory::IdentityData as u32, DataCategory::ContactData as u32],
            vec![&env, DataSubjectCategory::Customers as u32],
            recipients,
            transfers,
            2555,
            vec![&env, Bytes::from_slice(&env, b"encryption_at_rest"), Bytes::from_slice(&env, b"access_control")],
            Bytes::from_slice(&env, b"Customer support ticket processing"),
        );

        let record = ROPAManager::get_record(env.clone(), record_id);
        assert_eq!(record.retention_period_days, 2555);
        assert_eq!(record.processing_purposes.len(), 1);
    }

    #[test]
    fn test_register_activity() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        ROPAManager::initialize(env.clone(), owner.clone());

        let recipients = vec![&env, RecipientInfo {
            recipient_type: RecipientType::Processor as u32,
            name: Bytes::from_slice(&env, b"PaymentProcessor"),
            address: sample_address(&env, 4),
            country: Bytes::from_slice(&env, b"IE"),
            purpose: Bytes::from_slice(&env, b"payment_processing"),
            safeguards: Bytes::from_slice(&env, b"pci_dss"),
        }];

        let record_id = ROPAManager::create_record(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Payment Processing"),
            vec![&env, ProcessingPurpose::ContractPerformance as u32],
            vec![&env, DataCategory::FinancialData as u32],
            vec![&env, DataSubjectCategory::Customers as u32],
            recipients,
            vec![],
            3650,
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            Bytes::from_slice(&env, b"Payment processing"),
        );

        let activity_id = ROPAManager::register_activity(
            env.clone(),
            owner,
            record_id,
            Bytes::from_slice(&env, b"daily_settlement"),
            Bytes::from_slice(&env, b"Daily batch settlement processing"),
            ProcessingPurpose::ContractPerformance as u32,
        );

        let activity = ROPAManager::get_activity(env.clone(), activity_id);
        assert!(activity.is_active);
        assert_eq!(activity.legal_basis, ProcessingPurpose::ContractPerformance as u32);
    }

    #[test]
    fn test_update_record() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        ROPAManager::initialize(env.clone(), owner.clone());

        let recipients = vec![&env, RecipientInfo {
            recipient_type: RecipientType::InternalTeam as u32,
            name: Bytes::from_slice(&env, b"Legal Team"),
            address: sample_address(&env, 4),
            country: Bytes::from_slice(&env, b"GB"),
            purpose: Bytes::from_slice(&env, b"compliance"),
            safeguards: Bytes::from_slice(&env, b"access_control"),
        }];

        let record_id = ROPAManager::create_record(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"HR Processing"),
            vec![&env, ProcessingPurpose::LegalObligation as u32],
            vec![&env, DataCategory::IdentityData as u32],
            vec![&env, DataSubjectCategory::Employees as u32],
            recipients,
            vec![],
            3650,
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            Bytes::from_slice(&env, b"HR data processing"),
        );

        let updated = ROPAManager::update_record(
            env.clone(),
            owner,
            record_id,
            vec![&env, Bytes::from_slice(&env, b"enhanced_encryption"), Bytes::from_slice(&env, b"mfa")],
            1825,
            Bytes::from_slice(&env, b"Updated HR processing description"),
        );

        assert_eq!(updated.retention_period_days, 1825);
        assert_eq!(updated.security_measures.len(), 2);
    }

    #[test]
    fn test_mark_audited() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        ROPAManager::initialize(env.clone(), owner.clone());

        let recipients = vec![&env, RecipientInfo {
            recipient_type: RecipientType::Authority as u32,
            name: Bytes::from_slice(&env, b"DPA"),
            address: sample_address(&env, 4),
            country: Bytes::from_slice(&env, b"DK"),
            purpose: Bytes::from_slice(&env, b"supervisory"),
            safeguards: Bytes::from_slice(&env, b"legal_basis"),
        }];

        let record_id = ROPAManager::create_record(
            env.clone(),
            owner.clone(),
            controller,
            dpo,
            Bytes::from_slice(&env, b"Marketing Processing"),
            vec![&env, ProcessingPurpose::Consent as u32],
            vec![&env, DataCategory::ContactData as u32],
            vec![&env, DataSubjectCategory::Customers as u32],
            recipients,
            vec![],
            1095,
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            Bytes::from_slice(&env, b"Marketing"),
        );

        let audited = ROPAManager::mark_audited(env.clone(), owner, record_id);
        assert!(audited.last_audited_at > 0);
    }

    #[test]
    fn test_get_ropa_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let dpo = sample_address(&env, 3);

        ROPAManager::initialize(env.clone(), owner.clone());

        let recipients = vec![&env, RecipientInfo {
            recipient_type: RecipientType::Processor as u32,
            name: Bytes::from_slice(&env, b"AnalyticsProvider"),
            address: sample_address(&env, 4),
            country: Bytes::from_slice(&env, b"US"),
            purpose: Bytes::from_slice(&env, b"analytics"),
            safeguards: Bytes::from_slice(&env, b"tls"),
        }];

        ROPAManager::create_record(
            env.clone(),
            owner,
            controller,
            dpo,
            Bytes::from_slice(&env, b"Analytics Processing"),
            vec![&env, ProcessingPurpose::LegitimateInterest as u32],
            vec![&env, DataCategory::CommunicationData as u32],
            vec![&env, DataSubjectCategory::Users as u32],
            recipients,
            vec![],
            730,
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            Bytes::from_slice(&env, b"Analytics"),
        );

        let (records, activities) = ROPAManager::get_ropa_stats(env.clone());
        assert_eq!(records, 1);
        assert_eq!(activities, 0);
    }
}
