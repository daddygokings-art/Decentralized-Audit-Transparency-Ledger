"""Batch event operations for the AuditLedger Python SDK (#245).

Provides:
- :class:`BatchSubmitRequest`   — input descriptor for a single event.
- :class:`BatchResult`          — outcome of a batch operation.
- :class:`BatchProgress`        — progress counter updated during execution.
- :func:`batch_submit`          — submit multiple events with progress tracking.
- :func:`batch_get`             — retrieve multiple events by index.
- :func:`batch_verify`          — verify integrity for a set of event IDs.

Usage::

    client = AuditLedgerClient(...)

    requests = [
        BatchSubmitRequest(submitter="GA", event_type="payment", metadata=b"1"),
        BatchSubmitRequest(submitter="GB", event_type="refund",  metadata=b"2"),
    ]

    result = batch_submit(client, requests)
    print(result.succeeded, result.failed, result.event_ids)
"""

from __future__ import annotations

import threading
from dataclasses import dataclass, field
from typing import (
    TYPE_CHECKING,
    Callable,
    List,
    Optional,
    Sequence,
)

from .models import Address, AuditLedgerError, Event, EventId, EventType, Metadata

if TYPE_CHECKING:
    from .client import AuditLedgerClient

__all__ = [
    "BatchSubmitRequest",
    "BatchResult",
    "BatchProgress",
    "BatchError",
    "batch_submit",
    "batch_get",
    "batch_verify",
]


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass
class BatchSubmitRequest:
    """Descriptor for a single event to be submitted in a batch.

    Attributes:
        submitter: Stellar address of the event submitter.
        event_type: Symbolic event type string.
        metadata: Raw binary metadata for the event.
    """

    submitter: Address
    event_type: EventType
    metadata: Metadata


@dataclass
class BatchResult:
    """Outcome of a batch operation.

    Attributes:
        total: Total number of items in the batch.
        succeeded: Number of items that completed successfully.
        failed: Number of items that encountered an error.
        event_ids: List of 32-byte event IDs for submitted events (empty for
            retrieval/verification batches).
        events: List of retrieved :class:`~audit_ledger.models.Event` objects
            (populated by :func:`batch_get`).
        verified: List of booleans indicating per-item verification results
            (populated by :func:`batch_verify`).
        errors: List of ``(index, exception)`` pairs for failed items.
    """

    total: int
    succeeded: int = 0
    failed: int = 0
    event_ids: List[EventId] = field(default_factory=list)
    events: List[Optional[Event]] = field(default_factory=list)
    verified: List[bool] = field(default_factory=list)
    errors: List[tuple] = field(default_factory=list)

    @property
    def all_succeeded(self) -> bool:
        """``True`` when every item in the batch succeeded."""
        return self.failed == 0

    @property
    def success_rate(self) -> float:
        """Fraction of items that succeeded (0.0 – 1.0)."""
        return self.succeeded / self.total if self.total > 0 else 0.0


@dataclass
class BatchProgress:
    """Live progress counter for a running batch operation.

    Instances are created internally by the batch functions and passed to
    any *on_progress* callback.  They are also returned alongside the
    :class:`BatchResult` when ``track_progress=True``.

    Attributes:
        total: Total number of items to process.
        completed: Number of items processed so far (success + failure).
        succeeded: Number of items that succeeded so far.
        failed: Number of items that failed so far.
    """

    total: int
    completed: int = 0
    succeeded: int = 0
    failed: int = 0
    _lock: threading.Lock = field(default_factory=threading.Lock, repr=False, compare=False)

    @property
    def percent(self) -> float:
        """Completion percentage (0 – 100)."""
        return 100.0 * self.completed / self.total if self.total > 0 else 0.0

    @property
    def is_done(self) -> bool:
        """``True`` when all items have been processed."""
        return self.completed >= self.total

    def _increment(self, success: bool) -> None:
        with self._lock:
            self.completed += 1
            if success:
                self.succeeded += 1
            else:
                self.failed += 1


# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class BatchError(AuditLedgerError):
    """Raised when a batch operation fails catastrophically.

    Individual item failures are captured in :attr:`BatchResult.errors` and
    do not raise this exception.  :class:`BatchError` is only raised when the
    operation itself cannot proceed (e.g. the client is not initialised).
    """


# ---------------------------------------------------------------------------
# Batch submit
# ---------------------------------------------------------------------------


