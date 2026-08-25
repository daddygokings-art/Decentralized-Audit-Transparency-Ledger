//! # RWA Monitoring Module
//!
//! Compliance event tracking, red-flag detection, reporting workflows, and
//! regulatory filing generation for tokenized Real World Assets.
//!
//! ## Responsibilities
//! - Define structured compliance event types for the audit ledger
//! - Detect suspicious patterns (velocity breaches, wash trading, dormant
//!   accounts waking, concentration risk)
//! - Manage the reporting workflow: `Draft → InReview → Filed → Acknowledged`
//! - Generate regulatory filing payloads (FinCEN SAR, FATF Travel Rule,
//!   MiCA Article 76 disclosure, BCBS credit-risk exposure)

#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ─────────────────────────────────────────────────────────────────────────────
// Compliance Event Types
// ─────────────────────────────────────────────────────────────────────────────

/// Taxonomy of compliance events emitted during RWA token lifecycle.
///
/// Each variant maps to a Soroban event topic so off-chain indexers can
/// subscribe to individual event classes.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ComplianceEventType {
    // ── Token lifecycle ──────────────────────────────────────────────────────
    /// A new tokenized asset was proposed.
    TokenProposed = 0,
    /// Compliance review approved the token.
    TokenApproved = 1,
    /// Compliance review rejected the token.
    TokenRejected = 2,
    /// Token lifecycle transitioned (e.g., Active → Paused).
    TokenStateChanged = 3,
    /// Token reached terminal Retired state.
    TokenRetired = 4,

    // ── Mint / Burn ──────────────────────────────────────────────────────────
    /// New tokens were minted.
    TokensMinted = 10,
    /// Tokens were burned.
    TokensBurned = 11,
    /// Mint was denied due to cap or state.
    MintDenied = 12,

    // ── Transfers ────────────────────────────────────────────────────────────
    /// A transfer was completed successfully.
    TransferCompleted = 20,
    /// A transfer was blocked by restriction logic.
    TransferBlocked = 21,
    /// Large-value transfer exceeding threshold.
    LargeTransfer = 22,
    /// Transfer between parties with mismatched jurisdictions.
    CrossJurisdictionTransfer = 23,

    // ── Holder registry ──────────────────────────────────────────────────────
    /// New holder registered.
    HolderRegistered = 30,
    /// Holder KYC status was updated.
    HolderKycUpdated = 31,
    /// Holder was placed on the blocklist.
    HolderBlocked = 32,
    /// Holder was removed from the blocklist.
    HolderUnblocked = 33,
    /// Holder added to the per-token allowlist.
    HolderAllowlisted = 34,

    // ── Red flags ────────────────────────────────────────────────────────────
    /// Transaction velocity breach detected.
    VelocityBreach = 40,
    /// Possible wash-trading pattern identified.
    WashTradingDetected = 41,
    /// Dormant account suddenly active.
    DormantAccountActivated = 42,
    /// Holder concentration exceeded threshold.
    ConcentrationRisk = 43,
    /// Structuring pattern detected (smurfing).
    StructuringDetected = 44,
    /// Sanctions list match (external feed).
    SanctionsMatch = 45,

    // ── Regulatory filings ───────────────────────────────────────────────────
    /// SAR filed with FinCEN.
    SarFiled = 50,
    /// CTR filed (cash transaction report).
    CtrFiled = 51,
    /// FATF Travel Rule data shared.
    TravelRuleShared = 52,
    /// MiCA Article 76 periodic disclosure submitted.
    MicaDisclosure = 53,
    /// BCBS credit-risk exposure report generated.
    BcbsCreditReport = 54,
}

