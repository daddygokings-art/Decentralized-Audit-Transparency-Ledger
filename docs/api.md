# AuditLedger API Reference

Complete reference for every public function in the AuditLedger Soroban contract.

---

## Data Types

```rust
pub struct Event {
    pub index: u32,               // sequential position (0-based)
    pub timestamp: u64,           // ledger timestamp at log time
    pub event_type: Symbol,       // caller-defined event category
    pub category: Symbol,         // optional hierarchical category
    pub submitter: Address,       // account that logged the event
    pub metadata: Bytes,          // opaque payload (max 1 KB by default)
    pub sub_event_type: Option<Symbol>,
    pub event_hash: BytesN<32>,   // SHA-256 of this event's fields
    pub prev_hash: BytesN<32>,    // SHA-256 of the previous event (chain link)
}

pub struct EventHeader {          // lightweight view without metadata
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
}
```

### ContractError variants

| Code | Name | Meaning |
|------|------|---------|
| 1 | `CallerNotOwner` | The caller is not the contract owner |
| 2 | `GlobalMaxLogsReached` | Total event cap is full |
| 3 | `EventTypeMaxLogsReached` | Per-type cap is full |
| 4 | `EventDoesNotExist` | No event found for the given ID |
| 5 | `EventTypeIndexOutOfBounds` | `type_index` exceeds the type's count |
| 6 | `NewOwnerIsZero` | Attempted to transfer ownership to the null account |
| 7 | `CapNotSet` | Operation requires a cap that has not been set |
| 8 | `MetadataTooLarge` | Metadata exceeds the configured size limit |
| 9 | `ContractNotInitialized` | Contract has not been initialized yet |
| 10 | `TotalEventsOverflow` | Adding events would overflow `u32` |
| 11 | `TimestampOutOfRange` | Event timestamp drifts too far from ledger time |
| 12 | `InvalidSignature` | Signature verification failed |
| 13 | `ContractPaused` | Writes are blocked while the contract is paused |
| 14 | `RateLimitExceeded` | Submitter has exceeded their per-ledger rate limit |
| 15 | `SameOwner` | New owner is the same as the current owner |
| 16 | `MaxLogsBelowCurrentCount` | New cap would be below the current event count |
| 17 | `CapAlreadyRemoved` | Cap was already removed and cannot be removed again |
| 18 | `CapNeverSet` | Attempted to remove a cap that was never set |

---

## Write Functions

### `initialize`

```rust
fn initialize(env: Env, owner: Address, global_max_logs: u32)
```

Initializes the contract. Must be called exactly once by the intended owner immediately after deployment.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `owner` | `Address` | The account that will have governance rights |
| `global_max_logs` | `u32` | Hard cap on the total number of events ever logged |

**Access control:** The `owner` address must sign the transaction (`owner.require_auth()`).

**Errors:** Reverts with `AlreadyInitialized` if called a second time.

**Rust example:**

```rust
#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AuditLedger);
    let client = AuditLedgerClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner, &100_000);
    assert_eq!(client.total_events(), 0);
}
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_SECRET" \
  --network testnet \
  -- \
  initialize \
  --owner "$OWNER_PUBLIC_KEY" \
  --global_max_logs 100000
```

---

### `log_event`

```rust
fn log_event(
    env: Env,
    submitter: Address,
    event_type: Symbol,
    metadata: Bytes,
    category: Option<Symbol>,
    sub_event_type: Option<Symbol>,
) -> BytesN<32>
```

Logs a single event and returns its content-addressed ID.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `submitter` | `Address` | Account submitting the event (must sign) |
| `event_type` | `Symbol` | Event category (e.g., `"payment"`, `"transfer"`) |
| `metadata` | `Bytes` | Opaque payload; max size enforced by contract config |
| `category` | `Option<Symbol>` | Optional hierarchical category (e.g., `"finance"`) |
| `sub_event_type` | `Option<Symbol>` | Optional sub-classification |

**Returns:** `BytesN<32>` — the SHA-256 event ID. Use this to retrieve the event later with `get_event`.

**Access control:** Any address; the `submitter` must sign. Rate limits apply if configured.

**Errors:** `ContractPaused`, `ContractNotInitialized`, `GlobalMaxLogsReached`, `EventTypeMaxLogsReached`, `MetadataTooLarge`, `RateLimitExceeded`

**Rust example:**

