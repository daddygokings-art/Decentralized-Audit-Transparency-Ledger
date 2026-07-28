"""AuditLedger Python SDK.

Public API
----------
- :class:`AuditLedgerClient`  — main contract client.
- :class:`Event`              — on-chain event data model.
- :class:`Page`               — paginated result container.
- :class:`ContractError`      — contract-level error.
- :class:`RPCError`           — RPC / network error.
- :class:`AuditLedgerError`   — base exception class.
- :class:`CacheConfig`        — LRU cache configuration (#246).
- :class:`CacheStats`         — cache hit/miss statistics (#246).
- :class:`LRUCache`           — LRU cache implementation (#246).
- :class:`StreamConfig`       — streaming configuration (#244).
- :class:`StreamError`        — streaming error (#244).
- :func:`stream_events`       — free-function event stream generator (#244).
- :func:`stream_by_type`      — type-filtered stream generator (#244).
- :class:`BatchSubmitRequest` — single-event submit descriptor (#245).
- :class:`BatchResult`        — batch operation result (#245).
- :class:`BatchProgress`      — live progress counter (#245).
- :func:`batch_submit`        — free-function batch submit (#245).
- :func:`batch_get`           — free-function batch retrieval (#245).
- :func:`batch_verify`        — free-function batch verification (#245).
"""

from .batch import (
    BatchProgress,
    BatchResult,
    BatchSubmitRequest,
    batch_get,
    batch_submit,
    batch_verify,
)
from .cache import CacheConfig, CacheStats, LRUCache
from .client import AuditLedgerClient
from .models import Event, ContractError, RPCError, AuditLedgerError, Page
from .async_client import AsyncAuditLedgerClient
from .validation import (
    SchemaRegistry,
    SchemaValidationError,
    SchemaNotFoundError,
    get_default_registry,
    validate_event,
    BASE_EVENT_SCHEMA,
)

__all__ = [
    # Sync client
    "AuditLedgerClient",
    # Async client (#242)
    "AsyncAuditLedgerClient",
    # Models
    "Event",
    "Page",
    "ContractError",
    "RPCError",
    "AuditLedgerError",
    # Validation (#240)
    "SchemaRegistry",
    "SchemaValidationError",
    "SchemaNotFoundError",
    "get_default_registry",
    "validate_event",
    "BASE_EVENT_SCHEMA",
]
