//! # On-Chain Inverted Indexes for Keyword-Based Event Search (Issue #359)
//!
//! This module extends the AuditLedger with an on-chain inverted index that maps
//! keywords extracted from event metadata to the sequential event indices that
//! contain them. This replaces the O(N) linear scan in `search_events` with an
//! O(K) index lookup, where K is the number of events matching the keyword.
//!
//! ## Storage Schema
//!
//! ```text
//! IndexConfig                       → IndexConfig (global settings)
//! IndexTypeEnabled(event_type)      → bool (per-type governance flag)
//! KeywordIndex(keyword_hash)        → Bytes (packed u32 LE event indices)
//! KeywordCount(keyword_hash)        → u32  (entry count for fast quota checks)
//! EventKeywords(event_idx)          → Vec<BytesN<32>> (keyword hashes for an event)
//! IndexStats                        → IndexStats (aggregate statistics)
//! ```
//!
//! ## Keyword Extraction
//!
//! Keywords are extracted from metadata using whitespace, semicolons, equals
//! signs, commas, and colons as delimiters. Each token is lowercased
//! (ASCII only) and kept only if its byte-length is within
//! `[min_keyword_len, max_keyword_len]`. Stop words ("the", "a", "an", "in",
//! "of", "at", "to", "is", "it") are discarded. Each keyword is SHA-256
//! hashed to produce a 32-byte bucket key, enabling O(1) bucket lookup.
//!
//! ## Index–Event Atomicity
//!
//! `index_event` is called from `log_event`/`log_events` after the event is
//! written to storage. Because Soroban transactions are atomic, the write and
//! the index update commit together or not at all.
//!
//! ## Governance
//!
//! The contract owner can:
//! - Enable/disable indexing globally via `IndexConfig.indexing_enabled`.
//! - Enable/disable indexing per event type via
//!   `IndexTypeEnabled(event_type) → bool`.
//! - Adjust `min_keyword_len` and `max_keyword_len` to control storage cost.
//! - Cap each keyword bucket at `bucket_max_entries` (FIFO eviction when full).
//!
//! ## reindex_events
//!
//! `reindex_events(start, end)` iterates events `[start, end)` and rebuilds
//! all keyword index entries from their stored metadata. Each call is bounded by
//! `MAX_REINDEX_BATCH` events to keep ledger compute within limits.
//!
//! ## Storage Cost Note
//!
//! For each event, indexing writes approximately:
//! - 1 × `EventKeywords` entry (Vec of up to `max_keywords_per_event` × 32 B).
//! - Up to `max_keywords_per_event` × `KeywordIndex` bucket appends (4 B each).
//! - Up to `max_keywords_per_event` × `KeywordCount` updates.
//!
//! With defaults (max 20 keywords / event, bucket cap 10 000), 1 000 events adds
//! roughly 20 000 bucket entries × 4 B = 80 KB of index data — acceptable for
//! on-chain state.

#![allow(dead_code)]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default minimum keyword length in bytes.
pub const DEFAULT_MIN_KEYWORD_LEN: u32 = 3;
/// Default maximum keyword length in bytes.
pub const DEFAULT_MAX_KEYWORD_LEN: u32 = 32;
/// Default maximum number of keywords to index per event.
pub const DEFAULT_MAX_KEYWORDS_PER_EVENT: u32 = 20;
/// Default maximum bucket size (entries per keyword). FIFO eviction when full.
pub const DEFAULT_BUCKET_MAX_ENTRIES: u32 = 10_000;
/// Maximum events that `reindex_events` processes in a single call.
pub const MAX_REINDEX_BATCH: u32 = 50;
/// Maximum events returned per `search_events_by_keyword` page.
pub const MAX_SEARCH_PAGE_SIZE: u32 = 100;

