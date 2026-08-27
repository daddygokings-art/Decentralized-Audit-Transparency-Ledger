//! RWA KYC/AML Compliance Module
//!
//! Implements Know Your Customer (KYC) verification, Anti-Money Laundering (AML)
//! screening, risk scoring, and sanctions list checking for Real World Asset (RWA)
//! tokenization workflows on the Stellar/Soroban network.
//!
//! # State Machines
//!
//! ## Verification Status
//! ```text
//! Unverified → Pending → Approved
//!                     ↘ Rejected
//!                     ↘ Expired
//! Approved → Expired  (TTL reached)
//! Approved → Suspended (manual action)
//! Suspended → Approved (re-review)
//! ```
//!
//! ## Risk Level
//! ```text
//! Unknown → Low → Medium → High → Critical
//! (any risk level can escalate via new evidence)
//! (only manual review can de-escalate)
//! ```
//!
//! ## AML Screening State
//! ```text
//! NotScreened → Screening → Clear
//!                        ↘ Flagged → UnderReview → Cleared | Blocked
//! ```

#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// KYC/AML verification status for an entity.
///
/// Transitions:
/// - `Unverified`  → `Pending` when KYC submission is received.
/// - `Pending`     → `Approved` | `Rejected` after review.
/// - `Approved`    → `Expired` when TTL passes; `Suspended` by compliance officer.
/// - `Suspended`   → `Approved` after re-review passes.
/// - `Rejected`    → `Pending` if a new submission arrives.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum VerificationStatus {
    /// No KYC submitted; entity cannot participate in RWA operations.
    Unverified = 0,
    /// KYC documents submitted and under review.
    Pending = 1,
    /// KYC approved; entity is eligible to trade RWA tokens.
    Approved = 2,
    /// KYC rejected; entity must resubmit corrected documents.
    Rejected = 3,
    /// KYC expired; entity must renew before trading.
    Expired = 4,
    /// Entity suspended pending investigation; trading is blocked.
    Suspended = 5,
}

impl VerificationStatus {
    /// Whether the entity is currently allowed to participate in RWA trading.
    pub fn is_eligible(&self) -> bool {
        matches!(self, VerificationStatus::Approved)
    }

    /// Whether the entity requires active compliance review.
    pub fn requires_review(&self) -> bool {
        matches!(
            self,
            VerificationStatus::Pending | VerificationStatus::Suspended
        )
    }

    /// Whether the entity is in a terminal blocking state.
    pub fn is_blocked(&self) -> bool {
        matches!(
            self,
            VerificationStatus::Rejected
                | VerificationStatus::Suspended
                | VerificationStatus::Expired
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            VerificationStatus::Unverified => "UNVERIFIED",
            VerificationStatus::Pending => "PENDING",
            VerificationStatus::Approved => "APPROVED",
            VerificationStatus::Rejected => "REJECTED",
            VerificationStatus::Expired => "EXPIRED",
            VerificationStatus::Suspended => "SUSPENDED",
        }
    }

    /// Validates whether a transition from `self` to `next` is permissible.
    pub fn can_transition_to(&self, next: VerificationStatus) -> bool {
        use VerificationStatus::*;
        matches!(
            (self, next),
            (Unverified, Pending)
                | (Pending, Approved)
                | (Pending, Rejected)
                | (Rejected, Pending)
                | (Approved, Expired)
                | (Approved, Suspended)
                | (Expired, Pending)
                | (Suspended, Approved)
                | (Suspended, Rejected)
        )
    }
}

/// Risk classification for an entity.
///
/// Levels escalate based on AML signals; de-escalation requires manual review.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RiskLevel {
    /// Risk not yet assessed.
    Unknown = 0,
    /// Low risk: standard retail/institutional participant.
    Low = 1,
    /// Medium risk: enhanced due diligence required.
    Medium = 2,
    /// High risk: senior compliance officer sign-off required.
    High = 3,
    /// Critical: all activity frozen pending investigation.
    Critical = 4,
}

impl RiskLevel {
    /// Risk score ceiling for each tier (inclusive upper bound).
    pub fn score_ceiling(&self) -> u32 {
        match self {
            RiskLevel::Unknown => 0,
            RiskLevel::Low => 25,
            RiskLevel::Medium => 50,
            RiskLevel::High => 75,
            RiskLevel::Critical => 100,
        }
    }

    /// Derives a `RiskLevel` from a numeric score in [0, 100].
    pub fn from_score(score: u32) -> Self {
        match score {
            0 => RiskLevel::Unknown,
            1..=25 => RiskLevel::Low,
            26..=50 => RiskLevel::Medium,
            51..=75 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    /// Whether any RWA action is permitted at this risk level.
    pub fn permits_trading(&self) -> bool {
        matches!(self, RiskLevel::Low | RiskLevel::Medium)
    }

    /// Whether enhanced due diligence (EDD) is required.
    pub fn requires_edd(&self) -> bool {
        matches!(self, RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical)
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            RiskLevel::Unknown => "UNKNOWN",
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MEDIUM",
            RiskLevel::High => "HIGH",
            RiskLevel::Critical => "CRITICAL",
        }
    }
}

/// AML screening state for an entity or transaction.
///
/// Transitions:
/// - `NotScreened` → `Screening` when a screen is initiated.
/// - `Screening`   → `Clear` | `Flagged` after rule evaluation.
/// - `Flagged`     → `UnderReview` when a compliance officer picks it up.
/// - `UnderReview` → `Cleared` | `Blocked` after investigation.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AmlScreeningState {
    /// Entity/transaction has not been screened.
    NotScreened = 0,
    /// Screening is currently in progress.
    Screening = 1,
    /// Screening completed; no suspicious signals found.
    Clear = 2,
    /// Screening detected suspicious signals; pending officer review.
    Flagged = 3,
    /// Under active compliance officer investigation.
    UnderReview = 4,
    /// Investigation complete; entity/transaction cleared.
    Cleared = 5,
    /// Permanently blocked following investigation.
    Blocked = 6,
}

