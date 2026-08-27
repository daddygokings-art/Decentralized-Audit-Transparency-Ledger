//! # RWA Tokenization Module
//!
//! Implements Real World Asset (RWA) tokenization primitives on top of the
//! Soroban audit ledger:
//!
//! - Token lifecycle: creation → active → paused → retired
//! - Mint / burn management with supply caps
//! - Transfer restrictions (allowlists, blocklists, jurisdiction gates)
//! - Holder registry with per-holder balance and KYC status
//!
//! All state mutations emit a structured Soroban event so the off-chain
//! monitoring stack can index them without polling storage.

#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

// ─────────────────────────────────────────────────────────────────────────────
// Enumerations
// ─────────────────────────────────────────────────────────────────────────────

/// Asset classes that can be represented as on-chain tokens.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AssetClass {
    /// Commercial or residential real estate
    RealEstate = 0,
    /// Private equity or venture capital fund interests
    PrivateEquity = 1,
    /// Corporate or sovereign bond instruments
    Debt = 2,
    /// Physical commodities (gold, oil, agricultural)
    Commodity = 3,
    /// Fine art, collectibles, IP rights
    AlternativeAsset = 4,
    /// Infrastructure projects (energy, transport)
    Infrastructure = 5,
}

impl AssetClass {
    /// Human-readable label used in filings and reports.
    pub fn label(&self) -> &'static str {
        match self {
            AssetClass::RealEstate => "Real Estate",
            AssetClass::PrivateEquity => "Private Equity",
            AssetClass::Debt => "Debt Instrument",
            AssetClass::Commodity => "Commodity",
            AssetClass::AlternativeAsset => "Alternative Asset",
            AssetClass::Infrastructure => "Infrastructure",
        }
    }

    /// Whether fractional (sub-unit) ownership is permitted for this class.
    pub fn allows_fractional(&self) -> bool {
        matches!(
            self,
            AssetClass::RealEstate
                | AssetClass::Debt
                | AssetClass::Infrastructure
        )
    }

    /// Default decimal precision for token amounts.
    pub fn default_decimals(&self) -> u8 {
        match self {
            AssetClass::RealEstate | AssetClass::Debt | AssetClass::Infrastructure => 6,
            AssetClass::PrivateEquity => 4,
            AssetClass::Commodity => 8,
            AssetClass::AlternativeAsset => 0, // whole units only
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tokenization State Machine
// ─────────────────────────────────────────────────────────────────────────────

/// Lifecycle states for a tokenized asset.
///
/// Valid transitions:
/// ```text
/// Proposed → Approved → Active ⇄ Paused → Retired
///         ↘ Rejected (terminal)
/// ```
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TokenizationState {
    /// Token proposal submitted; awaiting compliance review.
    Proposed = 0,
    /// Passed compliance review; ready for first mint.
    Approved = 1,
    /// Live — minting, burning, and transfers are enabled.
    Active = 2,
    /// Temporarily suspended; no transfers or new mints.
    Paused = 3,
    /// Permanently decommissioned; all supply burned.
    Retired = 4,
    /// Proposal rejected; no further transitions permitted.
    Rejected = 5,
}

impl TokenizationState {
    /// Returns `true` when `to` is a valid next state from `self`.
    pub fn can_transition_to(&self, to: TokenizationState) -> bool {
        matches!(
            (self, to),
            (TokenizationState::Proposed, TokenizationState::Approved)
                | (TokenizationState::Proposed, TokenizationState::Rejected)
                | (TokenizationState::Approved, TokenizationState::Active)
                | (TokenizationState::Active, TokenizationState::Paused)
                | (TokenizationState::Paused, TokenizationState::Active)
                | (TokenizationState::Active, TokenizationState::Retired)
                | (TokenizationState::Paused, TokenizationState::Retired)
        )
    }

    /// Whether mint/burn operations are allowed in this state.
    pub fn allows_mint_burn(&self) -> bool {
        matches!(self, TokenizationState::Active)
    }

    /// Whether holder-to-holder transfers are allowed in this state.
    pub fn allows_transfer(&self) -> bool {
        matches!(self, TokenizationState::Active)
    }

    /// Whether the state is a terminal (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TokenizationState::Retired | TokenizationState::Rejected
        )
    }

    /// String label emitted in on-chain events.
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenizationState::Proposed => "PROPOSED",
            TokenizationState::Approved => "APPROVED",
            TokenizationState::Active => "ACTIVE",
            TokenizationState::Paused => "PAUSED",
            TokenizationState::Retired => "RETIRED",
            TokenizationState::Rejected => "REJECTED",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KYC / Transfer Restriction tiers
// ─────────────────────────────────────────────────────────────────────────────

/// Know-Your-Customer / Anti-Money-Laundering status of a holder.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum KycStatus {
    /// No KYC submitted; may only view, not trade.
    None = 0,
    /// Basic identity verified (tier 1).
    Basic = 1,
    /// Enhanced due-diligence completed (tier 2).
    Enhanced = 2,
    /// Institutional / qualified investor clearance (tier 3).
    Institutional = 3,
    /// Suspended pending re-verification.
    Suspended = 4,
    /// Permanently revoked.
    Revoked = 5,
}

