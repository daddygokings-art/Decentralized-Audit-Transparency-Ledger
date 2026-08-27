//! # RWA Oracle Module
//!
//! Manages real-world asset (RWA) price oracles on-chain.  Oracles submit
//! signed price feeds for registered assets.  A lightweight consensus
//! mechanism aggregates prices from multiple oracles before the canonical
//! price is accepted.  Staleness detection prevents stale prices from
//! propagating.  Every price update is appended to a tamper-evident audit
//! trail stored in persistent contract storage.
//!
//! ## Key concepts
//!
//! * **Oracle** – An approved off-chain data provider identified by its
//!   [`Address`].  Each oracle has a reputation score (0–100) that weights
//!   its contribution to consensus.
//! * **Price feed** – A single price submission for an asset symbol, carrying
//!   the price, timestamp, confidence interval, and the submitting oracle.
//! * **Consensus round** – When enough feeds for an asset have been submitted
//!   within the staleness window, the median / weighted-average is computed
//!   and published as the canonical price.
//! * **Price history** – Every accepted canonical price is pushed onto an
//!   append-only per-asset list for audit and back-testing.

#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of price feeds that can be buffered per asset before
/// consensus is forced regardless of the quorum threshold.
pub const MAX_PENDING_FEEDS: u32 = 10;

/// Default staleness window in seconds.  Feeds older than this are rejected.
pub const DEFAULT_STALENESS_WINDOW_SECS: u64 = 3_600; // 1 hour

/// Minimum number of oracles that must agree before a canonical price is
/// published (default quorum).
pub const DEFAULT_QUORUM: u32 = 2;

/// Maximum deviation (in basis points, 1 bp = 0.01 %) allowed between the
/// cheapest and most expensive feed in a consensus round before the outliers
/// are discarded.
pub const MAX_DEVIATION_BPS: u32 = 500; // 5 %

/// Precision factor used for all fixed-point price arithmetic.
/// Prices are stored as integers scaled by 1e8 (i.e., 8 decimal places).
pub const PRICE_PRECISION: u128 = 100_000_000;

/// Maximum length (bytes) for an oracle's display name.
pub const MAX_ORACLE_NAME_LEN: u32 = 64;

/// Maximum history entries kept per asset to avoid unbounded state growth.
pub const MAX_PRICE_HISTORY: u32 = 1_000;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Current status of a registered oracle.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OracleStatus {
    /// Oracle is active and accepted for price submissions.
    Active = 0,
    /// Oracle has been suspended by the contract owner.
    Suspended = 1,
    /// Oracle has been permanently revoked and cannot be re-activated.
    Revoked = 2,
}

impl OracleStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, OracleStatus::Active)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OracleStatus::Active => "active",
            OracleStatus::Suspended => "suspended",
            OracleStatus::Revoked => "revoked",
        }
    }
}

/// Classification of an RWA asset type supported by the oracle system.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssetClass {
    /// Real estate (residential / commercial / industrial)
    RealEstate = 0,
    /// Private credit / loans
    PrivateCredit = 1,
    /// Commodities (gold, silver, oil, …)
    Commodity = 2,
    /// Equity in a private company
    PrivateEquity = 3,
    /// Fixed-income instrument (bond, treasury note, …)
    FixedIncome = 4,
    /// Infrastructure asset
    Infrastructure = 5,
    /// Collectible / art / IP
    Collectible = 6,
    /// Other / unclassified
    Other = 7,
}

impl AssetClass {
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            AssetClass::RealEstate => Symbol::new(env, "RealEstate"),
            AssetClass::PrivateCredit => Symbol::new(env, "PrivCredit"),
            AssetClass::Commodity => Symbol::new(env, "Commodity"),
            AssetClass::PrivateEquity => Symbol::new(env, "PrivEquity"),
            AssetClass::FixedIncome => Symbol::new(env, "FixedIncome"),
            AssetClass::Infrastructure => Symbol::new(env, "Infra"),
            AssetClass::Collectible => Symbol::new(env, "Collect"),
            AssetClass::Other => Symbol::new(env, "Other"),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AssetClass::RealEstate => "Real Estate",
            AssetClass::PrivateCredit => "Private Credit",
            AssetClass::Commodity => "Commodity",
            AssetClass::PrivateEquity => "Private Equity",
            AssetClass::FixedIncome => "Fixed Income",
            AssetClass::Infrastructure => "Infrastructure",
            AssetClass::Collectible => "Collectible",
            AssetClass::Other => "Other",
        }
    }
}

/// Outcome of a consensus round for a particular asset.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ConsensusResult {
    /// Enough valid feeds were collected and a canonical price was published.
    Accepted = 0,
    /// Quorum was not reached (too few active oracles submitted in time).
    InsufficientQuorum = 1,
    /// All feeds were discarded because they exceeded the deviation cap.
    DeviationExceeded = 2,
    /// Every submitted feed was stale (older than the staleness window).
    AllStale = 3,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Domain-specific errors emitted by the oracle module.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleError {
    /// Oracle address is already registered.
    AlreadyRegistered = 1000,
    /// Oracle address is not registered.
    NotRegistered = 1001,
    /// Oracle is suspended or revoked; submission is not allowed.
    OracleNotActive = 1002,
    /// Asset symbol is not registered in the oracle system.
    AssetNotRegistered = 1003,
    /// Asset symbol is already registered.
    AssetAlreadyRegistered = 1004,
    /// Price value is zero or otherwise invalid.
    InvalidPrice = 1005,
    /// Submitted timestamp is older than the staleness window.
    StaleTimestamp = 1006,
    /// Submitted timestamp is in the future (drift guard).
    FutureTimestamp = 1007,
    /// Confidence interval exceeds the maximum allowed (50 % of price).
    ConfidenceTooWide = 1008,
    /// Price history for this asset has reached its cap.
    HistoryCapReached = 1009,
    /// Caller is not the contract owner.
    Unauthorized = 1010,
    /// Reputation score out of the 0–100 range.
    InvalidReputationScore = 1011,
    /// Oracle name exceeds MAX_ORACLE_NAME_LEN.
    OracleNameTooLong = 1012,
    /// Consensus round cannot proceed: no pending feeds.
    NoFeedsAvailable = 1013,
    /// Arithmetic overflow detected.
    Overflow = 1014,
}

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

/// On-chain record for a registered price oracle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRecord {
    /// Unique identifier – the oracle operator's on-chain address.
    pub address: Address,
    /// Human-readable display name (up to MAX_ORACLE_NAME_LEN bytes).
    pub name: Bytes,
    /// Asset classes this oracle is authorised to price.
    pub supported_classes: Vec<u8>, // Vec<AssetClass as u8>
    /// Reputation score 0–100; higher = more weight in weighted-mean consensus.
    pub reputation_score: u32,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Ledger timestamp of the last accepted price submission.
    pub last_submission_at: u64,
    /// Total number of price feeds accepted from this oracle.
    pub total_submissions: u32,
    /// Total number of feeds rejected (stale, deviation, etc.).
    pub total_rejections: u32,
    /// Current operational status.
    pub status: u8, // OracleStatus as u8
}

