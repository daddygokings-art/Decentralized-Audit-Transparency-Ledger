//! Real-World Asset (RWA) tokenization types for on-chain audit and compliance.
//!
//! This module defines all core data structures used across the RWA integration:
//! asset classes, lifecycle states, participant roles, valuation records,
//! compliance status, transfer structures, and collateral positions.
#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ── Asset Class Taxonomy ──────────────────────────────────────────────────────

/// Top-level classification of a real-world asset.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AssetClass {
    /// Commercial or residential real estate
    RealEstate = 0,
    /// Publicly or privately traded equity
    Equity = 1,
    /// Government or corporate bonds and notes
    Debt = 2,
    /// Physical commodities (gold, oil, grain …)
    Commodity = 3,
    /// Intellectual property, patents, royalties
    IntellectualProperty = 4,
    /// Infrastructure (bridges, power plants, toll roads)
    Infrastructure = 5,
    /// Accounts receivable and trade finance
    TradeFinance = 6,
    /// Private equity / venture capital fund interests
    PrivateFund = 7,
}

impl AssetClass {
    /// Return a short Symbol tag suitable for on-chain storage keys.
    pub fn as_symbol(&self) -> Symbol {
        match self {
            AssetClass::RealEstate => Symbol::new(&[b"REAL_EST"]),
            AssetClass::Equity => Symbol::new(&[b"EQUITY"]),
            AssetClass::Debt => Symbol::new(&[b"DEBT"]),
            AssetClass::Commodity => Symbol::new(&[b"COMMODITY"]),
            AssetClass::IntellectualProperty => Symbol::new(&[b"IP"]),
            AssetClass::Infrastructure => Symbol::new(&[b"INFRA"]),
            AssetClass::TradeFinance => Symbol::new(&[b"TRADE_FIN"]),
            AssetClass::PrivateFund => Symbol::new(&[b"PVT_FUND"]),
        }
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            AssetClass::RealEstate => "Real Estate",
            AssetClass::Equity => "Equity",
            AssetClass::Debt => "Debt / Fixed Income",
            AssetClass::Commodity => "Commodity",
            AssetClass::IntellectualProperty => "Intellectual Property",
            AssetClass::Infrastructure => "Infrastructure",
            AssetClass::TradeFinance => "Trade Finance",
            AssetClass::PrivateFund => "Private Fund",
        }
    }

    /// Whether the class typically requires independent appraisal before issuance.
    pub fn requires_appraisal(&self) -> bool {
        matches!(
            self,
            AssetClass::RealEstate
                | AssetClass::Infrastructure
                | AssetClass::IntellectualProperty
                | AssetClass::Commodity
        )
    }

    /// Minimum valuation frequency in days required by typical regulatory guidance.
    pub fn min_valuation_frequency_days(&self) -> u32 {
        match self {
            AssetClass::RealEstate => 365,
            AssetClass::Equity => 1,
            AssetClass::Debt => 1,
            AssetClass::Commodity => 1,
            AssetClass::IntellectualProperty => 180,
            AssetClass::Infrastructure => 365,
            AssetClass::TradeFinance => 30,
            AssetClass::PrivateFund => 90,
        }
    }

    /// Construct from a u8 discriminant; returns `None` for unknown values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(AssetClass::RealEstate),
            1 => Some(AssetClass::Equity),
            2 => Some(AssetClass::Debt),
            3 => Some(AssetClass::Commodity),
            4 => Some(AssetClass::IntellectualProperty),
            5 => Some(AssetClass::Infrastructure),
            6 => Some(AssetClass::TradeFinance),
            7 => Some(AssetClass::PrivateFund),
            _ => None,
        }
    }
}

// ── Lifecycle States ──────────────────────────────────────────────────────────

/// Current operational state of an on-chain RWA token.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TokenizationStatus {
    /// Legal and technical structuring in progress.
    Draft = 0,
    /// Submitted to regulator / compliance team for review.
    PendingApproval = 1,
    /// Approved: tokens may be issued and transferred.
    Active = 2,
    /// Temporarily halted (e.g., pending re-valuation or compliance hold).
    Suspended = 3,
    /// Redemption window open; no new issuances.
    Redeeming = 4,
    /// All tokens redeemed; record kept for audit purposes only.
    Matured = 5,
    /// Rejected during approval review.
    Rejected = 6,
}