impl KycStatus {
    /// Whether this status permits participation in token transfers.
    pub fn allows_trading(&self) -> bool {
        matches!(
            self,
            KycStatus::Basic | KycStatus::Enhanced | KycStatus::Institutional
        )
    }

    /// Minimum required tier to hold a given minimum balance.
    pub fn meets_minimum_tier(&self, required: KycStatus) -> bool {
        (*self as u8) >= (required as u8)
            && !matches!(self, KycStatus::Suspended | KycStatus::Revoked)
    }
}

/// Transfer restriction mode applied to an entire token.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransferRestriction {
    /// No restrictions beyond state-machine check.
    None = 0,
    /// Only allowlisted holder pairs may transfer.
    AllowlistOnly = 1,
    /// Transfers blocked except owner-authorized mints/burns.
    Locked = 2,
    /// Jurisdiction-gated: both parties must share an approved jurisdiction.
    JurisdictionGated = 3,
    /// Minimum KYC tier required on both sides.
    KycRequired = 4,
}

impl TransferRestriction {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferRestriction::None => "NONE",
            TransferRestriction::AllowlistOnly => "ALLOWLIST",
            TransferRestriction::Locked => "LOCKED",
            TransferRestriction::JurisdictionGated => "JURISDICTION",
            TransferRestriction::KycRequired => "KYC_REQUIRED",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Core data structs
// ─────────────────────────────────────────────────────────────────────────────

/// Immutable token definition written at creation time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenDefinition {
    /// Content-addressed token ID (SHA-256 of name + issuer + creation_ts).
    pub token_id: BytesN<32>,
    /// Human-readable ticker / symbol (up to 12 bytes).
    pub ticker: Symbol,
    /// Full name stored as opaque bytes (e.g. UTF-8).
    pub name: Bytes,
    /// Asset class classification.
    pub asset_class: u8, // AssetClass as u8
    /// Off-chain legal documentation hash (IPFS CID or similar).
    pub legal_doc_hash: BytesN<32>,
    /// Maximum total supply (0 = uncapped).
    pub max_supply: u128,
    /// Decimal precision for amounts.
    pub decimals: u8,
    /// Issuing authority / token owner address.
    pub issuer: Address,
    /// Ledger timestamp at creation.
    pub created_at: u64,
    /// Transfer restriction mode.
    pub transfer_restriction: u8, // TransferRestriction as u8
    /// Minimum KYC tier required to hold this token.
    pub min_kyc_tier: u8, // KycStatus as u8
}

impl TokenDefinition {
    /// Returns true when new tokens may be minted given current supply.
    pub fn can_mint(&self, current_supply: u128, amount: u128) -> bool {
        if self.max_supply == 0 {
            return true; // uncapped
        }
        current_supply.checked_add(amount).map_or(false, |s| s <= self.max_supply)
    }
}

/// Mutable runtime state for a tokenized asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    /// Current lifecycle state.
    pub state: u8, // TokenizationState as u8
    /// Outstanding supply (sum of all holder balances).
    pub current_supply: u128,
    /// Cumulative amount ever minted.
    pub total_minted: u128,
    /// Cumulative amount ever burned.
    pub total_burned: u128,
    /// Total number of distinct holders.
    pub holder_count: u32,
    /// Ledger timestamp of last state change.
    pub last_transition_at: u64,
    /// Optional pause reason (compliance reference).
    pub pause_reason: Option<Bytes>,
}

