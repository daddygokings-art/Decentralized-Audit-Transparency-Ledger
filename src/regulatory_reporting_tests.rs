#![cfg(test)]

//! Comprehensive integration tests for the automated regulatory reporting pipeline.
//!
//! Coverage matrix:
//! - All 7 authorities (FINRA, SEC, CFTC, FCA, BaFin, MAS, MiCA)
//! - All pipeline stages (generate → validate → submit → acknowledge → accept/reject)
//! - Retry and back-off behaviour
//! - Acknowledgment correlation
//! - Audit trail integrity (hash chain verification)
//! - Deadline enforcement
//! - State machine transition guards
//! - LEI format validation
//! - Cross-field validation rules
//! - Cancellation path

#[cfg(test)]
mod regulatory_reporting_tests {
    use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

    use crate::regulatory_reporting::{
        AuthorityConfig, RegulatoryAuthority, RegulatoryReport, RegulatorySubmission,
        ReportAction, ReportFormat, ReportStatus, ReportingError, ValidationResult,
    };
    use crate::report_generators::{
        BaFinGenerators, CftcGenerators, FcaGenerators, FinraGenerators, MasGenerators,
        MiCaGenerators, ReportInput, SecGenerators,
    };
    use crate::report_validation::validate_report;
    use crate::reporting_audit_trail::{
        record_accepted, record_cancelled, record_generated, record_overdue, record_rejected,
        record_submitted, record_validated, verify_trail,
    };
    use crate::submission_tracker::{
        apply_acknowledgment, cancel_report, check_transition, create_submission,
        ingest_acknowledgment, is_overdue, mark_overdue, mark_submitted, mark_validated,
        next_attempt,
    };

    // ─────────────────────────────────────────────────────────────────────
    // Test fixtures
    // ─────────────────────────────────────────────────────────────────────

    fn lei(env: &Env) -> Bytes {
        Bytes::from_slice(env, b"HWUPKR0MPOU8LEYPWAT0")
    }