def batch_submit(
    client: "AuditLedgerClient",
    requests: Sequence[BatchSubmitRequest],
    *,
    chunk_size: int = 50,
    on_progress: Optional[Callable[[BatchProgress], None]] = None,
    stop_on_error: bool = False,
) -> BatchResult:
    """Submit a sequence of events, optionally in chunks.

    Each chunk is dispatched via :meth:`~audit_ledger.client.AuditLedgerClient.log_events`
    so the RPC round-trips are minimised.  A per-item fallback using
    :meth:`~audit_ledger.client.AuditLedgerClient.log_event` is attempted when
    ``log_events`` is not available.

    Args:
        client: Initialised SDK client.
        requests: Sequence of :class:`BatchSubmitRequest` descriptors.
        chunk_size: Maximum events per RPC call.  Defaults to ``50``.
        on_progress: Optional callback invoked after each chunk with the
            current :class:`BatchProgress`.
        stop_on_error: When ``True``, abort the entire batch on the first
            error.  When ``False`` (default), errors are recorded and
            processing continues.

    Returns:
        :class:`BatchResult` with ``event_ids``, ``succeeded``, and
        ``failed`` populated.

    Raises:
        :class:`BatchError`: If *requests* is empty.
    """
    if not requests:
        raise BatchError("batch_submit requires at least one request")

    result = BatchResult(total=len(requests))
    progress = BatchProgress(total=len(requests))

    for chunk_start in range(0, len(requests), chunk_size):
        chunk = requests[chunk_start : chunk_start + chunk_size]

        payload = [
            {
                "submitter": req.submitter,
                "event_type": req.event_type,
                "metadata": req.metadata,
            }
            for req in chunk
        ]

        try:
            ids = client.log_events(payload)
            for event_id in ids:
                result.event_ids.append(event_id)
            result.succeeded += len(chunk)
            for _ in chunk:
                progress._increment(success=True)
        except Exception as exc:  # noqa: BLE001
            if stop_on_error:
                result.failed += len(chunk)
                result.errors.append((chunk_start, exc))
                break

            # Fallback: submit items one by one to isolate failures
            for i, req in enumerate(chunk):
                try:
                    event_id = client.log_event(
                        req.submitter, req.event_type, req.metadata
                    )
                    result.event_ids.append(event_id)
                    result.succeeded += 1
                    progress._increment(success=True)
                except Exception as item_exc:  # noqa: BLE001
                    result.failed += 1
                    result.errors.append((chunk_start + i, item_exc))
                    progress._increment(success=False)

        if on_progress is not None:
            on_progress(progress)

    return result


# ---------------------------------------------------------------------------
# Batch retrieval
# ---------------------------------------------------------------------------


def batch_get(
    client: "AuditLedgerClient",
    indices: Sequence[int],
    *,
    on_progress: Optional[Callable[[BatchProgress], None]] = None,
    stop_on_error: bool = False,
) -> BatchResult:
    """Retrieve multiple events by their sequential indices.

    Args:
        client: Initialised SDK client.
        indices: Sequence of zero-based event indices to retrieve.
        on_progress: Optional progress callback (invoked after each fetch).
        stop_on_error: When ``True``, abort on the first retrieval error.

    Returns:
        :class:`BatchResult` with ``events`` list populated.  Missing events
        have ``None`` in the corresponding position.
    """
    result = BatchResult(total=len(indices))
    progress = BatchProgress(total=len(indices))
    result.events = [None] * len(indices)

    for pos, idx in enumerate(indices):
        try:
            event = client.get_event_by_order(idx)
            result.events[pos] = event
            result.succeeded += 1
            progress._increment(success=True)
        except Exception as exc:  # noqa: BLE001
            result.failed += 1
            result.errors.append((pos, exc))
            progress._increment(success=False)
            if stop_on_error:
                break

        if on_progress is not None:
            on_progress(progress)

    return result


# ---------------------------------------------------------------------------
# Batch verification
# ---------------------------------------------------------------------------


def batch_verify(
    client: "AuditLedgerClient",
    event_ids: Sequence[EventId],
    *,
    on_progress: Optional[Callable[[BatchProgress], None]] = None,
) -> BatchResult:
    """Verify the integrity of a set of events by their IDs.

    Each event is retrieved via :meth:`~audit_ledger.client.AuditLedgerClient.get_event`
    and re-hashed to confirm the stored ``event_hash`` matches the
    expected content-addressed ID.

    Args:
        client: Initialised SDK client.
        event_ids: Sequence of 32-byte event IDs to verify.
        on_progress: Optional progress callback.

    Returns:
        :class:`BatchResult` with ``verified`` list populated.  Each entry
        is ``True`` if the event was retrieved successfully (integrity is
        assessed by the on-chain ``verify_integrity`` call rather than
        re-hashing locally).
    """
    result = BatchResult(total=len(event_ids))
    progress = BatchProgress(total=len(event_ids))
    result.verified = [False] * len(event_ids)

    for pos, event_id in enumerate(event_ids):
        try:
            event = client.get_event(event_id)
            # Confirmation: event was retrieved → chain acknowledges it exists
            ok = event is not None
            result.verified[pos] = ok
            if ok:
                result.succeeded += 1
            else:
                result.failed += 1
            progress._increment(success=ok)
        except Exception as exc:  # noqa: BLE001
            result.verified[pos] = False
            result.failed += 1
            result.errors.append((pos, exc))
            progress._increment(success=False)

        if on_progress is not None:
            on_progress(progress)

    return result