```rust
#[test]
fn test_log_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AuditLedger);
    let client = AuditLedgerClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    client.initialize(&owner, &1000);

    let metadata = Bytes::from_slice(&env, b"payment details");
    let id = client.log_event(
        &submitter,
        &Symbol::new(&env, "payment"),
        &metadata,
        &None,
        &None,
    );
    assert_eq!(client.total_events(), 1);
    let event = client.get_event(&id);
    assert_eq!(event.submitter, submitter);
}
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SUBMITTER_SECRET" \
  --network testnet \
  -- \
  log_event \
  --submitter "$SUBMITTER_PUBLIC_KEY" \
  --event_type "payment" \
  --metadata "7061796d656e74206465746169ls" \
  --category null \
  --sub_event_type null
```

---

### `log_events`

```rust
fn log_events(env: Env, events: Vec<(Address, Symbol, Bytes)>) -> Vec<u32>
```

Logs a batch of events atomically. All events succeed or none do.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `events` | `Vec<(Address, Symbol, Bytes)>` | List of `(submitter, event_type, metadata)` tuples |

**Returns:** `Vec<u32>` — sequential global indices of each logged event.

**Access control:** Each submitter in the batch must sign.

**Errors:** Same as `log_event`. If any event in the batch violates a cap or rate limit the entire batch reverts.

**Rust example:**

```rust
let batch = vec![
    &env,
    (submitter.clone(), Symbol::new(&env, "payment"), Bytes::from_slice(&env, b"tx1")),
    (submitter.clone(), Symbol::new(&env, "refund"),  Bytes::from_slice(&env, b"tx2")),
];
let indices = client.log_events(&batch);
assert_eq!(indices.len(), 2);
```

**CLI example:**

```bash
# Pass a JSON array of [submitter, event_type, metadata_hex] tuples
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SUBMITTER_SECRET" \
  --network testnet \
  -- \
  log_events \
  --events '[["G...", "payment", "deadbeef"], ["G...", "refund", "cafebabe"]]'
```

---

## Read Functions

### `total_events`

```rust
fn total_events(env: Env) -> u32
```

Returns the total number of events logged across all types.

**Access control:** Public — no signing required.

**Rust example:**

```rust
let count = client.total_events();
```

**CLI example:**

```bash
soroban contract invoke --id "$CONTRACT_ID" --network testnet -- total_events
```

---

### `get_event`

```rust
fn get_event(env: Env, id: BytesN<32>) -> Event
```

Retrieves a full event by its content-addressed ID.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `id` | `BytesN<32>` | SHA-256 event ID returned by `log_event` |

**Returns:** Full `Event` struct including metadata.

**Access control:** Public.

**Errors:** `EventDoesNotExist` if the ID is not found.

**Rust example:**

```rust
let event = client.get_event(&id);
println!("{:?}", event.metadata);
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  -- \
  get_event \
  --id "<32-byte-hex-id>"
```

---

### `get_event_by_order`

```rust
fn get_event_by_order(env: Env, order: u32) -> Event
```

Retrieves an event by its sequential insertion index (0-based).

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `order` | `u32` | Sequential index from `0` to `total_events() - 1` |

**Access control:** Public.

**Errors:** `EventDoesNotExist` if `order` is out of range.

---

### `get_event_header`

```rust
fn get_event_header(env: Env, id: BytesN<32>) -> EventHeader
```

Returns a lightweight event header (index, timestamp, event_type, submitter) without metadata — lower fee than `get_event`.

**Access control:** Public.

**Errors:** `EventDoesNotExist`.

---

### `get_event_metadata`

```rust
fn get_event_metadata(env: Env, id: BytesN<32>) -> Bytes
```

Returns only the raw metadata bytes for an event — useful in index-only emission mode.

**Access control:** Public.

**Errors:** `EventDoesNotExist`.

---

### `event_count`

```rust
fn event_count(env: Env, event_type: Symbol) -> u32
```

Returns the number of events logged for a specific event type.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `event_type` | `Symbol` | The event type to count |

**Access control:** Public. Unavailable in low-cost mode (returns error).

**Rust example:**

```rust
let count = client.event_count(&Symbol::new(&env, "payment"));
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  -- \
  event_count \
  --event_type "payment"
```

---

### `get_event_by_type`

```rust
fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event
```

