# On-Chain Inverted Indexes for Event Querying

Issue #388 — This document describes the on-chain inverted-index module that
enables efficient event querying beyond the sequential order and per-type
sub-ledgers already provided by AuditLedger.

---

## Overview

The core AuditLedger stores events sequentially and maintains per-event-type
index arrays (see `DataKey::EventTypeIndices`). However, querying events by
metadata field values, by submitter + type combination, or by category requires
either a full scan or off-chain indexing.

The `inverted_index` module adds four on-chain index dimensions:

| Dimension | Key | Use case |
|-----------|-----|---------|
| `CategoryTypeIndex` | (category, event_type) | "Give me all `payment` events in the `finance` category" |
| `SubmitterTypeIndex` | (submitter, event_type) | "Give me all `transfer` events from address GA..." |
| `SubEventTypeIndex` | sub_event_type | "Give me all events with sub-type `wire`" |
| `MetadataFieldIndex` | (field_name, sha256(value)) | "Give me all events where `currency=USD`" |

Each index bucket stores a packed array of `u32` global event indices (4 bytes
each, little-endian) — the same encoding used by `DataKey::EventTypeIndices`
and `DataKey::SubmitterEventIndices` in the main contract.

---

## Storage Layout

```
IndexKey::CategoryTypeIndex("finance", "payment")  →  Bytes: [00 00 00 00, 01 00 00 00, 07 00 00 00, ...]
                                                               event 0     event 1     event 7
IndexKey::SubmitterTypeIndex(GA..., "transfer")    →  Bytes: [03 00 00 00, 05 00 00 00, ...]
IndexKey::SubEventTypeIndex("wire")                →  Bytes: [02 00 00 00, ...]
IndexKey::MetadataFieldIndex("amount", sha256(…))  →  Bytes: [00 00 00 00, 04 00 00 00, ...]
```

All buckets live in Soroban **instance storage**. This matches the storage tier
used by `DataKey::EventTypeIndices` and is suitable for frequently-read index
data.

### Capacity limits

To prevent state bloat, each bucket is capped at `INDEX_MAX_ENTRIES = 1000`
entries. When a bucket is full, the **oldest** (leftmost) entries are evicted
before the new entry is appended (FIFO policy).

This means high-cardinality buckets (e.g., very common metadata values) retain
the most recent 1000 matching events. Less-active buckets never grow beyond 1000
entries.

---

## Metadata Parsing

`index_event_metadata` parses `key=value` pairs from event metadata bytes:

```
Format: key1=value1;key2=value2;key3=value3
```

Rules:
- Delimiter: `;` separates pairs, `=` separates key from value within a pair.
- Key length: max 32 bytes (matches Soroban Symbol limit).
- Key characters: ASCII alphanumeric + `_` only (other characters cause the
  pair to be skipped).
