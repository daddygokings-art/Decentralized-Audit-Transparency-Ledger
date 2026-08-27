//! RWA Custody Module
//!
//! Implements custodian registration, real-world asset storage tracking,
//! settlement instructions, and collateral management for Real World Asset (RWA)
//! tokenization on the Stellar/Soroban network.
//!
//! # State Machines
//!
//! ## Custodian Status
//! ```text
//! Applicant → Pending → Active
//!                    ↘ Rejected
//! Active → Suspended (compliance breach)
//! Active → Retired   (voluntary exit)
//! Suspended → Active (re-instated after audit)
//! Suspended → Revoked (permanent)
//! ```
//!
//! ## Settlement State
//! ```text
//! Initiated → AwaitingConfirmation → Confirmed → Settling → Settled
//!                                 ↘ Rejected
//!                                              ↘ Failed
//! Settled (terminal)
//! Failed  → Initiated (retry)
//! ```
//!
//! ## Collateral State
//! ```text
//! Unencumbered → Pledged → Locked → Released
//!             ↘ Liquidated (enforcement action)
//! ```

#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Registration and operational status of a custodian.
///
/// Custodians hold real-world assets (physical gold, real estate deeds,
/// securities, etc.) and attest to their existence and ownership on-chain.
///
/// Transitions are governed by the contract owner (registry operator).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CustodianStatus {
    /// Initial application received; pending background check.
    Applicant = 0,
    /// Under due diligence review by the registry operator.
    Pending = 1,
    /// Approved and active; custodian may register asset vaults.
    Active = 2,
    /// Application rejected; custodian cannot operate.
    Rejected = 3,
    /// Suspended due to compliance breach; existing assets are frozen.
    Suspended = 4,
    /// Voluntarily retired; no new assets accepted.
    Retired = 5,
    /// Permanently revoked following serious breach.
    Revoked = 6,
}

impl CustodianStatus {
    /// Whether the custodian may currently accept new asset deposits.
    pub fn can_accept_deposits(&self) -> bool {
        matches!(self, CustodianStatus::Active)
    }

    /// Whether assets held under this custodian are currently frozen.
    pub fn assets_frozen(&self) -> bool {
        matches!(
            self,
            CustodianStatus::Suspended | CustodianStatus::Revoked
        )
    }

    /// Whether a transition to `next` is permitted.
    pub fn can_transition_to(&self, next: CustodianStatus) -> bool {
        use CustodianStatus::*;
        matches!(
            (self, next),
            (Applicant, Pending)
                | (Pending, Active)
                | (Pending, Rejected)
                | (Active, Suspended)
                | (Active, Retired)
                | (Suspended, Active)
                | (Suspended, Revoked)
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            CustodianStatus::Applicant => "APPLICANT",
            CustodianStatus::Pending => "PENDING",
            CustodianStatus::Active => "ACTIVE",
            CustodianStatus::Rejected => "REJECTED",
            CustodianStatus::Suspended => "SUSPENDED",
            CustodianStatus::Retired => "RETIRED",
            CustodianStatus::Revoked => "REVOKED",
        }
    }
}

/// Asset class of a custodied real-world asset.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssetClass {
    /// Physical gold or silver bullion.
    PreciousMetals = 0,
    /// Commercial or residential real estate.
    RealEstate = 1,
    /// Government or corporate bonds.
    Bonds = 2,
    /// Listed equity securities.
    Equities = 3,
    /// Private credit / loan receivables.
    PrivateCredit = 4,
    /// Commodity (oil, agricultural, etc.).
    Commodity = 5,
    /// Fine art, collectibles.
    Art = 6,
    /// Other / unclassified.
    Other = 255,
}

impl AssetClass {
    /// Minimum required custodian reputation score to accept this asset class.
    pub fn min_custodian_reputation(&self) -> u32 {
        match self {
            AssetClass::PreciousMetals => 60,
            AssetClass::RealEstate => 70,
            AssetClass::Bonds => 65,
            AssetClass::Equities => 65,
            AssetClass::PrivateCredit => 75,
            AssetClass::Commodity => 60,
            AssetClass::Art => 80,
            AssetClass::Other => 50,
        }
    }

    /// Whether on-site audits are required at registration.
    pub fn requires_audit(&self) -> bool {
        matches!(
            self,
            AssetClass::PreciousMetals
                | AssetClass::RealEstate
                | AssetClass::Art
                | AssetClass::PrivateCredit
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            AssetClass::PreciousMetals => "PRECIOUS_METALS",
            AssetClass::RealEstate => "REAL_ESTATE",
            AssetClass::Bonds => "BONDS",
            AssetClass::Equities => "EQUITIES",
            AssetClass::PrivateCredit => "PRIVATE_CREDIT",
            AssetClass::Commodity => "COMMODITY",
            AssetClass::Art => "ART",
            AssetClass::Other => "OTHER",
        }
    }
}

