# Exception Reference

Full documentation for the `audit_ledger.exceptions` module (issue #249).

---

## Exception hierarchy

```
AuditLedgerError
├── ContractError
├── RPCError
│   ├── NetworkError
│   └── RateLimitError
├── ValidationError
│   └── MetadataTooLargeError
└── VerificationError
    ├── HashChainError
    ├── SignatureVerificationError
    └── EventIDMismatchError
```

---

## Base class

### `AuditLedgerError(message, context=None)`

All SDK exceptions inherit from this class.

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `message` | `str` | Human-readable error description. |
| `context` | `dict` | Structured error metadata. |

**Methods**

- `log(level=logging.ERROR)` — Emit a structured log record.

---

## Contract errors

### `ContractError(code, context=None)`

Raised when the Soroban contract returns an on-chain error code.

```python
from audit_ledger.exceptions import ContractError

try:
    client.set_global_max_logs("GNOT_OWNER", 100)
except ContractError as exc:
    print(exc.code)    # 1
    print(exc.name)    # "CallerNotOwner"
    print(exc.context) # {"error_code": 1, "error_name": "CallerNotOwner"}
```

**Error codes**

| Code | Name |
|------|------|
| 1 | `CallerNotOwner` |
| 2 | `GlobalMaxLogsReached` |
| 3 | `EventTypeMaxLogsReached` |
| 4 | `EventDoesNotExist` |
| 5 | `EventTypeIndexOutOfBounds` |
| 6 | `NewOwnerIsZero` |
| 7 | `CapNotSet` |
| 8 | `MetadataTooLarge` |
| 9 | `InvalidSignature` |
| 10 | `ContractPaused` |
| 11 | `RateLimitExceeded` |
| 14 | `NoEventsForType` |
| 15 | `AlreadyInitialized` |

---

## RPC errors

### `RPCError(message, context=None)`

Raised when a Soroban RPC call fails at the network or protocol level.

### `NetworkError(message, context=None)`

Subclass of `RPCError` for low-level connectivity failures (timeouts,
connection refused). Retryable by default with `with_retry`.

### `RateLimitError(message, retry_after=None, context=None)`

Raised when the RPC endpoint signals rate limiting.

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `retry_after` | `Optional[float]` | Seconds to wait before retrying. |

---

## Validation errors

### `ValidationError(message, field=None, context=None)`

Raised when input parameters fail client-side validation.

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `field` | `Optional[str]` | Name of the field that failed. |

### `MetadataTooLargeError(actual_size, max_size, context=None)`

Raised when metadata exceeds the configured size limit.

```python
from audit_ledger.exceptions import MetadataTooLargeError

raise MetadataTooLargeError(actual_size=8192, max_size=4096)
# MetadataTooLargeError: Metadata size 8192 bytes exceeds limit of 4096 bytes
```

---

## Verification errors

### `VerificationError(message, event_index=None, context=None)`

Base class for all client-side verification failures.

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `event_index` | `Optional[int]` | Index of the failing event. |

### `HashChainError`

Raised by `verify_hash_chain()` when a link in the hash chain is broken.

### `SignatureVerificationError`

Raised by `verify_event_signature()` when the Ed25519 signature is invalid.

### `EventIDMismatchError`

Raised by `verify_event_id()` when the recomputed event ID doesn't match.

---

## Retry decorator

### `with_retry(max_attempts=3, backoff_base=1.0, backoff_max=30.0, retryable=RETRYABLE_ERRORS)`

Decorator that retries a function on transient errors with exponential backoff.

```python
from audit_ledger.exceptions import with_retry, NetworkError

@with_retry(max_attempts=5, backoff_base=2.0)
def fetch_data():
    return client.total_events()
```

**Parameters**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_attempts` | `3` | Maximum total attempts. |
| `backoff_base` | `1.0` | Initial sleep time in seconds. |
| `backoff_max` | `30.0` | Maximum sleep time cap. |
| `retryable` | `(NetworkError, RateLimitError)` | Exception types that trigger a retry. |

---

## `log_and_raise(exc, level=logging.ERROR)`

Log an `AuditLedgerError` and re-raise it.

```python
from audit_ledger.exceptions import log_and_raise
import logging

try:
    client.total_events()
except NetworkError as exc:
    log_and_raise(exc, level=logging.WARNING)
```

---

## `RETRYABLE_ERRORS`

Tuple of exception classes that are safe to retry:

```python
RETRYABLE_ERRORS = (NetworkError, RateLimitError)
```