/// ASCII stop words (short common words) that are not worth indexing.
const STOP_WORDS: &[&[u8]] = &[
    b"the", b"a", b"an", b"in", b"of", b"at", b"to", b"is", b"it", b"and", b"or", b"for", b"on", b"by", b"as", b"be",
    b"no",
];

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum IndexKey {
    /// Global index configuration.
    IndexConfig,
    /// Per-event-type indexing flag: true = index this type; false = skip.
    IndexTypeEnabled(Symbol),
    /// Keyword bucket: SHA-256(keyword_bytes) → packed u32 LE event indices.
    KeywordIndex(BytesN<32>),
    /// Entry count for a keyword bucket (avoids reparsing the Bytes length).
    KeywordCount(BytesN<32>),
    /// Keywords indexed for a specific event (for reindex / cleanup).
    EventKeywords(u32),
    /// Aggregate index statistics.
    IndexStats,
}

// ── Data structures ───────────────────────────────────────────────────────────

/// Global configuration for the inverted index.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexConfig {
    /// Whether indexing is globally enabled.
    pub indexing_enabled: bool,
    /// Minimum keyword length in bytes (inclusive).
    pub min_keyword_len: u32,
    /// Maximum keyword length in bytes (inclusive).
    pub max_keyword_len: u32,
    /// Maximum keywords to extract and index per event.
    pub max_keywords_per_event: u32,
    /// Maximum entries per keyword bucket (FIFO eviction when exceeded).
    pub bucket_max_entries: u32,
}

impl IndexConfig {
    pub fn default_config() -> Self {
        IndexConfig {
            indexing_enabled: true,
            min_keyword_len: DEFAULT_MIN_KEYWORD_LEN,
            max_keyword_len: DEFAULT_MAX_KEYWORD_LEN,
            max_keywords_per_event: DEFAULT_MAX_KEYWORDS_PER_EVENT,
            bucket_max_entries: DEFAULT_BUCKET_MAX_ENTRIES,
        }
    }
}

/// Aggregate statistics about the index state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexStats {
    /// Total events that have been indexed.
    pub total_indexed_events: u32,
    /// Total keyword-index entries written (sum of all bucket sizes).
    pub total_index_entries: u64,
    /// Total unique keywords encountered (approximation — based on bucket creations).
    pub total_unique_keywords: u64,
    /// Last event index that was reindexed (for resuming batch reindex).
    pub last_reindex_event: u32,
}

/// A single search result returned by `search_events_by_keyword`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeywordSearchResult {
    /// Global event index (use `get_event_by_order` to load the full event).
    pub event_index: u32,
    /// The keyword hash that matched (SHA-256 of the keyword bytes).
    pub keyword_hash: BytesN<32>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Load the index configuration, or return the default.
pub fn get_index_config(env: &Env) -> IndexConfig {
    env.storage()
        .instance()
        .get::<_, IndexConfig>(&IndexKey::IndexConfig)
        .unwrap_or_else(IndexConfig::default_config)
}

/// Check whether indexing is enabled for a specific event type.
///
/// Returns `true` when:
/// - Global indexing is enabled, AND
/// - No per-type override is set, OR the per-type override is `true`.
pub fn is_indexing_enabled_for_type(env: &Env, event_type: &Symbol) -> bool {
    let config = get_index_config(env);
    if !config.indexing_enabled {
        return false;
    }
    // Per-type flag: None means "inherit global" (enabled by default).
    env.storage()
        .instance()
        .get::<_, bool>(&IndexKey::IndexTypeEnabled(event_type.clone()))
        .unwrap_or(true)
}

/// SHA-256 hash of a keyword byte slice. Used as the bucket discriminant.
pub fn keyword_hash(env: &Env, keyword: &[u8]) -> BytesN<32> {
    let mut b = Bytes::new(env);
    for &byte in keyword.iter() {
        b.push_back(byte);
    }
    env.crypto().sha256(&b)
}

/// Load a keyword bucket (packed u32 LE indices) from storage.
fn load_bucket(env: &Env, kw_hash: &BytesN<32>) -> Bytes {
    env.storage()
        .instance()
        .get::<_, Bytes>(&IndexKey::KeywordIndex(kw_hash.clone()))
        .unwrap_or_else(|| Bytes::new(env))
}

/// Store a keyword bucket to storage and update its count cache.
fn store_bucket(env: &Env, kw_hash: &BytesN<32>, packed: &Bytes) {
    env.storage()
        .instance()
        .set(&IndexKey::KeywordIndex(kw_hash.clone()), packed);
    let count = packed.len() / 4;
    env.storage()
        .instance()
        .set(&IndexKey::KeywordCount(kw_hash.clone()), &count);
}