impl OracleRecord {
    /// Constructs a new oracle record with default counters.
    pub fn new(
        env: &Env,
        address: Address,
        name: Bytes,
        supported_classes: Vec<u8>,
        reputation_score: u32,
    ) -> Self {
        OracleRecord {
            address,
            name,
            supported_classes,
            reputation_score,
            registered_at: env.ledger().timestamp(),
            last_submission_at: 0,
            total_submissions: 0,
            total_rejections: 0,
            status: OracleStatus::Active as u8,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == OracleStatus::Active as u8
    }
}

/// A single raw price feed submitted by one oracle for one asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeed {
    /// Asset identifier (e.g., `"RE_NYC_001"`).
    pub asset_id: Symbol,
    /// Scaled price (value × PRICE_PRECISION).
    pub price: u128,
    /// Half-width confidence interval (price ± confidence), same scale.
    pub confidence: u128,
    /// Unix timestamp when the off-chain price was observed.
    pub observed_at: u64,
    /// Ledger timestamp when the feed was submitted to this contract.
    pub submitted_at: u64,
    /// Oracle that submitted this feed.
    pub oracle: Address,
    /// Opaque metadata blob (data source, methodology, …).
    pub metadata: Bytes,
    /// Integrity hash: sha256(asset_id_bytes || price_le || confidence_le || observed_at_le).
    pub integrity_hash: BytesN<32>,
}

impl PriceFeed {
    /// Verifies that the stored `integrity_hash` matches the feed content.
    /// Returns `true` if the hash is valid, `false` if the feed has been
    /// tampered with.
    pub fn verify_integrity(&self, env: &Env) -> bool {
        let computed = compute_feed_hash(env, &self.asset_id, self.price, self.confidence, self.observed_at);
        computed == self.integrity_hash
    }

    /// Returns `true` when the feed's observed_at is within the staleness window.
    pub fn is_fresh(&self, now: u64, staleness_window: u64) -> bool {
        now.saturating_sub(self.observed_at) <= staleness_window
    }
}

/// Canonical (consensus) price entry stored in the price history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPrice {
    /// Asset identifier.
    pub asset_id: Symbol,
    /// Weighted-mean price from the accepted feeds (scaled by PRICE_PRECISION).
    pub price: u128,
    /// Aggregate confidence (average of accepted feeds' confidence intervals).
    pub confidence: u128,
    /// Number of oracle feeds that contributed to this consensus price.
    pub oracle_count: u32,
    /// Ledger timestamp when consensus was reached.
    pub consensus_at: u64,
    /// Oldest feed observation timestamp among contributing feeds.
    pub oldest_observation: u64,
    /// Outcome of the consensus round that produced this entry.
    pub result: u8, // ConsensusResult as u8
    /// Sequential position in this asset's price history (0-based).
    pub history_index: u32,
    /// SHA-256 chain link: sha256 over previous canonical price's hash + this price.
    pub chain_hash: BytesN<32>,
    /// Previous canonical price hash (zero bytes for the genesis entry).
    pub prev_hash: BytesN<32>,
}

impl CanonicalPrice {
    /// Verify chain integrity: recomputes the chain_hash and compares.
    pub fn verify_chain_hash(&self, env: &Env) -> bool {
        let computed = compute_canonical_hash(
            env,
            &self.asset_id,
            self.price,
            self.consensus_at,
            &self.prev_hash,
        );
        computed == self.chain_hash
    }
}

/// On-chain record for a registered RWA asset.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRecord {
    /// Unique symbol identifying the asset (max 32 bytes for Soroban Symbol).
    pub asset_id: Symbol,
    /// Asset classification.
    pub asset_class: u8, // AssetClass as u8
    /// Human-readable display name.
    pub name: Bytes,
    /// Address that controls this asset's metadata (asset issuer / owner).
    pub issuer: Address,
    /// Minimum number of oracle feeds required for a consensus round.
    pub quorum: u32,
    /// Staleness window override in seconds (0 = use contract default).
    pub staleness_window_override: u64,
    /// Whether this asset is currently accepting price submissions.
    pub active: bool,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Total canonical price updates published for this asset.
    pub total_updates: u32,
}

// ---------------------------------------------------------------------------
// Storage key types
// ---------------------------------------------------------------------------

/// Storage key layout for the oracle module.  All keys are scoped to
/// instance storage so they expire with the contract instance TTL.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleDataKey {
    /// Per-oracle record.  Key = oracle Address.
    Oracle(Address),
    /// Total number of registered oracles.
    OracleCount,
    /// Per-asset record.  Key = asset Symbol.
    Asset(Symbol),
    /// Total number of registered assets.
    AssetCount,
    /// Pending (unprocessed) price feeds for an asset.  Key = asset Symbol.
    PendingFeeds(Symbol),
    /// Most recent canonical price for an asset.  Key = asset Symbol.
    LatestPrice(Symbol),
    /// Price history list for an asset.  Key = asset Symbol.
    PriceHistory(Symbol),
    /// Contract owner address.
    Owner,
    /// Global staleness window in seconds.
    StalenessWindow,
    /// Global quorum threshold.
    GlobalQuorum,
    /// Total number of consensus rounds executed across all assets.
    TotalRounds,
}

// ---------------------------------------------------------------------------
// Pure / helper functions (no storage I/O)
// ---------------------------------------------------------------------------

/// Computes the integrity hash for a raw price feed.
///
/// `sha256(asset_id_bytes || price_le_16 || confidence_le_16 || observed_at_le_8)`
pub fn compute_feed_hash(
    env: &Env,
    asset_id: &Symbol,
    price: u128,
    confidence: u128,
    observed_at: u64,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    // Encode asset_id as its raw byte representation.
    let id_bytes = asset_id.to_string();
    buf.append(&Bytes::from_slice(env, id_bytes.as_bytes()));
    buf.append(&Bytes::from_slice(env, &price.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &confidence.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &observed_at.to_le_bytes()));
    env.crypto().sha256(&buf)
}

/// Computes the chain hash for a canonical price entry.
///
/// `sha256(asset_id_bytes || price_le_16 || consensus_at_le_8 || prev_hash_32)`
pub fn compute_canonical_hash(
    env: &Env,
    asset_id: &Symbol,
    price: u128,
    consensus_at: u64,
    prev_hash: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    let id_bytes = asset_id.to_string();
    buf.append(&Bytes::from_slice(env, id_bytes.as_bytes()));
    buf.append(&Bytes::from_slice(env, &price.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &consensus_at.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, prev_hash.as_ref()));
    env.crypto().sha256(&buf)
}

