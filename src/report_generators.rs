//! Per-Regulator Report Generators
//!
//! Each generator is responsible for:
//! 1. Accepting the relevant source data for a reporting period.
//! 2. Assembling the authority-specific content payload.
//! 3. Computing field-level hashes for the content.
//! 4. Returning a fully-populated `RegulatoryReport` in `Draft` status.
//!
//! No network I/O happens here — generators are pure data-assembly functions.
//! The submission layer (`submission_tracker`) handles dispatch.

use soroban_sdk::{Bytes, BytesN, Env, Vec};

use crate::regulatory_reporting::{
    AuthorityConfig, RegulatoryAuthority, RegulatoryReport, ReportFormat, ReportStatus,
    ValidationResult,
};

// ─────────────────────────────────────────────────────────────────────────────
// Generator Input — shared across all authorities
// ─────────────────────────────────────────────────────────────────────────────

/// Common inputs that every report generator receives.
///
/// Authority-specific fields are encoded in `extra_fields` as key=value
/// pairs (UTF-8 bytes, `\n`-separated) so that the core struct stays stable.
pub struct ReportInput<'a> {
    pub env: &'a Env,
    /// Reporting entity's on-chain address.
    pub entity: soroban_sdk::Address,
    /// Legal Entity Identifier (20 chars, ISO 17442).
    pub lei: Bytes,
    /// Period start — Unix seconds.
    pub period_start: u64,
    /// Period end — Unix seconds.
    pub period_end: u64,
    /// Deadline for submission — Unix seconds.
    pub deadline: u64,
    /// IDs of on-chain audit events used as source data.
    pub source_event_ids: Vec<BytesN<32>>,
    /// Authority-specific key=value fields, `\n`-delimited.
    pub extra_fields: Bytes,
    /// Hash of the previous report in this authority+entity chain.
    pub prev_report_hash: BytesN<32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a deterministic report ID: SHA-256 of
/// `authority(u32 LE) || format(u32 LE) || entity_bytes || period_start(u64 LE) || period_end(u64 LE)`.
pub fn compute_report_id(
    env: &Env,
    authority: RegulatoryAuthority,
    format: ReportFormat,
    entity_bytes: &Bytes,
    period_start: u64,
    period_end: u64,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.extend_from_array(&(authority as u32).to_le_bytes());
    buf.extend_from_array(&(format as u32).to_le_bytes());
    buf.append(entity_bytes);
    buf.extend_from_array(&period_start.to_le_bytes());
    buf.extend_from_array(&period_end.to_le_bytes());
    env.crypto().sha256(&buf)
}

/// Compute report content hash: SHA-256 of `prev_report_hash || content`.
pub fn compute_report_hash(env: &Env, prev_report_hash: &BytesN<32>, content: &Bytes) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, prev_report_hash.as_ref()));
    buf.append(content);
    env.crypto().sha256(&buf)
}

/// Build an empty `ValidationResult` for a freshly generated report.
fn empty_validation(env: &Env, now: u64) -> ValidationResult {
    ValidationResult {
        passed: false,
        error_count: 0,
        warning_count: 0,
        errors: Vec::new(env),
        warnings: Vec::new(env),
        validated_at: now,
    }
}

/// Encode a KV tag as `key=value\n` bytes.
fn kv(env: &Env, key: &[u8], value: &[u8]) -> Bytes {
    let mut b = Bytes::new(env);
    b.extend_from_slice(key);
    b.extend_from_slice(b"=");
    b.extend_from_slice(value);
    b.extend_from_slice(b"\n");
    b
}

// ─────────────────────────────────────────────────────────────────────────────
// FINRA Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct FinraGenerators;

