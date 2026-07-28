"""AuditLedger Python SDK — Pagination helpers.

Issue #248: Add cursor-based pagination, page iteration, total count caching,
and pagination state management.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import (
    Callable,
    Generic,
    Iterator,
    List,
    Optional,
    TypeVar,
)

from .models import Page

T = TypeVar("T")

# ── Cursor encoding ───────────────────────────────────────────────────────────


def encode_cursor(offset: int) -> str:
    """Encode an integer offset into an opaque cursor string.

    The cursor is a simple base-10 string.  Callers should treat it as an
    opaque token and not rely on its internal format.

    Args:
        offset: Zero-based item index.

    Returns:
        An opaque cursor string.
    """
    return str(offset)


def decode_cursor(cursor: str) -> int:
    """Decode a cursor string produced by :func:`encode_cursor`.

    Args:
        cursor: Cursor string previously returned by :func:`encode_cursor`.

    Returns:
        The decoded offset integer.

    Raises:
        ValueError: If the cursor is malformed.
    """
    try:
        value = int(cursor)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid cursor: {cursor!r}") from exc
    if value < 0:
        raise ValueError(f"Cursor offset must be non-negative, got {value}")
    return value


# ── Pagination state ──────────────────────────────────────────────────────────


@dataclass
class PaginationState:
    """Tracks the current position in a paginated sequence.

    Attributes:
        offset: Current zero-based item offset.
        limit: Number of items per page.
        total: Total item count (may be ``None`` before the first fetch).
        fetched: Total number of items fetched so far.
        page_number: Number of pages fetched so far (1-based after first fetch).
    """

    offset: int = 0
    limit: int = 50
    total: Optional[int] = None
    fetched: int = 0
    page_number: int = 0

    @property
    def has_next(self) -> bool:
        """True if there are more items to fetch."""
        if self.total is None:
            return True
        return self.offset < self.total

    @property
    def cursor(self) -> str:
        """Opaque cursor representing the current position."""
        return encode_cursor(self.offset)

    def advance(self, page_size: int) -> None:
        """Advance the state after fetching *page_size* items."""
        self.offset += page_size
        self.fetched += page_size
        self.page_number += 1

    def reset(self) -> None:
        """Reset the state back to the beginning."""
        self.offset = 0
        self.fetched = 0
        self.page_number = 0
        self.total = None


# ── Total count cache ─────────────────────────────────────────────────────────


@dataclass
class TotalCountCache:
    """A simple time-based cache for the total event count.

    Avoids redundant ``total_events()`` RPC calls within a short window.

    Args:
        ttl_seconds: How long a cached value remains valid.

    Example::

        cache = TotalCountCache(ttl_seconds=10)
        total = cache.get_or_fetch(client.total_events)
    """

    ttl_seconds: float = 10.0
    _value: Optional[int] = field(default=None, init=False, repr=False)
    _fetched_at: Optional[float] = field(default=None, init=False, repr=False)

    def get_or_fetch(self, fetch_fn: Callable[[], int]) -> int:
        """Return the cached total or call *fetch_fn* to refresh it.

        Args:
            fetch_fn: Zero-argument callable that returns the current total.

        Returns:
            The (possibly cached) event total.
        """
        now = time.monotonic()
        if (
            self._value is None
            or self._fetched_at is None
            or (now - self._fetched_at) > self.ttl_seconds
        ):
            self._value = fetch_fn()
            self._fetched_at = now
        return self._value

    def invalidate(self) -> None:
        """Invalidate the cached value, forcing a fresh fetch next time."""
        self._value = None
        self._fetched_at = None

    @property
    def cached_value(self) -> Optional[int]:
        """The currently cached total, or ``None`` if not yet fetched."""
        return self._value


# ── Page iterator ─────────────────────────────────────────────────────────────


class PageIterator(Generic[T]):
    """Iterate over pages returned by a paginated fetch function.

    The iterator lazily calls *fetch_fn* for each page and stops when the
    page's ``total`` has been exhausted.

    Args:
        fetch_fn: Callable with signature ``(offset, limit) -> Page[T]``.
        limit: Items per page.
        start_offset: Initial offset (default ``0``).

    Example::

        for page in PageIterator(client.get_events, limit=20):
            for event in page.items:
                process(event)
    """

    def __init__(
        self,
        fetch_fn: Callable[[int, int], Page[T]],
        limit: int = 50,
        start_offset: int = 0,
    ) -> None:
        self._fetch = fetch_fn
        self._state = PaginationState(offset=start_offset, limit=limit)

    def __iter__(self) -> Iterator[Page[T]]:
        return self

    def __next__(self) -> Page[T]:
        if not self._state.has_next:
            raise StopIteration

        page: Page[T] = self._fetch(self._state.offset, self._state.limit)

        # Update total from the response so has_next stays accurate
        self._state.total = page.total

        if not page.items:
            raise StopIteration

        self._state.advance(len(page.items))
        return page

    @property
    def state(self) -> PaginationState:
        """The current :class:`PaginationState`."""
        return self._state

    def reset(self) -> None:
        """Reset the iterator to the beginning."""
        self._state.reset()


# ── Item iterator (flattened) ─────────────────────────────────────────────────


def iter_all_items(
    fetch_fn: Callable[[int, int], Page[T]],
    limit: int = 50,
    start_offset: int = 0,
) -> Iterator[T]:
    """Yield every individual item across all pages.

    Args:
        fetch_fn: Callable with signature ``(offset, limit) -> Page[T]``.
        limit: Items per page.
        start_offset: Initial offset (default ``0``).

    Yields:
        Individual items of type ``T``.

    Example::

        for event in iter_all_items(client.get_events, limit=100):
            print(event.event_type)
    """
    for page in PageIterator(fetch_fn, limit=limit, start_offset=start_offset):
        yield from page.items


# ── Cursor-based fetch helper ─────────────────────────────────────────────────


def fetch_page_by_cursor(
    fetch_fn: Callable[[int, int], Page[T]],
    cursor: Optional[str],
    limit: int = 50,
) -> Page[T]:
    """Fetch a single page using an opaque cursor.

    Args:
        fetch_fn: Callable with signature ``(offset, limit) -> Page[T]``.
        cursor: Opaque cursor from a previous response, or ``None`` to start
            from the beginning.
        limit: Maximum number of items to return.

    Returns:
        A :class:`~audit_ledger.models.Page` of items.

    Raises:
        ValueError: If the cursor is malformed.
    """
    offset = decode_cursor(cursor) if cursor is not None else 0
    return fetch_fn(offset, limit)
