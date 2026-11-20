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

use soroban_sdk::xdr::ToXdr;
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

/// Maximum number of entries accepted by [`submit_price_feed_batch`].
pub const MAX_FEED_BATCH: u32 = 16;

/// Maximum number of history entries read by one chain-verification or
/// Merkle call. Keeps a single invocation well inside Soroban's per-call
/// ledger-entry footprint limit.
pub const MAX_VERIFY_SPAN: u32 = 100;

// ── Storage TTL bump policy (issue #811) ────────────────────────────────────

/// Bump persistent keys when their remaining TTL falls below this many ledgers.
pub const TTL_BUMP_THRESHOLD_LEDGERS: u32 = 10_000;

/// TTL extension window (ledgers) for hot records: oracle registrations,
/// asset registrations, and the latest canonical price. Refreshes on every
/// write plus via the permissionless `extend_*_ttl` entry points.
pub const RECORD_TTL_EXTEND_LEDGERS: u32 = 100_000;

/// Archival TTL window (ledgers) for canonical price history entries:
/// back-testing evidence-grade data must outlive hot records.
pub const ARCHIVE_TTL_EXTEND_LEDGERS: u32 = 200_000;

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
    /// The exact same feed (same integrity hash) was already accepted from
    /// this oracle for this asset; replays are rejected without state change.
    DuplicateFeed = 1015,
    /// The feed's `observed_at` is not strictly newer than the last feed
    /// accepted from this oracle for this asset.
    OutOfOrderTimestamp = 1016,
    /// The pending buffer already holds [`MAX_PENDING_FEEDS`] distinct
    /// oracles; run [`process_consensus`] before accepting more.
    PendingBufferFull = 1017,
    /// Stored price history failed hash-chain / index / tip verification.
    ChainIntegrityViolation = 1018,
    /// A batch submission contained no entries.
    EmptyBatch = 1019,
    /// A batch submission exceeded [`MAX_FEED_BATCH`] entries.
    BatchTooLarge = 1020,
    /// A history segment is outside the retained window or longer than
    /// [`MAX_VERIFY_SPAN`].
    InvalidRange = 1021,
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
    pub supported_classes: Vec<u32>, // Vec<AssetClass as u32>
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
    pub status: u32, // OracleStatus as u32
}

impl OracleRecord {
    /// Constructs a new oracle record with default counters.
    pub fn new(
        env: &Env,
        address: Address,
        name: Bytes,
        supported_classes: Vec<u32>,
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
            status: OracleStatus::Active as u32,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == OracleStatus::Active as u32
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
    /// Integrity hash: sha256(xdr(asset_id) || price_le || confidence_le || observed_at_le).
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
    pub result: u32, // ConsensusResult as u32
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
    pub asset_class: u32, // AssetClass as u32
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
    /// Number of archived canonical prices for an asset (absolute, 0-based
    /// next write index).  Key = asset Symbol.
    PriceHistoryCount(Symbol),
    /// One archived canonical price.  Key = (asset Symbol, absolute index).
    /// Per-entry persistent keys let archival readers bump TTL for a single
    /// price instead of paying rent on the whole history.
    PriceHistoryEntry(Symbol, u64),
    /// Contract owner address.
    Owner,
    /// Global staleness window in seconds.
    StalenessWindow,
    /// Global quorum threshold.
    GlobalQuorum,
    /// Total number of consensus rounds executed across all assets.
    TotalRounds,
    /// `(observed_at, integrity_hash)` of the last feed accepted from an
    /// oracle for an asset. Key = (oracle Address, asset Symbol). Enforces
    /// strictly increasing per-oracle observation timestamps across rounds.
    LastObservation(Address, Symbol),
}

/// One entry of a [`submit_price_feed_batch`] call.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeedInput {
    /// Asset identifier.
    pub asset_id: Symbol,
    /// Scaled price (value × PRICE_PRECISION).
    pub price: u128,
    /// Half-width confidence interval, same scale.
    pub confidence: u128,
    /// Off-chain observation timestamp.
    pub observed_at: u64,
    /// Opaque metadata blob.
    pub metadata: Bytes,
}

/// Merkle inclusion proof for one canonical price within a history segment
/// (see [`price_history_merkle_proof`]).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceHistoryProof {
    /// Absolute history index of the proven entry.
    pub history_index: u32,
    /// Leaf position of the entry inside the segment.
    pub position: u32,
    /// Number of leaves (segment entries) the root commits to.
    pub leaf_count: u32,
    /// Sibling hashes from the leaf level up to the root.
    pub siblings: Vec<BytesN<32>>,
}

// ---------------------------------------------------------------------------
// Pure / helper functions (no storage I/O)
// ---------------------------------------------------------------------------

/// Computes the integrity hash for a raw price feed.
///
/// `sha256(xdr(asset_id) || price_le_16 || confidence_le_16 || observed_at_le_8)`
///
/// The asset symbol is encoded with its canonical XDR form, which is
/// deterministic, length-prefixed, and available in `no_std` contract builds.
pub fn compute_feed_hash(
    env: &Env,
    asset_id: &Symbol,
    price: u128,
    confidence: u128,
    observed_at: u64,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&asset_id.clone().to_xdr(env));
    buf.append(&Bytes::from_slice(env, &price.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &confidence.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &observed_at.to_le_bytes()));
    env.crypto().sha256(&buf).to_bytes()
}

/// Computes the chain hash for a canonical price entry.
///
/// `sha256(xdr(asset_id) || price_le_16 || consensus_at_le_8 || prev_hash_32)`
pub fn compute_canonical_hash(
    env: &Env,
    asset_id: &Symbol,
    price: u128,
    consensus_at: u64,
    prev_hash: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&asset_id.clone().to_xdr(env));
    buf.append(&Bytes::from_slice(env, &price.to_le_bytes()));
    buf.append(&Bytes::from_slice(env, &consensus_at.to_le_bytes()));
    buf.append(&prev_hash.clone().into());
    env.crypto().sha256(&buf).to_bytes()
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

    if valid.len() < quorum {
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
        let diff = price.abs_diff(median);
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
    let count = survivors.len();
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
    supported_classes: Vec<u32>,
    reputation_score: u32,
) -> Result<OracleRecord, OracleError> {
    if name.len() > MAX_ORACLE_NAME_LEN {
        return Err(OracleError::OracleNameTooLong);
    }
    if reputation_score > 100 {
        return Err(OracleError::InvalidReputationScore);
    }
    let key = OracleDataKey::Oracle(address.clone());
    if env.storage().persistent().has(&key) {
        return Err(OracleError::AlreadyRegistered);
    }

    let record = OracleRecord::new(env, address, name, supported_classes, reputation_score);
    env.storage().persistent().set(&key, &record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    let count: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::OracleCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OracleDataKey::OracleCount, &(count.saturating_add(1)));

    Ok(record)
}

/// Retrieves an oracle record.  Returns `Err(NotRegistered)` if absent.
pub fn get_oracle(env: &Env, address: &Address) -> Result<OracleRecord, OracleError> {
    env.storage()
        .persistent()
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
        .persistent()
        .get(&key)
        .ok_or(OracleError::NotRegistered)?;
    record.status = status as u32;
    env.storage().persistent().set(&key, &record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
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
    if env.storage().persistent().has(&key) {
        return Err(OracleError::AssetAlreadyRegistered);
    }

    let effective_quorum = quorum.unwrap_or_else(|| get_global_quorum(env));
    let record = AssetRecord {
        asset_id: asset_id.clone(),
        asset_class: asset_class as u32,
        name,
        issuer,
        quorum: effective_quorum,
        staleness_window_override: staleness_window_override.unwrap_or(0),
        active: true,
        registered_at: env.ledger().timestamp(),
        total_updates: 0,
    };
    env.storage().persistent().set(&key, &record);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    let count: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::AssetCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OracleDataKey::AssetCount, &(count.saturating_add(1)));

    Ok(record)
}

/// Retrieves an asset record.
pub fn get_asset(env: &Env, asset_id: &Symbol) -> Result<AssetRecord, OracleError> {
    env.storage()
        .persistent()
        .get(&OracleDataKey::Asset(asset_id.clone()))
        .ok_or(OracleError::AssetNotRegistered)
}

