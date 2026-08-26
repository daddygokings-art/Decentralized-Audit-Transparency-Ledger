"""Social impact measurement module for the AuditLedger Python SDK.

Provides off-chain analytics on top of on-chain social impact data:

- :class:`SocialImpactMetrics`  — mirrors the on-chain struct for local computation.
- :class:`Stakeholder`          — stakeholder registry entry.
- :class:`ImpactReport`         — aggregated impact report.
- :class:`SocialImpactAnalyzer` — analytics engine: SROI, diversity, community.
- :func:`calculate_sroi`        — standalone SROI helper.
- :func:`diversity_score`       — weighted diversity index.
- :func:`community_investment_summary` — aggregate community investment metrics.
- :func:`labour_compliance_rate`       — remediation-to-violation ratio.
- :func:`aggregate_impact_report`      — build an ImpactReport from a list of metrics.

Monetary units
--------------
All monetary values follow the convention used by the on-chain contract: whole
units of the reference currency (e.g. USD cents stored as integers, or USD
whole dollars — whichever the organisation chooses, as long as it is
consistent within a set of periods).

SROI methodology
----------------
Social Return on Investment is expressed as a ratio of social value created to
the investment required to create it.  This module stores it in two forms:

- **float** (e.g. ``3.5``) — human-readable ratio.
- **basis points** (e.g. ``35000``) — integer representation matching the
  on-chain storage format (``sroi_bps = ratio × 10 000``).

The calculation follows the SROI Network / Cabinet Office methodology:
  ``SROI = total_social_value / total_investment``

Deadweight, attribution, and displacement adjustments can be applied through
the ``deadweight_bps``, ``attribution_bps``, and ``displacement_bps``
parameters of :func:`calculate_sroi`.

Usage::

    from audit_ledger.social_impact import (
        SocialImpactMetrics,
        Stakeholder,
        SocialImpactAnalyzer,
        calculate_sroi,
    )

    metrics = [
        SocialImpactMetrics(
            period="2026_Q1",
            jobs_created=50,
            training_positions=10,
            diversity_women_bps=4500,
            diversity_underrepresented_bps=3000,
            community_investment=100_000,
            community_beneficiaries=500,
            human_rights_assessment_done=True,
            labour_violations_remediated=2,
            collective_bargaining_agreements=3,
            total_investment=200_000,
            total_social_value=700_000,
        )
    ]

    analyzer = SocialImpactAnalyzer(metrics)
    print(analyzer.sroi())           # 3.5
    print(analyzer.sroi_bps())       # 35000
    print(analyzer.impact_report())  # ImpactReport(...)
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence


# ---------------------------------------------------------------------------
# Data models
# ---------------------------------------------------------------------------


@dataclass
class SocialImpactMetrics:
    """Off-chain mirror of the on-chain ``SocialImpactMetrics`` struct.

    All monetary amounts are in whole reference-currency units (e.g. USD).
    Percentage values (``*_bps``) are stored in basis points (0–10 000 = 0%–100%).

    Attributes:
        period: Reporting period tag (e.g. ``"2026_Q1"``).
        jobs_created: Full-time-equivalent jobs created during the period.
        training_positions: Apprenticeship / training positions opened.
        diversity_women_bps: Percentage of workforce identifying as women, in bps.
        diversity_underrepresented_bps: Percentage from under-represented groups, in bps.
        community_investment: Direct community investment in whole monetary units.
        community_beneficiaries: Number of people reached by community programmes.
        human_rights_assessment_done: Whether a human-rights due-diligence assessment
            was completed this period.
        labour_violations_remediated: Labour-standard violations reported & fixed.
        collective_bargaining_agreements: Active collective-bargaining agreements.
        total_investment: Total cost of interventions in whole monetary units.
        total_social_value: Total social value created in whole monetary units.
        recorded_at: Optional Unix timestamp when the record was submitted on-chain.
        submitter: Optional Stellar address of the submitter.
        metadata: Optional free-form dict for extended attributes.
    """

    period: str
    jobs_created: int = 0
    training_positions: int = 0
    diversity_women_bps: int = 0
    diversity_underrepresented_bps: int = 0
    community_investment: int = 0
    community_beneficiaries: int = 0
    human_rights_assessment_done: bool = False
    labour_violations_remediated: int = 0
    collective_bargaining_agreements: int = 0
    total_investment: int = 0
    total_social_value: int = 0
    recorded_at: Optional[int] = None
    submitter: Optional[str] = None
    metadata: Dict[str, object] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "SocialImpactMetrics":
        """Construct from a raw dict (e.g. contract RPC response).

        Unknown keys are stored in ``metadata``.

        Args:
            d: Raw dictionary.

        Returns:
            A :class:`SocialImpactMetrics` instance.
        """
        known = {
            "period", "jobs_created", "training_positions",
            "diversity_women_bps", "diversity_underrepresented_bps",
            "community_investment", "community_beneficiaries",
            "human_rights_assessment_done",
            "labour_violations_remediated", "collective_bargaining_agreements",
            "total_investment", "total_social_value",
            "recorded_at", "submitter",
        }
        metadata = {k: v for k, v in d.items() if k not in known}
        return cls(
            period=str(d.get("period", "")),
            jobs_created=int(d.get("jobs_created", 0)),  # type: ignore[arg-type]
            training_positions=int(d.get("training_positions", 0)),  # type: ignore[arg-type]
            diversity_women_bps=int(d.get("diversity_women_bps", 0)),  # type: ignore[arg-type]
            diversity_underrepresented_bps=int(d.get("diversity_underrepresented_bps", 0)),  # type: ignore[arg-type]
            community_investment=int(d.get("community_investment", 0)),  # type: ignore[arg-type]
            community_beneficiaries=int(d.get("community_beneficiaries", 0)),  # type: ignore[arg-type]
            human_rights_assessment_done=bool(d.get("human_rights_assessment_done", False)),
            labour_violations_remediated=int(d.get("labour_violations_remediated", 0)),  # type: ignore[arg-type]
            collective_bargaining_agreements=int(d.get("collective_bargaining_agreements", 0)),  # type: ignore[arg-type]
            total_investment=int(d.get("total_investment", 0)),  # type: ignore[arg-type]
            total_social_value=int(d.get("total_social_value", 0)),  # type: ignore[arg-type]
            recorded_at=int(d["recorded_at"]) if d.get("recorded_at") else None,  # type: ignore[arg-type]
            submitter=str(d["submitter"]) if d.get("submitter") else None,
            metadata=metadata,
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict suitable for JSON encoding."""
        return {
            "period": self.period,
            "jobs_created": self.jobs_created,
            "training_positions": self.training_positions,
            "diversity_women_bps": self.diversity_women_bps,
            "diversity_underrepresented_bps": self.diversity_underrepresented_bps,
            "community_investment": self.community_investment,
            "community_beneficiaries": self.community_beneficiaries,
            "human_rights_assessment_done": self.human_rights_assessment_done,
            "labour_violations_remediated": self.labour_violations_remediated,
            "collective_bargaining_agreements": self.collective_bargaining_agreements,
            "total_investment": self.total_investment,
            "total_social_value": self.total_social_value,
            "recorded_at": self.recorded_at,
            "submitter": self.submitter,
            **self.metadata,
        }


