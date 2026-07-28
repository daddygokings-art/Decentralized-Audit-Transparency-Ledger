"""Generator-based event streaming for the AuditLedger Python SDK (#244).

Provides:
- :class:`StreamConfig`   — poll interval, batch size, filter, error policy.
- :class:`StreamError`    — raised when streaming encounters a fatal error.
- :func:`stream_events`   — generator yielding events in real time.
- :func:`stream_by_type`  — convenience wrapper filtered to one event type.

Usage::

    client = AuditLedgerClient(...)

    # Stream all events, resuming from index 50
    for event in stream_events(client, after_index=50):
        print(event)

    # Stream only "payment" events with a custom config
    cfg = StreamConfig(poll_interval_s=2.0, event_type_filter="payment")
    for event in stream_events(client, config=cfg):
        process(event)
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import (
    TYPE_CHECKING,
    Callable,
    Generator,
    List,
    Optional,
)

from .models import AuditLedgerError, Event, EventType

if TYPE_CHECKING:
    from .client import AuditLedgerClient

__all__ = [
    "StreamConfig",
    "StreamError",
    "stream_events",
    "stream_by_type",
]


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class StreamConfig:
    """Configuration for :func:`stream_events`.

    Attributes:
        poll_interval_s: Seconds to wait between polls when no new events
            are available.  Defaults to ``5.0``.
        batch_size: Maximum number of events to fetch per polling cycle.
            ``0`` means unlimited.
        event_type_filter: When set, only events whose ``event_type``
            equals this value are yielded.
        predicate: Optional callable ``(Event) -> bool`` for arbitrary
            filtering.  Applied after ``event_type_filter``.
        max_errors: Maximum number of consecutive RPC errors tolerated
            before :class:`StreamError` is raised.  ``0`` means unlimited
            retries.
        backoff_factor: Multiplier applied to ``poll_interval_s`` on each
            consecutive error (exponential back-off).
        max_backoff_s: Upper bound on back-off wait time.
    """

    poll_interval_s: float = 5.0
    batch_size: int = 0
    event_type_filter: Optional[EventType] = None
    predicate: Optional[Callable[[Event], bool]] = field(
        default=None, repr=False
    )
    max_errors: int = 10
    backoff_factor: float = 2.0
    max_backoff_s: float = 60.0


# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class StreamError(AuditLedgerError):
    """Raised by :func:`stream_events` when the error threshold is exceeded.

    Attributes:
        consecutive_errors: Number of consecutive RPC errors that triggered
            this exception.
        last_error: The underlying exception from the most recent failure.
    """

    def __init__(
        self,
        consecutive_errors: int,
        last_error: Optional[Exception] = None,
    ) -> None:
        self.consecutive_errors: int = consecutive_errors
        self.last_error: Optional[Exception] = last_error
        msg = (
            f"Stream aborted after {consecutive_errors} consecutive errors"
            + (f": {last_error}" if last_error else "")
        )
        super().__init__(msg)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _matches(event: Event, config: StreamConfig) -> bool:
    """Return ``True`` if *event* passes the configured filters."""
    if config.event_type_filter and event.event_type != config.event_type_filter:
        return False
    if config.predicate and not config.predicate(event):
        return False
    return True


# ---------------------------------------------------------------------------
# Public streaming functions
# ---------------------------------------------------------------------------


def stream_events(
    client: "AuditLedgerClient",
    after_index: int = 0,
    config: Optional[StreamConfig] = None,
) -> Generator[Event, None, None]:
    """Yield :class:`~audit_ledger.models.Event` objects as they are logged.

    The generator polls ``client.total_events()`` at ``config.poll_interval_s``
    intervals and fetches any events that have appeared since the last poll,
    yielding them one at a time in ascending order.

    The generator runs indefinitely — break out of the ``for`` loop or call
    ``generator.close()`` to stop it.

    Args:
        client: An initialised :class:`~audit_ledger.client.AuditLedgerClient`.
        after_index: Cursor — only events at position >= ``after_index`` are
            yielded.  Pass the last-seen index + 1 to resume a previous
            session.
        config: Optional :class:`StreamConfig`.  Defaults are used when
            omitted.

    Yields:
        :class:`~audit_ledger.models.Event` in chronological order.

    Raises:
        :class:`StreamError`: When ``config.max_errors > 0`` and that many
            consecutive RPC errors have occurred.

    Example::

        for event in stream_events(client, after_index=last_cursor):
            print(event.index, event.event_type)
            last_cursor = event.index + 1
    """
    cfg = config or StreamConfig()
    cursor: int = after_index
    consecutive_errors: int = 0
    last_error: Optional[Exception] = None
    current_wait: float = cfg.poll_interval_s

    while True:
        try:
            total: int = client.total_events()
            consecutive_errors = 0
            current_wait = cfg.poll_interval_s  # reset back-off on success
        except Exception as exc:  # noqa: BLE001
            consecutive_errors += 1
            last_error = exc
            if cfg.max_errors > 0 and consecutive_errors >= cfg.max_errors:
                raise StreamError(consecutive_errors, last_error) from exc
            # Exponential back-off
            current_wait = min(current_wait * cfg.backoff_factor, cfg.max_backoff_s)
            time.sleep(current_wait)
            continue

        fetched: int = 0
        while cursor < total:
            try:
                event: Event = client.get_event_by_order(cursor)
                consecutive_errors = 0
            except Exception as exc:  # noqa: BLE001
                consecutive_errors += 1
                last_error = exc
                if cfg.max_errors > 0 and consecutive_errors >= cfg.max_errors:
                    raise StreamError(consecutive_errors, last_error) from exc
                current_wait = min(
                    current_wait * cfg.backoff_factor, cfg.max_backoff_s
                )
                time.sleep(current_wait)
                break  # retry from cursor on next poll cycle

            cursor += 1
            fetched += 1

            if _matches(event, cfg):
                yield event

            if cfg.batch_size > 0 and fetched >= cfg.batch_size:
                break

        time.sleep(cfg.poll_interval_s)


def stream_by_type(
    client: "AuditLedgerClient",
    event_type: EventType,
    after_index: int = 0,
    config: Optional[StreamConfig] = None,
) -> Generator[Event, None, None]:
    """Yield events of a single type as they are logged.

    Convenience wrapper around :func:`stream_events` that sets
    ``config.event_type_filter`` to *event_type*.

    Args:
        client: An initialised :class:`~audit_ledger.client.AuditLedgerClient`.
        event_type: Only events with this ``event_type`` are yielded.
        after_index: Resume cursor (see :func:`stream_events`).
        config: Base :class:`StreamConfig`.  ``event_type_filter`` is
            overridden by *event_type*.

    Yields:
        :class:`~audit_ledger.models.Event` whose ``event_type`` matches.
    """
    merged = StreamConfig(
        poll_interval_s=(config.poll_interval_s if config else 5.0),
        batch_size=(config.batch_size if config else 0),
        event_type_filter=event_type,
        predicate=(config.predicate if config else None),
        max_errors=(config.max_errors if config else 10),
        backoff_factor=(config.backoff_factor if config else 2.0),
        max_backoff_s=(config.max_backoff_s if config else 60.0),
    )
    yield from stream_events(client, after_index=after_index, config=merged)


def collect_events(
    client: "AuditLedgerClient",
    after_index: int = 0,
    max_events: int = 100,
    config: Optional[StreamConfig] = None,
) -> List[Event]:
    """Collect up to *max_events* events and return them as a list.

    Unlike :func:`stream_events` this function does **not** run forever —
    it fetches existing events from *after_index* up to *max_events* and
    returns.

    Args:
        client: An initialised :class:`~audit_ledger.client.AuditLedgerClient`.
        after_index: Start cursor.
        max_events: Stop after collecting this many events.
        config: Optional :class:`StreamConfig` for filtering.

    Returns:
        List of :class:`~audit_ledger.models.Event`.
    """
    cfg = config or StreamConfig()
    result: List[Event] = []
    total: int = client.total_events()

    for i in range(after_index, total):
        if len(result) >= max_events:
            break
        event = client.get_event_by_order(i)
        if _matches(event, cfg):
            result.append(event)

    return result