/// Submits a raw price feed for an asset.  Validates oracle status, asset
/// existence, price bounds, timestamp bounds, and integrity hash before
/// appending to the pending feeds buffer.
///
/// # State-machine invariants
///
/// * **INV-ORACLE-DEDUP** – the exact same feed (same integrity hash) from
///   the same oracle for the same asset is rejected with
///   [`OracleError::DuplicateFeed`]; no counter or buffer is touched.
/// * **INV-ORACLE-MONO** – per `(oracle, asset)`, accepted `observed_at`
///   timestamps are strictly increasing across rounds. Older or equal
///   observations are rejected with [`OracleError::OutOfOrderTimestamp`], so
///   a replayed or reordered feed can never be counted in a later round.
/// * **INV-ORACLE-ONE-VOTE** – each oracle has at most one pending feed per
///   asset; a newer observation replaces its previous pending feed, so a
///   single oracle can never satisfy quorum on its own.
/// * **INV-ORACLE-BOUNDED** – the buffer never exceeds
///   [`MAX_PENDING_FEEDS`] entries; a new oracle beyond the cap is rejected
///   with [`OracleError::PendingBufferFull`] instead of silently discarding
///   the feeds already collected.
///
/// All checks run before the first storage write, so a rejected submission
/// leaves state untouched.
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

    // Validate price, confidence and metadata bounds.
    validate_price_feed_bounds(price, confidence, &metadata)?;

    // Timestamp bounds: must not be in the future (allow 60 s drift) and must
    // be within the staleness window.
    let now = env.ledger().timestamp();
    let window = if asset_record.staleness_window_override > 0 {
        asset_record.staleness_window_override
    } else {
        get_staleness_window(env)
    };
    if observed_at > now.saturating_add(60) {
        return Err(OracleError::FutureTimestamp);
    }
    if now.saturating_sub(observed_at) > window {
        return Err(OracleError::StaleTimestamp);
    }

    let integrity_hash = compute_feed_hash(env, &asset_id, price, confidence, observed_at);

    // INV-ORACLE-DEDUP / INV-ORACLE-MONO.
    let obs_key = OracleDataKey::LastObservation(oracle.clone(), asset_id.clone());
    if let Some((last_observed_at, last_hash)) = env
        .storage()
        .persistent()
        .get::<_, (u64, BytesN<32>)>(&obs_key)
    {
        if last_hash == integrity_hash {
            return Err(OracleError::DuplicateFeed);
        }
        if observed_at <= last_observed_at {
            return Err(OracleError::OutOfOrderTimestamp);
        }
    }

    let feed = PriceFeed {
        asset_id: asset_id.clone(),
        price,
        confidence,
        observed_at,
        submitted_at: now,
        oracle: oracle.clone(),
        metadata,
        integrity_hash: integrity_hash.clone(),
    };

    // INV-ORACLE-ONE-VOTE / INV-ORACLE-BOUNDED.
    let pkey = OracleDataKey::PendingFeeds(asset_id.clone());
    let mut pending: Vec<PriceFeed> = env.storage().persistent().get(&pkey).unwrap_or_else(|| Vec::new(env));
    let mut replaced = false;
    for i in 0..pending.len() {
        if pending.get_unchecked(i).oracle == *oracle {
            pending.set(i, feed.clone());
            replaced = true;
            break;
        }
    }
    if !replaced {
        if pending.len() >= MAX_PENDING_FEEDS {
            return Err(OracleError::PendingBufferFull);
        }
        pending.push_back(feed.clone());
    }

    let mut orec = oracle_record;
    orec.total_submissions = orec
        .total_submissions
        .checked_add(1)
        .ok_or(OracleError::Overflow)?;
    orec.last_submission_at = now;

    // ── Commit (no fallible step below this point) ──────────────────────────
    env.storage().persistent().set(&pkey, &pending);
    env.storage()
        .persistent()
        .extend_ttl(&pkey, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    env.storage()
        .persistent()
        .set(&obs_key, &(observed_at, integrity_hash));
    env.storage()
        .persistent()
        .extend_ttl(&obs_key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    let okey = OracleDataKey::Oracle(oracle.clone());
    env.storage().persistent().set(&okey, &orec);
    env.storage()
        .persistent()
        .extend_ttl(&okey, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    Ok(feed)
}

/// Atomically submits up to [`MAX_FEED_BATCH`] feeds from one oracle.
///
/// **INV-ORACLE-BATCH-ATOMIC** – either every entry is accepted, or the
/// batch is rolled back and storage is exactly as it was before the call:
/// every key the batch can touch (the oracle record, and per asset the
/// pending buffer and last-observation marker) is snapshotted up front and
/// restored when any entry fails. The first failing entry's error is
/// returned and a `("rwa_orc", "batch_rb")` event records its index.
///
/// Returns the number of accepted entries on success.
pub fn submit_price_feed_batch(
    env: &Env,
    oracle: &Address,
    entries: &Vec<PriceFeedInput>,
) -> Result<u32, OracleError> {
    if entries.is_empty() {
        return Err(OracleError::EmptyBatch);
    }
    if entries.len() > MAX_FEED_BATCH {
        return Err(OracleError::BatchTooLarge);
    }

    // Snapshot every key the batch may mutate.
    let okey = OracleDataKey::Oracle(oracle.clone());
    let oracle_snapshot: Option<OracleRecord> = env.storage().persistent().get(&okey);
    let mut assets: Vec<Symbol> = Vec::new(env);
    let mut pending_snapshots: Vec<Option<Vec<PriceFeed>>> = Vec::new(env);
    let mut obs_snapshots: Vec<Option<(u64, BytesN<32>)>> = Vec::new(env);
    for entry in entries.iter() {
        if assets.contains(&entry.asset_id) {
            continue;
        }
        pending_snapshots.push_back(
            env.storage()
                .persistent()
                .get(&OracleDataKey::PendingFeeds(entry.asset_id.clone())),
        );
        obs_snapshots.push_back(env.storage().persistent().get(
            &OracleDataKey::LastObservation(oracle.clone(), entry.asset_id.clone()),
        ));
        assets.push_back(entry.asset_id);
    }

    for (index, entry) in entries.iter().enumerate() {
        if let Err(err) = submit_price_feed(
            env,
            oracle,
            entry.asset_id,
            entry.price,
            entry.confidence,
            entry.observed_at,
            entry.metadata,
        ) {
            // ── Mid-batch rollback ──────────────────────────────────────────
            match &oracle_snapshot {
                Some(record) => env.storage().persistent().set(&okey, record),
                None => env.storage().persistent().remove(&okey),
            }
            for i in 0..assets.len() {
                let asset = assets.get_unchecked(i);
                let pkey = OracleDataKey::PendingFeeds(asset.clone());
                match pending_snapshots.get_unchecked(i) {
                    Some(pending) => env.storage().persistent().set(&pkey, &pending),
                    None => env.storage().persistent().remove(&pkey),
                }
                let obs_key = OracleDataKey::LastObservation(oracle.clone(), asset);
                match obs_snapshots.get_unchecked(i) {
                    Some(obs) => env.storage().persistent().set(&obs_key, &obs),
                    None => env.storage().persistent().remove(&obs_key),
                }
            }
            env.events().publish(
                (Symbol::new(env, "rwa_orc"), Symbol::new(env, "batch_rb")),
                (oracle.clone(), index as u32, err as u32),
            );
            return Err(err);
        }
    }

    Ok(entries.len())
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
    let pending: Vec<PriceFeed> = env.storage().persistent().get(&pkey).unwrap_or_else(|| Vec::new(env));

    if pending.is_empty() {
        return Err(OracleError::NoFeedsAvailable);
    }

    // Build slices for the pure consensus function.
    //
    // INV-ORACLE-LIVE: only feeds from oracles that are *still* active at
    // consensus time are counted. An oracle suspended or revoked after it
    // submitted, but before the round closed, cannot influence the price.
    let mut feeds_slice: soroban_sdk::Vec<PriceFeed> = Vec::new(env);
    let mut rep_slice: soroban_sdk::Vec<u32> = Vec::new(env);

    for feed in pending.iter() {
        if let Ok(record) = get_oracle(env, &feed.oracle) {
            if record.is_active() {
                feeds_slice.push_back(feed);
                rep_slice.push_back(record.reputation_score);
            }
        }
    }
    let n = feeds_slice.len() as usize;

    // Build native slices for run_consensus.
    let feeds_vec: Vec<PriceFeed> = feeds_slice;
    let reps_vec: Vec<u32> = rep_slice;

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
            if let Some(mut orec) = env.storage().persistent().get::<_, OracleRecord>(&okey) {
                orec.total_rejections = orec.total_rejections.saturating_add(1);
                env.storage().persistent().set(&okey, &orec);
                env.storage()
                    .persistent()
                    .extend_ttl(&okey, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
            }
        }
        // Clear pending feeds so the next round starts fresh.
        let fresh = Vec::<PriceFeed>::new(env);
        env.storage().persistent().set(&pkey, &fresh);
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

    // Build canonical price and chain it against the previous archival entry.
    let count_key = OracleDataKey::PriceHistoryCount(asset_id.clone());
    let history_count: u32 = env
        .storage()
        .persistent()
        .get(&count_key)
        .unwrap_or(0);

    let history_index_u64: u64 = history_count as u64;
    let prev_hash: BytesN<32> = if history_count == 0 {
        BytesN::from_array(env, &[0u8; 32])
    } else {
        env.storage()
            .persistent()
            .get::<_, CanonicalPrice>(&OracleDataKey::PriceHistoryEntry(
                asset_id.clone(),
                (history_count - 1) as u64,
            ))
            .map(|prev| prev.chain_hash)
            .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]))
    };
    let chain_hash = compute_canonical_hash(env, asset_id, price, now, &prev_hash);

    // Pre-compute every fallible counter update before the first write so an
    // overflow can never leave a half-committed round (history appended but
    // counters / latest price stale).
    let next_history_count = history_count.checked_add(1).ok_or(OracleError::Overflow)?;
    let next_total_updates = asset_record
        .total_updates
        .checked_add(1)
        .ok_or(OracleError::Overflow)?;
    let rounds: u32 = env
        .storage()
        .instance()
        .get(&OracleDataKey::TotalRounds)
        .unwrap_or(0);
    let next_rounds = rounds.checked_add(1).ok_or(OracleError::Overflow)?;

    let canonical = CanonicalPrice {
        asset_id: asset_id.clone(),
        price,
        confidence,
        oracle_count: count,
        consensus_at: now,
        oldest_observation: oldest,
        result: ConsensusResult::Accepted as u32,
        history_index: history_count,
        chain_hash,
        prev_hash,
    };

    // Append to per-entry persistent archival storage, respecting the rolling
    // window cap: once the cap is reached the oldest entry's key is retired so
    // per-asset rent stays bounded instead of growing forever.
    if history_count >= MAX_PRICE_HISTORY {
        env.storage().persistent().remove(&OracleDataKey::PriceHistoryEntry(
            asset_id.clone(),
            (history_count - MAX_PRICE_HISTORY) as u64,
        ));
    }

    let entry_key = OracleDataKey::PriceHistoryEntry(asset_id.clone(), history_index_u64);
    env.storage().persistent().set(&entry_key, &canonical);
    env.storage()
        .persistent()
        .extend_ttl(&entry_key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);

    env.storage()
        .persistent()
        .set(&count_key, &next_history_count);

    // Update latest price (hot read key, archival window).
    let latest_key = OracleDataKey::LatestPrice(asset_id.clone());
    env.storage().persistent().set(&latest_key, &canonical);
    env.storage()
        .persistent()
        .extend_ttl(&latest_key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    // Update asset total_updates counter.
    let akey = OracleDataKey::Asset(asset_id.clone());
    let mut arec = asset_record;
    arec.total_updates = next_total_updates;
    env.storage().persistent().set(&akey, &arec);
    env.storage()
        .persistent()
        .extend_ttl(&akey, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);

    // Increment global round counter.
    env.storage()
        .instance()
        .set(&OracleDataKey::TotalRounds, &next_rounds);

    // Clear pending feeds so the next round starts fresh.
    let fresh = Vec::<PriceFeed>::new(env);
    env.storage().persistent().set(&pkey, &fresh);

    Ok(canonical)
}

