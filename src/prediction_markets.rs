/// Prediction Markets Contract - Binary Event Outcome Betting
///
/// Supports markets for:
/// - Compliance audit pass/fail
/// - Bridge latency thresholds
/// - Event volume targets
/// - Any binary outcome event
///
/// Architecture:
/// 1. Market Creation: Define event, outcomes, deadlines
/// 2. Trading: AMM for liquidity, order book for price discovery
/// 3. Resolution: Oracle-based outcome verification
/// 4. Settlement: Payout winners, burn losers' tokens

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Market types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum MarketType {
    /// Compliance audit: Pass/Fail
    ComplianceAudit = 0,
    /// Bridge latency: Below/Above threshold
    BridgeLatency = 1,
    /// Event volume: Above/Below target
    EventVolume = 2,
    /// Custom binary outcome
    Custom = 3,
}

/// Market status lifecycle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum MarketStatus {
    /// Active trading period
    Active = 0,
    /// Trading ended, awaiting resolution
    Closed = 1,
    /// Waiting for oracle outcome
    Pending = 2,
    /// Outcome determined, settlement in progress
    Resolved = 3,
    /// Market settled, positions closed
    Settled = 4,
    /// Cancelled (refund all participants)
    Cancelled = 5,
}

/// Outcome choice (binary)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum Outcome {
    /// First outcome (e.g., Pass, Below, Above)
    Yes = 0,
    /// Second outcome (e.g., Fail, Above, Below)
    No = 1,
    /// Invalid/Ambiguous outcome (all refunded)
    Invalid = 2,
}

/// A prediction market
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Market {
    /// Unique market ID
    pub market_id: u64,
    /// Market type
    pub market_type: MarketType,
    /// Title/description
    pub title: String,
    /// Detailed description (IPFS hash or URI)
    pub description: String,
    /// Yes outcome label
    pub outcome_yes_label: String,
    /// No outcome label
    pub outcome_no_label: String,
    /// Current market status
    pub status: MarketStatus,
    /// Trading deadline (ledger)
    pub trading_deadline: u32,
    /// Resolution deadline (ledger)
    pub resolution_deadline: u32,
    /// Final resolved outcome
    pub resolved_outcome: Option<Outcome>,
    /// Total liquidity supplied
    pub total_liquidity: u128,
    /// Token supply for Yes shares
    pub yes_shares_outstanding: u128,
    /// Token supply for No shares
    pub no_shares_outstanding: u128,
    /// Current Yes share price (0-10000, basis points)
    pub yes_price: u32,
    /// Current No share price (0-10000, basis points)
    pub no_price: u32,
    /// Oracle providing resolution
    pub oracle_address: Address,
    /// When market was created
    pub created_ledger: u32,
    /// Settlement fee in basis points (e.g., 500 = 5%)
    pub settlement_fee_bps: u32,
}

/// User's position in a market
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    /// User address
    pub user: Address,
    /// Market ID
    pub market_id: u64,
    /// Yes shares held
    pub yes_shares: u128,
    /// No shares held
    pub no_shares: u128,
    /// Total cost basis
    pub cost_basis: u128,
    /// Whether position has been settled
    pub settled: bool,
}

/// Order in order book
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    /// Unique order ID
    pub order_id: u64,
    /// Market ID
    pub market_id: u64,
    /// Order creator
    pub creator: Address,
    /// Buy or Sell
    pub is_buy: bool,
    /// Whether order is for Yes or No outcome
    pub is_yes: bool,
    /// Price in basis points (0-10000)
    pub price: u32,
    /// Quantity of shares
    pub quantity: u128,
    /// Filled quantity
    pub filled: u128,
    /// Status (active, cancelled, filled)
    pub active: bool,
}

/// Trade execution record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trade {
    /// Trade ID
    pub trade_id: u64,
    /// Market ID
    pub market_id: u64,
    /// Buyer address
    pub buyer: Address,
    /// Seller address
    pub seller: Address,
    /// Price per share (basis points)
    pub price: u32,
    /// Quantity traded
    pub quantity: u128,
    /// Whether trading Yes or No
    pub is_yes: bool,
    /// Trade timestamp
    pub timestamp: u64,
}