impl ComplianceEventType {
    /// Short symbolic string used as on-chain event topic.
    pub fn topic_name(&self) -> &'static str {
        match self {
            ComplianceEventType::TokenProposed => "TKN_PROPOSED",
            ComplianceEventType::TokenApproved => "TKN_APPROVED",
            ComplianceEventType::TokenRejected => "TKN_REJECTED",
            ComplianceEventType::TokenStateChanged => "TKN_STATE",
            ComplianceEventType::TokenRetired => "TKN_RETIRED",
            ComplianceEventType::TokensMinted => "MINT",
            ComplianceEventType::TokensBurned => "BURN",
            ComplianceEventType::MintDenied => "MINT_DENIED",
            ComplianceEventType::TransferCompleted => "XFER_OK",
            ComplianceEventType::TransferBlocked => "XFER_BLOCKED",
            ComplianceEventType::LargeTransfer => "LARGE_XFER",
            ComplianceEventType::CrossJurisdictionTransfer => "CROSS_JUR",
            ComplianceEventType::HolderRegistered => "HLDR_REG",
            ComplianceEventType::HolderKycUpdated => "HLDR_KYC",
            ComplianceEventType::HolderBlocked => "HLDR_BLOCKED",
            ComplianceEventType::HolderUnblocked => "HLDR_UNBLOCKED",
            ComplianceEventType::HolderAllowlisted => "HLDR_ALLOW",
            ComplianceEventType::VelocityBreach => "VEL_BREACH",
            ComplianceEventType::WashTradingDetected => "WASH_TRADE",
            ComplianceEventType::DormantAccountActivated => "DORMANT_ACT",
            ComplianceEventType::ConcentrationRisk => "CONC_RISK",
            ComplianceEventType::StructuringDetected => "STRUCTURING",
            ComplianceEventType::SanctionsMatch => "SANCTIONS",
            ComplianceEventType::SarFiled => "SAR_FILED",
            ComplianceEventType::CtrFiled => "CTR_FILED",
            ComplianceEventType::TravelRuleShared => "TRAVEL_RULE",
            ComplianceEventType::MicaDisclosure => "MICA_DISC",
            ComplianceEventType::BcbsCreditReport => "BCBS_CREDIT",
        }
    }

    /// True when this event type represents a regulatory red flag.
    pub fn is_red_flag(&self) -> bool {
        matches!(
            self,
            ComplianceEventType::VelocityBreach
                | ComplianceEventType::WashTradingDetected
                | ComplianceEventType::DormantAccountActivated
                | ComplianceEventType::ConcentrationRisk
                | ComplianceEventType::StructuringDetected
                | ComplianceEventType::SanctionsMatch
        )
    }

    /// True when automatic regulatory filing should be triggered.
    pub fn triggers_auto_filing(&self) -> bool {
        matches!(
            self,
            ComplianceEventType::WashTradingDetected
                | ComplianceEventType::SanctionsMatch
                | ComplianceEventType::LargeTransfer
        )
    }

    /// Suggested severity level (0 = info, 1 = warning, 2 = critical).
    pub fn default_severity(&self) -> u8 {
        match self {
            ComplianceEventType::SanctionsMatch
            | ComplianceEventType::WashTradingDetected => 2,
            ComplianceEventType::VelocityBreach
            | ComplianceEventType::StructuringDetected
            | ComplianceEventType::ConcentrationRisk
            | ComplianceEventType::TransferBlocked => 1,
            _ => 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compliance Event Record
// ─────────────────────────────────────────────────────────────────────────────

/// A single compliance event recorded against the audit ledger.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceEvent {
    /// Unique event ID (SHA-256 of content + timestamp).
    pub event_id: BytesN<32>,
    /// Event classification.
    pub event_type: u8, // ComplianceEventType as u8
    /// Token this event relates to (or zero-hash for global events).
    pub token_id: BytesN<32>,
    /// Primary party (holder, issuer, or operator).
    pub actor: Address,
    /// Secondary party where applicable (e.g., transfer receiver).
    pub counterparty: Option<Address>,
    /// Monetary amount involved (0 when not applicable).
    pub amount: u128,
    /// Severity: 0 = info, 1 = warning, 2 = critical.
    pub severity: u8,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Serialised payload (JSON or binary schema).
    pub payload: Bytes,
    /// Optional reference to a regulatory filing.
    pub filing_ref: Option<BytesN<32>>,
    /// SHA-256 of previous compliance event (forms a tamper-evident chain).
    pub prev_event_hash: BytesN<32>,
}

impl ComplianceEvent {
    /// Decode the event type enum.
    pub fn compliance_event_type(&self) -> ComplianceEventType {
        ComplianceEventType::from_u8(self.event_type)
    }

    /// Whether this event requires a regulatory follow-up action.
    pub fn requires_action(&self) -> bool {
        self.severity >= 1 || self.compliance_event_type().is_red_flag()
    }
}

impl ComplianceEventType {
    /// Infallible decode from stored u8.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => ComplianceEventType::TokenProposed,
            1 => ComplianceEventType::TokenApproved,
            2 => ComplianceEventType::TokenRejected,
            3 => ComplianceEventType::TokenStateChanged,
            4 => ComplianceEventType::TokenRetired,
            10 => ComplianceEventType::TokensMinted,
            11 => ComplianceEventType::TokensBurned,
            12 => ComplianceEventType::MintDenied,
            20 => ComplianceEventType::TransferCompleted,
            21 => ComplianceEventType::TransferBlocked,
            22 => ComplianceEventType::LargeTransfer,
            23 => ComplianceEventType::CrossJurisdictionTransfer,
            30 => ComplianceEventType::HolderRegistered,
            31 => ComplianceEventType::HolderKycUpdated,
            32 => ComplianceEventType::HolderBlocked,
            33 => ComplianceEventType::HolderUnblocked,
            34 => ComplianceEventType::HolderAllowlisted,
            40 => ComplianceEventType::VelocityBreach,
            41 => ComplianceEventType::WashTradingDetected,
            42 => ComplianceEventType::DormantAccountActivated,
            43 => ComplianceEventType::ConcentrationRisk,
            44 => ComplianceEventType::StructuringDetected,
            45 => ComplianceEventType::SanctionsMatch,
            50 => ComplianceEventType::SarFiled,
            51 => ComplianceEventType::CtrFiled,
            52 => ComplianceEventType::TravelRuleShared,
            53 => ComplianceEventType::MicaDisclosure,
            54 => ComplianceEventType::BcbsCreditReport,
            _ => ComplianceEventType::TransferCompleted, // safe default
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Red-Flag Detection Engine
// ─────────────────────────────────────────────────────────────────────────────

/// Thresholds used by the red-flag detection engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedFlagThresholds {
    /// Single transfer amount that triggers a large-transfer alert.
    pub large_transfer_threshold: u128,
    /// Maximum number of transfers per holder per rolling window.
    pub velocity_max_transfers: u32,
    /// Rolling window duration in seconds for velocity checks.
    pub velocity_window_seconds: u64,
    /// Maximum fraction of total supply a single holder may own (basis points, 10000 = 100%).
    pub concentration_limit_bps: u32,
    /// Minimum dormancy period in seconds before account is flagged.
    pub dormancy_period_seconds: u64,
    /// Maximum back-and-forth transfer pairs in a window (wash-trade heuristic).
    pub wash_trade_roundtrip_limit: u32,
    /// Structuring: maximum number of sub-threshold transfers before flag.
    pub structuring_tx_count: u32,
    /// Structuring: per-transfer amount ceiling (transactions below this count).
    pub structuring_amount_ceiling: u128,
}

impl RedFlagThresholds {
    /// Regulatory-grade defaults aligned with FATF Recommendation 16.
    pub fn default() -> Self {
        RedFlagThresholds {
            large_transfer_threshold: 10_000_00, // $10,000 in cents
            velocity_max_transfers: 50,
            velocity_window_seconds: 86_400, // 24 hours
            concentration_limit_bps: 2500,   // 25%
            dormancy_period_seconds: 15_552_000, // ~180 days
            wash_trade_roundtrip_limit: 3,
            structuring_tx_count: 5,
            structuring_amount_ceiling: 9_999_99, // just below $10,000
        }
    }
}

/// Outcome of a red-flag detection check.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RedFlagOutcome {
    /// No red flag detected.
    Clean = 0,
    /// Single pattern match; flag for review.
    FlaggedForReview = 1,
    /// High-confidence alert; file SAR immediately.
    AutoFileSar = 2,
    /// Immediate sanctions hit; block and escalate.
    SanctionsHit = 3,
}

