# Python SDK Troubleshooting Guide

Solutions to common issues when using the AuditLedger Python SDK.

---

## Installation issues

### `ImportError: stellar-sdk is required`

**Cause:** The `stellar-sdk` package is not installed.

**Fix:**

```bash
pip install stellar-sdk
# or install the full SDK
pip install audit-ledger-sdk
```

### `ModuleNotFoundError: No module named 'audit_ledger'`

**Cause:** The package is not installed in the active environment.

**Fix:**

```bash
# From the sdk/python directory
pip install -e .
# or
pip install audit-ledger-sdk
```

### `ImportError: pandas is required`

**Cause:** pandas integration requires the `pandas` extra.

**Fix:**

```bash
pip install "audit-ledger-sdk[pandas]"
```

---

## Connection issues

### `RPCError: RPC call failed: Connection refused`

**Cause:** The RPC URL is unreachable.

**Fixes:**

1. Check the `rpc_url` parameter points to a running Soroban RPC node.
2. For testnet: use `https://soroban-testnet.stellar.org`.
3. For mainnet: use `https://mainnet.stellar.validationcloud.io/v1/<API_KEY>`.
4. Check your network/firewall allows outbound HTTPS on port 443.

### `RPCError: RPC call failed: network timeout`

**Cause:** The RPC request timed out.

**Fix:** Use `with_retry` to handle transient timeouts automatically:

```python
from audit_ledger.exceptions import with_retry

@with_retry(max_attempts=3, backoff_base=2.0)
def get_total():
    return client.total_events()
```

---

## Contract errors

### `ContractError #1: CallerNotOwner`

**Cause:** You are calling a governance method (e.g., `set_global_max_logs`)
from an address that is not the contract owner.

**Fix:** Use the owner's keypair as `source_keypair` and the owner's public key
as the `caller` argument.

### `ContractError #2: GlobalMaxLogsReached`

**Cause:** The contract has hit its global log cap.

**Fix:**

```python
# Owner must increase the cap first
client.set_global_max_logs(caller=owner_address, new_max=new_limit)
```

### `ContractError #4: EventDoesNotExist`

**Cause:** The event ID or order index you requested does not exist.

**Fix:** Check `client.total_events()` before indexing; use `get_events()` with
pagination to safely iterate the full range.

### `ContractError #8: MetadataTooLarge`

**Cause:** The metadata bytes exceed the configured size limit.

**Fix:**

```python
# Check the current limit
limit = client.get_metadata_max_size(event_type)
print(f"Metadata limit: {limit} bytes")

# Compress or truncate your metadata, or request a limit increase:
client.set_metadata_max_size(caller=owner, max_size=8192)
```

### `ContractError #9: InvalidSignature`

**Cause:** The 96-byte signature payload supplied to `log_event_signed` is
invalid (wrong key, wrong message, or wrong format).

**Fix:** Make sure you are signing the `event_id` (not the metadata) with the
32-byte public key corresponding to the submitter address, and concatenating
`pubkey (32 bytes) + signature (64 bytes)`.

### `ContractError #10: ContractPaused`

**Cause:** The contract has been paused by the owner.

**Fix:** Contact the contract owner to resume operations.

### `ContractError #15: AlreadyInitialized`

**Cause:** `initialize()` has been called more than once.

**Fix:** Skip `initialize()` on a contract that is already set up.

---

## Pagination issues

### `ValueError: Invalid cursor`

**Cause:** A cursor string was modified or corrupted between requests.

**Fix:** Treat cursors as opaque tokens — do not parse or modify them:

```python
# Correct: pass the cursor directly
page2 = fetch_page_by_cursor(client.get_events, cursor=previous_cursor, limit=20)
```

### `PageIterator` stops early

**Cause:** A page returned fewer items than expected because `total` changed
between fetches, or the final page is a partial page.

**Fix:** This is expected behaviour. The iterator stops when `page.items` is
empty. Process each page as it arrives rather than assuming a fixed item count.

---

## Verification failures

### `EventIDMismatchError`

**Cause:** The event's stored ID does not match the recomputed value. This
could indicate data corruption or a different hashing configuration.

**Fix:**

1. Confirm you are using the correct `contract_id`.
2. Confirm the timestamp and index values match the on-chain record.
3. Compare the raw metadata bytes — any encoding difference (e.g., base64 vs
   raw bytes) will cause a mismatch.

### `HashChainError: Hash chain broken at event index N`

**Cause:** An event's `prev_hash` does not match the preceding event's
`event_hash`. This indicates either missing events in your local sequence or
on-chain tampering.

**Fix:**

1. Ensure you are fetching events in ascending order without gaps.
2. Call `client.verify_integrity()` to run the on-chain verification.
3. Compare the on-chain result with your local verification to isolate the
   source of the discrepancy.

### `SignatureVerificationError`

**Cause:** The Ed25519 signature is invalid for the given event ID and public
key.

**Common causes:**

- Wrong `event_id` (the signed message must be the 32-byte event ID).
- Swapped `pubkey` and `signature` arguments.
- The signature was created over the metadata rather than the event ID.

---

## Logging / debugging

Enable debug logging to see all SDK operations:

```python
import logging
logging.basicConfig(level=logging.DEBUG)
logging.getLogger("audit_ledger").setLevel(logging.DEBUG)
```

Each `AuditLedgerError` carries a `.context` dict with structured metadata:

```python
try:
    client.set_global_max_logs("GNOT_OWNER", 100)
except ContractError as exc:
    print(exc.context)
    # {'error_code': 1, 'error_name': 'CallerNotOwner'}
```

---

## Performance issues

### Fetching large numbers of events is slow

**Cause:** Each `get_event_by_order()` call makes a separate RPC round-trip.

**Fix:** Use the largest `limit` that your use case allows:

```python
from audit_ledger.pagination import iter_all_items

for event in iter_all_items(client.get_events, limit=200):
    process(event)
```

Cache the total count to avoid repeated calls:

```python
from audit_ledger.pagination import TotalCountCache

cache = TotalCountCache(ttl_seconds=30)
total = cache.get_or_fetch(client.total_events)
```

---

## Still stuck?

- Open an issue at [GitHub Issues](https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger/issues).
- Include the full exception message, traceback, and the `.context` dict.
- Specify the SDK version (`pip show audit-ledger-sdk`) and Python version.