impl AmlScreeningState {
    /// Whether the entity/transaction can proceed.
    pub fn allows_activity(&self) -> bool {
        matches!(self, AmlScreeningState::Clear | AmlScreeningState::Cleared)
    }

    /// Whether manual officer intervention is needed.
    pub fn needs_officer_action(&self) -> bool {
        matches!(
            self,
            AmlScreeningState::Flagged | AmlScreeningState::UnderReview
        )
    }

    /// Validates a state transition.
    pub fn can_transition_to(&self, next: AmlScreeningState) -> bool {
        use AmlScreeningState::*;
        matches!(
            (self, next),
            (NotScreened, Screening)
                | (Screening, Clear)
                | (Screening, Flagged)
                | (Flagged, UnderReview)
                | (UnderReview, Cleared)
                | (UnderReview, Blocked)
                | (Cleared, Screening) // re-screen is allowed
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            AmlScreeningState::NotScreened => "NOT_SCREENED",
            AmlScreeningState::Screening => "SCREENING",
            AmlScreeningState::Clear => "CLEAR",
            AmlScreeningState::Flagged => "FLAGGED",
            AmlScreeningState::UnderReview => "UNDER_REVIEW",
            AmlScreeningState::Cleared => "CLEARED",
            AmlScreeningState::Blocked => "BLOCKED",
        }
    }
}

/// Jurisdiction where the entity or asset is domiciled.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Jurisdiction {
    /// United States
    US = 0,
    /// European Union
    EU = 1,
    /// United Kingdom
    UK = 2,
    /// Singapore
    SG = 3,
    /// United Arab Emirates
    UAE = 4,
    /// Switzerland
    CH = 5,
    /// Cayman Islands
    KY = 6,
    /// Unknown or unclassified
    Unknown = 255,
}

impl Jurisdiction {
    /// Whether the jurisdiction is considered a high-risk zone for AML purposes.
    pub fn is_high_risk(&self) -> bool {
        matches!(self, Jurisdiction::KY | Jurisdiction::Unknown)
    }

    /// Base risk score contribution from this jurisdiction.
    pub fn base_risk_score(&self) -> u32 {
        match self {
            Jurisdiction::US => 5,
            Jurisdiction::EU => 5,
            Jurisdiction::UK => 5,
            Jurisdiction::SG => 10,
            Jurisdiction::UAE => 15,
            Jurisdiction::CH => 10,
            Jurisdiction::KY => 30,
            Jurisdiction::Unknown => 40,
        }
    }

    /// ISO 3166-1 alpha-2 code.
    pub fn code(&self) -> &'static str {
        match self {
            Jurisdiction::US => "US",
            Jurisdiction::EU => "EU",
            Jurisdiction::UK => "GB",
            Jurisdiction::SG => "SG",
            Jurisdiction::UAE => "AE",
            Jurisdiction::CH => "CH",
            Jurisdiction::KY => "KY",
            Jurisdiction::Unknown => "XX",
        }
    }
}

/// Sanctions list a match may originate from.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SanctionsList {
    /// U.S. Office of Foreign Assets Control — SDN list.
    OFAC = 0,
    /// European Union consolidated sanctions list.
    EUSanctions = 1,
    /// United Nations Security Council sanctions.
    UNSanctions = 2,
    /// U.K. Office of Financial Sanctions Implementation.
    OFSI = 3,
    /// Interpol Red Notice (not a formal sanctions list, but treated similarly).
    Interpol = 4,
}

impl SanctionsList {
    /// Severity weight when computing the overall sanctions score.
    pub fn weight(&self) -> u32 {
        match self {
            SanctionsList::OFAC => 50,
            SanctionsList::EUSanctions => 45,
            SanctionsList::UNSanctions => 50,
            SanctionsList::OFSI => 45,
            SanctionsList::Interpol => 40,
        }
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            SanctionsList::OFAC => "OFAC SDN",
            SanctionsList::EUSanctions => "EU Sanctions",
            SanctionsList::UNSanctions => "UN Sanctions",
            SanctionsList::OFSI => "UK OFSI",
            SanctionsList::Interpol => "Interpol Red Notice",
        }
    }
}

// ---------------------------------------------------------------------------
// Core Data Structures
// ---------------------------------------------------------------------------

/// A sanctions list match record for an entity or address.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanctionsMatch {
    /// The list that produced this match.
    pub list: u8, // SanctionsList as u8
    /// Match confidence in the range [0, 100].
    pub confidence: u32,
    /// Timestamp when the match was detected.
    pub detected_at: u64,
    /// Unique match reference from the sanctions provider.
    pub match_ref: BytesN<32>,
    /// Optional additional context (e.g., alias matched).
    pub notes: Bytes,
}

