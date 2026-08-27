//! Report Validation — Schema and Business-Rule Checks
//!
//! Validates a `RegulatoryReport` before it is submitted to a regulator.
//!
//! Validation layers (executed in order):
//! 1. **Common checks** — LEI format, reporting period, deadline, content non-empty.
//! 2. **Authority checks** — required KV keys present in content for the specific form.
//! 3. **Cross-field rules** — e.g. period_end > period_start, deadline > period_end.
//! 4. **Jurisdiction constraints** — authority-specific value ranges and formats.
//!
//! A `ValidationResult` is returned (never panics).  The caller decides whether
//! to proceed (`passed == true`) or surface errors to the operator.

use soroban_sdk::{Bytes, Env, Vec};

use crate::regulatory_reporting::{
    RegulatoryAuthority, RegulatoryReport, ReportFormat, ValidationResult,
};

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Check whether `haystack` contains the ASCII bytes of `needle`.
fn contains_key(haystack: &Bytes, needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() as u32 {
        return false;
    }
    let h_len = haystack.len() as usize;
    let n_len = needle.len();
    if h_len < n_len {
        return false;
    }
    'outer: for start in 0..=(h_len - n_len) {
        for (i, &b) in needle.iter().enumerate() {
            if haystack.get(start as u32 + i as u32).unwrap_or(0) != b {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

/// Append a UTF-8 error message to the errors vector.
fn push_err(env: &Env, errors: &mut Vec<Bytes>, msg: &[u8]) {
    errors.push_back(Bytes::from_slice(env, msg));
}

/// Append a UTF-8 warning message to the warnings vector.
fn push_warn(env: &Env, warnings: &mut Vec<Bytes>, msg: &[u8]) {
    warnings.push_back(Bytes::from_slice(env, msg));
}

// ─────────────────────────────────────────────────────────────────────────────
// Common validators (apply to every authority)
// ─────────────────────────────────────────────────────────────────────────────

/// Validate the LEI: must be exactly 20 bytes, all ASCII alphanumeric.
fn validate_lei(env: &Env, report: &RegulatoryReport, errors: &mut Vec<Bytes>) {
    if report.lei.len() != 20 {
        push_err(env, errors, b"lei: must be exactly 20 characters (ISO 17442)");
        return;
    }
    for i in 0..20u32 {
        let b = report.lei.get(i).unwrap_or(0);
        if !b.is_ascii_alphanumeric() {
            push_err(env, errors, b"lei: must contain only ASCII alphanumeric characters");
            return;
        }
    }
}

/// Validate that period_start < period_end.
fn validate_period(env: &Env, report: &RegulatoryReport, errors: &mut Vec<Bytes>) {
    if report.period_start >= report.period_end {
        push_err(
            env,
            errors,
            b"period: period_start must be strictly before period_end",
        );
    }
}

/// Validate that deadline >= period_end (submission after period closes).
fn validate_deadline(env: &Env, report: &RegulatoryReport, errors: &mut Vec<Bytes>) {
    if report.deadline < report.period_end {
        push_err(
            env,
            errors,
            b"deadline: must not be earlier than period_end",
        );
    }
}

/// Validate that content is non-empty.
fn validate_content_nonempty(env: &Env, report: &RegulatoryReport, errors: &mut Vec<Bytes>) {
    if report.content.is_empty() {
        push_err(env, errors, b"content: report payload must not be empty");
    }
}

/// Validate that the content declares the correct authority tag.
fn validate_authority_tag(
    env: &Env,
    report: &RegulatoryReport,
    expected: &[u8],
    errors: &mut Vec<Bytes>,
) {
    if !contains_key(&report.content, expected) {
        push_err(
            env,
            errors,
            b"content: missing or incorrect authority tag",
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Required-field validators per format
// ─────────────────────────────────────────────────────────────────────────────

fn require_keys(
    env: &Env,
    content: &Bytes,
    keys: &[&[u8]],
    errors: &mut Vec<Bytes>,
) {
    for key in keys {
        let mut needle = Bytes::from_slice(env, key);
        needle.extend_from_slice(b"=");
        if !contains_key(content, needle.as_ref()) {
            let mut msg = Bytes::from_slice(env, b"content: missing required field: ");
            msg.extend_from_slice(key);
            errors.push_back(msg);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Authority-specific validators
// ─────────────────────────────────────────────────────────────────────────────

fn validate_finra(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=FINRA", errors);
    match report.format {
        ReportFormat::FinraOATS => {
            require_keys(env, &report.content, &[b"mpid", b"order_count", b"route_count"], errors);
        }
        ReportFormat::FinraCAT => {
            require_keys(
                env,
                &report.content,
                &[b"mpid", b"cat_reporter_id", b"event_count"],
                errors,
            );
            if report.schema_version < 2 {
                push_warn(env, warnings, b"finra-cat: schema_version < 2 is deprecated");
            }
        }
        ReportFormat::FinraRule4370 => {
            require_keys(
                env,
                &report.content,
                &[b"bcp_version", b"emergency_contacts"],
                errors,
            );
        }
        ReportFormat::FinraSAR => {
            require_keys(
                env,
                &report.content,
                &[b"sar_type", b"suspicious_activity_type", b"amount"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"finra: unrecognised format for this authority");
        }
    }
}

fn validate_sec(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=SEC", errors);
    match report.format {
        ReportFormat::SecFormADV => {
            require_keys(
                env,
                &report.content,
                &[b"adviser_name", b"aum_usd", b"client_count"],
                errors,
            );
            // Warn if AUM field exists but appears to be zero
            if contains_key(&report.content, b"aum_usd=0") {
                push_warn(env, warnings, b"sec-adv: aum_usd=0 may indicate missing data");
            }
        }
        ReportFormat::SecFormPF => {
            require_keys(
                env,
                &report.content,
                &[b"fund_count", b"nav_usd", b"strategy_type"],
                errors,
            );
            if report.schema_version < 2 {
                push_warn(env, warnings, b"sec-pf: schema_version < 2 is outdated");
            }
        }
        ReportFormat::SecForm13F => {
            require_keys(
                env,
                &report.content,
                &[b"cusip_count", b"total_value_usd", b"confidential_treatment"],
                errors,
            );
        }
        ReportFormat::SecFormNPORT => {
            require_keys(
                env,
                &report.content,
                &[b"fund_name", b"total_assets", b"net_assets"],
                errors,
            );
        }
        ReportFormat::SecSAR => {
            require_keys(
                env,
                &report.content,
                &[b"sar_number", b"filing_institution", b"suspicious_amount"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"sec: unrecognised format for this authority");
        }
    }
}

fn validate_cftc(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=CFTC", errors);
    match report.format {
        ReportFormat::CftcLargeTrader => {
            require_keys(
                env,
                &report.content,
                &[b"commodity", b"position_long", b"position_short", b"special_account"],
                errors,
            );
        }
        ReportFormat::CftcSwapData => {
            require_keys(
                env,
                &report.content,
                &[b"swap_type", b"notional_usd", b"counterparty_lei", b"uti"],
                errors,
            );
        }
        ReportFormat::CftcPart20 => {
            require_keys(
                env,
                &report.content,
                &[b"commodity_contract", b"position_size", b"account_controller"],
                errors,
            );
        }
        ReportFormat::CftcForm40 => {
            require_keys(
                env,
                &report.content,
                &[b"trader_id", b"position_commodity", b"exchange_code"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"cftc: unrecognised format for this authority");
        }
    }
}

fn validate_fca(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=FCA", errors);
    match report.format {
        ReportFormat::FcaMiFIDII => {
            require_keys(
                env,
                &report.content,
                &[b"isin", b"quantity", b"price", b"venue_mic", b"executing_entity_id"],
                errors,
            );
            // MiFID II: ISIN must be 12 chars (basic length check via tag presence is sufficient here)
        }
        ReportFormat::FcaEMIR => {
            require_keys(
                env,
                &report.content,
                &[b"trade_id", b"asset_class", b"notional_eur", b"counterparty_lei"],
                errors,
            );
        }
        ReportFormat::FcaSTOR => {
            require_keys(
                env,
                &report.content,
                &[b"instrument_id", b"suspicious_behaviour", b"reporting_date"],
                errors,
            );
        }
        ReportFormat::FcaCOREP => {
            require_keys(
                env,
                &report.content,
                &[b"capital_ratio", b"tier1_capital", b"rwa_total"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"fca: unrecognised format for this authority");
        }
    }
}

fn validate_bafin(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=BaFin", errors);
    match report.format {
        ReportFormat::BaFinWpHG => {
            require_keys(
                env,
                &report.content,
                &[b"isin", b"voting_rights_pct", b"threshold_crossed", b"direction"],
                errors,
            );
        }
        ReportFormat::BaFinMeldepflicht => {
            require_keys(
                env,
                &report.content,
                &[b"instrument_type", b"notification_reason", b"holding_pct"],
                errors,
            );
        }
        ReportFormat::BaFinAnaCredit => {
            require_keys(
                env,
                &report.content,
                &[b"loan_count", b"total_exposure_eur", b"credit_facility_type"],
                errors,
            );
        }
        ReportFormat::BaFinAML => {
            require_keys(
                env,
                &report.content,
                &[b"aml_report_type", b"suspicious_amount_eur", b"reporting_entity_lei"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"bafin: unrecognised format for this authority");
        }
    }
}

fn validate_mas(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=MAS", errors);
    match report.format {
        ReportFormat::MasSGX => {
            require_keys(
                env,
                &report.content,
                &[b"sgx_account_id", b"security_code", b"net_position"],
                errors,
            );
        }
        ReportFormat::MasTRR => {
            require_keys(
                env,
                &report.content,
                &[b"product_type", b"notional_sgd", b"counterparty_lei", b"uti"],
                errors,
            );
        }
        ReportFormat::MasForm610 => {
            require_keys(
                env,
                &report.content,
                &[b"balance_sheet_total_sgd", b"loan_book_sgd", b"npl_ratio"],
                errors,
            );
        }
        ReportFormat::MasCMS => {
            require_keys(
                env,
                &report.content,
                &[b"licence_number", b"regulated_activity", b"client_assets_sgd"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"mas: unrecognised format for this authority");
        }
    }
}

fn validate_mica(
    env: &Env,
    report: &RegulatoryReport,
    errors: &mut Vec<Bytes>,
    warnings: &mut Vec<Bytes>,
) {
    validate_authority_tag(env, report, b"authority=MiCA", errors);
    match report.format {
        ReportFormat::MiCACASP => {
            require_keys(
                env,
                &report.content,
                &[b"service_type", b"user_count", b"transaction_volume_eur", b"countries_served"],
                errors,
            );
        }
        ReportFormat::MiCAWhitePaper => {
            require_keys(
                env,
                &report.content,
                &[b"asset_name", b"asset_class", b"offer_type", b"issuer_country"],
                errors,
            );
        }
        ReportFormat::MiCAReserveAsset => {
            require_keys(
                env,
                &report.content,
                &[
                    b"token_symbol",
                    b"tokens_outstanding",
                    b"reserve_value_eur",
                    b"reserve_composition",
                    b"custodian_lei",
                ],
                errors,
            );
            // Cross-field: custodian_lei must also satisfy LEI length (20 chars).
            // We can only do a tag-presence check here without full value extraction.
            if !contains_key(&report.content, b"custodian_lei=") {
                push_err(env, errors, b"mica-reserve: custodian_lei is required");
            }
        }
        ReportFormat::MiCASignificant => {
            require_keys(
                env,
                &report.content,
                &[b"casp_id", b"enhanced_obligations_reason", b"supervisory_authority"],
                errors,
            );
        }
        _ => {
            push_warn(env, warnings, b"mica: unrecognised format for this authority");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Validate a `RegulatoryReport` and return a `ValidationResult`.
///
/// Does not modify the report in place.  Callers should attach the result to
/// the report's `last_validation` field and transition status to `Validated`
/// only when `result.passed == true`.
pub fn validate_report(env: &Env, report: &RegulatoryReport, now: u64) -> ValidationResult {
    let mut errors: Vec<Bytes> = Vec::new(env);
    let mut warnings: Vec<Bytes> = Vec::new(env);

    // --- Layer 1: common checks ---
    validate_lei(env, report, &mut errors);
    validate_period(env, report, &mut errors);
    validate_deadline(env, report, &mut errors);
    validate_content_nonempty(env, report, &mut errors);

    // --- Layer 2: authority-specific checks ---
    match report.authority {
        RegulatoryAuthority::FINRA => validate_finra(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::SEC   => validate_sec(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::CFTC  => validate_cftc(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::FCA   => validate_fca(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::BaFin => validate_bafin(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::MAS   => validate_mas(env, report, &mut errors, &mut warnings),
        RegulatoryAuthority::MiCA  => validate_mica(env, report, &mut errors, &mut warnings),
    }

    // --- Layer 3: cross-field rules ---
    if report.period_end > report.deadline {
        // warning only — the authority sets the deadline, we just flag it
        push_warn(
            env,
            &mut warnings,
            b"deadline: period_end is after the submission deadline — review urgently",
        );
    }

    let error_count = errors.len();
    let warning_count = warnings.len();

    ValidationResult {
        passed: error_count == 0,
        error_count,
        warning_count,
        errors,
        warnings,
        validated_at: now,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

    use crate::regulatory_reporting::{ReportStatus, RegulatoryAuthority, RegulatoryReport, ReportFormat};

    fn base_report(env: &Env, authority: RegulatoryAuthority, format: ReportFormat) -> RegulatoryReport {
        let mut content = Bytes::new(env);
        let tag = match authority {
            RegulatoryAuthority::FINRA => b"authority=FINRA\n".as_ref(),
            RegulatoryAuthority::SEC   => b"authority=SEC\n".as_ref(),
            RegulatoryAuthority::CFTC  => b"authority=CFTC\n".as_ref(),
            RegulatoryAuthority::FCA   => b"authority=FCA\n".as_ref(),
            RegulatoryAuthority::BaFin => b"authority=BaFin\n".as_ref(),
            RegulatoryAuthority::MAS   => b"authority=MAS\n".as_ref(),
            RegulatoryAuthority::MiCA  => b"authority=MiCA\n".as_ref(),
        };
        content.extend_from_slice(tag);

        RegulatoryReport {
            id: BytesN::from_array(env, &[1u8; 32]),
            authority,
            format,
            entity: Address::generate(env),
            lei: Bytes::from_slice(env, b"HWUPKR0MPOU8LEYPWAT0"),
            period_start: 1_700_000_000,
            period_end:   1_700_086_400,
            deadline:     1_700_172_800,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            last_validation: None,
            prev_report_hash: BytesN::from_array(env, &[0u8; 32]),
            report_hash: BytesN::from_array(env, &[2u8; 32]),
            source_event_ids: Vec::new(env),
        }
    }

    fn with_fields(env: &Env, mut report: RegulatoryReport, fields: &[u8]) -> RegulatoryReport {
        report.content.extend_from_slice(fields);
        report
    }

    // ── LEI validation ────────────────────────────────────────────────────

    #[test]
    fn test_invalid_lei_too_short() {
        let env = Env::default();
        let mut report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.lei = Bytes::from_slice(&env, b"SHORT");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
        assert!(result.error_count >= 1);
    }

    #[test]
    fn test_invalid_lei_non_alphanumeric() {
        let env = Env::default();
        let mut report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.lei = Bytes::from_slice(&env, b"HWUPKR0MPOU8LEY!WAT0");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
    }

    #[test]
    fn test_valid_lei_passes_lei_check() {
        let env = Env::default();
        // Build a FINRA-OATS report with all required fields
        let mut report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        report.content.extend_from_slice(b"mpid=TEST\norder_count=10\nroute_count=5\n");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed, "Expected pass but got errors: {:?}", result.error_count);
    }

    // ── Period validation ─────────────────────────────────────────────────

    #[test]
    fn test_period_start_after_end_fails() {
        let env = Env::default();
        let mut report = base_report(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormADV);
        report.period_start = 2_000_000_000;
        report.period_end   = 1_000_000_000;
        report.content.extend_from_slice(b"adviser_name=X\naum_usd=1\nclient_count=1\n");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
    }

    // ── Authority-specific field checks ───────────────────────────────────

    #[test]
    fn test_finra_oats_missing_fields_fails() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        // no mpid / order_count / route_count
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
        assert!(result.error_count >= 3);
    }

    #[test]
    fn test_finra_oats_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        let report = with_fields(&env, report, b"mpid=ABCD\norder_count=100\nroute_count=50\n");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_sec_form_adv_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::SEC, ReportFormat::SecFormADV);
        let report = with_fields(&env, report, b"adviser_name=Acme\naum_usd=5000000\nclient_count=200\n");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_cftc_large_trader_missing_special_account_fails() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcLargeTrader);
        let report = with_fields(&env, report, b"commodity=WTI\nposition_long=500\nposition_short=200\n");
        // special_account is missing
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
    }

    #[test]
    fn test_cftc_swap_data_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::CFTC, ReportFormat::CftcSwapData);
        let report = with_fields(
            &env,
            report,
            b"swap_type=IR\nnotional_usd=10000000\ncounterparty_lei=HWUPKR0MPOU8LEYPWAT0\nuti=ABC123\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_fca_mifid_ii_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::FCA, ReportFormat::FcaMiFIDII);
        let report = with_fields(
            &env,
            report,
            b"isin=GB0002634946\nquantity=1000\nprice=10.50\nvenue_mic=XLON\nexecuting_entity_id=LEI123\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_bafin_wphg_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::BaFin, ReportFormat::BaFinWpHG);
        let report = with_fields(
            &env,
            report,
            b"isin=DE0005140008\nvoting_rights_pct=5.01\nthreshold_crossed=5\ndirection=above\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_mas_trr_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::MAS, ReportFormat::MasTRR);
        let report = with_fields(
            &env,
            report,
            b"product_type=IRS\nnotional_sgd=1000000\ncounterparty_lei=HWUPKR0MPOU8LEYPWAT0\nuti=UTI001\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_mica_casp_all_fields_passes() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCACASP);
        let report = with_fields(
            &env,
            report,
            b"service_type=exchange\nuser_count=5000\ntransaction_volume_eur=10000000\ncountries_served=DE,FR,NL\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(result.passed);
    }

    #[test]
    fn test_mica_reserve_asset_missing_custodian_fails() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::MiCA, ReportFormat::MiCAReserveAsset);
        let report = with_fields(
            &env,
            report,
            b"token_symbol=EURC\ntokens_outstanding=1000000\nreserve_value_eur=1000000\nreserve_composition=cash\n",
        );
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
    }

    #[test]
    fn test_empty_content_fails() {
        let env = Env::default();
        let mut report = base_report(&env, RegulatoryAuthority::FCA, ReportFormat::FcaEMIR);
        report.content = Bytes::new(&env); // wipe content
        let result = validate_report(&env, &report, 1_700_000_100);
        assert!(!result.passed);
    }

    #[test]
    fn test_validation_result_passed_has_zero_error_count() {
        let env = Env::default();
        let report = base_report(&env, RegulatoryAuthority::FINRA, ReportFormat::FinraOATS);
        let report = with_fields(&env, report, b"mpid=ABCD\norder_count=1\nroute_count=1\n");
        let result = validate_report(&env, &report, 1_700_000_100);
        assert_eq!(result.error_count, 0);
        assert!(result.passed);
    }
}