/// Append an event index to a keyword bucket, evicting the oldest entry (FIFO)
/// if the bucket is at capacity.
fn bucket_append(env: &Env, kw_hash: &BytesN<32>, event_idx: u32, bucket_max: u32) {
    let mut packed = load_bucket(env, kw_hash);

    // Append new entry (4 bytes LE).
    let le = event_idx.to_le_bytes();
    packed.append(&Bytes::from_slice(env, &le));

    // FIFO eviction: if over capacity, drop the oldest entries.
    let entry_count = packed.len() / 4;
    if entry_count > bucket_max {
        let drop_entries = entry_count - bucket_max;
        let drop_bytes = drop_entries * 4;
        let new_len = bucket_max * 4;
        let mut trimmed = Bytes::new(env);
        for i in 0..new_len {
            if let Some(b) = packed.get(drop_bytes + i) {
                trimmed.push_back(b);
            }
        }
        packed = trimmed;
    }

    // Track whether this is a new keyword bucket.
    let is_new = env
        .storage()
        .instance()
        .get::<_, u32>(&IndexKey::KeywordCount(kw_hash.clone()))
        .is_none();

    store_bucket(env, kw_hash, &packed);

    if is_new {
        let mut stats = load_stats(env);
        stats.total_unique_keywords = stats.total_unique_keywords.saturating_add(1);
        save_stats(env, &stats);
    }
}

/// Decode all u32 entries from a packed bucket.
fn decode_bucket(packed: &Bytes) -> impl Iterator<Item = u32> + '_ {
    let count = packed.len() / 4;
    (0..count).filter_map(move |i| {
        let off = i * 4;
        let b0 = packed.get(off)? as u32;
        let b1 = packed.get(off + 1)? as u32;
        let b2 = packed.get(off + 2)? as u32;
        let b3 = packed.get(off + 3)? as u32;
        Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    })
}

fn load_stats(env: &Env) -> IndexStats {
    env.storage()
        .instance()
        .get::<_, IndexStats>(&IndexKey::IndexStats)
        .unwrap_or(IndexStats {
            total_indexed_events: 0,
            total_index_entries: 0,
            total_unique_keywords: 0,
            last_reindex_event: 0,
        })
}

fn save_stats(env: &Env, stats: &IndexStats) {
    env.storage().instance().set(&IndexKey::IndexStats, stats);
}

// ── Keyword extraction ────────────────────────────────────────────────────────

/// Determine whether a byte is a word delimiter.
///
/// Delimiters: whitespace (space, tab, newline, carriage-return),
/// semicolon, equals, comma, colon, pipe, slash, hyphen when at a boundary.
#[inline]
fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\n' | b'\r' | b';' | b'=' | b',' | b':' | b'|' | b'/' | b'-' | b'.' | b'_'
    )
}

/// Lowercase an ASCII byte. Non-ASCII bytes are passed through unchanged.
#[inline]
fn to_lower(b: u8) -> u8 {
    if b >= b'A' && b <= b'Z' {
        b + 32
    } else {
        b
    }
}

/// Check whether a byte is a stop word.
fn is_stop_word(token: &[u8]) -> bool {
    for &stop in STOP_WORDS.iter() {
        if token.len() == stop.len() {
            let mut matched = true;
            for (i, &sb) in stop.iter().enumerate() {
                if token[i] != sb {
                    matched = false;
                    break;
                }
            }
            if matched {
                return true;
            }
        }
    }
    false
}