impl TokenizationStatus {
    /// `true` if new token issuance is permitted in this state.
    pub fn allows_issuance(&self) -> bool {
        matches!(self, TokenizationStatus::Active)
    }

    /// `true` if token transfers are permitted in this state.
    pub fn allows_transfer(&self) -> bool {
        matches!(self, TokenizationStatus::Active | TokenizationStatus::Redeeming)
    }

    /// `true` if the lifecycle has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TokenizationStatus::Matured | TokenizationStatus::Rejected
        )
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(TokenizationStatus::Draft),
            1 => Some(TokenizationStatus::PendingApproval),
            2 => Some(TokenizationStatus::Active),
            3 => Some(TokenizationStatus::Suspended),
            4 => Some(TokenizationStatus::Redeeming),
            5 => Some(TokenizationStatus::Matured),
            6 => Some(TokenizationStatus::Rejected),
            _ => None,
        }
    }
}

// ── Compliance Framework ──────────────────────────────────────────────────────

/// Jurisdictional / regulatory compliance frameworks applicable to a token.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ComplianceFramework {
    /// U.S. Securities and Exchange Commission regulations.
    SEC = 0,
    /// EU Markets in Crypto-Assets regulation.
    MiCA = 1,
    /// EU Alternative Investment Fund Managers Directive.
    AIFMD = 2,
    /// U.S. Commodity Futures Trading Commission regulations.
    CFTC = 3,
    /// Basel III / IV banking capital requirements.
    BaselIII = 4,
    /// Financial Action Task Force (AML/CFT standards).
    FATF = 5,
    /// U.S. Regulation D private placement exemption.
    RegD = 6,
    /// U.S. Regulation S offshore offering exemption.
    RegS = 7,
}

impl ComplianceFramework {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComplianceFramework::SEC => "SEC",
            ComplianceFramework::MiCA => "MiCA",
            ComplianceFramework::AIFMD => "AIFMD",
            ComplianceFramework::CFTC => "CFTC",
            ComplianceFramework::BaselIII => "Basel III",
            ComplianceFramework::FATF => "FATF",
            ComplianceFramework::RegD => "Reg D",
            ComplianceFramework::RegS => "Reg S",
        }
    }

    /// `true` if KYC/AML screening is mandated before transfer under this framework.
    pub fn requires_kyc(&self) -> bool {
        !matches!(self, ComplianceFramework::RegS)
    }

    /// `true` if periodic auditor sign-off on financial statements is required.
    pub fn requires_periodic_audit(&self) -> bool {
        matches!(
            self,
            ComplianceFramework::SEC
                | ComplianceFramework::MiCA
                | ComplianceFramework::AIFMD
                | ComplianceFramework::BaselIII
        )
    }
}

// ── KYC / AML Tiers ──────────────────────────────────────────────────────────

/// Know-Your-Customer verification tier for an investor or issuer.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum KycTier {
    /// No verification performed.
    None = 0,
    /// Basic identity verified (name + government ID).
    Basic = 1,
    /// Enhanced due diligence (source of funds, beneficial ownership).
    Enhanced = 2,
    /// Institutional-grade checks (full corporate structure, audited financials).
    Institutional = 3,
}

impl KycTier {
    /// Minimum required tier to participate in regulated token transfers.
    pub fn minimum_for_transfer() -> KycTier {
        KycTier::Basic
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(KycTier::None),
            1 => Some(KycTier::Basic),
            2 => Some(KycTier::Enhanced),
            3 => Some(KycTier::Institutional),
            _ => None,
        }
    }

    /// `true` if this tier satisfies the `required` tier.
    pub fn satisfies(&self, required: KycTier) -> bool {
        (*self as u8) >= (required as u8)
    }
}

// ── Core Token Record ─────────────────────────────────────────────────────────