- Value: any bytes sequence; SHA-256-hashed before indexing (so large values
  don't bloat the key).
- Empty keys or values are silently skipped.

Example:

```
metadata = b"amount=1000;currency=USD;counterparty=GA..."
```

Creates three `MetadataFieldIndex` entries:
- `("amount", sha256("1000"))  → [event_index]`
- `("currency", sha256("USD")) → [event_index]`
- `("counterparty", sha256("GA...")) → [event_index]`

---

## API Reference

All functions live in `src/inverted_index.rs`.

### `index_add_entry(env, key, event_global_index)`

Append `event_global_index` to the bucket identified by `key`. Enforces
`INDEX_MAX_ENTRIES` capacity with FIFO eviction.

### `index_query(env, key) -> Vec<u32>`

Return all global event indices stored in `key`'s bucket. Returns an empty
`Vec` when the bucket does not exist.

### `index_get_count(env, key) -> u32`

Return the number of entries in the bucket (cheap O(1) operation — bytes /4).

### `index_event_metadata(env, event_global_index, event_type, category, submitter, sub_event_type, metadata)`

Index a new event across all four dimensions. Called by `logEvent` after the
event is stored.

---

## Usage Example

```rust
use crate::inverted_index::{index_event_metadata, index_query, IndexKey};
use soroban_sdk::{Symbol, Bytes, Env};

// After logEvent stores event at global index 42:
let metadata = Bytes::from_slice(&env, b"amount=500;currency=EUR");
index_event_metadata(
    &env,
    42,                             // global event index
    &Symbol::new(&env, "payment"),  // event_type
    &Symbol::new(&env, "finance"),  // category
    &submitter,
    &Some(Symbol::new(&env, "sepa")),
    &metadata,
);

// Later: find all "payment" events in "finance"
let indices = index_query(
    &env,
    IndexKey::CategoryTypeIndex(
        Symbol::new(&env, "finance"),
        Symbol::new(&env, "payment"),
    ),
);
// indices: Vec<u32> containing 42 (and any other matching events)

// Find all events where currency=EUR
use soroban_sdk::BytesN;
let val_bytes = Bytes::from_slice(&env, b"EUR");
let val_hash: BytesN<32> = env.crypto().sha256(&val_bytes);
let eur_events = index_query(
    &env,
    IndexKey::MetadataFieldIndex(Symbol::new(&env, "currency"), val_hash),
);
```

---

## Integration with logEvent

The inverted index is designed to be called from `logEvent` immediately after
the event struct is stored. Add the following to the `logEvent` implementation
in `src/lib.rs`:

```rust
// After storing the event:
crate::inverted_index::index_event_metadata(
    &env,
    event.index,
    &event.event_type,
    &event.category,
    &event.submitter,
    &event.sub_event_type,
    &event.metadata,
);
```

This is not yet wired in (the module exists as an opt-in feature to avoid
breaking the existing contract ABI). The integration will be tracked in a
follow-up issue.

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `index_add_entry` | O(n) eviction + O(1) append | n = INDEX_MAX_ENTRIES (eviction only when full) |
| `index_query` | O(k) | k = number of matching events (at most 1000) |
| `index_get_count` | O(1) | 1 instance storage read |
| `index_event_metadata` | O(m) | m = number of key=value pairs in metadata |

Each index bucket uses 4 bytes per entry in Soroban instance storage. A full
bucket (1000 entries) consumes 4 KB of on-chain storage.

---

## Limitations

1. **No true deletion** — inverted indexes are append-only. Erased events
   (GDPR crypto-shredding) may still appear in index buckets. Callers should
   check whether an event is erased after retrieving it from an index.

2. **Hash collisions** — metadata values are indexed by SHA-256 hash. SHA-256
   collisions are computationally infeasible in practice.

3. **FIFO eviction** — high-volume buckets drop old events. If you need to
   query all events matching a field across the full history, use off-chain
   indexing (see `bridge/relayer/`).

4. **Instance storage** — all buckets use instance storage (permanent, but
   charged per byte per ledger). Monitor storage costs for high-cardinality
   deployments.

---

## Tests

See `src/inverted_index_tests.rs` for 11 unit tests covering:

- Basic add and query
- Zero-entry query (empty vec)
- `index_get_count` consistency
- FIFO eviction at capacity
- Multiple independent field indexes
- Combined category+type index
- Submitter+type index
- Metadata field parsing (key=value)
- Empty metadata (no panic)
- Duplicate entries allowed
- FIFO removes exactly the oldest entries

Run tests:

```bash
cargo test inverted_index
```

---

## References

- [`src/inverted_index.rs`](../src/inverted_index.rs) — implementation
- [`src/inverted_index_tests.rs`](../src/inverted_index_tests.rs) — tests
- [Soroban storage guide](https://soroban.stellar.org/docs/fundamentals-and-concepts/persisting-data)
- `DataKey::EventTypeIndices` in [`src/lib.rs`](../src/lib.rs) — existing packed-bytes index pattern
