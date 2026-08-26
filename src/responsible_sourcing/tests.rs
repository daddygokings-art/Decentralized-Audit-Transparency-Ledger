#[cfg(test)]
mod tests {
    use crate::responsible_sourcing::*;
    use soroban_sdk::{Env, Address, Bytes, Symbol, vec, bytes};

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

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        // TODO: Verify initialization state
    }

    // ── Certifier Registration Tests ─────────────────────────────────────

    #[test]
    fn test_register_certifier() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        assert!(ResponsibleSourcing::is_certifier_approved(env.clone(), certifier.clone()));
    }

    #[test]
    fn test_revoke_certifier() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());
        ResponsibleSourcing::revoke_certifier(env.clone(), owner.clone(), certifier.clone());

        assert!(!ResponsibleSourcing::is_certifier_approved(env.clone(), certifier.clone()));
    }

    // ── Certification Tests ──────────────────────────────────────────────

    #[test]
    fn test_issue_certification() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1, // RJC scheme
            material.clone(),
            0, // No expiry
            vec![&env, 1, 2], // Audit standards
            1, // ResponsiblyMined origin
            Bytes::from_slice(&env, b"metadata"),
        );

        let cert = ResponsibleSourcing::get_certification(env.clone(), cert_id.clone());
        assert_eq!(cert.scheme, 1);
        assert_eq!(cert.status, 1); // active
    }

    #[test]
    fn test_revoke_certification() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        ResponsibleSourcing::revoke_certification(env.clone(), certifier.clone(), cert_id.clone());
        let cert = ResponsibleSourcing::get_certification(env.clone(), cert_id.clone());
        assert_eq!(cert.status, 3); // revoked
    }

    // ── Shipment Tracking Tests ──────────────────────────────────────────

    #[test]
    fn test_create_shipment() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        let shipment = ResponsibleSourcing::get_shipment(env.clone(), shipment_id.clone());
        assert_eq!(shipment.quantity, 1000);
        assert_eq!(shipment.current_custodian, creator);
        assert!(shipment.custody_verified);
    }

    // ── Chain of Custody Tests ───────────────────────────────────────────

    #[test]
    fn test_transfer_custody() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);
        let recipient = sample_address(&env, 4);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        let signature = Bytes::from_slice(&env, &[0u8; 96]); // Mock signature
        let location = Bytes::from_slice(&env, b"warehouse_a");

        let transfer_seq = ResponsibleSourcing::transfer_custody(
            env.clone(),
            creator.clone(),
            recipient.clone(),
            shipment_id.clone(),
            location,
            signature,
        );

        assert_eq!(transfer_seq, 0);

        let shipment = ResponsibleSourcing::get_shipment(env.clone(), shipment_id.clone());
        assert_eq!(shipment.current_custodian, recipient);
    }

    #[test]
    fn test_verify_custody_chain_single_custodian() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        // Single custodian should have valid chain
        assert!(ResponsibleSourcing::verify_custody_chain(env.clone(), shipment_id.clone()));
    }

    // ── Traceability Tests ───────────────────────────────────────────────

    #[test]
    fn test_record_checkpoint() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);
        let party = sample_address(&env, 4);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        let checkpoint_seq = ResponsibleSourcing::record_checkpoint(
            env.clone(),
            party.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"warehouse_a"),
            Bytes::from_slice(&env, b"entry_metadata"),
        );

        assert_eq!(checkpoint_seq, 0);

        let checkpoint = ResponsibleSourcing::get_checkpoint(env.clone(), shipment_id.clone(), checkpoint_seq);
        assert_eq!(checkpoint.party, party);
    }

    #[test]
    fn test_verify_traceability_chain() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);
        let party1 = sample_address(&env, 4);
        let party2 = sample_address(&env, 5);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            party1.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"warehouse_a"),
            Bytes::from_slice(&env, b"data1"),
        );

        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            party2.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"warehouse_b"),
            Bytes::from_slice(&env, b"data2"),
        );

        assert!(ResponsibleSourcing::verify_traceability_chain(env.clone(), shipment_id.clone()));
    }

    #[test]
    fn test_get_traceability_path() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let creator = sample_address(&env, 3);
        let party = sample_address(&env, 4);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            creator.clone(),
            cert_id.clone(),
            1000u64,
            Bytes::from_slice(&env, b"oz"),
        );

        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            party.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"warehouse_a"),
            Bytes::from_slice(&env, b"data"),
        );

        let path = ResponsibleSourcing::get_traceability_path(env.clone(), shipment_id.clone());
        assert_eq!(path.len(), 1);
    }

    // ── Material Origin Tests ────────────────────────────────────────────

    #[test]
    fn test_record_material_origin() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let origin_id = ResponsibleSourcing::record_material_origin(
            env.clone(),
            certifier.clone(),
            Bytes::from_slice(&env, b"gold"),
            Bytes::from_slice(&env, b"mine_location_xyz"),
            100u64, // extraction date (timestamp)
            true,   // conflict_free
            true,   // legally_sourced
            true,   // environmentally_compliant
            Bytes::from_slice(&env, b"documentation"),
        );

        let origin = ResponsibleSourcing::get_material_origin(env.clone(), origin_id.clone());
        assert!(origin.conflict_free);
        assert!(origin.legally_sourced);
        assert!(origin.environmentally_compliant);
    }

    // ── Audit Reporting Tests ────────────────────────────────────────────

    #[test]
    fn test_file_audit_report() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1, 2],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let report_id = ResponsibleSourcing::file_audit_report(
            env.clone(),
            certifier.clone(),
            cert_id.clone(),
            vec![&env, 1, 2],
            5,                                         // shipments audited
            Bytes::from_slice(&env, b"findings"),
            1, // compliant
        );

        let report = ResponsibleSourcing::get_audit_report(env.clone(), report_id.clone());
        assert_eq!(report.shipments_audited, 5);
        assert_eq!(report.compliance_status, 1);
        assert!(report.finalized);
    }

    // ── Consumer Claims Tests ────────────────────────────────────────────

    #[test]
    fn test_submit_consumer_claim() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let claimer = sample_address(&env, 3);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let report_id = ResponsibleSourcing::file_audit_report(
            env.clone(),
            certifier.clone(),
            cert_id.clone(),
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"findings"),
            1,
        );

        let claim_id = ResponsibleSourcing::submit_consumer_claim(
            env.clone(),
            claimer.clone(),
            Bytes::from_slice(&env, b"100% responsibly sourced"),
            cert_id.clone(),
            vec![&env, report_id.clone()],
        );

        let claim = ResponsibleSourcing::get_consumer_claim(env.clone(), claim_id.clone());
        assert_eq!(claim.claimer, claimer);
        assert_eq!(claim.verification_status, 1); // verified
    }

    #[test]
    fn test_verify_consumer_claim() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let claimer = sample_address(&env, 3);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        let material = Bytes::from_slice(&env, b"gold");
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1,
            material,
            0,
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"metadata"),
        );

        let report_id = ResponsibleSourcing::file_audit_report(
            env.clone(),
            certifier.clone(),
            cert_id.clone(),
            vec![&env, 1],
            1,
            Bytes::from_slice(&env, b"findings"),
            1, // compliant
        );

        let claim_id = ResponsibleSourcing::submit_consumer_claim(
            env.clone(),
            claimer.clone(),
            Bytes::from_slice(&env, b"100% responsibly sourced"),
            cert_id.clone(),
            vec![&env, report_id.clone()],
        );

        assert!(ResponsibleSourcing::verify_consumer_claim(
            env.clone(),
            claim_id.clone()
        ));
    }

    // ── Conflict Minerals Tests ──────────────────────────────────────────

    #[test]
    fn test_register_conflict_alert() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());

        let material = Bytes::from_slice(&env, b"conflict_tin");
        ResponsibleSourcing::register_conflict_alert(env.clone(), owner.clone(), material.clone());

        assert!(ResponsibleSourcing::is_conflict_material(env.clone(), material.clone()));
    }

    #[test]
    fn test_is_conflict_material_false() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ResponsibleSourcing::initialize(env.clone(), owner.clone());

        let material = Bytes::from_slice(&env, b"non_conflict_material");
        assert!(!ResponsibleSourcing::is_conflict_material(env.clone(), material.clone()));
    }

    // ── Integration Tests ────────────────────────────────────────────────

    #[test]
    fn test_full_supply_chain_workflow() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let certifier = sample_address(&env, 2);
        let mine_operator = sample_address(&env, 3);
        let refiner = sample_address(&env, 4);
        let distributor = sample_address(&env, 5);
        let retailer = sample_address(&env, 6);

        // Initialize
        ResponsibleSourcing::initialize(env.clone(), owner.clone());
        ResponsibleSourcing::register_certifier(env.clone(), owner.clone(), certifier.clone());

        // Issue certification
        let cert_id = ResponsibleSourcing::issue_certification(
            env.clone(),
            certifier.clone(),
            1, // RJC
            Bytes::from_slice(&env, b"recycled_gold"),
            0,
            vec![&env, 1, 2, 3],
            2, // Recycled origin
            Bytes::from_slice(&env, b"cert_metadata"),
        );

        // Record material origin
        let origin_id = ResponsibleSourcing::record_material_origin(
            env.clone(),
            certifier.clone(),
            Bytes::from_slice(&env, b"gold"),
            Bytes::from_slice(&env, b"recycling_facility"),
            1000u64,
            true,
            true,
            true,
            Bytes::from_slice(&env, b"recycling_docs"),
        );

        // Create shipment
        let shipment_id = ResponsibleSourcing::create_shipment(
            env.clone(),
            mine_operator.clone(),
            cert_id.clone(),
            100u64,
            Bytes::from_slice(&env, b"oz"),
        );

        // Record checkpoints
        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            mine_operator.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"recycling_facility"),
            Bytes::from_slice(&env, b"processed"),
        );

        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            refiner.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"refinery"),
            Bytes::from_slice(&env, b"refined"),
        );

        ResponsibleSourcing::record_checkpoint(
            env.clone(),
            distributor.clone(),
            shipment_id.clone(),
            Bytes::from_slice(&env, b"distribution_center"),
            Bytes::from_slice(&env, b"distributed"),
        );

        // File audit report
        let report_id = ResponsibleSourcing::file_audit_report(
            env.clone(),
            certifier.clone(),
            cert_id.clone(),
            vec![&env, 1, 2, 3],
            1,
            Bytes::from_slice(&env, b"all_standards_met"),
            1, // compliant
        );

        // Submit consumer claim
        let claim_id = ResponsibleSourcing::submit_consumer_claim(
            env.clone(),
            retailer.clone(),
            Bytes::from_slice(&env, b"100% ethically recycled gold"),
            cert_id.clone(),
            vec![&env, report_id.clone()],
        );

        // Verify everything
        assert!(ResponsibleSourcing::verify_custody_chain(env.clone(), shipment_id.clone()));
        assert!(ResponsibleSourcing::verify_traceability_chain(env.clone(), shipment_id.clone()));
        assert!(ResponsibleSourcing::verify_consumer_claim(env.clone(), claim_id.clone()));

        let path = ResponsibleSourcing::get_traceability_path(env.clone(), shipment_id.clone());
        assert_eq!(path.len(), 3); // Three checkpoints
    }
}