/// Returns the latest canonical price for an asset, if one has been published.
pub fn get_latest_price(env: &Env, asset_id: &Symbol) -> Option<CanonicalPrice> {
    env.storage()
        .persistent()
        .get(&OracleDataKey::LatestPrice(asset_id.clone()))
}

/// Retrieves a single archived canonical price by absolute history index.
pub fn get_canonical_price(env: &Env, asset_id: &Symbol, index: u64) -> Option<CanonicalPrice> {
    env.storage()
        .persistent()
        .get(&OracleDataKey::PriceHistoryEntry(asset_id.clone(), index))
}

/// Returns the price history window for an asset (up to MAX_PRICE_HISTORY
/// most recent canonical prices, oldest first).
pub fn get_price_history(env: &Env, asset_id: &Symbol) -> Vec<CanonicalPrice> {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&OracleDataKey::PriceHistoryCount(asset_id.clone()))
        .unwrap_or(0);

    let mut history = Vec::new(env);
    let start = count.saturating_sub(MAX_PRICE_HISTORY);
    for idx in (start as u64)..(count as u64) {
        if let Some(entry) = env
            .storage()
            .persistent()
            .get::<_, CanonicalPrice>(&OracleDataKey::PriceHistoryEntry(asset_id.clone(), idx))
        {
            history.push_back(entry);
        }
    }
    history
}

/// Absolute index range `[start, count)` of the retained history window.
fn retained_window(env: &Env, asset_id: &Symbol) -> (u32, u32) {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&OracleDataKey::PriceHistoryCount(asset_id.clone()))
        .unwrap_or(0);
    (count.saturating_sub(MAX_PRICE_HISTORY), count)
}

/// Validates a `[start, start + len)` history segment against the retained
/// window and [`MAX_VERIFY_SPAN`], returning its exclusive end.
fn checked_segment(env: &Env, asset_id: &Symbol, start: u32, len: u32) -> Result<u32, OracleError> {
    let (window_start, count) = retained_window(env, asset_id);
    let end = start.checked_add(len).ok_or(OracleError::InvalidRange)?;
    if len > MAX_VERIFY_SPAN || start < window_start || end > count {
        return Err(OracleError::InvalidRange);
    }
    Ok(end)
}

/// Loads the entries of a validated segment, failing on any missing entry.
fn load_segment(
    env: &Env,
    asset_id: &Symbol,
    start: u32,
    end: u32,
) -> Result<Vec<CanonicalPrice>, OracleError> {
    let mut entries = Vec::new(env);
    for idx in start..end {
        entries.push_back(
            get_canonical_price(env, asset_id, idx as u64)
                .ok_or(OracleError::ChainIntegrityViolation)?,
        );
    }
    Ok(entries)
}

/// Verifies the hash-chain integrity of the history segment
/// `[start, start + len)` of an asset.
///
/// **INV-ORACLE-CHAIN** – for every entry `i` in the segment:
/// * the entry exists and `history_index == i` (contiguous indices),
/// * `chain_hash` recomputes from its own fields ([`CanonicalPrice::verify_chain_hash`]),
/// * `prev_hash` equals entry `i - 1`'s `chain_hash` — including the entry
///   just before the segment when it is still retained — or 32 zero bytes
///   for genesis (`i == 0`); only the first entry of a rolled-over window is
///   unanchored, because its predecessor was evicted,
/// * `consensus_at` is non-decreasing,
///
/// and, when the segment reaches the newest entry, the latest-price pointer
/// equals it. Segments are capped at [`MAX_VERIFY_SPAN`] entries so a single
/// call stays within Soroban's per-invocation ledger footprint; verify a
/// long history by walking consecutive segments.
///
/// Returns the number of verified entries.
pub fn verify_price_history_chain(
    env: &Env,
    asset_id: &Symbol,
    start: u32,
    len: u32,
) -> Result<u32, OracleError> {
    let end = checked_segment(env, asset_id, start, len)?;
    let (window_start, count) = retained_window(env, asset_id);
    let zero = BytesN::from_array(env, &[0u8; 32]);

    let mut prev: Option<CanonicalPrice> = if start > window_start {
        Some(
            get_canonical_price(env, asset_id, (start - 1) as u64)
                .ok_or(OracleError::ChainIntegrityViolation)?,
        )
    } else {
        None
    };
    let mut last: Option<CanonicalPrice> = None;
    for (offset, cp) in load_segment(env, asset_id, start, end)?.iter().enumerate() {
        let idx = start + offset as u32;
        if cp.history_index != idx || cp.asset_id != *asset_id || !cp.verify_chain_hash(env) {
            return Err(OracleError::ChainIntegrityViolation);
        }
        let linked = match &prev {
            None => idx != 0 || cp.prev_hash == zero,
            Some(p) => cp.prev_hash == p.chain_hash && cp.consensus_at >= p.consensus_at,
        };
        if !linked {
            return Err(OracleError::ChainIntegrityViolation);
        }
        prev = Some(cp.clone());
        last = Some(cp);
    }
    if end == count && last.is_some() && last != get_latest_price(env, asset_id) {
        return Err(OracleError::ChainIntegrityViolation);
    }
    Ok(end - start)
}

/// Verifies the newest `min(retained, MAX_VERIFY_SPAN)` history entries,
/// including the latest-price tip. Returns the number of verified entries.
pub fn verify_recent_price_history(env: &Env, asset_id: &Symbol) -> Result<u32, OracleError> {
    let (window_start, count) = retained_window(env, asset_id);
    let len = (count - window_start).min(MAX_VERIFY_SPAN);
    verify_price_history_chain(env, asset_id, count - len, len)
}

/// Merkle root over the chain hashes of history segment `[start, start + len)`.
pub fn price_history_merkle_root(
    env: &Env,
    asset_id: &Symbol,
    start: u32,
    len: u32,
) -> Result<BytesN<32>, OracleError> {
    let end = checked_segment(env, asset_id, start, len)?;
    let mut leaves = Vec::new(env);
    for cp in load_segment(env, asset_id, start, end)?.iter() {
        leaves.push_back(cp.chain_hash);
    }
    Ok(crate::merkle::merkle_root(env, &leaves))
}

/// Merkle inclusion proof for the entry at absolute `history_index` within
/// segment `[start, start + len)`.
pub fn price_history_merkle_proof(
    env: &Env,
    asset_id: &Symbol,
    start: u32,
    len: u32,
    history_index: u32,
) -> Result<PriceHistoryProof, OracleError> {
    let end = checked_segment(env, asset_id, start, len)?;
    if history_index < start || history_index >= end {
        return Err(OracleError::InvalidRange);
    }
    let mut leaves = Vec::new(env);
    for cp in load_segment(env, asset_id, start, end)?.iter() {
        leaves.push_back(cp.chain_hash);
    }
    let position = history_index - start;
    let siblings = crate::merkle::merkle_proof(env, &leaves, position)
        .ok_or(OracleError::InvalidRange)?;
    Ok(PriceHistoryProof {
        history_index,
        position,
        leaf_count: len,
        siblings,
    })
}

/// Verifies that `entry` is committed to by `root` via `proof`. The entry's
/// own chain hash must also recompute, so a forged entry carrying a valid
/// leaf hash is rejected.
pub fn verify_price_history_proof(
    env: &Env,
    entry: &CanonicalPrice,
    proof: &PriceHistoryProof,
    root: &BytesN<32>,
) -> bool {
    entry.history_index == proof.history_index
        && entry.verify_chain_hash(env)
        && crate::merkle::verify_merkle_proof(
            env,
            &entry.chain_hash,
            proof.position,
            proof.leaf_count,
            &proof.siblings,
            root,
        )
}