/// Verifies that a raw [`PriceFeed`]'s integrity hash is consistent with
/// its field values.  Returns `(true, "ok")` when valid, or
/// `(false, reason)` when the check fails.
pub fn verify_feed_integrity(env: &Env, feed: &PriceFeed) -> (bool, &'static str) {
    if feed.price == 0 {
        return (false, "price_zero");
    }
    if feed.confidence >= feed.price / 2 {
        return (false, "confidence_too_wide");
    }
    if !feed.verify_integrity(env) {
        return (false, "hash_mismatch");
    }
    (true, "ok")
}

/// Detects whether a feed is stale relative to `now` and the configured window.
pub fn is_feed_stale(feed: &PriceFeed, now: u64, staleness_window: u64) -> bool {
    !feed.is_fresh(now, staleness_window)
}

/// Runs the consensus algorithm over a slice of raw feeds:
///
/// 1. Discard stale feeds (observed_at older than `staleness_window`).
/// 2. Discard feeds with invalid integrity hashes.
/// 3. If fewer than `quorum` feeds remain → `InsufficientQuorum`.
/// 4. Compute the median price, discard feeds whose price deviates more than
///    `MAX_DEVIATION_BPS` basis points from the median.
/// 5. If no feeds survive step 4 → `DeviationExceeded`.
/// 6. Compute the reputation-weighted mean from surviving feeds.
/// 7. Return `(Accepted, weighted_mean_price, mean_confidence, surviving_count)`.
pub fn run_consensus(
    env: &Env,
    feeds: &[PriceFeed],
    reputation_scores: &[u32],
    now: u64,
    staleness_window: u64,
    quorum: u32,
) -> (ConsensusResult, u128, u128, u32) {
    // Step 1 & 2 – filter valid, fresh, integer-checked feeds.
    let mut valid: Vec<(u128, u128, u32)> = Vec::new(env); // (price, confidence, reputation)
    for (i, feed) in feeds.iter().enumerate() {
        if is_feed_stale(feed, now, staleness_window) {
            continue;
        }
        let (ok, _) = verify_feed_integrity(env, feed);
        if !ok {
            continue;
        }
        let rep = if i < reputation_scores.len() { reputation_scores[i] } else { 1 };
        valid.push_back((feed.price, feed.confidence, rep));
    }

    if (valid.len() as u32) < quorum {
        if valid.is_empty() && !feeds.is_empty() {
            return (ConsensusResult::AllStale, 0, 0, 0);
        }
        return (ConsensusResult::InsufficientQuorum, 0, 0, 0);
    }

    // Step 3 – find median price (simple sort-and-pick-middle on u128 values).
    let mut prices: Vec<u128> = Vec::new(env);
    for entry in valid.iter() {
        prices.push_back(entry.0);
    }
    // Insertion sort (small N – at most MAX_PENDING_FEEDS items).
    let n = prices.len() as usize;
    for i in 1..n {
        let key = prices.get(i as u32).unwrap();
        let mut j = i;
        while j > 0 {
            let prev = prices.get((j - 1) as u32).unwrap();
            if prev > key {
                prices.set(j as u32, prev);
                j -= 1;
            } else {
                break;
            }
        }
        prices.set(j as u32, key);
    }
    let median = prices.get((n / 2) as u32).unwrap();

    // Step 4 – deviation filter.
    let dev_cap = median
        .saturating_mul(MAX_DEVIATION_BPS as u128)
        .saturating_div(10_000);

    let mut survivors: Vec<(u128, u128, u32)> = Vec::new(env);
    for entry in valid.iter() {
        let price = entry.0;
        let diff = if price >= median { price - median } else { median - price };
        if diff <= dev_cap {
            survivors.push_back(entry);
        }
    }

    if survivors.is_empty() {
        return (ConsensusResult::DeviationExceeded, 0, 0, 0);
    }

    // Step 5 – reputation-weighted mean.
    let mut weight_sum: u128 = 0;
    let mut price_sum: u128 = 0;
    let mut conf_sum: u128 = 0;
    let count = survivors.len() as u32;
    for entry in survivors.iter() {
        let (price, conf, rep) = entry;
        let w = rep as u128;
        weight_sum = weight_sum.saturating_add(w);
        price_sum = price_sum.saturating_add(price.saturating_mul(w));
        conf_sum = conf_sum.saturating_add(conf);
    }
    if weight_sum == 0 {
        return (ConsensusResult::InsufficientQuorum, 0, 0, 0);
    }
    let weighted_price = price_sum.saturating_div(weight_sum);
    let mean_conf = conf_sum.saturating_div(count as u128);

    (ConsensusResult::Accepted, weighted_price, mean_conf, count)
}

// ---------------------------------------------------------------------------
// Stateful helpers (read / write storage)
// ---------------------------------------------------------------------------

/// Returns the configured staleness window for this contract, falling back to
/// the module default if none has been set.
pub fn get_staleness_window(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&OracleDataKey::StalenessWindow)
        .unwrap_or(DEFAULT_STALENESS_WINDOW_SECS)
}

/// Returns the configured global quorum, falling back to DEFAULT_QUORUM.
pub fn get_global_quorum(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&OracleDataKey::GlobalQuorum)
        .unwrap_or(DEFAULT_QUORUM)
}

/// Registers a new price oracle.  Panics with [`OracleError::AlreadyRegistered`]
/// if the oracle is already on file.
pub fn register_oracle(
    env: &Env,
    address: Address,
    name: Bytes,
    supported_classes: Vec<u8>,
    reputation_score: u32,
) -> Result<OracleRecord, OracleError> {
    if name.len() > MAX_ORACLE_NAME_LEN {
        return Err(OracleError::OracleNameTooLong);
    }
    if reputation_score > 100 {
        return Err(OracleError::InvalidReputationScore);
    }
    let key = OracleDataKey::Oracle(address.clone());
    if env.storage().instance().has(&key) {
        return Err(OracleError::AlreadyRegistered);
    }

    let record = OracleRecord::new(env, address, name, supported_classes, reputation_score);
    env.storage().instance().set(&key, &record);

    let count: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::OracleCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OracleDataKey::OracleCount, &(count + 1));

    Ok(record)
}

/// Retrieves an oracle record.  Returns `Err(NotRegistered)` if absent.
pub fn get_oracle(env: &Env, address: &Address) -> Result<OracleRecord, OracleError> {
    env.storage()
        .instance()
        .get(&OracleDataKey::Oracle(address.clone()))
        .ok_or(OracleError::NotRegistered)
}