/// Extract keywords from metadata bytes.
///
/// Returns a `Vec` of up to `max_keywords` SHA-256 keyword hashes. Each hash
/// is 32 bytes. Deduplication is performed: the same keyword hash will not
/// appear twice in the result.
///
/// The extraction process:
/// 1. Split on delimiters.
/// 2. Lowercase each token (ASCII only).
/// 3. Discard tokens outside `[min_len, max_len]`.
/// 4. Discard stop words.
/// 5. Hash each accepted token.
/// 6. Deduplicate hashes.
/// 7. Stop after `max_keywords`.
pub fn extract_keywords(env: &Env, metadata: &Bytes, min_len: u32, max_len: u32, max_keywords: u32) -> Vec<BytesN<32>> {
    let mut result: Vec<BytesN<32>> = Vec::new(&env);
    if metadata.is_empty() || max_keywords == 0 {
        return result;
    }

    let meta_len = metadata.len();
    // We build tokens in a fixed-size stack buffer (max 32 bytes per token).
    let mut token_buf = [0u8; 32];
    let mut token_len: usize = 0;

    let flush_token = |token_buf: &[u8],
                       token_len: usize,
                       result: &mut Vec<BytesN<32>>,
                       env: &Env,
                       min_len: u32,
                       max_len: u32,
                       max_keywords: u32| {
        if result.len() >= max_keywords {
            return;
        }
        let tl = token_len as u32;
        if tl < min_len || tl > max_len {
            return;
        }
        let token = &token_buf[..token_len];
        if is_stop_word(token) {
            return;
        }
        let h = keyword_hash(env, token);
        // Simple dedup: check if already in result.
        for i in 0..result.len() {
            if result.get(i).unwrap() == h {
                return;
            }
        }
        result.push_back(h);
    };

    for i in 0..meta_len {
        let raw_byte = metadata.get(i).unwrap_or(0);

        if is_delimiter(raw_byte) {
            flush_token(&token_buf, token_len, &mut result, env, min_len, max_len, max_keywords);
            token_len = 0;
        } else {
            // Only index printable ASCII (32–126); skip non-printable.
            if raw_byte < 32 || raw_byte > 126 {
                continue;
            }
            let lowered = to_lower(raw_byte);
            if token_len < 32 {
                token_buf[token_len] = lowered;
                token_len += 1;
            }
            // If token overflows 32 bytes just stop accumulating; it will be
            // discarded at flush because tl > max_len.
        }

        if result.len() >= max_keywords {
            break;
        }
    }
    // Flush the last token.
    flush_token(&token_buf, token_len, &mut result, env, min_len, max_len, max_keywords);

    result
}

// ── Public index management API ───────────────────────────────────────────────

/// Initialize or update the index configuration.
///
/// Call this once from the contract owner to set keyword extraction parameters.
/// Subsequent calls overwrite the existing config.
pub fn init_index_config(env: &Env, config: IndexConfig) {
    env.storage().instance().set(&IndexKey::IndexConfig, &config);
}

/// Enable or disable indexing for a specific event type.
///
/// When `enabled = false`, events of this type will not be indexed during
/// `log_event`. Existing index entries for this type are not removed.
pub fn set_index_type_enabled(env: &Env, event_type: Symbol, enabled: bool) {
    env.storage()
        .instance()
        .set(&IndexKey::IndexTypeEnabled(event_type), &enabled);
}

/// Get the per-type indexing flag. Returns `None` when not explicitly set
/// (meaning the global `indexing_enabled` flag applies).
pub fn get_index_type_enabled(env: &Env, event_type: &Symbol) -> Option<bool> {
    env.storage()
        .instance()
        .get::<_, bool>(&IndexKey::IndexTypeEnabled(event_type.clone()))
}

/// Index a single event's metadata for keyword search.
///
/// This is the core indexing routine called during `log_event`. It:
/// 1. Checks global and per-type indexing flags.
/// 2. Extracts keywords from the event metadata.
/// 3. Appends the event index to each keyword's bucket (with FIFO eviction).
/// 4. Stores the list of keyword hashes for this event (for potential cleanup).
/// 5. Updates aggregate statistics.
///
/// # Arguments
/// * `event_idx`  – Global sequential event index (from `Config.total_events`).
/// * `event_type` – Event type Symbol (used for per-type governance check).
/// * `metadata`   – Raw metadata bytes to extract keywords from.
pub fn index_event(env: &Env, event_idx: u32, event_type: &Symbol, metadata: &Bytes) {
    if !is_indexing_enabled_for_type(env, event_type) {
        return;
    }

    let config = get_index_config(env);
    let kw_hashes = extract_keywords(
        env,
        metadata,
        config.min_keyword_len,
        config.max_keyword_len,
        config.max_keywords_per_event,
    );

    let num_keywords = kw_hashes.len();
    if num_keywords == 0 {
        return;
    }

    // Append event_idx to each keyword bucket.
    for i in 0..num_keywords {
        let h = kw_hashes.get(i).unwrap();
        bucket_append(env, &h, event_idx, config.bucket_max_entries);
    }

    // Store keyword hashes for this event (enables future cleanup / reindex).
    env.storage()
        .instance()
        .set(&IndexKey::EventKeywords(event_idx), &kw_hashes);

    // Update stats.
    let mut stats = load_stats(env);
    stats.total_indexed_events = stats.total_indexed_events.saturating_add(1);
    stats.total_index_entries = stats.total_index_entries.saturating_add(num_keywords as u64);
    save_stats(env, &stats);
}