@dataclass
class Stakeholder:
    """Off-chain mirror of the on-chain ``Stakeholder`` struct.

    Attributes:
        address: Stellar address uniquely identifying this stakeholder.
        name: Human-readable name.
        category: Stakeholder category (``"worker"``, ``"community"``,
            ``"investor"``, ``"regulator"``, etc.).
        weight_bps: Impact weight in basis points (0–10 000).
        registered_at: Optional Unix timestamp of on-chain registration.
    """

    address: str
    name: str
    category: str
    weight_bps: int = 5000
    registered_at: Optional[int] = None

    @classmethod
    def from_dict(cls, d: Dict[str, object]) -> "Stakeholder":
        """Construct from a raw dict."""
        return cls(
            address=str(d["address"]),
            name=str(d["name"]),
            category=str(d["category"]),
            weight_bps=int(d.get("weight_bps", 5000)),  # type: ignore[arg-type]
            registered_at=int(d["registered_at"]) if d.get("registered_at") else None,  # type: ignore[arg-type]
        )

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "address": self.address,
            "name": self.name,
            "category": self.category,
            "weight_bps": self.weight_bps,
            "registered_at": self.registered_at,
        }


@dataclass
class ImpactReport:
    """Aggregated social impact report, mirroring the on-chain ``ImpactReport`` struct.

    All monetary amounts are in whole reference-currency units.

    Attributes:
        periods_included: Number of reporting periods in this report.
        total_jobs_created: Sum of jobs created across all periods.
        total_training_positions: Sum of training positions across all periods.
        total_community_investment: Sum of community investment across all periods.
        total_community_beneficiaries: Sum of community beneficiaries.
        avg_diversity_women_bps: Average ``diversity_women_bps`` across periods.
        avg_diversity_underrepresented_bps: Average underrepresented-group diversity.
        total_social_value: Sum of ``total_social_value`` across all periods.
        total_investment: Sum of ``total_investment`` across all periods.
        sroi: SROI ratio as a float (e.g. ``3.5``).
        sroi_bps: SROI as integer basis points (e.g. ``35000``).
        human_rights_assessments: Number of periods where assessment was done.
        total_labour_violations_remediated: Total violations remediated.
        total_collective_bargaining_agreements: Total active CBAs (latest period).
        stakeholder_count: Total stakeholders registered.
        periods: List of period tags included.
        generated_at: Optional Unix timestamp of report generation.
    """

    periods_included: int = 0
    total_jobs_created: int = 0
    total_training_positions: int = 0
    total_community_investment: int = 0
    total_community_beneficiaries: int = 0
    avg_diversity_women_bps: int = 0
    avg_diversity_underrepresented_bps: int = 0
    total_social_value: int = 0
    total_investment: int = 0
    sroi: float = 0.0
    sroi_bps: int = 0
    human_rights_assessments: int = 0
    total_labour_violations_remediated: int = 0
    total_collective_bargaining_agreements: int = 0
    stakeholder_count: int = 0
    periods: List[str] = field(default_factory=list)
    generated_at: Optional[int] = None

    def to_dict(self) -> Dict[str, object]:
        """Serialise to a plain dict."""
        return {
            "periods_included": self.periods_included,
            "total_jobs_created": self.total_jobs_created,
            "total_training_positions": self.total_training_positions,
            "total_community_investment": self.total_community_investment,
            "total_community_beneficiaries": self.total_community_beneficiaries,
            "avg_diversity_women_bps": self.avg_diversity_women_bps,
            "avg_diversity_underrepresented_bps": self.avg_diversity_underrepresented_bps,
            "total_social_value": self.total_social_value,
            "total_investment": self.total_investment,
            "sroi": self.sroi,
            "sroi_bps": self.sroi_bps,
            "human_rights_assessments": self.human_rights_assessments,
            "total_labour_violations_remediated": self.total_labour_violations_remediated,
            "total_collective_bargaining_agreements": self.total_collective_bargaining_agreements,
            "stakeholder_count": self.stakeholder_count,
            "periods": self.periods,
            "generated_at": self.generated_at,
        }

    def to_json(self, indent: int = 2) -> str:
        """Serialise to a JSON string.

        Args:
            indent: JSON indentation width.

        Returns:
            JSON-encoded report string.
        """
        return json.dumps(self.to_dict(), indent=indent)


