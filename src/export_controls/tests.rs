#[cfg(test)]
mod tests {
    use crate::export_controls::*;
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

        ExportControls::initialize(env.clone(), owner.clone());
        // Verify initialization successful
    }

    // ── Denied Party Tests ───────────────────────────────────────────────

    #[test]
    fn test_add_denied_party() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ExportControls::initialize(env.clone(), owner.clone());

        let entry_id = ExportControls::add_denied_party(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Sanctioned Company Ltd"),
            vec![&env, Bytes::from_slice(&env, b"SCL Inc")],
            Bytes::from_slice(&env, b"123 Main St, Tehran, Iran"),
            Bytes::from_slice(&env, b"IR"),
            1, // OFAC
            Bytes::from_slice(&env, b"IRGC-linked entity"),
        );

        assert!(entry_id != BytesN::from_array(&env, &[0u8; 32]));
    }

    #[test]
    #[should_panic]
    fn test_screen_denied_party_match() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let party = sample_address(&env, 2);

        ExportControls::initialize(env.clone(), owner.clone());

        ExportControls::add_denied_party(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Sanctioned Corp"),
            vec![&env],
            Bytes::from_slice(&env, b"Tehran, Iran"),
            Bytes::from_slice(&env, b"IR"),
            1,
            Bytes::from_slice(&env, b"OFAC listed"),
        );

        // Screen party with matching name
        ExportControls::screen_denied_party(
            env.clone(),
            party.clone(),
            party.clone(),
            Bytes::from_slice(&env, b"Sanctioned Corp"),
        );
    }

    // ── Export License Tests ─────────────────────────────────────────────

    #[test]
    fn test_issue_export_license() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);
        let end_user = sample_address(&env, 3);

        ExportControls::initialize(env.clone(), owner.clone());

        let license_id = ExportControls::issue_export_license(
            env.clone(),
            owner.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"NLR-001"),
            vec![&env, Bytes::from_slice(&env, b"Commodity A")],
            vec![&env, Bytes::from_slice(&env, b"GB")],
            Bytes::from_slice(&env, b"Commercial use"),
            end_user.clone(),
            1000u64,
            30, // 30 days validity
        );

        let license = ExportControls::get_export_license(env.clone(), license_id.clone());
        assert_eq!(license.status, 0); // active
    }

    #[test]
    fn test_verify_license() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);
        let end_user = sample_address(&env, 3);

        ExportControls::initialize(env.clone(), owner.clone());

        let license_id = ExportControls::issue_export_license(
            env.clone(),
            owner.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"LICENSE-001"),
            vec![&env, Bytes::from_slice(&env, b"Item X")],
            vec![&env, Bytes::from_slice(&env, b"DE")],
            Bytes::from_slice(&env, b"End-use statement"),
            end_user.clone(),
            5000u64,
            60,
        );

        // Verify license
        assert!(ExportControls::verify_license(
            env.clone(),
            license_id,
            Bytes::from_slice(&env, b"Item X"),
            Bytes::from_slice(&env, b"DE")
        ));
    }

    // ── Controlled Commodity Tests ───────────────────────────────────────

    #[test]
    fn test_register_commodity() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ExportControls::initialize(env.clone(), owner.clone());

        let commodity_id = ExportControls::register_commodity(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Advanced Semiconductor"),
            Bytes::from_slice(&env, b"3A001"),
            Bytes::from_slice(&env, b"Advanced Computing"),
            4, // BIS
            vec![&env, Bytes::from_slice(&env, b"IR"), Bytes::from_slice(&env, b"KP")],
            1,  // License required
            true,
            256, // 256-bit encryption
            false,
        );

        let commodity = ExportControls::get_commodity(env.clone(), commodity_id.clone());
        assert_eq!(commodity.license_requirement, 1);
    }

    // ── End-Use Check Tests ──────────────────────────────────────────────

    #[test]
    fn test_check_end_use_cleared() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let end_user = sample_address(&env, 2);

        ExportControls::initialize(env.clone(), owner.clone());

        let check_id = ExportControls::check_end_use(
            env.clone(),
            end_user.clone(),
            Bytes::from_slice(&env, b"Item X"),
            Bytes::from_slice(&env, b"Commercial manufacturing"),
            end_user.clone(),
            Bytes::from_slice(&env, b"DE"),
        );

        let check = ExportControls::get_end_use_check(env.clone(), check_id.clone());
        assert_eq!(check.result, 0); // cleared
    }

    #[test]
    #[should_panic]
    fn test_check_end_use_military() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let end_user = sample_address(&env, 2);

        ExportControls::initialize(env.clone(), owner.clone());

        ExportControls::check_end_use(
            env.clone(),
            end_user.clone(),
            Bytes::from_slice(&env, b"Dual-use component"),
            Bytes::from_slice(&env, b"Military weapons system"),
            end_user.clone(),
            Bytes::from_slice(&env, b"IR"),
        );
    }

    // ── Re-Export Tests ──────────────────────────────────────────────────

    #[test]
    fn test_record_re_export() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter1 = sample_address(&env, 2);
        let re_exporter = sample_address(&env, 3);
        let end_user = sample_address(&env, 4);

        ExportControls::initialize(env.clone(), owner.clone());

        // Create license
        let license_id = ExportControls::issue_export_license(
            env.clone(),
            owner.clone(),
            exporter1.clone(),
            Bytes::from_slice(&env, b"LICENSE-001"),
            vec![&env, Bytes::from_slice(&env, b"Item Y")],
            vec![&env, Bytes::from_slice(&env, b"GB")],
            Bytes::from_slice(&env, b"End-use"),
            end_user.clone(),
            1000u64,
            60,
        );

        // Record re-export to friendly destination
        let re_export_id = ExportControls::record_re_export(
            env.clone(),
            re_exporter.clone(),
            re_exporter.clone(),
            exporter1.clone(),
            Bytes::from_slice(&env, b"Item Y"),
            Bytes::from_slice(&env, b"GB"),
            Bytes::from_slice(&env, b"FR"),
            license_id.clone(),
        );

        let re_export = ExportControls::get_re_export(env.clone(), re_export_id);
        assert!(!re_export.authorization_required || re_export.approved);
    }

    #[test]
    fn test_approve_re_export() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter1 = sample_address(&env, 2);
        let re_exporter = sample_address(&env, 3);
        let end_user = sample_address(&env, 4);

        ExportControls::initialize(env.clone(), owner.clone());

        let license_id = ExportControls::issue_export_license(
            env.clone(),
            owner.clone(),
            exporter1.clone(),
            Bytes::from_slice(&env, b"LIC-002"),
            vec![&env, Bytes::from_slice(&env, b"Item Z")],
            vec![&env, Bytes::from_slice(&env, b"GB")],
            Bytes::from_slice(&env, b"End-use"),
            end_user.clone(),
            500u64,
            60,
        );

        let re_export_id = ExportControls::record_re_export(
            env.clone(),
            re_exporter.clone(),
            re_exporter.clone(),
            exporter1.clone(),
            Bytes::from_slice(&env, b"Item Z"),
            Bytes::from_slice(&env, b"GB"),
            Bytes::from_slice(&env, b"FR"),
            license_id.clone(),
        );

        ExportControls::approve_re_export(env.clone(), owner.clone(), re_export_id.clone());

        let re_export = ExportControls::get_re_export(env.clone(), re_export_id);
        assert!(re_export.approved);
    }

    // ── Screening Tests ──────────────────────────────────────────────────

    #[test]
    fn test_screen_export_cleared() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);

        ExportControls::initialize(env.clone(), owner.clone());

        // Set country group for friendly destination
        ExportControls::set_country_group(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"GB"),
            1, // Group A
        );

        let screening_id = ExportControls::screen_export(
            env.clone(),
            owner.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"Commercial Good"),
            Bytes::from_slice(&env, b"GB"),
            Bytes::from_slice(&env, b"Commercial use"),
            exporter.clone(),
        );

        let screening = ExportControls::get_screening_result(env.clone(), screening_id);
        assert_eq!(screening.result, 0); // cleared
    }

    #[test]
    #[should_panic]
    fn test_screen_export_blocked() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);

        ExportControls::initialize(env.clone(), owner.clone());

        ExportControls::screen_export(
            env.clone(),
            owner.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"Dual-use Item"),
            Bytes::from_slice(&env, b"IR"),
            Bytes::from_slice(&env, b"Weapons development"),
            exporter.clone(),
        );
    }

    // ── Country Classification Tests ─────────────────────────────────────

    #[test]
    fn test_set_and_get_country_group() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ExportControls::initialize(env.clone(), owner.clone());

        ExportControls::set_country_group(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"KP"),
            4, // Group E (embargo)
        );

        let group = ExportControls::get_country_group(
            env.clone(),
            Bytes::from_slice(&env, b"KP"),
        );
        assert_eq!(group, 4);
    }

    // ── Statistics Tests ─────────────────────────────────────────────────

    #[test]
    fn test_get_export_controls_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        ExportControls::initialize(env.clone(), owner.clone());

        // Add some data
        ExportControls::add_denied_party(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Entity"),
            vec![&env],
            Bytes::from_slice(&env, b"Address"),
            Bytes::from_slice(&env, b"IR"),
            1,
            Bytes::from_slice(&env, b"Reason"),
        );

        ExportControls::register_commodity(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Commodity"),
            Bytes::from_slice(&env, b"3A001"),
            Bytes::from_slice(&env, b"Type"),
            4,
            vec![&env],
            1,
            false,
            256,
            false,
        );

        let (denied, screenings, blocked, licenses, commodities) =
            ExportControls::get_export_controls_stats(env.clone());

        assert_eq!(denied, 1);
        assert_eq!(commodities, 1);
    }

    // ── Integration Tests ────────────────────────────────────────────────

    #[test]
    fn test_full_export_workflow() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);
        let end_user = sample_address(&env, 3);

        ExportControls::initialize(env.clone(), owner.clone());

        // 1. Register controlled commodity
        let _commodity_id = ExportControls::register_commodity(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"Encryption Software"),
            Bytes::from_slice(&env, b"5D002"),
            Bytes::from_slice(&env, b"Encryption"),
            1,
            vec![&env, Bytes::from_slice(&env, b"IR")],
            1, // License required
            true,
            256,
            false,
        );

        // 2. Issue export license
        let license_id = ExportControls::issue_export_license(
            env.clone(),
            owner.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"LICENSE-EXPORT-001"),
            vec![&env, Bytes::from_slice(&env, b"Encryption Software")],
            vec![&env, Bytes::from_slice(&env, b"GB"), Bytes::from_slice(&env, b"DE")],
            Bytes::from_slice(&env, b"Commercial encryption for authorized use"),
            end_user.clone(),
            100u64,
            90, // 90 days
        );

        // 3. Perform end-use check
        let _check_id = ExportControls::check_end_use(
            env.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"Encryption Software"),
            Bytes::from_slice(&env, b"Commercial software development"),
            end_user.clone(),
            Bytes::from_slice(&env, b"GB"),
        );

        // 4. Screen export
        ExportControls::set_country_group(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"GB"),
            1, // Group A
        );

        let screening_id = ExportControls::screen_export(
            env.clone(),
            exporter.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"Encryption Software"),
            Bytes::from_slice(&env, b"GB"),
            Bytes::from_slice(&env, b"Commercial software"),
            end_user.clone(),
        );

        // Verify workflow completed
        let screening = ExportControls::get_screening_result(env.clone(), screening_id);
        let (_, screenings, _, licenses, _) = ExportControls::get_export_controls_stats(env.clone());

        assert_eq!(screenings, 1);
        assert_eq!(licenses, 1);
        assert!(screening.risk_score <= 100);
    }
}
