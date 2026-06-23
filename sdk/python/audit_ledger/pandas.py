"""Pandas integration for AuditLedger — load events into DataFrames."""

from __future__ import annotations

from typing import Optional

from .client import AuditLedgerClient

try:
    import pandas as pd
    PANDAS_AVAILABLE = True
except ImportError:
    PANDAS_AVAILABLE = False


def to_dataframe(events: list) -> "pd.DataFrame":
    """Convert a list of Event objects to a pandas DataFrame.

    Args:
        events: List of Event namedtuples/dataclasses.

    Returns:
        DataFrame with columns: index, timestamp, event_type, submitter,
        metadata_size, event_hash_hex, prev_hash_hex.
    """
    if not PANDAS_AVAILABLE:
        raise ImportError("pandas is required. Install with: pip install pandas")

    records = []
    for evt in events:
        records.append({
            "index": evt.index,
            "timestamp": evt.timestamp,
            "event_type": evt.event_type,
            "submitter": evt.submitter,
            "metadata_size": len(evt.metadata),
            "metadata_hex": evt.metadata.hex(),
            "event_hash_hex": evt.event_hash.hex(),
            "prev_hash_hex": evt.prev_hash.hex(),
        })
    df = pd.DataFrame(records)
    if not df.empty:
        df["datetime"] = pd.to_datetime(df["timestamp"], unit="s")
    return df


def load_all_events(client: AuditLedgerClient) -> "pd.DataFrame":
    """Load every event from the contract into a DataFrame.

    Args:
        client: An initialized AuditLedgerClient.

    Returns:
        DataFrame with all events.
    """
    total = client.total_events()
    events = []
    for i in range(total):
        events.append(client.get_event_by_order(i))
    return to_dataframe(events)


def load_events_by_type(
    client: AuditLedgerClient, event_type: str
) -> "pd.DataFrame":
    """Load events of a specific type into a DataFrame.

    Args:
        client: An initialized AuditLedgerClient.
        event_type: The event type symbol to filter by.

    Returns:
        DataFrame with events of the given type.
    """
    count = client.event_count(event_type)
    events = []
    for i in range(count):
        events.append(client.get_event_by_type(event_type, i))
    return to_dataframe(events)