# ---------------------------------------------------------------------------
# Standalone helper functions
# ---------------------------------------------------------------------------


def calculate_sroi(
    metrics: Sequence[SocialImpactMetrics],
    *,
    deadweight_bps: int = 0,
    attribution_bps: int = 10_000,
    displacement_bps: int = 0,
) -> float:
    """Calculate the Social Return on Investment (SROI) ratio.

    Applies standard SROI Network adjustments:

    - **Deadweight** (``deadweight_bps``): proportion of the outcome that would
      have happened anyway (0–10 000). Reduces social value.
    - **Attribution** (``attribution_bps``): proportion attributable to the
      organisation as opposed to other actors (0–10 000). Defaults to full
      (10 000 bps = 100%).
    - **Displacement** (``displacement_bps``): negative outcomes displaced onto
      others (0–10 000). Reduces social value.

    Effective social value per period after adjustments:
    ``adjusted = value × (1 - deadweight/10000) × (attribution/10000) × (1 - displacement/10000)``

    Args:
        metrics: Sequence of :class:`SocialImpactMetrics` records.
        deadweight_bps: Deadweight adjustment in basis points (default 0).
        attribution_bps: Attribution adjustment in basis points (default 10 000).
        displacement_bps: Displacement adjustment in basis points (default 0).

    Returns:
        SROI as a float ratio (e.g. ``3.5`` for £3.50 return per £1 invested).

    Raises:
        ValueError: If total investment across all periods is zero.
        ValueError: If any basis-point parameter is outside 0–10 000.
    """
    for name, val in (
        ("deadweight_bps", deadweight_bps),
        ("attribution_bps", attribution_bps),
        ("displacement_bps", displacement_bps),
    ):
        if not (0 <= val <= 10_000):
            raise ValueError(f"{name} must be in 0–10000, got {val}")

    total_inv = sum(m.total_investment for m in metrics)
    if total_inv == 0:
        raise ValueError(
            "Total investment is zero across all periods. "
            "SROI cannot be calculated without investment data."
        )

    factor = (
        (1.0 - deadweight_bps / 10_000.0)
        * (attribution_bps / 10_000.0)
        * (1.0 - displacement_bps / 10_000.0)
    )
    total_val = sum(m.total_social_value * factor for m in metrics)
    return total_val / total_inv