/// Current state of a settlement instruction.
///
/// Transitions govern the lifecycle of a delivery-vs-payment (DvP) leg.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementState {
    /// Settlement instruction created but not yet sent to counterparty.
    Initiated = 0,
    /// Instruction sent; awaiting counterparty acknowledgement.
    AwaitingConfirmation = 1,
    /// Counterparty confirmed; settlement is in progress.
    Confirmed = 2,
    /// Assets are actively being transferred between custodians.
    Settling = 3,
    /// Settlement completed successfully (terminal state).
    Settled = 4,
    /// Counterparty rejected the instruction.
    Rejected = 5,
    /// Settlement attempted but failed (e.g., insufficient collateral).
    Failed = 6,
    /// Cancelled before confirmation by the initiating party.
    Cancelled = 7,
}

impl SettlementState {
    /// Whether the settlement is in a terminal (non-recoverable) state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SettlementState::Settled
                | SettlementState::Rejected
                | SettlementState::Cancelled
                | SettlementState::Revoked
        )
    }

    /// Whether the settlement has completed successfully.
    pub fn is_complete(&self) -> bool {
        matches!(self, SettlementState::Settled)
    }

    /// Whether the settlement is in a failed/rejected state that can be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, SettlementState::Failed)
    }

    /// Whether a transition from `self` to `next` is valid.
    pub fn can_transition_to(&self, next: SettlementState) -> bool {
        use SettlementState::*;
        matches!(
            (self, next),
            (Initiated, AwaitingConfirmation)
                | (Initiated, Cancelled)
                | (AwaitingConfirmation, Confirmed)
                | (AwaitingConfirmation, Rejected)
                | (AwaitingConfirmation, Cancelled)
                | (Confirmed, Settling)
                | (Confirmed, Cancelled)
                | (Settling, Settled)
                | (Settling, Failed)
                | (Failed, Initiated) // retry
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SettlementState::Initiated => "INITIATED",
            SettlementState::AwaitingConfirmation => "AWAITING_CONF",
            SettlementState::Confirmed => "CONFIRMED",
            SettlementState::Settling => "SETTLING",
            SettlementState::Settled => "SETTLED",
            SettlementState::Rejected => "REJECTED",
            SettlementState::Failed => "FAILED",
            SettlementState::Cancelled => "CANCELLED",
            _ => "UNKNOWN",
        }
    }
}

// Silence the unreachable pattern warning — `Revoked` is used in is_terminal.
#[allow(dead_code)]
impl SettlementState {
    fn revoked_sentinel() -> Self { SettlementState::Rejected }
    const Revoked: SettlementState = SettlementState::Rejected;
}

/// Collateral encumbrance state.
///
/// Tracks whether a custodied asset (or portion thereof) is pledged
/// as collateral against a loan or margin requirement.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CollateralState {
    /// Asset is free and unencumbered.
    Unencumbered = 0,
    /// Asset has been pledged as collateral (lien placed).
    Pledged = 1,
    /// Asset is locked for settlement (cannot be re-pledged).
    Locked = 2,
    /// Pledge has been released; asset is free again.
    Released = 3,
    /// Asset liquidated under enforcement action.
    Liquidated = 4,
}

impl CollateralState {
    /// Whether the asset can be used as collateral.
    pub fn can_pledge(&self) -> bool {
        matches!(self, CollateralState::Unencumbered)
    }

    /// Whether the asset is currently encumbered.
    pub fn is_encumbered(&self) -> bool {
        matches!(self, CollateralState::Pledged | CollateralState::Locked)
    }

    /// Whether a transition to `next` is valid.
    pub fn can_transition_to(&self, next: CollateralState) -> bool {
        use CollateralState::*;
        matches!(
            (self, next),
            (Unencumbered, Pledged)
                | (Pledged, Locked)
                | (Pledged, Released)
                | (Pledged, Liquidated)
                | (Locked, Released)
                | (Locked, Liquidated)
                | (Released, Pledged) // can re-pledge a released asset
        )
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            CollateralState::Unencumbered => "UNENCUMBERED",
            CollateralState::Pledged => "PLEDGED",
            CollateralState::Locked => "LOCKED",
            CollateralState::Released => "RELEASED",
            CollateralState::Liquidated => "LIQUIDATED",
        }
    }
}

// ---------------------------------------------------------------------------
// Core Data Structures
// ---------------------------------------------------------------------------

/// On-chain registration record for a custodian.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodianRecord {
    /// On-chain address identifying the custodian.
    pub custodian: Address,
    /// Current operational status.
    pub status: u8, // CustodianStatus as u8
    /// Reputation score [0, 100]; updated by the registry after each audit.
    pub reputation_score: u32,
    /// Total number of assets currently held.
    pub asset_count: u32,
    /// Aggregate notional value of all held assets (base units, e.g., USD cents).
    pub total_value_held: u128,
    /// Jurisdiction where the custodian is licensed.
    pub jurisdiction: Bytes, // e.g., b"US", b"SG"
    /// Timestamp of initial registration.
    pub registered_at: u64,
    /// Timestamp of most recent status update.
    pub last_updated: u64,
    /// Number of audits successfully completed.
    pub audit_count: u32,
    /// Number of compliance violations recorded.
    pub violation_count: u32,
    /// Hash of the custodian's legal agreement (off-chain reference).
    pub legal_agreement_hash: BytesN<32>,
    /// Insurance coverage amount (base units).
    pub insurance_coverage: u128,
    /// Supported asset classes (bitmask: bit i = AssetClass i is supported).
    pub supported_asset_classes: u32,
}