impl SanctionsMatch {
    /// Whether the match confidence exceeds the blocking threshold (≥ 80).
    pub fn is_blocking(&self) -> bool {
        self.confidence >= 80
    }

    /// Whether this is a definitive hit (confidence = 100).
    pub fn is_definitive(&self) -> bool {
        self.confidence == 100
    }
}

/// KYC document submission and verification record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KycRecord {
    /// On-chain address of the entity being verified.
    pub entity: Address,
    /// Current verification status.
    pub status: u8, // VerificationStatus as u8
    /// Jurisdiction of entity domicile.
    pub jurisdiction: u8, // Jurisdiction as u8
    /// Timestamp of the last status update (ledger time).
    pub last_updated: u64,
    /// Timestamp when Approved status expires (0 = never reviewed).
    pub expires_at: u64,
    /// Hash of the KYC document package (off-chain storage reference).
    pub document_hash: BytesN<32>,
    /// Compliance officer who performed the last review (if any).
    pub reviewed_by: Option<Address>,
    /// Schema version of this record.
    pub version: u32,
    /// Supplementary compliance notes.
    pub notes: Bytes,
}

impl KycRecord {
    /// Whether this record is currently approved and not expired.
    pub fn is_active(&self, current_time: u64) -> bool {
        self.status == VerificationStatus::Approved as u8
            && (self.expires_at == 0 || current_time < self.expires_at)
    }

    /// Whether this record has elapsed past its expiry.
    pub fn is_expired(&self, current_time: u64) -> bool {
        self.expires_at > 0 && current_time >= self.expires_at
    }
}

/// AML screening result for an entity or individual transaction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmlScreeningResult {
    /// Entity being screened.
    pub entity: Address,
    /// Current AML screening state.
    pub state: u8, // AmlScreeningState as u8
    /// Computed AML risk score [0, 100].
    pub risk_score: u32,
    /// Number of AML rule hits that contributed to the score.
    pub rule_hits: u32,
    /// Timestamp when this screening was last run.
    pub screened_at: u64,
    /// List of sanctions matches (may be empty).
    pub sanctions_matches: Vec<SanctionsMatch>,
    /// AML screening session ID for audit trail linkage.
    pub session_id: BytesN<32>,
    /// Notes from the compliance officer (if under review).
    pub officer_notes: Bytes,
}

impl AmlScreeningResult {
    /// Whether the entity can proceed based solely on this screening result.
    pub fn is_clear(&self) -> bool {
        matches!(
            AmlScreeningState::from_u8(self.state),
            Some(AmlScreeningState::Clear) | Some(AmlScreeningState::Cleared)
        )
    }

    /// Whether there are any blocking sanctions matches.
    pub fn has_blocking_sanctions(&self) -> bool {
        self.sanctions_matches.iter().any(|m| m.is_blocking())
    }
}

impl AmlScreeningState {
    /// Lossless conversion from raw u8.
    pub fn from_u8(v: u8) -> Option<AmlScreeningState> {
        match v {
            0 => Some(AmlScreeningState::NotScreened),
            1 => Some(AmlScreeningState::Screening),
            2 => Some(AmlScreeningState::Clear),
            3 => Some(AmlScreeningState::Flagged),
            4 => Some(AmlScreeningState::UnderReview),
            5 => Some(AmlScreeningState::Cleared),
            6 => Some(AmlScreeningState::Blocked),
            _ => None,
        }
    }
}

/// Composite compliance profile aggregating KYC and AML for one entity.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceProfile {
    /// On-chain address of the entity.
    pub entity: Address,
    /// Current KYC verification status.
    pub kyc_status: u8, // VerificationStatus as u8
    /// Current AML screening state.
    pub aml_state: u8, // AmlScreeningState as u8
    /// Current risk level.
    pub risk_level: u8, // RiskLevel as u8
    /// Composite compliance score [0, 100]; higher = more risk.
    pub compliance_score: u32,
    /// Number of AML flags raised against this entity (lifetime).
    pub aml_flag_count: u32,
    /// Number of sanctions matches found (lifetime).
    pub sanctions_hit_count: u32,
    /// Whether this entity is on the internal watchlist.
    pub watchlisted: bool,
    /// Timestamp of the most recent compliance review.
    pub last_review_at: u64,
    /// Jurisdiction of the entity.
    pub jurisdiction: u8, // Jurisdiction as u8
}

impl ComplianceProfile {
    /// Whether the entity is fully compliant and eligible for RWA trading.
    pub fn is_fully_compliant(&self) -> bool {
        let kyc_ok = self.kyc_status == VerificationStatus::Approved as u8;
        let aml_ok = matches!(
            AmlScreeningState::from_u8(self.aml_state),
            Some(AmlScreeningState::Clear) | Some(AmlScreeningState::Cleared)
        );
        let risk_ok = RiskLevel::from_score(self.compliance_score).permits_trading();
        kyc_ok && aml_ok && risk_ok && !self.watchlisted
    }