/// Market configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketConfig {
    /// Base token address (for liquidity)
    pub base_token: Address,
    /// Minimum liquidity to create market
    pub min_liquidity: u128,
    /// Maximum outstanding shares per market
    pub max_shares: u128,
    /// Default settlement fee
    pub default_settlement_fee_bps: u32,
    /// Oracle minimum threshold
    pub oracle_min_threshold: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum MarketKey {
    /// Owner/admin
    Owner,
    /// Market configuration
    Config,
    /// Market details: u64 (market_id) → Market
    Market(u64),
    /// Next market ID counter
    MarketCounter,
    /// User positions: (Address, u64) → Position
    Position(Address, u64),
    /// Order book: u64 (order_id) → Order
    Order(u64),
    /// Next order ID counter
    OrderCounter,
    /// Trade history: u64 (trade_id) → Trade
    Trade(u64),
    /// Next trade ID counter
    TradeCounter,
    /// Active orders for market: u64 (market_id) → Vec<u64> (order_ids)
    MarketOrders(u64),
    /// User's markets: Address → Vec<u64> (market_ids)
    UserMarkets(Address),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Market not found
    MarketNotFound = 2,
    /// Market status invalid for operation
    InvalidStatus = 3,
    /// Insufficient liquidity
    InsufficientLiquidity = 4,
    /// Invalid price (0 or > 10000 basis points)
    InvalidPrice = 5,
    /// Insufficient balance
    InsufficientBalance = 6,
    /// Order not found
    OrderNotFound = 7,
    /// Market already resolved
    AlreadyResolved = 8,
    /// Oracle call failed
    OracleCallFailed = 9,
    /// Invalid outcome
    InvalidOutcome = 10,
    /// Deadline passed
    DeadlinePassed = 11,
    /// Trading not yet started
    TradingNotStarted = 12,
    /// Market not yet resolved
    MarketNotResolved = 13,
    /// Position not found
    PositionNotFound = 14,
    /// Invalid quantity
    InvalidQuantity = 15,
}

// ============================================================================
// Core Prediction Market Contract
// ============================================================================

#[contract]
pub struct PredictionMarket;

