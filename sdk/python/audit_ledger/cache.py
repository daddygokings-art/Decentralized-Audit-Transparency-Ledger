"""LRU caching layer for the AuditLedger Python SDK (#246).

Provides:
- :class:`CacheConfig`    — size, TTL, and on/off knobs.
- :class:`CacheStats`     — hit/miss/eviction counters.
- :class:`LRUCache`       — thread-safe LRU cache with TTL and per-key invalidation.
- :func:`cached_method`   — decorator that wires cache into a client method.
"""

from __future__ import annotations

import threading
import time
from collections import OrderedDict
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, Generic, Hashable, Optional, Tuple, TypeVar

__all__ = [
    "CacheConfig",
    "CacheStats",
    "LRUCache",
    "cached_method",
]

# ---------------------------------------------------------------------------
# Type aliases
# ---------------------------------------------------------------------------

CacheKey = Hashable
V = TypeVar("V")


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class CacheConfig:
    """Configuration parameters for the SDK-level LRU cache.

    Attributes:
        max_size: Maximum number of entries in the cache.  Oldest entries are
            evicted once this limit is reached.  Set to ``0`` to disable
            caching entirely.
        ttl_seconds: Time-to-live in seconds for each cached value.  A value
            of ``0`` or ``None`` means entries never expire.
        enabled: Master switch.  When ``False`` the cache is bypassed and
            every request goes directly to the RPC layer.
    """

    max_size: int = 256
    ttl_seconds: Optional[float] = 60.0
    enabled: bool = True


# ---------------------------------------------------------------------------
# Statistics
# ---------------------------------------------------------------------------


@dataclass
class CacheStats:
    """Counters tracking cache utilisation.

    Attributes:
        hits: Number of successful cache lookups.
        misses: Number of cache lookups that fell through to the origin.
        evictions: Number of entries removed due to capacity pressure.
        expirations: Number of entries removed because their TTL elapsed.
        current_size: Number of entries currently in the cache.
    """

    hits: int = 0
    misses: int = 0
    evictions: int = 0
    expirations: int = 0
    current_size: int = 0

    @property
    def total_requests(self) -> int:
        """Total number of cache lookup attempts."""
        return self.hits + self.misses

    @property
    def hit_rate(self) -> float:
        """Fraction of requests satisfied from cache (0.0 – 1.0)."""
        total = self.total_requests
        return self.hits / total if total > 0 else 0.0

    def reset(self) -> None:
        """Zero all counters (does not clear the cache itself)."""
        self.hits = 0
        self.misses = 0
        self.evictions = 0
        self.expirations = 0
        # current_size is kept in sync by LRUCache; we leave it alone.


# ---------------------------------------------------------------------------
# Core LRU cache
# ---------------------------------------------------------------------------

# Internal entry: (value, expiry_timestamp | None)
_CacheEntry = Tuple[Any, Optional[float]]