def diversity_score(metrics: Sequence[SocialImpactMetrics]) -> Dict[str, float]:
    """Compute average diversity scores across all reporting periods.

    Args:
        metrics: Sequence of :class:`SocialImpactMetrics` records.

    Returns:
        Dict with keys:

        - ``women_pct``: Average percentage of women in the workforce (0–100).
        - ``underrepresented_pct``: Average percentage from under-represented groups.
        - ``women_bps``: Same as ``women_pct`` but in basis points (0–10 000).
        - ``underrepresented_bps``: Same as ``underrepresented_pct`` but in bps.

    Returns zero values when no metrics are provided.
    """
    if not metrics:
        return {
            "women_pct": 0.0,
            "underrepresented_pct": 0.0,
            "women_bps": 0.0,
            "underrepresented_bps": 0.0,
        }

    n = len(metrics)
    avg_women_bps = sum(m.diversity_women_bps for m in metrics) / n
    avg_under_bps = sum(m.diversity_underrepresented_bps for m in metrics) / n
    return {
        "women_pct": avg_women_bps / 100.0,
        "underrepresented_pct": avg_under_bps / 100.0,
        "women_bps": avg_women_bps,
        "underrepresented_bps": avg_under_bps,
    }


def community_investment_summary(
    metrics: Sequence[SocialImpactMetrics],
) -> Dict[str, object]:
    """Aggregate community investment metrics across periods.

    Args:
        metrics: Sequence of :class:`SocialImpactMetrics` records.

    Returns:
        Dict with:

        - ``total_investment``: Sum of ``community_investment`` across periods.
        - ``total_beneficiaries``: Sum of ``community_beneficiaries``.
        - ``avg_investment_per_period``: Mean community investment.
        - ``cost_per_beneficiary``: Total investment divided by total beneficiaries,
          or ``None`` if beneficiaries are zero.
        - ``period_count``: Number of periods aggregated.
    """
    if not metrics:
        return {
            "total_investment": 0,
            "total_beneficiaries": 0,
            "avg_investment_per_period": 0.0,
            "cost_per_beneficiary": None,
            "period_count": 0,
        }

    total_inv = sum(m.community_investment for m in metrics)
    total_ben = sum(m.community_beneficiaries for m in metrics)
    n = len(metrics)
    cost_per_ben: Optional[float] = (total_inv / total_ben) if total_ben > 0 else None

    return {
        "total_investment": total_inv,
        "total_beneficiaries": total_ben,
        "avg_investment_per_period": total_inv / n,
        "cost_per_beneficiary": cost_per_ben,
        "period_count": n,
    }