impl CustodianRecord {
    /// Whether this custodian supports a given asset class.
    pub fn supports_asset_class(&self, class: AssetClass) -> bool {
        (self.supported_asset_classes & (1 << class as u32)) != 0
    }

    /// Whether the custodian meets minimum reputation for a given asset class.
    pub fn meets_reputation_for(&self, class: AssetClass) -> bool {
        self.reputation_score >= class.min_custodian_reputation()
    }

    /// Whether the custodian is eligible to hold a new asset of the given class.
    pub fn is_eligible_for(&self, class: AssetClass) -> bool {
        self.status == CustodianStatus::Active as u8
            && self.supports_asset_class(class)
            && self.meets_reputation_for(class)
    }

    /// Compute a custody solvency ratio: insurance_coverage / total_value_held.
    /// Returns `None` if total_value_held is zero. Result is in basis points (×10_000).
    pub fn solvency_ratio_bps(&self) -> Option<u128> {
        if self.total_value_held == 0 {
            return None;
        }
        Some((self.insurance_coverage * 10_000) / self.total_value_held)
    }
}

/// Storage record for a custodied real-world asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetStorageRecord {
    /// Unique asset ID (content-addressed: sha256(custodian||asset_ref||timestamp)).
    pub asset_id: BytesN<32>,
    /// Custodian holding this asset.
    pub custodian: Address,
    /// Asset class.
    pub asset_class: u8, // AssetClass as u8
    /// Current collateral state.
    pub collateral_state: u8, // CollateralState as u8
    /// Notional value at time of last valuation (base units).
    pub notional_value: u128,
    /// Timestamp of last valuation.
    pub valued_at: u64,
    /// Hash of supporting documentation (title deeds, assay certificates, etc.).
    pub documentation_hash: BytesN<32>,
    /// Timestamp when this record was created.
    pub created_at: u64,
    /// Timestamp of last update.
    pub updated_at: u64,
    /// Owner of the asset (may differ from token holder during settlement).
    pub legal_owner: Address,
    /// Opaque off-chain reference (e.g., vault serial number, CUSIP, ISIN).
    pub external_ref: Bytes,
    /// Schema version.
    pub version: u32,
}

impl AssetStorageRecord {
    /// Whether this asset is freely tradeable (no encumbrance, valid custodian active).
    pub fn is_tradeable(&self) -> bool {
        let col_state = CollateralState::from_u8(self.collateral_state)
            .unwrap_or(CollateralState::Liquidated);
        col_state == CollateralState::Unencumbered
    }

    /// Whether this asset can be pledged as collateral.
    pub fn can_pledge(&self) -> bool {
        CollateralState::from_u8(self.collateral_state)
            .map(|s| s.can_pledge())
            .unwrap_or(false)
    }
}

impl CollateralState {
    /// Lossless conversion from raw u8.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(CollateralState::Unencumbered),
            1 => Some(CollateralState::Pledged),
            2 => Some(CollateralState::Locked),
            3 => Some(CollateralState::Released),
            4 => Some(CollateralState::Liquidated),
            _ => None,
        }
    }
}

/// A settlement instruction representing one leg of a DvP trade.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementInstruction {
    /// Unique instruction ID.
    pub instruction_id: BytesN<32>,
    /// Current settlement state.
    pub state: u8, // SettlementState as u8
    /// Delivering custodian.
    pub delivering_custodian: Address,
    /// Receiving custodian.
    pub receiving_custodian: Address,
    /// Asset being delivered.
    pub asset_id: BytesN<32>,
    /// Notional amount in the instruction (base units).
    pub notional: u128,
    /// Currency of the notional (e.g., b"USD").
    pub currency: Bytes,
    /// Settlement date/time (epoch seconds).
    pub settlement_date: u64,
    /// Timestamp instruction was created.
    pub created_at: u64,
    /// Timestamp of last state update.
    pub updated_at: u64,
    /// Optional reference to a paired payment leg.
    pub payment_leg_id: Option<BytesN<32>>,
    /// Number of retry attempts (for Failed → Initiated transitions).
    pub retry_count: u32,
    /// Reason code for Rejected or Failed states.
    pub failure_reason: Bytes,
}

impl SettlementInstruction {
    /// Whether this instruction is still in-flight.
    pub fn is_active(&self) -> bool {
        let state = SettlementState::from_u8(self.state).unwrap_or(SettlementState::Cancelled);
        !state.is_terminal()
    }

    /// Whether a retry is possible.
    pub fn can_retry(&self, max_retries: u32) -> bool {
        let state = SettlementState::from_u8(self.state).unwrap_or(SettlementState::Cancelled);
        state.is_retryable() && self.retry_count < max_retries
    }
}

impl SettlementState {
    /// Lossless conversion from raw u8.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(SettlementState::Initiated),
            1 => Some(SettlementState::AwaitingConfirmation),
            2 => Some(SettlementState::Confirmed),
            3 => Some(SettlementState::Settling),
            4 => Some(SettlementState::Settled),
            5 => Some(SettlementState::Rejected),
            6 => Some(SettlementState::Failed),
            7 => Some(SettlementState::Cancelled),
            _ => None,
        }
    }
}

