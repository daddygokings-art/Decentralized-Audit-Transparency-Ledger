# AuditLedger Python SDK

Python client library for the [AuditLedger](../..) Soroban smart contract on Stellar.

## Installation

```bash
pip install audit-ledger-sdk
```

For pandas and analytics support:

```bash
pip install "audit-ledger-sdk[pandas]"
pip install "audit-ledger-sdk[analytics]"
```

## Quickstart

```python
from audit_ledger import AuditLedgerClient

client = AuditLedgerClient(
    contract_id="CCXMTP7...",
    rpc_url="https://soroban-testnet.stellar.org",
)

total = client.total_events()
print(f"Events: {total}")

event = client.get_event_by_order(0)
print(f"First event: {event.event_type} by {event.submitter}")
```

## Usage

### Client

```python
# Write
client.log_event(submitter, "payment", b'{"amount": "100"}')

# Read
client.get_event_by_order(0)
client.get_event_by_type("payment", 0)

# Governance (owner-only)
client.set_global_max_logs(caller, 5000)
client.transfer_ownership(caller, new_owner)
```

### Pandas Integration

```python
from audit_ledger import pandas as al_pd

df = al_pd.load_all_events(client)
df["event_type"].value_counts().plot(kind="bar")
```

### Analytics

```python
from audit_ledger import analytics

rate = analytics.event_rate(events)
top = analytics.top_submitters(events, n=5)
dist = analytics.event_distribution(events)
stats = analytics.metadata_stats(events)
```

## Development

```bash
pip install -e ".[dev]"
pytest tests/
black audit_ledger/
ruff check audit_ledger/
```

## Publishing to PyPI

```bash
pip install build twine
python -m build
twine upload dist/*
```
