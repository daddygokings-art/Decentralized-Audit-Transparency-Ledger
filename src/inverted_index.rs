//! On-chain inverted indexes for efficient event querying (Issue #388).
//!
//! # Design
//!
//! Events stored in the AuditLedger are retrieved by sequential index or by
//! event-type sub-ledger. This module adds **inverted indexes** so that callers
//! can look up events by arbitrary metadata fields, by category+type, by
//! submitter+type, or by sub-event-type — without scanning the entire log.
//!
//! ## Storage layout
//!
//! Each index entry is stored as a packed `Bytes` value: a sequence of 4-byte
//! little-endian `u32` global event indices. This mirrors the approach used by
//! `DataKey::EventTypeIndices` and `DataKey::SubmitterEventIndices` in
//! `src/lib.rs`, so the same decode helper logic applies here.
//!
//! ## Capacity and eviction
//!
//! Each index bucket is capped at [`INDEX_MAX_ENTRIES`] entries. Once a bucket
//! is full, the oldest (lowest-index) entries are evicted (FIFO) to make room
//! for the new entry. This prevents unbounded state growth for high-cardinality
//! fields.
//!
//! ## Metadata parsing
//!
//! [`index_event_metadata`] parses `key=value;key=value` formatted metadata
//! bytes. Each `value` is hashed with SHA-256 to produce the 32-byte bucket
//! key stored as `IndexKey::MetadataFieldIndex(field_symbol, sha256(value))`.
//!
//! ## No-std
//!
//! This module operates under `#![no_std]` (inherited from `lib.rs`) and uses
//! only `soroban_sdk` types.

#![allow(unused)]