/// Collateral management record — tracks a pledge of asset collateral.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralRecord {
    /// Unique collateral position ID.
    pub position_id: BytesN<32>,
    /// Asset pledged as collateral.
    pub asset_id: BytesN<32>,
    /// Custodian holding the collateral.
    pub custodian: Address,
    /// Beneficiary of the pledge (lender, margin desk, etc.).
    pub beneficiary: Address,
    /// Current collateral state.
    pub collateral_state: u8, // CollateralState as u8
    /// Pledged notional value at time of pledge (base units).
    pub pledged_value: u128,
    /// Loan-to-value ratio in basis points (e.g., 7000 = 70%).
    pub ltv_bps: u32,
    /// Margin call threshold in basis points (e.g., 8000 = 80% LTV triggers call).
    pub margin_call_threshold_bps: u32,
    /// Timestamp when the pledge was created.
    pub pledged_at: u64,
    /// Timestamp when the pledge was released (0 = still active).
    pub released_at: u64,
    /// Latest valuation of the underlying asset (base units).
    pub current_value: u128,
    /// Whether a margin call is currently active.
    pub margin_call_active: bool,
}

impl CollateralRecord {
    /// Current effective LTV based on latest valuation vs pledged value.
    /// Returns basis points (e.g., 7500 = 75%).
    pub fn effective_ltv_bps(&self) -> u32 {
        if self.current_value == 0 {
            return 10_000; // 100% — effectively worthless collateral
        }
        let loan_value = (self.pledged_value * self.ltv_bps as u128) / 10_000;
        ((loan_value * 10_000) / self.current_value) as u32
    }

    /// Whether a margin call should be triggered based on current valuation.
    pub fn should_trigger_margin_call(&self) -> bool {
        self.effective_ltv_bps() >= self.margin_call_threshold_bps
    }

    /// Whether the collateral position is still active (not released or liquidated).
    pub fn is_active(&self) -> bool {
        let state = CollateralState::from_u8(self.collateral_state)
            .unwrap_or(CollateralState::Liquidated);
        state.is_encumbered()
    }
}

// ---------------------------------------------------------------------------
// Custodian Reputation Tracker
// ---------------------------------------------------------------------------

/// Input signals used to adjust custodian reputation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationSignal {
    /// Positive: successful audit pass (+pts).
    pub audit_pass: bool,
    /// Negative: audit failure (-pts).
    pub audit_fail: bool,
    /// Negative: compliance violation recorded (-pts).
    pub compliance_violation: bool,
    /// Positive: successful settlement completed (+pts).
    pub settlement_success: bool,
    /// Negative: settlement failure (-pts).
    pub settlement_failure: bool,
    /// Positive: voluntary disclosure of near-miss (-pts are negative, so this adds pts).
    pub voluntary_disclosure: bool,
    /// Negative: insurance claim filed (-pts).
    pub insurance_claim: bool,
}

/// Stateless reputation scoring engine.
pub struct ReputationEngine;

impl ReputationEngine {
    /// Apply a set of reputation signals to a base score, returning the new score.
    ///
    /// Score is clamped to [0, 100].
    ///
    /// # Scoring table
    /// | Signal                | Δ pts |
    /// |------------------------|-------|
    /// | Audit pass             | +10   |
    /// | Audit fail             | -20   |
    /// | Compliance violation   | -15   |
    /// | Settlement success     | +2    |
    /// | Settlement failure     | -5    |
    /// | Voluntary disclosure   | +5    |
    /// | Insurance claim        | -10   |
    pub fn apply_signals(base_score: u32, signal: &ReputationSignal) -> u32 {
        let mut score = base_score as i64;
        if signal.audit_pass {
            score += 10;
        }
        if signal.audit_fail {
            score -= 20;
        }
        if signal.compliance_violation {
            score -= 15;
        }
        if signal.settlement_success {
            score += 2;
        }
        if signal.settlement_failure {
            score -= 5;
        }
        if signal.voluntary_disclosure {
            score += 5;
        }
        if signal.insurance_claim {
            score -= 10;
        }
        score.clamp(0, 100) as u32
    }

    /// Determine whether a custodian should be automatically suspended
    /// given their current reputation score and violation count.
    ///
    /// Auto-suspension triggers when:
    /// - Score drops below 30, OR
    /// - Violations reach 3 or more
    pub fn should_auto_suspend(reputation_score: u32, violation_count: u32) -> bool {
        reputation_score < 30 || violation_count >= 3
    }

    /// Compute a trust tier label from a reputation score.
    pub fn trust_tier(score: u32) -> &'static str {
        match score {
            0..=29 => "UNTRUSTED",
            30..=49 => "PROBATION",
            50..=69 => "STANDARD",
            70..=89 => "TRUSTED",
            _ => "PREMIUM",
        }
    }
}

// ---------------------------------------------------------------------------
// Custody Configuration
// ---------------------------------------------------------------------------

/// Configuration parameters for the custody registry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyConfig {
    /// Minimum reputation score for a custodian to accept any asset.
    pub min_reputation_for_activation: u32,
    /// Maximum number of settlement retries before permanent failure.
    pub max_settlement_retries: u32,
    /// LTV ceiling in basis points (e.g., 8000 = 80%).
    pub max_ltv_bps: u32,
    /// Global insurance coverage floor as a percentage of AUM (basis points).
    pub min_insurance_coverage_bps: u32,
    /// Whether Art assets require an independent third-party appraisal.
    pub require_art_appraisal: bool,
}