/// Permissionless TTL extension for an oracle registration record. Keeps the
/// record live for monitoring without paying for a full re-registration.
pub fn extend_oracle_ttl(env: &Env, oracle: &Address) {
    let key = OracleDataKey::Oracle(oracle.clone());
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
}

/// Permissionless TTL extension for an asset registration record.
pub fn extend_asset_ttl(env: &Env, asset_id: &Symbol) {
    let key = OracleDataKey::Asset(asset_id.clone());
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
}

/// Permissionless archival TTL extension for a canonical price history entry.
/// `back_index` counts back from the newest entry (0 = newest) so archival
/// readers can keep consensus evidence live without bumping the whole window.
pub fn extend_canonical_price_ttl(env: &Env, asset_id: &Symbol, back_index: u32) {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&OracleDataKey::PriceHistoryCount(asset_id.clone()))
        .unwrap_or(0);
    let absolute = count.saturating_sub(back_index.saturating_add(1)) as u64;
    let key = OracleDataKey::PriceHistoryEntry(asset_id.clone(), absolute);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);
}

/// Permissionless TTL extension for the latest canonical price of an asset
/// (the hot read cached by integrators).
pub fn extend_latest_price_ttl(env: &Env, asset_id: &Symbol) {
    let key = OracleDataKey::LatestPrice(asset_id.clone());
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, RECORD_TTL_EXTEND_LEDGERS);
}

/// Hard cap on the number of price-history entries returned in a single page.
pub const MAX_PRICE_PAGE_LIMIT: u32 = 100;

/// Maximum byte length for a price feed's opaque metadata blob (#809).
pub const MAX_FEED_METADATA_LEN: u32 = 512;

/// Maximum number of asset classes an oracle may register at once (#809).
pub const MAX_SUPPORTED_CLASSES_LEN: u32 = 16;

/// Maximum allowed scaled price to prevent overflow during weighted-mean consensus (`u128::MAX / 1_000`) (#809).
pub const MAX_SAFE_ORACLE_PRICE: u128 = u128::MAX / 1_000;

/// Validate deterministic bounds and overflow safety on raw price feed inputs (#809).
pub fn validate_price_feed_bounds(
    price: u128,
    confidence: u128,
    metadata: &Bytes,
) -> Result<(), OracleError> {
    if price == 0 || price > MAX_SAFE_ORACLE_PRICE {
        return Err(OracleError::InvalidPrice);
    }
    let max_conf = price.checked_div(2).ok_or(OracleError::Overflow)?;
    if confidence >= max_conf {
        return Err(OracleError::ConfidenceTooWide);
    }
    if metadata.len() > MAX_FEED_METADATA_LEN {
        return Err(OracleError::OracleNameTooLong);
    }
    Ok(())
}

/// Compute reputation-weighted mean price and mean confidence using strictly
/// checked `checked_mul`, `checked_add`, and `checked_div` arithmetic (#809).
pub fn checked_weighted_mean_price(
    entries: &[(u128, u128, u32)],
) -> Result<(u128, u128), OracleError> {
    if entries.is_empty() {
        return Err(OracleError::NoFeedsAvailable);
    }
    let mut weight_sum: u128 = 0;
    let mut price_sum: u128 = 0;
    let mut conf_sum: u128 = 0;

    for &(price, conf, rep) in entries.iter() {
        if price == 0 || price > MAX_SAFE_ORACLE_PRICE {
            return Err(OracleError::InvalidPrice);
        }
        let w = rep as u128;
        let weighted = price.checked_mul(w).ok_or(OracleError::Overflow)?;
        weight_sum = weight_sum.checked_add(w).ok_or(OracleError::Overflow)?;
        price_sum = price_sum.checked_add(weighted).ok_or(OracleError::Overflow)?;
        conf_sum = conf_sum.checked_add(conf).ok_or(OracleError::Overflow)?;
    }
    if weight_sum == 0 {
        return Err(OracleError::InvalidPrice);
    }
    let weighted_price = price_sum.checked_div(weight_sum).ok_or(OracleError::Overflow)?;
    let mean_conf = conf_sum
        .checked_div(entries.len() as u128)
        .ok_or(OracleError::Overflow)?;
    Ok((weighted_price, mean_conf))
}

/// Cursor token for deterministic cursor-based pagination over price history.
///
/// The cursor encodes the composite sort key `(consensus_at, history_index)`
/// of the last item returned on the previous page. Ordering on this composite
/// key is stable across ledger upgrades and archival restores, so traversal is
/// reproducible.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceHistoryCursor {
    /// Consensus timestamp of the last returned item.
    pub consensus_at: u64,
    /// Tie-breaking history index of the last returned item.
    pub history_index: u32,
}

/// Requested page size, capped at [`MAX_PRICE_PAGE_LIMIT`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceHistoryLimit {
    /// Requested number of items.
    pub limit: u32,
}

impl PriceHistoryLimit {
    /// Construct a page limit, rejecting zero and clamping to the hard cap.
    pub fn new(limit: u32) -> Result<Self, &'static str> {
        if limit == 0 {
            return Err("Page limit must be greater than zero");
        }
        Ok(Self {
            limit: if limit > MAX_PRICE_PAGE_LIMIT {
                MAX_PRICE_PAGE_LIMIT
            } else {
                limit
            },
        })
    }

    /// Effective (capped) limit.
    pub fn effective(&self) -> u32 {
        if self.limit > MAX_PRICE_PAGE_LIMIT {
            MAX_PRICE_PAGE_LIMIT
        } else {
            self.limit
        }
    }
}

/// Multi-predicate filter for price history queries.
///
/// All predicates are optional; `None` means "no constraint". When several
/// predicates are supplied they are combined with logical AND.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceHistoryFilter {
    /// Inclusive lower bound on consensus timestamp.
    pub min_consensus_at: Option<u64>,
    /// Inclusive upper bound on consensus timestamp.
    pub max_consensus_at: Option<u64>,
    /// Inclusive lower bound on price (scaled by PRICE_PRECISION).
    pub min_price: Option<u128>,
    /// Inclusive upper bound on price (scaled by PRICE_PRECISION).
    pub max_price: Option<u128>,
    /// Minimum number of oracle feeds that contributed to the consensus price.
    pub min_oracle_count: Option<u32>,
}

impl PriceHistoryFilter {
    /// Empty filter matching every item.
    pub fn none() -> Self {
        Self {
            min_consensus_at: None,
            max_consensus_at: None,
            min_price: None,
            max_price: None,
            min_oracle_count: None,
        }
    }

    /// Validate the filter, returning a typed error for invalid ranges.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let (Some(min), Some(max)) = (self.min_consensus_at, self.max_consensus_at) {
            if min > max {
                return Err("Invalid consensus timestamp range: min exceeds max");
            }
        }
        if let (Some(min), Some(max)) = (self.min_price, self.max_price) {
            if min > max {
                return Err("Invalid price range: min exceeds max");
            }
        }
        Ok(())
    }

    /// Returns true when `cp` satisfies all predicates.
    pub fn matches(&self, cp: &CanonicalPrice) -> bool {
        if let Some(min) = self.min_consensus_at {
            if cp.consensus_at < min {
                return false;
            }
        }
        if let Some(max) = self.max_consensus_at {
            if cp.consensus_at > max {
                return false;
            }
        }
        if let Some(min) = self.min_price {
            if cp.price < min {
                return false;
            }
        }
        if let Some(max) = self.max_price {
            if cp.price > max {
                return false;
            }
        }
        if let Some(min) = self.min_oracle_count {
            if cp.oracle_count < min {
                return false;
            }
        }
        true
    }
}

/// A page of canonical price results plus the cursor for the next page.
///
/// Plain Rust return type (not a `#[contracttype]`): it is never stored, and
/// `Option<PriceHistoryCursor>` has no `ScVal` conversion under the SDK 28
/// test utilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPricePage {
    /// Items in this page, deterministically ordered.
    pub items: Vec<CanonicalPrice>,
    /// Cursor to pass to the next call, or `None` when exhausted.
    pub next_cursor: Option<PriceHistoryCursor>,
}

/// Compare two composite sort keys `(consensus_at, history_index)`.
fn compare_price_keys(
    a: &(u64, u32),
    b: &(u64, u32),
) -> core::cmp::Ordering {
    match a.0.cmp(&b.0) {
        core::cmp::Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    }
}

/// Returns true when `key` sorts strictly after `cursor`.
fn price_key_after_cursor(key: &(u64, u32), cursor: &PriceHistoryCursor) -> bool {
    compare_price_keys(key, &(cursor.consensus_at, cursor.history_index))
        == core::cmp::Ordering::Greater
}

/// Build the next cursor from the last item of a full page.
fn price_next_cursor_for(key: &(u64, u32), page_len: u32, limit: u32) -> Option<PriceHistoryCursor> {
    if page_len < limit {
        None
    } else {
        Some(PriceHistoryCursor {
            consensus_at: key.0,
            history_index: key.1,
        })
    }
}

