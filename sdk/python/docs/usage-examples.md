# Python SDK Usage Examples

Practical code examples for common AuditLedger SDK patterns.

---

## Basic setup

```python
from audit_ledger import AuditLedgerClient

client = AuditLedgerClient(
    contract_id="CCXMTP7ABCDEF...",
    rpc_url="https://soroban-testnet.stellar.org",
    network_passphrase="Test SDF Network ; September 2015",
    # Optional: provide a signing keypair for write operations
    source_keypair="SXXXXX...",
)
```

---

## Logging events

### Single event

```python
event_id = client.log_event(
    submitter="GABCDEF...",
    event_type="payment",
    metadata=b'{"amount": "100", "currency": "USD"}',
)
print(f"Logged event ID: {event_id.hex()}")
```

### Batch events

```python
indices = client.log_events([
    {"submitter": "GA...", "event_type": "payment", "metadata": b'{"amount":"100"}'},
    {"submitter": "GA...", "event_type": "refund",  "metadata": b'{"amount":"50"}'},
])
print(f"Logged {len(indices)} events at indices {indices}")
```

### Signed event (with Ed25519 signature)

```python
import os

pubkey = bytes.fromhex("abc123...")  # 32-byte public key
signature = bytes.fromhex("def456...")  # 64-byte Ed25519 signature
signature_payload = pubkey + signature  # 96 bytes total

event_id = client.log_event_signed(
    submitter="GA...",
    event_type="secure_transfer",
    metadata=b'{"amount": "500"}',
    signature_payload=signature_payload,
)
```

---

## Reading events

### Single event by order

```python
event = client.get_event_by_order(0)
print(f"Type: {event.event_type}, Submitter: {event.submitter}")
```

### Single event by ID

```python
event = client.get_event(event_id)
print(f"Index: {event.index}, Timestamp: {event.timestamp}")
```

### Events by type

```python
count = client.event_count("payment")
for i in range(count):
    event = client.get_event_by_type("payment", i)
    print(event)
```

---

## Pagination

### Simple page fetch

```python
page = client.get_events(offset=0, limit=25)
print(f"Page 1 of {(page.total // 25) + 1}: {len(page.items)} events")
```

### Iterate all pages with `PageIterator`

```python
from audit_ledger.pagination import PageIterator

for page in PageIterator(client.get_events, limit=50):
    for event in page.items:
        print(f"[{event.index}] {event.event_type} by {event.submitter}")
```

### Iterate all items (flattened)

```python
from audit_ledger.pagination import iter_all_items

for event in iter_all_items(client.get_events, limit=100):
    print(event.event_type)
```

### Cursor-based navigation

```python
from audit_ledger.pagination import fetch_page_by_cursor

# First page
page = fetch_page_by_cursor(client.get_events, cursor=None, limit=20)
next_cursor = str(page.offset + len(page.items))

# Next page
page2 = fetch_page_by_cursor(client.get_events, cursor=next_cursor, limit=20)
```

### Total count caching

```python
from audit_ledger.pagination import TotalCountCache

cache = TotalCountCache(ttl_seconds=30)

# This calls total_events() once and caches the result for 30 seconds
total = cache.get_or_fetch(client.total_events)
total_again = cache.get_or_fetch(client.total_events)  # uses cache
```

---

## Error handling

### Catching contract errors

```python
from audit_ledger.exceptions import ContractError

try:
    client.set_global_max_logs(caller="GNOT_OWNER", new_max=999)
except ContractError as exc:
    print(f"Contract error #{exc.code}: {exc.name}")
    print(f"Context: {exc.context}")
```

### Catching all SDK errors

```python
from audit_ledger.exceptions import AuditLedgerError, RPCError, NetworkError

try:
    event = client.get_event_by_order(9999)
except ContractError as exc:
    print(f"Event not found: {exc}")
except NetworkError as exc:
    print(f"Network failure: {exc}")
except RPCError as exc:
    print(f"RPC failure: {exc}")
except AuditLedgerError as exc:
    print(f"SDK error: {exc}")
```

### Retry on transient network errors

```python
from audit_ledger.exceptions import with_retry, NetworkError

@with_retry(max_attempts=3, backoff_base=1.0)
def fetch_total():
    return client.total_events()

total = fetch_total()
```

### Logging errors

```python
from audit_ledger.exceptions import log_and_raise, NetworkError
import logging

try:
    client.total_events()
except NetworkError as exc:
    log_and_raise(exc, level=logging.WARNING)
```

---

## Event verification

### Verify a single event ID

```python
from audit_ledger.verification import verify_event_id

expected_id = bytes.fromhex("abc123...")
verify_event_id(event, contract_id="C...", expected_id=expected_id)
```

### Verify the hash chain

```python
from audit_ledger.verification import verify_hash_chain
from audit_ledger.exceptions import HashChainError

events = [client.get_event_by_order(i) for i in range(100)]
try:
    verify_hash_chain(events)
    print("Hash chain intact ✓")
except HashChainError as exc:
    print(f"Chain broken at index {exc.event_index}: {exc}")
```

### Generate an integrity proof

```python
from audit_ledger.verification import generate_integrity_proof

events = [client.get_event_by_order(i) for i in range(50)]
proof = generate_integrity_proof(events)

print(f"Events: {proof.event_count}")
print(f"Root hash: {proof.hex_root()}")
print(f"Chain valid: {proof.chain_valid}")
print(f"Overall valid: {proof.is_valid}")
print(proof.to_dict())
```

### Verify an Ed25519 signature

```python
from audit_ledger.verification import verify_event_signature
from audit_ledger.exceptions import SignatureVerificationError

try:
    verify_event_signature(event_id, pubkey=pubkey, signature=sig)
    print("Signature valid ✓")
except SignatureVerificationError as exc:
    print(f"Invalid signature: {exc}")
```

---

## Analytics

```python
from audit_ledger import analytics

events = [client.get_event_by_order(i) for i in range(500)]

rate = analytics.event_rate(events, time_unit="hour")
top  = analytics.top_submitters(events, n=5)
dist = analytics.event_distribution(events)
stat = analytics.metadata_stats(events)

print(f"Rate: {rate:.1f} events/hour")
print(f"Top submitters: {top}")
```

---

## Pandas integration

```python
from audit_ledger.pandas import load_all_events, load_events_by_type

# Load everything into a DataFrame
df = load_all_events(client)
print(df.head())

# Load only payment events
payments_df = load_events_by_type(client, "payment")
print(payments_df["submitter"].value_counts())
```

Requires `pip install "audit-ledger[pandas]"`.