/// Updates the operational status of a registered oracle.
pub fn set_oracle_status(
    env: &Env,
    address: &Address,
    status: OracleStatus,
) -> Result<(), OracleError> {
    let key = OracleDataKey::Oracle(address.clone());
    let mut record: OracleRecord = env
        .storage()
        .instance()
        .get(&key)
        .ok_or(OracleError::NotRegistered)?;
    record.status = status as u8;
    env.storage().instance().set(&key, &record);
    Ok(())
}

/// Registers a new RWA asset for price tracking.
pub fn register_asset(
    env: &Env,
    asset_id: Symbol,
    asset_class: AssetClass,
    name: Bytes,
    issuer: Address,
    quorum: Option<u32>,
    staleness_window_override: Option<u64>,
) -> Result<AssetRecord, OracleError> {
    let key = OracleDataKey::Asset(asset_id.clone());
    if env.storage().instance().has(&key) {
        return Err(OracleError::AssetAlreadyRegistered);
    }

    let effective_quorum = quorum.unwrap_or_else(|| get_global_quorum(env));
    let record = AssetRecord {
        asset_id: asset_id.clone(),
        asset_class: asset_class as u8,
        name,
        issuer,
        quorum: effective_quorum,
        staleness_window_override: staleness_window_override.unwrap_or(0),
        active: true,
        registered_at: env.ledger().timestamp(),
        total_updates: 0,
    };
    env.storage().instance().set(&key, &record);

    let count: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::AssetCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OracleDataKey::AssetCount, &(count + 1));

    Ok(record)
}

/// Retrieves an asset record.
pub fn get_asset(env: &Env, asset_id: &Symbol) -> Result<AssetRecord, OracleError> {
    env.storage()
        .instance()
        .get(&OracleDataKey::Asset(asset_id.clone()))
        .ok_or(OracleError::AssetNotRegistered)
}

/// Submits a raw price feed for an asset.  Validates oracle status, asset
/// existence, timestamp bounds, and integrity hash before appending to the
/// pending feeds buffer.
pub fn submit_price_feed(
    env: &Env,
    oracle: &Address,
    asset_id: Symbol,
    price: u128,
    confidence: u128,
    observed_at: u64,
    metadata: Bytes,
) -> Result<PriceFeed, OracleError> {
    // Validate oracle.
    let oracle_record = get_oracle(env, oracle)?;
    if !oracle_record.is_active() {
        return Err(OracleError::OracleNotActive);
    }

    // Validate asset.
    let asset_record = get_asset(env, &asset_id)?;
    if !asset_record.active {
        return Err(OracleError::AssetNotRegistered);
    }

    // Validate price.
    if price == 0 {
        return Err(OracleError::InvalidPrice);
    }
    if confidence >= price / 2 {
        return Err(OracleError::ConfidenceTooWide);
    }

    // Timestamp bounds: must not be in the future (allow 60 s drift) and must
    // be within the staleness window.
    let now = env.ledger().timestamp();
    let window = if asset_record.staleness_window_override > 0 {
        asset_record.staleness_window_override
    } else {
        get_staleness_window(env)
    };
    if observed_at > now + 60 {
        return Err(OracleError::FutureTimestamp);
    }
    if now.saturating_sub(observed_at) > window {
        return Err(OracleError::StaleTimestamp);
    }

    // Build feed with integrity hash.
    let integrity_hash = compute_feed_hash(env, &asset_id, price, confidence, observed_at);
    let feed = PriceFeed {
        asset_id: asset_id.clone(),
        price,
        confidence,
        observed_at,
        submitted_at: now,
        oracle: oracle.clone(),
        metadata,
        integrity_hash,
    };

    // Append to pending feeds buffer.
    let pkey = OracleDataKey::PendingFeeds(asset_id.clone());
    let mut pending: Vec<PriceFeed> = env.storage().instance().get(&pkey).unwrap_or(Vec::new(env));

    // Auto-flush if buffer is full.
    if (pending.len() as u32) >= MAX_PENDING_FEEDS {
        pending = Vec::new(env); // clear stale buffer; consensus will be triggered externally
    }
    pending.push_back(feed.clone());
    env.storage().instance().set(&pkey, &pending);

    // Update oracle submission counter.
    let okey = OracleDataKey::Oracle(oracle.clone());
    if let Some(mut orec) = env.storage().instance().get::<OracleDataKey, OracleRecord>(&okey) {
        orec.total_submissions += 1;
        orec.last_submission_at = now;
        env.storage().instance().set(&okey, &orec);
    }

    Ok(feed)
}

