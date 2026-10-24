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

Pagination (#852)
~~~~~~~~~~~~~~~~~
- :class:`PageCursor`         — opaque cursor token for deterministic pagination.
- :class:`PageLimit`          — page size cap (max 100 items).
- :class:`QueryFilter`        — multi-predicate query filter.
- :func:`paginate`            — apply cursor/limit to a collection.
- :func:`query_paginated`     — filtered, cursor-based query helper.
- :func:`iter_pages`          — full multi-page traversal generator.

Social impact
~~~~~~~~~~~~~
- :class:`SocialImpactMetrics`        — on-chain social impact snapshot model.
- :class:`Stakeholder`                — stakeholder registry entry.
- :class:`ImpactReport`               — aggregated impact report.
- :class:`SocialImpactAnalyzer`       — analytics engine (SROI, diversity, etc.).
- :func:`calculate_sroi`              — standalone SROI ratio calculator.
- :func:`diversity_score`             — workforce diversity averages.
- :func:`community_investment_summary` — community investment aggregation.
- :func:`labour_compliance_rate`      — labour-standards compliance summary.
- :func:`aggregate_impact_report`     — build full ImpactReport from metrics list.

SupTech rules (#832)
~~~~~~~~~~~~~~~~~~~~
- :class:`PageCursor`                 — opaque cursor token for paginated queries.
- :class:`PageLimit`                  — page size cap (max 100 items).
- :class:`RuleQueryFilter`            — multi-predicate filter for rule queries.
- :class:`RuleQueryPage`              — paginated rule query result.
- :func:`query_rules`                 — paginated, filtered rule query helper.
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
from .crdt import CRDTOperation, EventCRDT
from .async_client import AsyncAuditLedgerClient
from .social_impact import (
    SocialImpactMetrics,
    Stakeholder,
    ImpactReport,
    SocialImpactAnalyzer,
    calculate_sroi,
    diversity_score,
    community_investment_summary,
    labour_compliance_rate,
    aggregate_impact_report,
)
from .modern_slavery import (
    RiskAssessment,
    SupplyChainNode,
    TrainingRecord,
    DueDiligenceRecord,
    MSAPolicy,
    MSAReport,
    ModernSlaveryAnalyzer,
    calculate_risk_score,
    supply_chain_risk_summary,
    training_effectiveness,
    remediation_progress,
    build_compliance_report,
)
from .validation import (
    SchemaRegistry,
    SchemaValidationError,
    SchemaNotFoundError,
    get_default_registry,
    validate_event,
    BASE_EVENT_SCHEMA,
)
from .async_streaming import (
    async_stream_events,
    async_stream_by_type,
    AsyncEventBatcher,
)
from .confidential import (
    AccessControlPolicy,
    AccessControlPolicyType,
    ConfidentialEventPayload,
    ConfidentialMetadataHelper,
)
from .pagination import (
    PageCursor,
    PageLimit,
    QueryFilter,
    paginate,
    query_paginated,
    iter_pages,
)
from .suptech_rules import (
    RuleQueryFilter,
    RuleQueryPage,
    query_rules,
)
from .supply_chain import (
    Brand,
    ProductSKU,
    Provenance,
    Certification,
    LaborConditions,
    EnvironmentalImpact,
    Location,
    CustodyTransfer,
    AuditEntry,
    SupplyChainQueryFilters,
    PaginatedResult,
    SupplyChainClient,
    paginate_all_async,
)


__all__ = [
    # Sync client
    "AuditLedgerClient",
    # Async client (#242)
    "AsyncAuditLedgerClient",
    # Async streaming & batching (#392)
    "async_stream_events",
    "async_stream_by_type",
    "AsyncEventBatcher",
    # Confidential metadata (#401)
    "AccessControlPolicy",
    "AccessControlPolicyType",
    "ConfidentialEventPayload",
    "ConfidentialMetadataHelper",
    # Models
    "Event",
    "Page",
    # Pagination (#852)
    "PageCursor",
    "PageLimit",
    "QueryFilter",
    "paginate",
    "query_paginated",
    "iter_pages",
    # Contract event collaboration (issue #431)
    "CRDTOperation",
    "EventCRDT",
    # Exceptions (issue #249)
    "AuditLedgerError",
    "ContractError",
    "RPCError",
    # Validation (#240)
    "SchemaRegistry",
    "SchemaValidationError",
    "SchemaNotFoundError",
    "get_default_registry",
    "validate_event",
    "BASE_EVENT_SCHEMA",
    # Social impact
    "SocialImpactMetrics",
    "Stakeholder",
    "ImpactReport",
    "SocialImpactAnalyzer",
    "calculate_sroi",
    "diversity_score",
    "community_investment_summary",
    "labour_compliance_rate",
    "aggregate_impact_report",
    # Modern slavery transparency
    "RiskAssessment",
    "SupplyChainNode",
    "TrainingRecord",
    "DueDiligenceRecord",
    "MSAPolicy",
    "MSAReport",
    "ModernSlaveryAnalyzer",
    "calculate_risk_score",
    "supply_chain_risk_summary",
    "training_effectiveness",
    "remediation_progress",
    "build_compliance_report",
    # SupTech rules pagination & filtering (#832)
    "RuleQueryFilter",
    "RuleQueryPage",
    "query_rules",
    # Supply chain pagination & filtering (#872)
    "Brand",
    "ProductSKU",
    "Provenance",
    "Certification",
    "LaborConditions",
    "EnvironmentalImpact",
    "Location",
    "CustodyTransfer",
    "AuditEntry",
    "SupplyChainQueryFilters",
    "PaginatedResult",
    "SupplyChainClient",
    "paginate_all_async",
]