impl RedFlagOutcome {
    pub fn requires_immediate_action(&self) -> bool {
        matches!(
            self,
            RedFlagOutcome::AutoFileSar | RedFlagOutcome::SanctionsHit
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RedFlagOutcome::Clean => "CLEAN",
            RedFlagOutcome::FlaggedForReview => "REVIEW",
            RedFlagOutcome::AutoFileSar => "AUTO_SAR",
            RedFlagOutcome::SanctionsHit => "SANCTIONS_HIT",
        }
    }
}

/// Stateless red-flag detection engine.
pub struct RedFlagDetector;

impl RedFlagDetector {
    /// Check whether a transfer amount exceeds the large-transfer threshold.
    pub fn check_large_transfer(
        amount: u128,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        amount >= thresholds.large_transfer_threshold
    }

    /// Velocity check: returns true when the transfer count in the rolling
    /// window exceeds the configured maximum.
    pub fn check_velocity_breach(
        transfers_in_window: u32,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        transfers_in_window > thresholds.velocity_max_transfers
    }

    /// Concentration risk: returns true when a single holder's balance
    /// exceeds the concentration limit expressed in basis points.
    pub fn check_concentration_risk(
        holder_balance: u128,
        total_supply: u128,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        if total_supply == 0 {
            return false;
        }
        // holder_bps = (holder_balance * 10000) / total_supply
        let holder_bps = (holder_balance.saturating_mul(10_000)) / total_supply;
        holder_bps > thresholds.concentration_limit_bps as u128
    }

    /// Dormancy check: returns true when `last_active_ts` is older than the
    /// dormancy threshold relative to `now_ts`.
    pub fn check_dormant_activation(
        last_active_ts: u64,
        now_ts: u64,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        now_ts.saturating_sub(last_active_ts) >= thresholds.dormancy_period_seconds
    }

    /// Wash-trade check: returns true when round-trip transfer pairs between
    /// the same pair exceed the configured limit.
    pub fn check_wash_trading(
        roundtrip_count: u32,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        roundtrip_count >= thresholds.wash_trade_roundtrip_limit
    }

    /// Structuring check: multiple sub-threshold transactions that collectively
    /// appear to circumvent the CTR/SAR trigger.
    pub fn check_structuring(
        sub_threshold_tx_count: u32,
        single_amount: u128,
        thresholds: &RedFlagThresholds,
    ) -> bool {
        single_amount < thresholds.structuring_amount_ceiling
            && sub_threshold_tx_count >= thresholds.structuring_tx_count
    }

