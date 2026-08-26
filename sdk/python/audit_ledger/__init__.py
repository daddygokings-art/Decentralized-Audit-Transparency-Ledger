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

__all__ = [
    # Sync client
    "AuditLedgerClient",
    # Async client (#242)
    "AsyncAuditLedgerClient",
    # Models
    "Event",
    "Page",
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
]