Returns the Nth event of a given type (0-based within that type's sequence).

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `event_type` | `Symbol` | The event type to query |
| `type_index` | `u32` | 0-based index within this type's event sequence |

**Access control:** Public. Unavailable in low-cost mode.

**Errors:** `EventTypeIndexOutOfBounds` if `type_index >= event_count(event_type)`.

**Rust example:**

```rust
let first_payment = client.get_event_by_type(&Symbol::new(&env, "payment"), &0);
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  -- \
  get_event_by_type \
  --event_type "payment" \
  --type_index 0
```

---

### `list_events`

```rust
fn list_events(env: Env, offset: u32, limit: u32) -> Vec<Event>
```

Paginated list of all events in insertion order.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `offset` | `u32` | Starting index (0-based) |
| `limit` | `u32` | Maximum events to return |

**Access control:** Public.

---

### `verify_integrity`

```rust
fn verify_integrity(env: Env) -> bool
```

Verifies the SHA-256 hash chain across all events. Returns `true` if no tampering is detected.

**Access control:** Public.

---

## Governance Functions (Owner Only)

All governance functions require the `caller` to be the current owner and to sign the transaction.

### `set_global_max_logs`

```rust
fn set_global_max_logs(env: Env, caller: Address, new_max: u32)
```

Updates the global event cap.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `caller` | `Address` | Must be the current owner |
| `new_max` | `u32` | New global cap; must be ≥ current `total_events()` |

**Errors:** `CallerNotOwner`, `MaxLogsBelowCurrentCount`, `ContractPaused`

**Rust example:**

```rust
client.set_global_max_logs(&owner, &500_000);
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_SECRET" \
  --network testnet \
  -- \
  set_global_max_logs \
  --caller "$OWNER_PUBLIC_KEY" \
  --new_max 500000
```

---

### `set_event_max_logs`

```rust
fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32)
```

Sets (or updates) a per-event-type cap.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `caller` | `Address` | Must be the current owner |
| `event_type` | `Symbol` | The event type to cap |
| `new_max` | `u32` | Maximum events allowed for this type |

**Errors:** `CallerNotOwner`, `ContractPaused`

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_SECRET" \
  --network testnet \
  -- \
  set_event_max_logs \
  --caller "$OWNER_PUBLIC_KEY" \
  --event_type "payment" \
  --new_max 10000
```

---

### `remove_event_cap`

```rust
fn remove_event_cap(env: Env, caller: Address, event_type: Symbol)
```

Removes the per-event-type cap for the given type, allowing it to grow up to the global cap.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `caller` | `Address` | Must be the current owner |
| `event_type` | `Symbol` | The event type whose cap will be removed |

**Errors:** `CallerNotOwner`, `CapNeverSet`, `CapAlreadyRemoved`, `ContractPaused`

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_SECRET" \
  --network testnet \
  -- \
  remove_event_cap \
  --caller "$OWNER_PUBLIC_KEY" \
  --event_type "payment"
```

---

### `transfer_ownership`

```rust
fn transfer_ownership(env: Env, caller: Address, new_owner: Address)
```

Transfers governance rights to a new owner atomically. The previous owner loses all privileges immediately.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `caller` | `Address` | Must be the current owner |
| `new_owner` | `Address` | The new owner address; must not be the null account or the current owner |

**Errors:** `CallerNotOwner`, `NewOwnerIsZero`, `SameOwner`, `ContractPaused`

**Rust example:**

```rust
let new_owner = Address::generate(&env);
client.transfer_ownership(&owner, &new_owner);
// owner can no longer call governance functions
```

**CLI example:**

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_SECRET" \
  --network testnet \
  -- \
  transfer_ownership \
  --caller "$OWNER_PUBLIC_KEY" \
  --new_owner "$NEW_OWNER_PUBLIC_KEY"
```

---

### `pause` / `unpause`

```rust
fn pause(env: Env, caller: Address)
fn unpause(env: Env, caller: Address)
```

`pause` blocks all write operations (log_event, log_events, governance writes). `unpause` re-enables them.

**Access control:** Owner only.

**Errors:** `CallerNotOwner`

---

### `set_metadata_max_size`

```rust
fn set_metadata_max_size(env: Env, caller: Address, max_size: u32)
```

Sets the global metadata byte cap (default: 1024). Events with `metadata.len() > max_size` are rejected.

**Access control:** Owner only.

---

### `set_event_emission_mode`

```rust
fn set_event_emission_mode(env: Env, caller: Address, mode: u32)
```

Controls what data is emitted as Soroban events. See [ADR-005](adr/ADR-005-event-emission.md) for details.

| Mode | Emitted data |
|------|--------------|
| `0` | Full: `(event_type, submitter, index, metadata)` |
| `1` | Index-only: `(event_type, submitter, index)` |
| `2` | Hash-only: `(event_type, submitter, index, sha256(metadata))` |
| `3` | Silent: nothing emitted |

**Access control:** Owner only.

---

### `upgrade_contract`

```rust
fn upgrade_contract(env: Env, caller: Address, new_wasm_hash: BytesN<32>)
```

Upgrades the contract to a new WASM binary. All contract state is preserved.

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `caller` | `Address` | Must be the current owner |
| `new_wasm_hash` | `BytesN<32>` | Hash returned by `soroban contract install` |

**Access control:** Owner only.

See [docs/upgrade-guide.md](upgrade-guide.md) for the full upgrade procedure.
