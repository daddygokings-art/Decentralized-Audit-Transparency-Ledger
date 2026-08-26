//! DeFi Protocol Auditing Module
//!
//! Provides comprehensive auditing capabilities for DeFi protocols including:
//! - TVL (Total Value Locked) tracking across assets and pools
//! - Oracle price verification and anomaly detection
//! - Liquidation event monitoring and risk assessment
//! - Governance proposal and voting activity tracking
//! - Protocol risk metrics and health monitoring
//! - Automated audit report generation

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Error codes for DeFi auditing operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum DeFiAuditError {
    /// Protocol not found
    ProtocolNotFound = 1,
    /// Pool not found
    PoolNotFound = 2,
    /// Oracle price invalid or stale
    OraclePriceInvalid = 3,
    /// Price anomaly detected
    PriceAnomalyDetected = 4,
    /// Liquidation threshold exceeded
    LiquidationThresholdExceeded = 5,
    /// Governance proposal not found
    GovernanceProposalNotFound = 6,
    /// Invalid governance state
    InvalidGovernanceState = 7,
    /// Risk calculation error
    RiskCalculationError = 8,
    /// Report generation failed
    ReportGenerationFailed = 9,
    /// Insufficient data for analysis
    InsufficientData = 10,
    /// Invalid parameter
    InvalidParameter = 11,
}

/// DeFi protocol types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolType {
    /// Automated Market Maker (Uniswap, SushiSwap)
    AMM,
    /// Lending Protocol (Aave, Compound)
    Lending,
    /// Derivatives (dYdX, Perpetual Protocol)
    Derivatives,
    /// Staking Protocol (Lido, Rocket Pool)
    Staking,
    /// Liquidity Mining
    LiquidityMining,
    /// Other protocols
    Other,
}

/// TVL data for a pool
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolTVL {
    /// Pool ID
    pub pool_id: BytesN<32>,
    /// Protocol address
    pub protocol: Address,
    /// Pool name/identifier
    pub pool_name: Symbol,
    /// Total value locked in USD
    pub tvl_usd: u128,
    /// Total value locked in native units
    pub tvl_native: u128,
    /// Number of liquidity providers
    pub lp_count: u32,
    /// Last update timestamp
    pub updated_at: u64,
}

/// Asset in a pool
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolAsset {
    /// Asset address
    pub asset: Address,
    /// Amount locked
    pub amount: u128,
    /// USD value
    pub usd_value: u128,
    /// Percentage of pool
    pub percentage: u32,
}

/// Oracle price record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OraclePrice {
    /// Oracle ID
    pub oracle_id: BytesN<32>,
    /// Asset being priced
    pub asset: Address,
    /// Price in USD (scaled)
    pub price_usd: u128,
    /// Timestamp of price
    pub timestamp: u64,
    /// Oracle source (Chainlink, Band, Pyth, etc.)
    pub source: Symbol,
    /// Confidence interval (basis points)
    pub confidence_bp: u32,
    /// Price update frequency in seconds
    pub update_frequency: u64,
}

/// Price history for anomaly detection
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceHistory {
    /// Asset address
    pub asset: Address,
    /// Previous price
    pub prev_price: u128,
    /// Current price
    pub current_price: u128,
    /// Price change percentage (basis points)
    pub change_bp: i64,
    /// Is anomaly detected
    pub is_anomaly: bool,
    /// Timestamp
    pub timestamp: u64,
}

/// Liquidation event
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidationEvent {
    /// Event ID
    pub event_id: BytesN<32>,
    /// Protocol address
    pub protocol: Address,
    /// Liquidated position address
    pub position: Address,
    /// Liquidator address
    pub liquidator: Address,
    /// Collateral asset
    pub collateral_asset: Address,
    /// Debt asset
    pub debt_asset: Address,
    /// Collateral amount liquidated
    pub collateral_amount: u128,
    /// Debt repaid
    pub debt_amount: u128,
    /// Liquidation price
    pub liquidation_price: u128,
    /// Timestamp
    pub timestamp: u64,
}

