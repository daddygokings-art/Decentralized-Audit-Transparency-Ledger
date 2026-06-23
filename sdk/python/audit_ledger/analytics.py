"""Analytics module for AuditLedger event data.

Provides statistical analysis of event activity:
- event_rate: events per unit time
- top_submitters: most active submitter addresses
- event_distribution: breakdown by event type
- metadata_stats: metadata size statistics
"""

from __future__ import annotations

from collections import Counter
from typing import List, Tuple

from .models import Event


def event_rate(
    events: List[Event], time_unit: str = "hour"
) -> float:
    """Calculate the average event rate.

    Args:
        events: List of Event objects.
        time_unit: One of "second", "minute", "hour", "day".

    Returns:
        Average number of events per time unit.
    """
    if len(events) < 2:
        return 0.0

    timestamps = sorted(e.timestamp for e in events)
    span_seconds = timestamps[-1] - timestamps[0]
    if span_seconds <= 0:
        return float(len(events))

    unit_seconds = {"second": 1, "minute": 60, "hour": 3600, "day": 86400}
    divisor = unit_seconds.get(time_unit, 3600)
    return len(events) / (span_seconds / divisor)


def top_submitters(events: List[Event], n: int = 10) -> List[Tuple[str, int]]:
    """Return the N most active submitter addresses.

    Args:
        events: List of Event objects.
        n: Number of top submitters to return (default 10).

    Returns:
        List of (address, count) tuples, sorted desc by count.
    """
    counter = Counter(e.submitter for e in events)
    return counter.most_common(n)


def event_distribution(events: List[Event]) -> List[Tuple[str, int]]:
    """Return the distribution of events by event type.

    Args:
        events: List of Event objects.

    Returns:
        List of (event_type, count) tuples, sorted desc by count.
    """
    counter = Counter(e.event_type for e in events)
    return counter.most_common()


def metadata_stats(events: List[Event]) -> dict:
    """Compute metadata size statistics.

    Args:
        events: List of Event objects.

    Returns:
        Dict with keys: min_size, max_size, avg_size, total_bytes.
    """
    if not events:
        return {"min_size": 0, "max_size": 0, "avg_size": 0.0, "total_bytes": 0}

    sizes = [len(e.metadata) for e in events]
    return {
        "min_size": min(sizes),
        "max_size": max(sizes),
        "avg_size": sum(sizes) / len(sizes),
        "total_bytes": sum(sizes),
    }
