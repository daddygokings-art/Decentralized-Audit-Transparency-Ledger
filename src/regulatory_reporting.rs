//! Automated Regulatory Reporting — Core Data Types
//!
//! Provides data structures and enums for the full regulatory reporting pipeline:
//! - Supported regulatory authorities (FINRA, SEC, CFTC, FCA, BaFin, MAS, MiCA)
//! - Report formats and schemas per authority
//! - Submission status state machine
//! - Acknowledgment and reference tracking
//! - Immutable audit trail entry types

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ─────────────────────────────────────────────────────────────────────────────
// Regulatory Authority
// ─────────────────────────────────────────────────────────────────────────────

/// Supported regulatory authorities.
///
/// Each variant corresponds to a distinct filing regime with its own
/// report formats, deadlines, and submission endpoint conventions.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegulatoryAuthority {
    /// Financial Industry Regulatory Authority (US)
    FINRA = 0,
    /// Securities and Exchange Commission (US)
    SEC = 1,
    /// Commodity Futures Trading Commission (US)
    CFTC = 2,
    /// Financial Conduct Authority (UK)
    FCA = 3,
    /// Bundesanstalt für Finanzdienstleistungsaufsicht (Germany)
    BaFin = 4,
    /// Monetary Authority of Singapore
    MAS = 5,
    /// Markets in Crypto-Assets Regulation (EU)
    MiCA = 6,
}

impl RegulatoryAuthority {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            RegulatoryAuthority::FINRA => "FINRA",
            RegulatoryAuthority::SEC => "SEC",
            RegulatoryAuthority::CFTC => "CFTC",
            RegulatoryAuthority::FCA => "FCA",
            RegulatoryAuthority::BaFin => "BaFin",
            RegulatoryAuthority::MAS => "MAS",
            RegulatoryAuthority::MiCA => "MiCA",
        }
    }

    /// ISO 3166-1 jurisdiction code.
    pub fn jurisdiction(&self) -> &'static str {
        match self {
            RegulatoryAuthority::FINRA => "US",
            RegulatoryAuthority::SEC => "US",
            RegulatoryAuthority::CFTC => "US",
            RegulatoryAuthority::FCA => "GB",
            RegulatoryAuthority::BaFin => "DE",
            RegulatoryAuthority::MAS => "SG",
            RegulatoryAuthority::MiCA => "EU",
        }
    }

    /// Returns all supported authorities as an array for iteration.
    pub fn all() -> [RegulatoryAuthority; 7] {
        [
            RegulatoryAuthority::FINRA,
            RegulatoryAuthority::SEC,
            RegulatoryAuthority::CFTC,
            RegulatoryAuthority::FCA,
            RegulatoryAuthority::BaFin,
            RegulatoryAuthority::MAS,
            RegulatoryAuthority::MiCA,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Report Format
// ─────────────────────────────────────────────────────────────────────────────

/// Report form / schema identifier.
///
/// Each variant maps to an official filing form. The naming follows the
/// authority's own designations where possible.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReportFormat {
    // ── FINRA ──────────────────────────────────────────────────────────────
    /// Order Audit Trail System report
    FinraOATS = 0,
    /// Consolidated Audit Trail submission
    FinraCAT = 1,
    /// Business Continuity Plan (Rule 4370)
    FinraRule4370 = 2,
    /// Suspicious Activity Report
    FinraSAR = 3,

    // ── SEC ────────────────────────────────────────────────────────────────
    /// Form ADV — Investment Adviser Registration
    SecFormADV = 10,
    /// Form PF — Reporting for Investment Advisers to Private Funds
    SecFormPF = 11,
    /// Form 13F — Institutional Investment Manager Holdings
    SecForm13F = 12,
    /// Suspicious Activity Report (FinCEN SAR)
    SecSAR = 13,
    /// Form N-PORT — Monthly Portfolio Investments
    SecFormNPORT = 14,

    // ── CFTC ───────────────────────────────────────────────────────────────
    /// Large Trader Reporting
    CftcLargeTrader = 20,
    /// Swap Data Report (SDR submission)
    CftcSwapData = 21,
    /// Part 20 — Large Trader Swaps
    CftcPart20 = 22,
    /// Form 40 — Statement of Reporting Trader
    CftcForm40 = 23,

    // ── FCA ────────────────────────────────────────────────────────────────
    /// MiFID II Transaction Report
    FcaMiFIDII = 30,
    /// EMIR Trade Repository Report
    FcaEMIR = 31,
    /// Suspicious Transaction and Order Report (STOR)
    FcaSTOR = 32,
    /// Regulatory Capital Reporting (COREP)
    FcaCOREP = 33,

    // ── BaFin ──────────────────────────────────────────────────────────────
    /// Securities Trading Act reporting (WpHG)
    BaFinWpHG = 40,
    /// Notification obligation (Meldepflicht)
    BaFinMeldepflicht = 41,
    /// AnaCredit credit data reporting
    BaFinAnaCredit = 42,
    /// Anti-money laundering suspicious activity
    BaFinAML = 43,

    // ── MAS ────────────────────────────────────────────────────────────────
    /// SGX market position reporting
    MasSGX = 50,
    /// Trade Repository Report
    MasTRR = 51,
    /// MAS Form 610 — Banks statistical return
    MasForm610 = 52,
    /// Capital Markets Services licence disclosure
    MasCMS = 53,

    // ── MiCA ───────────────────────────────────────────────────────────────
    /// Crypto-Asset Service Provider (CASP) report
    MiCACASP = 60,
    /// White paper / issuance disclosure
    MiCAWhitePaper = 61,
    /// Reserve asset backing report (for ARTs and EMTs)
    MiCAReserveAsset = 62,
    /// Significant CASP enhanced obligations report
    MiCASignificant = 63,
}

// ─────────────────────────────────────────────────────────────────────────────
// Report Status
// ─────────────────────────────────────────────────────────────────────────────

/// Lifecycle state of a single regulatory report.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReportStatus {
    /// Report data has been captured but not yet validated.
    Draft = 0,
    /// All schema and business rules have passed; ready to submit.
    Validated = 1,
    /// Submitted to the regulator's system; awaiting acknowledgment.
    Submitted = 2,
    /// Regulator confirmed receipt (but not yet accepted/rejected).
    Acknowledged = 3,
    /// Regulator accepted the submission as compliant.
    Accepted = 4,
    /// Regulator rejected the submission; resubmission required.
    Rejected = 5,
    /// Operator manually cancelled or withdrew the report.
    Cancelled = 6,
    /// Report deadline passed without successful submission.
    Overdue = 7,
}