impl TokenState {
    /// Construct initial state for a newly created token.
    pub fn initial(ts: u64) -> Self {
        TokenState {
            state: TokenizationState::Proposed as u8,
            current_supply: 0,
            total_minted: 0,
            total_burned: 0,
            holder_count: 0,
            last_transition_at: ts,
            pause_reason: None,
        }
    }

    /// Decode the embedded enum.
    pub fn lifecycle_state(&self) -> TokenizationState {
        match self.state {
            0 => TokenizationState::Proposed,
            1 => TokenizationState::Approved,
            2 => TokenizationState::Active,
            3 => TokenizationState::Paused,
            4 => TokenizationState::Retired,
            _ => TokenizationState::Rejected,
        }
    }
}

/// Per-holder registry entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HolderRecord {
    /// Holder address.
    pub holder: Address,
    /// Current token balance.
    pub balance: u128,
    /// Verified KYC status.
    pub kyc_status: u8, // KycStatus as u8
    /// ISO-3166-1 alpha-2 jurisdiction codes the holder is registered in.
    pub jurisdictions: Vec<Symbol>,
    /// Whether holder is allowlisted for restricted tokens.
    pub allowlisted: bool,
    /// Whether holder is blocked from all operations.
    pub blocked: bool,
    /// Ledger timestamp of last balance update.
    pub last_updated: u64,
    /// Optional compliance note (e.g. SAR reference).
    pub compliance_note: Option<Bytes>,
}