/// Primary on-chain representation of a tokenized real-world asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RwaToken {
    /// Content-addressed unique ID derived from issuer + asset hash + timestamp.
    pub token_id: BytesN<32>,
    /// ISIN, CUSIP, or other off-chain identifier (UTF-8, max 24 bytes).
    pub external_id: Bytes,
    /// Human-readable name (max 64 bytes).
    pub name: Bytes,
    /// Asset class discriminant.
    pub asset_class: u8, // AssetClass as u8
    /// Current lifecycle state.
    pub status: u8, // TokenizationStatus as u8
    /// Address of the issuing entity.
    pub issuer: Address,
    /// Applicable compliance framework discriminant.
    pub compliance_framework: u8, // ComplianceFramework as u8
    /// Total supply of fractional tokens, denominated in the smallest unit (e.g., 1e-8).
    pub total_supply: u128,
    /// Number of tokens currently in circulation (issued minus redeemed).
    pub circulating_supply: u128,
    /// Latest valuation in USD cents (u64 provides ~$184 billion at cent precision).
    pub latest_valuation_usd_cents: u64,
    /// Unix timestamp of the latest valuation.
    pub valuation_timestamp: u64,
    /// Unix timestamp of token creation.
    pub created_at: u64,
    /// Unix timestamp of last status change.
    pub updated_at: u64,
    /// Arbitrary opaque metadata (legal docs hash, offering memorandum CID, …).
    pub metadata: Bytes,
    /// SHA-256 of the previous token state for tamper-evident history.
    pub prev_state_hash: BytesN<32>,
}

impl RwaToken {
    /// Create a new RWA token in `Draft` state.
    pub fn new(
        token_id: BytesN<32>,
        external_id: Bytes,
        name: Bytes,
        asset_class: AssetClass,
        issuer: Address,
        compliance_framework: ComplianceFramework,
        total_supply: u128,
        metadata: Bytes,
        now: u64,
        prev_state_hash: BytesN<32>,
    ) -> Self {
        RwaToken {
            token_id,
            external_id,
            name,
            asset_class: asset_class as u8,
            status: TokenizationStatus::Draft as u8,
            issuer,
            compliance_framework: compliance_framework as u8,
            total_supply,
            circulating_supply: 0,
            latest_valuation_usd_cents: 0,
            valuation_timestamp: 0,
            created_at: now,
            updated_at: now,
            metadata,
            prev_state_hash,
        }
    }

    /// Decode the asset class.
    pub fn asset_class(&self) -> Option<AssetClass> {
        AssetClass::from_u8(self.asset_class)
    }

    /// Decode the lifecycle status.
    pub fn status(&self) -> Option<TokenizationStatus> {
        TokenizationStatus::from_u8(self.status)
    }

    /// `true` if the token is currently live for trading.
    pub fn is_active(&self) -> bool {
        self.status == TokenizationStatus::Active as u8
    }

    /// Remaining supply available for issuance.
    pub fn remaining_supply(&self) -> u128 {
        self.total_supply.saturating_sub(self.circulating_supply)
    }

    /// Token price per unit in USD cents derived from valuation and total supply.
    /// Returns 0 if total supply is zero or valuation is unset.
    pub fn price_per_token_usd_cents(&self) -> u64 {
        if self.total_supply == 0 || self.latest_valuation_usd_cents == 0 {
            return 0;
        }
        (self.latest_valuation_usd_cents as u128)
            .checked_div(self.total_supply)
            .unwrap_or(0) as u64
    }
}

// ── Investor / Holder Record ──────────────────────────────────────────────────

/// On-chain KYC/AML profile for an investor participating in RWA markets.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvestorProfile {
    /// Unique profile ID (sha256 of investor address + timestamp).
    pub profile_id: BytesN<32>,
    /// Stellar address of the investor.
    pub address: Address,
    /// KYC tier achieved.
    pub kyc_tier: u8, // KycTier as u8
    /// Whether AML screening has been completed.
    pub aml_cleared: bool,
    /// Whether investor is accredited (for Reg D / Reg S purposes).
    pub is_accredited: bool,
    /// Jurisdiction of residence (ISO 3166-1 alpha-2, 2 bytes).
    pub jurisdiction: Bytes,
    /// Unix timestamp when KYC was last verified.
    pub kyc_verified_at: u64,
    /// Unix timestamp when KYC expires (0 = no expiry).
    pub kyc_expiry: u64,
    /// Profile creation timestamp.
    pub created_at: u64,
    /// Whether the investor account is currently restricted.
    pub is_restricted: bool,
    /// Optional compliance notes.
    pub notes: Bytes,
}