/// Search for events containing a specific keyword.
///
/// Returns a paginated list of `KeywordSearchResult` values. Each result
/// contains the global event index, which the caller uses to load the full
/// event via `get_event_by_order`.
///
/// ## Algorithm
/// 1. Hash the provided `keyword` bytes.
/// 2. Load the keyword bucket (packed u32 event indices).
/// 3. Apply pagination: skip `page * page_size` entries, return up to
///    `page_size` entries.
///
/// ## Complexity
/// O(K + P) where K = total bucket size (bounded by `bucket_max_entries`) and
/// P = `page_size`. Much faster than the O(N) linear scan in `search_events`.
///
/// # Arguments
/// * `keyword`   – Raw keyword bytes (will be lowercased and hashed).
/// * `page`      – 0-based page number.
/// * `page_size` – Results per page (1–`MAX_SEARCH_PAGE_SIZE`).
///
/// # Returns
/// Vec of `KeywordSearchResult`. Empty when no matches or page is out of range.
pub fn search_events_by_keyword(env: &Env, keyword: &Bytes, page: u32, page_size: u32) -> Vec<KeywordSearchResult> {
    let mut results: Vec<KeywordSearchResult> = Vec::new(env);

    if keyword.is_empty() || page_size == 0 {
        return results;
    }
    let effective_size = page_size.min(MAX_SEARCH_PAGE_SIZE);

    // Build lowercased keyword bytes.
    let mut lowered = Bytes::new(env);
    for i in 0..keyword.len() {
        let b = keyword.get(i).unwrap_or(0);
        lowered.push_back(to_lower(b));
    }

    // Hash the keyword to get the bucket key.
    let kw_hash = env.crypto().sha256(&lowered);

    let packed = load_bucket(env, &kw_hash);
    if packed.is_empty() {
        return results;
    }

    let total_entries = packed.len() / 4;
    let start = page.saturating_mul(effective_size);
    if start >= total_entries {
        return results;
    }

    let end = (start + effective_size).min(total_entries);
    for i in start..end {
        let off = i * 4;
        let b0 = packed.get(off).unwrap_or(0) as u32;
        let b1 = packed.get(off + 1).unwrap_or(0) as u32;
        let b2 = packed.get(off + 2).unwrap_or(0) as u32;
        let b3 = packed.get(off + 3).unwrap_or(0) as u32;
        let event_idx = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
        results.push_back(KeywordSearchResult {
            event_index: event_idx,
            keyword_hash: kw_hash.clone(),
        });
    }

    results
}

/// Get the number of events indexed for a specific keyword.
///
/// Returns `0` when the keyword has never been indexed.
pub fn keyword_entry_count(env: &Env, keyword: &Bytes) -> u32 {
    if keyword.is_empty() {
        return 0;
    }
    let mut lowered = Bytes::new(env);
    for i in 0..keyword.len() {
        let b = keyword.get(i).unwrap_or(0);
        lowered.push_back(to_lower(b));
    }
    let kw_hash = env.crypto().sha256(&lowered);
    env.storage()
        .instance()
        .get::<_, u32>(&IndexKey::KeywordCount(kw_hash))
        .unwrap_or(0)
}