def labour_compliance_rate(metrics: Sequence[SocialImpactMetrics]) -> Dict[str, object]:
    """Summarise labour standards compliance across reporting periods.

    Args:
        metrics: Sequence of :class:`SocialImpactMetrics` records.

    Returns:
        Dict with:

        - ``total_violations_remediated``: Sum of remediated violations.
        - ``total_cba_agreements``: Sum of collective-bargaining agreements.
        - ``human_rights_assessment_rate_pct``: Percentage of periods where a
          human-rights assessment was completed (0–100).
        - ``period_count``: Number of periods aggregated.
    """
    if not metrics:
        return {
            "total_violations_remediated": 0,
            "total_cba_agreements": 0,
            "human_rights_assessment_rate_pct": 0.0,
            "period_count": 0,
        }

    n = len(metrics)
    total_violations = sum(m.labour_violations_remediated for m in metrics)
    total_cba = sum(m.collective_bargaining_agreements for m in metrics)
    hr_done = sum(1 for m in metrics if m.human_rights_assessment_done)

    return {
        "total_violations_remediated": total_violations,
        "total_cba_agreements": total_cba,
        "human_rights_assessment_rate_pct": (hr_done / n) * 100.0,
        "period_count": n,
    }


def aggregate_impact_report(
    metrics: Sequence[SocialImpactMetrics],
    stakeholders: Optional[Sequence[Stakeholder]] = None,
    *,
    deadweight_bps: int = 0,
    attribution_bps: int = 10_000,
    displacement_bps: int = 0,
    generated_at: Optional[int] = None,
) -> ImpactReport:
    """Build an :class:`ImpactReport` from a list of metrics and optional stakeholders.

    This is the off-chain equivalent of the on-chain ``generate_impact_report``
    contract function.  All SROI adjustments are applied before storing into the
    report.

    Args:
        metrics: Sequence of :class:`SocialImpactMetrics` records to aggregate.
        stakeholders: Optional list of registered :class:`Stakeholder` objects.
        deadweight_bps: Deadweight adjustment (0–10 000). Default 0.
        attribution_bps: Attribution fraction (0–10 000). Default 10 000 (100%).
        displacement_bps: Displacement adjustment (0–10 000). Default 0.
        generated_at: Optional Unix timestamp; if omitted the caller must supply
            their own timestamp.

    Returns:
        A populated :class:`ImpactReport`.

    Raises:
        ValueError: If total investment is zero (SROI cannot be computed).
    """
    if not metrics:
        return ImpactReport(generated_at=generated_at)

    n = len(metrics)
    total_jobs = sum(m.jobs_created for m in metrics)
    total_training = sum(m.training_positions for m in metrics)
    total_comm_inv = sum(m.community_investment for m in metrics)
    total_comm_ben = sum(m.community_beneficiaries for m in metrics)
    avg_diversity_women = int(sum(m.diversity_women_bps for m in metrics) / n)
    avg_diversity_under = int(sum(m.diversity_underrepresented_bps for m in metrics) / n)
    total_social_value = sum(m.total_social_value for m in metrics)
    total_investment = sum(m.total_investment for m in metrics)
    hr_done = sum(1 for m in metrics if m.human_rights_assessment_done)
    total_violations = sum(m.labour_violations_remediated for m in metrics)
    total_cba = sum(m.collective_bargaining_agreements for m in metrics)
    period_tags = [m.period for m in metrics]

    sroi_ratio = calculate_sroi(
        metrics,
        deadweight_bps=deadweight_bps,
        attribution_bps=attribution_bps,
        displacement_bps=displacement_bps,
    )
    sroi_bps = int(sroi_ratio * 10_000)

    return ImpactReport(
        periods_included=n,
        total_jobs_created=total_jobs,
        total_training_positions=total_training,
        total_community_investment=total_comm_inv,
        total_community_beneficiaries=total_comm_ben,
        avg_diversity_women_bps=avg_diversity_women,
        avg_diversity_underrepresented_bps=avg_diversity_under,
        total_social_value=total_social_value,
        total_investment=total_investment,
        sroi=sroi_ratio,
        sroi_bps=sroi_bps,
        human_rights_assessments=hr_done,
        total_labour_violations_remediated=total_violations,
        total_collective_bargaining_agreements=total_cba,
        stakeholder_count=len(stakeholders) if stakeholders is not None else 0,
        periods=period_tags,
        generated_at=generated_at,
    )