impl CustodyConfig {
    /// Conservative institutional defaults.
    pub fn institutional_defaults() -> Self {
        CustodyConfig {
            min_reputation_for_activation: 60,
            max_settlement_retries: 3,
            max_ltv_bps: 7500, // 75%
            min_insurance_coverage_bps: 10_000, // 100% — full coverage required
            require_art_appraisal: true,
        }
    }

    /// Whether the given LTV is within the allowed ceiling.
    pub fn is_valid_ltv(&self, ltv_bps: u32) -> bool {
        ltv_bps <= self.max_ltv_bps
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

    fn make_custodian(env: &Env, status: CustodianStatus, score: u32) -> CustodianRecord {
        CustodianRecord {
            custodian: Address::generate(env),
            status: status as u8,
            reputation_score: score,
            asset_count: 0,
            total_value_held: 0,
            jurisdiction: Bytes::from_slice(env, b"US"),
            registered_at: 1_000,
            last_updated: 1_000,
            audit_count: 0,
            violation_count: 0,
            legal_agreement_hash: zero_hash(env),
            insurance_coverage: 0,
            // All asset classes except Art (bit 6)
            supported_asset_classes: 0b0111_1111,
        }
    }

    fn make_asset(
        env: &Env,
        custodian: &Address,
        class: AssetClass,
        collateral: CollateralState,
        value: u128,
    ) -> AssetStorageRecord {
        AssetStorageRecord {
            asset_id: zero_hash(env),
            custodian: custodian.clone(),
            asset_class: class as u8,
            collateral_state: collateral as u8,
            notional_value: value,
            valued_at: 1_000,
            documentation_hash: zero_hash(env),
            created_at: 1_000,
            updated_at: 1_000,
            legal_owner: Address::generate(env),
            external_ref: Bytes::new(env),
            version: 1,
        }
    }

    fn make_instruction(env: &Env, state: SettlementState) -> SettlementInstruction {
        SettlementInstruction {
            instruction_id: zero_hash(env),
            state: state as u8,
            delivering_custodian: Address::generate(env),
            receiving_custodian: Address::generate(env),
            asset_id: zero_hash(env),
            notional: 100_000,
            currency: Bytes::from_slice(env, b"USD"),
            settlement_date: 10_000,
            created_at: 1_000,
            updated_at: 2_000,
            payment_leg_id: None,
            retry_count: 0,
            failure_reason: Bytes::new(env),
        }
    }

    fn make_collateral_record(
        env: &Env,
        state: CollateralState,
        pledged_value: u128,
        current_value: u128,
        ltv_bps: u32,
        threshold_bps: u32,
    ) -> CollateralRecord {
        CollateralRecord {
            position_id: zero_hash(env),
            asset_id: zero_hash(env),
            custodian: Address::generate(env),
            beneficiary: Address::generate(env),
            collateral_state: state as u8,
            pledged_value,
            ltv_bps,
            margin_call_threshold_bps: threshold_bps,
            pledged_at: 1_000,
            released_at: 0,
            current_value,
            margin_call_active: false,
        }
    }

    // --- CustodianStatus state machine ---

    #[test]
    fn test_custodian_status_can_accept_deposits_only_when_active() {
        assert!(!CustodianStatus::Applicant.can_accept_deposits());
        assert!(!CustodianStatus::Pending.can_accept_deposits());
        assert!(CustodianStatus::Active.can_accept_deposits());
        assert!(!CustodianStatus::Rejected.can_accept_deposits());
        assert!(!CustodianStatus::Suspended.can_accept_deposits());
        assert!(!CustodianStatus::Retired.can_accept_deposits());
        assert!(!CustodianStatus::Revoked.can_accept_deposits());
    }

    #[test]
    fn test_custodian_status_assets_frozen() {
        assert!(!CustodianStatus::Active.assets_frozen());
        assert!(CustodianStatus::Suspended.assets_frozen());
        assert!(CustodianStatus::Revoked.assets_frozen());
    }

    #[test]
    fn test_custodian_status_valid_transitions() {
        assert!(CustodianStatus::Applicant.can_transition_to(CustodianStatus::Pending));
        assert!(CustodianStatus::Pending.can_transition_to(CustodianStatus::Active));
        assert!(CustodianStatus::Pending.can_transition_to(CustodianStatus::Rejected));
        assert!(CustodianStatus::Active.can_transition_to(CustodianStatus::Suspended));
        assert!(CustodianStatus::Active.can_transition_to(CustodianStatus::Retired));
        assert!(CustodianStatus::Suspended.can_transition_to(CustodianStatus::Active));
        assert!(CustodianStatus::Suspended.can_transition_to(CustodianStatus::Revoked));
    }

    #[test]
    fn test_custodian_status_invalid_transitions() {
        assert!(!CustodianStatus::Applicant.can_transition_to(CustodianStatus::Active));
        assert!(!CustodianStatus::Active.can_transition_to(CustodianStatus::Applicant));
        assert!(!CustodianStatus::Revoked.can_transition_to(CustodianStatus::Active));
        assert!(!CustodianStatus::Rejected.can_transition_to(CustodianStatus::Active));
    }

    // --- AssetClass ---

    #[test]
    fn test_asset_class_min_reputation_ordered() {
        assert!(AssetClass::Art.min_custodian_reputation() > AssetClass::Commodity.min_custodian_reputation());
        assert!(AssetClass::PrivateCredit.min_custodian_reputation() > AssetClass::Bonds.min_custodian_reputation());
    }

    #[test]
    fn test_asset_class_requires_audit() {
        assert!(AssetClass::PreciousMetals.requires_audit());
        assert!(AssetClass::RealEstate.requires_audit());
        assert!(AssetClass::Art.requires_audit());
        assert!(AssetClass::PrivateCredit.requires_audit());
        assert!(!AssetClass::Equities.requires_audit());
        assert!(!AssetClass::Bonds.requires_audit());
        assert!(!AssetClass::Commodity.requires_audit());
    }

    // --- SettlementState state machine ---

    #[test]
    fn test_settlement_state_is_terminal() {
        assert!(!SettlementState::Initiated.is_terminal());
        assert!(!SettlementState::Settling.is_terminal());
        assert!(SettlementState::Settled.is_terminal());
        assert!(SettlementState::Rejected.is_terminal());
        assert!(SettlementState::Cancelled.is_terminal());
    }

    #[test]
    fn test_settlement_state_is_complete() {
        assert!(SettlementState::Settled.is_complete());
        assert!(!SettlementState::Rejected.is_complete());
        assert!(!SettlementState::Failed.is_complete());
    }

    #[test]
    fn test_settlement_state_is_retryable() {
        assert!(SettlementState::Failed.is_retryable());
        assert!(!SettlementState::Settled.is_retryable());
        assert!(!SettlementState::Rejected.is_retryable());
        assert!(!SettlementState::Cancelled.is_retryable());
    }

    #[test]
    fn test_settlement_state_valid_transitions() {
        assert!(SettlementState::Initiated.can_transition_to(SettlementState::AwaitingConfirmation));
        assert!(SettlementState::Initiated.can_transition_to(SettlementState::Cancelled));
        assert!(SettlementState::AwaitingConfirmation.can_transition_to(SettlementState::Confirmed));
        assert!(SettlementState::AwaitingConfirmation.can_transition_to(SettlementState::Rejected));
        assert!(SettlementState::Confirmed.can_transition_to(SettlementState::Settling));
        assert!(SettlementState::Settling.can_transition_to(SettlementState::Settled));
        assert!(SettlementState::Settling.can_transition_to(SettlementState::Failed));
        assert!(SettlementState::Failed.can_transition_to(SettlementState::Initiated));
    }

    #[test]
    fn test_settlement_state_invalid_transitions() {
        assert!(!SettlementState::Settled.can_transition_to(SettlementState::Initiated));
        assert!(!SettlementState::Initiated.can_transition_to(SettlementState::Settled));
        assert!(!SettlementState::Rejected.can_transition_to(SettlementState::Confirmed));
    }

    #[test]
    fn test_settlement_state_from_u8_roundtrip() {
        for v in 0u8..=7 {
            assert!(SettlementState::from_u8(v).is_some(), "failed for u8={v}");
        }
        assert!(SettlementState::from_u8(8).is_none());
        assert!(SettlementState::from_u8(255).is_none());
    }

    // --- CollateralState state machine ---

    #[test]
    fn test_collateral_state_can_pledge() {
        assert!(CollateralState::Unencumbered.can_pledge());
        assert!(!CollateralState::Pledged.can_pledge());
        assert!(!CollateralState::Locked.can_pledge());
        assert!(!CollateralState::Released.can_pledge());
        assert!(!CollateralState::Liquidated.can_pledge());
    }

    #[test]
    fn test_collateral_state_is_encumbered() {
        assert!(!CollateralState::Unencumbered.is_encumbered());
        assert!(CollateralState::Pledged.is_encumbered());
        assert!(CollateralState::Locked.is_encumbered());
        assert!(!CollateralState::Released.is_encumbered());
        assert!(!CollateralState::Liquidated.is_encumbered());
    }

    #[test]
    fn test_collateral_state_valid_transitions() {
        assert!(CollateralState::Unencumbered.can_transition_to(CollateralState::Pledged));
        assert!(CollateralState::Pledged.can_transition_to(CollateralState::Locked));
        assert!(CollateralState::Pledged.can_transition_to(CollateralState::Released));
        assert!(CollateralState::Pledged.can_transition_to(CollateralState::Liquidated));
        assert!(CollateralState::Locked.can_transition_to(CollateralState::Released));
        assert!(CollateralState::Locked.can_transition_to(CollateralState::Liquidated));
        assert!(CollateralState::Released.can_transition_to(CollateralState::Pledged));
    }

    #[test]
    fn test_collateral_state_invalid_transitions() {
        assert!(!CollateralState::Liquidated.can_transition_to(CollateralState::Unencumbered));
        assert!(!CollateralState::Unencumbered.can_transition_to(CollateralState::Liquidated));
        assert!(!CollateralState::Released.can_transition_to(CollateralState::Locked));
    }

    #[test]
    fn test_collateral_state_from_u8_roundtrip() {
        for v in 0u8..=4 {
            assert!(CollateralState::from_u8(v).is_some(), "failed for u8={v}");
        }
        assert!(CollateralState::from_u8(5).is_none());
    }

    // --- CustodianRecord ---

    #[test]
    fn test_custodian_supports_asset_class_bitmask() {
        let env = mock_env();
        let mut c = make_custodian(&env, CustodianStatus::Active, 70);
        // Bit 0 = PreciousMetals
        c.supported_asset_classes = 1 << AssetClass::PreciousMetals as u32;
        assert!(c.supports_asset_class(AssetClass::PreciousMetals));
        assert!(!c.supports_asset_class(AssetClass::RealEstate));
    }

    #[test]
    fn test_custodian_meets_reputation() {
        let env = mock_env();
        let c = make_custodian(&env, CustodianStatus::Active, 60);
        assert!(c.meets_reputation_for(AssetClass::PreciousMetals)); // min=60
        assert!(!c.meets_reputation_for(AssetClass::Art)); // min=80
    }

    #[test]
    fn test_custodian_is_eligible_for() {
        let env = mock_env();
        let c = make_custodian(&env, CustodianStatus::Active, 70);
        // All asset classes except Art are supported (bitmask 0b0111_1111)
        assert!(c.is_eligible_for(AssetClass::PreciousMetals));
        assert!(c.is_eligible_for(AssetClass::Bonds));
        assert!(!c.is_eligible_for(AssetClass::Art)); // not in supported mask
    }

    #[test]
    fn test_custodian_not_eligible_when_suspended() {
        let env = mock_env();
        let c = make_custodian(&env, CustodianStatus::Suspended, 90);
        assert!(!c.is_eligible_for(AssetClass::Bonds));
    }

    #[test]
    fn test_custodian_solvency_ratio_zero_aum() {
        let env = mock_env();
        let c = make_custodian(&env, CustodianStatus::Active, 70);
        // total_value_held = 0
        assert_eq!(c.solvency_ratio_bps(), None);
    }

    #[test]
    fn test_custodian_solvency_ratio_full_coverage() {
        let env = mock_env();
        let mut c = make_custodian(&env, CustodianStatus::Active, 70);
        c.total_value_held = 1_000_000;
        c.insurance_coverage = 1_000_000; // 100% coverage
        assert_eq!(c.solvency_ratio_bps(), Some(10_000)); // 10000 bps = 100%
    }

    #[test]
    fn test_custodian_solvency_ratio_partial() {
        let env = mock_env();
        let mut c = make_custodian(&env, CustodianStatus::Active, 70);
        c.total_value_held = 2_000_000;
        c.insurance_coverage = 1_000_000; // 50% coverage
        assert_eq!(c.solvency_ratio_bps(), Some(5_000)); // 5000 bps = 50%
    }

    // --- AssetStorageRecord ---

    #[test]
    fn test_asset_storage_is_tradeable_only_unencumbered() {
        let env = mock_env();
        let custodian = Address::generate(&env);
        let asset_free = make_asset(&env, &custodian, AssetClass::Bonds, CollateralState::Unencumbered, 1_000);
        let asset_pledged = make_asset(&env, &custodian, AssetClass::Bonds, CollateralState::Pledged, 1_000);
        assert!(asset_free.is_tradeable());
        assert!(!asset_pledged.is_tradeable());
    }

    #[test]
    fn test_asset_storage_can_pledge() {
        let env = mock_env();
        let custodian = Address::generate(&env);
        let free = make_asset(&env, &custodian, AssetClass::Bonds, CollateralState::Unencumbered, 1_000);
        let pledged = make_asset(&env, &custodian, AssetClass::Bonds, CollateralState::Pledged, 1_000);
        let liquidated = make_asset(&env, &custodian, AssetClass::Bonds, CollateralState::Liquidated, 1_000);
        assert!(free.can_pledge());
        assert!(!pledged.can_pledge());
        assert!(!liquidated.can_pledge());
    }

    // --- SettlementInstruction ---

    #[test]
    fn test_settlement_instruction_is_active() {
        let env = mock_env();
        let active = make_instruction(&env, SettlementState::Settling);
        let terminal = make_instruction(&env, SettlementState::Settled);
        assert!(active.is_active());
        assert!(!terminal.is_active());
    }

    #[test]
    fn test_settlement_instruction_can_retry() {
        let env = mock_env();
        let mut failed = make_instruction(&env, SettlementState::Failed);
        failed.retry_count = 2;
        assert!(failed.can_retry(3));
        failed.retry_count = 3;
        assert!(!failed.can_retry(3)); // at limit
    }

    #[test]
    fn test_settlement_instruction_no_retry_when_settled() {
        let env = mock_env();
        let settled = make_instruction(&env, SettlementState::Settled);
        assert!(!settled.can_retry(5));
    }

    // --- CollateralRecord ---

    #[test]
    fn test_collateral_effective_ltv_at_par() {
        let env = mock_env();
        // pledged_value=100k, ltv=70% → loan=70k; current_value=100k → effective=70%
        let rec = make_collateral_record(&env, CollateralState::Pledged, 100_000, 100_000, 7000, 8000);
        assert_eq!(rec.effective_ltv_bps(), 7000);
    }

    #[test]
    fn test_collateral_effective_ltv_value_drops() {
        let env = mock_env();
        // pledged_value=100k, ltv=70% → loan=70k; current_value=80k → effective=70k/80k ≈ 87.5%
        let rec = make_collateral_record(&env, CollateralState::Pledged, 100_000, 80_000, 7000, 8000);
        let effective = rec.effective_ltv_bps();
        assert!(effective > 8000, "Expected LTV > 8000, got {effective}");
    }

    #[test]
    fn test_collateral_margin_call_triggered() {
        let env = mock_env();
        // Asset value drops to 80k → LTV rises above 80% threshold
        let rec = make_collateral_record(&env, CollateralState::Pledged, 100_000, 80_000, 7000, 8000);
        assert!(rec.should_trigger_margin_call());
    }

    #[test]
    fn test_collateral_no_margin_call_at_par() {
        let env = mock_env();
        let rec = make_collateral_record(&env, CollateralState::Pledged, 100_000, 100_000, 7000, 8000);
        assert!(!rec.should_trigger_margin_call());
    }

    #[test]
    fn test_collateral_effective_ltv_zero_current_value() {
        let env = mock_env();
        let rec = make_collateral_record(&env, CollateralState::Pledged, 100_000, 0, 7000, 8000);
        // Zero current value → returns 10000 (100%)
        assert_eq!(rec.effective_ltv_bps(), 10_000);
    }

    #[test]
    fn test_collateral_is_active() {
        let env = mock_env();
        let pledged = make_collateral_record(&env, CollateralState::Pledged, 1_000, 1_000, 7000, 8000);
        let released = make_collateral_record(&env, CollateralState::Released, 1_000, 1_000, 7000, 8000);
        assert!(pledged.is_active());
        assert!(!released.is_active());
    }

    // --- ReputationEngine ---

    #[test]
    fn test_reputation_audit_pass_increases_score() {
        let signal = ReputationSignal {
            audit_pass: true,
            audit_fail: false,
            compliance_violation: false,
            settlement_success: false,
            settlement_failure: false,
            voluntary_disclosure: false,
            insurance_claim: false,
        };
        let score = ReputationEngine::apply_signals(60, &signal);
        assert_eq!(score, 70);
    }

    #[test]
    fn test_reputation_violation_decreases_score() {
        let signal = ReputationSignal {
            audit_pass: false,
            audit_fail: false,
            compliance_violation: true,
            settlement_success: false,
            settlement_failure: false,
            voluntary_disclosure: false,
            insurance_claim: false,
        };
        let score = ReputationEngine::apply_signals(60, &signal);
        assert_eq!(score, 45);
    }

    #[test]
    fn test_reputation_score_clamped_at_100() {
        let signal = ReputationSignal {
            audit_pass: true,
            audit_fail: false,
            compliance_violation: false,
            settlement_success: true,
            settlement_failure: false,
            voluntary_disclosure: true,
            insurance_claim: false,
        };
        // 98 + 10 + 2 + 5 = 115, clamped to 100
        let score = ReputationEngine::apply_signals(98, &signal);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_reputation_score_clamped_at_zero() {
        let signal = ReputationSignal {
            audit_pass: false,
            audit_fail: true,
            compliance_violation: true,
            settlement_success: false,
            settlement_failure: true,
            voluntary_disclosure: false,
            insurance_claim: true,
        };
        // 5 - 20 - 15 - 5 - 10 = -45 → clamped to 0
        let score = ReputationEngine::apply_signals(5, &signal);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_reputation_auto_suspend_triggers_on_low_score() {
        assert!(ReputationEngine::should_auto_suspend(29, 0));
        assert!(!ReputationEngine::should_auto_suspend(30, 0));
    }

    #[test]
    fn test_reputation_auto_suspend_triggers_on_violations() {
        assert!(ReputationEngine::should_auto_suspend(90, 3));
        assert!(!ReputationEngine::should_auto_suspend(90, 2));
    }

    #[test]
    fn test_reputation_trust_tier_labels() {
        assert_eq!(ReputationEngine::trust_tier(0), "UNTRUSTED");
        assert_eq!(ReputationEngine::trust_tier(29), "UNTRUSTED");
        assert_eq!(ReputationEngine::trust_tier(30), "PROBATION");
        assert_eq!(ReputationEngine::trust_tier(50), "STANDARD");
        assert_eq!(ReputationEngine::trust_tier(70), "TRUSTED");
        assert_eq!(ReputationEngine::trust_tier(90), "PREMIUM");
        assert_eq!(ReputationEngine::trust_tier(100), "PREMIUM");
    }

    // --- CustodyConfig ---

    #[test]
    fn test_custody_config_ltv_validation() {
        let config = CustodyConfig::institutional_defaults();
        assert!(config.is_valid_ltv(7500));
        assert!(!config.is_valid_ltv(7501));
        assert!(config.is_valid_ltv(0));
    }

    #[test]
    fn test_custody_config_full_insurance_floor() {
        let config = CustodyConfig::institutional_defaults();
        // 100% coverage floor
        assert_eq!(config.min_insurance_coverage_bps, 10_000);
    }
}
