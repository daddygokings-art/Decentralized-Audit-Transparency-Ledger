//! Tokenized Asset Lifecycle Management
//!
//! Comprehensive system for managing tokenized assets including issuance,
//! compliance, corporate actions, secondary trading, redemption, and maturity.

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Error codes for tokenized asset operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AssetLifecycleError {
    /// Asset not found
    AssetNotFound = 1,
    /// Compliance check failed
    ComplianceFailed = 2,
    /// Insufficient balance
    InsufficientBalance = 3,
    /// Invalid transfer
    InvalidTransfer = 4,
    /// Corporate action failed
    CorporateActionFailed = 5,
    /// Trading not allowed
    TradingNotAllowed = 6,
    /// Asset not matured
    AssetNotMatured = 7,
    /// Redemption failed
    RedemptionFailed = 8,
    /// Investor not verified
    InvestorNotVerified = 9,
    /// Invalid compliance rule
    InvalidComplianceRule = 10,
}

/// Asset status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetStatus {
    Draft,
    Issued,
    Trading,
    Restricted,
    Matured,
    Redeemed,
}

/// Compliance rule type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComplianceRuleType {
    Accredited,
    KYC,
    Whitelisted,
    TransferRestriction,
    HoldingPeriod,
}

/// Tokenized asset metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizedAsset {
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Asset name
    pub name: Bytes,
    /// Asset symbol
    pub symbol: Bytes,
    /// Issuer address
    pub issuer: Address,
    /// Total supply
    pub total_supply: u128,
    /// Decimals (precision)
    pub decimals: u32,
    /// Asset status
    pub status: AssetStatus,
    /// Maturity date (Unix timestamp)
    pub maturity_date: u64,
    /// Coupon rate (basis points)
    pub coupon_rate_bp: u32,
    /// Par value
    pub par_value: u128,
    /// Issuance date
    pub issued_at: u64,
    /// Legal document hash
    pub legal_document_hash: BytesN<32>,
}

/// Investor profile with compliance data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvestorProfile {
    /// Investor address
    pub investor: Address,
    /// Investor name
    pub name: Bytes,
    /// KYC verified
    pub kyc_verified: bool,
    /// Accredited investor
    pub accredited: bool,
    /// Whitelisted chains (comma-separated)
    pub whitelisted_chains: Bytes,
    /// Verification timestamp
    pub verified_at: u64,
    /// Portfolio value
    pub portfolio_value: u128,
    /// Risk rating (1-10)
    pub risk_rating: u32,
}

/// Holding record for investor balance
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
    /// Holding ID
    pub holding_id: BytesN<32>,
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Investor address
    pub investor: Address,
    /// Quantity held
    pub quantity: u128,
    /// Acquisition price
    pub acquisition_price: u128,
    /// Acquisition date
    pub acquired_at: u64,
    /// Locked until date (for transfer restrictions)
    pub locked_until: u64,
    /// Accrued interest/coupon
    pub accrued_coupon: u128,
}

/// Corporate action (dividend, split, redemption)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorporateAction {
    /// Action ID
    pub action_id: BytesN<32>,
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Action type: 0=dividend, 1=split, 2=bonus, 3=redemption
    pub action_type: u32,
    /// Effective date
    pub effective_date: u64,
    /// Dividend amount per unit
    pub dividend_amount: u128,
    /// Stock split ratio (numerator/denominator)
    pub split_numerator: u32,
    pub split_denominator: u32,
    /// Record date (for dividend eligibility)
    pub record_date: u64,
    /// Payment date
    pub payment_date: u64,
    /// Status: 0=announced, 1=effective, 2=paid, 3=completed
    pub status: u32,
}

/// Secondary market trade
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trade {
    /// Trade ID
    pub trade_id: BytesN<32>,
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Seller
    pub seller: Address,
    /// Buyer
    pub buyer: Address,
    /// Quantity
    pub quantity: u128,
    /// Price per unit
    pub price: u128,
    /// Total consideration
    pub total: u128,
    /// Trade date
    pub trade_date: u64,
    /// Settlement date
    pub settlement_date: u64,
    /// Status: 0=pending, 1=settled, 2=failed
    pub status: u32,
    /// Compliance check passed
    pub compliance_passed: bool,
}

/// Redemption record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedemptionRecord {
    /// Redemption ID
    pub redemption_id: BytesN<32>,
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Investor
    pub investor: Address,
    /// Quantity redeemed
    pub quantity: u128,
    /// Redemption price
    pub redemption_price: u128,
    /// Total amount
    pub total_amount: u128,
    /// Redemption date
    pub redemption_date: u64,
    /// Settlement date
    pub settlement_date: u64,
    /// Status: 0=requested, 1=approved, 2=paid, 3=rejected
    pub status: u32,
}

/// Compliance rule
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceRule {
    /// Rule ID
    pub rule_id: BytesN<32>,
    /// Asset ID
    pub asset_id: BytesN<32>,
    /// Rule type
    pub rule_type: ComplianceRuleType,
    /// Minimum holding period (days)
    pub min_holding_period: u32,
    /// Maximum ownership percentage
    pub max_ownership_pct: u32,
    /// Requires whitelist
    pub requires_whitelist: bool,
    /// Requires accreditation
    pub requires_accreditation: bool,
    /// Enabled
    pub enabled: bool,
}