    /// Aggregate outcome given individual flag results.
    pub fn aggregate_outcome(flags: &[bool], has_sanctions_hit: bool) -> RedFlagOutcome {
        if has_sanctions_hit {
            return RedFlagOutcome::SanctionsHit;
        }
        let flag_count = flags.iter().filter(|&&f| f).count();
        match flag_count {
            0 => RedFlagOutcome::Clean,
            1 => RedFlagOutcome::FlaggedForReview,
            _ => RedFlagOutcome::AutoFileSar,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Reporting Workflow
// ─────────────────────────────────────────────────────────────────────────────

/// States in the compliance reporting workflow.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ReportStatus {
    /// Report drafted; not yet submitted for review.
    Draft = 0,
    /// Under compliance officer review.
    InReview = 1,
    /// Approved internally; ready to file.
    Approved = 2,
    /// Electronically filed with the regulator.
    Filed = 3,
    /// Regulator acknowledged receipt.
    Acknowledged = 4,
    /// Filing rejected by regulator (resubmit required).
    Rejected = 5,
    /// Withdrawn before filing.
    Withdrawn = 6,
}

impl ReportStatus {
    /// Valid next states from `self`.
    pub fn can_transition_to(&self, to: ReportStatus) -> bool {
        matches!(
            (self, to),
            (ReportStatus::Draft, ReportStatus::InReview)
                | (ReportStatus::Draft, ReportStatus::Withdrawn)
                | (ReportStatus::InReview, ReportStatus::Approved)
                | (ReportStatus::InReview, ReportStatus::Draft) // send back for revision
                | (ReportStatus::Approved, ReportStatus::Filed)
                | (ReportStatus::Approved, ReportStatus::Withdrawn)
                | (ReportStatus::Filed, ReportStatus::Acknowledged)
                | (ReportStatus::Filed, ReportStatus::Rejected)
                | (ReportStatus::Rejected, ReportStatus::Draft) // re-open for revision
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ReportStatus::Acknowledged | ReportStatus::Withdrawn
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReportStatus::Draft => "DRAFT",
            ReportStatus::InReview => "IN_REVIEW",
            ReportStatus::Approved => "APPROVED",
            ReportStatus::Filed => "FILED",
            ReportStatus::Acknowledged => "ACKNOWLEDGED",
            ReportStatus::Rejected => "REJECTED",
            ReportStatus::Withdrawn => "WITHDRAWN",
        }
    }
}

/// Regulatory filing types supported by this module.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FilingType {
    /// FinCEN Suspicious Activity Report (US).
    FinCenSar = 0,
    /// FinCEN Currency Transaction Report (US).
    FinCenCtr = 1,
    /// FATF Travel Rule originator/beneficiary data.
    FatfTravelRule = 2,
    /// EU MiCA Article 76 periodic disclosure.
    MicaArticle76 = 3,
    /// BCBS credit-risk exposure summary.
    BcbsCreditExposure = 4,
    /// ESMA position limit report.
    EsmaPosition = 5,
    /// IOSCO Cross-border RWA report.
    IosCoCrossBorder = 6,
}

impl FilingType {
    pub fn label(&self) -> &'static str {
        match self {
            FilingType::FinCenSar => "FinCEN SAR",
            FilingType::FinCenCtr => "FinCEN CTR",
            FilingType::FatfTravelRule => "FATF Travel Rule",
            FilingType::MicaArticle76 => "MiCA Article 76",
            FilingType::BcbsCreditExposure => "BCBS Credit Exposure",
            FilingType::EsmaPosition => "ESMA Position",
            FilingType::IosCoCrossBorder => "IOSCO Cross-Border",
        }
    }

    /// Whether this filing type has a mandatory submission deadline (in hours).
    pub fn deadline_hours(&self) -> Option<u32> {
        match self {
            FilingType::FinCenSar => Some(30 * 24), // 30 calendar days
            FilingType::FinCenCtr => Some(15 * 24), // 15 calendar days
            FilingType::FatfTravelRule => Some(1),   // near-real-time
            FilingType::MicaArticle76 => Some(90 * 24), // quarterly
            FilingType::BcbsCreditExposure => Some(30 * 24),
            FilingType::EsmaPosition => Some(1 * 24),
            FilingType::IosCoCrossBorder => None, // best-efforts
        }
    }