impl InvestorProfile {
    /// `true` if KYC satisfies the minimum tier for transfers and has not expired.
    pub fn is_kyc_valid(&self, now: u64) -> bool {
        let tier = KycTier::from_u8(self.kyc_tier).unwrap_or(KycTier::None);
        let tier_ok = tier.satisfies(KycTier::minimum_for_transfer());
        let not_expired = self.kyc_expiry == 0 || now < self.kyc_expiry;
        tier_ok && not_expired && !self.is_restricted
    }
}

// ── Valuation Record ──────────────────────────────────────────────────────────

/// Immutable point-in-time valuation snapshot for an RWA token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValuationRecord {
    /// Sequential valuation index for this token (0-based).
    pub valuation_index: u32,
    /// Token this valuation applies to.
    pub token_id: BytesN<32>,
    /// Valuation in USD cents.
    pub value_usd_cents: u64,
    /// Valuation methodology identifier (e.g., "DCF", "COMPARABLE", "APPRAISAL").
    pub methodology: Bytes,
    /// Address of the appraiser / oracle submitting this record.
    pub appraiser: Address,
    /// Unix timestamp of the valuation date.
    pub valuation_date: u64,
    /// Unix timestamp when this record was logged on-chain.
    pub logged_at: u64,
    /// SHA-256 of supporting off-chain appraisal document.
    pub document_hash: BytesN<32>,
    /// Confidence score 0-100 (0 = no confidence, 100 = certain).
    pub confidence_score: u8,
    /// `true` if this is an independent third-party appraisal.
    pub is_independent: bool,
}

// ── Transfer Record ───────────────────────────────────────────────────────────

/// Represents a token transfer between two investors with full compliance context.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRecord {
    /// Content-addressed transfer ID.
    pub transfer_id: BytesN<32>,
    /// Token being transferred.
    pub token_id: BytesN<32>,
    /// Sender Stellar address.
    pub from: Address,
    /// Recipient Stellar address.
    pub to: Address,
    /// Number of fractional token units transferred.
    pub amount: u128,
    /// Transfer price in USD cents per token unit (0 if off-market / gift).
    pub price_per_token_usd_cents: u64,
    /// Unix timestamp of transfer execution.
    pub timestamp: u64,
    /// Whether compliance checks passed.
    pub compliance_passed: bool,
    /// Optional compliance check reference (e.g., trace ID from AML provider).
    pub compliance_ref: Bytes,
    /// Transfer type: "primary", "secondary", "redemption", "collateral".
    pub transfer_type: Symbol,
}

// ── Collateral Position ───────────────────────────────────────────────────────

/// Represents an RWA token locked as collateral for a lending or repo agreement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralPosition {
    /// Unique position ID.
    pub position_id: BytesN<32>,
    /// Token pledged as collateral.
    pub token_id: BytesN<32>,
    /// Owner of the collateral.
    pub borrower: Address,
    /// Lender / counterparty address.
    pub lender: Address,
    /// Token units locked.
    pub locked_amount: u128,
    /// Collateral value at time of lock in USD cents.
    pub locked_value_usd_cents: u64,
    /// Loan-to-value ratio in basis points (e.g., 7000 = 70%).
    pub ltv_bps: u32,
    /// Position created timestamp.
    pub created_at: u64,
    /// Maturity timestamp (0 = open-ended).
    pub maturity: u64,
    /// Whether this position is currently active.
    pub is_active: bool,
    /// Whether a margin call has been triggered.
    pub margin_call_triggered: bool,
}

impl CollateralPosition {
    /// Compute current LTV given a new mark-to-market value.
    /// Returns basis points (e.g., 8500 = 85% LTV).
    pub fn current_ltv_bps(&self, current_value_usd_cents: u64) -> u32 {
        if current_value_usd_cents == 0 {
            return 10_000; // 100% (underwater)
        }
        let loan_value = self.locked_value_usd_cents as u128 * self.ltv_bps as u128 / 10_000;
        let ltv = loan_value * 10_000 / current_value_usd_cents as u128;
        ltv.min(10_000) as u32
    }