/// At-risk position
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtRiskPosition {
    /// Position ID
    pub position_id: BytesN<32>,
    /// Position owner
    pub owner: Address,
    /// Protocol address
    pub protocol: Address,
    /// Collateral value USD
    pub collateral_value: u128,
    /// Debt value USD
    pub debt_value: u128,
    /// Health factor (scaled)
    pub health_factor: u128,
    /// Liquidation risk percentage (0-100)
    pub liquidation_risk_percent: u32,
    /// Timestamp
    pub timestamp: u64,
}

/// Governance proposal
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceProposal {
    /// Proposal ID
    pub proposal_id: BytesN<32>,
    /// Protocol address
    pub protocol: Address,
    /// Proposal title
    pub title: Bytes,
    /// Proposal description
    pub description: Bytes,
    /// Proposer address
    pub proposer: Address,
    /// Status: 0=pending, 1=active, 2=passed, 3=failed, 4=executed
    pub status: u32,
    /// For votes
    pub votes_for: u128,
    /// Against votes
    pub votes_against: u128,
    /// Abstain votes
    pub votes_abstain: u128,
    /// Start time
    pub start_time: u64,
    /// End time
    pub end_time: u64,
    /// Execution time (if executed)
    pub execution_time: u64,
}

/// Voting record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VotingRecord {
    /// Vote ID
    pub vote_id: BytesN<32>,
    /// Proposal ID
    pub proposal_id: BytesN<32>,
    /// Voter address
    pub voter: Address,
    /// Vote direction: 0=against, 1=for, 2=abstain
    pub vote_direction: u32,
    /// Voting power
    pub voting_power: u128,
    /// Timestamp
    pub timestamp: u64,
}

/// Risk metrics for a protocol
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskMetrics {
    /// Metrics ID
    pub metrics_id: BytesN<32>,
    /// Protocol address
    pub protocol: Address,
    /// TVL in USD
    pub tvl_usd: u128,
    /// Concentration risk: percentage in top 3 assets (0-100)
    pub concentration_risk: u32,
    /// Average health factor (scaled)
    pub avg_health_factor: u128,
    /// Liquidation risk: percentage of positions at risk (0-100)
    pub liquidation_risk: u32,
    /// Price volatility: annualized volatility in basis points
    pub price_volatility: u32,
    /// Protocol health score (0-100)
    pub protocol_health: u32,
    /// Last update timestamp
    pub updated_at: u64,
}

/// Audit report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditReport {
    /// Report ID
    pub report_id: BytesN<32>,
    /// Protocol address
    pub protocol: Address,
    /// Report period start
    pub period_start: u64,
    /// Report period end
    pub period_end: u64,
    /// Total TVL average
    pub avg_tvl: u128,
    /// Peak TVL
    pub peak_tvl: u128,
    /// Minimum TVL
    pub min_tvl: u128,
    /// Total liquidations
    pub total_liquidations: u32,
    /// Total liquidation value
    pub liquidation_value: u128,
    /// Governance proposals in period
    pub proposals_count: u32,
    /// Average participation rate (basis points)
    pub avg_participation: u32,
    /// Protocol health score (0-100)
    pub health_score: u32,
    /// Key findings (hash)
    pub findings_hash: BytesN<32>,
    /// Generated timestamp
    pub generated_at: u64,
}

/// Protocol registry entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolRegistry {
    /// Protocol address
    pub protocol: Address,
    /// Protocol name
    pub name: Symbol,
    /// Protocol type
    pub protocol_type: ProtocolType,
    /// Chain/network
    pub chain: Symbol,
    /// Governance token
    pub gov_token: Option<Address>,
    /// Registration timestamp
    pub registered_at: u64,
}