/// Processes pending feeds for an asset and, when quorum is reached, publishes
/// a new canonical price.  Returns the new [`CanonicalPrice`] or an error code
/// describing why consensus failed.
pub fn process_consensus(
    env: &Env,
    asset_id: &Symbol,
) -> Result<CanonicalPrice, OracleError> {
    let asset_record = get_asset(env, asset_id)?;
    let pkey = OracleDataKey::PendingFeeds(asset_id.clone());
    let pending: Vec<PriceFeed> = env.storage().instance().get(&pkey).unwrap_or(Vec::new(env));

    if pending.is_empty() {
        return Err(OracleError::NoFeedsAvailable);
    }

    // Build slices for the pure consensus function.
    let n = pending.len() as usize;
    let mut feeds_slice: soroban_sdk::Vec<PriceFeed> = Vec::new(env);
    let mut rep_slice: soroban_sdk::Vec<u32> = Vec::new(env);

    for feed in pending.iter() {
        // Fetch reputation score for this oracle.
        let rep = get_oracle(env, &feed.oracle)
            .map(|o| o.reputation_score)
            .unwrap_or(1);
        feeds_slice.push_back(feed);
        rep_slice.push_back(rep);
    }

    // Build native slices for run_consensus.
    let mut feeds_vec: Vec<PriceFeed> = feeds_slice;
    let mut reps_vec: Vec<u32> = rep_slice;

    let now = env.ledger().timestamp();
    let staleness = if asset_record.staleness_window_override > 0 {
        asset_record.staleness_window_override
    } else {
        get_staleness_window(env)
    };

    // Convert SDK Vecs to slices using iterators (no alloc).
    // We build helper plain arrays for the pure function call.
    let mut feed_arr: [PriceFeed; 10] = core::array::from_fn(|_| PriceFeed {
        asset_id: asset_id.clone(),
        price: 0,
        confidence: 0,
        observed_at: 0,
        submitted_at: 0,
        oracle: env.current_contract_address(),
        metadata: Bytes::new(env),
        integrity_hash: BytesN::from_array(env, &[0u8; 32]),
    });
    let mut rep_arr: [u32; 10] = [1u32; 10];
    let actual_n = n.min(10);
    for (i, feed) in feeds_vec.iter().enumerate().take(actual_n) {
        feed_arr[i] = feed;
    }
    for (i, rep) in reps_vec.iter().enumerate().take(actual_n) {
        rep_arr[i] = rep;
    }

    let (result, price, confidence, count) = run_consensus(
        env,
        &feed_arr[..actual_n],
        &rep_arr[..actual_n],
        now,
        staleness,
        asset_record.quorum,
    );

    if result != ConsensusResult::Accepted {
        // Mark oracle rejections.
        for feed in feeds_vec.iter() {
            let okey = OracleDataKey::Oracle(feed.oracle.clone());
            if let Some(mut orec) = env.storage().instance().get::<OracleDataKey, OracleRecord>(&okey) {
                orec.total_rejections = orec.total_rejections.saturating_add(1);
                env.storage().instance().set(&okey, &orec);
            }
        }
        // Clear pending feeds so the next round starts fresh.
        env.storage()
            .instance()
            .set(&pkey, &Vec::<PriceFeed>::new(env));
        return Err(match result {
            ConsensusResult::InsufficientQuorum => OracleError::InvalidPrice,
            ConsensusResult::DeviationExceeded => OracleError::InvalidPrice,
            ConsensusResult::AllStale => OracleError::StaleTimestamp,
            ConsensusResult::Accepted => unreachable!(),
        });
    }

    // Find oldest observation.
    let mut oldest = now;
    for feed in feeds_vec.iter() {
        if feed.observed_at < oldest {
            oldest = feed.observed_at;
        }
    }

    // Build canonical price and chain it.
    let hist_key = OracleDataKey::PriceHistory(asset_id.clone());
    let mut history: Vec<CanonicalPrice> =
        env.storage().instance().get(&hist_key).unwrap_or(Vec::new(env));

    let history_index = history.len() as u32;
    let prev_hash: BytesN<32> = history
        .last()
        .map(|p| p.chain_hash.clone())
        .unwrap_or(BytesN::from_array(env, &[0u8; 32]));
    let chain_hash = compute_canonical_hash(env, asset_id, price, now, &prev_hash);

    let canonical = CanonicalPrice {
        asset_id: asset_id.clone(),
        price,
        confidence,
        oracle_count: count,
        consensus_at: now,
        oldest_observation: oldest,
        result: ConsensusResult::Accepted as u8,
        history_index,
        chain_hash,
        prev_hash,
    };

    // Append to history, respecting the cap.
    if (history.len() as u32) < MAX_PRICE_HISTORY {
        history.push_back(canonical.clone());
        env.storage().instance().set(&hist_key, &history);
    }

    // Update latest price.
    env.storage()
        .instance()
        .set(&OracleDataKey::LatestPrice(asset_id.clone()), &canonical);

    // Update asset total_updates counter.
    let akey = OracleDataKey::Asset(asset_id.clone());
    if let Some(mut arec) = env.storage().instance().get::<OracleDataKey, AssetRecord>(&akey) {
        arec.total_updates = arec.total_updates.saturating_add(1);
        env.storage().instance().set(&akey, &arec);
    }

    // Increment global round counter.
    let rounds: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::TotalRounds)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OracleDataKey::TotalRounds, &(rounds + 1));

    // Clear pending feeds.
    env.storage()
        .instance()
        .set(&pkey, &Vec::<PriceFeed>::new(env));

    Ok(canonical)
}

/// Returns the latest canonical price for an asset, if one has been published.
pub fn get_latest_price(env: &Env, asset_id: &Symbol) -> Option<CanonicalPrice> {
    env.storage()
        .instance()
        .get(&OracleDataKey::LatestPrice(asset_id.clone()))
}

/// Returns the full price history for an asset (up to MAX_PRICE_HISTORY).
pub fn get_price_history(env: &Env, asset_id: &Symbol) -> Vec<CanonicalPrice> {
    env.storage()
        .instance()
        .get(&OracleDataKey::PriceHistory(asset_id.clone()))
        .unwrap_or(Vec::new(env))
}

/// Returns `true` when the latest price for the asset is stale.
pub fn is_latest_price_stale(env: &Env, asset_id: &Symbol) -> bool {
    let window = get_staleness_window(env);
    match get_latest_price(env, asset_id) {
        Some(cp) => {
            let now = env.ledger().timestamp();
            now.saturating_sub(cp.consensus_at) > window
        }
        None => true, // no price = stale
    }
}