/// Storage key enumeration
#[derive(Clone)]
#[contracttype]
pub enum AssetLifecycleDataKey {
    /// Asset ID → TokenizedAsset
    Asset(BytesN<32>),
    /// Investor → InvestorProfile
    InvestorProfile(Address),
    /// Holding ID → Holding
    Holding(BytesN<32>),
    /// Asset+Investor → Holding list
    InvestorAssetHoldings(Address, BytesN<32>),
    /// Corporate action ID → CorporateAction
    CorporateAction(BytesN<32>),
    /// Trade ID → Trade
    Trade(BytesN<32>),
    /// Redemption ID → RedemptionRecord
    Redemption(BytesN<32>),
    /// Compliance rule ID → ComplianceRule
    ComplianceRule(BytesN<32>),
    /// Asset → List of compliance rules
    AssetComplianceRules(BytesN<32>),
    /// Asset → List of trades
    AssetTradeList(BytesN<32>),
    /// Asset → List of corporate actions
    AssetCorporateActions(BytesN<32>),
    /// Asset → List of redemptions
    AssetRedemptions(BytesN<32>),
    /// Asset counter
    AssetCount,
    /// Investor counter
    InvestorCount,
    /// Trade counter
    TradeCount,
    /// Corporate action counter
    CorporateActionCount,
    /// Redemption counter
    RedemptionCount,
    /// Total issued tokens per asset
    IssuedSupply(BytesN<32>),
    /// Asset → List of active investors
    AssetInvestorList(BytesN<32>),
}

/// Asset lifecycle management trait
pub trait AssetLifecycleTrait {
    // ==================== ISSUANCE ====================
    fn issue_asset(
        env: Env,
        issuer: Address,
        name: Bytes,
        symbol: Bytes,
        total_supply: u128,
        decimals: u32,
        maturity_date: u64,
        coupon_rate_bp: u32,
        par_value: u128,
        legal_document_hash: BytesN<32>,
    ) -> Result<BytesN<32>, AssetLifecycleError>;

    fn get_asset(env: Env, asset_id: BytesN<32>) -> Result<TokenizedAsset, AssetLifecycleError>;

    fn update_asset_status(
        env: Env,
        asset_id: BytesN<32>,
        status: AssetStatus,
    ) -> Result<(), AssetLifecycleError>;

    // ==================== COMPLIANCE ====================
    fn register_investor(
        env: Env,
        investor: Address,
        name: Bytes,
    ) -> Result<(), AssetLifecycleError>;

    fn verify_investor_kyc(
        env: Env,
        investor: Address,
    ) -> Result<(), AssetLifecycleError>;

    fn set_accredited_status(
        env: Env,
        investor: Address,
        accredited: bool,
    ) -> Result<(), AssetLifecycleError>;

    fn get_investor(
        env: Env,
        investor: Address,
    ) -> Result<InvestorProfile, AssetLifecycleError>;

    fn add_compliance_rule(
        env: Env,
        asset_id: BytesN<32>,
        rule_type: ComplianceRuleType,
        min_holding_period: u32,
        max_ownership_pct: u32,
        requires_whitelist: bool,
        requires_accreditation: bool,
    ) -> Result<BytesN<32>, AssetLifecycleError>;

    fn check_compliance(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
        quantity: u128,
    ) -> Result<bool, AssetLifecycleError>;

    // ==================== TRADING ====================
    fn transfer_tokens(
        env: Env,
        asset_id: BytesN<32>,
        from: Address,
        to: Address,
        quantity: u128,
        price: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError>;

    fn get_trade(env: Env, trade_id: BytesN<32>) -> Result<Trade, AssetLifecycleError>;

    fn settle_trade(
        env: Env,
        trade_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError>;

    // ==================== HOLDINGS ====================
    fn get_holding(
        env: Env,
        holding_id: BytesN<32>,
    ) -> Result<Holding, AssetLifecycleError>;

    fn get_investor_balance(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
    ) -> Result<u128, AssetLifecycleError>;

    // ==================== CORPORATE ACTIONS ====================
    fn declare_corporate_action(
        env: Env,
        asset_id: BytesN<32>,
        action_type: u32,
        effective_date: u64,
        record_date: u64,
        payment_date: u64,
        dividend_amount: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError>;

    fn execute_corporate_action(
        env: Env,
        action_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError>;

    fn get_corporate_action(
        env: Env,
        action_id: BytesN<32>,
    ) -> Result<CorporateAction, AssetLifecycleError>;

    // ==================== REDEMPTION ====================
    fn request_redemption(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
        quantity: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError>;

    fn approve_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError>;

    fn process_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError>;

    fn get_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<RedemptionRecord, AssetLifecycleError>;

    // ==================== QUERIES ====================
    fn total_asset_count(env: Env) -> u32;

    fn total_investor_count(env: Env) -> u32;

    fn total_trades(env: Env) -> u32;

    fn total_corporate_actions(env: Env) -> u32;

    fn asset_issued_supply(env: Env, asset_id: BytesN<32>) -> u128;

    fn asset_remaining_supply(env: Env, asset_id: BytesN<32>) -> u128;
}

/// Helper to compute asset ID
pub fn compute_asset_id(env: &Env, issuer: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let issuer_bytes = issuer.to_string().as_bytes();
    let mut data = [0u8; 40];
    if issuer_bytes.len() <= 32 {
        data[0..issuer_bytes.len()].copy_from_slice(issuer_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_id_generation() {
        let env = Env::new();
        let issuer = Address::generate(&env);
        let id1 = compute_asset_id(&env, &issuer);
        assert_ne!(id1.to_array(), [0u8; 32]);
    }
}