    /// Derive the effective risk level from the stored compliance score.
    pub fn effective_risk_level(&self) -> RiskLevel {
        // Use the stored risk_level if explicitly set to Critical or High.
        let stored = RiskLevel::from_score(self.compliance_score);
        let explicit = match self.risk_level {
            v if v == RiskLevel::Critical as u8 => RiskLevel::Critical,
            v if v == RiskLevel::High as u8 => RiskLevel::High,
            _ => stored,
        };
        // Return the higher of the two.
        if explicit as u8 > stored as u8 {
            explicit
        } else {
            stored
        }
    }
}

/// Configuration parameters for the KYC/AML engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KycAmlConfig {
    /// Minimum risk score threshold that triggers enhanced due diligence.
    pub edd_threshold: u32,
    /// Minimum sanctions match confidence that triggers blocking (0–100).
    pub sanctions_block_confidence: u32,
    /// TTL in seconds for an Approved KYC record (0 = never expires).
    pub kyc_ttl_seconds: u64,
    /// Maximum AML risk score before an entity is automatically suspended.
    pub auto_suspend_score: u32,
    /// Whether high-risk jurisdictions automatically trigger EDD.
    pub highrisk_jurisdiction_edd: bool,
}

impl KycAmlConfig {
    /// Conservative defaults suitable for institutional RWA issuance.
    pub fn institutional_defaults() -> Self {
        KycAmlConfig {
            edd_threshold: 40,
            sanctions_block_confidence: 70,
            kyc_ttl_seconds: 365 * 24 * 3600, // 1 year
            auto_suspend_score: 80,
            highrisk_jurisdiction_edd: true,
        }
    }

    /// Relaxed defaults for lower-risk retail use cases.
    pub fn retail_defaults() -> Self {
        KycAmlConfig {
            edd_threshold: 60,
            sanctions_block_confidence: 90,
            kyc_ttl_seconds: 2 * 365 * 24 * 3600, // 2 years
            auto_suspend_score: 90,
            highrisk_jurisdiction_edd: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Compliance Scoring Engine
// ---------------------------------------------------------------------------

/// Result produced by the compliance scoring engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceScoringResult {
    /// Final composite risk score [0, 100].
    pub score: u32,
    /// Derived risk level.
    pub risk_level: u8, // RiskLevel as u8
    /// Breakdown: jurisdictional risk contribution.
    pub jurisdiction_score: u32,
    /// Breakdown: AML rule-hit contribution.
    pub aml_rule_score: u32,
    /// Breakdown: sanctions severity contribution.
    pub sanctions_score: u32,
    /// Breakdown: historical flag contribution.
    pub history_score: u32,
    /// Whether EDD is required based on the computed score.
    pub requires_edd: bool,
    /// Whether auto-suspension is triggered.
    pub auto_suspend: bool,
}

/// Stateless compliance scoring engine.
///
/// All inputs are plain data; no on-chain state access required.
pub struct ComplianceScoringEngine;

impl ComplianceScoringEngine {
    /// Compute a composite compliance score from individual signal inputs.
    ///
    /// # Scoring methodology
    /// - Jurisdiction base risk: 0–40 pts
    /// - AML rule hits: 5 pts each, capped at 30
    /// - Sanctions severity (weighted): capped at 50
    /// - Historical AML flags: 2 pts each, capped at 20
    ///
    /// Final score is min(sum, 100).
    pub fn compute(
        jurisdiction: Jurisdiction,
        aml_rule_hits: u32,
        sanctions_matches: &[SanctionsMatch],
        historical_flags: u32,
        config: &KycAmlConfig,
    ) -> ComplianceScoringResult {
        let jurisdiction_score = jurisdiction.base_risk_score().min(40);

        let aml_rule_score = (aml_rule_hits * 5).min(30);

        let sanctions_score: u32 = sanctions_matches
            .iter()
            .map(|m| {
                let list = match m.list {
                    0 => SanctionsList::OFAC,
                    1 => SanctionsList::EUSanctions,
                    2 => SanctionsList::UNSanctions,
                    3 => SanctionsList::OFSI,
                    _ => SanctionsList::Interpol,
                };
                // Weight scaled by confidence percentage.
                (list.weight() * m.confidence) / 100
            })
            .fold(0u32, |acc, s| acc.saturating_add(s))
            .min(50);

        let history_score = (historical_flags * 2).min(20);

        let raw = jurisdiction_score
            .saturating_add(aml_rule_score)
            .saturating_add(sanctions_score)
            .saturating_add(history_score);

        let score = raw.min(100);
        let risk_level = RiskLevel::from_score(score);

        ComplianceScoringResult {
            score,
            risk_level: risk_level as u8,
            jurisdiction_score,
            aml_rule_score,
            sanctions_score,
            history_score,
            requires_edd: score >= config.edd_threshold
                || (config.highrisk_jurisdiction_edd && jurisdiction.is_high_risk()),
            auto_suspend: score >= config.auto_suspend_score,
        }
    }

    /// Re-score an existing `ComplianceProfile` with updated signals.
    pub fn rescore_profile(
        profile: &ComplianceProfile,
        additional_rule_hits: u32,
        new_sanctions_matches: &[SanctionsMatch],
        config: &KycAmlConfig,
    ) -> ComplianceScoringResult {
        let jurisdiction = match profile.jurisdiction {
            0 => Jurisdiction::US,
            1 => Jurisdiction::EU,
            2 => Jurisdiction::UK,
            3 => Jurisdiction::SG,
            4 => Jurisdiction::UAE,
            5 => Jurisdiction::CH,
            6 => Jurisdiction::KY,
            _ => Jurisdiction::Unknown,
        };
        Self::compute(
            jurisdiction,
            profile.aml_flag_count + additional_rule_hits,
            new_sanctions_matches,
            profile.aml_flag_count,
            config,
        )
    }
}

// ---------------------------------------------------------------------------
// Sanctions Checking
// ---------------------------------------------------------------------------

/// Outcome of a sanctions screening pass.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanctionsCheckOutcome {
    /// Whether any match was found across all lists.
    pub any_match: bool,
    /// Whether any match is above the blocking threshold.
    pub blocked: bool,
    /// All matches found (may be empty).
    pub matches: Vec<SanctionsMatch>,
    /// Aggregate severity score (0–100).
    pub aggregate_severity: u32,
    /// Timestamp of the check.
    pub checked_at: u64,
}

impl SanctionsCheckOutcome {
    /// Whether the entity must be blocked from trading.
    pub fn must_block(&self) -> bool {
        self.blocked
    }

