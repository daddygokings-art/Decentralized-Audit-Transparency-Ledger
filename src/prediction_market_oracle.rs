/// Prediction Market Oracle Integration
///
/// Provides outcome verification from multiple oracle sources:
/// - Compliance audit results
/// - Bridge latency measurements
/// - Event volume tracking
/// - Custom price feeds

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Oracle feed types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum OracleFeedType {
    /// Compliance audit result
    ComplianceAudit = 0,
    /// Bridge latency measurement
    BridgeLatency = 1,
    /// Event volume count
    EventVolume = 2,
    /// Custom external data feed
    CustomFeed = 3,
}

/// Price data from oracle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    /// Feed type
    pub feed_type: OracleFeedType,
    /// Actual value/price
    pub value: u128,
    /// Timestamp of data point
    pub timestamp: u64,
    /// Decimal places (e.g., 6 for USDC)
    pub decimals: u32,
    /// Which oracle provided this
    pub source: Address,
    /// Confidence/confidence interval
    pub confidence: u32,
}

/// Aggregated price from multiple sources
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregatedPrice {
    /// Feed identifier
    pub feed_id: Symbol,
    /// Median price from sources
    pub median_price: u128,
    /// Mean price from sources
    pub mean_price: u128,
    /// Number of sources
    pub source_count: u32,
    /// Latest update timestamp
    pub last_updated: u64,
    /// Price confidence
    pub confidence: u32,
    /// Status (valid, stale, invalid)
    pub status: u32,
}

/// Oracle provider configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleProvider {
    /// Oracle address
    pub address: Address,
    /// Feed types this oracle provides
    pub feed_types: Vec<OracleFeedType>,
    /// Reputation score (0-100)
    pub reputation: u32,
    /// Number of valid reports
    pub valid_reports: u32,
    /// Number of disputed reports
    pub disputed_reports: u32,
    /// Whether oracle is currently active
    pub active: bool,
}

/// Market outcome threshold
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutcomeThreshold {
    /// Market ID
    pub market_id: u64,
    /// Feed to check
    pub feed_id: Symbol,
    /// If price >= threshold: Yes, else: No
    pub threshold: u128,
    /// Verification deadline
    pub deadline: u32,
}

/// Oracle configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfig {
    /// Min oracles required for aggregate
    pub min_oracles: u32,
    /// Max price staleness in ledgers before invalid
    pub max_staleness_ledgers: u32,
    /// Price deviation tolerance in basis points
    pub deviation_tolerance_bps: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum OracleKey {
    /// Owner
    Owner,
    /// Configuration
    Config,
    /// Oracle providers: Address → OracleProvider
    OracleProvider(Address),
    /// Price data: (Symbol, Address) → PriceData
    PriceData(Symbol, Address),
    /// Aggregated prices: Symbol → AggregatedPrice
    AggregatedPrice(Symbol),
    /// Outcome thresholds: u64 (market_id) → OutcomeThreshold
    OutcomeThreshold(u64),
    /// Last price update: Symbol → u64 (timestamp)
    LastUpdate(Symbol),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    /// Caller not authorized
    Unauthorized = 1,
    /// Oracle provider not found
    ProviderNotFound = 2,
    /// Insufficient oracle reports
    InsufficientReports = 3,
    /// Price data stale
    PriceStale = 4,
    /// Price deviation too high
    PriceDeviation = 5,
    /// Invalid feed type
    InvalidFeedType = 6,
    /// Market outcome threshold not found
    ThresholdNotFound = 7,
    /// Deadline exceeded
    DeadlineExceeded = 8,
    /// Invalid price value
    InvalidPrice = 9,
    /// Oracle already registered
    ProviderAlreadyExists = 10,
    /// Aggregation failed
    AggregationFailed = 11,
    /// Feed not found
    FeedNotFound = 12,
}

// ============================================================================
// Oracle Contract
// ============================================================================

#[contract]
pub struct PredictionMarketOracle;