    /// `true` if LTV has breached the provided liquidation threshold (in bps).
    pub fn is_undercollateralized(&self, current_value_usd_cents: u64, threshold_bps: u32) -> bool {
        self.current_ltv_bps(current_value_usd_cents) >= threshold_bps
    }
}

// ── Dividend / Distribution Record ───────────────────────────────────────────

/// Records a cash or in-kind distribution to token holders.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionRecord {
    /// Sequential distribution index for this token.
    pub distribution_index: u32,
    /// Token for which the distribution is declared.
    pub token_id: BytesN<32>,
    /// Distribution type: "dividend", "interest", "return_of_capital", "coupon".
    pub distribution_type: Symbol,
    /// Total amount to be distributed in USD cents.
    pub total_amount_usd_cents: u64,
    /// Amount per token in USD cents (total / circulating_supply).
    pub per_token_usd_cents: u64,
    /// Record date (snapshot date for eligibility), Unix timestamp.
    pub record_date: u64,
    /// Payment date, Unix timestamp.
    pub payment_date: u64,
    /// Declared by issuer address.
    pub declared_by: Address,
    /// Whether the distribution has been paid out.
    pub is_paid: bool,
}

// ── Compliance Check Record ───────────────────────────────────────────────────

/// Tracks the outcome of an automated or manual compliance check.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceCheck {
    /// Check ID.
    pub check_id: BytesN<32>,
    /// Entity being checked (investor, issuer, or token ID encoded as bytes).
    pub subject: Bytes,
    /// Check type: "kyc", "aml", "sanctions", "accreditation", "transfer_restriction".
    pub check_type: Symbol,
    /// Result: "pass", "fail", "pending", "manual_review".
    pub result: Symbol,
    /// Score 0-100 (higher = cleaner).
    pub score: u8,
    /// Unix timestamp of check.
    pub checked_at: u64,
    /// Address of entity that performed the check (oracle or admin).
    pub checked_by: Address,
    /// Optional human-readable finding.
    pub finding: Bytes,
    /// Whether the check must be re-run before next transfer.
    pub requires_refresh: bool,
}

// ── RWA Configuration ─────────────────────────────────────────────────────────

/// Global configuration for the RWA integration layer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RwaConfig {
    /// Maximum number of tokens that can be registered.
    pub max_tokens: u32,
    /// Current registered token count.
    pub token_count: u32,
    /// Default compliance framework for new tokens.
    pub default_framework: u8, // ComplianceFramework as u8
    /// Minimum KYC tier required for secondary market transfers.
    pub min_kyc_tier: u8, // KycTier as u8
    /// Maximum metadata size in bytes.
    pub max_metadata_size: u32,
    /// Whether the RWA module is globally paused.
    pub is_paused: bool,
    /// Whether accreditation verification is enforced.
    pub enforce_accreditation: bool,
    /// Whether AML checks are mandatory before all transfers.
    pub enforce_aml: bool,
    /// Configuration version for migration tracking.
    pub version: u32,
}

impl RwaConfig {
    /// Production-safe defaults.
    pub fn default_config() -> Self {
        RwaConfig {
            max_tokens: 10_000,
            token_count: 0,
            default_framework: ComplianceFramework::SEC as u8,
            min_kyc_tier: KycTier::Basic as u8,
            max_metadata_size: 4096,
            is_paused: false,
            enforce_accreditation: true,
            enforce_aml: true,
            version: 1,
        }
    }

    /// Permissive configuration for sandbox / testing environments.
    pub fn sandbox_config() -> Self {
        RwaConfig {
            max_tokens: 100,
            token_count: 0,
            default_framework: ComplianceFramework::RegD as u8,
            min_kyc_tier: KycTier::None as u8,
            max_metadata_size: 1024,
            is_paused: false,
            enforce_accreditation: false,
            enforce_aml: false,
            version: 1,
        }
    }

    /// `true` if registration of a new token would exceed the max cap.
    pub fn can_register_token(&self) -> bool {
        !self.is_paused && self.token_count < self.max_tokens
    }
}