    /// Whether the entity should be flagged for manual review (match found but
    /// below the auto-blocking confidence threshold).
    pub fn needs_review(&self) -> bool {
        self.any_match && !self.blocked
    }
}

/// Stateless sanctions check helper.
///
/// In production, `known_matches` would come from off-chain oracle feeds.
pub struct SanctionsChecker;

impl SanctionsChecker {
    /// Evaluate a slice of pre-fetched `SanctionsMatch` records for an entity.
    ///
    /// Returns a `SanctionsCheckOutcome` that callers use to update AML state.
    pub fn evaluate(
        matches: &[SanctionsMatch],
        config: &KycAmlConfig,
        current_time: u64,
    ) -> SanctionsCheckOutcome {
        if matches.is_empty() {
            return SanctionsCheckOutcome {
                any_match: false,
                blocked: false,
                matches: Vec::new(&soroban_sdk::env()),
                aggregate_severity: 0,
                checked_at: current_time,
            };
        }

        let blocked = matches
            .iter()
            .any(|m| m.confidence >= config.sanctions_block_confidence);

        let aggregate_severity: u32 = matches
            .iter()
            .map(|m| {
                let list = match m.list {
                    0 => SanctionsList::OFAC,
                    1 => SanctionsList::EUSanctions,
                    2 => SanctionsList::UNSanctions,
                    3 => SanctionsList::OFSI,
                    _ => SanctionsList::Interpol,
                };
                (list.weight() * m.confidence) / 100
            })
            .fold(0u32, |acc, s| acc.saturating_add(s))
            .min(100);

        SanctionsCheckOutcome {
            any_match: true,
            blocked,
            matches: Vec::new(&soroban_sdk::env()), // populated by caller for on-chain use
            aggregate_severity,
            checked_at: current_time,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Bytes, BytesN, Env, Vec};

    fn mock_env() -> Env {
        Env::default()
    }

    fn zero_hash(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0u8; 32])
    }

    fn make_match(env: &Env, list: SanctionsList, confidence: u32) -> SanctionsMatch {
        SanctionsMatch {
            list: list as u8,
            confidence,
            detected_at: 1_000_000,
            match_ref: zero_hash(env),
            notes: Bytes::new(env),
        }
    }

    // --- VerificationStatus state machine ---

    #[test]
    fn test_verification_status_is_eligible_only_approved() {
        assert!(!VerificationStatus::Unverified.is_eligible());
        assert!(!VerificationStatus::Pending.is_eligible());
        assert!(VerificationStatus::Approved.is_eligible());
        assert!(!VerificationStatus::Rejected.is_eligible());
        assert!(!VerificationStatus::Expired.is_eligible());
        assert!(!VerificationStatus::Suspended.is_eligible());
    }

    #[test]
    fn test_verification_status_is_blocked() {
        assert!(!VerificationStatus::Unverified.is_blocked());
        assert!(!VerificationStatus::Pending.is_blocked());
        assert!(!VerificationStatus::Approved.is_blocked());
        assert!(VerificationStatus::Rejected.is_blocked());
        assert!(VerificationStatus::Expired.is_blocked());
        assert!(VerificationStatus::Suspended.is_blocked());
    }

    #[test]
    fn test_verification_status_requires_review() {
        assert!(VerificationStatus::Pending.requires_review());
        assert!(VerificationStatus::Suspended.requires_review());
        assert!(!VerificationStatus::Approved.requires_review());
        assert!(!VerificationStatus::Rejected.requires_review());
    }

    #[test]
    fn test_verification_status_valid_transitions() {
        assert!(VerificationStatus::Unverified.can_transition_to(VerificationStatus::Pending));
        assert!(VerificationStatus::Pending.can_transition_to(VerificationStatus::Approved));
        assert!(VerificationStatus::Pending.can_transition_to(VerificationStatus::Rejected));
        assert!(VerificationStatus::Approved.can_transition_to(VerificationStatus::Suspended));
        assert!(VerificationStatus::Approved.can_transition_to(VerificationStatus::Expired));
        assert!(VerificationStatus::Suspended.can_transition_to(VerificationStatus::Approved));
        assert!(VerificationStatus::Rejected.can_transition_to(VerificationStatus::Pending));
        assert!(VerificationStatus::Expired.can_transition_to(VerificationStatus::Pending));
    }