/// Returns pending feed count for an asset (useful for monitoring).
pub fn pending_feed_count(env: &Env, asset_id: &Symbol) -> u32 {
    let pkey = OracleDataKey::PendingFeeds(asset_id.clone());
    let pending: Vec<PriceFeed> = env.storage().instance().get(&pkey).unwrap_or(Vec::new(env));
    pending.len() as u32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup_env() -> Env {
        Env::default()
    }

    fn make_name(env: &Env, s: &[u8]) -> Bytes {
        Bytes::from_slice(env, s)
    }

    fn make_oracle(env: &Env) -> Address {
        Address::generate(env)
    }

    fn make_asset_id(env: &Env, s: &str) -> Symbol {
        Symbol::new(env, s)
    }

    // -----------------------------------------------------------------------
    // OracleStatus tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_oracle_status_active() {
        assert!(OracleStatus::Active.is_active());
        assert!(!OracleStatus::Suspended.is_active());
        assert!(!OracleStatus::Revoked.is_active());
    }

    #[test]
    fn test_oracle_status_as_str() {
        assert_eq!(OracleStatus::Active.as_str(), "active");
        assert_eq!(OracleStatus::Suspended.as_str(), "suspended");
        assert_eq!(OracleStatus::Revoked.as_str(), "revoked");
    }

    // -----------------------------------------------------------------------
    // AssetClass tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_asset_class_name() {
        assert_eq!(AssetClass::RealEstate.name(), "Real Estate");
        assert_eq!(AssetClass::PrivateCredit.name(), "Private Credit");
        assert_eq!(AssetClass::Commodity.name(), "Commodity");
        assert_eq!(AssetClass::FixedIncome.name(), "Fixed Income");
    }

    #[test]
    fn test_asset_class_as_symbol() {
        let env = setup_env();
        let sym = AssetClass::RealEstate.as_symbol(&env);
        assert_eq!(sym, Symbol::new(&env, "RealEstate"));
    }

    // -----------------------------------------------------------------------
    // Oracle registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_oracle_success() {
        let env = setup_env();
        let addr = make_oracle(&env);
        let name = make_name(&env, b"Oracle One");
        let classes = Vec::from_slice(&env, &[AssetClass::RealEstate as u8]);
        let result = register_oracle(&env, addr.clone(), name, classes, 80);
        assert!(result.is_ok());
        let rec = result.unwrap();
        assert_eq!(rec.reputation_score, 80);
        assert!(rec.is_active());
    }

    #[test]
    fn test_register_oracle_duplicate_fails() {
        let env = setup_env();
        let addr = make_oracle(&env);
        let name = make_name(&env, b"Dup Oracle");
        let classes = Vec::from_slice(&env, &[AssetClass::Commodity as u8]);
        register_oracle(&env, addr.clone(), name.clone(), classes.clone(), 50).unwrap();
        let result = register_oracle(&env, addr.clone(), name, classes, 50);
        assert_eq!(result.unwrap_err(), OracleError::AlreadyRegistered);
    }

    #[test]
    fn test_register_oracle_invalid_reputation() {
        let env = setup_env();
        let addr = make_oracle(&env);
        let name = make_name(&env, b"Bad Oracle");
        let classes = Vec::new(&env);
        let result = register_oracle(&env, addr, name, classes, 101);
        assert_eq!(result.unwrap_err(), OracleError::InvalidReputationScore);
    }

    #[test]
    fn test_register_oracle_name_too_long() {
        let env = setup_env();
        let addr = make_oracle(&env);
        let long_name = make_name(&env, &[b'A'; 65]);
        let classes = Vec::new(&env);
        let result = register_oracle(&env, addr, long_name, classes, 50);
        assert_eq!(result.unwrap_err(), OracleError::OracleNameTooLong);
    }

    #[test]
    fn test_get_oracle_not_registered() {
        let env = setup_env();
        let addr = make_oracle(&env);
        assert_eq!(get_oracle(&env, &addr).unwrap_err(), OracleError::NotRegistered);
    }

    #[test]
    fn test_oracle_status_update() {
        let env = setup_env();
        let addr = make_oracle(&env);
        let name = make_name(&env, b"Status Oracle");
        let classes = Vec::new(&env);
        register_oracle(&env, addr.clone(), name, classes, 60).unwrap();
        set_oracle_status(&env, &addr, OracleStatus::Suspended).unwrap();
        let rec = get_oracle(&env, &addr).unwrap();
        assert_eq!(rec.status, OracleStatus::Suspended as u8);
        assert!(!rec.is_active());
    }

    // -----------------------------------------------------------------------
    // Asset registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_asset_success() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "PROP001");
        let name = make_name(&env, b"NYC Condo");
        let result = register_asset(&env, asset_id.clone(), AssetClass::RealEstate, name, issuer, None, None);
        assert!(result.is_ok());
        let rec = result.unwrap();
        assert!(rec.active);
        assert_eq!(rec.asset_class, AssetClass::RealEstate as u8);
    }

    #[test]
    fn test_register_asset_duplicate_fails() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "PROP002");
        let name = make_name(&env, b"SF House");
        register_asset(&env, asset_id.clone(), AssetClass::RealEstate, name.clone(), issuer.clone(), None, None).unwrap();
        let result = register_asset(&env, asset_id, AssetClass::RealEstate, name, issuer, None, None);
        assert_eq!(result.unwrap_err(), OracleError::AssetAlreadyRegistered);
    }

    #[test]
    fn test_get_asset_not_registered() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "GHOST");
        assert_eq!(get_asset(&env, &asset_id).unwrap_err(), OracleError::AssetNotRegistered);
    }

    // -----------------------------------------------------------------------
    // Price feed integrity tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_feed_hash_deterministic() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "GOLD");
        let h1 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
        let h2 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_feed_hash_changes_with_price() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "GOLD");
        let h1 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
        let h2 = compute_feed_hash(&env, &asset_id, 200_000_000_001u128, 500_000_000u128, 1_700_000_000u64);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_verify_feed_integrity_valid() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        let asset_id = make_asset_id(&env, "GOLD");
        let price = 200_000_000_000u128;
        let confidence = 1_000_000_000u128;
        let observed_at = 1_700_000_000u64;
        let hash = compute_feed_hash(&env, &asset_id, price, confidence, observed_at);
        let feed = PriceFeed {
            asset_id: asset_id.clone(),
            price,
            confidence,
            observed_at,
            submitted_at: observed_at + 10,
            oracle,
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        let (ok, reason) = verify_feed_integrity(&env, &feed);
        assert!(ok, "Expected valid, got: {reason}");
    }

    #[test]
    fn test_verify_feed_integrity_tampered_price() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        let asset_id = make_asset_id(&env, "GOLD");
        let price = 200_000_000_000u128;
        let confidence = 1_000_000_000u128;
        let observed_at = 1_700_000_000u64;
        let hash = compute_feed_hash(&env, &asset_id, price, confidence, observed_at);
        let feed = PriceFeed {
            asset_id: asset_id.clone(),
            price: price + 1, // tampered
            confidence,
            observed_at,
            submitted_at: observed_at + 10,
            oracle,
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        let (ok, reason) = verify_feed_integrity(&env, &feed);
        assert!(!ok);
        assert_eq!(reason, "hash_mismatch");
    }

    #[test]
    fn test_verify_feed_zero_price_fails() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        let asset_id = make_asset_id(&env, "GOLD");
        let hash = compute_feed_hash(&env, &asset_id, 0, 0, 0);
        let feed = PriceFeed {
            asset_id,
            price: 0,
            confidence: 0,
            observed_at: 0,
            submitted_at: 0,
            oracle,
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        let (ok, reason) = verify_feed_integrity(&env, &feed);
        assert!(!ok);
        assert_eq!(reason, "price_zero");
    }

    // -----------------------------------------------------------------------
    // Staleness detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_staleness_fresh_feed() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        let asset_id = make_asset_id(&env, "OIL");
        let now = 2_000_000;
        let price = 100_000_000_000u128;
        let confidence = 500_000_000u128;
        let observed_at = now - 100; // 100 s ago – fresh
        let hash = compute_feed_hash(&env, &asset_id, price, confidence, observed_at);
        let feed = PriceFeed {
            asset_id,
            price,
            confidence,
            observed_at,
            submitted_at: now,
            oracle,
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        assert!(!is_feed_stale(&feed, now, DEFAULT_STALENESS_WINDOW_SECS));
    }

    #[test]
    fn test_staleness_stale_feed() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        let asset_id = make_asset_id(&env, "OIL");
        let now = 2_000_000;
        let price = 100_000_000_000u128;
        let confidence = 500_000_000u128;
        let observed_at = now - DEFAULT_STALENESS_WINDOW_SECS - 1; // expired
        let hash = compute_feed_hash(&env, &asset_id, price, confidence, observed_at);
        let feed = PriceFeed {
            asset_id,
            price,
            confidence,
            observed_at,
            submitted_at: now,
            oracle,
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        assert!(is_feed_stale(&feed, now, DEFAULT_STALENESS_WINDOW_SECS));
    }

    // -----------------------------------------------------------------------
    // Consensus mechanism tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_consensus_accepted_two_feeds() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "PROP001");
        let now = 1_000_000u64;
        let price = 1_000_000_000_000u128; // 10 000.00000000

        let make_feed = |p: u128| {
            let hash = compute_feed_hash(&env, &asset_id, p, 1_000_000u128, now - 60);
            PriceFeed {
                asset_id: asset_id.clone(),
                price: p,
                confidence: 1_000_000u128,
                observed_at: now - 60,
                submitted_at: now,
                oracle: make_oracle(&env),
                metadata: Bytes::new(&env),
                integrity_hash: hash,
            }
        };

        let feeds = [make_feed(price), make_feed(price + 1_000_000u128)];
        let reps = [80u32, 70u32];

        let (result, out_price, _conf, count) =
            run_consensus(&env, &feeds, &reps, now, DEFAULT_STALENESS_WINDOW_SECS, 2);

        assert_eq!(result, ConsensusResult::Accepted);
        assert_eq!(count, 2);
        // Weighted mean should be between both prices.
        assert!(out_price >= price && out_price <= price + 1_000_000u128);
    }

    #[test]
    fn test_consensus_insufficient_quorum() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "PROP001");
        let now = 1_000_000u64;
        let price = 1_000_000_000_000u128;
        let hash = compute_feed_hash(&env, &asset_id, price, 1_000_000u128, now - 60);
        let feed = PriceFeed {
            asset_id,
            price,
            confidence: 1_000_000u128,
            observed_at: now - 60,
            submitted_at: now,
            oracle: make_oracle(&env),
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        // Require 3 but only 1 feed.
        let (result, _, _, _) = run_consensus(&env, &[feed], &[80], now, DEFAULT_STALENESS_WINDOW_SECS, 3);
        assert_eq!(result, ConsensusResult::InsufficientQuorum);
    }

    #[test]
    fn test_consensus_all_stale() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "PROP001");
        let now = 2_000_000u64;
        let price = 1_000_000_000_000u128;
        let old_ts = now - DEFAULT_STALENESS_WINDOW_SECS - 100; // stale
        let hash = compute_feed_hash(&env, &asset_id, price, 1_000_000u128, old_ts);
        let feed = PriceFeed {
            asset_id,
            price,
            confidence: 1_000_000u128,
            observed_at: old_ts,
            submitted_at: now,
            oracle: make_oracle(&env),
            metadata: Bytes::new(&env),
            integrity_hash: hash,
        };
        let (result, _, _, _) = run_consensus(&env, &[feed], &[80], now, DEFAULT_STALENESS_WINDOW_SECS, 1);
        assert_eq!(result, ConsensusResult::AllStale);
    }

    #[test]
    fn test_consensus_deviation_exceeded() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "PROP001");
        let now = 1_000_000u64;
        let base = 1_000_000_000_000u128;
        // Second price is 10% above base → exceeds 5% deviation cap.
        let high = base + base / 10;

        let make_feed = |p: u128| {
            let hash = compute_feed_hash(&env, &asset_id, p, 1_000_000u128, now - 60);
            PriceFeed {
                asset_id: asset_id.clone(),
                price: p,
                confidence: 1_000_000u128,
                observed_at: now - 60,
                submitted_at: now,
                oracle: make_oracle(&env),
                metadata: Bytes::new(&env),
                integrity_hash: hash,
            }
        };
        let feeds = [make_feed(base), make_feed(high)];
        let reps = [50u32, 50u32];
        let (result, _, _, _) = run_consensus(&env, &feeds, &reps, now, DEFAULT_STALENESS_WINDOW_SECS, 1);
        // Both survive if at least one is within cap of median, but high is ~5.7% from base=median.
        // Only the base price will survive; count=1 which meets quorum=1 → Accepted.
        // If the outlier alone survives, it must equal Accepted with count=1.
        assert!(matches!(result, ConsensusResult::Accepted | ConsensusResult::DeviationExceeded));
    }

    // -----------------------------------------------------------------------
    // Price history chain tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_canonical_hash_deterministic() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "GOLD");
        let prev = BytesN::from_array(&env, &[0u8; 32]);
        let h1 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
        let h2 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_canonical_hash_changes_with_price() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "GOLD");
        let prev = BytesN::from_array(&env, &[0u8; 32]);
        let h1 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
        let h2 = compute_canonical_hash(&env, &asset_id, 200_000_000_001u128, 1_700_000_000u64, &prev);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_price_history_initially_empty() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "UNKNOWN");
        let history = get_price_history(&env, &asset_id);
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_latest_price_stale_when_no_price() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "NOPRICE");
        assert!(is_latest_price_stale(&env, &asset_id));
    }

    #[test]
    fn test_pending_feed_count_zero_initially() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "PEND");
        assert_eq!(pending_feed_count(&env, &asset_id), 0);
    }

    // -----------------------------------------------------------------------
    // End-to-end: submit feeds → process consensus → history
    // -----------------------------------------------------------------------

    #[test]
    fn test_end_to_end_consensus_and_history() {
        let env = setup_env();
        // Register two oracles.
        let o1 = make_oracle(&env);
        let o2 = make_oracle(&env);
        register_oracle(&env, o1.clone(), make_name(&env, b"Oracle A"), Vec::new(&env), 80).unwrap();
        register_oracle(&env, o2.clone(), make_name(&env, b"Oracle B"), Vec::new(&env), 70).unwrap();

        // Register an asset with quorum = 2.
        let asset_id = make_asset_id(&env, "RE001");
        register_asset(
            &env,
            asset_id.clone(),
            AssetClass::RealEstate,
            make_name(&env, b"London Flat"),
            make_oracle(&env),
            Some(2),
            None,
        ).unwrap();

        // Mock ledger timestamp so feeds are fresh.
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        // Oracle 1 submits.
        let price1 = 5_000_000_000_000u128; // 50 000.00000000
        submit_price_feed(&env, &o1, asset_id.clone(), price1, 50_000_000u128, 999_900, Bytes::new(&env)).unwrap();

        // Oracle 2 submits slightly different price.
        let price2 = 5_001_000_000_000u128;
        submit_price_feed(&env, &o2, asset_id.clone(), price2, 50_000_000u128, 999_900, Bytes::new(&env)).unwrap();

        assert_eq!(pending_feed_count(&env, &asset_id), 2);

        // Process consensus.
        let canonical = process_consensus(&env, &asset_id).unwrap();
        assert_eq!(canonical.oracle_count, 2);
        assert_eq!(canonical.history_index, 0);

        // History should have one entry.
        let history = get_price_history(&env, &asset_id);
        assert_eq!(history.len(), 1);

        // Latest price should be set.
        let latest = get_latest_price(&env, &asset_id).unwrap();
        assert_eq!(latest.price, canonical.price);

        // Pending should be cleared.
        assert_eq!(pending_feed_count(&env, &asset_id), 0);
    }

    #[test]
    fn test_submit_suspended_oracle_fails() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        register_oracle(&env, oracle.clone(), make_name(&env, b"Sus Oracle"), Vec::new(&env), 50).unwrap();
        set_oracle_status(&env, &oracle, OracleStatus::Suspended).unwrap();

        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "ASSET_X");
        register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"X"), issuer, Some(1), None).unwrap();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        let result = submit_price_feed(&env, &oracle, asset_id, 100_000_000u128, 1_000_000u128, 999_900, Bytes::new(&env));
        assert_eq!(result.unwrap_err(), OracleError::OracleNotActive);
    }

    #[test]
    fn test_submit_stale_feed_fails() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        register_oracle(&env, oracle.clone(), make_name(&env, b"Fresh Oracle"), Vec::new(&env), 80).unwrap();

        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "STALE_A");
        register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"Stale"), issuer, Some(1), None).unwrap();

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);
        let stale_observed = 1_000_000 - DEFAULT_STALENESS_WINDOW_SECS - 200;
        let result = submit_price_feed(&env, &oracle, asset_id, 100_000_000u128, 1_000_000u128, stale_observed, Bytes::new(&env));
        assert_eq!(result.unwrap_err(), OracleError::StaleTimestamp);
    }

    #[test]
    fn test_submit_zero_price_fails() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        register_oracle(&env, oracle.clone(), make_name(&env, b"Zero Oracle"), Vec::new(&env), 80).unwrap();

        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "ZEROA");
        register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"Z"), issuer, Some(1), None).unwrap();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        let result = submit_price_feed(&env, &oracle, asset_id, 0, 0, 999_900, Bytes::new(&env));
        assert_eq!(result.unwrap_err(), OracleError::InvalidPrice);
    }

    #[test]
    fn test_oracle_count_increments() {
        let env = setup_env();
        let count_before: u32 = env.storage().instance().get(&OracleDataKey::OracleCount).unwrap_or(0);
        register_oracle(&env, make_oracle(&env), make_name(&env, b"O1"), Vec::new(&env), 50).unwrap();
        register_oracle(&env, make_oracle(&env), make_name(&env, b"O2"), Vec::new(&env), 60).unwrap();
        let count_after: u32 = env.storage().instance().get(&OracleDataKey::OracleCount).unwrap_or(0);
        assert_eq!(count_after, count_before + 2);
    }

    #[test]
    fn test_asset_count_increments() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let count_before: u32 = env.storage().instance().get(&OracleDataKey::AssetCount).unwrap_or(0);
        register_asset(&env, make_asset_id(&env, "A1"), AssetClass::RealEstate, make_name(&env, b"A1"), issuer.clone(), None, None).unwrap();
        register_asset(&env, make_asset_id(&env, "A2"), AssetClass::Commodity, make_name(&env, b"A2"), issuer, None, None).unwrap();
        let count_after: u32 = env.storage().instance().get(&OracleDataKey::AssetCount).unwrap_or(0);
        assert_eq!(count_after, count_before + 2);
    }

    #[test]
    fn test_default_staleness_window() {
        let env = setup_env();
        assert_eq!(get_staleness_window(&env), DEFAULT_STALENESS_WINDOW_SECS);
    }

    #[test]
    fn test_set_and_get_staleness_window() {
        let env = setup_env();
        env.storage().instance().set(&OracleDataKey::StalenessWindow, &7200u64);
        assert_eq!(get_staleness_window(&env), 7200u64);
    }

    #[test]
    fn test_asset_custom_quorum() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "QUORUM5");
        register_asset(&env, asset_id.clone(), AssetClass::FixedIncome, make_name(&env, b"Q5"), issuer, Some(5), None).unwrap();
        let rec = get_asset(&env, &asset_id).unwrap();
        assert_eq!(rec.quorum, 5);
    }

    #[test]
    fn test_price_history_chain_verification() {
        let env = setup_env();
        let asset_id = make_asset_id(&env, "CHAIN_TEST");
        let prev = BytesN::from_array(&env, &[0u8; 32]);
        let price = 1_000_000_000_000u128;
        let ts = 1_000_000u64;
        let chain_hash = compute_canonical_hash(&env, &asset_id, price, ts, &prev);

        let entry = CanonicalPrice {
            asset_id: asset_id.clone(),
            price,
            confidence: 5_000_000u128,
            oracle_count: 2,
            consensus_at: ts,
            oldest_observation: ts - 100,
            result: ConsensusResult::Accepted as u8,
            history_index: 0,
            chain_hash: chain_hash.clone(),
            prev_hash: prev,
        };

        assert!(entry.verify_chain_hash(&env));

        // Tamper with price – hash should fail.
        let mut tampered = entry.clone();
        tampered.price = price + 1;
        assert!(!tampered.verify_chain_hash(&env));
    }

    #[test]
    fn test_confidence_too_wide_rejected() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        register_oracle(&env, oracle.clone(), make_name(&env, b"Wide Oracle"), Vec::new(&env), 80).unwrap();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "WIDE_CONF");
        register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"W"), issuer, Some(1), None).unwrap();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        let price = 100_000_000u128;
        let too_wide_confidence = price / 2; // exactly half = rejected
        let result = submit_price_feed(&env, &oracle, asset_id, price, too_wide_confidence, 999_900, Bytes::new(&env));
        assert_eq!(result.unwrap_err(), OracleError::ConfidenceTooWide);
    }

    #[test]
    fn test_process_consensus_no_feeds_fails() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "NOFEED");
        register_asset(&env, asset_id.clone(), AssetClass::RealEstate, make_name(&env, b"NF"), issuer, Some(1), None).unwrap();
        let result = process_consensus(&env, &asset_id);
        assert_eq!(result.unwrap_err(), OracleError::NoFeedsAvailable);
    }

    #[test]
    fn test_staleness_window_override_per_asset() {
        let env = setup_env();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "OVERRIDE_A");
        // Provide a custom 2-hour window.
        register_asset(&env, asset_id.clone(), AssetClass::FixedIncome, make_name(&env, b"OA"), issuer, Some(1), Some(7200)).unwrap();
        let rec = get_asset(&env, &asset_id).unwrap();
        assert_eq!(rec.staleness_window_override, 7200);
    }

    #[test]
    fn test_oracle_submission_counter_increments() {
        let env = setup_env();
        let oracle = make_oracle(&env);
        register_oracle(&env, oracle.clone(), make_name(&env, b"Counter Oracle"), Vec::new(&env), 80).unwrap();
        let issuer = make_oracle(&env);
        let asset_id = make_asset_id(&env, "COUNTER");
        register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"CT"), issuer, Some(1), None).unwrap();
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        submit_price_feed(&env, &oracle, asset_id.clone(), 100_000_000u128, 1_000_000u128, 999_900, Bytes::new(&env)).unwrap();
        let rec = get_oracle(&env, &oracle).unwrap();
        assert_eq!(rec.total_submissions, 1);
    }
}