class LRUCache(Generic[V]):
    """Thread-safe Least-Recently-Used cache with optional TTL.

    Args:
        config: Cache configuration instance.

    Example::

        cache: LRUCache[Event] = LRUCache(CacheConfig(max_size=128, ttl_seconds=30))
        cache.set("event:0", event)
        cached = cache.get("event:0")   # returns event or None
        cache.invalidate("event:0")     # remove a single key
        cache.clear()                   # remove all keys
        stats = cache.stats             # access counters
    """

    def __init__(self, config: Optional[CacheConfig] = None) -> None:
        self._config: CacheConfig = config or CacheConfig()
        self._store: OrderedDict[CacheKey, _CacheEntry] = OrderedDict()
        self._lock: threading.RLock = threading.RLock()
        self._stats: CacheStats = CacheStats()

    # ------------------------------------------------------------------
    # Public interface
    # ------------------------------------------------------------------

    @property
    def config(self) -> CacheConfig:
        """Current cache configuration (read-only view)."""
        return self._config

    @property
    def stats(self) -> CacheStats:
        """Live cache statistics."""
        with self._lock:
            self._stats.current_size = len(self._store)
        return self._stats

    def configure(self, config: CacheConfig) -> None:
        """Replace the cache configuration at runtime.

        Shrinks or clears the cache if the new ``max_size`` is smaller.

        Args:
            config: New :class:`CacheConfig` to apply.
        """
        with self._lock:
            self._config = config
            if not config.enabled or config.max_size == 0:
                self._store.clear()
            else:
                self._evict_to_size(config.max_size)

    def get(self, key: CacheKey) -> Optional[V]:
        """Look up a value in the cache.

        Returns ``None`` when the key is absent, disabled, or expired.

        Args:
            key: Cache key to look up.

        Returns:
            The cached value, or ``None`` on a miss.
        """
        if not self._config.enabled or self._config.max_size == 0:
            self._stats.misses += 1
            return None

        with self._lock:
            if key not in self._store:
                self._stats.misses += 1
                return None

            value, expiry = self._store[key]

            # TTL check
            if expiry is not None and time.monotonic() >= expiry:
                del self._store[key]
                self._stats.expirations += 1
                self._stats.misses += 1
                return None

            # Promote to most-recently-used position
            self._store.move_to_end(key)
            self._stats.hits += 1
            return value  # type: ignore[return-value]

    def set(self, key: CacheKey, value: V) -> None:
        """Store a value in the cache.

        If the key already exists it is overwritten and its TTL is
        refreshed.  Least-recently-used entries are evicted once
        ``max_size`` is exceeded.

        Args:
            key: Cache key.
            value: Value to store.
        """
        if not self._config.enabled or self._config.max_size == 0:
            return

        ttl = self._config.ttl_seconds
        expiry: Optional[float] = (
            time.monotonic() + ttl if ttl and ttl > 0 else None
        )

        with self._lock:
            if key in self._store:
                self._store.move_to_end(key)
            self._store[key] = (value, expiry)
            self._evict_to_size(self._config.max_size)

    def invalidate(self, key: CacheKey) -> bool:
        """Remove a single key from the cache.

        Args:
            key: The key to remove.

        Returns:
            ``True`` if the key existed and was removed, ``False`` otherwise.
        """
        with self._lock:
            if key in self._store:
                del self._store[key]
                return True
            return False

    def invalidate_prefix(self, prefix: str) -> int:
        """Remove all keys whose string representation starts with *prefix*.

        Useful for bulk invalidation of a whole event type or submitter.

        Args:
            prefix: Key prefix to match against.

        Returns:
            Number of keys removed.
        """
        with self._lock:
            to_delete = [k for k in self._store if str(k).startswith(prefix)]
            for k in to_delete:
                del self._store[k]
            return len(to_delete)

    def clear(self) -> None:
        """Remove all entries from the cache."""
        with self._lock:
            self._store.clear()

    def reset_stats(self) -> None:
        """Reset hit/miss/eviction counters without clearing cached data."""
        with self._lock:
            self._stats.reset()

    def __len__(self) -> int:
        with self._lock:
            return len(self._store)

    def __contains__(self, key: object) -> bool:
        # Fast path: no TTL check — use get() if TTL semantics are needed.
        with self._lock:
            return key in self._store

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _evict_to_size(self, max_size: int) -> None:
        """Evict the oldest entries until ``len(store) <= max_size``."""
        while len(self._store) > max_size:
            self._store.popitem(last=False)  # FIFO/LRU eviction
            self._stats.evictions += 1


# ---------------------------------------------------------------------------
# Decorator
# ---------------------------------------------------------------------------


def cached_method(
    cache_attr: str = "_cache",
    key_fn: Optional[Callable[..., CacheKey]] = None,
) -> Callable[[Callable[..., V]], Callable[..., V]]:
    """Decorator that caches the return value of a client method.

    The decorated method must belong to a class that has a
    :class:`LRUCache` attribute named *cache_attr* (default: ``"_cache"``).

    Args:
        cache_attr: Name of the :class:`LRUCache` attribute on ``self``.
        key_fn: Optional callable ``(self, *args, **kwargs) -> CacheKey``.
            When omitted a default key of ``(method_name, args, frozenset
            (kwargs.items()))`` is used.

    Returns:
        A decorated function that transparently reads/writes the cache.

    Example::

        class MyClient:
            _cache: LRUCache[Event] = LRUCache()

            @cached_method()
            def get_event_by_order(self, order: int) -> Event:
                ...
    """

    def decorator(fn: Callable[..., V]) -> Callable[..., V]:
        def wrapper(self: Any, *args: Any, **kwargs: Any) -> V:
            cache: Optional[LRUCache[V]] = getattr(self, cache_attr, None)
            if cache is None:
                return fn(self, *args, **kwargs)

            if key_fn is not None:
                key: CacheKey = key_fn(self, *args, **kwargs)
            else:
                key = (fn.__name__, args, frozenset(kwargs.items()))

            cached_val = cache.get(key)
            if cached_val is not None:
                return cached_val

            result: V = fn(self, *args, **kwargs)
            cache.set(key, result)
            return result

        wrapper.__name__ = fn.__name__
        wrapper.__doc__ = fn.__doc__
        wrapper.__wrapped__ = fn  # type: ignore[attr-defined]
        return wrapper  # type: ignore[return-value]

    return decorator