    #[test]
    fn test_verification_status_invalid_transitions() {
        // Cannot go from Unverified directly to Approved
        assert!(!VerificationStatus::Unverified.can_transition_to(VerificationStatus::Approved));
        // Cannot go from Approved directly to Unverified
        assert!(!VerificationStatus::Approved.can_transition_to(VerificationStatus::Unverified));
        // Cannot self-transition
        assert!(!VerificationStatus::Pending.can_transition_to(VerificationStatus::Pending));
    }

    // --- RiskLevel ---

    #[test]
    fn test_risk_level_from_score_boundaries() {
        assert_eq!(RiskLevel::from_score(0), RiskLevel::Unknown);
        assert_eq!(RiskLevel::from_score(1), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(25), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(26), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(50), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(51), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(75), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(76), RiskLevel::Critical);
        assert_eq!(RiskLevel::from_score(100), RiskLevel::Critical);
    }

    #[test]
    fn test_risk_level_permits_trading() {
        assert!(!RiskLevel::Unknown.permits_trading());
        assert!(RiskLevel::Low.permits_trading());
        assert!(RiskLevel::Medium.permits_trading());
        assert!(!RiskLevel::High.permits_trading());
        assert!(!RiskLevel::Critical.permits_trading());
    }

    #[test]
    fn test_risk_level_requires_edd() {
        assert!(!RiskLevel::Unknown.requires_edd());
        assert!(!RiskLevel::Low.requires_edd());
        assert!(RiskLevel::Medium.requires_edd());
        assert!(RiskLevel::High.requires_edd());
        assert!(RiskLevel::Critical.requires_edd());
    }

    #[test]
    fn test_risk_level_score_ceiling_ordering() {
        assert!(RiskLevel::Low.score_ceiling() < RiskLevel::Medium.score_ceiling());
        assert!(RiskLevel::Medium.score_ceiling() < RiskLevel::High.score_ceiling());
        assert!(RiskLevel::High.score_ceiling() < RiskLevel::Critical.score_ceiling());
    }

    // --- AmlScreeningState ---

    #[test]
    fn test_aml_state_allows_activity() {
        assert!(!AmlScreeningState::NotScreened.allows_activity());
        assert!(!AmlScreeningState::Screening.allows_activity());
        assert!(AmlScreeningState::Clear.allows_activity());
        assert!(!AmlScreeningState::Flagged.allows_activity());
        assert!(!AmlScreeningState::UnderReview.allows_activity());
        assert!(AmlScreeningState::Cleared.allows_activity());
        assert!(!AmlScreeningState::Blocked.allows_activity());
    }

    #[test]
    fn test_aml_state_needs_officer_action() {
        assert!(AmlScreeningState::Flagged.needs_officer_action());
        assert!(AmlScreeningState::UnderReview.needs_officer_action());
        assert!(!AmlScreeningState::Clear.needs_officer_action());
        assert!(!AmlScreeningState::Blocked.needs_officer_action());
    }

    #[test]
    fn test_aml_state_valid_transitions() {
        assert!(AmlScreeningState::NotScreened.can_transition_to(AmlScreeningState::Screening));
        assert!(AmlScreeningState::Screening.can_transition_to(AmlScreeningState::Clear));
        assert!(AmlScreeningState::Screening.can_transition_to(AmlScreeningState::Flagged));
        assert!(AmlScreeningState::Flagged.can_transition_to(AmlScreeningState::UnderReview));
        assert!(AmlScreeningState::UnderReview.can_transition_to(AmlScreeningState::Cleared));
        assert!(AmlScreeningState::UnderReview.can_transition_to(AmlScreeningState::Blocked));
        assert!(AmlScreeningState::Cleared.can_transition_to(AmlScreeningState::Screening));
    }

    #[test]
    fn test_aml_state_invalid_transitions() {
        assert!(!AmlScreeningState::Clear.can_transition_to(AmlScreeningState::Blocked));
        assert!(!AmlScreeningState::Blocked.can_transition_to(AmlScreeningState::Clear));
        assert!(!AmlScreeningState::NotScreened.can_transition_to(AmlScreeningState::Cleared));
    }

    #[test]
    fn test_aml_state_from_u8_roundtrip() {
        for v in 0u8..=6 {
            assert!(AmlScreeningState::from_u8(v).is_some(), "failed for u8={v}");
        }
        assert!(AmlScreeningState::from_u8(7).is_none());
        assert!(AmlScreeningState::from_u8(255).is_none());
    }

    // --- Jurisdiction ---

    #[test]
    fn test_jurisdiction_high_risk() {
        assert!(Jurisdiction::KY.is_high_risk());
        assert!(Jurisdiction::Unknown.is_high_risk());
        assert!(!Jurisdiction::US.is_high_risk());
        assert!(!Jurisdiction::EU.is_high_risk());
        assert!(!Jurisdiction::UK.is_high_risk());
    }