    /// Whether this filing type is cross-border.
    pub fn is_cross_border(&self) -> bool {
        matches!(
            self,
            FilingType::FatfTravelRule
                | FilingType::MicaArticle76
                | FilingType::IosCoCrossBorder
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Regulatory Filing Record
// ─────────────────────────────────────────────────────────────────────────────

/// A complete regulatory filing record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryFiling {
    /// Content-addressed filing ID.
    pub filing_id: BytesN<32>,
    /// Type of regulatory filing.
    pub filing_type: u8, // FilingType as u8
    /// Current workflow status.
    pub status: u8, // ReportStatus as u8
    /// Token this filing concerns.
    pub token_id: BytesN<32>,
    /// Subject party (the entity being reported on).
    pub subject: Address,
    /// Filing officer (the compliance officer submitting).
    pub filed_by: Address,
    /// Ledger timestamp when draft was created.
    pub created_at: u64,
    /// Ledger timestamp of last status change.
    pub updated_at: u64,
    /// Optional external regulator-assigned reference number (as bytes).
    pub regulator_ref: Option<Bytes>,
    /// Serialised filing payload (schema defined off-chain per FilingType).
    pub payload: Bytes,
    /// IDs of compliance events that triggered this filing.
    pub trigger_event_ids: Vec<BytesN<32>>,
    /// SHA-256 hash of the filing payload for integrity verification.
    pub payload_hash: BytesN<32>,
}

impl RegulatoryFiling {
    /// Decode the filing type enum.
    pub fn filing_type_enum(&self) -> FilingType {
        match self.filing_type {
            0 => FilingType::FinCenSar,
            1 => FilingType::FinCenCtr,
            2 => FilingType::FatfTravelRule,
            3 => FilingType::MicaArticle76,
            4 => FilingType::BcbsCreditExposure,
            5 => FilingType::EsmaPosition,
            _ => FilingType::IosCoCrossBorder,
        }
    }

    /// Decode the report status enum.
    pub fn report_status(&self) -> ReportStatus {
        match self.status {
            0 => ReportStatus::Draft,
            1 => ReportStatus::InReview,
            2 => ReportStatus::Approved,
            3 => ReportStatus::Filed,
            4 => ReportStatus::Acknowledged,
            5 => ReportStatus::Rejected,
            _ => ReportStatus::Withdrawn,
        }
    }

    /// Whether this filing is overdue given the current timestamp.
    pub fn is_overdue(&self, now_ts: u64) -> bool {
        if self.report_status().is_terminal() {
            return false;
        }
        if let Some(deadline_h) = self.filing_type_enum().deadline_hours() {
            let deadline_ts = self
                .created_at
                .saturating_add((deadline_h as u64) * 3600);
            now_ts > deadline_ts
        } else {
            false
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Monitoring Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime configuration for the monitoring subsystem.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitoringConfig {
    /// Maximum compliance events stored per token.
    pub max_events_per_token: u32,
    /// Maximum open (non-terminal) filings at once.
    pub max_open_filings: u32,
    /// Whether auto-SAR filing is enabled for high-confidence red flags.
    pub auto_sar_enabled: bool,
    /// Whether FATF Travel Rule is mandated for all transfers.
    pub travel_rule_mandatory: bool,
    /// Minimum amount (in base units) above which Travel Rule applies.
    pub travel_rule_threshold: u128,
    /// Red-flag detection thresholds.
    pub red_flag_thresholds: RedFlagThresholds,
}

impl MonitoringConfig {
    pub fn default() -> Self {
        MonitoringConfig {
            max_events_per_token: 50_000,
            max_open_filings: 1_000,
            auto_sar_enabled: true,
            travel_rule_mandatory: false,
            travel_rule_threshold: 100_000, // e.g., $1,000 in cents
            red_flag_thresholds: RedFlagThresholds::default(),
        }
    }

    /// Whether Travel Rule applies to a given transfer amount.
    pub fn requires_travel_rule(&self, amount: u128) -> bool {
        self.travel_rule_mandatory || amount >= self.travel_rule_threshold
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Reporting Workflow Engine
// ─────────────────────────────────────────────────────────────────────────────

/// Stateless workflow engine for report lifecycle management.
pub struct ReportingWorkflow;

impl ReportingWorkflow {
    /// Attempt to advance a filing from its current status to `target`.
    /// Returns `Ok(target)` on success or an error string.
    pub fn transition<'a>(
        current: ReportStatus,
        target: ReportStatus,
    ) -> Result<ReportStatus, &'a str> {
        if current.can_transition_to(target) {
            Ok(target)
        } else {
            Err("INVALID_TRANSITION")
        }
    }

    /// Determine whether a filing needs an auto-SAR based on red-flag outcome
    /// and monitoring configuration.
    pub fn should_auto_file_sar(
        outcome: RedFlagOutcome,
        config: &MonitoringConfig,
    ) -> bool {
        config.auto_sar_enabled
            && matches!(
                outcome,
                RedFlagOutcome::AutoFileSar | RedFlagOutcome::SanctionsHit
            )
    }

    /// Return true when the compliance event type requires a Travel Rule payload.
    pub fn requires_travel_rule_payload(
        event_type: ComplianceEventType,
        amount: u128,
        config: &MonitoringConfig,
    ) -> bool {
        matches!(
            event_type,
            ComplianceEventType::TransferCompleted | ComplianceEventType::LargeTransfer
        ) && config.requires_travel_rule(amount)
    }

    /// Generate a filing type recommendation given a red-flag outcome and event.
    pub fn recommend_filing_type(
        outcome: RedFlagOutcome,
        event_type: ComplianceEventType,
    ) -> Option<FilingType> {
        match outcome {
            RedFlagOutcome::SanctionsHit => Some(FilingType::FinCenSar),
            RedFlagOutcome::AutoFileSar => Some(FilingType::FinCenSar),
            RedFlagOutcome::FlaggedForReview => {
                match event_type {
                    ComplianceEventType::LargeTransfer => Some(FilingType::FinCenCtr),
                    ComplianceEventType::CrossJurisdictionTransfer => {
                        Some(FilingType::FatfTravelRule)
                    }
                    ComplianceEventType::ConcentrationRisk => {
                        Some(FilingType::BcbsCreditExposure)
                    }
                    _ => None,
                }
            }
            RedFlagOutcome::Clean => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline unit tests — 25+ lines
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ComplianceEventType ──────────────────────────────────────────────────

    #[test]
    fn test_compliance_event_topic_names_non_empty() {
        let types = [
            ComplianceEventType::TokenProposed,
            ComplianceEventType::TokensMinted,
            ComplianceEventType::TransferCompleted,
            ComplianceEventType::VelocityBreach,
            ComplianceEventType::SarFiled,
            ComplianceEventType::TravelRuleShared,
        ];
        for t in &types {
            assert!(!t.topic_name().is_empty(), "Empty topic for {:?}", t);
        }
    }

    #[test]
    fn test_compliance_event_is_red_flag() {
        assert!(ComplianceEventType::VelocityBreach.is_red_flag());
        assert!(ComplianceEventType::WashTradingDetected.is_red_flag());
        assert!(ComplianceEventType::SanctionsMatch.is_red_flag());
        assert!(ComplianceEventType::StructuringDetected.is_red_flag());
        assert!(ComplianceEventType::ConcentrationRisk.is_red_flag());
        assert!(!ComplianceEventType::TransferCompleted.is_red_flag());
        assert!(!ComplianceEventType::TokenApproved.is_red_flag());
    }

    #[test]
    fn test_compliance_event_triggers_auto_filing() {
        assert!(ComplianceEventType::WashTradingDetected.triggers_auto_filing());
        assert!(ComplianceEventType::SanctionsMatch.triggers_auto_filing());
        assert!(ComplianceEventType::LargeTransfer.triggers_auto_filing());
        assert!(!ComplianceEventType::TokensMinted.triggers_auto_filing());
        assert!(!ComplianceEventType::HolderRegistered.triggers_auto_filing());
    }

    #[test]
    fn test_compliance_event_severity_levels() {
        assert_eq!(ComplianceEventType::SanctionsMatch.default_severity(), 2);
        assert_eq!(ComplianceEventType::WashTradingDetected.default_severity(), 2);
        assert_eq!(ComplianceEventType::VelocityBreach.default_severity(), 1);
        assert_eq!(ComplianceEventType::TransferBlocked.default_severity(), 1);
        assert_eq!(ComplianceEventType::TokensMinted.default_severity(), 0);
    }

    #[test]
    fn test_compliance_event_type_from_u8_roundtrip() {
        let variants: &[(u8, ComplianceEventType)] = &[
            (0, ComplianceEventType::TokenProposed),
            (10, ComplianceEventType::TokensMinted),
            (20, ComplianceEventType::TransferCompleted),
            (40, ComplianceEventType::VelocityBreach),
            (50, ComplianceEventType::SarFiled),
            (54, ComplianceEventType::BcbsCreditReport),
        ];
        for (v, expected) in variants {
            assert_eq!(ComplianceEventType::from_u8(*v), *expected);
        }
    }

    // ── RedFlagThresholds ────────────────────────────────────────────────────

    #[test]
    fn test_red_flag_thresholds_defaults_are_sensible() {
        let t = RedFlagThresholds::default();
        assert!(t.large_transfer_threshold > 0);
        assert!(t.velocity_max_transfers > 0);
        assert!(t.concentration_limit_bps <= 10_000); // ≤ 100%
        assert!(t.dormancy_period_seconds > 0);
    }

    // ── RedFlagDetector ──────────────────────────────────────────────────────

    #[test]
    fn test_detect_large_transfer_above_threshold() {
        let t = RedFlagThresholds::default();
        assert!(RedFlagDetector::check_large_transfer(
            t.large_transfer_threshold,
            &t
        ));
        assert!(!RedFlagDetector::check_large_transfer(
            t.large_transfer_threshold - 1,
            &t
        ));
    }

    #[test]
    fn test_detect_velocity_breach() {
        let t = RedFlagThresholds::default();
        assert!(RedFlagDetector::check_velocity_breach(
            t.velocity_max_transfers + 1,
            &t
        ));
        assert!(!RedFlagDetector::check_velocity_breach(
            t.velocity_max_transfers,
            &t
        ));
    }

    #[test]
    fn test_detect_concentration_risk() {
        let t = RedFlagThresholds::default(); // 25%
        // 30% of 1_000_000 = 300_000 → above 25% → flag
        assert!(RedFlagDetector::check_concentration_risk(
            300_000, 1_000_000, &t
        ));
        // 20% of 1_000_000 = 200_000 → below 25% → clean
        assert!(!RedFlagDetector::check_concentration_risk(
            200_000, 1_000_000, &t
        ));
    }

    #[test]
    fn test_detect_concentration_zero_supply() {
        let t = RedFlagThresholds::default();
        assert!(!RedFlagDetector::check_concentration_risk(0, 0, &t));
    }

    #[test]
    fn test_detect_dormant_activation() {
        let t = RedFlagThresholds::default(); // 180 days
        let now = 20_000_000u64;
        let dormant_last_active = now - t.dormancy_period_seconds;
        assert!(RedFlagDetector::check_dormant_activation(
            dormant_last_active,
            now,
            &t
        ));
        let recent_last_active = now - 100;
        assert!(!RedFlagDetector::check_dormant_activation(
            recent_last_active,
            now,
            &t
        ));
    }

    #[test]
    fn test_detect_wash_trading() {
        let t = RedFlagThresholds::default(); // limit = 3
        assert!(RedFlagDetector::check_wash_trading(3, &t));
        assert!(!RedFlagDetector::check_wash_trading(2, &t));
    }

    #[test]
    fn test_detect_structuring() {
        let t = RedFlagThresholds::default();
        // 5 transactions just below $10,000 → flag
        assert!(RedFlagDetector::check_structuring(5, 9_999_99, &t));
        // Amount at or above ceiling → no structuring flag
        assert!(!RedFlagDetector::check_structuring(5, 10_000_00, &t));
        // Count below threshold → no structuring flag
        assert!(!RedFlagDetector::check_structuring(4, 9_000_00, &t));
    }

    #[test]
    fn test_aggregate_outcome_clean() {
        let outcome = RedFlagDetector::aggregate_outcome(&[false, false, false], false);
        assert_eq!(outcome, RedFlagOutcome::Clean);
    }

    #[test]
    fn test_aggregate_outcome_one_flag() {
        let outcome = RedFlagDetector::aggregate_outcome(&[true, false, false], false);
        assert_eq!(outcome, RedFlagOutcome::FlaggedForReview);
    }

    #[test]
    fn test_aggregate_outcome_multi_flag_auto_sar() {
        let outcome = RedFlagDetector::aggregate_outcome(&[true, true, false], false);
        assert_eq!(outcome, RedFlagOutcome::AutoFileSar);
    }

    #[test]
    fn test_aggregate_outcome_sanctions_overrides() {
        // Even zero flags → sanctions hit if sanctions_hit=true
        let outcome = RedFlagDetector::aggregate_outcome(&[false, false], true);
        assert_eq!(outcome, RedFlagOutcome::SanctionsHit);
    }

    #[test]
    fn test_red_flag_outcome_requires_immediate_action() {
        assert!(RedFlagOutcome::AutoFileSar.requires_immediate_action());
        assert!(RedFlagOutcome::SanctionsHit.requires_immediate_action());
        assert!(!RedFlagOutcome::FlaggedForReview.requires_immediate_action());
        assert!(!RedFlagOutcome::Clean.requires_immediate_action());
    }

    // ── ReportStatus workflow ────────────────────────────────────────────────

    #[test]
    fn test_report_status_valid_transitions() {
        assert!(ReportStatus::Draft.can_transition_to(ReportStatus::InReview));
        assert!(ReportStatus::InReview.can_transition_to(ReportStatus::Approved));
        assert!(ReportStatus::Approved.can_transition_to(ReportStatus::Filed));
        assert!(ReportStatus::Filed.can_transition_to(ReportStatus::Acknowledged));
        assert!(ReportStatus::Filed.can_transition_to(ReportStatus::Rejected));
        assert!(ReportStatus::Rejected.can_transition_to(ReportStatus::Draft));
    }

    #[test]
    fn test_report_status_invalid_transitions() {
        assert!(!ReportStatus::Draft.can_transition_to(ReportStatus::Filed));
        assert!(!ReportStatus::Acknowledged.can_transition_to(ReportStatus::Draft));
        assert!(!ReportStatus::Withdrawn.can_transition_to(ReportStatus::InReview));
    }

    #[test]
    fn test_report_status_is_terminal() {
        assert!(ReportStatus::Acknowledged.is_terminal());
        assert!(ReportStatus::Withdrawn.is_terminal());
        assert!(!ReportStatus::Filed.is_terminal());
        assert!(!ReportStatus::Draft.is_terminal());
    }

    #[test]
    fn test_report_status_str_labels() {
        assert_eq!(ReportStatus::Draft.as_str(), "DRAFT");
        assert_eq!(ReportStatus::Filed.as_str(), "FILED");
        assert_eq!(ReportStatus::Acknowledged.as_str(), "ACKNOWLEDGED");
    }

    // ── ReportingWorkflow engine ─────────────────────────────────────────────

    #[test]
    fn test_workflow_transition_success() {
        let result = ReportingWorkflow::transition(ReportStatus::Draft, ReportStatus::InReview);
        assert_eq!(result, Ok(ReportStatus::InReview));
    }

    #[test]
    fn test_workflow_transition_failure() {
        let result = ReportingWorkflow::transition(ReportStatus::Draft, ReportStatus::Filed);
        assert_eq!(result, Err("INVALID_TRANSITION"));
    }

    #[test]
    fn test_workflow_should_auto_file_sar() {
        let cfg = MonitoringConfig::default();
        assert!(ReportingWorkflow::should_auto_file_sar(
            RedFlagOutcome::AutoFileSar,
            &cfg
        ));
        assert!(ReportingWorkflow::should_auto_file_sar(
            RedFlagOutcome::SanctionsHit,
            &cfg
        ));
        assert!(!ReportingWorkflow::should_auto_file_sar(
            RedFlagOutcome::FlaggedForReview,
            &cfg
        ));
        assert!(!ReportingWorkflow::should_auto_file_sar(
            RedFlagOutcome::Clean,
            &cfg
        ));
    }

    #[test]
    fn test_workflow_recommend_filing_type_sanctions() {
        let rec = ReportingWorkflow::recommend_filing_type(
            RedFlagOutcome::SanctionsHit,
            ComplianceEventType::SanctionsMatch,
        );
        assert_eq!(rec, Some(FilingType::FinCenSar));
    }

    #[test]
    fn test_workflow_recommend_filing_type_large_transfer() {
        let rec = ReportingWorkflow::recommend_filing_type(
            RedFlagOutcome::FlaggedForReview,
            ComplianceEventType::LargeTransfer,
        );
        assert_eq!(rec, Some(FilingType::FinCenCtr));
    }

    #[test]
    fn test_workflow_recommend_filing_type_cross_jurisdiction() {
        let rec = ReportingWorkflow::recommend_filing_type(
            RedFlagOutcome::FlaggedForReview,
            ComplianceEventType::CrossJurisdictionTransfer,
        );
        assert_eq!(rec, Some(FilingType::FatfTravelRule));
    }

    #[test]
    fn test_workflow_recommend_no_filing_when_clean() {
        let rec = ReportingWorkflow::recommend_filing_type(
            RedFlagOutcome::Clean,
            ComplianceEventType::TransferCompleted,
        );
        assert!(rec.is_none());
    }

    // ── FilingType ───────────────────────────────────────────────────────────

    #[test]
    fn test_filing_type_labels_non_empty() {
        let types = [
            FilingType::FinCenSar,
            FilingType::FinCenCtr,
            FilingType::FatfTravelRule,
            FilingType::MicaArticle76,
            FilingType::BcbsCreditExposure,
            FilingType::EsmaPosition,
            FilingType::IosCoCrossBorder,
        ];
        for t in &types {
            assert!(!t.label().is_empty());
        }
    }

    #[test]
    fn test_filing_type_deadlines() {
        assert_eq!(FilingType::FinCenSar.deadline_hours(), Some(30 * 24));
        assert_eq!(FilingType::FatfTravelRule.deadline_hours(), Some(1));
        assert_eq!(FilingType::IosCoCrossBorder.deadline_hours(), None);
    }

    #[test]
    fn test_filing_type_cross_border() {
        assert!(FilingType::FatfTravelRule.is_cross_border());
        assert!(FilingType::MicaArticle76.is_cross_border());
        assert!(FilingType::IosCoCrossBorder.is_cross_border());
        assert!(!FilingType::FinCenSar.is_cross_border());
        assert!(!FilingType::FinCenCtr.is_cross_border());
    }

    // ── MonitoringConfig ─────────────────────────────────────────────────────

    #[test]
    fn test_monitoring_config_defaults() {
        let cfg = MonitoringConfig::default();
        assert!(cfg.max_events_per_token > 0);
        assert!(cfg.max_open_filings > 0);
        assert!(cfg.auto_sar_enabled);
    }

    #[test]
    fn test_monitoring_config_travel_rule_threshold() {
        let cfg = MonitoringConfig::default();
        // Exactly at threshold
        assert!(cfg.requires_travel_rule(cfg.travel_rule_threshold));
        // Below threshold, not mandatory
        assert!(!cfg.requires_travel_rule(cfg.travel_rule_threshold - 1));
    }

    #[test]
    fn test_monitoring_config_travel_rule_mandatory() {
        let mut cfg = MonitoringConfig::default();
        cfg.travel_rule_mandatory = true;
        // Any amount requires Travel Rule when mandatory
        assert!(cfg.requires_travel_rule(1));
        assert!(cfg.requires_travel_rule(0));
    }

    // ── RegulatoryFiling ─────────────────────────────────────────────────────

    #[test]
    fn test_regulatory_filing_overdue() {
        use soroban_sdk::Env;
        let env = Env::default();
        let subject = soroban_sdk::Address::generate(&env);
        let officer = soroban_sdk::Address::generate(&env);

        let filing = RegulatoryFiling {
            filing_id: BytesN::from_array(&env, &[0u8; 32]),
            filing_type: FilingType::FinCenSar as u8,
            status: ReportStatus::Draft as u8,
            token_id: BytesN::from_array(&env, &[1u8; 32]),
            subject,
            filed_by: officer,
            created_at: 1_000,
            updated_at: 1_000,
            regulator_ref: None,
            payload: Bytes::from_slice(&env, b"test"),
            trigger_event_ids: Vec::new(&env),
            payload_hash: BytesN::from_array(&env, &[2u8; 32]),
        };

        // SAR deadline = 30 days = 2_592_000 seconds
        let within_deadline = 1_000 + 2_592_000 - 1;
        assert!(!filing.is_overdue(within_deadline));

        let past_deadline = 1_000 + 2_592_001;
        assert!(filing.is_overdue(past_deadline));
    }

    #[test]
    fn test_regulatory_filing_not_overdue_when_terminal() {
        use soroban_sdk::Env;
        let env = Env::default();
        let subject = soroban_sdk::Address::generate(&env);
        let officer = soroban_sdk::Address::generate(&env);

        let filing = RegulatoryFiling {
            filing_id: BytesN::from_array(&env, &[0u8; 32]),
            filing_type: FilingType::FinCenSar as u8,
            status: ReportStatus::Acknowledged as u8, // terminal
            token_id: BytesN::from_array(&env, &[1u8; 32]),
            subject,
            filed_by: officer,
            created_at: 0,
            updated_at: 0,
            regulator_ref: None,
            payload: Bytes::from_slice(&env, b"done"),
            trigger_event_ids: Vec::new(&env),
            payload_hash: BytesN::from_array(&env, &[3u8; 32]),
        };

        // Even very far in the future, an acknowledged filing is not overdue
        assert!(!filing.is_overdue(u64::MAX));
    }

    // ── requires_travel_rule_payload ─────────────────────────────────────────

    #[test]
    fn test_requires_travel_rule_payload_transfer_above_threshold() {
        let cfg = MonitoringConfig::default();
        assert!(ReportingWorkflow::requires_travel_rule_payload(
            ComplianceEventType::TransferCompleted,
            cfg.travel_rule_threshold,
            &cfg
        ));
    }

    #[test]
    fn test_does_not_require_travel_rule_for_non_transfer_events() {
        let cfg = MonitoringConfig::default();
        assert!(!ReportingWorkflow::requires_travel_rule_payload(
            ComplianceEventType::TokensMinted,
            cfg.travel_rule_threshold + 1,
            &cfg
        ));
    }
}