#[contractimpl]
impl PredictionMarket {
    /// Initialize prediction markets (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        base_token: Address,
        min_liquidity: u128,
        max_shares: u128,
        default_settlement_fee_bps: u32,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&MarketKey::Owner) {
            panic_with_error!(&env, MarketError::Unauthorized);
        }

        let config = MarketConfig {
            base_token,
            min_liquidity,
            max_shares,
            default_settlement_fee_bps,
            oracle_min_threshold: 2, // Minimum 2 oracle confirmations
        };

        env.storage().instance().set(&MarketKey::Owner, &owner);
        env.storage().instance().set(&MarketKey::Config, &config);
        env.storage()
            .instance()
            .set(&MarketKey::MarketCounter, &0u64);
        env.storage()
            .instance()
            .set(&MarketKey::OrderCounter, &0u64);
        env.storage()
            .instance()
            .set(&MarketKey::TradeCounter, &0u64);

        log!(
            &env,
            "PredictionMarket: initialized - min_liquidity={}",
            min_liquidity
        );
    }

    // ========================================================================
    // Market Creation
    // ========================================================================

    /// Create a new prediction market
    pub fn create_market(
        env: Env,
        market_type: MarketType,
        title: String,
        description: String,
        outcome_yes_label: String,
        outcome_no_label: String,
        initial_liquidity: u128,
        trading_deadline_ledgers: u32,
        resolution_deadline_ledgers: u32,
        oracle_address: Address,
        settlement_fee_bps: u32,
    ) -> u64 {
        let creator = env.invoker();
        let config = Self::get_config(&env);
        let current_ledger = env.ledger().sequence();

        // Validate parameters
        if initial_liquidity < config.min_liquidity {
            panic_with_error!(&env, MarketError::InsufficientLiquidity);
        }

        if trading_deadline_ledgers == 0 || resolution_deadline_ledgers <= trading_deadline_ledgers {
            panic_with_error!(&env, MarketError::InvalidStatus);
        }

        // Get next market ID
        let market_id = Self::get_next_market_id(&env);

        let market = Market {
            market_id,
            market_type,
            title,
            description,
            outcome_yes_label,
            outcome_no_label,
            status: MarketStatus::Active,
            trading_deadline: current_ledger + trading_deadline_ledgers,
            resolution_deadline: current_ledger + resolution_deadline_ledgers,
            resolved_outcome: None,
            total_liquidity: initial_liquidity,
            yes_shares_outstanding: initial_liquidity / 2, // Start at 50/50
            no_shares_outstanding: initial_liquidity / 2,
            yes_price: 5000, // Start at 0.5 (50%)
            no_price: 5000,
            oracle_address,
            created_ledger: current_ledger,
            settlement_fee_bps,
        };

        // TODO: Collect initial_liquidity from creator
        // Transfer base_token from creator to market

        env.storage()
            .instance()
            .set(&MarketKey::Market(market_id), &market);

        // Add to creator's markets
        let mut user_markets = env
            .storage()
            .instance()
            .get::<_, Vec<u64>>(&MarketKey::UserMarkets(creator.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        user_markets.push_back(market_id);
        env.storage()
            .instance()
            .set(&MarketKey::UserMarkets(creator.clone()), &user_markets);

        log!(
            &env,
            "PredictionMarket: market created - id={}, type={}, liquidity={}",
            market_id,
            market_type as u32,
            initial_liquidity
        );

        market_id
    }

    // ========================================================================
    // Trading - Automated Market Maker (AMM)
    // ========================================================================

    /// Buy shares using AMM (constant product formula)
    pub fn buy_shares(
        env: Env,
        market_id: u64,
        is_yes: bool,
        quantity: u128,
        max_price: u32,
    ) -> u32 {
        let buyer = env.invoker();
        let mut market = Self::get_market_or_panic(&env, market_id);
        let current_ledger = env.ledger().sequence();

        // Validate market status
        if market.status != MarketStatus::Active {
            panic_with_error!(&env, MarketError::InvalidStatus);
        }
        if current_ledger > market.trading_deadline {
            panic_with_error!(&env, MarketError::DeadlinePassed);
        }

        // Calculate price using AMM formula: p = y / (x + y)
        let (current_supply, other_supply) = if is_yes {
            (market.yes_shares_outstanding, market.no_shares_outstanding)
        } else {
            (market.no_shares_outstanding, market.yes_shares_outstanding)
        };

        // Average price over quantity
        // Simplified: current price = other_supply / total_supply
        let avg_price = (other_supply * 10_000) / (current_supply + other_supply + quantity);

        if avg_price > max_price {
            panic_with_error!(&env, MarketError::InvalidPrice);
        }

        // Calculate cost: quantity * avg_price
        let cost = (quantity * avg_price as u128) / 10_000;

        // TODO: Transfer cost from buyer
        // Transfer base_token from buyer to market

        // Update user position
        let mut position = Self::get_position(&env, &buyer, market_id);
        if is_yes {
            position.yes_shares += quantity;
        } else {
            position.no_shares += quantity;
        }
        position.cost_basis += cost;

        // Update market state
        if is_yes {
            market.yes_shares_outstanding += quantity;
            market.yes_price = avg_price as u32;
        } else {
            market.no_shares_outstanding += quantity;
            market.no_price = avg_price as u32;
        }

        env.storage()
            .instance()
            .set(&MarketKey::Position(buyer.clone(), market_id), &position);
        env.storage()
            .instance()
            .set(&MarketKey::Market(market_id), &market);

        log!(
            &env,
            "PredictionMarket: buy executed - market={}, buyer={}, qty={}, price={}",
            market_id,
            buyer,
            quantity,
            avg_price
        );

        avg_price as u32
    }

    /// Sell shares back to AMM
    pub fn sell_shares(
        env: Env,
        market_id: u64,
        is_yes: bool,
        quantity: u128,
        min_price: u32,
    ) -> u32 {
        let seller = env.invoker();
        let mut market = Self::get_market_or_panic(&env, market_id);
        let current_ledger = env.ledger().sequence();

        // Validate market status
        if market.status != MarketStatus::Active {
            panic_with_error!(&env, MarketError::InvalidStatus);
        }
        if current_ledger > market.trading_deadline {
            panic_with_error!(&env, MarketError::DeadlinePassed);
        }

        let mut position = env
            .storage()
            .instance()
            .get::<_, Position>(&MarketKey::Position(seller.clone(), market_id))
            .unwrap_or_else(|| panic_with_error!(&env, MarketError::PositionNotFound));

        // Check balance
        let balance = if is_yes {
            position.yes_shares
        } else {
            position.no_shares
        };

        if balance < quantity {
            panic_with_error!(&env, MarketError::InsufficientBalance);
        }

        // Calculate price
        let (current_supply, other_supply) = if is_yes {
            (market.yes_shares_outstanding, market.no_shares_outstanding)
        } else {
            (market.no_shares_outstanding, market.yes_shares_outstanding)
        };

        let avg_price = (other_supply * 10_000) / (current_supply - quantity + other_supply);

        if avg_price < min_price {
            panic_with_error!(&env, MarketError::InvalidPrice);
        }

        // Calculate proceeds
        let proceeds = (quantity * avg_price as u128) / 10_000;

        // TODO: Transfer proceeds to seller
        // Transfer base_token from market to seller

        // Update position
        if is_yes {
            position.yes_shares -= quantity;
        } else {
            position.no_shares -= quantity;
        }

        // Update market
        if is_yes {
            market.yes_shares_outstanding -= quantity;
            market.yes_price = avg_price as u32;
        } else {
            market.no_shares_outstanding -= quantity;
            market.no_price = avg_price as u32;
        }

        env.storage()
            .instance()
            .set(&MarketKey::Position(seller.clone(), market_id), &position);
        env.storage()
            .instance()
            .set(&MarketKey::Market(market_id), &market);

        log!(
            &env,
            "PredictionMarket: sell executed - market={}, seller={}, qty={}, price={}",
            market_id,
            seller,
            quantity,
            avg_price
        );

        avg_price as u32
    }

    // ========================================================================
    // Market Resolution
    // ========================================================================

    /// Resolve market with oracle outcome
    pub fn resolve_market(env: Env, market_id: u64, outcome: Outcome) {
        let caller = env.invoker();
        let mut market = Self::get_market_or_panic(&env, market_id);

        // Only oracle can resolve
        if caller != market.oracle_address {
            panic_with_error!(&env, MarketError::Unauthorized);
        }

        // Market must be closed
        if market.status != MarketStatus::Closed && market.status != MarketStatus::Pending {
            panic_with_error!(&env, MarketError::InvalidStatus);
        }

        market.resolved_outcome = Some(outcome);
        market.status = MarketStatus::Resolved;

        env.storage()
            .instance()
            .set(&MarketKey::Market(market_id), &market);

        log!(
            &env,
            "PredictionMarket: market resolved - id={}, outcome={}",
            market_id,
            outcome as u32
        );
    }

    /// Close market (no more trading, awaiting resolution)
    pub fn close_market(env: Env, market_id: u64) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let mut market = Self::get_market_or_panic(&env, market_id);
        let current_ledger = env.ledger().sequence();

        if current_ledger < market.trading_deadline {
            panic_with_error!(&env, MarketError::TradingNotStarted);
        }

        if market.status != MarketStatus::Active {
            panic_with_error!(&env, MarketError::InvalidStatus);
        }

        market.status = MarketStatus::Closed;

        env.storage()
            .instance()
            .set(&MarketKey::Market(market_id), &market);

        log!(
            &env,
            "PredictionMarket: market closed - id={}",
            market_id
        );
    }

    // ========================================================================
    // Settlement
    // ========================================================================

    /// Settle user's position in resolved market
    pub fn settle_position(env: Env, market_id: u64, user: Address) -> u128 {
        let market = Self::get_market_or_panic(&env, market_id);

        if market.status != MarketStatus::Resolved {
            panic_with_error!(&env, MarketError::MarketNotResolved);
        }

        let resolved_outcome = market
            .resolved_outcome
            .as_ref()
            .unwrap_or_else(|| panic_with_error!(&env, MarketError::InvalidOutcome));

        let mut position = env
            .storage()
            .instance()
            .get::<_, Position>(&MarketKey::Position(user.clone(), market_id))
            .unwrap_or_else(|| panic_with_error!(&env, MarketError::PositionNotFound));

        if position.settled {
            return 0; // Already settled
        }

        let payout = match resolved_outcome {
            Outcome::Yes => {
                // Yes holders win
                (position.yes_shares * 10_000) / market.yes_shares_outstanding
            }
            Outcome::No => {
                // No holders win
                (position.no_shares * 10_000) / market.no_shares_outstanding
            }
            Outcome::Invalid => {
                // Everyone refunded
                position.cost_basis
            }
        };

        // Calculate fees
        let fee = (payout * market.settlement_fee_bps as u128) / 10_000;
        let net_payout = payout - fee;

        // TODO: Transfer net_payout to user

        position.settled = true;

        env.storage()
            .instance()
            .set(&MarketKey::Position(user.clone(), market_id), &position);

        log!(
            &env,
            "PredictionMarket: position settled - user={}, market={}, payout={}",
            user,
            market_id,
            net_payout
        );

        net_payout
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get market details
    pub fn get_market(env: Env, market_id: u64) -> Option<Market> {
        env.storage()
            .instance()
            .get(&MarketKey::Market(market_id))
    }

    /// Get user position
    pub fn get_position_info(env: Env, user: Address, market_id: u64) -> Option<Position> {
        env.storage()
            .instance()
            .get(&MarketKey::Position(user, market_id))
    }

    /// Get current prices for market
    pub fn get_prices(env: Env, market_id: u64) -> (u32, u32) {
        let market = Self::get_market_or_panic(&env, market_id);
        (market.yes_price, market.no_price)
    }

    /// Get market configuration
    pub fn get_market_config(env: Env) -> MarketConfig {
        Self::get_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&MarketKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, MarketError::Unauthorized))
    }

    fn get_config(env: &Env) -> MarketConfig {
        env.storage()
            .instance()
            .get(&MarketKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, MarketError::Unauthorized))
    }

    fn get_market_or_panic(env: &Env, market_id: u64) -> Market {
        env.storage()
            .instance()
            .get(&MarketKey::Market(market_id))
            .unwrap_or_else(|| panic_with_error!(env, MarketError::MarketNotFound))
    }

    fn get_position(env: &Env, user: &Address, market_id: u64) -> Position {
        env.storage()
            .instance()
            .get(&MarketKey::Position(user.clone(), market_id))
            .unwrap_or(Position {
                user: user.clone(),
                market_id,
                yes_shares: 0,
                no_shares: 0,
                cost_basis: 0,
                settled: false,
            })
    }

    fn get_next_market_id(env: &Env) -> u64 {
        let current = env
            .storage()
            .instance()
            .get::<_, u64>(&MarketKey::MarketCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&MarketKey::MarketCounter, &(current + 1));

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_creation() {
        // Tests for market setup
    }

    #[test]
    fn test_amm_trading() {
        // Tests for AMM pricing and trades
    }

    #[test]
    fn test_market_resolution() {
        // Tests for oracle resolution
    }

    #[test]
    fn test_settlement() {
        // Tests for payout calculations
    }
}