/// Backfill / rebuild keyword indexes for events in the range `[start, end)`.
///
/// This maintenance function re-extracts keywords from each event's stored
/// metadata and rebuilds the index entries. Useful after:
/// - Changing keyword extraction parameters (`min_keyword_len`, etc.).
/// - Enabling indexing for an event type that was previously excluded.
/// - Recovering from partial index corruption.
///
/// **Bounded execution**: processes at most `MAX_REINDEX_BATCH` events per
/// call to stay within Soroban ledger compute limits.
///
/// The caller supplies a `load_metadata` closure that retrieves the raw
/// metadata for a given event index. In production this loads from
/// `DataKey::EventData`, but the closure design makes this function testable
/// in isolation without the full contract.
///
/// # Returns
/// The index of the first event that was NOT processed (`start + events_processed`).
/// The caller can resume from this index in subsequent calls.
pub fn reindex_events<F>(env: &Env, start: u32, end: u32, load_event: F) -> u32
where
    F: Fn(u32) -> Option<(Symbol, Bytes)>, // (event_type, metadata)
{
    let config = get_index_config(env);
    let batch_end = end.min(start.saturating_add(MAX_REINDEX_BATCH));
    let mut processed = start;

    for event_idx in start..batch_end {
        if let Some((event_type, metadata)) = load_event(event_idx) {
            // Remove old index entries for this event (if any).
            remove_event_from_index(env, event_idx);

            // Re-index with current config.
            if is_indexing_enabled_for_type(env, &event_type) {
                let kw_hashes = extract_keywords(
                    env,
                    &metadata,
                    config.min_keyword_len,
                    config.max_keyword_len,
                    config.max_keywords_per_event,
                );
                let num_keywords = kw_hashes.len();
                for i in 0..num_keywords {
                    let h = kw_hashes.get(i).unwrap();
                    bucket_append(env, &h, event_idx, config.bucket_max_entries);
                }
                if num_keywords > 0 {
                    env.storage()
                        .instance()
                        .set(&IndexKey::EventKeywords(event_idx), &kw_hashes);
                }
            }
        }
        processed = event_idx + 1;
    }

    // Update last_reindex_event in stats.
    let mut stats = load_stats(env);
    if processed > stats.last_reindex_event {
        stats.last_reindex_event = processed;
    }
    save_stats(env, &stats);

    processed
}

/// Remove a single event's contributions from all keyword indexes.
///
/// This is a best-effort cleanup: it removes the event from the bucket entries
/// stored under `EventKeywords(event_idx)`. Buckets are not compacted; only
/// matching entries are removed to keep the operation O(K × B) where K is
/// keywords per event and B is the bucket size.
pub fn remove_event_from_index(env: &Env, event_idx: u32) {
    let kw_hashes: Vec<BytesN<32>> = env
        .storage()
        .instance()
        .get::<_, Vec<BytesN<32>>>(&IndexKey::EventKeywords(event_idx))
        .unwrap_or_else(|| Vec::new(env));

    for i in 0..kw_hashes.len() {
        let h = kw_hashes.get(i).unwrap();
        let packed = load_bucket(env, &h);
        if packed.is_empty() {
            continue;
        }
        // Rebuild the bucket without entries matching event_idx.
        let mut new_packed = Bytes::new(env);
        let entry_count = packed.len() / 4;
        for j in 0..entry_count {
            let off = j * 4;
            let b0 = packed.get(off).unwrap_or(0) as u32;
            let b1 = packed.get(off + 1).unwrap_or(0) as u32;
            let b2 = packed.get(off + 2).unwrap_or(0) as u32;
            let b3 = packed.get(off + 3).unwrap_or(0) as u32;
            let idx = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
            if idx != event_idx {
                new_packed.append(&Bytes::from_slice(env, &idx.to_le_bytes()));
            }
        }
        store_bucket(env, &h, &new_packed);
    }

    // Remove the per-event keyword list.
    env.storage().instance().remove(&IndexKey::EventKeywords(event_idx));
}

/// Get aggregate index statistics.
pub fn get_index_stats(env: &Env) -> IndexStats {
    load_stats(env)
}

