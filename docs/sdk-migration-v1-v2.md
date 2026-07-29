# SDK Migration Guide: v1 → v2

This guide covers every breaking change introduced in the v2 AuditLedger contract API and
shows you exactly how to update your JavaScript and Python SDK calls.

---

## Table of Contents

- [Breaking Change Summary](#breaking-change-summary)
- [Changed Function Signatures](#changed-function-signatures)
  - [log_event](#log_event)
  - [log_events](#log_events)
  - [get_event](#get_event)
- [New Optional Fields](#new-optional-fields)
- [Updated Event Struct](#updated-event-struct)
- [JavaScript SDK Migration](#javascript-sdk-migration)
- [Python SDK Migration](#python-sdk-migration)
- [CLI Migration](#cli-migration)
- [Common Errors After Upgrading](#common-errors-after-upgrading)

---

## Breaking Change Summary

| Area | v1 | v2 | Impact |
|------|----|----|--------|
| `log_event` return type | `u32` (sequential index) | `BytesN<32>` (hex string) | **Breaking** — stored IDs are no longer integers |
| `log_event` parameter count | 3 args | 6 args (3 new optional) | **Breaking** — old call sites compile but contract rejects them at runtime |
| `log_events` return type | `Vec<u32>` | `Vec<BytesN<32>>` | **Breaking** — list of IDs is now hex strings |
| `get_event` parameter | `u32` index | `BytesN<32>` hex ID | **Breaking** — integer-based lookup removed |
| `Event` struct | 5 fields | 11 fields | Non-breaking for reads; new fields are populated by the contract |

---

## Changed Function Signatures

### log_event

The core write function gained three new parameters and changed its return type.

**v1 (Rust contract)**
```rust
fn log_event(
    env: Env,
    submitter: Address,
    event_type: Symbol,
    metadata: Bytes,
) -> u32
```

**v2 (Rust contract)**
```rust
fn log_event(
    env: Env,
    submitter: Address,
    event_type: Symbol,
    metadata: Bytes,
    category: Option<Symbol>,       // new — hierarchical category label
    sub_event_type: Option<Symbol>, // new — sub-classification of the event
    force: bool,                    // new — bypass deduplication when true
) -> BytesN<32>
```

Key differences:
- **Return type**: `u32` (sequential index) → `BytesN<32>` (content-addressed 32-byte ID, returned as a hex string over RPC).
- **`category`**: optional top-level classification (e.g., `"finance"`, `"compliance"`). Pass `null` / `None` to omit.
- **`sub_event_type`**: optional secondary type for hierarchical event trees. Pass `null` / `None` to omit.
- **`force`**: when `false` (the default), the contract deduplicates identical events and returns the existing ID instead of creating a duplicate. Pass `true` to always store a new event.

### log_events

**v1** returned `Vec<u32>` (list of sequential indices).  
**v2** returns `Vec<BytesN<32>>` (list of hex-encoded content-addressed IDs).

The input shape is unchanged — each element is still `(submitter, event_type, metadata)`.
The new optional fields (`category`, `sub_event_type`, `force`) default to `null / false` for
all events in a batch; use individual `log_event` calls if you need per-event overrides.

### get_event

**v1** accepted a `u32` index.  
**v2** accepts a `BytesN<32>` hex string ID.

```bash
# v1
get_event --index 42

# v2
get_event --id "a1b2c3d4..."
```

Use `get_event_by_order` if you need index-based access (that function is unchanged).

---

## New Optional Fields

All three new `log_event` parameters are optional. Passing `null` (JS) or `None` (Python)
for `category` and `sub_event_type`, and `false` for `force`, is exactly equivalent to a
v1 call — the contract behaviour is identical.

| Parameter | Type | Default | Purpose |
|-----------|------|---------|---------|
| `category` | `Option<Symbol>` | `null` / `None` | Top-level event classification |
| `sub_event_type` | `Option<Symbol>` | `null` / `None` | Secondary event sub-classification |
| `force` | `bool` | `false` | Force store even if a duplicate exists |

Symbols are short strings (≤ 32 bytes). Examples: `"finance"`, `"compliance"`, `"kyc"`.

---

## Updated Event Struct

The `Event` struct returned by `get_event`, `get_event_by_type`, and `get_event_by_order`
has grown from 5 to 11 fields:

| Field | v1 | v2 | Notes |
|-------|----|----|-------|
| `index` | ✓ | ✓ | Sequential 0-based position |
| `timestamp` | ✓ | ✓ | Ledger timestamp (Unix seconds) |
| `event_type` | ✓ | ✓ | |
| `submitter` | ✓ | ✓ | |
| `metadata` | ✓ | ✓ | |
| `category` | — | ✓ | Empty `Symbol` when not set |
| `sub_event_type` | — | ✓ | `None` when not set |
| `version` | — | ✓ | Schema version; currently `1` |
| `event_hash` | — | ✓ | SHA-256 of this event's fields |
| `prev_hash` | — | ✓ | SHA-256 of the previous event; all-zeros for genesis |
| `parent_event_id` | — | ✓ | `None` unless this event is a child |

Code that destructures `Event` by position will break. Destructure by field name instead.

---

## JavaScript SDK Migration

The JS SDK (`AuditLedgerClient`) exposes `logEvent(submitter, eventType, metadata)`.
The internal call to the contract now passes the three new optional parameters automatically
with safe defaults. **No code change is required for the basic 3-argument call.**

However, the **return value changed** from a `number` to a `string` (hex-encoded 32-byte ID).
Any code that treats the return value as an integer must be updated.

### Minimal migration (return value only)

```ts
// v1 — return value was a number
const index: number = await client.logEvent(submitter, eventType, metadata);
console.log(`Logged event at index ${index}`);

// v2 — return value is now a hex string ID
const id: string = await client.logEvent(submitter, eventType, metadata);
console.log(`Logged event with ID ${id}`);
```

### Using the new optional parameters

```ts
// v2 — with category and sub_event_type
const id = await client.logEvent(
  'GABC123...', // submitter
  'payment',    // eventType
  metadata,     // metadata (string or Buffer)
  // Optional v2 parameters — not yet exposed as named args in the SDK;
  // pass via the low-level transport if needed:
);

// Low-level call with all v2 parameters
const id = await client.callTransport('log_event', [
  submitter,
  'payment',
  metadata,
  'finance',      // category
  'wire-transfer', // sub_event_type
  false,          // force
]);
```

> **Note:** Named `category`, `subEventType`, and `force` arguments will be added to
> `AuditLedgerClient.logEvent()` in a future SDK release. Until then, use the
> low-level `callTransport` approach shown above for events that need these fields.

### Retrieving an event by ID

```ts
// v1 — get event by integer index (no longer supported directly)
// const event = await client.getEvent(42); // ❌ removed

// v2 — get event by hex ID returned from logEvent
const event = await client.getEvent(id);

// v2 — get event by sequential order index (unchanged)
const event = await client.getEventByOrder(42);
```

### Handling the new Event fields

```ts
import type { Event } from '@audit-ledger/sdk';

const event: Event = await client.getEvent(id);

// New fields available in v2:
console.log(event.event_hash);      // hex string — SHA-256 of this event
console.log(event.prev_hash);       // hex string — SHA-256 of previous event
console.log(event.version);         // number — schema version
console.log(event.category);        // string | null
console.log(event.sub_event_type);  // string | null
console.log(event.parent_event_id); // string | null
```

### logEvents return type

```ts
// v1 — returned number[]
const indices: number[] = await client.logEvents(events);

// v2 — returns string[] (hex IDs)
const ids: string[] = await client.logEvents(events);
```

---

## Python SDK Migration

The Python SDK (`AuditLedgerClient.log_event`) signature is unchanged at the Python level —
it still accepts `(submitter, event_type, metadata)`. The SDK passes safe defaults for the new
parameters. **No code change is required for the basic 3-argument call.**

The **return type changed** from `int` to `bytes` (32-byte content-addressed ID).

### Minimal migration (return value only)

```python
from audit_ledger import AuditLedgerClient

client = AuditLedgerClient(
    contract_id="CCXMTP7...",
    rpc_url="https://soroban-testnet.stellar.org",
    network_passphrase="Test SDF Network ; September 2015",
)

# v1 — returned an int
index: int = client.log_event(submitter, event_type, metadata)
print(f"Logged event at index {index}")

# v2 — returns bytes (32-byte ID)
event_id: bytes = client.log_event(submitter, event_type, metadata)
print(f"Logged event with ID {event_id.hex()}")
```

### Using the new optional parameters

```python
# v2 — with category and sub_event_type
# These are not yet keyword arguments on the high-level method.
# Use the low-level _invoke() for full control:
result = client._invoke("log_event", {
    "submitter": submitter,
    "event_type": "payment",
    "metadata": base64.b64encode(metadata).decode(),
    "category": "finance",          # new in v2 — omit or pass None to skip
    "sub_event_type": "wire-transfer",  # new in v2 — omit or pass None to skip
    "force": False,                 # new in v2 — False = deduplicate (default)
})
event_id = bytes.fromhex(result)
```

> **Note:** Named `category`, `sub_event_type`, and `force` keyword arguments will be
> added to `AuditLedgerClient.log_event()` in a future SDK release.

### Retrieving an event by ID

```python
# v1 — get event by integer index
# event = client.get_event(42)  # ❌ signature changed

# v2 — get event by bytes ID returned from log_event
event = client.get_event(event_id)           # event_id is bytes

# v2 — get event by sequential order index (unchanged)
event = client.get_event_by_order(42)
```

### Handling the new Event fields

```python
from audit_ledger.models import Event

event: Event = client.get_event(event_id)

# New fields available in v2:
print(event.event_hash)       # bytes — SHA-256 of this event
print(event.prev_hash)        # bytes — SHA-256 of previous event
print(event.version)          # int — schema version (currently 1)
print(event.category)         # str | None
print(event.sub_event_type)   # str | None
print(event.parent_event_id)  # bytes | None
```

### log_events return type

```python
# v1 — returned List[int]
indices: list[int] = client.log_events(events)

# v2 — returns List[bytes] (32-byte IDs)
ids: list[bytes] = client.log_events(events)
```

---

## CLI Migration

```bash
# v1 — log_event returned a u32 index
soroban contract invoke ... -- log_event \
  --submitter $ADDR \
  --event_type payment \
  --metadata "dGVzdA=="
# output: 42

# v2 — log_event returns a 32-byte hex ID; new required params added
soroban contract invoke ... -- log_event \
  --submitter $ADDR \
  --event_type payment \
  --metadata "dGVzdA==" \
  --category null \          # pass null to omit
  --sub_event_type null \    # pass null to omit
  --force false
# output: "a1b2c3d4e5f6..."

# v2 — get_event now takes --id instead of --index
soroban contract invoke ... -- get_event \
  --id "a1b2c3d4e5f6..."

# Unchanged: get_event_by_order still uses --order_index
soroban contract invoke ... -- get_event_by_order --order_index 42
```

---

## Common Errors After Upgrading

### `HostError` / argument count mismatch

**Cause:** The contract now requires 6 arguments for `log_event`; calling with 3 will fail.

**Fix:** Add the three new arguments (`category`, `sub_event_type`, `force`). Pass `null`/`None`/`false` to preserve v1 behaviour.

### Type error — expected string, got number

**Cause:** Code stores the `log_event` return value as a `number` / `int`, but it is now a hex `string` / `bytes`.

**Fix:** Update all storage and comparisons of event IDs to use string/bytes types.

### `EventDoesNotExist` when calling `get_event`

**Cause:** Passing an old integer index to `get_event`, which now expects a 32-byte hex ID.

**Fix:** Use `get_event_by_order(index)` for index-based lookups, or retrieve the hex ID from `log_event` and store it.

### Deduplication surprise — same call returns an old ID

**Cause:** By default (`force: false`) the contract deduplicates events with identical `(event_type, submitter, metadata)`. If you log the same payload twice you get the same ID.

**Fix:** Pass `force: true` if you need to store duplicate events (e.g., repeated heartbeat events with identical metadata).

---

## Quick Reference

| Action | v1 | v2 |
|--------|----|----|
| Log an event | `logEvent(s, t, m)` → `number` | `logEvent(s, t, m)` → `string` |
| Log with category | not supported | `callTransport('log_event', [s,t,m,'finance',null,false])` |
| Get event by ID | `getEvent(42)` (integer) | `getEvent("a1b2c3...")` (hex string) |
| Get event by index | `getEvent(42)` | `getEventByOrder(42)` |
| Batch log | returns `number[]` | returns `string[]` |
| Event fields | 5 | 11 (6 new) |