# ---------------------------------------------------------------------------
# SocialImpactAnalyzer
# ---------------------------------------------------------------------------


class SocialImpactAnalyzer:
    """High-level analytics engine for on-chain social impact data.

    Wraps a collection of :class:`SocialImpactMetrics` records and provides
    convenience methods for SROI calculation, diversity scoring, stakeholder
    engagement analysis, and impact report generation.

    Args:
        metrics: List of :class:`SocialImpactMetrics` records.
        stakeholders: Optional list of registered :class:`Stakeholder` objects.

    Example::

        analyzer = SocialImpactAnalyzer(
            metrics=[
                SocialImpactMetrics(
                    period="2026_Q1",
                    jobs_created=50,
                    total_investment=200_000,
                    total_social_value=700_000,
                    ...
                )
            ]
        )
        print(analyzer.sroi())           # 3.5
        print(analyzer.sroi_bps())       # 35000
        report = analyzer.impact_report()
    """

    def __init__(
        self,
        metrics: List[SocialImpactMetrics],
        stakeholders: Optional[List[Stakeholder]] = None,
    ) -> None:
        self._metrics = list(metrics)
        self._stakeholders: List[Stakeholder] = list(stakeholders) if stakeholders else []

    # ── Metrics management ────────────────────────────────────────────────

    def add_metrics(self, m: SocialImpactMetrics) -> None:
        """Append a new metrics record to the analyzer.

        Args:
            m: :class:`SocialImpactMetrics` to add.

        Raises:
            ValueError: If a record with the same period already exists.
        """
        if any(existing.period == m.period for existing in self._metrics):
            raise ValueError(f"Metrics for period '{m.period}' already exist.")
        self._metrics.append(m)

    def get_metrics(self, period: str) -> SocialImpactMetrics:
        """Retrieve metrics for a specific period.

        Args:
            period: Period tag to look up.

        Returns:
            The matching :class:`SocialImpactMetrics`.

        Raises:
            KeyError: If no record exists for the given period.
        """
        for m in self._metrics:
            if m.period == period:
                return m
        raise KeyError(f"No social impact record for period '{period}'.")

    @property
    def periods(self) -> List[str]:
        """Return the list of all recorded period tags."""
        return [m.period for m in self._metrics]

    @property
    def period_count(self) -> int:
        """Return the number of recorded periods."""
        return len(self._metrics)

    # ── Stakeholder management ────────────────────────────────────────────

    def add_stakeholder(self, stakeholder: Stakeholder) -> None:
        """Register a stakeholder.

        Args:
            stakeholder: :class:`Stakeholder` to register.

        Raises:
            ValueError: If a stakeholder with the same address is already registered.
        """
        if any(s.address == stakeholder.address for s in self._stakeholders):
            raise ValueError(
                f"Stakeholder with address '{stakeholder.address}' is already registered."
            )
        self._stakeholders.append(stakeholder)

    def get_stakeholder(self, address: str) -> Stakeholder:
        """Look up a stakeholder by Stellar address.

        Args:
            address: Stellar address to look up.

        Returns:
            The matching :class:`Stakeholder`.

        Raises:
            KeyError: If no stakeholder is registered for the address.
        """
        for s in self._stakeholders:
            if s.address == address:
                return s
        raise KeyError(f"No stakeholder registered for address '{address}'.")

    @property
    def stakeholders(self) -> List[Stakeholder]:
        """Return the list of all registered stakeholders."""
        return list(self._stakeholders)

    def stakeholder_count(self) -> int:
        """Return the total number of registered stakeholders."""
        return len(self._stakeholders)

    def stakeholders_by_category(self) -> Dict[str, List[Stakeholder]]:
        """Group stakeholders by category.

        Returns:
            Dict mapping category string to list of :class:`Stakeholder`.
        """
        result: Dict[str, List[Stakeholder]] = {}
        for s in self._stakeholders:
            result.setdefault(s.category, []).append(s)
        return result

    # ── SROI ──────────────────────────────────────────────────────────────

    def sroi(
        self,
        *,
        deadweight_bps: int = 0,
        attribution_bps: int = 10_000,
        displacement_bps: int = 0,
    ) -> float:
        """Return the SROI ratio as a float.

        Args:
            deadweight_bps: Deadweight adjustment in basis points (0–10 000).
            attribution_bps: Attribution fraction in basis points (0–10 000).
            displacement_bps: Displacement adjustment in basis points (0–10 000).

        Returns:
            SROI as a float (e.g. ``3.5``).

        Raises:
            ValueError: If total investment is zero.
        """
        return calculate_sroi(
            self._metrics,
            deadweight_bps=deadweight_bps,
            attribution_bps=attribution_bps,
            displacement_bps=displacement_bps,
        )

    def sroi_bps(
        self,
        *,
        deadweight_bps: int = 0,
        attribution_bps: int = 10_000,
        displacement_bps: int = 0,
    ) -> int:
        """Return the SROI ratio in basis points (on-chain storage format).

        Args:
            deadweight_bps: Deadweight adjustment in basis points (0–10 000).
            attribution_bps: Attribution fraction in basis points (0–10 000).
            displacement_bps: Displacement adjustment in basis points (0–10 000).

        Returns:
            SROI × 10 000, e.g. ``35000`` for a 3.5× return.

        Raises:
            ValueError: If total investment is zero.
        """
        return int(
            self.sroi(
                deadweight_bps=deadweight_bps,
                attribution_bps=attribution_bps,
                displacement_bps=displacement_bps,
            )
            * 10_000
        )

    # ── Diversity ─────────────────────────────────────────────────────────

    def diversity(self) -> Dict[str, float]:
        """Compute average diversity scores across all periods.

        Returns:
            Dict with ``women_pct``, ``underrepresented_pct``, ``women_bps``,
            ``underrepresented_bps``.
        """
        return diversity_score(self._metrics)

    # ── Community investment ──────────────────────────────────────────────

    def community_investment(self) -> Dict[str, object]:
        """Aggregate community investment metrics.

        Returns:
            Dict with ``total_investment``, ``total_beneficiaries``,
            ``avg_investment_per_period``, ``cost_per_beneficiary``,
            ``period_count``.
        """
        return community_investment_summary(self._metrics)

    # ── Labour standards ──────────────────────────────────────────────────

    def labour_compliance(self) -> Dict[str, object]:
        """Summarise labour-standards compliance metrics.

        Returns:
            Dict with ``total_violations_remediated``, ``total_cba_agreements``,
            ``human_rights_assessment_rate_pct``, ``period_count``.
        """
        return labour_compliance_rate(self._metrics)

    # ── Job creation ──────────────────────────────────────────────────────

    def job_creation_summary(self) -> Dict[str, object]:
        """Summarise job creation metrics across all periods.

        Returns:
            Dict with ``total_jobs_created``, ``total_training_positions``,
            ``avg_jobs_per_period``, ``period_count``.
        """
        if not self._metrics:
            return {
                "total_jobs_created": 0,
                "total_training_positions": 0,
                "avg_jobs_per_period": 0.0,
                "period_count": 0,
            }
        total_jobs = sum(m.jobs_created for m in self._metrics)
        total_training = sum(m.training_positions for m in self._metrics)
        n = len(self._metrics)
        return {
            "total_jobs_created": total_jobs,
            "total_training_positions": total_training,
            "avg_jobs_per_period": total_jobs / n,
            "period_count": n,
        }

    # ── Weighted stakeholder impact ───────────────────────────────────────

    def weighted_impact_by_stakeholder_category(self) -> Dict[str, float]:
        """Distribute total social value across stakeholder categories by weight.

        Stakeholder weights (``weight_bps``) are normalised to sum to 1.0 across
        all stakeholders.  Each category's share is the sum of the normalised
        weights for all stakeholders in that category, multiplied by total social
        value across all recorded periods.

        Returns:
            Dict mapping category → attributed social value (float). An empty
            dict is returned when there are no stakeholders or no metrics.
        """
        if not self._stakeholders or not self._metrics:
            return {}

        total_weight = sum(s.weight_bps for s in self._stakeholders)
        if total_weight == 0:
            return {}

        total_value = sum(m.total_social_value for m in self._metrics)
        category_weights: Dict[str, int] = {}
        for s in self._stakeholders:
            category_weights[s.category] = (
                category_weights.get(s.category, 0) + s.weight_bps
            )

        return {
            cat: (weight / total_weight) * total_value
            for cat, weight in category_weights.items()
        }

    # ── Full report ───────────────────────────────────────────────────────

    def impact_report(
        self,
        *,
        deadweight_bps: int = 0,
        attribution_bps: int = 10_000,
        displacement_bps: int = 0,
        generated_at: Optional[int] = None,
    ) -> ImpactReport:
        """Generate a full aggregated :class:`ImpactReport`.

        Args:
            deadweight_bps: Deadweight adjustment in basis points (0–10 000).
            attribution_bps: Attribution fraction in basis points (0–10 000).
            displacement_bps: Displacement adjustment in basis points (0–10 000).
            generated_at: Optional Unix timestamp for the report header.

        Returns:
            A populated :class:`ImpactReport`.

        Raises:
            ValueError: If total investment is zero.
        """
        return aggregate_impact_report(
            self._metrics,
            self._stakeholders if self._stakeholders else None,
            deadweight_bps=deadweight_bps,
            attribution_bps=attribution_bps,
            displacement_bps=displacement_bps,
            generated_at=generated_at,
        )


# ---------------------------------------------------------------------------
# Public re-exports
# ---------------------------------------------------------------------------

__all__ = [
    # Data models
    "SocialImpactMetrics",
    "Stakeholder",
    "ImpactReport",
    # Standalone helpers
    "calculate_sroi",
    "diversity_score",
    "community_investment_summary",
    "labour_compliance_rate",
    "aggregate_impact_report",
    # Analyzer class
    "SocialImpactAnalyzer",
]