    #[test]
    fn test_jurisdiction_base_risk_scores_ordered() {
        // Major regulated jurisdictions should score lower than offshore/unknown
        assert!(Jurisdiction::US.base_risk_score() < Jurisdiction::KY.base_risk_score());
        assert!(Jurisdiction::EU.base_risk_score() < Jurisdiction::Unknown.base_risk_score());
    }

    // --- SanctionsList ---

    #[test]
    fn test_sanctions_list_weights_nonzero() {
        assert!(SanctionsList::OFAC.weight() > 0);
        assert!(SanctionsList::EUSanctions.weight() > 0);
        assert!(SanctionsList::UNSanctions.weight() > 0);
        assert!(SanctionsList::OFSI.weight() > 0);
        assert!(SanctionsList::Interpol.weight() > 0);
    }

    #[test]
    fn test_sanctions_match_is_blocking_threshold() {
        let env = mock_env();
        let below = make_match(&env, SanctionsList::OFAC, 79);
        let at = make_match(&env, SanctionsList::OFAC, 80);
        let above = make_match(&env, SanctionsList::OFAC, 95);
        assert!(!below.is_blocking());
        assert!(at.is_blocking());
        assert!(above.is_blocking());
    }

    #[test]
    fn test_sanctions_match_is_definitive() {
        let env = mock_env();
        let partial = make_match(&env, SanctionsList::OFAC, 99);
        let definitive = make_match(&env, SanctionsList::OFAC, 100);
        assert!(!partial.is_definitive());
        assert!(definitive.is_definitive());
    }

    // --- KycRecord ---