impl ReportStatus {
    /// Returns `true` if the report is in a terminal (non-retryable) state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ReportStatus::Accepted | ReportStatus::Cancelled | ReportStatus::Overdue
        )
    }

    /// Returns `true` if the report may be resubmitted.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ReportStatus::Rejected)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation Result
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome of schema and business-rule validation for a report.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationResult {
    /// Whether all required checks passed.
    pub passed: bool,
    /// Number of errors found (0 on success).
    pub error_count: u32,
    /// Number of non-blocking warnings.
    pub warning_count: u32,
    /// Encoded error messages (one entry per error).
    pub errors: Vec<Bytes>,
    /// Encoded warning messages (one entry per warning).
    pub warnings: Vec<Bytes>,
    /// Timestamp at which validation ran (Unix seconds).
    pub validated_at: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Report
// ─────────────────────────────────────────────────────────────────────────────

/// A regulatory report ready for submission.
///
/// Encompasses generated content, schema metadata, current lifecycle status,
/// and traceability back to the originating on-chain audit events.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryReport {
    /// Unique content-addressed report identifier (SHA-256 of key fields).
    pub id: BytesN<32>,
    /// Regulatory authority this report is addressed to.
    pub authority: RegulatoryAuthority,
    /// Specific form / schema being used.
    pub format: ReportFormat,
    /// Reporting entity (submitter's on-chain address).
    pub entity: Address,
    /// LEI — Legal Entity Identifier (20-char ISO 17442 alphanumeric).
    pub lei: Bytes,
    /// Reporting period start (Unix seconds).
    pub period_start: u64,
    /// Reporting period end (Unix seconds).
    pub period_end: u64,
    /// UTC deadline by which submission must be accepted (Unix seconds).
    pub deadline: u64,
    /// Serialised report payload (authority-specific encoding).
    pub content: Bytes,
    /// Schema version of the content encoding.
    pub schema_version: u32,
    /// Current lifecycle status.
    pub status: ReportStatus,
    /// Timestamp this record was first created (Unix seconds).
    pub created_at: u64,
    /// Timestamp of the most recent status change (Unix seconds).
    pub updated_at: u64,
    /// Last validation result attached to this report.
    pub last_validation: Option<ValidationResult>,
    /// SHA-256 of the previous report for this authority+entity combination
    /// (zero-hash for the first ever report).
    pub prev_report_hash: BytesN<32>,
    /// SHA-256 of this report's content + metadata fields.
    pub report_hash: BytesN<32>,
    /// On-chain event IDs that provided source data for this report.
    pub source_event_ids: Vec<BytesN<32>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Submission
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks a single submission attempt for a report.
///
/// A report may have multiple submissions (e.g., initial + resubmissions
/// after rejection). Each attempt is independently tracked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatorySubmission {
    /// Unique submission identifier.
    pub id: BytesN<32>,
    /// ID of the report being submitted.
    pub report_id: BytesN<32>,
    /// Attempt number (1-based; 1 = first attempt).
    pub attempt: u32,
    /// When this submission was dispatched (Unix seconds).
    pub submitted_at: u64,
    /// Endpoint or queue identifier the submission was sent to.
    pub endpoint: Bytes,
    /// Authority-assigned reference number (populated on acknowledgment).
    pub reference_number: Option<Bytes>,
    /// HTTP/API response status code received (0 = not yet received).
    pub response_code: u32,
    /// Raw response payload from the authority's API.
    pub response_payload: Bytes,
    /// Submission-level status.
    pub status: ReportStatus,
    /// Whether this submission is eligible for automatic retry.
    pub retry_eligible: bool,
    /// Earliest time at which a retry may be dispatched (Unix seconds; 0 = ASAP).
    pub retry_after: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Acknowledgment
// ─────────────────────────────────────────────────────────────────────────────

/// Acknowledgment received from the regulator for a submission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionAcknowledgment {
    /// Unique acknowledgment identifier.
    pub id: BytesN<32>,
    /// Submission this acknowledgment pertains to.
    pub submission_id: BytesN<32>,
    /// Report this acknowledgment ultimately resolves.
    pub report_id: BytesN<32>,
    /// Authority-assigned reference number confirming receipt.
    pub reference_number: Bytes,
    /// Whether the authority fully accepted the submission.
    pub accepted: bool,
    /// Human-readable reason for rejection (empty on acceptance).
    pub rejection_reason: Bytes,
    /// Error codes returned by the authority (empty on acceptance).
    pub error_codes: Vec<Bytes>,
    /// Timestamp of acknowledgment receipt (Unix seconds).
    pub received_at: u64,
    /// SHA-256 of the acknowledgment payload for tamper-evidence.
    pub ack_hash: BytesN<32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Reporting Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Per-authority reporting configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityConfig {
    /// Regulatory authority this config applies to.
    pub authority: RegulatoryAuthority,
    /// Whether reporting to this authority is enabled.
    pub enabled: bool,
    /// Primary submission endpoint URI.
    pub endpoint: Bytes,
    /// API key or credential reference (never stored as plaintext; use a reference).
    pub credential_ref: Bytes,
    /// Maximum automatic retry attempts before marking as failed.
    pub max_retries: u32,
    /// Base delay in seconds between retry attempts.
    pub retry_delay_seconds: u32,
    /// Whether to use exponential back-off on retries.
    pub exponential_backoff: bool,
    /// Number of ledgers to retain report records.
    pub retention_ledgers: u32,
}

/// Global reporting pipeline configuration stored on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingConfig {
    /// Entity whose reports are managed by this config.
    pub entity: Address,
    /// Per-authority settings (index matches RegulatoryAuthority discriminant).
    pub authority_configs: Vec<AuthorityConfig>,
    /// Whether the entire reporting pipeline is active.
    pub pipeline_active: bool,
    /// Operator address allowed to update config.
    pub operator: Address,
}