/// Storage key enumeration for DeFi auditing
#[derive(Clone)]
#[contracttype]
pub enum DeFiAuditDataKey {
    /// Protocol ID → ProtocolRegistry
    Protocol(Address),
    /// Pool ID → PoolTVL
    PoolTVL(BytesN<32>),
    /// Oracle ID → OraclePrice
    OraclePrice(BytesN<32>),
    /// Asset → PriceHistory
    PriceHistory(Address),
    /// Liquidation ID → LiquidationEvent
    Liquidation(BytesN<32>),
    /// Position ID → AtRiskPosition
    AtRiskPosition(BytesN<32>),
    /// Proposal ID → GovernanceProposal
    GovernanceProposal(BytesN<32>),
    /// Vote ID → VotingRecord
    VotingRecord(BytesN<32>),
    /// Metrics ID → RiskMetrics
    RiskMetrics(BytesN<32>),
    /// Report ID → AuditReport
    AuditReport(BytesN<32>),
    /// List of all protocol addresses
    ProtocolList,
    /// List of all pool IDs for a protocol
    ProtocolPoolList(Address),
    /// List of all liquidation events for a protocol
    LiquidationList(Address),
    /// List of all at-risk positions for a protocol
    AtRiskPositionList(Address),
    /// List of all proposals for a protocol
    ProposalList(Address),
    /// List of all reports for a protocol
    ReportList(Address),
    /// Protocol counter
    ProtocolCount,
    /// Pool counter for protocol
    PoolCount(Address),
    /// Liquidation counter for protocol
    LiquidationCount(Address),
    /// At-risk position counter for protocol
    AtRiskPositionCount(Address),
    /// Proposal counter for protocol
    ProposalCount(Address),
    /// Report counter for protocol
    ReportCount(Address),
    /// Total protocol TVL
    TotalTVL,
    /// Last audit timestamp for protocol
    LastAuditTime(Address),
    /// TVL history for protocol
    TVLHistory(Address),
    /// Current TVL for protocol
    CurrentTVL(Address),
}

/// DeFi auditing contract trait
pub trait DeFiAuditingTrait {
    // ==================== PROTOCOL MANAGEMENT ====================
    fn register_protocol(
        env: Env,
        protocol: Address,
        name: Symbol,
        protocol_type: ProtocolType,
        chain: Symbol,
        gov_token: Option<Address>,
    ) -> Result<(), DeFiAuditError>;

    fn get_protocol(env: Env, protocol: Address) -> Result<ProtocolRegistry, DeFiAuditError>;

    // ==================== TVL TRACKING ====================
    fn update_pool_tvl(
        env: Env,
        pool_id: BytesN<32>,
        protocol: Address,
        pool_name: Symbol,
        tvl_usd: u128,
        tvl_native: u128,
        lp_count: u32,
    ) -> Result<(), DeFiAuditError>;

    fn get_pool_tvl(env: Env, pool_id: BytesN<32>) -> Result<PoolTVL, DeFiAuditError>;

    fn get_protocol_tvl(env: Env, protocol: Address) -> Result<u128, DeFiAuditError>;

    // ==================== ORACLE VERIFICATION ====================
    fn record_oracle_price(
        env: Env,
        oracle_id: BytesN<32>,
        asset: Address,
        price_usd: u128,
        source: Symbol,
        confidence_bp: u32,
        update_frequency: u64,
    ) -> Result<(), DeFiAuditError>;

    fn verify_price_anomaly(
        env: Env,
        asset: Address,
        current_price: u128,
        anomaly_threshold_bp: u32,
    ) -> Result<bool, DeFiAuditError>;

    fn get_oracle_price(env: Env, oracle_id: BytesN<32>) -> Result<OraclePrice, DeFiAuditError>;