#[contractimpl]
impl PredictionMarketOracle {
    /// Initialize oracle (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        min_oracles: u32,
        max_staleness_ledgers: u32,
        deviation_tolerance_bps: u32,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&OracleKey::Owner) {
            panic_with_error!(&env, OracleError::Unauthorized);
        }

        let config = OracleConfig {
            min_oracles,
            max_staleness_ledgers,
            deviation_tolerance_bps,
        };

        env.storage().instance().set(&OracleKey::Owner, &owner);
        env.storage().instance().set(&OracleKey::Config, &config);

        log!(
            &env,
            "PredictionMarketOracle: initialized - min_oracles={}, max_staleness={}",
            min_oracles,
            max_staleness_ledgers
        );
    }

    // ========================================================================
    // Oracle Provider Management
    // ========================================================================

    /// Register an oracle provider
    pub fn register_provider(
        env: Env,
        provider_address: Address,
        feed_types: Vec<OracleFeedType>,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        if env
            .storage()
            .instance()
            .get::<_, Option<OracleProvider>>(&OracleKey::OracleProvider(provider_address.clone()))
            .is_some()
        {
            panic_with_error!(&env, OracleError::ProviderAlreadyExists);
        }

        let provider = OracleProvider {
            address: provider_address.clone(),
            feed_types,
            reputation: 100,
            valid_reports: 0,
            disputed_reports: 0,
            active: true,
        };

        env.storage()
            .instance()
            .set(&OracleKey::OracleProvider(provider_address.clone()), &provider);

        log!(
            &env,
            "PredictionMarketOracle: provider registered - address={}",
            provider_address
        );
    }

    /// Deactivate provider
    pub fn deactivate_provider(env: Env, provider_address: Address) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let mut provider = env
            .storage()
            .instance()
            .get::<_, OracleProvider>(&OracleKey::OracleProvider(provider_address.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, OracleError::ProviderNotFound));

        provider.active = false;

        env.storage()
            .instance()
            .set(&OracleKey::OracleProvider(provider_address.clone()), &provider);

        log!(
            &env,
            "PredictionMarketOracle: provider deactivated - address={}",
            provider_address
        );
    }

    // ========================================================================
    // Price Feed Submission
    // ========================================================================

    /// Submit price data to oracle (called by oracle providers)
    pub fn submit_price(
        env: Env,
        feed_id: Symbol,
        value: u128,
        decimals: u32,
        confidence: u32,
    ) {
        let provider = env.invoker();

        // Verify provider is registered and active
        let mut oracle_provider = env
            .storage()
            .instance()
            .get::<_, OracleProvider>(&OracleKey::OracleProvider(provider.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, OracleError::ProviderNotFound));

        if !oracle_provider.active {
            panic_with_error!(&env, OracleError::ProviderNotFound);
        }

        // Store price data
        let price_data = PriceData {
            feed_type: OracleFeedType::CustomFeed, // TODO: Determine from feed_id
            value,
            timestamp: env.ledger().timestamp(),
            decimals,
            source: provider.clone(),
            confidence,
        };

        env.storage()
            .instance()
            .set(&OracleKey::PriceData(feed_id.clone(), provider.clone()), &price_data);

        // Update last update timestamp
        env.storage()
            .instance()
            .set(&OracleKey::LastUpdate(feed_id.clone()), &env.ledger().timestamp());

        oracle_provider.valid_reports += 1;
        env.storage()
            .instance()
            .set(&OracleKey::OracleProvider(provider.clone()), &oracle_provider);

        log!(
            &env,
            "PredictionMarketOracle: price submitted - feed={}, value={}, provider={}",
            feed_id,
            value,
            provider
        );
    }

    // ========================================================================
    // Price Aggregation
    // ========================================================================

    /// Aggregate prices from multiple oracles (median)
    pub fn aggregate_prices(env: Env, feed_id: Symbol) -> AggregatedPrice {
        let config = Self::get_config(&env);

        // TODO: Collect prices from all providers for this feed
        // For now, return placeholder

        let aggregated = AggregatedPrice {
            feed_id,
            median_price: 0,
            mean_price: 0,
            source_count: 0,
            last_updated: env.ledger().timestamp(),
            confidence: 0,
            status: 0, // 0=invalid, 1=stale, 2=valid
        };

        env.storage()
            .instance()
            .set(&OracleKey::AggregatedPrice(feed_id.clone()), &aggregated);

        log!(
            &env,
            "PredictionMarketOracle: prices aggregated - feed={}, sources={}",
            feed_id,
            aggregated.source_count
        );

        aggregated
    }

    // ========================================================================
    // Market Resolution
    // ========================================================================

    /// Set outcome threshold for market
    pub fn set_outcome_threshold(
        env: Env,
        market_id: u64,
        feed_id: Symbol,
        threshold: u128,
        deadline: u32,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let threshold_config = OutcomeThreshold {
            market_id,
            feed_id,
            threshold,
            deadline,
        };

        env.storage()
            .instance()
            .set(&OracleKey::OutcomeThreshold(market_id), &threshold_config);

        log!(
            &env,
            "PredictionMarketOracle: outcome threshold set - market={}, threshold={}",
            market_id,
            threshold
        );
    }

    /// Determine market outcome from price feed
    pub fn get_market_outcome(env: Env, market_id: u64) -> u32 {
        let threshold_config = env
            .storage()
            .instance()
            .get::<_, OutcomeThreshold>(&OracleKey::OutcomeThreshold(market_id))
            .unwrap_or_else(|| panic_with_error!(&env, OracleError::ThresholdNotFound));

        let current_ledger = env.ledger().sequence();
        if current_ledger > threshold_config.deadline {
            panic_with_error!(&env, OracleError::DeadlineExceeded);
        }

        let aggregated = env
            .storage()
            .instance()
            .get::<_, AggregatedPrice>(&OracleKey::AggregatedPrice(threshold_config.feed_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, OracleError::FeedNotFound));

        // Return: 0 = Yes (above threshold), 1 = No (below threshold)
        if aggregated.median_price >= threshold_config.threshold {
            0 // Yes
        } else {
            1 // No
        }
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get latest aggregated price
    pub fn get_price(env: Env, feed_id: Symbol) -> Option<AggregatedPrice> {
        env.storage()
            .instance()
            .get(&OracleKey::AggregatedPrice(feed_id))
    }

    /// Get provider info
    pub fn get_provider(env: Env, provider_address: Address) -> Option<OracleProvider> {
        env.storage()
            .instance()
            .get(&OracleKey::OracleProvider(provider_address))
    }

    /// Get configuration
    pub fn get_oracle_config(env: Env) -> OracleConfig {
        Self::get_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&OracleKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, OracleError::Unauthorized))
    }

    fn get_config(env: &Env) -> OracleConfig {
        env.storage()
            .instance()
            .get(&OracleKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, OracleError::Unauthorized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_registration() {
        // Tests for oracle provider setup
    }

    #[test]
    fn test_price_submission() {
        // Tests for price feed submission
    }

    #[test]
    fn test_price_aggregation() {
        // Tests for aggregation logic
    }

    #[test]
    fn test_outcome_determination() {
        // Tests for threshold-based outcomes
    }
}