/// Deterministic in-place insertion sort over composite price keys.
fn sort_price_keys(keys: &mut Vec<(u64, u32)>) {
    let len = keys.len();
    if len < 2 {
        return;
    }
    let mut i = 1u32;
    while i < len {
        let mut j = i;
        while j > 0 {
            let prev = keys.get(j - 1).unwrap();
            let cur = keys.get(j).unwrap();
            if compare_price_keys(&prev, &cur) == core::cmp::Ordering::Greater {
                keys.set(j - 1, cur);
                keys.set(j, prev);
                j -= 1;
            } else {
                break;
            }
        }
        i += 1;
    }
}

/// Paginated, filtered query over an asset's price history.
///
/// Results are ordered by the composite key `(consensus_at, history_index)`
/// which is stable across ledger upgrades and archival restores. Returns the
/// page items and a `next_cursor` token, or `None` when exhausted.
pub fn get_price_history_paginated(
    env: &Env,
    asset_id: &Symbol,
    filter: &PriceHistoryFilter,
    cursor: &Option<PriceHistoryCursor>,
    limit: &PriceHistoryLimit,
) -> Result<CanonicalPricePage, &'static str> {
    filter.validate()?;
    let effective = limit.effective();

    let history: Vec<CanonicalPrice> = get_price_history(env, asset_id);

    // Collect matching keys and sort deterministically.
    let mut keys: Vec<(u64, u32)> = Vec::new(env);
    for (i, cp) in history.iter().enumerate() {
        if !filter.matches(&cp) {
            continue;
        }
        keys.push_back((cp.consensus_at, i as u32));
    }
    sort_price_keys(&mut keys);

    let mut items = Vec::new(env);
    let mut last_key: Option<(u64, u32)> = None;
    let mut page_len = 0u32;

    for key in keys.iter() {
        if let Some(c) = cursor {
            if !price_key_after_cursor(&key, c) {
                continue;
            }
        }
        if page_len >= effective {
            break;
        }
        if let Some(cp) = history.get(key.1) {
            items.push_back(cp.clone());
            last_key = Some(key);
            page_len += 1;
        }
    }

    let next_cursor = match last_key {
        Some(k) => price_next_cursor_for(&k, page_len, effective),
        None => None,
    };

    Ok(CanonicalPricePage { items, next_cursor })
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
    let pending: Vec<PriceFeed> = env.storage().persistent().get(&pkey).unwrap_or_else(|| Vec::new(env));
    pending.len()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl, Env};

    /// Empty host contract providing a storage context for module calls.
    #[contract]
    struct OracleHarness;

    #[contractimpl]
    impl OracleHarness {}

    fn setup_env() -> (Env, Address) {
        let env = Env::default();
        let contract = env.register(OracleHarness, ());
        (env, contract)
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
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let sym = AssetClass::RealEstate.as_symbol(&env);
            assert_eq!(sym, Symbol::new(&env, "RealEstate"));
        });
    }

    // -----------------------------------------------------------------------
    // Oracle registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_oracle_success() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            let name = make_name(&env, b"Oracle One");
            let classes = Vec::from_slice(&env, &[AssetClass::RealEstate as u32]);
            let result = register_oracle(&env, addr.clone(), name, classes, 80);
            assert!(result.is_ok());
            let rec = result.unwrap();
            assert_eq!(rec.reputation_score, 80);
            assert!(rec.is_active());
        });
    }

    #[test]
    fn test_register_oracle_duplicate_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            let name = make_name(&env, b"Dup Oracle");
            let classes = Vec::from_slice(&env, &[AssetClass::Commodity as u32]);
            register_oracle(&env, addr.clone(), name.clone(), classes.clone(), 50).unwrap();
            let result = register_oracle(&env, addr.clone(), name, classes, 50);
            assert_eq!(result.unwrap_err(), OracleError::AlreadyRegistered);
        });
    }

    #[test]
    fn test_register_oracle_invalid_reputation() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            let name = make_name(&env, b"Bad Oracle");
            let classes = Vec::new(&env);
            let result = register_oracle(&env, addr, name, classes, 101);
            assert_eq!(result.unwrap_err(), OracleError::InvalidReputationScore);
        });
    }

    #[test]
    fn test_register_oracle_name_too_long() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            let long_name = make_name(&env, &[b'A'; 65]);
            let classes = Vec::new(&env);
            let result = register_oracle(&env, addr, long_name, classes, 50);
            assert_eq!(result.unwrap_err(), OracleError::OracleNameTooLong);
        });
    }

    #[test]
    fn test_get_oracle_not_registered() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            assert_eq!(get_oracle(&env, &addr).unwrap_err(), OracleError::NotRegistered);
        });
    }

    #[test]
    fn test_oracle_status_update() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let addr = make_oracle(&env);
            let name = make_name(&env, b"Status Oracle");
            let classes = Vec::new(&env);
            register_oracle(&env, addr.clone(), name, classes, 60).unwrap();
            set_oracle_status(&env, &addr, OracleStatus::Suspended).unwrap();
            let rec = get_oracle(&env, &addr).unwrap();
            assert_eq!(rec.status, OracleStatus::Suspended as u32);
            assert!(!rec.is_active());
        });
    }

    // -----------------------------------------------------------------------
    // Asset registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_asset_success() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "PROP001");
            let name = make_name(&env, b"NYC Condo");
            let result = register_asset(&env, asset_id.clone(), AssetClass::RealEstate, name, issuer, None, None);
            assert!(result.is_ok());
            let rec = result.unwrap();
            assert!(rec.active);
            assert_eq!(rec.asset_class, AssetClass::RealEstate as u32);
        });
    }

    #[test]
    fn test_register_asset_duplicate_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "PROP002");
            let name = make_name(&env, b"SF House");
            register_asset(&env, asset_id.clone(), AssetClass::RealEstate, name.clone(), issuer.clone(), None, None).unwrap();
            let result = register_asset(&env, asset_id, AssetClass::RealEstate, name, issuer, None, None);
            assert_eq!(result.unwrap_err(), OracleError::AssetAlreadyRegistered);
        });
    }

    #[test]
    fn test_get_asset_not_registered() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "GHOST");
            assert_eq!(get_asset(&env, &asset_id).unwrap_err(), OracleError::AssetNotRegistered);
        });
    }

    // -----------------------------------------------------------------------
    // Price feed integrity tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_feed_hash_deterministic() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "GOLD");
            let h1 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
            let h2 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
            assert_eq!(h1, h2);
        });
    }

    #[test]
    fn test_compute_feed_hash_changes_with_price() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "GOLD");
            let h1 = compute_feed_hash(&env, &asset_id, 200_000_000_000u128, 500_000_000u128, 1_700_000_000u64);
            let h2 = compute_feed_hash(&env, &asset_id, 200_000_000_001u128, 500_000_000u128, 1_700_000_000u64);
            assert_ne!(h1, h2);
        });
    }

    #[test]
    fn test_verify_feed_integrity_valid() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_verify_feed_integrity_tampered_price() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_verify_feed_zero_price_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    // -----------------------------------------------------------------------
    // Staleness detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_staleness_fresh_feed() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_staleness_stale_feed() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    // -----------------------------------------------------------------------
    // Consensus mechanism tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_consensus_accepted_two_feeds() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_consensus_insufficient_quorum() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_consensus_all_stale() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_consensus_deviation_exceeded() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    // -----------------------------------------------------------------------
    // Price history chain tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_canonical_hash_deterministic() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "GOLD");
            let prev = BytesN::from_array(&env, &[0u8; 32]);
            let h1 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
            let h2 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
            assert_eq!(h1, h2);
        });
    }

    #[test]
    fn test_canonical_hash_changes_with_price() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "GOLD");
            let prev = BytesN::from_array(&env, &[0u8; 32]);
            let h1 = compute_canonical_hash(&env, &asset_id, 200_000_000_000u128, 1_700_000_000u64, &prev);
            let h2 = compute_canonical_hash(&env, &asset_id, 200_000_000_001u128, 1_700_000_000u64, &prev);
            assert_ne!(h1, h2);
        });
    }

    #[test]
    fn test_price_history_initially_empty() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "UNKNOWN");
            let history = get_price_history(&env, &asset_id);
            assert_eq!(history.len(), 0);
        });
    }

    #[test]
    fn test_latest_price_stale_when_no_price() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "NOPRICE");
            assert!(is_latest_price_stale(&env, &asset_id));
        });
    }

    #[test]
    fn test_pending_feed_count_zero_initially() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let asset_id = make_asset_id(&env, "PEND");
            assert_eq!(pending_feed_count(&env, &asset_id), 0);
        });
    }

    // -----------------------------------------------------------------------
    // End-to-end: submit feeds → process consensus → history
    // -----------------------------------------------------------------------

    #[test]
    fn test_end_to_end_consensus_and_history() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_submit_suspended_oracle_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let oracle = make_oracle(&env);
            register_oracle(&env, oracle.clone(), make_name(&env, b"Sus Oracle"), Vec::new(&env), 50).unwrap();
            set_oracle_status(&env, &oracle, OracleStatus::Suspended).unwrap();

            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "ASSET_X");
            register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"X"), issuer, Some(1), None).unwrap();
            env.ledger().with_mut(|l| l.timestamp = 1_000_000);

            let result = submit_price_feed(&env, &oracle, asset_id, 100_000_000u128, 1_000_000u128, 999_900, Bytes::new(&env));
            assert_eq!(result.unwrap_err(), OracleError::OracleNotActive);
        });
    }

    #[test]
    fn test_submit_stale_feed_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let oracle = make_oracle(&env);
            register_oracle(&env, oracle.clone(), make_name(&env, b"Fresh Oracle"), Vec::new(&env), 80).unwrap();

            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "STALE_A");
            register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"Stale"), issuer, Some(1), None).unwrap();

            env.ledger().with_mut(|l| l.timestamp = 1_000_000);
            let stale_observed = 1_000_000 - DEFAULT_STALENESS_WINDOW_SECS - 200;
            let result = submit_price_feed(&env, &oracle, asset_id, 100_000_000u128, 1_000_000u128, stale_observed, Bytes::new(&env));
            assert_eq!(result.unwrap_err(), OracleError::StaleTimestamp);
        });
    }

    #[test]
    fn test_submit_zero_price_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let oracle = make_oracle(&env);
            register_oracle(&env, oracle.clone(), make_name(&env, b"Zero Oracle"), Vec::new(&env), 80).unwrap();

            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "ZEROA");
            register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"Z"), issuer, Some(1), None).unwrap();
            env.ledger().with_mut(|l| l.timestamp = 1_000_000);

            let result = submit_price_feed(&env, &oracle, asset_id, 0, 0, 999_900, Bytes::new(&env));
            assert_eq!(result.unwrap_err(), OracleError::InvalidPrice);
        });
    }

    #[test]
    fn test_oracle_count_increments() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let count_before: u32 = env.storage().instance().get(&OracleDataKey::OracleCount).unwrap_or(0);
            register_oracle(&env, make_oracle(&env), make_name(&env, b"O1"), Vec::new(&env), 50).unwrap();
            register_oracle(&env, make_oracle(&env), make_name(&env, b"O2"), Vec::new(&env), 60).unwrap();
            let count_after: u32 = env.storage().instance().get(&OracleDataKey::OracleCount).unwrap_or(0);
            assert_eq!(count_after, count_before + 2);
        });
    }

    #[test]
    fn test_asset_count_increments() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let count_before: u32 = env.storage().instance().get(&OracleDataKey::AssetCount).unwrap_or(0);
            register_asset(&env, make_asset_id(&env, "A1"), AssetClass::RealEstate, make_name(&env, b"A1"), issuer.clone(), None, None).unwrap();
            register_asset(&env, make_asset_id(&env, "A2"), AssetClass::Commodity, make_name(&env, b"A2"), issuer, None, None).unwrap();
            let count_after: u32 = env.storage().instance().get(&OracleDataKey::AssetCount).unwrap_or(0);
            assert_eq!(count_after, count_before + 2);
        });
    }

    #[test]
    fn test_default_staleness_window() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            assert_eq!(get_staleness_window(&env), DEFAULT_STALENESS_WINDOW_SECS);
        });
    }

    #[test]
    fn test_set_and_get_staleness_window() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            env.storage().instance().set(&OracleDataKey::StalenessWindow, &7200u64);
            assert_eq!(get_staleness_window(&env), 7200u64);
        });
    }

    #[test]
    fn test_asset_custom_quorum() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "QUORUM5");
            register_asset(&env, asset_id.clone(), AssetClass::FixedIncome, make_name(&env, b"Q5"), issuer, Some(5), None).unwrap();
            let rec = get_asset(&env, &asset_id).unwrap();
            assert_eq!(rec.quorum, 5);
        });
    }

    #[test]
    fn test_price_history_chain_verification() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
                result: ConsensusResult::Accepted as u32,
                history_index: 0,
                chain_hash: chain_hash.clone(),
                prev_hash: prev,
            };

            assert!(entry.verify_chain_hash(&env));

            // Tamper with price – hash should fail.
            let mut tampered = entry.clone();
            tampered.price = price + 1;
            assert!(!tampered.verify_chain_hash(&env));
        });
    }

    #[test]
    fn test_confidence_too_wide_rejected() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
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
        });
    }

    #[test]
    fn test_process_consensus_no_feeds_fails() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "NOFEED");
            register_asset(&env, asset_id.clone(), AssetClass::RealEstate, make_name(&env, b"NF"), issuer, Some(1), None).unwrap();
            let result = process_consensus(&env, &asset_id);
            assert_eq!(result.unwrap_err(), OracleError::NoFeedsAvailable);
        });
    }

    #[test]
    fn test_staleness_window_override_per_asset() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "OVERRIDE_A");
            // Provide a custom 2-hour window.
            register_asset(&env, asset_id.clone(), AssetClass::FixedIncome, make_name(&env, b"OA"), issuer, Some(1), Some(7200)).unwrap();
            let rec = get_asset(&env, &asset_id).unwrap();
            assert_eq!(rec.staleness_window_override, 7200);
        });
    }

    #[test]
    fn test_oracle_submission_counter_increments() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let oracle = make_oracle(&env);
            register_oracle(&env, oracle.clone(), make_name(&env, b"Counter Oracle"), Vec::new(&env), 80).unwrap();
            let issuer = make_oracle(&env);
            let asset_id = make_asset_id(&env, "COUNTER");
            register_asset(&env, asset_id.clone(), AssetClass::Commodity, make_name(&env, b"CT"), issuer, Some(1), None).unwrap();
            env.ledger().with_mut(|l| l.timestamp = 1_000_000);

            submit_price_feed(&env, &oracle, asset_id.clone(), 100_000_000u128, 1_000_000u128, 999_900, Bytes::new(&env)).unwrap();
            let rec = get_oracle(&env, &oracle).unwrap();
            assert_eq!(rec.total_submissions, 1);
        });
    }

    #[test]
    fn test_oracle_bounds_and_checked_weighted_mean_overflow_safety() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            assert_eq!(
                validate_price_feed_bounds(0, 0, &Bytes::new(&env)),
                Err(OracleError::InvalidPrice)
            );
            assert_eq!(
                validate_price_feed_bounds(u128::MAX, 1, &Bytes::new(&env)),
                Err(OracleError::InvalidPrice)
            );
            let entries = [(100_000_000u128, 1_000_000u128, 80u32), (102_000_000u128, 1_000_000u128, 20u32)];
            let (weighted, conf) = checked_weighted_mean_price(&entries).unwrap();
            assert_eq!(weighted, 100_400_000u128);
            assert_eq!(conf, 1_000_000u128);
            assert_eq!(
                checked_weighted_mean_price(&[(u128::MAX, 1, 100)]),
                Err(OracleError::InvalidPrice)
            );
        });
    }
    // -----------------------------------------------------------------------
    // State-machine hardening: dedup, ordering, batch rollback, chain, Merkle
    // -----------------------------------------------------------------------

    const NOW: u64 = 1_000_000;
    const PRICE: u128 = 5_000_000_000_000;
    const CONF: u128 = 50_000_000;

    /// Registers `n` oracles (reputation 80) and asset `id` with `quorum`.
    fn bootstrap(env: &Env, id: &str, n: u32, quorum: u32) -> (Symbol, soroban_sdk::Vec<Address>) {
        env.ledger().with_mut(|l| l.timestamp = NOW);
        let mut oracles = soroban_sdk::Vec::new(env);
        for _ in 0..n {
            let o = make_oracle(env);
            register_oracle(env, o.clone(), make_name(env, b"O"), Vec::new(env), 80).unwrap();
            oracles.push_back(o);
        }
        let asset = make_asset_id(env, id);
        register_asset(env, asset.clone(), AssetClass::Commodity, make_name(env, b"A"), make_oracle(env), Some(quorum), None).unwrap();
        (asset, oracles)
    }

    fn submit(env: &Env, oracle: &Address, asset: &Symbol, price: u128, observed_at: u64) -> Result<PriceFeed, OracleError> {
        submit_price_feed(env, oracle, asset.clone(), price, CONF, observed_at, Bytes::new(env))
    }

    fn input(asset: &Symbol, price: u128, observed_at: u64, env: &Env) -> PriceFeedInput {
        PriceFeedInput { asset_id: asset.clone(), price, confidence: CONF, observed_at, metadata: Bytes::new(env) }
    }

    /// Run one full round with every oracle at `NOW` (advancing time by 1s).
    fn round(env: &Env, oracles: &soroban_sdk::Vec<Address>, asset: &Symbol, price: u128) -> CanonicalPrice {
        let t = env.ledger().timestamp() + 1;
        env.ledger().with_mut(|l| l.timestamp = t);
        for o in oracles.iter() {
            submit(env, &o, asset, price, t).unwrap();
        }
        process_consensus(env, asset).unwrap()
    }

    #[test]
    fn test_duplicate_feed_rejected_without_state_change() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "DUP", 2, 2);
            let o = oracles.get(0).unwrap();
            submit(&env, &o, &asset, PRICE, NOW - 10).unwrap();
            let before = get_oracle(&env, &o).unwrap();
            assert_eq!(submit(&env, &o, &asset, PRICE, NOW - 10), Err(OracleError::DuplicateFeed));
            assert_eq!(get_oracle(&env, &o).unwrap(), before);
            assert_eq!(pending_feed_count(&env, &asset), 1);
        });
    }

    #[test]
    fn test_out_of_order_feed_rejected_and_newer_replaces_pending() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "OOO", 2, 2);
            let o = oracles.get(0).unwrap();
            submit(&env, &o, &asset, PRICE, NOW - 10).unwrap();
            // Older or equal observation with a different price is reordered.
            assert_eq!(submit(&env, &o, &asset, PRICE + 1, NOW - 20), Err(OracleError::OutOfOrderTimestamp));
            assert_eq!(submit(&env, &o, &asset, PRICE + 1, NOW - 10), Err(OracleError::OutOfOrderTimestamp));
            // Newer observation replaces the oracle's pending feed (one vote).
            submit(&env, &o, &asset, PRICE + 2, NOW - 5).unwrap();
            assert_eq!(pending_feed_count(&env, &asset), 1);
            // A single oracle can never satisfy quorum = 2 on its own.
            assert!(process_consensus(&env, &asset).is_err());
            assert_eq!(get_oracle(&env, &o).unwrap().total_submissions, 2);
        });
    }

    #[test]
    fn test_replay_across_rounds_rejected() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "REPLAY", 2, 2);
            for o in oracles.iter() {
                submit(&env, &o, &asset, PRICE, NOW - 10).unwrap();
            }
            process_consensus(&env, &asset).unwrap();
            // Replaying the same feeds into the next round is rejected.
            for o in oracles.iter() {
                assert_eq!(submit(&env, &o, &asset, PRICE, NOW - 10), Err(OracleError::DuplicateFeed));
            }
            assert_eq!(pending_feed_count(&env, &asset), 0);
        });
    }

    #[test]
    fn test_pending_buffer_full_rejects_new_oracle_but_allows_refresh() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "FULL", MAX_PENDING_FEEDS + 1, 2);
            for i in 0..MAX_PENDING_FEEDS {
                submit(&env, &oracles.get(i).unwrap(), &asset, PRICE, NOW - 10).unwrap();
            }
            let extra = oracles.get(MAX_PENDING_FEEDS).unwrap();
            assert_eq!(submit(&env, &extra, &asset, PRICE, NOW - 10), Err(OracleError::PendingBufferFull));
            assert_eq!(get_oracle(&env, &extra).unwrap().total_submissions, 0);
            // An oracle already in the buffer can still refresh its feed.
            submit(&env, &oracles.get(0).unwrap(), &asset, PRICE, NOW - 5).unwrap();
            assert_eq!(pending_feed_count(&env, &asset), MAX_PENDING_FEEDS);
        });
    }

    #[test]
    fn test_submit_rejects_out_of_bound_price_and_metadata() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "BOUNDS", 1, 1);
            let o = oracles.get(0).unwrap();
            assert_eq!(
                submit_price_feed(&env, &o, asset.clone(), MAX_SAFE_ORACLE_PRICE + 1, 0, NOW, Bytes::new(&env)),
                Err(OracleError::InvalidPrice)
            );
            let big = Bytes::from_slice(&env, &[0u8; (MAX_FEED_METADATA_LEN + 1) as usize]);
            assert_eq!(
                submit_price_feed(&env, &o, asset.clone(), PRICE, CONF, NOW, big),
                Err(OracleError::OracleNameTooLong)
            );
            assert_eq!(submit(&env, &o, &asset, PRICE, NOW + 61), Err(OracleError::FutureTimestamp));
            let inactive = make_asset_id(&env, "INACTIVE");
            register_asset(&env, inactive.clone(), AssetClass::Other, make_name(&env, b"I"), o.clone(), None, None).unwrap();
            let mut rec = get_asset(&env, &inactive).unwrap();
            rec.active = false;
            env.storage().persistent().set(&OracleDataKey::Asset(inactive.clone()), &rec);
            assert_eq!(submit(&env, &o, &inactive, PRICE, NOW), Err(OracleError::AssetNotRegistered));
        });
    }

    #[test]
    fn test_batch_success_and_bounds() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "BATCH", 1, 1);
            let o = oracles.get(0).unwrap();
            assert_eq!(submit_price_feed_batch(&env, &o, &Vec::new(&env)), Err(OracleError::EmptyBatch));
            let mut too_many = Vec::new(&env);
            for i in 0..=MAX_FEED_BATCH {
                too_many.push_back(input(&asset, PRICE, NOW - 100 + i as u64, &env));
            }
            assert_eq!(submit_price_feed_batch(&env, &o, &too_many), Err(OracleError::BatchTooLarge));

            let mut ok = Vec::new(&env);
            ok.push_back(input(&asset, PRICE, NOW - 3, &env));
            ok.push_back(input(&asset, PRICE + 1, NOW - 2, &env));
            assert_eq!(submit_price_feed_batch(&env, &o, &ok), Ok(2));
            assert_eq!(pending_feed_count(&env, &asset), 1);
            assert_eq!(get_oracle(&env, &o).unwrap().total_submissions, 2);
        });
    }

    #[test]
    fn test_batch_mid_failure_rolls_back_every_touched_key() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset_a, oracles) = bootstrap(&env, "RB_A", 2, 2);
            let asset_b = make_asset_id(&env, "RB_B");
            register_asset(&env, asset_b.clone(), AssetClass::Other, make_name(&env, b"B"), make_oracle(&env), None, None).unwrap();
            let o = oracles.get(0).unwrap();
            // Pre-existing state for asset A (asset B has none).
            submit(&env, &o, &asset_a, PRICE, NOW - 50).unwrap();
            let oracle_before = get_oracle(&env, &o).unwrap();
            let pending_a: Vec<PriceFeed> = env.storage().persistent().get(&OracleDataKey::PendingFeeds(asset_a.clone())).unwrap();
            let obs_a: (u64, BytesN<32>) = env.storage().persistent()
                .get(&OracleDataKey::LastObservation(o.clone(), asset_a.clone())).unwrap();

            let mut batch = Vec::new(&env);
            batch.push_back(input(&asset_a, PRICE + 1, NOW - 40, &env)); // ok
            batch.push_back(input(&asset_b, PRICE, NOW - 40, &env));     // ok (new keys)
            batch.push_back(input(&asset_a, PRICE + 2, NOW - 45, &env)); // out of order
            assert_eq!(submit_price_feed_batch(&env, &o, &batch), Err(OracleError::OutOfOrderTimestamp));

            assert_eq!(get_oracle(&env, &o).unwrap(), oracle_before);
            let pending_after: Vec<PriceFeed> = env.storage().persistent().get(&OracleDataKey::PendingFeeds(asset_a.clone())).unwrap();
            assert_eq!(pending_after, pending_a);
            let obs_after: (u64, BytesN<32>) = env.storage().persistent()
                .get(&OracleDataKey::LastObservation(o.clone(), asset_a.clone())).unwrap();
            assert_eq!(obs_after, obs_a);
            // Keys that did not exist before the batch are removed again.
            assert!(!env.storage().persistent().has(&OracleDataKey::PendingFeeds(asset_b.clone())));
            assert!(!env.storage().persistent().has(&OracleDataKey::LastObservation(o.clone(), asset_b.clone())));

            // The same batch with the bad entry fixed now succeeds.
            batch.set(2, input(&asset_a, PRICE + 2, NOW - 30, &env));
            assert_eq!(submit_price_feed_batch(&env, &o, &batch), Ok(3));
        });
    }

    #[test]
    fn test_batch_from_unregistered_oracle_leaves_no_trace() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, _) = bootstrap(&env, "GHOST", 1, 1);
            let ghost = make_oracle(&env);
            let mut batch = Vec::new(&env);
            batch.push_back(input(&asset, PRICE, NOW, &env));
            assert_eq!(submit_price_feed_batch(&env, &ghost, &batch), Err(OracleError::NotRegistered));
            assert!(!env.storage().persistent().has(&OracleDataKey::Oracle(ghost.clone())));
            assert_eq!(pending_feed_count(&env, &asset), 0);
        });
    }

    #[test]
    fn test_history_chain_and_merkle_proofs_verify() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "CHAIN", 3, 2);
            assert_eq!(verify_recent_price_history(&env, &asset), Ok(0));
            assert_eq!(
                price_history_merkle_root(&env, &asset, 0, 0),
                Ok(BytesN::from_array(&env, &[0u8; 32]))
            );
            for i in 0..7u128 {
                round(&env, &oracles, &asset, PRICE + i * 1_000);
            }
            assert_eq!(verify_recent_price_history(&env, &asset), Ok(7));
            // Every sub-segment verifies, anchored to its predecessor.
            for start in 0..7u32 {
                for len in 0..=(7 - start) {
                    assert_eq!(verify_price_history_chain(&env, &asset, start, len), Ok(len));
                }
            }
            let root = price_history_merkle_root(&env, &asset, 0, 7).unwrap();
            for i in 0..7u32 {
                let entry = get_canonical_price(&env, &asset, i as u64).unwrap();
                let proof = price_history_merkle_proof(&env, &asset, 0, 7, i).unwrap();
                assert!(verify_price_history_proof(&env, &entry, &proof, &root));
                // Proof for a different entry does not verify this one.
                let other = price_history_merkle_proof(&env, &asset, 0, 7, (i + 1) % 7).unwrap();
                assert!(!verify_price_history_proof(&env, &entry, &other, &root));
                // A forged entry fails even with a genuine proof.
                let mut forged = entry.clone();
                forged.price += 1;
                assert!(!verify_price_history_proof(&env, &forged, &proof, &root));
            }
            // Sub-segment roots prove their own entries.
            let seg_root = price_history_merkle_root(&env, &asset, 2, 3).unwrap();
            let entry = get_canonical_price(&env, &asset, 3).unwrap();
            let proof = price_history_merkle_proof(&env, &asset, 2, 3, 3).unwrap();
            assert_eq!(proof.position, 1);
            assert!(verify_price_history_proof(&env, &entry, &proof, &seg_root));
            assert!(!verify_price_history_proof(&env, &entry, &proof, &root));

            // Range validation.
            assert_eq!(price_history_merkle_proof(&env, &asset, 0, 7, 7), Err(OracleError::InvalidRange));
            assert_eq!(price_history_merkle_proof(&env, &asset, 2, 3, 1), Err(OracleError::InvalidRange));
            assert_eq!(price_history_merkle_root(&env, &asset, 5, 3), Err(OracleError::InvalidRange));
            assert_eq!(verify_price_history_chain(&env, &asset, u32::MAX, 2), Err(OracleError::InvalidRange));
            assert_eq!(
                verify_price_history_chain(&env, &asset, 0, MAX_VERIFY_SPAN + 1),
                Err(OracleError::InvalidRange)
            );
        });
    }

    #[test]
    fn test_chain_verification_detects_tampering() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "TAMPER", 2, 2);
            for i in 0..3u128 {
                round(&env, &oracles, &asset, PRICE + i);
            }
            let key = OracleDataKey::PriceHistoryEntry(asset.clone(), 1);
            let original: CanonicalPrice = env.storage().persistent().get(&key).unwrap();

            // Field tampering breaks the entry's own chain hash.
            let mut t = original.clone();
            t.price += 1;
            env.storage().persistent().set(&key, &t);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));

            // A self-consistent but re-linked entry breaks the chain link.
            let mut relinked = original.clone();
            relinked.prev_hash = BytesN::from_array(&env, &[7u8; 32]);
            relinked.chain_hash = compute_canonical_hash(&env, &asset, relinked.price, relinked.consensus_at, &relinked.prev_hash);
            env.storage().persistent().set(&key, &relinked);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));

            // Wrong index.
            let mut wrong_index = original.clone();
            wrong_index.history_index = 9;
            env.storage().persistent().set(&key, &wrong_index);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));

            // Missing entry.
            env.storage().persistent().remove(&key);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));

            // Restored entry verifies again; a stale latest pointer does not.
            env.storage().persistent().set(&key, &original);
            assert_eq!(verify_recent_price_history(&env, &asset), Ok(3));
            env.storage().persistent().set(&OracleDataKey::LatestPrice(asset.clone()), &original);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));

            // Genesis must link to zero.
            let gkey = OracleDataKey::PriceHistoryEntry(asset.clone(), 0);
            let mut genesis: CanonicalPrice = env.storage().persistent().get(&gkey).unwrap();
            genesis.prev_hash = BytesN::from_array(&env, &[1u8; 32]);
            genesis.chain_hash = compute_canonical_hash(&env, &asset, genesis.price, genesis.consensus_at, &genesis.prev_hash);
            env.storage().persistent().set(&gkey, &genesis);
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));
        });
    }

    /// Stores a self-consistent canonical entry at `index` chained to `prev_hash`.
    fn seed_entry(env: &Env, asset: &Symbol, index: u32, prev_hash: BytesN<32>) -> CanonicalPrice {
        let cp = CanonicalPrice {
            asset_id: asset.clone(),
            price: PRICE,
            confidence: CONF,
            oracle_count: 2,
            consensus_at: NOW,
            oldest_observation: NOW,
            result: ConsensusResult::Accepted as u32,
            history_index: index,
            chain_hash: compute_canonical_hash(env, asset, PRICE, NOW, &prev_hash),
            prev_hash,
        };
        env.storage().persistent().set(&OracleDataKey::PriceHistoryEntry(asset.clone(), index as u64), &cp);
        cp
    }

    #[test]
    fn test_rolled_over_window_eviction_and_anchoring() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "EVICT", 2, 2);
            // Simulate a full window: count = MAX + 4 (window start = 4), with
            // the oldest retained entry and the newest entry seeded.
            let count = MAX_PRICE_HISTORY + 4;
            let oldest = seed_entry(&env, &asset, 4, BytesN::from_array(&env, &[3u8; 32]));
            let newest = seed_entry(&env, &asset, count - 1, BytesN::from_array(&env, &[9u8; 32]));
            env.storage().persistent().set(&OracleDataKey::PriceHistoryCount(asset.clone()), &count);
            env.storage().persistent().set(&OracleDataKey::LatestPrice(asset.clone()), &newest);

            // The window-start entry's predecessor was evicted, so it is not
            // anchored to genesis zero bytes.
            assert_eq!(verify_price_history_chain(&env, &asset, 4, 1), Ok(1));
            // Before the window start is out of range.
            assert_eq!(verify_price_history_chain(&env, &asset, 3, 1), Err(OracleError::InvalidRange));
            // A segment whose predecessor is retained but missing is a violation.
            assert_eq!(
                verify_price_history_chain(&env, &asset, count - 1, 1),
                Err(OracleError::ChainIntegrityViolation)
            );

            // The next accepted round chains onto the newest entry and evicts
            // the oldest retained one.
            let cp = round(&env, &oracles, &asset, PRICE);
            assert_eq!(cp.history_index, count);
            assert_eq!(cp.prev_hash, newest.chain_hash);
            assert!(get_canonical_price(&env, &asset, oldest.history_index as u64).is_none());
            assert_eq!(verify_price_history_chain(&env, &asset, 4, 1), Err(OracleError::InvalidRange));
            // Newest segment verifies (anchored to the seeded predecessor), tip included.
            assert_eq!(verify_price_history_chain(&env, &asset, count, 1), Ok(1));
            // Missing entries within the recent span are reported.
            assert_eq!(verify_recent_price_history(&env, &asset), Err(OracleError::ChainIntegrityViolation));
            assert_eq!(
                price_history_merkle_root(&env, &asset, count - 2, 2),
                Err(OracleError::ChainIntegrityViolation)
            );
        });
    }

    #[test]
    fn test_consensus_counters_are_consistent_and_overflow_is_atomic() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "ATOMIC", 2, 2);
            round(&env, &oracles, &asset, PRICE);
            assert_eq!(get_asset(&env, &asset).unwrap().total_updates, 1);
            let rounds: u32 = env.storage().instance().get(&OracleDataKey::TotalRounds).unwrap();
            assert_eq!(rounds, 1);

            // Force the global round counter to the edge: the next accepted
            // round must fail *before* writing anything.
            env.storage().instance().set(&OracleDataKey::TotalRounds, &u32::MAX);
            let t = env.ledger().timestamp() + 1;
            env.ledger().with_mut(|l| l.timestamp = t);
            for o in oracles.iter() {
                submit(&env, &o, &asset, PRICE, t).unwrap();
            }
            let history_before = get_price_history(&env, &asset);
            let latest_before = get_latest_price(&env, &asset);
            assert_eq!(process_consensus(&env, &asset), Err(OracleError::Overflow));
            assert_eq!(get_price_history(&env, &asset), history_before);
            assert_eq!(get_latest_price(&env, &asset), latest_before);
            assert_eq!(get_asset(&env, &asset).unwrap().total_updates, 1);
            assert_eq!(pending_feed_count(&env, &asset), 2);
            assert_eq!(verify_recent_price_history(&env, &asset), Ok(1));
        });
    }

    #[test]
    fn test_consensus_is_independent_of_submission_order() {
        let run = |reverse: bool| {
            let (env, contract) = setup_env();
            env.as_contract(&contract, || {
                let (asset, oracles) = bootstrap(&env, "ORDER", 3, 2);
                let prices = [PRICE, PRICE + 10_000, PRICE + 20_000];
                let mut idx = [0u32, 1, 2];
                if reverse {
                    idx.reverse();
                }
                for i in idx {
                    submit(&env, &oracles.get(i).unwrap(), &asset, prices[i as usize], NOW - 1).unwrap();
                }
                let cp = process_consensus(&env, &asset).unwrap();
                (cp.price, cp.confidence, cp.oracle_count)
            })
        };
        assert_eq!(run(false), run(true));
    }

    #[test]
    fn test_oracle_suspended_after_submitting_is_excluded_from_round() {
        let (env, contract) = setup_env();
        env.as_contract(&contract, || {
            let (asset, oracles) = bootstrap(&env, "LIVE", 3, 2);
            submit(&env, &oracles.get(0).unwrap(), &asset, PRICE, NOW - 1).unwrap();
            submit(&env, &oracles.get(1).unwrap(), &asset, PRICE + 10_000, NOW - 1).unwrap();
            // Oracle 1 is suspended while its feed is still pending.
            set_oracle_status(&env, &oracles.get(1).unwrap(), OracleStatus::Suspended).unwrap();
            // Only one live feed remains: quorum 2 is not met.
            assert_eq!(process_consensus(&env, &asset), Err(OracleError::InvalidPrice));
            assert_eq!(get_price_history(&env, &asset).len(), 0);

            // With a third live oracle, the suspended oracle's feed is ignored.
            submit(&env, &oracles.get(0).unwrap(), &asset, PRICE, NOW).unwrap();
            submit(&env, &oracles.get(1).unwrap(), &asset, PRICE * 2, NOW).unwrap_err();
            submit(&env, &oracles.get(2).unwrap(), &asset, PRICE, NOW).unwrap();
            let cp = process_consensus(&env, &asset).unwrap();
            assert_eq!(cp.oracle_count, 2);
            assert_eq!(cp.price, PRICE);
        });
    }
}