    // ==================== LIQUIDATION MONITORING ====================
    fn record_liquidation(
        env: Env,
        protocol: Address,
        position: Address,
        liquidator: Address,
        collateral_asset: Address,
        debt_asset: Address,
        collateral_amount: u128,
        debt_amount: u128,
        liquidation_price: u128,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn add_at_risk_position(
        env: Env,
        protocol: Address,
        owner: Address,
        collateral_value: u128,
        debt_value: u128,
        health_factor: u128,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn get_at_risk_position(
        env: Env,
        position_id: BytesN<32>,
    ) -> Result<AtRiskPosition, DeFiAuditError>;

    fn get_protocol_liquidations(env: Env, protocol: Address) -> Result<u32, DeFiAuditError>;

    fn get_at_risk_positions(env: Env, protocol: Address) -> Result<u32, DeFiAuditError>;

    // ==================== GOVERNANCE TRACKING ====================
    fn create_proposal(
        env: Env,
        protocol: Address,
        title: Bytes,
        description: Bytes,
        proposer: Address,
        start_time: u64,
        end_time: u64,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn record_vote(
        env: Env,
        proposal_id: BytesN<32>,
        voter: Address,
        vote_direction: u32,
        voting_power: u128,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn update_proposal_status(
        env: Env,
        proposal_id: BytesN<32>,
        status: u32,
    ) -> Result<(), DeFiAuditError>;

    fn get_proposal(env: Env, proposal_id: BytesN<32>) -> Result<GovernanceProposal, DeFiAuditError>;

    fn get_proposal_votes(env: Env, proposal_id: BytesN<32>) -> Result<(u128, u128, u128), DeFiAuditError>;

    // ==================== RISK METRICS ====================
    fn calculate_risk_metrics(
        env: Env,
        protocol: Address,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn get_risk_metrics(env: Env, metrics_id: BytesN<32>) -> Result<RiskMetrics, DeFiAuditError>;

    fn get_protocol_health_score(env: Env, protocol: Address) -> Result<u32, DeFiAuditError>;

    // ==================== AUDIT REPORTS ====================
    fn generate_audit_report(
        env: Env,
        protocol: Address,
        period_start: u64,
        period_end: u64,
        findings_hash: BytesN<32>,
    ) -> Result<BytesN<32>, DeFiAuditError>;

    fn get_audit_report(env: Env, report_id: BytesN<32>) -> Result<AuditReport, DeFiAuditError>;

    fn get_latest_audit_report(env: Env, protocol: Address) -> Result<AuditReport, DeFiAuditError>;

    // ==================== QUERY FUNCTIONS ====================
    fn total_protocol_count(env: Env) -> u32;

    fn protocol_tvl(env: Env, protocol: Address) -> Result<u128, DeFiAuditError>;

    fn protocol_pool_count(env: Env, protocol: Address) -> u32;

    fn protocol_liquidation_count(env: Env, protocol: Address) -> u32;

    fn protocol_at_risk_count(env: Env, protocol: Address) -> u32;

    fn protocol_proposal_count(env: Env, protocol: Address) -> u32;

    fn protocol_report_count(env: Env, protocol: Address) -> u32;
}

/// Helper to compute hash from data
pub fn compute_hash(data: &Bytes) -> BytesN<32> {
    let env = Env::new();
    env.crypto().sha256(data)
}

/// Helper to calculate health factor from collateral and debt
pub fn calculate_health_factor(collateral: u128, debt: u128, liquidation_threshold: u128) -> u128 {
    if debt == 0 {
        return u128::MAX;
    }
    (collateral * liquidation_threshold) / debt
}

/// Helper to detect price anomaly
pub fn is_price_anomaly(prev_price: u128, current_price: u128, threshold_bp: u32) -> bool {
    if prev_price == 0 {
        return false;
    }
    
    let max_allowed = prev_price.saturating_mul(10000u128 + threshold_bp as u128) / 10000u128;
    let min_allowed = prev_price.saturating_mul(10000u128).saturating_sub(prev_price.saturating_mul(threshold_bp as u128)) / 10000u128;
    
    current_price > max_allowed || current_price < min_allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_factor_calculation() {
        let health = calculate_health_factor(1000u128, 500u128, 8000u128);
        assert!(health > 0);
    }

    #[test]
    fn test_price_anomaly_detection() {
        let is_anomaly = is_price_anomaly(100u128, 150u128, 400u32); // 50% change, 4% threshold
        assert!(is_anomaly);
    }
}
