# ADR-004: Storage Key Design and DataKey Enum

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2024-01-01 |
| **Deciders** | Core team |

---

## Context

Soroban contract storage is a key-value map where keys are XDR-encoded values. Without a deliberate key design, two different logical pieces of data could produce the same encoded key (collision), causing one to silently overwrite the other.

The contract stores a heterogeneous set of data: global config, per-type indices, per-event payloads, governance state, rate-limit state, and more. A systematic approach to key naming is essential.

---

## Decision

All storage keys are variants of a single `DataKey` enum tagged with `#[contracttype]`. Each variant encodes its semantic context:

```rust
pub enum DataKey {
    Owner,
    Config,                          // global_max_logs + total_events combined
    Paused,
    EventData(BytesN<32>),          // event ID → full Event struct
    EventOrder(u32),                 // sequential index → event ID
    EventHeaderKey(BytesN<32>),     // event ID → lightweight EventHeader
    EventMetadata(BytesN<32>),      // event ID → raw Bytes metadata only
    EventTypeIndices(Symbol),        // event type → packed u32 indices
    EventTypeCount(Symbol),          // event type → cached count
    EventCapSet(Symbol),             // event type cap gate
    EventMaxLogs(Symbol),            // event type → cap value
    EventCapRemoved(Symbol),         // tombstone: cap was intentionally removed
    GlobalMetadataMaxSize,           // global metadata byte cap
    EventMetadataMaxSize(Symbol),   // per-type metadata byte cap
    SubmitterRateLimit(Address),    // submitter → max events per ledger
    SubmitterRateState(Address),    // submitter → (last_ts, count)
    SubmitterNonce(Address),         // submitter → last accepted nonce
    // ... (see src/lib.rs for full list)
}
```

### Key collision avoidance

Soroban encodes `#[contracttype]` enums with their variant index as a discriminant. Two variants with different names never produce the same encoded key, even if they hold the same inner value (e.g., `EventTypeCount(Symbol)` and `EventMaxLogs(Symbol)` with the same symbol are distinct keys).

### Tombstone variants

Variants like `GlobalMaxLogs` and `TotalEvents` are retained in the enum as tombstones even though they are no longer written. This prevents accidental key reuse if the enum is extended in the future.

---

## Consequences

**Positive:**
- Zero risk of key collision across different data categories.
- The enum serves as a self-documenting registry of all storage keys.
- Soroban's XDR encoding ensures variant + payload combinations are always unique.
- Tombstone variants document historical key usage and prevent accidental recycling.

**Negative:**
- Adding a new storage category requires modifying the `DataKey` enum, which must be done carefully to avoid renumbering existing variants (that would corrupt existing state).
- The enum grows with the contract; reading it requires understanding the full storage layout.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|-------------|----------------|
| String keys | Typos cause silent bugs; no compiler enforcement. |
| Flat struct keys (one key per concept) | Does not scale to per-type or per-address keys. |
| Nested maps | Soroban does not natively support nested maps; would require manual serialization. |
| Hash-based keys | Opaque; makes debugging and inspection very difficult. |