// ─────────────────────────────────────────────────────────────────────────────
// Audit Trail Entry
// ─────────────────────────────────────────────────────────────────────────────

/// Every action on a report is recorded as an immutable audit entry.
///
/// Actions form a hash-chained sequence keyed by `(report_id, sequence)`,
/// providing an append-only log of everything that happened to each report.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReportAction {
    /// Report payload was generated.
    Generated = 0,
    /// Validation ran (pass or fail).
    Validated = 1,
    /// Report was submitted to the authority.
    Submitted = 2,
    /// Acknowledgment was received from the authority.
    AcknowledgmentReceived = 3,
    /// Authority accepted the report.
    Accepted = 4,
    /// Authority rejected the report.
    Rejected = 5,
    /// Resubmission was triggered.
    Resubmitted = 6,
    /// Report was manually cancelled.
    Cancelled = 7,
    /// Report was marked overdue.
    MarkedOverdue = 8,
    /// Configuration was updated.
    ConfigUpdated = 9,
}

/// A single immutable audit trail entry for a regulatory report action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingAuditEntry {
    /// Sequential entry number within this report's audit trail (0-based).
    pub sequence: u32,
    /// Report this entry belongs to.
    pub report_id: BytesN<32>,
    /// Action that was performed.
    pub action: ReportAction,
    /// Address of the actor who triggered this action.
    pub actor: Address,
    /// Timestamp of the action (Unix seconds).
    pub timestamp: u64,
    /// Previous entry hash in this report's audit chain (zero-hash for first entry).
    pub prev_entry_hash: BytesN<32>,
    /// SHA-256 of this entry's fields + prev_entry_hash.
    pub entry_hash: BytesN<32>,
    /// Additional context encoded as bytes (e.g., submission ID, error code).
    pub context: Bytes,
    /// Status the report transitioned to as a result of this action.
    pub resulting_status: ReportStatus,
}

