#[cfg(test)]
mod tests {
    use crate::data_processing_agreements::*;
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
        DataProcessingAgreements::initialize(env.clone(), owner.clone());
    }

    #[test]
    fn test_register_agreement() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-001"),
            vec![&env, Bytes::from_slice(&env, b"payroll_processing")],
            vec![&env, Bytes::from_slice(&env, b"employee_pii")],
            vec![&env, Bytes::from_slice(&env, b"employees")],
            vec![&env, Bytes::from_slice(&env, b"encryption_at_rest")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let agreement = DataProcessingAgreements::get_agreement(env.clone(), agreement_id);
        assert_eq!(agreement.status, AgreementStatus::Active as u32);
        assert!(agreement.audit_rights_granted);
        assert!(agreement.renewal_auto);
    }

    #[test]
    fn test_register_subprocessor() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-002"),
            vec![&env, Bytes::from_slice(&env, b"cloud_hosting")],
            vec![&env, Bytes::from_slice(&env, b"customer_data")],
            vec![&env, Bytes::from_slice(&env, b"eu_customers")],
            vec![&env, Bytes::from_slice(&env, b"iso27001")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let subprocessor_id = DataProcessingAgreements::register_subprocessor(
            env.clone(),
            owner.clone(),
            agreement_id.clone(),
            Bytes::from_slice(&env, b"CloudSub GmbH"),
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"DE"),
            vec![&env, Bytes::from_slice(&env, b"cloud_hosting")],
            vec![&env, Bytes::from_slice(&env, b"iso27001"), Bytes::from_slice(&env, b"soc2")],
            1740000000,
        );

        let subprocessor = DataProcessingAgreements::get_subprocessor(env.clone(), subprocessor_id);
        assert_eq!(subprocessor.authorization_status, AuthorizationStatus::Pending as u32);
    }

    #[test]
    fn test_authorize_subprocessor() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-003"),
            vec![&env, Bytes::from_slice(&env, b"analytics")],
            vec![&env, Bytes::from_slice(&env, b"usage_data")],
            vec![&env, Bytes::from_slice(&env, b"users")],
            vec![&env, Bytes::from_slice(&env, b"encryption_in_transit")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let subprocessor_id = DataProcessingAgreements::register_subprocessor(
            env.clone(),
            owner.clone(),
            agreement_id,
            Bytes::from_slice(&env, b"AnalyticsCo"),
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"US"),
            vec![&env, Bytes::from_slice(&env, b"analytics")],
            vec![],
            1740000000,
        );

        let authorized = DataProcessingAgreements::authorize_subprocessor(
            env.clone(),
            owner,
            subprocessor_id.clone(),
        );

        assert_eq!(authorized.authorization_status, AuthorizationStatus::Authorized as u32);
    }

    #[test]
    fn test_register_transfer() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-004"),
            vec![&env, Bytes::from_slice(&env, b"support")],
            vec![&env, Bytes::from_slice(&env, b"ticket_data")],
            vec![&env, Bytes::from_slice(&env, b"customers")],
            vec![&env, Bytes::from_slice(&env, b"tls")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let transfer_id = DataProcessingAgreements::register_transfer(
            env.clone(),
            owner.clone(),
            agreement_id,
            TransferMechanism::SCCs as u32,
            Bytes::from_slice(&env, b"US"),
            vec![&env, Bytes::from_slice(&env, b"ticket_data")],
            Bytes::from_slice(&env, b"continuous"),
            vec![&env, Bytes::from_slice(&env, b"tls"), Bytes::from_slice(&env, b"pseudonymization")],
            Bytes::from_slice(&env, b"TIA-2026-001"),
        );

        let transfer = DataProcessingAgreements::get_transfer(env.clone(), transfer_id);
        assert_eq!(transfer.mechanism, TransferMechanism::SCCs as u32);
    }

    #[test]
    fn test_schedule_and_complete_audit() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-005"),
            vec![&env, Bytes::from_slice(&env, b"hr")],
            vec![&env, Bytes::from_slice(&env, b"employee_data")],
            vec![&env, Bytes::from_slice(&env, b"staff")],
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let audit_id = DataProcessingAgreements::schedule_audit(
            env.clone(),
            owner.clone(),
            agreement_id,
            sample_address(&env, 4),
            Bytes::from_slice(&env, b"security_audit"),
            Bytes::from_slice(&env, b"full_scope"),
            1740000000,
        );

        let audit = DataProcessingAgreements::get_audit(env.clone(), audit_id);
        assert_eq!(audit.status, 0);

        let completed = DataProcessingAgreements::complete_audit(
            env.clone(),
            owner,
            audit_id,
            Bytes::from_slice(&env, b"no_critical_findings"),
            BytesN::from_array(&env, &[1u8; 32]),
        );

        assert_eq!(completed.status, 1);
        assert_eq!(completed.completed_date, 1740000000);
    }

    #[test]
    fn test_renew_agreement() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        let agreement_id = DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-006"),
            vec![&env, Bytes::from_slice(&env, b"finance")],
            vec![&env, Bytes::from_slice(&env, b"transaction_data")],
            vec![&env, Bytes::from_slice(&env, b"customers")],
            vec![&env, Bytes::from_slice(&env, b"encryption_at_rest")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let renewed = DataProcessingAgreements::renew_agreement(
            env.clone(),
            owner,
            agreement_id,
            1765069200,
        );

        assert_eq!(renewed.expiration_date, 1765069200);
    }

    #[test]
    fn test_get_dpa_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let controller = sample_address(&env, 2);
        let processor = sample_address(&env, 3);

        DataProcessingAgreements::initialize(env.clone(), owner.clone());

        DataProcessingAgreements::register_agreement(
            env.clone(),
            owner.clone(),
            controller,
            processor,
            Bytes::from_slice(&env, b"DPA-2026-007"),
            vec![&env, Bytes::from_slice(&env, b"marketing")],
            vec![&env, Bytes::from_slice(&env, b"email_data")],
            vec![&env, Bytes::from_slice(&env, b"prospects")],
            vec![&env, Bytes::from_slice(&env, b"encryption")],
            true,
            30,
            1700000000,
            1738539600,
            true,
            90,
        );

        let (agreements, subprocessors, transfers, audits) =
            DataProcessingAgreements::get_dpa_stats(env.clone());
        assert_eq!(agreements, 1);
        assert_eq!(subprocessors, 0);
        assert_eq!(transfers, 0);
        assert_eq!(audits, 0);
    }
}