    fn zero_hash(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0u8; 32])
    }

    fn base_input<'a>(env: &'a Env) -> ReportInput<'a> {
        ReportInput {
            env,
            entity: Address::generate(env),
            lei: lei(env),
            period_start: 1_700_000_000,
            period_end:   1_700_086_400,
            deadline:     1_700_172_800,
            source_event_ids: Vec::new(env),
            extra_fields: Bytes::new(env),
            prev_report_hash: zero_hash(env),
        }
    }

    fn authority_config(env: &Env, authority: RegulatoryAuthority) -> AuthorityConfig {
        AuthorityConfig {
            authority,
            enabled: true,
            endpoint: Bytes::from_slice(env, b"https://reg.example/submit"),
            credential_ref: Bytes::from_slice(env, b"cred-001"),
            max_retries: 3,
            retry_delay_seconds: 30,
            exponential_backoff: true,
            retention_ledgers: 52_560,
        }
    }

    // Build a minimal valid report that will pass validation for the given authority+format
    fn valid_report_for(env: &Env, authority: RegulatoryAuthority, format: ReportFormat) -> RegulatoryReport {
        let extra = match format {
            ReportFormat::FinraOATS      => b"mpid=ABCD\norder_count=100\nroute_count=50\n".as_ref(),
            ReportFormat::FinraCAT       => b"mpid=ABCD\ncat_reporter_id=CAT001\nevent_count=500\n".as_ref(),
            ReportFormat::FinraRule4370  => b"bcp_version=3\nemergency_contacts=contacts\n".as_ref(),
            ReportFormat::SecFormADV     => b"adviser_name=Acme\naum_usd=5000000\nclient_count=200\n".as_ref(),
            ReportFormat::SecFormPF      => b"fund_count=2\nnav_usd=10000000\nstrategy_type=hedge\n".as_ref(),
            ReportFormat::SecForm13F     => b"cusip_count=50\ntotal_value_usd=500000000\nconfidential_treatment=no\n".as_ref(),
            ReportFormat::CftcLargeTrader => b"commodity=WTI\nposition_long=500\nposition_short=200\nspecial_account=none\n".as_ref(),
            ReportFormat::CftcSwapData   => b"swap_type=IRS\nnotional_usd=10000000\ncounterparty_lei=HWUPKR0MPOU8LEYPWAT0\nuti=UTI001\n".as_ref(),
            ReportFormat::FcaMiFIDII     => b"isin=GB0002634946\nquantity=1000\nprice=10.50\nvenue_mic=XLON\nexecuting_entity_id=EID001\n".as_ref(),
            ReportFormat::FcaEMIR        => b"trade_id=T001\nasset_class=IR\nnotional_eur=5000000\ncounterparty_lei=HWUPKR0MPOU8LEYPWAT0\n".as_ref(),
            ReportFormat::BaFinWpHG      => b"isin=DE0005140008\nvoting_rights_pct=5.01\nthreshold_crossed=5\ndirection=above\n".as_ref(),
            ReportFormat::BaFinAnaCredit => b"loan_count=150\ntotal_exposure_eur=2000000\ncredit_facility_type=term_loan\n".as_ref(),
            ReportFormat::MasTRR         => b"product_type=IRS\nnotional_sgd=1000000\ncounterparty_lei=HWUPKR0MPOU8LEYPWAT0\nuti=UTI002\n".as_ref(),
            ReportFormat::MasForm610     => b"balance_sheet_total_sgd=500000000\nloan_book_sgd=200000000\nnpl_ratio=1.2\n".as_ref(),
            ReportFormat::MiCACASP       => b"service_type=exchange\nuser_count=5000\ntransaction_volume_eur=10000000\ncountries_served=DE\n".as_ref(),
            ReportFormat::MiCAReserveAsset => b"token_symbol=EURC\ntokens_outstanding=1000000\nreserve_value_eur=1000000\nreserve_composition=cash\ncustodian_lei=HWUPKR0MPOU8LEYPWAT0\n".as_ref(),
            ReportFormat::MiCAWhitePaper => b"asset_name=EuroToken\nasset_class=EMT\noffer_type=public\nissuer_country=DE\n".as_ref(),
            _ => b"generic=true\n".as_ref(),
        };

        let tag = match authority {
            RegulatoryAuthority::FINRA => b"authority=FINRA\n".as_ref(),
            RegulatoryAuthority::SEC   => b"authority=SEC\n".as_ref(),
            RegulatoryAuthority::CFTC  => b"authority=CFTC\n".as_ref(),
            RegulatoryAuthority::FCA   => b"authority=FCA\n".as_ref(),
            RegulatoryAuthority::BaFin => b"authority=BaFin\n".as_ref(),
            RegulatoryAuthority::MAS   => b"authority=MAS\n".as_ref(),
            RegulatoryAuthority::MiCA  => b"authority=MiCA\n".as_ref(),
        };

        let mut content = Bytes::new(env);
        content.extend_from_slice(tag);
        content.extend_from_slice(extra);

        RegulatoryReport {
            id: BytesN::from_array(env, &[1u8; 32]),
            authority,
            format,
            entity: Address::generate(env),
            lei: lei(env),
            period_start: 1_700_000_000,
            period_end:   1_700_086_400,
            deadline:     1_700_172_800,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            last_validation: None,
            prev_report_hash: zero_hash(env),
            report_hash: BytesN::from_array(env, &[2u8; 32]),
            source_event_ids: Vec::new(env),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // §1  Authority metadata
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_all_seven_authorities_listed() {
        assert_eq!(RegulatoryAuthority::all().len(), 7);
    }

    #[test]
    fn test_authority_names_correct() {
        assert_eq!(RegulatoryAuthority::FINRA.name(), "FINRA");
        assert_eq!(RegulatoryAuthority::SEC.name(),   "SEC");
        assert_eq!(RegulatoryAuthority::CFTC.name(),  "CFTC");
        assert_eq!(RegulatoryAuthority::FCA.name(),   "FCA");
        assert_eq!(RegulatoryAuthority::BaFin.name(), "BaFin");
        assert_eq!(RegulatoryAuthority::MAS.name(),   "MAS");
        assert_eq!(RegulatoryAuthority::MiCA.name(),  "MiCA");
    }

    #[test]
    fn test_jurisdiction_codes() {
        assert_eq!(RegulatoryAuthority::FINRA.jurisdiction(), "US");
        assert_eq!(RegulatoryAuthority::FCA.jurisdiction(),   "GB");
        assert_eq!(RegulatoryAuthority::BaFin.jurisdiction(), "DE");
        assert_eq!(RegulatoryAuthority::MAS.jurisdiction(),   "SG");
        assert_eq!(RegulatoryAuthority::MiCA.jurisdiction(),  "EU");
    }

    // ─────────────────────────────────────────────────────────────────────
    // §2  Report generation — one per authority
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_generate_finra_oats() {
        let env = Env::default();
        let mut input = base_input(&env);
        input.extra_fields = Bytes::from_slice(&env, b"mpid=ABCD\norder_count=100\nroute_count=50\n");
        let r = FinraGenerators::oats(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::FINRA);
        assert_eq!(r.format, ReportFormat::FinraOATS);
        assert_eq!(r.status, ReportStatus::Draft);
    }

    #[test]
    fn test_generate_finra_cat() {
        let env = Env::default();
        let input = base_input(&env);
        let r = FinraGenerators::cat(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::FinraCAT);
    }

    #[test]
    fn test_generate_finra_rule_4370() {
        let env = Env::default();
        let input = base_input(&env);
        let r = FinraGenerators::rule_4370(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::FinraRule4370);
    }

    #[test]
    fn test_generate_sec_form_adv() {
        let env = Env::default();
        let input = base_input(&env);
        let r = SecGenerators::form_adv(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::SEC);
        assert_eq!(r.format, ReportFormat::SecFormADV);
    }

    #[test]
    fn test_generate_sec_form_pf() {
        let env = Env::default();
        let input = base_input(&env);
        let r = SecGenerators::form_pf(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::SecFormPF);
    }

    #[test]
    fn test_generate_sec_form_13f() {
        let env = Env::default();
        let input = base_input(&env);
        let r = SecGenerators::form_13f(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::SecForm13F);
    }

    #[test]
    fn test_generate_cftc_large_trader() {
        let env = Env::default();
        let input = base_input(&env);
        let r = CftcGenerators::large_trader(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::CFTC);
    }

    #[test]
    fn test_generate_cftc_swap_data() {
        let env = Env::default();
        let input = base_input(&env);
        let r = CftcGenerators::swap_data(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::CftcSwapData);
    }

    #[test]
    fn test_generate_fca_mifid_ii() {
        let env = Env::default();
        let input = base_input(&env);
        let r = FcaGenerators::mifid_ii(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::FCA);
    }

    #[test]
    fn test_generate_fca_emir() {
        let env = Env::default();
        let input = base_input(&env);
        let r = FcaGenerators::emir(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::FcaEMIR);
    }

    #[test]
    fn test_generate_bafin_wphg() {
        let env = Env::default();
        let input = base_input(&env);
        let r = BaFinGenerators::wphg(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::BaFin);
    }

    #[test]
    fn test_generate_bafin_anacredit() {
        let env = Env::default();
        let input = base_input(&env);
        let r = BaFinGenerators::anacredit(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::BaFinAnaCredit);
    }

    #[test]
    fn test_generate_mas_trr() {
        let env = Env::default();
        let input = base_input(&env);
        let r = MasGenerators::trr(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::MAS);
    }

    #[test]
    fn test_generate_mas_form_610() {
        let env = Env::default();
        let input = base_input(&env);
        let r = MasGenerators::form_610(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::MasForm610);
    }

    #[test]
    fn test_generate_mica_casp() {
        let env = Env::default();
        let input = base_input(&env);
        let r = MiCaGenerators::casp(&input, 1_700_000_000);
        assert_eq!(r.authority, RegulatoryAuthority::MiCA);
        assert_eq!(r.format, ReportFormat::MiCACASP);
    }

    #[test]
    fn test_generate_mica_reserve_asset() {
        let env = Env::default();
        let input = base_input(&env);
        let r = MiCaGenerators::reserve_asset(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::MiCAReserveAsset);
    }

    #[test]
    fn test_generate_mica_white_paper() {
        let env = Env::default();
        let input = base_input(&env);
        let r = MiCaGenerators::white_paper(&input, 1_700_000_000);
        assert_eq!(r.format, ReportFormat::MiCAWhitePaper);
    }

    #[test]
    fn test_report_ids_are_content_addressed() {
        let env = Env::default();
        let mut i1 = base_input(&env);
        let mut i2 = base_input(&env);
        i1.period_start = 1_700_000_000;
        i2.period_start = 1_700_100_000; // different period
        let r1 = FinraGenerators::oats(&i1, 1_700_000_000);
        let r2 = FinraGenerators::oats(&i2, 1_700_100_000);
        assert_ne!(r1.id, r2.id);
    }

    // ─────────────────────────────────────────────────────────────────────
    // §3  Validation — pass cases (all 7 authorities)
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_validate_finra_oats_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed, "Expected pass; errors: {}", r.error_count);
    }

    #[test]
    fn test_validate_finra_cat_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraCAT);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_sec_form_adv_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormADV);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_sec_form_pf_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormPF);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_sec_form_13f_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::SEC, ReportFormat::SecForm13F);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_cftc_large_trader_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcLargeTrader);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_cftc_swap_data_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcSwapData);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_fca_mifid_ii_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_fca_emir_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaEMIR);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_bafin_wphg_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::BaFin, ReportFormat::BaFinWpHG);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_bafin_anacredit_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::BaFin, ReportFormat::BaFinAnaCredit);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_mas_trr_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::MAS, ReportFormat::MasTRR);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_mas_form_610_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::MAS, ReportFormat::MasForm610);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_mica_casp_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCACASP);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_mica_reserve_asset_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCAReserveAsset);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    #[test]
    fn test_validate_mica_white_paper_pass() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCAWhitePaper);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(r.passed);
    }

    // ─────────────────────────────────────────────────────────────────────
    // §4  Validation — failure cases
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_validation_fails_short_lei() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.lei = Bytes::from_slice(&env, b"SHORT");
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed);
        assert!(r.error_count >= 1);
    }

    #[test]
    fn test_validation_fails_non_alphanumeric_lei() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.lei = Bytes::from_slice(&env, b"HWUPKR0MPOU8LEY!WAT0");
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed);
    }

    #[test]
    fn test_validation_fails_invalid_period() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormADV);
        report.period_start = 2_000_000_000;
        report.period_end   = 1_000_000_000; // end before start
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed);
    }

    #[test]
    fn test_validation_fails_empty_content() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        report.content = Bytes::new(&env);
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed);
    }

    #[test]
    fn test_validation_fails_missing_required_field() {
        let env = Env::default();
        // CFTC LargeTrader without special_account
        let mut report = valid_report_for(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcLargeTrader);
        // Replace content with missing field
        let mut bad = Bytes::new(&env);
        bad.extend_from_slice(b"authority=CFTC\ncommodity=WTI\nposition_long=500\nposition_short=200\n");
        report.content = bad;
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed);
        assert!(r.error_count >= 1);
    }

    #[test]
    fn test_validation_deadline_before_period_end_adds_warning() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        // Deadline == period_start (technically passes the deadline >= period_end check, but
        // deadline < period_end would cause a warning)
        report.deadline    = 1_700_000_001; // just after start but before end
        report.period_end  = 1_700_086_400;
        // deadline < period_end → warning only (not an error in this layer)
        // But deadline < period_end triggers the cross-field warning from validate_deadline
        // In validate_deadline we error if deadline < period_end
        let r = validate_report(&env, &report, 1_700_000_100);
        assert!(!r.passed); // error because deadline < period_end
    }

    // ─────────────────────────────────────────────────────────────────────
    // §5  Submission lifecycle
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_full_happy_path_finra_oats() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        let config = authority_config(&env, RegulatoryAuthority::FINRA);
        let now = 1_700_000_100u64;

        // Step 1: validate
        let vr = validate_report(&env, &report, now);
        assert!(vr.passed);
        mark_validated(&mut report, now).unwrap();
        assert_eq!(report.status, ReportStatus::Validated);

        // Step 2: submit
        let sub = create_submission(&env, &report, 1, &config, now).unwrap();
        mark_submitted(&mut report, now).unwrap();
        assert_eq!(sub.attempt, 1);
        assert_eq!(report.status, ReportStatus::Submitted);

        // Step 3: ingest acceptance
        let ack = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-001"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            now + 100,
        ).unwrap();
        assert!(ack.accepted);

        // Step 4: apply to report
        let action = apply_acknowledgment(&mut report, &ack, now + 100).unwrap();
        assert_eq!(report.status, ReportStatus::Accepted);
        assert_eq!(action, ReportAction::Accepted);
    }

    #[test]
    fn test_rejection_then_resubmit_mica_casp() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCACASP);
        let config = authority_config(&env, RegulatoryAuthority::MiCA);
        let now = 1_700_000_100u64;

        // Validate and submit attempt 1
        mark_validated(&mut report, now).unwrap();
        let sub1 = create_submission(&env, &report, 1, &config, now).unwrap();
        mark_submitted(&mut report, now).unwrap();

        // Rejection
        let ack_rej = ingest_acknowledgment(
            &env,
            &sub1,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-ERR"),
            false,
            Bytes::from_slice(&env, b"Missing service_type"),
            Vec::new(&env),
            now + 100,
        ).unwrap();
        let action = apply_acknowledgment(&mut report, &ack_rej, now + 100).unwrap();
        assert_eq!(action, ReportAction::Rejected);
        assert_eq!(report.status, ReportStatus::Rejected);

        // Retry — get next attempt number
        let attempt2 = next_attempt(1, &config).unwrap();
        assert_eq!(attempt2, 2);

        // Fix content and submit attempt 2
        report.content.extend_from_slice(b"service_type=custody\n");
        let sub2 = create_submission(&env, &report, attempt2, &config, now + 200).unwrap();
        mark_submitted(&mut report, now + 200).unwrap();
        assert_eq!(sub2.attempt, 2);
        assert_eq!(report.status, ReportStatus::Submitted);
    }

    #[test]
    fn test_max_retries_exceeded() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormADV);
        let config = authority_config(&env, RegulatoryAuthority::SEC); // max_retries=3
        // Attempt 4 exceeds max
        let result = create_submission(&env, &report, 4, &config, 1_700_000_100);
        assert!(matches!(result, Err(ReportingError::MaxRetriesExceeded)));
    }

    #[test]
    fn test_submission_deadline_exceeded() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcSwapData);
        let config = authority_config(&env, RegulatoryAuthority::CFTC);
        let result = create_submission(&env, &report, 1, &config, 2_000_000_000); // past deadline
        assert!(matches!(result, Err(ReportingError::DeadlineExceeded)));
    }

    #[test]
    fn test_disabled_authority_blocks_submission() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::BaFin, ReportFormat::BaFinWpHG);
        let mut config = authority_config(&env, RegulatoryAuthority::BaFin);
        config.enabled = false;
        let result = create_submission(&env, &report, 1, &config, 1_700_000_100);
        assert!(matches!(result, Err(ReportingError::AuthorityDisabled)));
    }

    // ─────────────────────────────────────────────────────────────────────
    // §6  State machine transition guards
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_state_machine_full_valid_path() {
        assert!(check_transition(ReportStatus::Draft,        ReportStatus::Validated).is_ok());
        assert!(check_transition(ReportStatus::Validated,    ReportStatus::Submitted).is_ok());
        assert!(check_transition(ReportStatus::Submitted,    ReportStatus::Acknowledged).is_ok());
        assert!(check_transition(ReportStatus::Acknowledged, ReportStatus::Accepted).is_ok());
    }

    #[test]
    fn test_state_machine_terminal_states_block_all() {
        for terminal in [ReportStatus::Accepted, ReportStatus::Cancelled, ReportStatus::Overdue] {
            for next in [
                ReportStatus::Draft, ReportStatus::Validated, ReportStatus::Submitted,
                ReportStatus::Acknowledged, ReportStatus::Rejected, ReportStatus::Cancelled,
            ] {
                let result = check_transition(terminal, next);
                assert!(result.is_err(), "{:?} → {:?} should be disallowed", terminal, next);
            }
        }
    }

    #[test]
    fn test_state_machine_skip_validated_fails() {
        assert!(check_transition(ReportStatus::Draft, ReportStatus::Submitted).is_err());
    }

    #[test]
    fn test_state_machine_rejected_allows_resubmit() {
        assert!(check_transition(ReportStatus::Rejected, ReportStatus::Submitted).is_ok());
    }

    // ─────────────────────────────────────────────────────────────────────
    // §7  Deadline and overdue
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_is_overdue_before_deadline_false() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        assert!(!is_overdue(&report, 1_700_000_100));
    }

    #[test]
    fn test_is_overdue_after_deadline_true() {
        let env = Env::default();
        let report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        assert!(is_overdue(&report, 1_800_000_000));
    }

    #[test]
    fn test_is_overdue_accepted_report_not_overdue() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        report.status = ReportStatus::Accepted;
        assert!(!is_overdue(&report, 1_800_000_000)); // terminal state
    }

    #[test]
    fn test_mark_overdue_from_submitted() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::MAS, ReportFormat::MasTRR);
        report.status = ReportStatus::Submitted;
        mark_overdue(&mut report, 1_800_000_000).unwrap();
        assert_eq!(report.status, ReportStatus::Overdue);
    }

    #[test]
    fn test_cancel_from_validated() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::BaFin, ReportFormat::BaFinAnaCredit);
        mark_validated(&mut report, 1_700_000_100).unwrap();
        cancel_report(&mut report, 1_700_000_200).unwrap();
        assert_eq!(report.status, ReportStatus::Cancelled);
    }

    // ─────────────────────────────────────────────────────────────────────
    // §8  Retry back-off
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_linear_retry_delay() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.status = ReportStatus::Validated;
        let mut config = authority_config(&env, RegulatoryAuthority::FINRA);
        config.exponential_backoff = false;
        config.retry_delay_seconds = 60;
        let now = 1_700_000_100u64;
        let sub = create_submission(&env, &report, 2, &config, now).unwrap();
        assert_eq!(sub.retry_after, now + 60);
    }

    #[test]
    fn test_exponential_retry_delay_attempt_3() {
        let env = Env::default();
        let mut report = valid_report_for(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.status = ReportStatus::Validated;
        let config = authority_config(&env, RegulatoryAuthority::FINRA); // backoff=true, delay=30s
        let now = 1_700_000_100u64;
        // attempt 3: 2^(3-1) * 30 = 4 * 30 = 120
        let sub = create_submission(&env, &report, 3, &config, now).unwrap();
        assert_eq!(sub.retry_after, now + 120);
    }

    #[test]
    fn test_next_attempt_within_limit() {
        let env = Env::default();
        let config = authority_config(&env, RegulatoryAuthority::SEC); // max=3
        assert_eq!(next_attempt(1, &config).unwrap(), 2);
        assert_eq!(next_attempt(2, &config).unwrap(), 3);
    }

    #[test]
    fn test_next_attempt_at_limit_fails() {
        let env = Env::default();
        let config = authority_config(&env, RegulatoryAuthority::SEC);
        assert!(next_attempt(3, &config).is_err());
    }

    // ─────────────────────────────────────────────────────────────────────
    // §9  Acknowledgment tracking
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_ack_correlates_report_and_submission() {
        let env = Env::default();
        let sub = RegulatorySubmission {
            id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: BytesN::from_array(&env, &[1u8; 32]),
            attempt: 1,
            submitted_at: 1_700_000_100,
            endpoint: Bytes::from_slice(&env, b"https://x"),
            reference_number: None,
            response_code: 200,
            response_payload: Bytes::new(&env),
            status: ReportStatus::Submitted,
            retry_eligible: true,
            retry_after: 0,
        };
        let ack = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"ok"),
            Bytes::from_slice(&env, b"REF-999"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        ).unwrap();
        assert_eq!(ack.submission_id, sub.id);
        assert_eq!(ack.report_id, sub.report_id);
        assert_eq!(ack.reference_number, Bytes::from_slice(&env, b"REF-999"));
    }

    #[test]
    fn test_empty_reference_number_fails_ack() {
        let env = Env::default();
        let sub = RegulatorySubmission {
            id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: BytesN::from_array(&env, &[1u8; 32]),
            attempt: 1,
            submitted_at: 1_700_000_100,
            endpoint: Bytes::from_slice(&env, b"https://x"),
            reference_number: None,
            response_code: 200,
            response_payload: Bytes::new(&env),
            status: ReportStatus::Submitted,
            retry_eligible: true,
            retry_after: 0,
        };
        let result = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"ok"),
            Bytes::new(&env), // empty reference
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        );
        assert!(matches!(result, Err(ReportingError::AcknowledgmentOrphan)));
    }

    #[test]
    fn test_ack_hash_is_deterministic() {
        let env = Env::default();
        let payload = Bytes::from_slice(&env, b"{\"status\":\"accepted\"}");
        let h1 = crate::reporting_audit_trail::compute_entry_hash(
            &env,
            &BytesN::from_array(&env, &[1u8; 32]),
            ReportAction::Accepted,
            0,
            1_700_000_000,
            &payload,
            &zero_hash(&env),
        );
        let h2 = crate::reporting_audit_trail::compute_entry_hash(
            &env,
            &BytesN::from_array(&env, &[1u8; 32]),
            ReportAction::Accepted,
            0,
            1_700_000_000,
            &payload,
            &zero_hash(&env),
        );
        assert_eq!(h1, h2, "Same inputs must produce the same hash");
    }

    // ─────────────────────────────────────────────────────────────────────
    // §10  Audit trail integrity
    // ─────────────────────────────────────────────────────────────────────

    fn run_full_pipeline_trail(env: &Env) -> Vec<crate::regulatory_reporting::ReportingAuditEntry> {
        let mut trail = Vec::new(env);
        let rid = BytesN::from_array(env, &[7u8; 32]);
        let actor = Address::generate(env);

        record_generated(env, &mut trail, &rid, actor.clone(), Bytes::from_slice(env, b"MiCACASP"), 1_000);
        record_validated(env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        record_submitted(env, &mut trail, &rid, actor.clone(), &BytesN::from_array(env, &[9u8; 32]), 1, 3_000);
        crate::reporting_audit_trail::record_acknowledgment_received(
            env, &mut trail, &rid, actor.clone(),
            &BytesN::from_array(env, &[11u8; 32]), true, 4_000,
        );
        record_accepted(env, &mut trail, &rid, actor.clone(), Bytes::from_slice(env, b"REF-XYZ"), 5_000);
        trail
    }

    #[test]
    fn test_audit_trail_has_correct_length() {
        let env = Env::default();
        let trail = run_full_pipeline_trail(&env);
        assert_eq!(trail.len(), 5);
    }

    #[test]
    fn test_audit_trail_hash_chain_valid() {
        let env = Env::default();
        let trail = run_full_pipeline_trail(&env);
        assert!(verify_trail(&env, &trail).is_ok());
    }

    #[test]
    fn test_audit_trail_first_entry_action_generated() {
        let env = Env::default();
        let trail = run_full_pipeline_trail(&env);
        assert_eq!(trail.get(0).unwrap().action, ReportAction::Generated);
    }

    #[test]
    fn test_audit_trail_last_entry_action_accepted() {
        let env = Env::default();
        let trail = run_full_pipeline_trail(&env);
        let last = trail.get(trail.len() - 1).unwrap();
        assert_eq!(last.action, ReportAction::Accepted);
        assert_eq!(last.resulting_status, ReportStatus::Accepted);
    }

    #[test]
    fn test_audit_trail_tamper_detected() {
        let env = Env::default();
        let mut trail = run_full_pipeline_trail(&env);
        // Corrupt entry 2's hash
        let mut e = trail.get(2).unwrap();
        e.entry_hash = BytesN::from_array(&env, &[0xAAu8; 32]);
        trail.set(2, e);
        assert!(verify_trail(&env, &trail).is_err());
    }

    #[test]
    fn test_audit_trail_cancelled_pipeline() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = BytesN::from_array(&env, &[8u8; 32]);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"BaFinWpHG"), 1_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), false, 2, 2_000);
        record_cancelled(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"Operator request"), 3_000);

        assert_eq!(trail.len(), 3);
        assert!(verify_trail(&env, &trail).is_ok());
        let last = trail.get(2).unwrap();
        assert_eq!(last.resulting_status, ReportStatus::Cancelled);
    }

    #[test]
    fn test_audit_trail_overdue_pipeline() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = BytesN::from_array(&env, &[9u8; 32]);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"CFTC"), 1_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        record_overdue(&env, &mut trail, &rid, actor.clone(), 1_700_172_800, 1_800_000_000);

        assert!(verify_trail(&env, &trail).is_ok());
        assert_eq!(trail.get(2).unwrap().resulting_status, ReportStatus::Overdue);
    }

    #[test]
    fn test_audit_trail_rejection_resubmit_pipeline() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = BytesN::from_array(&env, &[99u8; 32]);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"FINRA-OATS"), 1_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[5u8; 32]), 1, 3_000);
        record_rejected(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"Bad MPID"), Vec::new(&env), 4_000);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[6u8; 32]), 2, 5_000);
        crate::reporting_audit_trail::record_acknowledgment_received(
            &env, &mut trail, &rid, actor.clone(),
            &BytesN::from_array(&env, &[12u8; 32]), true, 6_000,
        );
        record_accepted(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"REF-222"), 7_000);

        assert_eq!(trail.len(), 7);
        assert!(verify_trail(&env, &trail).is_ok());
    }

    // ─────────────────────────────────────────────────────────────────────
    // §11  ReportStatus helpers
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_terminal_status_set() {
        assert!(ReportStatus::Accepted.is_terminal());
        assert!(ReportStatus::Cancelled.is_terminal());
        assert!(ReportStatus::Overdue.is_terminal());
    }

    #[test]
    fn test_non_terminal_statuses() {
        assert!(!ReportStatus::Draft.is_terminal());
        assert!(!ReportStatus::Validated.is_terminal());
        assert!(!ReportStatus::Submitted.is_terminal());
        assert!(!ReportStatus::Acknowledged.is_terminal());
        assert!(!ReportStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_only_rejected_is_retryable() {
        assert!(ReportStatus::Rejected.is_retryable());
        assert!(!ReportStatus::Accepted.is_retryable());
        assert!(!ReportStatus::Submitted.is_retryable());
        assert!(!ReportStatus::Draft.is_retryable());
    }
}