// ─────────────────────────────────────────────────────────────────────────────
// Error codes
// ─────────────────────────────────────────────────────────────────────────────

/// Error conditions for the reporting pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReportingError {
    /// Report with given ID was not found.
    ReportNotFound = 200,
    /// Submission with given ID was not found.
    SubmissionNotFound = 201,
    /// Report is not in a valid state for the requested transition.
    InvalidStatusTransition = 202,
    /// Report failed schema or business-rule validation.
    ValidationFailed = 203,
    /// Required field is missing from the report content.
    MissingRequiredField = 204,
    /// Field value is outside the allowed range or format.
    InvalidFieldValue = 205,
    /// Authority is not enabled in the current configuration.
    AuthorityDisabled = 206,
    /// Maximum retry limit has been reached.
    MaxRetriesExceeded = 207,
    /// Submission deadline has passed.
    DeadlineExceeded = 208,
    /// LEI format is invalid (must be 20 alphanumeric chars).
    InvalidLEI = 209,
    /// Reporting period is invalid (start must precede end).
    InvalidReportingPeriod = 210,
    /// Acknowledgment reference does not match any known submission.
    AcknowledgmentOrphan = 211,
    /// Authority configuration is not found.
    ConfigNotFound = 212,
    /// Pipeline is paused; no submissions allowed.
    PipelinePaused = 213,
    /// Entity is not authorized to manage this report.
    UnauthorizedEntity = 214,
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authority_ordering() {
        assert!(RegulatoryAuthority::FINRA < RegulatoryAuthority::SEC);
        assert!(RegulatoryAuthority::SEC < RegulatoryAuthority::CFTC);
        assert!(RegulatoryAuthority::CFTC < RegulatoryAuthority::FCA);
        assert!(RegulatoryAuthority::FCA < RegulatoryAuthority::BaFin);
        assert!(RegulatoryAuthority::BaFin < RegulatoryAuthority::MAS);
        assert!(RegulatoryAuthority::MAS < RegulatoryAuthority::MiCA);
    }

    #[test]
    fn test_authority_all_returns_seven() {
        assert_eq!(RegulatoryAuthority::all().len(), 7);
    }

    #[test]
    fn test_authority_names() {
        assert_eq!(RegulatoryAuthority::FINRA.name(), "FINRA");
        assert_eq!(RegulatoryAuthority::SEC.name(), "SEC");
        assert_eq!(RegulatoryAuthority::CFTC.name(), "CFTC");
        assert_eq!(RegulatoryAuthority::FCA.name(), "FCA");
        assert_eq!(RegulatoryAuthority::BaFin.name(), "BaFin");
        assert_eq!(RegulatoryAuthority::MAS.name(), "MAS");
        assert_eq!(RegulatoryAuthority::MiCA.name(), "MiCA");
    }

    #[test]
    fn test_authority_jurisdictions() {
        assert_eq!(RegulatoryAuthority::FINRA.jurisdiction(), "US");
        assert_eq!(RegulatoryAuthority::FCA.jurisdiction(), "GB");
        assert_eq!(RegulatoryAuthority::BaFin.jurisdiction(), "DE");
        assert_eq!(RegulatoryAuthority::MAS.jurisdiction(), "SG");
        assert_eq!(RegulatoryAuthority::MiCA.jurisdiction(), "EU");
    }

    #[test]
    fn test_report_status_terminal() {
        assert!(ReportStatus::Accepted.is_terminal());
        assert!(ReportStatus::Cancelled.is_terminal());
        assert!(ReportStatus::Overdue.is_terminal());
        assert!(!ReportStatus::Draft.is_terminal());
        assert!(!ReportStatus::Submitted.is_terminal());
        assert!(!ReportStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_report_status_retryable() {
        assert!(ReportStatus::Rejected.is_retryable());
        assert!(!ReportStatus::Accepted.is_retryable());
        assert!(!ReportStatus::Submitted.is_retryable());
    }

    #[test]
    fn test_report_action_ordering() {
        assert!(ReportAction::Generated < ReportAction::Validated);
        assert!(ReportAction::Validated < ReportAction::Submitted);
        assert!(ReportAction::Submitted < ReportAction::AcknowledgmentReceived);
    }
}