/// Get all keyword hashes that were indexed for a specific event.
///
/// Returns an empty Vec when the event was not indexed.
pub fn get_event_keywords(env: &Env, event_idx: u32) -> Vec<BytesN<32>> {
    env.storage()
        .instance()
        .get::<_, Vec<BytesN<32>>>(&IndexKey::EventKeywords(event_idx))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Backward-compatible helpers (preserved from original Issue #388 API) ──────

/// Maximum entries per index bucket (kept for backward compatibility with #388).
#[deprecated(note = "Use DEFAULT_BUCKET_MAX_ENTRIES or IndexConfig.bucket_max_entries instead")]
pub const INDEX_MAX_ENTRIES: u32 = DEFAULT_BUCKET_MAX_ENTRIES;

/// Legacy `IndexKey` variants from Issue #388 — preserved for existing callers.
#[contracttype]
#[derive(Clone)]
pub enum LegacyIndexKey {
    MetadataFieldIndex(Symbol, BytesN<32>),
    CategoryTypeIndex(Symbol, Symbol),
    SubmitterTypeIndex(Address, Symbol),
    SubEventTypeIndex(Symbol),
    IndexedFieldCount,
}

/// Append `event_global_index` to a legacy index bucket (Issue #388 API).
///
/// Kept for backward compatibility. New code should use `index_event`.
pub fn index_add_entry(env: &Env, key: LegacyIndexKey, event_global_index: u32) {
    let storage_key = legacy_to_bytes_key(env, &key);
    let mut packed: Bytes = env
        .storage()
        .instance()
        .get::<_, Bytes>(&storage_key)
        .unwrap_or_else(|| Bytes::new(env));
    packed.append(&Bytes::from_slice(env, &event_global_index.to_le_bytes()));
    // Evict oldest if over DEFAULT_BUCKET_MAX_ENTRIES.
    let count = packed.len() / 4;
    if count > DEFAULT_BUCKET_MAX_ENTRIES {
        let drop = (count - DEFAULT_BUCKET_MAX_ENTRIES) * 4;
        let keep = DEFAULT_BUCKET_MAX_ENTRIES * 4;
        let mut trimmed = Bytes::new(env);
        for i in 0..keep {
            if let Some(b) = packed.get(drop + i) {
                trimmed.push_back(b);
            }
        }
        packed = trimmed;
    }
    env.storage().instance().set(&storage_key, &packed);
}

/// Query a legacy index bucket (Issue #388 API).
pub fn index_query(env: &Env, key: LegacyIndexKey) -> Vec<u32> {
    let storage_key = legacy_to_bytes_key(env, &key);
    let packed: Bytes = env
        .storage()
        .instance()
        .get::<_, Bytes>(&storage_key)
        .unwrap_or_else(|| Bytes::new(env));
    let count = packed.len() / 4;
    let mut result: Vec<u32> = Vec::new(env);
    for i in 0..count {
        let off = i * 4;
        let b0 = packed.get(off).unwrap_or(0) as u32;
        let b1 = packed.get(off + 1).unwrap_or(0) as u32;
        let b2 = packed.get(off + 2).unwrap_or(0) as u32;
        let b3 = packed.get(off + 3).unwrap_or(0) as u32;
        result.push_back(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24));
    }
    result
}

/// Get the entry count of a legacy index bucket (Issue #388 API).
pub fn index_get_count(env: &Env, key: LegacyIndexKey) -> u32 {
    let storage_key = legacy_to_bytes_key(env, &key);
    let packed: Bytes = env
        .storage()
        .instance()
        .get::<_, Bytes>(&storage_key)
        .unwrap_or_else(|| Bytes::new(env));
    packed.len() / 4
}

/// Index all applicable dimensions for a single event (legacy Issue #388 API).
///
/// Kept for backward compatibility. New code should use `index_event`.
pub fn index_event_metadata(
    env: &Env,
    event_global_index: u32,
    event_type: &Symbol,
    category: &Symbol,
    submitter: &Address,
    sub_event_type: &Option<Symbol>,
    metadata: &Bytes,
) {
    // CategoryTypeIndex
    index_add_entry(
        env,
        LegacyIndexKey::CategoryTypeIndex(category.clone(), event_type.clone()),
        event_global_index,
    );
    // SubmitterTypeIndex
    index_add_entry(
        env,
        LegacyIndexKey::SubmitterTypeIndex(submitter.clone(), event_type.clone()),
        event_global_index,
    );
    // SubEventTypeIndex
    if let Some(sub) = sub_event_type {
        index_add_entry(env, LegacyIndexKey::SubEventTypeIndex(sub.clone()), event_global_index);
    }
    // Metadata field index (key=value;key=value parsing)
    legacy_parse_and_index_metadata(env, event_global_index, metadata);
}

// ── Legacy private helpers ────────────────────────────────────────────────────

/// Convert a LegacyIndexKey to a stable storage key (Bytes discriminant).
///
/// We store the variant discriminant byte followed by the payload bytes.
/// Symbol payloads are stored as their raw u64 value in little-endian 8 bytes.
/// This keeps legacy data isolated from the new `IndexKey` storage space.
fn legacy_to_bytes_key(env: &Env, key: &LegacyIndexKey) -> Bytes {
    let mut b = Bytes::new(env);
    // Append a u64 as 8 little-endian bytes.
    fn push_u64(b: &mut Bytes, env: &Env, v: u64) {
        b.append(&Bytes::from_slice(env, &v.to_le_bytes()));
    }
    match key {
        LegacyIndexKey::MetadataFieldIndex(sym, hash) => {
            b.push_back(0x01);
            push_u64(&mut b, env, sym.to_val().get_payload());
            b.append(hash.as_ref());
        }
        LegacyIndexKey::CategoryTypeIndex(cat, etype) => {
            b.push_back(0x02);
            push_u64(&mut b, env, cat.to_val().get_payload());
            push_u64(&mut b, env, etype.to_val().get_payload());
        }
        LegacyIndexKey::SubmitterTypeIndex(addr, etype) => {
            b.push_back(0x03);
            b.append(&addr.to_string().to_bytes());
            push_u64(&mut b, env, etype.to_val().get_payload());
        }
        LegacyIndexKey::SubEventTypeIndex(sub) => {
            b.push_back(0x04);
            push_u64(&mut b, env, sub.to_val().get_payload());
        }
        LegacyIndexKey::IndexedFieldCount => {
            b.push_back(0x05);
        }
    }
    b
}

fn legacy_parse_and_index_metadata(env: &Env, event_global_index: u32, metadata: &Bytes) {
    let len = metadata.len();
    if len == 0 {
        return;
    }
    let mut key_buf: Vec<u8> = Vec::new(env);
    let mut val_buf: Vec<u8> = Vec::new(env);
    let mut in_value = false;

    let flush = |env: &Env, key_buf: &Vec<u8>, val_buf: &Vec<u8>, idx: u32| {
        if key_buf.is_empty() || val_buf.is_empty() {
            return;
        }
        if key_buf.len() > 32 {
            return;
        }
        // Validate key bytes are ASCII alphanumeric or underscore.
        for b in key_buf.iter() {
            let valid = (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z') || (b >= b'0' && b <= b'9') || b == b'_';
            if !valid {
                return;
            }
        }
        let mut val_bytes = Bytes::new(env);
        for &vb in val_buf.iter() {
            val_bytes.push_back(vb);
        }
        let val_hash: BytesN<32> = env.crypto().sha256(&val_bytes);
        if let Some(sym) = legacy_bytes_to_symbol(env, key_buf) {
            index_add_entry(env, LegacyIndexKey::MetadataFieldIndex(sym, val_hash), idx);
        }
    };

    for i in 0..len {
        let b = metadata.get(i).unwrap_or(0);
        if b == b';' {
            if in_value {
                flush(env, &key_buf, &val_buf, event_global_index);
            }
            key_buf = Vec::new(env);
            val_buf = Vec::new(env);
            in_value = false;
        } else if b == b'=' && !in_value {
            in_value = true;
        } else if in_value {
            val_buf.push_back(b);
        } else {
            key_buf.push_back(b);
        }
    }
    if in_value {
        flush(env, &key_buf, &val_buf, event_global_index);
    }
}

fn legacy_bytes_to_symbol(env: &Env, buf: &Vec<u8>) -> Option<Symbol> {
    let n = buf.len() as usize;
    if n == 0 || n > 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    for (i, b) in buf.iter().enumerate() {
        if i >= 32 {
            break;
        }
        arr[i] = b;
    }
    let slice = &arr[..n];
    if let Ok(s) = core::str::from_utf8(slice) {
        Some(Symbol::new(env, s))
    } else {
        None
    }
}