use soroban_sdk::{contracttype, bytes, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of event indices stored per index bucket.
/// When this limit is reached, the oldest entries are dropped (FIFO eviction).
pub const INDEX_MAX_ENTRIES: u32 = 1000;

/// Maximum field name length in bytes (mirrors the Soroban Symbol limit).
pub const INDEX_FIELD_MAX_LEN: u32 = 32;

// ── Storage Key ───────────────────────────────────────────────────────────────

/// Storage keys for all inverted-index buckets.
///
/// Each variant maps to a `Bytes` packed value that holds a sequence of
/// 4-byte little-endian `u32` global event indices.
#[derive(Clone)]
#[contracttype]
pub enum IndexKey {
    /// Inverted index over metadata field values.
    /// Key: `(field_symbol, sha256(field_value))` → packed u32 indices.
    MetadataFieldIndex(Symbol, BytesN<32>),

    /// Combined category + event_type index.
    /// Key: `(category, event_type)` → packed u32 indices.
    CategoryTypeIndex(Symbol, Symbol),

    /// Combined submitter + event_type index.
    /// Key: `(submitter, event_type)` → packed u32 indices.
    SubmitterTypeIndex(Address, Symbol),

    /// Sub-event-type index.
    /// Key: `sub_event_type` → packed u32 indices.
    SubEventTypeIndex(Symbol),

    /// Total number of distinct fields that have been indexed (metadata counter).
    IndexedFieldCount,
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Encode a `u32` as 4 little-endian bytes in a `Bytes` value.
fn u32_to_4bytes(env: &Env, v: u32) -> Bytes {
    bytes!(
        env,
        [
            (v & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
            ((v >> 24) & 0xff) as u8,
        ]
    )
}

/// Decode the `n`-th 4-byte little-endian `u32` from a packed `Bytes` buffer.
/// `n` is a 0-based element index (not a byte offset).
/// Returns `None` when `n` is out of range.
fn read_u32_at(packed: &Bytes, n: u32) -> Option<u32> {
    let byte_offset = n * 4;
    if byte_offset + 4 > packed.len() {
        return None;
    }
    let b0 = packed.get(byte_offset)? as u32;
    let b1 = packed.get(byte_offset + 1)? as u32;
    let b2 = packed.get(byte_offset + 2)? as u32;
    let b3 = packed.get(byte_offset + 3)? as u32;
    Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
}

/// Return the number of u32 entries encoded in a packed `Bytes` buffer.
fn packed_len(packed: &Bytes) -> u32 {
    packed.len() / 4
}

/// Load a packed index from instance storage, or return empty `Bytes`.
fn load_packed(env: &Env, key: &IndexKey) -> Bytes {
    env.storage()
        .instance()
        .get::<IndexKey, Bytes>(key)
        .unwrap_or_else(|| Bytes::new(env))
}

/// Store a packed index into instance storage.
fn store_packed(env: &Env, key: &IndexKey, packed: &Bytes) {
    env.storage().instance().set(key, packed);
}

/// Trim the packed buffer to at most `INDEX_MAX_ENTRIES` entries, dropping the
/// oldest (leftmost) entries when over capacity.  Returns the (possibly
/// unchanged) buffer.
fn enforce_capacity(env: &Env, packed: Bytes) -> Bytes {
    let count = packed_len(&packed);
    if count <= INDEX_MAX_ENTRIES {
        return packed;
    }
    // Number of entries to drop from the front.
    let drop_count = count - INDEX_MAX_ENTRIES;
    let drop_bytes = drop_count * 4;
    // Build a new Bytes containing only the tail entries.
    let new_len = INDEX_MAX_ENTRIES * 4;
    let mut trimmed = Bytes::new(env);
    for i in 0..new_len {
        if let Some(b) = packed.get(drop_bytes + i) {
            let single = bytes!(env, [b]);
            trimmed.append(&single);
        }
    }
    trimmed
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Append `event_global_index` to the packed index bucket identified by `key`.
///
/// If the bucket already contains [`INDEX_MAX_ENTRIES`] entries, the oldest
/// entry is evicted (FIFO) before the new one is appended, so the bucket never
/// grows beyond the capacity limit.
pub fn index_add_entry(env: &Env, key: IndexKey, event_global_index: u32) {
    let mut packed = load_packed(env, &key);
    packed.append(&u32_to_4bytes(env, event_global_index));
    let packed = enforce_capacity(env, packed);
    store_packed(env, &key, &packed);
}

/// Return all global event indices stored in the bucket identified by `key`.
///
/// Returns an empty `Vec` when the bucket does not exist.
pub fn index_query(env: &Env, key: IndexKey) -> Vec<u32> {
    let packed = load_packed(env, &key);
    let count = packed_len(&packed);
    let mut result: Vec<u32> = Vec::new(env);
    for i in 0..count {
        if let Some(v) = read_u32_at(&packed, i) {
            result.push_back(v);
        }
    }
    result
}

/// Return the number of event indices stored in the bucket identified by `key`.
///
/// This is a cheap O(1) operation (reads one storage entry and divides its
/// byte-length by 4).
pub fn index_get_count(env: &Env, key: IndexKey) -> u32 {
    let packed = load_packed(env, &key);
    packed_len(&packed)
}

/// Index a single event across all applicable inverted-index dimensions.
///
/// Dimensions indexed:
///
/// 1. **CategoryTypeIndex** — `(category, event_type)`
/// 2. **SubmitterTypeIndex** — `(submitter, event_type)`
/// 3. **SubEventTypeIndex** — `(sub_event_type)` (only when `Some`)
/// 4. **MetadataFieldIndex** — one bucket per `key=value` pair found in
///    `metadata`, keyed by `(Symbol::new(env, key), sha256(value_bytes))`.
///    Metadata format: `key1=value1;key2=value2` (ASCII/UTF-8, semicolon
///    separated, `=` delimiter).  Fields longer than [`INDEX_FIELD_MAX_LEN`]
///    bytes are silently skipped.
pub fn index_event_metadata(
    env: &Env,
    event_global_index: u32,
    event_type: &Symbol,
    category: &Symbol,
    submitter: &Address,
    sub_event_type: &Option<Symbol>,
    metadata: &Bytes,
) {
    // 1. Category + type index.
    index_add_entry(
        env,
        IndexKey::CategoryTypeIndex(category.clone(), event_type.clone()),
        event_global_index,
    );

    // 2. Submitter + type index.
    index_add_entry(
        env,
        IndexKey::SubmitterTypeIndex(submitter.clone(), event_type.clone()),
        event_global_index,
    );

    // 3. Sub-event-type index (optional).
    if let Some(sub_type) = sub_event_type {
        index_add_entry(
            env,
            IndexKey::SubEventTypeIndex(sub_type.clone()),
            event_global_index,
        );
    }

    // 4. Metadata field index.
    // Parse key=value;key=value pairs from the metadata bytes.
    parse_and_index_metadata(env, event_global_index, metadata);
}

/// Parse `key=value;key=value` formatted metadata and index each pair.
///
/// Rules:
/// - Pairs are separated by `;` (byte `0x3B`).
/// - Within a pair, the first `=` (byte `0x3D`) splits key from value.
/// - Keys longer than [`INDEX_FIELD_MAX_LEN`] bytes are skipped (too long to
///   fit in a Soroban `Symbol`).
/// - Empty keys or values are skipped.
/// - The value is SHA-256 hashed to produce a `BytesN<32>` bucket discriminant.
fn parse_and_index_metadata(env: &Env, event_global_index: u32, metadata: &Bytes) {
    let len = metadata.len();
    if len == 0 {
        return;
    }

    // We iterate byte-by-byte collecting segments:
    // State machine: building `key_buf` until `=`, then `val_buf` until `;` or EOF.

    // Accumulate raw bytes for key and value.
    let mut key_buf: Vec<u8> = Vec::new(env);
    let mut val_buf: Vec<u8> = Vec::new(env);
    let mut in_value = false; // false = reading key, true = reading value

    let flush = |env: &Env, key_buf: &Vec<u8>, val_buf: &Vec<u8>, idx: u32| {
        if key_buf.is_empty() || val_buf.is_empty() {
            return;
        }
        if key_buf.len() > INDEX_FIELD_MAX_LEN {
            return;
        }
        // Build a Bytes for the value so we can hash it.
        let mut val_bytes = Bytes::new(env);
        for b in val_buf.iter() {
            let single = bytes!(env, [b]);
            val_bytes.append(&single);
        }
        let val_hash: BytesN<32> = env.crypto().sha256(&val_bytes);

        // Build a Bytes for the key so we can construct a Symbol.
        // Symbol::new requires a &str; since we're no_std we convert via
        // a fixed-length ASCII buffer approach.
        // We only index keys that are valid ASCII (all bytes < 128).
        let key_len = key_buf.len() as usize;
        if key_len == 0 || key_len > 32 {
            return;
        }
        // Validate all key bytes are ASCII alphanumeric or underscore.
        for &b in key_buf.iter() {
            let valid = (b >= b'a' && b <= b'z')
                || (b >= b'A' && b <= b'Z')
                || (b >= b'0' && b <= b'9')
                || b == b'_';
            if !valid {
                return;
            }
        }
        // Build the field Symbol from the key bytes.
        // We use a fixed-size string buffer.
        let field_sym = bytes_to_symbol(env, key_buf);
        if let Some(sym) = field_sym {
            index_add_entry(
                env,
                IndexKey::MetadataFieldIndex(sym, val_hash),
                idx,
            );
        }
    };

    for i in 0..len {
        let b = metadata.get(i).unwrap_or(0);
        if b == b';' {
            // End of current pair.
            if in_value {
                flush(env, &key_buf, &val_buf, event_global_index);
            }
            key_buf = Vec::new(env);
            val_buf = Vec::new(env);
            in_value = false;
        } else if b == b'=' && !in_value {
            // Transition from key to value.
            in_value = true;
        } else if in_value {
            val_buf.push_back(b);
        } else {
            key_buf.push_back(b);
        }
    }
    // Flush the last pair (no trailing `;` required).
    if in_value {
        flush(env, &key_buf, &val_buf, event_global_index);
    }
}

/// Attempt to construct a Soroban `Symbol` from a sequence of ASCII bytes.
///
/// Returns `None` when the bytes cannot be represented as a valid Symbol
/// (e.g. length > 32, or non-ASCII characters).
///
/// Because this runs under `#![no_std]` we cannot use `format!` or `String`.
/// Instead we match on known lengths and construct via a fixed-size array.
fn bytes_to_symbol(env: &Env, buf: &Vec<u8>) -> Option<Symbol> {
    let n = buf.len() as usize;
    if n == 0 || n > 32 {
        return None;
    }
    // Build a fixed 32-byte array with the key bytes, then slice.
    let mut arr = [0u8; 32];
    for (i, &b) in buf.iter().enumerate() {
        if i >= 32 {
            break;
        }
        arr[i] = b;
    }
    // Construct the Symbol using the known-length slice from arr.
    // We use a match ladder for lengths 1-32 to avoid heap allocation.
    // Each branch creates a &[u8] slice of the correct length, converts to
    // a &str (safe because we validated ASCII above), and calls Symbol::new.
    macro_rules! sym_from_len {
        ($len:expr) => {{
            // SAFETY: all bytes are validated ASCII in parse_and_index_metadata.
            let slice = &arr[..$len];
            // Convert &[u8] → &str without std using core::str::from_utf8.
            if let Ok(s) = core::str::from_utf8(slice) {
                Some(Symbol::new(env, s))
            } else {
                None
            }
        }};
    }

    match n {
        1 => sym_from_len!(1),
        2 => sym_from_len!(2),
        3 => sym_from_len!(3),
        4 => sym_from_len!(4),
        5 => sym_from_len!(5),
        6 => sym_from_len!(6),
        7 => sym_from_len!(7),
        8 => sym_from_len!(8),
        9 => sym_from_len!(9),
        10 => sym_from_len!(10),
        11 => sym_from_len!(11),
        12 => sym_from_len!(12),
        13 => sym_from_len!(13),
        14 => sym_from_len!(14),
        15 => sym_from_len!(15),
        16 => sym_from_len!(16),
        17 => sym_from_len!(17),
        18 => sym_from_len!(18),
        19 => sym_from_len!(19),
        20 => sym_from_len!(20),
        21 => sym_from_len!(21),
        22 => sym_from_len!(22),
        23 => sym_from_len!(23),
        24 => sym_from_len!(24),
        25 => sym_from_len!(25),
        26 => sym_from_len!(26),
        27 => sym_from_len!(27),
        28 => sym_from_len!(28),
        29 => sym_from_len!(29),
        30 => sym_from_len!(30),
        31 => sym_from_len!(31),
        32 => sym_from_len!(32),
        _ => None,
    }
}