    #[test]
    fn test_kyc_record_is_active_and_expired() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let record = KycRecord {
            entity: entity.clone(),
            status: VerificationStatus::Approved as u8,
            jurisdiction: Jurisdiction::US as u8,
            last_updated: 1_000,
            expires_at: 5_000,
            document_hash: zero_hash(&env),
            reviewed_by: None,
            version: 1,
            notes: Bytes::new(&env),
        };
        assert!(record.is_active(3_000));
        assert!(!record.is_active(5_000)); // expired at boundary
        assert!(!record.is_active(6_000));
        assert!(record.is_expired(5_000));
        assert!(!record.is_expired(4_999));
    }

    #[test]
    fn test_kyc_record_no_expiry() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let record = KycRecord {
            entity,
            status: VerificationStatus::Approved as u8,
            jurisdiction: Jurisdiction::EU as u8,
            last_updated: 1_000,
            expires_at: 0, // never expires
            document_hash: zero_hash(&env),
            reviewed_by: None,
            version: 1,
            notes: Bytes::new(&env),
        };
        assert!(record.is_active(999_999_999));
        assert!(!record.is_expired(999_999_999));
    }

    #[test]
    fn test_kyc_record_non_approved_not_active() {
        let env = mock_env();
        let entity = Address::generate(&env);
        for status in [
            VerificationStatus::Unverified,
            VerificationStatus::Pending,
            VerificationStatus::Rejected,
            VerificationStatus::Expired,
            VerificationStatus::Suspended,
        ] {
            let record = KycRecord {
                entity: entity.clone(),
                status: status as u8,
                jurisdiction: Jurisdiction::US as u8,
                last_updated: 1_000,
                expires_at: 0,
                document_hash: zero_hash(&env),
                reviewed_by: None,
                version: 1,
                notes: Bytes::new(&env),
            };
            assert!(
                !record.is_active(2_000),
                "Expected not active for status {:?}",
                status
            );
        }
    }

    // --- ComplianceScoringEngine ---

    #[test]
    fn test_scoring_engine_zero_signals_low_us() {
        let config = KycAmlConfig::institutional_defaults();
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::US, 0, &[], 0, &config);
        assert_eq!(result.jurisdiction_score, 5);
        assert_eq!(result.aml_rule_score, 0);
        assert_eq!(result.sanctions_score, 0);
        assert_eq!(result.history_score, 0);
        assert_eq!(result.score, 5);
        assert_eq!(RiskLevel::from_score(result.score), RiskLevel::Low);
    }

    #[test]
    fn test_scoring_engine_high_risk_jurisdiction() {
        let config = KycAmlConfig::institutional_defaults();
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::KY, 0, &[], 0, &config);
        assert_eq!(result.jurisdiction_score, 30);
        assert!(result.requires_edd); // highrisk_jurisdiction_edd = true
    }

    #[test]
    fn test_scoring_engine_aml_rule_hits_capped_at_30() {
        let config = KycAmlConfig::institutional_defaults();
        // 10 rule hits * 5 pts = 50, capped at 30
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::US, 10, &[], 0, &config);
        assert_eq!(result.aml_rule_score, 30);
    }

    #[test]
    fn test_scoring_engine_sanctions_contribution() {
        let env = mock_env();
        let config = KycAmlConfig::institutional_defaults();
        let matches = [make_match(&env, SanctionsList::OFAC, 100)];
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::US, 0, &matches, 0, &config);
        // OFAC weight=50, confidence=100 → 50*100/100 = 50
        assert_eq!(result.sanctions_score, 50);
        assert_eq!(
            RiskLevel::from_score(result.score),
            RiskLevel::Critical
        );
    }

    #[test]
    fn test_scoring_engine_score_capped_at_100() {
        let env = mock_env();
        let config = KycAmlConfig::institutional_defaults();
        let matches = [
            make_match(&env, SanctionsList::OFAC, 100),
            make_match(&env, SanctionsList::UNSanctions, 100),
        ];
        // Even with extreme inputs, score must not exceed 100
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::Unknown, 20, &matches, 20, &config);
        assert!(result.score <= 100);
    }

    #[test]
    fn test_scoring_engine_auto_suspend_triggered() {
        let env = mock_env();
        let config = KycAmlConfig::institutional_defaults(); // auto_suspend_score = 80
        let matches = [make_match(&env, SanctionsList::OFAC, 100)];
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::Unknown, 5, &matches, 5, &config);
        assert!(result.auto_suspend);
    }

    #[test]
    fn test_scoring_engine_history_score_capped_at_20() {
        let config = KycAmlConfig::institutional_defaults();
        // 20 historical flags * 2 pts = 40, capped at 20
        let result =
            ComplianceScoringEngine::compute(Jurisdiction::US, 0, &[], 20, &config);
        assert_eq!(result.history_score, 20);
    }

    // --- ComplianceProfile ---

    #[test]
    fn test_compliance_profile_fully_compliant() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let profile = ComplianceProfile {
            entity,
            kyc_status: VerificationStatus::Approved as u8,
            aml_state: AmlScreeningState::Clear as u8,
            risk_level: RiskLevel::Low as u8,
            compliance_score: 10, // Low risk
            aml_flag_count: 0,
            sanctions_hit_count: 0,
            watchlisted: false,
            last_review_at: 1_000,
            jurisdiction: Jurisdiction::US as u8,
        };
        assert!(profile.is_fully_compliant());
    }

    #[test]
    fn test_compliance_profile_not_compliant_kyc_pending() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let profile = ComplianceProfile {
            entity,
            kyc_status: VerificationStatus::Pending as u8,
            aml_state: AmlScreeningState::Clear as u8,
            risk_level: RiskLevel::Low as u8,
            compliance_score: 10,
            aml_flag_count: 0,
            sanctions_hit_count: 0,
            watchlisted: false,
            last_review_at: 1_000,
            jurisdiction: Jurisdiction::US as u8,
        };
        assert!(!profile.is_fully_compliant());
    }

    #[test]
    fn test_compliance_profile_not_compliant_watchlisted() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let profile = ComplianceProfile {
            entity,
            kyc_status: VerificationStatus::Approved as u8,
            aml_state: AmlScreeningState::Clear as u8,
            risk_level: RiskLevel::Low as u8,
            compliance_score: 10,
            aml_flag_count: 0,
            sanctions_hit_count: 0,
            watchlisted: true, // <-- watchlisted
            last_review_at: 1_000,
            jurisdiction: Jurisdiction::US as u8,
        };
        assert!(!profile.is_fully_compliant());
    }

    #[test]
    fn test_compliance_profile_effective_risk_level_escalates() {
        let env = mock_env();
        let entity = Address::generate(&env);
        // Score maps to Low, but explicit risk_level is Critical
        let profile = ComplianceProfile {
            entity,
            kyc_status: VerificationStatus::Approved as u8,
            aml_state: AmlScreeningState::Clear as u8,
            risk_level: RiskLevel::Critical as u8,
            compliance_score: 10,
            aml_flag_count: 0,
            sanctions_hit_count: 0,
            watchlisted: false,
            last_review_at: 1_000,
            jurisdiction: Jurisdiction::US as u8,
        };
        assert_eq!(profile.effective_risk_level(), RiskLevel::Critical);
    }

    // --- KycAmlConfig defaults ---

    #[test]
    fn test_kyc_aml_config_institutional_defaults_are_strict() {
        let inst = KycAmlConfig::institutional_defaults();
        let retail = KycAmlConfig::retail_defaults();
        // Institutional should have stricter EDD threshold (lower number)
        assert!(inst.edd_threshold < retail.edd_threshold);
        // Institutional should have stricter sanctions blocking (lower confidence)
        assert!(inst.sanctions_block_confidence < retail.sanctions_block_confidence);
    }

    #[test]
    fn test_kyc_aml_config_retail_allows_longer_ttl() {
        let inst = KycAmlConfig::institutional_defaults();
        let retail = KycAmlConfig::retail_defaults();
        assert!(retail.kyc_ttl_seconds > inst.kyc_ttl_seconds);
    }

    // --- AmlScreeningResult ---

    #[test]
    fn test_aml_screening_result_clear_is_clear() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let result = AmlScreeningResult {
            entity,
            state: AmlScreeningState::Clear as u8,
            risk_score: 5,
            rule_hits: 0,
            screened_at: 1_000,
            sanctions_matches: Vec::new(&env),
            session_id: zero_hash(&env),
            officer_notes: Bytes::new(&env),
        };
        assert!(result.is_clear());
        assert!(!result.has_blocking_sanctions());
    }

    #[test]
    fn test_aml_screening_result_flagged_not_clear() {
        let env = mock_env();
        let entity = Address::generate(&env);
        let result = AmlScreeningResult {
            entity,
            state: AmlScreeningState::Flagged as u8,
            risk_score: 60,
            rule_hits: 3,
            screened_at: 2_000,
            sanctions_matches: Vec::new(&env),
            session_id: zero_hash(&env),
            officer_notes: Bytes::new(&env),
        };
        assert!(!result.is_clear());
    }
}