impl FinraGenerators {
    /// Generate a FINRA Order Audit Trail System (OATS) report.
    ///
    /// Required extra_fields keys:
    /// - `mpid`         — Market Participant Identifier (4 chars)
    /// - `order_count`  — total orders in period
    /// - `route_count`  — total order routes in period
    pub fn oats(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"FINRA"));
        content.append(&kv(env, b"form", b"OATS"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        // period_start encoded as raw LE bytes
        let mut ps = Bytes::new(env);
        ps.extend_from_slice(b"period_start=");
        ps.extend_from_array(&input.period_start.to_le_bytes());
        ps.extend_from_slice(b"\n");
        content.append(&ps);
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]); // placeholder; real impl passes address bytes
        let id = compute_report_id(
            env,
            RegulatoryAuthority::FINRA,
            ReportFormat::FinraOATS,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::FINRA,
            format: ReportFormat::FinraOATS,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a FINRA Consolidated Audit Trail (CAT) report.
    ///
    /// Required extra_fields keys: `mpid`, `cat_reporter_id`, `event_count`.
    pub fn cat(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"FINRA"));
        content.append(&kv(env, b"form", b"CAT"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::FINRA,
            ReportFormat::FinraCAT,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::FINRA,
            format: ReportFormat::FinraCAT,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a FINRA Rule 4370 Business Continuity Plan report.
    pub fn rule_4370(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"FINRA"));
        content.append(&kv(env, b"form", b"Rule4370"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::FINRA,
            ReportFormat::FinraRule4370,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::FINRA,
            format: ReportFormat::FinraRule4370,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SEC Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct SecGenerators;

impl SecGenerators {
    /// Generate an SEC Form ADV report.
    ///
    /// Required extra_fields: `adviser_name`, `aum_usd`, `client_count`.
    pub fn form_adv(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"SEC"));
        content.append(&kv(env, b"form", b"ADV"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::SEC,
            ReportFormat::SecFormADV,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::SEC,
            format: ReportFormat::SecFormADV,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 3,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate an SEC Form PF (Private Fund) report.
    ///
    /// Required extra_fields: `fund_count`, `nav_usd`, `strategy_type`.
    pub fn form_pf(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"SEC"));
        content.append(&kv(env, b"form", b"PF"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::SEC,
            ReportFormat::SecFormPF,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::SEC,
            format: ReportFormat::SecFormPF,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate an SEC Form 13F (Institutional Holdings) report.
    ///
    /// Required extra_fields: `cusip_count`, `total_value_usd`, `confidential_treatment`.
    pub fn form_13f(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"SEC"));
        content.append(&kv(env, b"form", b"13F"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::SEC,
            ReportFormat::SecForm13F,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::SEC,
            format: ReportFormat::SecForm13F,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CFTC Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct CftcGenerators;

impl CftcGenerators {
    /// Generate a CFTC Large Trader report.
    ///
    /// Required extra_fields: `commodity`, `position_long`, `position_short`, `special_account`.
    pub fn large_trader(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"CFTC"));
        content.append(&kv(env, b"form", b"LargeTrader"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::CFTC,
            ReportFormat::CftcLargeTrader,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::CFTC,
            format: ReportFormat::CftcLargeTrader,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a CFTC Swap Data Repository (SDR) report.
    ///
    /// Required extra_fields: `swap_type`, `notional_usd`, `counterparty_lei`, `uti`.
    pub fn swap_data(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"CFTC"));
        content.append(&kv(env, b"form", b"SwapData"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::CFTC,
            ReportFormat::CftcSwapData,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::CFTC,
            format: ReportFormat::CftcSwapData,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 3,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FCA Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct FcaGenerators;

impl FcaGenerators {
    /// Generate an FCA MiFID II Transaction Report.
    ///
    /// Required extra_fields: `isin`, `quantity`, `price`, `venue_mic`, `executing_entity_id`.
    pub fn mifid_ii(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"FCA"));
        content.append(&kv(env, b"form", b"MiFIDII"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::FCA,
            ReportFormat::FcaMiFIDII,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::FCA,
            format: ReportFormat::FcaMiFIDII,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate an FCA EMIR trade repository report.
    ///
    /// Required extra_fields: `trade_id`, `asset_class`, `notional_eur`, `counterparty_lei`.
    pub fn emir(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"FCA"));
        content.append(&kv(env, b"form", b"EMIR"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::FCA,
            ReportFormat::FcaEMIR,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::FCA,
            format: ReportFormat::FcaEMIR,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 3,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BaFin Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct BaFinGenerators;

impl BaFinGenerators {
    /// Generate a BaFin WpHG (Securities Trading Act) report.
    ///
    /// Required extra_fields: `isin`, `voting_rights_pct`, `threshold_crossed`, `direction`.
    pub fn wphg(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"BaFin"));
        content.append(&kv(env, b"form", b"WpHG"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::BaFin,
            ReportFormat::BaFinWpHG,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::BaFin,
            format: ReportFormat::BaFinWpHG,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a BaFin AnaCredit report.
    ///
    /// Required extra_fields: `loan_count`, `total_exposure_eur`, `credit_facility_type`.
    pub fn anacredit(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"BaFin"));
        content.append(&kv(env, b"form", b"AnaCredit"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::BaFin,
            ReportFormat::BaFinAnaCredit,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::BaFin,
            format: ReportFormat::BaFinAnaCredit,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MAS Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct MasGenerators;

impl MasGenerators {
    /// Generate a MAS Trade Repository Report (TRR).
    ///
    /// Required extra_fields: `product_type`, `notional_sgd`, `counterparty_lei`, `uti`.
    pub fn trr(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"MAS"));
        content.append(&kv(env, b"form", b"TRR"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::MAS,
            ReportFormat::MasTRR,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::MAS,
            format: ReportFormat::MasTRR,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a MAS Form 610 statistical return.
    ///
    /// Required extra_fields: `balance_sheet_total_sgd`, `loan_book_sgd`, `npl_ratio`.
    pub fn form_610(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"MAS"));
        content.append(&kv(env, b"form", b"Form610"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::MAS,
            ReportFormat::MasForm610,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::MAS,
            format: ReportFormat::MasForm610,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 2,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MiCA Generators
// ─────────────────────────────────────────────────────────────────────────────

pub struct MiCaGenerators;

impl MiCaGenerators {
    /// Generate a MiCA Crypto-Asset Service Provider (CASP) report.
    ///
    /// Required extra_fields: `service_type`, `user_count`, `transaction_volume_eur`, `countries_served`.
    pub fn casp(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"MiCA"));
        content.append(&kv(env, b"form", b"CASP"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::MiCA,
            ReportFormat::MiCACASP,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::MiCA,
            format: ReportFormat::MiCACASP,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a MiCA Reserve Asset backing report (for ARTs / EMTs).
    ///
    /// Required extra_fields: `token_symbol`, `tokens_outstanding`, `reserve_value_eur`,
    /// `reserve_composition`, `custodian_lei`.
    pub fn reserve_asset(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"MiCA"));
        content.append(&kv(env, b"form", b"ReserveAsset"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::MiCA,
            ReportFormat::MiCAReserveAsset,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::MiCA,
            format: ReportFormat::MiCAReserveAsset,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }

    /// Generate a MiCA White Paper disclosure report.
    ///
    /// Required extra_fields: `asset_name`, `asset_class`, `offer_type`, `issuer_country`.
    pub fn white_paper(input: &ReportInput<'_>, now: u64) -> RegulatoryReport {
        let env = input.env;
        let mut content = Bytes::new(env);
        content.append(&kv(env, b"authority", b"MiCA"));
        content.append(&kv(env, b"form", b"WhitePaper"));
        content.append(&kv(env, b"lei", input.lei.as_ref()));
        content.append(&input.extra_fields);

        let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
        let id = compute_report_id(
            env,
            RegulatoryAuthority::MiCA,
            ReportFormat::MiCAWhitePaper,
            &entity_bytes,
            input.period_start,
            input.period_end,
        );
        let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

        RegulatoryReport {
            id,
            authority: RegulatoryAuthority::MiCA,
            format: ReportFormat::MiCAWhitePaper,
            entity: input.entity.clone(),
            lei: input.lei.clone(),
            period_start: input.period_start,
            period_end: input.period_end,
            deadline: input.deadline,
            content,
            schema_version: 1,
            status: ReportStatus::Draft,
            created_at: now,
            updated_at: now,
            last_validation: Some(empty_validation(env, now)),
            prev_report_hash: input.prev_report_hash.clone(),
            report_hash,
            source_event_ids: input.source_event_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispatch helper
// ─────────────────────────────────────────────────────────────────────────────

/// Generate the appropriate report given an authority config and input.
///
/// Dispatches to the authority-specific generator that matches
/// `config.authority` and the `format` argument.
pub fn generate_report(
    format: ReportFormat,
    input: &ReportInput<'_>,
    _config: &AuthorityConfig,
    now: u64,
) -> RegulatoryReport {
    match format {
        // FINRA
        ReportFormat::FinraOATS => FinraGenerators::oats(input, now),
        ReportFormat::FinraCAT => FinraGenerators::cat(input, now),
        ReportFormat::FinraRule4370 => FinraGenerators::rule_4370(input, now),
        // SEC
        ReportFormat::SecFormADV => SecGenerators::form_adv(input, now),
        ReportFormat::SecFormPF => SecGenerators::form_pf(input, now),
        ReportFormat::SecForm13F => SecGenerators::form_13f(input, now),
        // CFTC
        ReportFormat::CftcLargeTrader => CftcGenerators::large_trader(input, now),
        ReportFormat::CftcSwapData => CftcGenerators::swap_data(input, now),
        // FCA
        ReportFormat::FcaMiFIDII => FcaGenerators::mifid_ii(input, now),
        ReportFormat::FcaEMIR => FcaGenerators::emir(input, now),
        // BaFin
        ReportFormat::BaFinWpHG => BaFinGenerators::wphg(input, now),
        ReportFormat::BaFinAnaCredit => BaFinGenerators::anacredit(input, now),
        // MAS
        ReportFormat::MasTRR => MasGenerators::trr(input, now),
        ReportFormat::MasForm610 => MasGenerators::form_610(input, now),
        // MiCA
        ReportFormat::MiCACASP => MiCaGenerators::casp(input, now),
        ReportFormat::MiCAReserveAsset => MiCaGenerators::reserve_asset(input, now),
        ReportFormat::MiCAWhitePaper => MiCaGenerators::white_paper(input, now),
        // For formats not explicitly handled, fall back to a generic CASP-style report
        _ => {
            let env = input.env;
            let mut content = Bytes::new(env);
            content.append(&kv(env, b"form", b"Generic"));
            content.append(&kv(env, b"lei", input.lei.as_ref()));
            content.append(&input.extra_fields);

            let entity_bytes = Bytes::from_slice(env, &[0u8; 32]);
            let id = compute_report_id(
                env,
                _config.authority,
                format,
                &entity_bytes,
                input.period_start,
                input.period_end,
            );
            let report_hash = compute_report_hash(env, &input.prev_report_hash, &content);

            RegulatoryReport {
                id,
                authority: _config.authority,
                format,
                entity: input.entity.clone(),
                lei: input.lei.clone(),
                period_start: input.period_start,
                period_end: input.period_end,
                deadline: input.deadline,
                content,
                schema_version: 1,
                status: ReportStatus::Draft,
                created_at: now,
                updated_at: now,
                last_validation: Some(empty_validation(env, now)),
                prev_report_hash: input.prev_report_hash.clone(),
                report_hash,
                source_event_ids: input.source_event_ids.clone(),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Bytes, BytesN, Env, Vec};

    fn make_input(env: &Env) -> ReportInput<'_> {
        ReportInput {
            env,
            entity: soroban_sdk::Address::generate(env),
            lei: Bytes::from_slice(env, b"HWUPKR0MPOU8LEYPWAT0"),
            period_start: 1_700_000_000,
            period_end:   1_700_086_400,
            deadline:     1_700_172_800,
            source_event_ids: Vec::new(env),
            extra_fields: Bytes::from_slice(env, b"mpid=TEST\norder_count=42\n"),
            prev_report_hash: BytesN::from_array(env, &[0u8; 32]),
        }
    }

    #[test]
    fn test_finra_oats_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = FinraGenerators::oats(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::FINRA);
        assert_eq!(report.format, ReportFormat::FinraOATS);
        assert_eq!(report.status, ReportStatus::Draft);
        assert_eq!(report.schema_version, 1);
    }

    #[test]
    fn test_finra_cat_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = FinraGenerators::cat(&input, 1_700_000_000);
        assert_eq!(report.format, ReportFormat::FinraCAT);
        assert_eq!(report.schema_version, 2);
    }

    #[test]
    fn test_sec_form_adv_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = SecGenerators::form_adv(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::SEC);
        assert_eq!(report.format, ReportFormat::SecFormADV);
    }

    #[test]
    fn test_sec_form_pf_schema_version() {
        let env = Env::default();
        let input = make_input(&env);
        let report = SecGenerators::form_pf(&input, 1_700_000_000);
        assert_eq!(report.schema_version, 2);
    }

    #[test]
    fn test_cftc_large_trader_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = CftcGenerators::large_trader(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::CFTC);
        assert_eq!(report.format, ReportFormat::CftcLargeTrader);
    }

    #[test]
    fn test_cftc_swap_data_schema_version() {
        let env = Env::default();
        let input = make_input(&env);
        let report = CftcGenerators::swap_data(&input, 1_700_000_000);
        assert_eq!(report.schema_version, 3);
    }

    #[test]
    fn test_fca_mifid_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = FcaGenerators::mifid_ii(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::FCA);
    }

    #[test]
    fn test_bafin_wphg_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = BaFinGenerators::wphg(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::BaFin);
        assert_eq!(report.format, ReportFormat::BaFinWpHG);
    }

    #[test]
    fn test_mas_trr_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = MasGenerators::trr(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::MAS);
    }

    #[test]
    fn test_mica_casp_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = MiCaGenerators::casp(&input, 1_700_000_000);
        assert_eq!(report.authority, RegulatoryAuthority::MiCA);
        assert_eq!(report.format, ReportFormat::MiCACASP);
    }

    #[test]
    fn test_mica_reserve_asset_generates_draft() {
        let env = Env::default();
        let input = make_input(&env);
        let report = MiCaGenerators::reserve_asset(&input, 1_700_000_000);
        assert_eq!(report.format, ReportFormat::MiCAReserveAsset);
    }

    #[test]
    fn test_report_ids_differ_across_authorities() {
        let env = Env::default();
        let input = make_input(&env);
        let r1 = FinraGenerators::oats(&input, 1_700_000_000);
        let r2 = SecGenerators::form_adv(&input, 1_700_000_000);
        assert_ne!(r1.id, r2.id, "Different authorities must produce different IDs");
    }

    #[test]
    fn test_report_ids_differ_for_different_periods() {
        let env = Env::default();
        let mut input1 = make_input(&env);
        input1.period_start = 1_700_000_000;
        input1.period_end   = 1_700_086_400;

        let mut input2 = make_input(&env);
        input2.period_start = 1_700_086_400;
        input2.period_end   = 1_700_172_800;

        let r1 = FinraGenerators::oats(&input1, 1_700_000_000);
        let r2 = FinraGenerators::oats(&input2, 1_700_086_400);
        assert_ne!(r1.id, r2.id, "Different periods must produce different IDs");
    }

    #[test]
    fn test_content_contains_authority_tag() {
        let env = Env::default();
        let input = make_input(&env);
        let report = FinraGenerators::oats(&input, 1_700_000_000);
        // Content must be non-empty and start with 'a' (the "authority=" prefix)
        assert!(report.content.len() > 0, "Content must not be empty");
        assert_eq!(report.content.get(0).unwrap(), b'a');
    }
}
