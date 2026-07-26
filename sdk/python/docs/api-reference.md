# Python SDK API Reference

Complete reference for all public classes, functions, and types in the
`audit_ledger` Python SDK.

---

## `audit_ledger.client`

### `AuditLedgerClient`

Primary client for the AuditLedger Soroban contract.

```python
class AuditLedgerClient(
    contract_id: str,
    rpc_url: str = "https://soroban-testnet.stellar.org",
    network_passphrase: str = "Test SDF Network ; September 2015",
    source_keypair: Optional[str] = None,
)
```

**Parameters**

| Name | Type | Description |
|------|------|-------------|
| `contract_id` | `str` | Stellar contract ID (starts with `C`). |
| `rpc_url` | `str` | Soroban RPC endpoint URL. |
| `network_passphrase` | `str` | Stellar network passphrase. |
| `source_keypair` | `Optional[str]` | Base58-encoded secret key for signing. |

**Raises** `ImportError` if `stellar-sdk` is not installed.

---

#### Write methods

##### `initialize(owner, global_max_logs)`

Initialize the contract. Must be called exactly once.

```python
client.initialize(owner="GOWNER...", global_max_logs=100_000)
```

##### `log_event(submitter, event_type, metadata) -> bytes`

Log a single event and return its 32-byte content-addressed ID.

```python
event_id = client.log_event("GSUBMITTER...", "payment", b'{"amount":"100"}')
```

##### `log_events(events) -> list[int]`

Log a batch of events. Returns sequential indices.

```python
indices = client.log_events([
    {"submitter": "GA...", "event_type": "payment", "metadata": b"..."},
    {"submitter": "GB...", "event_type": "refund",  "metadata": b"..."},
])
```

##### `log_event_signed(submitter, event_type, metadata, signature_payload) -> bytes`

Log an event with a 96-byte Ed25519 signature payload (32-byte pubkey + 64-byte sig).

---

#### Read methods

##### `total_events() -> int`

Return the total number of events on-chain.

##### `get_event(event_id: bytes) -> Event`

Retrieve an event by its 32-byte content-addressed ID.

##### `get_event_by_order(order: int) -> Event`

Retrieve an event by its sequential order index (0-based).

##### `event_count(event_type: str) -> int`

Return the number of events for a specific type.

##### `get_event_by_type(event_type: str, type_index: int) -> Event`

Retrieve an event by type and type-relative index.

##### `get_events(offset=0, limit=50) -> Page[Event]`

Return a paginated slice of all events.

```python
page = client.get_events(offset=0, limit=25)
print(page.total, len(page.items))
```

---

#### Governance methods (owner-only)

| Method | Description |
|--------|-------------|
| `set_global_max_logs(caller, new_max)` | Set global log cap. |
| `set_event_max_logs(caller, event_type, new_max)` | Set per-type log cap. |
| `remove_event_cap(caller, event_type)` | Remove per-type cap. |
| `transfer_ownership(caller, new_owner)` | Transfer ownership. |
| `set_metadata_max_size(caller, max_size)` | Set global metadata size cap. |
| `set_event_metadata_max_size(caller, event_type, max_size)` | Set per-type metadata cap. |
| `get_metadata_max_size(event_type)` | Get effective metadata cap. |

---

#### Utility methods

##### `AuditLedgerClient.compute_event_id(...) -> bytes` *(static)*

Recompute the content-addressed event ID off-chain.

```python
event_id = AuditLedgerClient.compute_event_id(
    contract_id="C...",
    submitter="G...",
    event_type="payment",
    metadata=b"data",
    timestamp=1_700_000_000,
    index=0,
)
```

##### `AuditLedgerClient.verify_signature(event_id, pubkey, signature) -> bool` *(static)*

Verify an Ed25519 signature using `stellar-sdk`.

---

## `audit_ledger.models`

### `Event`

```python
@dataclass
class Event:
    index: int
    timestamp: int
    event_type: str
    submitter: str
    metadata: bytes
    event_hash: bytes
    prev_hash: bytes
```

**Class methods**

- `Event.from_dict(d: dict) -> Event` — Deserialize from a raw RPC response dict.

---

### `Page[T]`

```python
@dataclass
class Page(Generic[T]):
    items: List[T]
    total: int
    offset: int
    limit: int
```

---

### Exceptions

| Class | Description |
|-------|-------------|
| `AuditLedgerError` | Base class for all SDK errors. |
| `ContractError` | On-chain contract error (carries `.code` and `.name`). |
| `RPCError` | Soroban RPC-level failure. |

See [`exceptions.md`](exceptions.md) for the full exception hierarchy.

---

## `audit_ledger.exceptions`

See [`exceptions.md`](exceptions.md).

---

## `audit_ledger.pagination`

See [`pagination.md`](pagination.md).

---

## `audit_ledger.verification`

See [`verification.md`](verification.md).

---

## `audit_ledger.analytics`

| Function | Description |
|----------|-------------|
| `event_rate(events, time_unit)` | Events per time unit. |
| `top_submitters(events, n)` | Top N submitter addresses. |
| `event_distribution(events)` | Breakdown by event type. |
| `metadata_stats(events)` | Min/max/avg metadata sizes. |

---

## `audit_ledger.pandas`

| Function | Description |
|----------|-------------|
| `to_dataframe(events)` | Convert a list of `Event` objects to a `DataFrame`. |
| `load_all_events(client)` | Load all on-chain events into a `DataFrame`. |
| `load_events_by_type(client, event_type)` | Load events of a specific type. |

Requires `pip install "audit-ledger[pandas]"`.
