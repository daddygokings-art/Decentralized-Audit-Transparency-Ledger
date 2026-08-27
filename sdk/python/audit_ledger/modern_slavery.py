"""Modern slavery transparency module for the AuditLedger Python SDK.

Provides off-chain analytics and reporting for modern slavery due diligence per:

- UK Modern Slavery Act 2015
- Australian Modern Slavery Act 2018

Covers risk assessment, supply chain mapping, training effectiveness,
due diligence tracking, policy management, and compliance reporting.

Key classes:
- :class:`RiskAssessment`         — risk assessment snapshot.
- :class:`SupplyChainNode`        — supplier / partner registry entry.
- :class:`TrainingRecord`         — training session record.
- :class:`DueDiligenceRecord`     — investigation findings.
- :class:`MSAPolicy`              — policy document entry.
- :class:`MSAReport`              — aggregated compliance report.
- :class:`ModernSlaveryAnalyzer`  — analytics engine for compliance.

Key functions:
- :func:`calculate_risk_score`    — weighted risk scoring.
- :func:`supply_chain_risk_summary` — aggregate supplier risk profile.
- :func:`training_effectiveness`  — training reach and content coverage.
- :func:`remediation_progress`    — corrective action status tracking.
- :func:`build_compliance_report` — comprehensive compliance snapshot.

Usage::

    from audit_ledger.modern_slavery import (
        RiskAssessment,
        SupplyChainNode,
        ModernSlaveryAnalyzer,
        calculate_risk_score,
    )

    assessments = [
        RiskAssessment(
            assessment_id="2026_q1",
            scope="global",
            risk_level=1,
            high_risk_areas=3,
            stakeholder_consultation_done=True,
        )
    ]

    analyzer = ModernSlaveryAnalyzer(assessments)
    print(analyzer.risk_summary())  # dict with risk metrics
    report = analyzer.compliance_report()
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple


# ---------------------------------------------------------------------------
# Data models
# ---------------------------------------------------------------------------


@dataclass
class RiskAssessment:
    """Modern slavery risk assessment snapshot.

    Attributes:
        assessment_id: Unique identifier (e.g. ``"2026_q1_assessment"``).
        recorded_at: Optional Unix timestamp of assessment.
        submitter: Optional Stellar address of assessor.
        scope: Geographic or operational scope (e.g. ``"global"``).
        risk_level: 0=low, 1=medium, 2=high, 3=critical.
        high_risk_areas: Number of identified high-risk areas.
        key_risks: Brief description of key risks.
        planned_remediations: Number of planned corrective actions.
        stakeholder_consultation_done: Whether stakeholder input was sought.
        metadata: Optional free-form dict for extended attributes.
    """

    assessment_id: str
    scope: str = "global"
    risk_level: int = 0
    high_risk_areas: int = 0
    key_risks: str = ""
    planned_remediations: int = 0
    stakeholder_consultation_done: bool = False
    recorded_at: Optional[int] = None
    submitter: Optional[str] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "RiskAssessment":
        """Construct from a raw dict (e.g. contract RPC response)."""
        known = {
            "assessment_id", "scope", "risk_level", "high_risk_areas",
            "key_risks", "planned_remediations", "stakeholder_consultation_done",
            "recorded_at", "submitter",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            assessment_id=str(d.get("assessment_id", "")),
            scope=str(d.get("scope", "global")),
            risk_level=int(d.get("risk_level", 0)),  # type: ignore[arg-type]
            high_risk_areas=int(d.get("high_risk_areas", 0)),  # type: ignore[arg-type]
            key_risks=str(d.get("key_risks", "")),
            planned_remediations=int(d.get("planned_remediations", 0)),  # type: ignore[arg-type]
            stakeholder_consultation_done=bool(d.get("stakeholder_consultation_done", False)),
            recorded_at=int(d["recorded_at"]) if d.get("recorded_at") else None,  # type: ignore[arg-type]
            submitter=str(d["submitter"]) if d.get("submitter") else None,
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "assessment_id": self.assessment_id,
            "scope": self.scope,
            "risk_level": self.risk_level,
            "high_risk_areas": self.high_risk_areas,
            "key_risks": self.key_risks,
            "planned_remediations": self.planned_remediations,
            "stakeholder_consultation_done": self.stakeholder_consultation_done,
            "recorded_at": self.recorded_at,
            "submitter": self.submitter,
            **self.metadata,
        }


@dataclass
class SupplyChainNode:
    """Supply chain node (supplier / partner) record.

    Attributes:
        supplier_id: Unique supplier identifier.
        name: Organization name.
        country: Country code or region.
        risk_level: 0=low, 1=medium, 2=high, 3=critical.
        audited: Whether supplier has been audited.
        last_audit_date: Unix timestamp of last audit (0 = never).
        registered_at: Optional Unix timestamp of registration.
        metadata: Optional free-form dict.
    """

    supplier_id: str
    name: str = ""
    country: str = ""
    risk_level: int = 0
    audited: bool = False
    last_audit_date: int = 0
    registered_at: Optional[int] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "SupplyChainNode":
        """Construct from a raw dict."""
        known = {
            "supplier_id", "name", "country", "risk_level",
            "audited", "last_audit_date", "registered_at",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            supplier_id=str(d.get("supplier_id", "")),
            name=str(d.get("name", "")),
            country=str(d.get("country", "")),
            risk_level=int(d.get("risk_level", 0)),  # type: ignore[arg-type]
            audited=bool(d.get("audited", False)),
            last_audit_date=int(d.get("last_audit_date", 0)),  # type: ignore[arg-type]
            registered_at=int(d["registered_at"]) if d.get("registered_at") else None,  # type: ignore[arg-type]
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "supplier_id": self.supplier_id,
            "name": self.name,
            "country": self.country,
            "risk_level": self.risk_level,
            "audited": self.audited,
            "last_audit_date": self.last_audit_date,
            "registered_at": self.registered_at,
            **self.metadata,
        }


@dataclass
class TrainingRecord:
    """Training session record.

    Attributes:
        training_id: Unique training identifier.
        delivered_at: Optional Unix timestamp of delivery.
        topic: Training topic (e.g. ``"msa_awareness"``).
        attendees: Number of personnel trained.
        risk_assessment_covered: Whether risk assessment methodology was included.
        due_diligence_covered: Whether due diligence procedures were covered.
        reporting_covered: Whether reporting obligations were covered.
        content_summary: Brief description of content.
        metadata: Optional free-form dict.
    """

    training_id: str
    topic: str = ""
    attendees: int = 0
    risk_assessment_covered: bool = False
    due_diligence_covered: bool = False
    reporting_covered: bool = False
    content_summary: str = ""
    delivered_at: Optional[int] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "TrainingRecord":
        """Construct from a raw dict."""
        known = {
            "training_id", "topic", "attendees",
            "risk_assessment_covered", "due_diligence_covered", "reporting_covered",
            "content_summary", "delivered_at",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            training_id=str(d.get("training_id", "")),
            topic=str(d.get("topic", "")),
            attendees=int(d.get("attendees", 0)),  # type: ignore[arg-type]
            risk_assessment_covered=bool(d.get("risk_assessment_covered", False)),
            due_diligence_covered=bool(d.get("due_diligence_covered", False)),
            reporting_covered=bool(d.get("reporting_covered", False)),
            content_summary=str(d.get("content_summary", "")),
            delivered_at=int(d["delivered_at"]) if d.get("delivered_at") else None,  # type: ignore[arg-type]
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "training_id": self.training_id,
            "topic": self.topic,
            "attendees": self.attendees,
            "risk_assessment_covered": self.risk_assessment_covered,
            "due_diligence_covered": self.due_diligence_covered,
            "reporting_covered": self.reporting_covered,
            "content_summary": self.content_summary,
            "delivered_at": self.delivered_at,
            **self.metadata,
        }


@dataclass
class DueDiligenceRecord:
    """Due diligence investigation record.

    Attributes:
        record_id: Unique record identifier.
        completed_at: Optional Unix timestamp of completion.
        subject: Supplier or entity being investigated.
        scope: Investigation scope (e.g. ``"labour_practices"``).
        findings: Investigation findings summary.
        risk_level: 0=none, 1=low, 2=medium, 3=high, 4=critical.
        corrective_actions_required: Number of corrective actions identified.
        corrective_actions_completed_pct: Percentage completion (0-100).
        metadata: Optional free-form dict.
    """

    record_id: str
    subject: str = ""
    scope: str = ""
    findings: str = ""
    risk_level: int = 0
    corrective_actions_required: int = 0
    corrective_actions_completed_pct: int = 0
    completed_at: Optional[int] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "DueDiligenceRecord":
        """Construct from a raw dict."""
        known = {
            "record_id", "subject", "scope", "findings",
            "risk_level", "corrective_actions_required",
            "corrective_actions_completed_pct", "completed_at",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            record_id=str(d.get("record_id", "")),
            subject=str(d.get("subject", "")),
            scope=str(d.get("scope", "")),
            findings=str(d.get("findings", "")),
            risk_level=int(d.get("risk_level", 0)),  # type: ignore[arg-type]
            corrective_actions_required=int(d.get("corrective_actions_required", 0)),  # type: ignore[arg-type]
            corrective_actions_completed_pct=int(d.get("corrective_actions_completed_pct", 0)),  # type: ignore[arg-type]
            completed_at=int(d["completed_at"]) if d.get("completed_at") else None,  # type: ignore[arg-type]
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "record_id": self.record_id,
            "subject": self.subject,
            "scope": self.scope,
            "findings": self.findings,
            "risk_level": self.risk_level,
            "corrective_actions_required": self.corrective_actions_required,
            "corrective_actions_completed_pct": self.corrective_actions_completed_pct,
            "completed_at": self.completed_at,
            **self.metadata,
        }


@dataclass
class MSAPolicy:
    """Modern slavery prevention policy entry.

    Attributes:
        policy_id: Unique policy identifier.
        adopted_at: Optional Unix timestamp of adoption.
        last_updated_at: Optional Unix timestamp of last update.
        version: Policy version number.
        scope: Policy scope (e.g. ``"global"``).
        content_summary: Policy content summary.
        stakeholder_input_included: Whether stakeholder input was included.
        metadata: Optional free-form dict.
    """

    policy_id: str
    version: int = 1
    scope: str = "global"
    content_summary: str = ""
    stakeholder_input_included: bool = False
    adopted_at: Optional[int] = None
    last_updated_at: Optional[int] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "MSAPolicy":
        """Construct from a raw dict."""
        known = {
            "policy_id", "version", "scope", "content_summary",
            "stakeholder_input_included", "adopted_at", "last_updated_at",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            policy_id=str(d.get("policy_id", "")),
            version=int(d.get("version", 1)),  # type: ignore[arg-type]
            scope=str(d.get("scope", "global")),
            content_summary=str(d.get("content_summary", "")),
            stakeholder_input_included=bool(d.get("stakeholder_input_included", False)),
            adopted_at=int(d["adopted_at"]) if d.get("adopted_at") else None,  # type: ignore[arg-type]
            last_updated_at=int(d["last_updated_at"]) if d.get("last_updated_at") else None,  # type: ignore[arg-type]
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "policy_id": self.policy_id,
            "version": self.version,
            "scope": self.scope,
            "content_summary": self.content_summary,
            "stakeholder_input_included": self.stakeholder_input_included,
            "adopted_at": self.adopted_at,
            "last_updated_at": self.last_updated_at,
            **self.metadata,
        }


@dataclass
class MSAReport:
    """Aggregated modern slavery compliance report.

    Attributes:
        generated_at: Unix timestamp of report generation.
        assessments_count: Total number of risk assessments.
        max_risk_level: Highest risk level found (0=low, 3=critical).
        total_high_risk_areas: Total high-risk areas identified.
        supply_chain_nodes: Number of suppliers / partners mapped.
        high_risk_suppliers: Number of high/critical-risk suppliers.
        total_trained_personnel: Total personnel trained.
        due_diligence_investigations: Number of due diligence investigations.
        total_corrective_actions: Total corrective actions identified.
        corrective_actions_completion_pct: Completion percentage (0-100).
        active_policies: Number of active policies.
        assessments: Optional list of included assessments.
    """

    generated_at: Optional[int] = None
    assessments_count: int = 0
    max_risk_level: int = 0
    total_high_risk_areas: int = 0
    supply_chain_nodes: int = 0
    high_risk_suppliers: int = 0
    total_trained_personnel: int = 0
    due_diligence_investigations: int = 0
    total_corrective_actions: int = 0
    corrective_actions_completion_pct: int = 0
    active_policies: int = 0
    assessments: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "generated_at": self.generated_at,
            "assessments_count": self.assessments_count,
            "max_risk_level": self.max_risk_level,
            "total_high_risk_areas": self.total_high_risk_areas,
            "supply_chain_nodes": self.supply_chain_nodes,
            "high_risk_suppliers": self.high_risk_suppliers,
            "total_trained_personnel": self.total_trained_personnel,
            "due_diligence_investigations": self.due_diligence_investigations,
            "total_corrective_actions": self.total_corrective_actions,
            "corrective_actions_completion_pct": self.corrective_actions_completion_pct,
            "active_policies": self.active_policies,
            "assessments": self.assessments,
        }

    def to_json(self, indent: int = 2) -> str:
        """Serialise to a JSON string."""
        return json.dumps(self.to_dict(), indent=indent)


# ---------------------------------------------------------------------------
# Standalone helper functions
# ---------------------------------------------------------------------------


def calculate_risk_score(
    assessments: Sequence[RiskAssessment],
    *,
    weight_high_risk_areas: float = 1.0,
    weight_stakeholder_consultation: float = 0.5,
) -> float:
    """Calculate a weighted modern slavery risk score.

    Factors:
    - Maximum risk level found (0-3 scale).
    - Number and proportion of high-risk areas.
    - Stakeholder consultation (reduces final score).

    Args:
        assessments: Sequence of risk assessment records.
        weight_high_risk_areas: Weight for high-risk area count (default 1.0).
        weight_stakeholder_consultation: Consultation reduction factor (0-1, default 0.5).

    Returns:
        Risk score on 0-10 scale (0=low risk, 10=critical risk).

    Raises:
        ValueError: If weight parameters are invalid.
    """
    if not (0 <= weight_stakeholder_consultation <= 1.0):
        raise ValueError("weight_stakeholder_consultation must be 0-1")

    if not assessments:
        return 0.0

    # Base score from maximum risk level
    max_level = max((a.risk_level for a in assessments), default=0)
    base_score = min(max_level * 2.5, 7.5)  # 0-3 level → 0-7.5 score

    # Add high-risk area factor
    total_high_risk = sum(a.high_risk_areas for a in assessments)
    area_score = min(total_high_risk * weight_high_risk_areas * 0.5, 2.5)

    score = base_score + area_score

    # Consultation discount (if most assessments include consultation, reduce score)
    with_consultation = sum(1 for a in assessments if a.stakeholder_consultation_done)
    consultation_rate = with_consultation / len(assessments)
    if consultation_rate >= 0.7:
        score = score * (1.0 - (0.3 * weight_stakeholder_consultation))

    return min(score, 10.0)


def supply_chain_risk_summary(
    nodes: Sequence[SupplyChainNode],
) -> Dict[str, object]:
    """Aggregate supply chain risk profile.

    Args:
        nodes: List of supply chain nodes.

    Returns:
        Dict with:

        - ``total_suppliers``: Number of suppliers.
        - ``by_risk_level``: Count per risk level.
        - ``audited_count``: Number of audited suppliers.
        - ``audit_rate_pct``: Percentage audited (0-100).
        - ``high_or_critical_count``: Suppliers at risk level 2 or 3.
        - ``avg_days_since_audit``: Average days since last audit (or -1 if never).
    """
    if not nodes:
        return {
            "total_suppliers": 0,
            "by_risk_level": {},
            "audited_count": 0,
            "audit_rate_pct": 0.0,
            "high_or_critical_count": 0,
            "avg_days_since_audit": -1,
        }

    by_level: Dict[int, int] = {}
    audited = 0
    high_risk = 0
    now = 0  # In real code, use current Unix timestamp

    for n in nodes:
        by_level[n.risk_level] = by_level.get(n.risk_level, 0) + 1
        if n.audited:
            audited += 1
        if n.risk_level >= 2:
            high_risk += 1

    return {
        "total_suppliers": len(nodes),
        "by_risk_level": by_level,
        "audited_count": audited,
        "audit_rate_pct": (audited / len(nodes)) * 100.0 if nodes else 0.0,
        "high_or_critical_count": high_risk,
        "avg_days_since_audit": 0,  # Placeholder
    }


def training_effectiveness(
    records: Sequence[TrainingRecord],
) -> Dict[str, object]:
    """Summarise training reach and content coverage.

    Args:
        records: List of training records.

    Returns:
        Dict with:

        - ``total_personnel_trained``: Total attendees across sessions.
        - ``total_sessions``: Number of training sessions.
        - ``avg_attendees_per_session``: Mean session size.
        - ``risk_assessment_covered_pct``: % of sessions covering risk assessment.
        - ``due_diligence_covered_pct``: % of sessions covering due diligence.
        - ``reporting_covered_pct``: % of sessions covering reporting.
    """
    if not records:
        return {
            "total_personnel_trained": 0,
            "total_sessions": 0,
            "avg_attendees_per_session": 0.0,
            "risk_assessment_covered_pct": 0.0,
            "due_diligence_covered_pct": 0.0,
            "reporting_covered_pct": 0.0,
        }

    total = sum(r.attendees for r in records)
    ra_covered = sum(1 for r in records if r.risk_assessment_covered)
    dd_covered = sum(1 for r in records if r.due_diligence_covered)
    rep_covered = sum(1 for r in records if r.reporting_covered)
    n = len(records)

    return {
        "total_personnel_trained": total,
        "total_sessions": n,
        "avg_attendees_per_session": total / n,
        "risk_assessment_covered_pct": (ra_covered / n) * 100.0,
        "due_diligence_covered_pct": (dd_covered / n) * 100.0,
        "reporting_covered_pct": (rep_covered / n) * 100.0,
    }


def remediation_progress(
    records: Sequence[DueDiligenceRecord],
) -> Dict[str, object]:
    """Track corrective action completion progress.

    Args:
        records: List of due diligence investigation records.

    Returns:
        Dict with:

        - ``total_actions_identified``: Total corrective actions.
        - ``total_investigations``: Number of investigations.
        - ``avg_completion_pct``: Average completion percentage.
        - ``fully_completed``: Count of fully remediated investigations (100%).
        - ``partially_completed``: Count at 1-99% completion.
        - ``not_started``: Count at 0% completion.
    """
    if not records:
        return {
            "total_actions_identified": 0,
            "total_investigations": 0,
            "avg_completion_pct": 0.0,
            "fully_completed": 0,
            "partially_completed": 0,
            "not_started": 0,
        }

    total_actions = sum(r.corrective_actions_required for r in records)
    fully = sum(1 for r in records if r.corrective_actions_completed_pct == 100)
    partial = sum(1 for r in records if 0 < r.corrective_actions_completed_pct < 100)
    not_started = sum(1 for r in records if r.corrective_actions_completed_pct == 0)
    avg_pct = sum(r.corrective_actions_completed_pct for r in records) / len(records)

    return {
        "total_actions_identified": total_actions,
        "total_investigations": len(records),
        "avg_completion_pct": avg_pct,
        "fully_completed": fully,
        "partially_completed": partial,
        "not_started": not_started,
    }


def build_compliance_report(
    assessments: Sequence[RiskAssessment],
    nodes: Optional[Sequence[SupplyChainNode]] = None,
    trainings: Optional[Sequence[TrainingRecord]] = None,
    due_diligence: Optional[Sequence[DueDiligenceRecord]] = None,
    policies: Optional[Sequence[MSAPolicy]] = None,
    generated_at: Optional[int] = None,
) -> MSAReport:
    """Build a comprehensive modern slavery compliance report.

    Aggregates all compliance dimensions into a single snapshot.

    Args:
        assessments: Risk assessment records.
        nodes: Supply chain nodes.
        trainings: Training sessions.
        due_diligence: Due diligence investigations.
        policies: Policy documents.
        generated_at: Optional Unix timestamp for report.

    Returns:
        A populated :class:`MSAReport`.
    """
    nodes = nodes or []
    trainings = trainings or []
    due_diligence = due_diligence or []
    policies = policies or []

    max_risk = max((a.risk_level for a in assessments), default=0)
    total_high_risk_areas = sum(a.high_risk_areas for a in assessments)
    high_risk_suppliers = sum(1 for n in nodes if n.risk_level >= 2)
    total_trained = sum(t.attendees for t in trainings)
    total_actions = sum(d.corrective_actions_required for d in due_diligence)
    completed_actions = sum(
        d.corrective_actions_required * d.corrective_actions_completed_pct // 100
        for d in due_diligence
    )
    completion_pct = (
        (completed_actions / total_actions) * 100 if total_actions > 0 else 0
    )

    return MSAReport(
        generated_at=generated_at,
        assessments_count=len(assessments),
        max_risk_level=max_risk,
        total_high_risk_areas=total_high_risk_areas,
        supply_chain_nodes=len(nodes),
        high_risk_suppliers=high_risk_suppliers,
        total_trained_personnel=total_trained,
        due_diligence_investigations=len(due_diligence),
        total_corrective_actions=total_actions,
        corrective_actions_completion_pct=int(completion_pct),
        active_policies=len(policies),
        assessments=[a.assessment_id for a in assessments],
    )


# ---------------------------------------------------------------------------
# ModernSlaveryAnalyzer
# ---------------------------------------------------------------------------


class ModernSlaveryAnalyzer:
    """Analytics engine for modern slavery compliance tracking.

    Wraps collections of compliance records and provides convenience methods
    for risk scoring, supply chain analysis, training effectiveness,
    remediation tracking, and report generation.

    Args:
        assessments: List of risk assessment records.
        nodes: Optional list of supply chain nodes.
        trainings: Optional list of training records.
        due_diligence: Optional list of due diligence records.
        policies: Optional list of policy records.

    Example::

        analyzer = ModernSlaveryAnalyzer(
            assessments=[
                RiskAssessment(
                    assessment_id="2026_q1",
                    scope="global",
                    risk_level=1,
                    high_risk_areas=3,
                )
            ]
        )
        print(analyzer.risk_score())          # 1.875
        print(analyzer.supply_chain_summary()) # dict with SC profile
    """

    def __init__(
        self,
        assessments: List[RiskAssessment],
        nodes: Optional[List[SupplyChainNode]] = None,
        trainings: Optional[List[TrainingRecord]] = None,
        due_diligence: Optional[List[DueDiligenceRecord]] = None,
        policies: Optional[List[MSAPolicy]] = None,
    ) -> None:
        self._assessments = list(assessments)
        self._nodes: List[SupplyChainNode] = list(nodes) if nodes else []
        self._trainings: List[TrainingRecord] = list(trainings) if trainings else []
        self._due_diligence: List[DueDiligenceRecord] = (
            list(due_diligence) if due_diligence else []
        )
        self._policies: List[MSAPolicy] = list(policies) if policies else []

    # ── Risk assessment ───────────────────────────────────────────────────

    def risk_score(self) -> float:
        """Calculate overall risk score (0-10 scale)."""
        return calculate_risk_score(self._assessments)

    def risk_summary(self) -> Dict[str, object]:
        """Return risk assessment summary."""
        if not self._assessments:
            return {
                "assessment_count": 0,
                "max_risk_level": 0,
                "total_high_risk_areas": 0,
                "with_consultation_pct": 0.0,
            }

        max_level = max(a.risk_level for a in self._assessments)
        total_areas = sum(a.high_risk_areas for a in self._assessments)
        with_consult = sum(1 for a in self._assessments if a.stakeholder_consultation_done)
        consult_pct = (with_consult / len(self._assessments)) * 100.0

        return {
            "assessment_count": len(self._assessments),
            "max_risk_level": max_level,
            "total_high_risk_areas": total_areas,
            "with_consultation_pct": consult_pct,
        }

    # ── Supply chain ──────────────────────────────────────────────────────

    def supply_chain_summary(self) -> Dict[str, object]:
        """Aggregate supply chain risk profile."""
        return supply_chain_risk_summary(self._nodes)

    def supply_chain_by_country(self) -> Dict[str, int]:
        """Group suppliers by country."""
        result: Dict[str, int] = {}
        for n in self._nodes:
            result[n.country] = result.get(n.country, 0) + 1
        return result

    def high_risk_suppliers(self) -> List[SupplyChainNode]:
        """Return all high or critical-risk suppliers."""
        return [n for n in self._nodes if n.risk_level >= 2]

    def unaudited_suppliers(self) -> List[SupplyChainNode]:
        """Return suppliers that have never been audited."""
        return [n for n in self._nodes if not n.audited]

    # ── Training ──────────────────────────────────────────────────────────

    def training_summary(self) -> Dict[str, object]:
        """Get training reach and content coverage."""
        return training_effectiveness(self._trainings)

    def total_personnel_trained(self) -> int:
        """Return total number of personnel trained."""
        return sum(t.attendees for t in self._trainings)

    # ── Due diligence ─────────────────────────────────────────────────────

    def remediation_summary(self) -> Dict[str, object]:
        """Get corrective action completion status."""
        return remediation_progress(self._due_diligence)

    def high_risk_investigations(self) -> List[DueDiligenceRecord]:
        """Return high and critical-risk investigation findings."""
        return [d for d in self._due_diligence if d.risk_level >= 3]

    # ── Policy ────────────────────────────────────────────────────────────

    def policy_count(self) -> int:
        """Return total number of active policies."""
        return len(self._policies)

    def policies_with_stakeholder_input(self) -> List[MSAPolicy]:
        """Return policies developed with stakeholder input."""
        return [p for p in self._policies if p.stakeholder_input_included]

    # ── Full compliance report ────────────────────────────────────────────

    def compliance_report(self, generated_at: Optional[int] = None) -> MSAReport:
        """Generate a comprehensive compliance snapshot.

        Args:
            generated_at: Optional Unix timestamp for report header.

        Returns:
            A populated :class:`MSAReport`.
        """
        return build_compliance_report(
            self._assessments,
            self._nodes,
            self._trainings,
            self._due_diligence,
            self._policies,
            generated_at,
        )

    # ── Data management ───────────────────────────────────────────────────

    def add_assessment(self, assessment: RiskAssessment) -> None:
        """Add a risk assessment record."""
        self._assessments.append(assessment)

    def add_node(self, node: SupplyChainNode) -> None:
        """Add a supply chain node."""
        self._nodes.append(node)

    def add_training(self, training: TrainingRecord) -> None:
        """Add a training record."""
        self._trainings.append(training)

    def add_due_diligence(self, record: DueDiligenceRecord) -> None:
        """Add a due diligence investigation record."""
        self._due_diligence.append(record)

    def add_policy(self, policy: MSAPolicy) -> None:
        """Add a policy record."""
        self._policies.append(policy)


# ---------------------------------------------------------------------------
# Public re-exports
# ---------------------------------------------------------------------------

__all__ = [
    # Data models
    "RiskAssessment",
    "SupplyChainNode",
    "TrainingRecord",
    "DueDiligenceRecord",
    "MSAPolicy",
    "MSAReport",
    # Standalone helpers
    "calculate_risk_score",
    "supply_chain_risk_summary",
    "training_effectiveness",
    "remediation_progress",
    "build_compliance_report",
    # Analyzer class
    "ModernSlaveryAnalyzer",
]