impl HolderRecord {
    /// Whether this holder may receive a transfer under `restriction`.
    pub fn can_receive(
        &self,
        restriction: TransferRestriction,
        sender_jurisdictions: &Vec<Symbol>,
    ) -> bool {
        if self.blocked {
            return false;
        }
        match restriction {
            TransferRestriction::None => true,
            TransferRestriction::AllowlistOnly => self.allowlisted,
            TransferRestriction::Locked => false,
            TransferRestriction::KycRequired => {
                let kyc = KycStatus::from_u8(self.kyc_status);
                kyc.allows_trading()
            }
            TransferRestriction::JurisdictionGated => {
                // At least one jurisdiction must overlap
                for j in self.jurisdictions.iter() {
                    for s in sender_jurisdictions.iter() {
                        if j == s {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }
}

impl KycStatus {
    /// Infallible decode from stored u8.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => KycStatus::None,
            1 => KycStatus::Basic,
            2 => KycStatus::Enhanced,
            3 => KycStatus::Institutional,
            4 => KycStatus::Suspended,
            _ => KycStatus::Revoked,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mint / Burn record
// ─────────────────────────────────────────────────────────────────────────────

/// Recorded details of a mint or burn operation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintBurnRecord {
    /// Unique operation ID.
    pub op_id: BytesN<32>,
    /// Token this operation belongs to.
    pub token_id: BytesN<32>,
    /// True = mint, false = burn.
    pub is_mint: bool,
    /// Amount minted or burned.
    pub amount: u128,
    /// Beneficiary (for mint) or redeemer (for burn).
    pub holder: Address,
    /// Authorizing party (must be issuer or delegate).
    pub authorized_by: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Supporting documentation hash (e.g., deed of transfer).
    pub doc_hash: Option<BytesN<32>>,
    /// Free-form compliance reference.
    pub reference: Bytes,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transfer record
// ─────────────────────────────────────────────────────────────────────────────

/// Validated transfer between two holders.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRecord {
    /// Unique transfer ID.
    pub transfer_id: BytesN<32>,
    /// Token transferred.
    pub token_id: BytesN<32>,
    /// Sending holder.
    pub from: Address,
    /// Receiving holder.
    pub to: Address,
    /// Amount transferred.
    pub amount: u128,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Result of restriction check (see `TransferValidation`).
    pub validation_result: u8,
    /// Optional trade reference (e.g., OTC settlement ID).
    pub trade_ref: Option<Bytes>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Transfer validation logic
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome codes for transfer validation.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransferValidation {
    /// Transfer is permitted.
    Allowed = 0,
    /// Token is not in Active state.
    TokenNotActive = 1,
    /// Sender has insufficient balance.
    InsufficientBalance = 2,
    /// Sender is blocked.
    SenderBlocked = 3,
    /// Receiver is blocked.
    ReceiverBlocked = 4,
    /// Receiver not on allowlist (AllowlistOnly mode).
    ReceiverNotAllowlisted = 5,
    /// Token is in Locked restriction mode.
    TransferLocked = 6,
    /// KYC check failed.
    KycFailed = 7,
    /// Jurisdiction mismatch.
    JurisdictionMismatch = 8,
    /// Amount is zero.
    ZeroAmount = 9,
    /// Sender KYC invalid.
    SenderKycFailed = 10,
}

impl TransferValidation {
    pub fn is_allowed(&self) -> bool {
        matches!(self, TransferValidation::Allowed)
    }

    /// Short error code for compliance events.
    pub fn error_code(&self) -> &'static str {
        match self {
            TransferValidation::Allowed => "OK",
            TransferValidation::TokenNotActive => "E_NOT_ACTIVE",
            TransferValidation::InsufficientBalance => "E_INSUF_BAL",
            TransferValidation::SenderBlocked => "E_SENDER_BLOCKED",
            TransferValidation::ReceiverBlocked => "E_RECV_BLOCKED",
            TransferValidation::ReceiverNotAllowlisted => "E_NOT_ALLOWLISTED",
            TransferValidation::TransferLocked => "E_LOCKED",
            TransferValidation::KycFailed => "E_KYC",
            TransferValidation::JurisdictionMismatch => "E_JURISDICTION",
            TransferValidation::ZeroAmount => "E_ZERO_AMT",
            TransferValidation::SenderKycFailed => "E_SENDER_KYC",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transfer Validator
// ─────────────────────────────────────────────────────────────────────────────

/// Stateless transfer validation engine.
pub struct TransferValidator;

impl TransferValidator {
    /// Validate a proposed transfer and return the outcome code.
    ///
    /// `sender_balance` — current on-chain balance of `from`.
    /// `sender_record`  — holder registry entry for `from`.
    /// `receiver_record`— holder registry entry for `to`.
    pub fn validate(
        amount: u128,
        token_state: &TokenState,
        token_def: &TokenDefinition,
        sender_balance: u128,
        sender_record: &HolderRecord,
        receiver_record: &HolderRecord,
    ) -> TransferValidation {
        if amount == 0 {
            return TransferValidation::ZeroAmount;
        }
        let lifecycle = token_state.lifecycle_state();
        if !lifecycle.allows_transfer() {
            return TransferValidation::TokenNotActive;
        }
        if sender_balance < amount {
            return TransferValidation::InsufficientBalance;
        }
        if sender_record.blocked {
            return TransferValidation::SenderBlocked;
        }
        if receiver_record.blocked {
            return TransferValidation::ReceiverBlocked;
        }

        let restriction = match token_def.transfer_restriction {
            1 => TransferRestriction::AllowlistOnly,
            2 => TransferRestriction::Locked,
            3 => TransferRestriction::JurisdictionGated,
            4 => TransferRestriction::KycRequired,
            _ => TransferRestriction::None,
        };

        match restriction {
            TransferRestriction::Locked => TransferValidation::TransferLocked,
            TransferRestriction::AllowlistOnly => {
                if !receiver_record.allowlisted {
                    TransferValidation::ReceiverNotAllowlisted
                } else {
                    TransferValidation::Allowed
                }
            }
            TransferRestriction::KycRequired => {
                let min_tier = token_def.min_kyc_tier;
                let sender_ok = KycStatus::from_u8(sender_record.kyc_status)
                    .meets_minimum_tier(KycStatus::from_u8(min_tier));
                let receiver_ok = KycStatus::from_u8(receiver_record.kyc_status)
                    .meets_minimum_tier(KycStatus::from_u8(min_tier));
                if !sender_ok {
                    TransferValidation::SenderKycFailed
                } else if !receiver_ok {
                    TransferValidation::KycFailed
                } else {
                    TransferValidation::Allowed
                }
            }
            TransferRestriction::JurisdictionGated => {
                if receiver_record
                    .can_receive(restriction, &sender_record.jurisdictions)
                {
                    TransferValidation::Allowed
                } else {
                    TransferValidation::JurisdictionMismatch
                }
            }
            TransferRestriction::None => TransferValidation::Allowed,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Token Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Global configuration for the RWA tokenization subsystem.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RwaConfig {
    /// Maximum number of concurrent active tokens.
    pub max_tokens: u32,
    /// Maximum holders per token.
    pub max_holders_per_token: u32,
    /// Maximum single mint/burn amount (0 = uncapped).
    pub max_single_op_amount: u128,
    /// Whether multi-token atomic swaps are enabled.
    pub atomic_swaps_enabled: bool,
    /// Minimum legal doc hash size (bytes).
    pub min_legal_doc_hash_len: u32,
}

impl RwaConfig {
    /// Sensible production defaults.
    pub fn default() -> Self {
        RwaConfig {
            max_tokens: 10_000,
            max_holders_per_token: 100_000,
            max_single_op_amount: 1_000_000_000_000, // 1 trillion base units
            atomic_swaps_enabled: false,
            min_legal_doc_hash_len: 32,
        }
    }

    pub fn can_add_token(&self, current_count: u32) -> bool {
        current_count < self.max_tokens
    }

    pub fn is_valid_op_amount(&self, amount: u128) -> bool {
        amount > 0
            && (self.max_single_op_amount == 0 || amount <= self.max_single_op_amount)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AssetClass ──────────────────────────────────────────────────────────

    #[test]
    fn test_asset_class_labels() {
        assert_eq!(AssetClass::RealEstate.label(), "Real Estate");
        assert_eq!(AssetClass::PrivateEquity.label(), "Private Equity");
        assert_eq!(AssetClass::Debt.label(), "Debt Instrument");
        assert_eq!(AssetClass::Commodity.label(), "Commodity");
        assert_eq!(AssetClass::AlternativeAsset.label(), "Alternative Asset");
        assert_eq!(AssetClass::Infrastructure.label(), "Infrastructure");
    }

    #[test]
    fn test_asset_class_fractional() {
        assert!(AssetClass::RealEstate.allows_fractional());
        assert!(AssetClass::Debt.allows_fractional());
        assert!(AssetClass::Infrastructure.allows_fractional());
        assert!(!AssetClass::PrivateEquity.allows_fractional());
        assert!(!AssetClass::AlternativeAsset.allows_fractional());
    }

    #[test]
    fn test_asset_class_decimals() {
        assert_eq!(AssetClass::RealEstate.default_decimals(), 6);
        assert_eq!(AssetClass::Commodity.default_decimals(), 8);
        assert_eq!(AssetClass::AlternativeAsset.default_decimals(), 0);
        assert_eq!(AssetClass::PrivateEquity.default_decimals(), 4);
    }

    // ── TokenizationState ───────────────────────────────────────────────────

    #[test]
    fn test_valid_state_transitions() {
        assert!(TokenizationState::Proposed.can_transition_to(TokenizationState::Approved));
        assert!(TokenizationState::Proposed.can_transition_to(TokenizationState::Rejected));
        assert!(TokenizationState::Approved.can_transition_to(TokenizationState::Active));
        assert!(TokenizationState::Active.can_transition_to(TokenizationState::Paused));
        assert!(TokenizationState::Paused.can_transition_to(TokenizationState::Active));
        assert!(TokenizationState::Active.can_transition_to(TokenizationState::Retired));
        assert!(TokenizationState::Paused.can_transition_to(TokenizationState::Retired));
    }

    #[test]
    fn test_invalid_state_transitions() {
        // Cannot skip states
        assert!(!TokenizationState::Proposed.can_transition_to(TokenizationState::Active));
        assert!(!TokenizationState::Approved.can_transition_to(TokenizationState::Retired));
        // Terminal states have no outbound transitions
        assert!(!TokenizationState::Retired.can_transition_to(TokenizationState::Active));
        assert!(!TokenizationState::Rejected.can_transition_to(TokenizationState::Approved));
    }

    #[test]
    fn test_state_terminal_flags() {
        assert!(TokenizationState::Retired.is_terminal());
        assert!(TokenizationState::Rejected.is_terminal());
        assert!(!TokenizationState::Active.is_terminal());
        assert!(!TokenizationState::Proposed.is_terminal());
        assert!(!TokenizationState::Paused.is_terminal());
    }

    #[test]
    fn test_state_allows_operations() {
        assert!(TokenizationState::Active.allows_mint_burn());
        assert!(TokenizationState::Active.allows_transfer());
        assert!(!TokenizationState::Paused.allows_mint_burn());
        assert!(!TokenizationState::Paused.allows_transfer());
        assert!(!TokenizationState::Proposed.allows_transfer());
    }

    #[test]
    fn test_state_str_labels() {
        assert_eq!(TokenizationState::Active.as_str(), "ACTIVE");
        assert_eq!(TokenizationState::Paused.as_str(), "PAUSED");
        assert_eq!(TokenizationState::Retired.as_str(), "RETIRED");
    }

    // ── KycStatus ───────────────────────────────────────────────────────────

    #[test]
    fn test_kyc_trading_permission() {
        assert!(!KycStatus::None.allows_trading());
        assert!(KycStatus::Basic.allows_trading());
        assert!(KycStatus::Enhanced.allows_trading());
        assert!(KycStatus::Institutional.allows_trading());
        assert!(!KycStatus::Suspended.allows_trading());
        assert!(!KycStatus::Revoked.allows_trading());
    }

    #[test]
    fn test_kyc_minimum_tier() {
        assert!(KycStatus::Enhanced.meets_minimum_tier(KycStatus::Basic));
        assert!(KycStatus::Institutional.meets_minimum_tier(KycStatus::Enhanced));
        assert!(!KycStatus::Basic.meets_minimum_tier(KycStatus::Enhanced));
        assert!(!KycStatus::Suspended.meets_minimum_tier(KycStatus::Basic));
        assert!(!KycStatus::Revoked.meets_minimum_tier(KycStatus::None));
    }

    #[test]
    fn test_kyc_from_u8_roundtrip() {
        for v in 0u8..=5 {
            let s = KycStatus::from_u8(v);
            assert_eq!(s as u8, v);
        }
        // Values beyond 5 map to Revoked
        assert_eq!(KycStatus::from_u8(99), KycStatus::Revoked);
    }

    // ── TokenDefinition / supply cap ────────────────────────────────────────

    #[test]
    fn test_token_definition_capped_supply() {
        use soroban_sdk::Env;
        let env = Env::default();
        let issuer = soroban_sdk::Address::generate(&env);

        let def = TokenDefinition {
            token_id: BytesN::from_array(&env, &[0u8; 32]),
            ticker: Symbol::new(&env, "RETKN"),
            name: Bytes::from_slice(&env, b"Real Estate Token"),
            asset_class: AssetClass::RealEstate as u8,
            legal_doc_hash: BytesN::from_array(&env, &[1u8; 32]),
            max_supply: 1_000_000,
            decimals: 6,
            issuer,
            created_at: 0,
            transfer_restriction: TransferRestriction::None as u8,
            min_kyc_tier: KycStatus::Basic as u8,
        };

        assert!(def.can_mint(0, 500_000));
        assert!(def.can_mint(500_000, 500_000));
        assert!(!def.can_mint(500_001, 500_000)); // would exceed cap
        assert!(!def.can_mint(1_000_000, 1));     // at cap already
    }

    #[test]
    fn test_token_definition_uncapped_supply() {
        use soroban_sdk::Env;
        let env = Env::default();
        let issuer = soroban_sdk::Address::generate(&env);

        let def = TokenDefinition {
            token_id: BytesN::from_array(&env, &[0u8; 32]),
            ticker: Symbol::new(&env, "UNLIM"),
            name: Bytes::from_slice(&env, b"Uncapped Token"),
            asset_class: AssetClass::Debt as u8,
            legal_doc_hash: BytesN::from_array(&env, &[2u8; 32]),
            max_supply: 0, // uncapped
            decimals: 6,
            issuer,
            created_at: 0,
            transfer_restriction: TransferRestriction::None as u8,
            min_kyc_tier: KycStatus::None as u8,
        };

        assert!(def.can_mint(u128::MAX - 1, 1));
    }

    // ── TokenState ──────────────────────────────────────────────────────────

    #[test]
    fn test_token_state_initial() {
        let s = TokenState::initial(1000);
        assert_eq!(s.lifecycle_state(), TokenizationState::Proposed);
        assert_eq!(s.current_supply, 0);
        assert_eq!(s.holder_count, 0);
        assert_eq!(s.last_transition_at, 1000);
        assert!(s.pause_reason.is_none());
    }

    #[test]
    fn test_token_state_decode_all_variants() {
        for v in 0u8..=5 {
            let s = TokenState {
                state: v,
                current_supply: 0,
                total_minted: 0,
                total_burned: 0,
                holder_count: 0,
                last_transition_at: 0,
                pause_reason: None,
            };
            // should not panic
            let _ = s.lifecycle_state();
        }
    }

    // ── TransferValidation ──────────────────────────────────────────────────

    #[test]
    fn test_transfer_validation_allowed_is_allowed() {
        assert!(TransferValidation::Allowed.is_allowed());
    }

    #[test]
    fn test_transfer_validation_error_codes() {
        assert_eq!(TransferValidation::Allowed.error_code(), "OK");
        assert_eq!(TransferValidation::ZeroAmount.error_code(), "E_ZERO_AMT");
        assert_eq!(
            TransferValidation::InsufficientBalance.error_code(),
            "E_INSUF_BAL"
        );
        assert_eq!(
            TransferValidation::TransferLocked.error_code(),
            "E_LOCKED"
        );
        assert_eq!(
            TransferValidation::JurisdictionMismatch.error_code(),
            "E_JURISDICTION"
        );
    }

    // ── TransferValidator ───────────────────────────────────────────────────

    fn make_active_state() -> TokenState {
        TokenState {
            state: TokenizationState::Active as u8,
            current_supply: 100_000,
            total_minted: 100_000,
            total_burned: 0,
            holder_count: 5,
            last_transition_at: 0,
            pause_reason: None,
        }
    }

    fn make_def_no_restriction(env: &soroban_sdk::Env) -> TokenDefinition {
        TokenDefinition {
            token_id: BytesN::from_array(env, &[0u8; 32]),
            ticker: Symbol::new(env, "TSTTKN"),
            name: Bytes::from_slice(env, b"Test Token"),
            asset_class: AssetClass::RealEstate as u8,
            legal_doc_hash: BytesN::from_array(env, &[0u8; 32]),
            max_supply: 1_000_000,
            decimals: 6,
            issuer: soroban_sdk::Address::generate(env),
            created_at: 0,
            transfer_restriction: TransferRestriction::None as u8,
            min_kyc_tier: KycStatus::Basic as u8,
        }
    }

    fn make_holder(env: &soroban_sdk::Env, kyc: KycStatus) -> HolderRecord {
        HolderRecord {
            holder: soroban_sdk::Address::generate(env),
            balance: 50_000,
            kyc_status: kyc as u8,
            jurisdictions: Vec::new(env),
            allowlisted: false,
            blocked: false,
            last_updated: 0,
            compliance_note: None,
        }
    }

    #[test]
    fn test_validate_happy_path() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let def = make_def_no_restriction(&env);
        let sender = make_holder(&env, KycStatus::Basic);
        let receiver = make_holder(&env, KycStatus::Basic);
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::Allowed);
    }

    #[test]
    fn test_validate_zero_amount() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let def = make_def_no_restriction(&env);
        let sender = make_holder(&env, KycStatus::Basic);
        let receiver = make_holder(&env, KycStatus::Basic);
        let result = TransferValidator::validate(0, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::ZeroAmount);
    }

    #[test]
    fn test_validate_token_paused() {
        let env = soroban_sdk::Env::default();
        let mut state = make_active_state();
        state.state = TokenizationState::Paused as u8;
        let def = make_def_no_restriction(&env);
        let sender = make_holder(&env, KycStatus::Basic);
        let receiver = make_holder(&env, KycStatus::Basic);
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::TokenNotActive);
    }

    #[test]
    fn test_validate_insufficient_balance() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let def = make_def_no_restriction(&env);
        let sender = make_holder(&env, KycStatus::Basic);
        let receiver = make_holder(&env, KycStatus::Basic);
        let result =
            TransferValidator::validate(100_001, &state, &def, 100_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::InsufficientBalance);
    }

    #[test]
    fn test_validate_sender_blocked() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let def = make_def_no_restriction(&env);
        let mut sender = make_holder(&env, KycStatus::Basic);
        sender.blocked = true;
        let receiver = make_holder(&env, KycStatus::Basic);
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::SenderBlocked);
    }

    #[test]
    fn test_validate_receiver_blocked() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let def = make_def_no_restriction(&env);
        let sender = make_holder(&env, KycStatus::Basic);
        let mut receiver = make_holder(&env, KycStatus::Basic);
        receiver.blocked = true;
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::ReceiverBlocked);
    }

    #[test]
    fn test_validate_allowlist_restriction_passes() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let mut def = make_def_no_restriction(&env);
        def.transfer_restriction = TransferRestriction::AllowlistOnly as u8;
        let sender = make_holder(&env, KycStatus::Enhanced);
        let mut receiver = make_holder(&env, KycStatus::Enhanced);
        receiver.allowlisted = true;
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::Allowed);
    }

    #[test]
    fn test_validate_allowlist_restriction_fails() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let mut def = make_def_no_restriction(&env);
        def.transfer_restriction = TransferRestriction::AllowlistOnly as u8;
        let sender = make_holder(&env, KycStatus::Enhanced);
        let receiver = make_holder(&env, KycStatus::Enhanced); // allowlisted=false
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::ReceiverNotAllowlisted);
    }

    #[test]
    fn test_validate_locked_restriction() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let mut def = make_def_no_restriction(&env);
        def.transfer_restriction = TransferRestriction::Locked as u8;
        let sender = make_holder(&env, KycStatus::Institutional);
        let receiver = make_holder(&env, KycStatus::Institutional);
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::TransferLocked);
    }

    #[test]
    fn test_validate_kyc_required_sender_fails() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let mut def = make_def_no_restriction(&env);
        def.transfer_restriction = TransferRestriction::KycRequired as u8;
        def.min_kyc_tier = KycStatus::Enhanced as u8;
        let sender = make_holder(&env, KycStatus::Basic); // below tier
        let receiver = make_holder(&env, KycStatus::Enhanced);
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::SenderKycFailed);
    }

    #[test]
    fn test_validate_kyc_required_receiver_fails() {
        let env = soroban_sdk::Env::default();
        let state = make_active_state();
        let mut def = make_def_no_restriction(&env);
        def.transfer_restriction = TransferRestriction::KycRequired as u8;
        def.min_kyc_tier = KycStatus::Enhanced as u8;
        let sender = make_holder(&env, KycStatus::Institutional);
        let receiver = make_holder(&env, KycStatus::Basic); // below tier
        let result =
            TransferValidator::validate(1_000, &state, &def, 50_000, &sender, &receiver);
        assert_eq!(result, TransferValidation::KycFailed);
    }

    // ── RwaConfig ───────────────────────────────────────────────────────────

    #[test]
    fn test_rwa_config_defaults() {
        let cfg = RwaConfig::default();
        assert_eq!(cfg.max_tokens, 10_000);
        assert_eq!(cfg.max_holders_per_token, 100_000);
        assert!(!cfg.atomic_swaps_enabled);
    }

    #[test]
    fn test_rwa_config_can_add_token() {
        let cfg = RwaConfig::default();
        assert!(cfg.can_add_token(9_999));
        assert!(!cfg.can_add_token(10_000));
    }

    #[test]
    fn test_rwa_config_valid_op_amount() {
        let cfg = RwaConfig::default();
        assert!(cfg.is_valid_op_amount(1));
        assert!(cfg.is_valid_op_amount(1_000_000_000_000));
        assert!(!cfg.is_valid_op_amount(0));
        assert!(!cfg.is_valid_op_amount(1_000_000_000_001)); // over cap
    }

    // ── TransferRestriction string labels ───────────────────────────────────

    #[test]
    fn test_transfer_restriction_labels() {
        assert_eq!(TransferRestriction::None.as_str(), "NONE");
        assert_eq!(TransferRestriction::AllowlistOnly.as_str(), "ALLOWLIST");
        assert_eq!(TransferRestriction::Locked.as_str(), "LOCKED");
        assert_eq!(
            TransferRestriction::JurisdictionGated.as_str(),
            "JURISDICTION"
        );
        assert_eq!(TransferRestriction::KycRequired.as_str(), "KYC_REQUIRED");
    }
}
