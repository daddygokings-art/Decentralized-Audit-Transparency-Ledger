# Python SDK Tutorial

Step-by-step guide to building an audit trail application with the
AuditLedger Python SDK.

---

## Prerequisites

- Python 3.9 or later
- A deployed AuditLedger contract (or testnet access)
- Stellar testnet account and funded keypair

```bash
pip install audit-ledger-sdk
# For analytics and DataFrames:
pip install "audit-ledger-sdk[pandas]"
```

---

## Step 1 — Connect to the contract

```python
import os
from audit_ledger import AuditLedgerClient

CONTRACT_ID = os.environ["AUDIT_LEDGER_CONTRACT"]
SECRET_KEY   = os.environ["STELLAR_SECRET_KEY"]

client = AuditLedgerClient(
    contract_id=CONTRACT_ID,
    rpc_url="https://soroban-testnet.stellar.org",
    network_passphrase="Test SDF Network ; September 2015",
    source_keypair=SECRET_KEY,
)

print(f"Connected. Total events: {client.total_events()}")
```

---

## Step 2 — Log your first event

```python
import json

metadata = json.dumps({
    "action": "user_login",
    "user_id": "u-42",
    "ip": "192.0.2.1",
}).encode()

event_id = client.log_event(
    submitter=os.environ["STELLAR_PUBLIC_KEY"],
    event_type="auth",
    metadata=metadata,
)

print(f"Event logged. ID: {event_id.hex()}")
```

---

## Step 3 — Read events back

```python
# By sequential order
event = client.get_event_by_order(0)
print(f"First event: {event.event_type} at {event.timestamp}")

# By content-addressed ID
same_event = client.get_event(event_id)
assert same_event.index == event.index

# By type
auth_count = client.event_count("auth")
print(f"Total auth events: {auth_count}")
```

---

## Step 4 — Paginate through all events

```python
from audit_ledger.pagination import PageIterator

all_events = []
for page in PageIterator(client.get_events, limit=50):
    all_events.extend(page.items)
    print(f"Page {page.offset // 50 + 1}: {len(page.items)} events fetched")

print(f"Total fetched: {len(all_events)}")
```

---

## Step 5 — Verify the audit trail

### Verify a single event ID

```python
from audit_ledger.verification import verify_event_id, compute_event_id
from audit_ledger.exceptions import EventIDMismatchError

expected = compute_event_id(
    contract_id=CONTRACT_ID,
    submitter=event.submitter,
    event_type=event.event_type,
    metadata=event.metadata,
    timestamp=event.timestamp,
    index=event.index,
)

try:
    verify_event_id(event, CONTRACT_ID, expected)
    print("Event ID verified ✓")
except EventIDMismatchError as exc:
    print(f"Event ID mismatch: {exc}")
```

### Verify the full hash chain

```python
from audit_ledger.verification import verify_hash_chain
from audit_ledger.exceptions import HashChainError

try:
    verify_hash_chain(all_events)
    print(f"Hash chain verified across {len(all_events)} events ✓")
except HashChainError as exc:
    print(f"Chain broken at index {exc.event_index}: {exc}")
```

### Generate an integrity proof

```python
from audit_ledger.verification import generate_integrity_proof

proof = generate_integrity_proof(all_events)
if proof.is_valid:
    print(f"Integrity proof valid ✓  Root: {proof.hex_root()}")
else:
    print(f"Integrity proof FAILED: {proof.errors}")
```

---

## Step 6 — Add retry logic for production

```python
from audit_ledger.exceptions import with_retry, NetworkError, RateLimitError

@with_retry(max_attempts=5, backoff_base=2.0, backoff_max=60.0)
def safe_total():
    return client.total_events()

total = safe_total()
```

---

## Step 7 — Analyse event data (optional, requires pandas)

```python
from audit_ledger.pandas import load_all_events
from audit_ledger import analytics

df = load_all_events(client)
print(df.groupby("event_type").size())

rate = analytics.event_rate(all_events, time_unit="hour")
top  = analytics.top_submitters(all_events, n=10)
print(f"Rate: {rate:.1f} events/hour")
print("Top submitters:", top)
```

---

## Step 8 — Governance (owner only)

```python
owner_public = os.environ["STELLAR_PUBLIC_KEY"]

# Raise the global log cap
client.set_global_max_logs(caller=owner_public, new_max=500_000)

# Cap the "debug" event type at 1 000 entries
client.set_event_max_logs(caller=owner_public, event_type="debug", new_max=1_000)

# Remove the cap on a high-value type
client.remove_event_cap(caller=owner_public, event_type="payment")
```

---

## Summary

You have learned how to:

1. Connect to the AuditLedger Soroban contract.
2. Log individual and batch events.
3. Read events by order, ID, and type.
4. Paginate through large event sets.
5. Verify event IDs, hash chains, and signatures.
6. Generate an integrity proof for a sequence of events.
7. Use retry logic for robust production integrations.
8. Analyse event data with pandas.

**Next steps:**

- Explore the [API Reference](api-reference.md) for full method signatures.
- Read the [Troubleshooting Guide](troubleshooting.md) for common issues.
- Check [usage-examples.md](usage-examples.md) for more code snippets.
