#!/usr/bin/env python3
"""AuditLedger SDK — Jupyter Quickstart Notebook (saved as .py for version control).

Open this in Jupyter::

    jupyter notebook examples/analysis.ipynb

Or convert to HTML::

    jupyter nbconvert --to html examples/analysis.ipynb
"""

# %% [markdown]
# # AuditLedger Event Analysis
#
# This notebook demonstrates how to use the AuditLedger Python SDK to load,
# analyze, and visualize on-chain audit events.

# %% Setup
import os
import sys

sys.path.insert(0, os.path.abspath(".."))

from audit_ledger import AuditLedgerClient
from audit_ledger import pandas as al_pd
from audit_ledger import analytics as al_analytics

# %% [markdown]
# ## 1. Connect to the contract
#
# Replace `CONTRACT_ID` with your deployed contract address.

# %%
CONTRACT_ID = os.getenv("AUDIT_LEDGER_CONTRACT", "YOUR_CONTRACT_ID")
RPC_URL = "https://soroban-testnet.stellar.org"

client = AuditLedgerClient(
    contract_id=CONTRACT_ID,
    rpc_url=RPC_URL,
)

# %% [markdown]
# ## 2. Load all events

# %%
total = client.total_events()
print(f"Total events on-chain: {total}")

df = al_pd.load_all_events(client)
print(f"Loaded {len(df)} events into DataFrame.")
df.head()

# %% [markdown]
# ## 3. Event type distribution

# %%
dist = al_analytics.event_distribution(
    [client.get_event_by_order(i) for i in range(min(total, 100))]
)
print("Event type distribution:")
for event_type, count in dist:
    print(f"  {event_type}: {count}")

# %% [markdown]
# ## 4. Top submitters

# %%
events_batch = [client.get_event_by_order(i) for i in range(min(total, 500))]
top = al_analytics.top_submitters(events_batch, n=5)
print("Top submitters:")
for addr, count in top:
    print(f"  {addr}: {count} events")

# %% [markdown]
# ## 5. Event rate

# %%
rate = al_analytics.event_rate(events_batch, time_unit="hour")
print(f"Event rate: {rate:.2f} events/hour")

# %% [markdown]
# ## 6. Metadata statistics

# %%
stats = al_analytics.metadata_stats(events_batch)
print(f"Metadata stats: {stats}")

# %% [markdown]
# ## 7. Visualize (if matplotlib is available)

# %%
try:
    import matplotlib.pyplot as plt

    if not df.empty and "event_type" in df.columns:
        df["event_type"].value_counts().plot(kind="bar", title="Events by Type")
        plt.ylabel("Count")
        plt.tight_layout()
        plt.show()
except ImportError:
    print("matplotlib not installed. Skipping visualization.")
