"""Tests for sdk/python/audit_ledger/social_impact.py.

Covers:
- SocialImpactMetrics serialisation / deserialisation
- Stakeholder serialisation / deserialisation
- ImpactReport serialisation and JSON output
- calculate_sroi: basic ratio, SROI-Network adjustments, zero-investment guard
- diversity_score: basis-point averages, percentage conversion
- community_investment_summary: totals, cost-per-beneficiary
- labour_compliance_rate: violation totals, HR-assessment rate
- aggregate_impact_report: full aggregation, deadweight adjustment
- SocialImpactAnalyzer: CRUD, sroi/sroi_bps, diversity, community,
  labour compliance, job creation, weighted impact, full report
"""

from __future__ import annotations

import json
import sys
import types

import pytest

# ---------------------------------------------------------------------------
# Import social_impact directly to avoid triggering audit_ledger/__init__.py
# which attempts to import client.py (pre-existing syntax issue in that file).
# Other test files in this project follow the same pattern of direct submodule
# imports.
# ---------------------------------------------------------------------------
if "audit_ledger" not in sys.modules:
    _pkg = types.ModuleType("audit_ledger")
    _pkg.__path__ = ["audit_ledger"]  # type: ignore[assignment]
    _pkg.__package__ = "audit_ledger"
    _pkg.__spec__ = None  # type: ignore[assignment]
    sys.modules["audit_ledger"] = _pkg

from audit_ledger.social_impact import (
    ImpactReport,
    SocialImpactAnalyzer,
    SocialImpactMetrics,
    Stakeholder,
    aggregate_impact_report,
    calculate_sroi,
    community_investment_summary,
    diversity_score,
    labour_compliance_rate,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def make_metrics(
    period: str = "2026_Q1",
    jobs_created: int = 50,
    training_positions: int = 10,
    diversity_women_bps: int = 4500,
    diversity_underrepresented_bps: int = 3000,
    community_investment: int = 100_000,
    community_beneficiaries: int = 500,
    human_rights_assessment_done: bool = True,
    labour_violations_remediated: int = 2,
    collective_bargaining_agreements: int = 3,
    total_investment: int = 200_000,
    total_social_value: int = 700_000,
) -> SocialImpactMetrics:
    """Build a minimal valid SocialImpactMetrics."""
    return SocialImpactMetrics(
        period=period,
        jobs_created=jobs_created,
        training_positions=training_positions,
        diversity_women_bps=diversity_women_bps,
        diversity_underrepresented_bps=diversity_underrepresented_bps,
        community_investment=community_investment,
        community_beneficiaries=community_beneficiaries,
        human_rights_assessment_done=human_rights_assessment_done,
        labour_violations_remediated=labour_violations_remediated,
        collective_bargaining_agreements=collective_bargaining_agreements,
        total_investment=total_investment,
        total_social_value=total_social_value,
    )


def make_stakeholder(
    address: str = "GABC1",
    name: str = "Community Group A",
    category: str = "community",
    weight_bps: int = 3000,
) -> Stakeholder:
    """Build a minimal valid Stakeholder."""
    return Stakeholder(
        address=address,
        name=name,
        category=category,
        weight_bps=weight_bps,
    )


# ---------------------------------------------------------------------------
# SocialImpactMetrics
# ---------------------------------------------------------------------------


class TestSocialImpactMetrics:
    def test_defaults(self):
        m = SocialImpactMetrics(period="2026_Q1")
        assert m.jobs_created == 0
        assert m.total_investment == 0
        assert m.total_social_value == 0
        assert m.human_rights_assessment_done is False
        assert m.metadata == {}
        assert m.submitter is None
        assert m.recorded_at is None

    def test_from_dict_complete(self):
        d = {
            "period": "2026_Q2",
            "jobs_created": 30,
            "training_positions": 5,
            "diversity_women_bps": 4000,
            "diversity_underrepresented_bps": 2500,
            "community_investment": 50_000,
            "community_beneficiaries": 200,
            "human_rights_assessment_done": True,
            "labour_violations_remediated": 1,
            "collective_bargaining_agreements": 2,
            "total_investment": 100_000,
            "total_social_value": 350_000,
            "recorded_at": 1_700_000_000,
            "submitter": "GABC1",
        }
        m = SocialImpactMetrics.from_dict(d)
        assert m.period == "2026_Q2"
        assert m.jobs_created == 30
        assert m.total_investment == 100_000
        assert m.total_social_value == 350_000
        assert m.human_rights_assessment_done is True
        assert m.recorded_at == 1_700_000_000
        assert m.submitter == "GABC1"
        assert m.metadata == {}

    def test_from_dict_unknown_keys_go_to_metadata(self):
        d = {
            "period": "2026_Q1",
            "total_investment": 10_000,
            "total_social_value": 30_000,
            "custom_field": "custom_value",
        }
        m = SocialImpactMetrics.from_dict(d)
        assert m.metadata == {"custom_field": "custom_value"}

    def test_from_dict_missing_optional_fields_default(self):
        m = SocialImpactMetrics.from_dict({"period": "2026_Q1"})
        assert m.jobs_created == 0
        assert m.total_investment == 0
        assert m.recorded_at is None

    def test_to_dict_roundtrip(self):
        m = make_metrics()
        d = m.to_dict()
        m2 = SocialImpactMetrics.from_dict(d)
        assert m2.period == m.period
        assert m2.jobs_created == m.jobs_created
        assert m2.total_investment == m.total_investment
        assert m2.human_rights_assessment_done == m.human_rights_assessment_done

    def test_to_dict_contains_all_known_keys(self):
        m = make_metrics()
        d = m.to_dict()
        for key in (
            "period", "jobs_created", "training_positions",
            "diversity_women_bps", "diversity_underrepresented_bps",
            "community_investment", "community_beneficiaries",
            "human_rights_assessment_done",
            "labour_violations_remediated", "collective_bargaining_agreements",
            "total_investment", "total_social_value",
        ):
            assert key in d, f"Key '{key}' missing from to_dict() output"


# ---------------------------------------------------------------------------
# Stakeholder
# ---------------------------------------------------------------------------


class TestStakeholder:
    def test_defaults(self):
        s = Stakeholder(address="GABC1", name="Test", category="community")
        assert s.weight_bps == 5000
        assert s.registered_at is None

    def test_from_dict(self):
        d = {
            "address": "GABC2",
            "name": "Worker Union",
            "category": "worker",
            "weight_bps": 7000,
            "registered_at": 1_700_000_000,
        }
        s = Stakeholder.from_dict(d)
        assert s.address == "GABC2"
        assert s.name == "Worker Union"
        assert s.category == "worker"
        assert s.weight_bps == 7000
        assert s.registered_at == 1_700_000_000

    def test_to_dict_roundtrip(self):
        s = make_stakeholder()
        d = s.to_dict()
        s2 = Stakeholder.from_dict(d)
        assert s2.address == s.address
        assert s2.name == s.name
        assert s2.category == s.category
        assert s2.weight_bps == s.weight_bps


# ---------------------------------------------------------------------------
# ImpactReport
# ---------------------------------------------------------------------------


class TestImpactReport:
    def test_default_values(self):
        r = ImpactReport()
        assert r.periods_included == 0
        assert r.sroi == 0.0
        assert r.sroi_bps == 0
        assert r.periods == []

    def test_to_dict_keys(self):
        r = ImpactReport(periods_included=2, sroi=3.5, sroi_bps=35_000)
        d = r.to_dict()
        for key in (
            "periods_included", "total_jobs_created", "total_community_investment",
            "sroi", "sroi_bps", "stakeholder_count", "periods",
        ):
            assert key in d

    def test_to_json(self):
        r = ImpactReport(periods_included=1, sroi=2.5, sroi_bps=25_000)
        j = r.to_json()
        parsed = json.loads(j)
        assert parsed["sroi"] == pytest.approx(2.5)
        assert parsed["sroi_bps"] == 25_000


# ---------------------------------------------------------------------------
# calculate_sroi
# ---------------------------------------------------------------------------


class TestCalculateSroi:
    def test_basic_ratio(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        result = calculate_sroi([m])
        assert result == pytest.approx(3.5)

    def test_sroi_below_one(self):
        m = make_metrics(total_investment=1_000_000, total_social_value=500_000)
        result = calculate_sroi([m])
        assert result == pytest.approx(0.5)

    def test_multiple_periods(self):
        # Period 1: inv=200_000, val=700_000
        # Period 2: inv=100_000, val=250_000
        # Total: inv=300_000, val=950_000 → 950_000/300_000 ≈ 3.1667
        m1 = make_metrics("2026_Q1", total_investment=200_000, total_social_value=700_000)
        m2 = make_metrics("2026_Q2", total_investment=100_000, total_social_value=250_000)
        result = calculate_sroi([m1, m2])
        assert result == pytest.approx(950_000 / 300_000)

    def test_zero_investment_raises(self):
        m = make_metrics(total_investment=0, total_social_value=100_000)
        with pytest.raises(ValueError, match="zero"):
            calculate_sroi([m])

    def test_deadweight_adjustment(self):
        # Deadweight 20% → effective value = 700_000 * 0.8 = 560_000
        # SROI = 560_000 / 200_000 = 2.8
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        result = calculate_sroi([m], deadweight_bps=2000)
        assert result == pytest.approx(2.8)

    def test_attribution_adjustment(self):
        # Attribution 50% → effective value = 700_000 * 0.5 = 350_000
        # SROI = 350_000 / 200_000 = 1.75
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        result = calculate_sroi([m], attribution_bps=5000)
        assert result == pytest.approx(1.75)

    def test_displacement_adjustment(self):
        # Displacement 10% → effective value = 700_000 * 0.9 = 630_000
        # SROI = 630_000 / 200_000 = 3.15
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        result = calculate_sroi([m], displacement_bps=1000)
        assert result == pytest.approx(3.15)

    def test_combined_adjustments(self):
        # deadweight=10%, attribution=90%, displacement=5%
        # factor = 0.9 * 0.9 * 0.95 = 0.7695
        # effective = 700_000 * 0.7695 = 538_650
        # SROI = 538_650 / 200_000 = 2.69325
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        result = calculate_sroi([m], deadweight_bps=1000, attribution_bps=9000, displacement_bps=500)
        expected = 700_000 * (0.9 * 0.9 * 0.95) / 200_000
        assert result == pytest.approx(expected, rel=1e-6)

    def test_invalid_bps_raises(self):
        m = make_metrics(total_investment=100, total_social_value=300)
        with pytest.raises(ValueError):
            calculate_sroi([m], deadweight_bps=10_001)
        with pytest.raises(ValueError):
            calculate_sroi([m], attribution_bps=-1)
        with pytest.raises(ValueError):
            calculate_sroi([m], displacement_bps=10_001)

    def test_empty_list_raises(self):
        with pytest.raises((ValueError, ZeroDivisionError)):
            calculate_sroi([])


# ---------------------------------------------------------------------------
# diversity_score
# ---------------------------------------------------------------------------


class TestDiversityScore:
    def test_single_period(self):
        m = make_metrics(
            diversity_women_bps=4500,
            diversity_underrepresented_bps=3000,
        )
        result = diversity_score([m])
        assert result["women_bps"] == pytest.approx(4500)
        assert result["underrepresented_bps"] == pytest.approx(3000)
        assert result["women_pct"] == pytest.approx(45.0)
        assert result["underrepresented_pct"] == pytest.approx(30.0)

    def test_average_across_periods(self):
        m1 = make_metrics("2026_Q1", diversity_women_bps=4000)
        m2 = make_metrics("2026_Q2", diversity_women_bps=5000)
        result = diversity_score([m1, m2])
        assert result["women_bps"] == pytest.approx(4500)
        assert result["women_pct"] == pytest.approx(45.0)

    def test_empty_list(self):
        result = diversity_score([])
        assert result["women_pct"] == 0.0
        assert result["underrepresented_pct"] == 0.0
        assert result["women_bps"] == 0.0
        assert result["underrepresented_bps"] == 0.0


# ---------------------------------------------------------------------------
# community_investment_summary
# ---------------------------------------------------------------------------


class TestCommunityInvestmentSummary:
    def test_basic(self):
        m = make_metrics(community_investment=100_000, community_beneficiaries=500)
        result = community_investment_summary([m])
        assert result["total_investment"] == 100_000
        assert result["total_beneficiaries"] == 500
        assert result["cost_per_beneficiary"] == pytest.approx(200.0)
        assert result["period_count"] == 1

    def test_multiple_periods(self):
        m1 = make_metrics("2026_Q1", community_investment=100_000, community_beneficiaries=400)
        m2 = make_metrics("2026_Q2", community_investment=60_000, community_beneficiaries=200)
        result = community_investment_summary([m1, m2])
        assert result["total_investment"] == 160_000
        assert result["total_beneficiaries"] == 600
        assert result["avg_investment_per_period"] == pytest.approx(80_000.0)
        assert result["cost_per_beneficiary"] == pytest.approx(160_000 / 600)

    def test_zero_beneficiaries(self):
        m = make_metrics(community_investment=100_000, community_beneficiaries=0)
        result = community_investment_summary([m])
        assert result["cost_per_beneficiary"] is None

    def test_empty_list(self):
        result = community_investment_summary([])
        assert result["total_investment"] == 0
        assert result["total_beneficiaries"] == 0
        assert result["period_count"] == 0


# ---------------------------------------------------------------------------
# labour_compliance_rate
# ---------------------------------------------------------------------------


class TestLabourComplianceRate:
    def test_basic(self):
        m = make_metrics(
            labour_violations_remediated=5,
            collective_bargaining_agreements=3,
            human_rights_assessment_done=True,
        )
        result = labour_compliance_rate([m])
        assert result["total_violations_remediated"] == 5
        assert result["total_cba_agreements"] == 3
        assert result["human_rights_assessment_rate_pct"] == pytest.approx(100.0)

    def test_partial_hr_assessments(self):
        m1 = make_metrics("2026_Q1", human_rights_assessment_done=True)
        m2 = make_metrics("2026_Q2", human_rights_assessment_done=False)
        m3 = make_metrics("2026_Q3", human_rights_assessment_done=True)
        result = labour_compliance_rate([m1, m2, m3])
        assert result["human_rights_assessment_rate_pct"] == pytest.approx(100.0 * 2 / 3)

    def test_no_assessments(self):
        m1 = make_metrics("2026_Q1", human_rights_assessment_done=False)
        m2 = make_metrics("2026_Q2", human_rights_assessment_done=False)
        result = labour_compliance_rate([m1, m2])
        assert result["human_rights_assessment_rate_pct"] == 0.0

    def test_empty_list(self):
        result = labour_compliance_rate([])
        assert result["total_violations_remediated"] == 0
        assert result["period_count"] == 0


# ---------------------------------------------------------------------------
# aggregate_impact_report
# ---------------------------------------------------------------------------


class TestAggregateImpactReport:
    def test_empty_metrics_returns_empty_report(self):
        report = aggregate_impact_report([])
        assert report.periods_included == 0
        assert report.total_jobs_created == 0

    def test_single_period_basic(self):
        m = make_metrics(
            jobs_created=50,
            total_investment=200_000,
            total_social_value=700_000,
            community_investment=100_000,
            community_beneficiaries=500,
        )
        report = aggregate_impact_report([m])
        assert report.periods_included == 1
        assert report.total_jobs_created == 50
        assert report.total_community_investment == 100_000
        assert report.total_investment == 200_000
        assert report.total_social_value == 700_000
        assert report.sroi == pytest.approx(3.5)
        assert report.sroi_bps == 35_000

    def test_multiple_periods(self):
        m1 = make_metrics("2026_Q1", jobs_created=50, total_investment=200_000, total_social_value=700_000)
        m2 = make_metrics("2026_Q2", jobs_created=30, total_investment=100_000, total_social_value=300_000)
        report = aggregate_impact_report([m1, m2])
        assert report.periods_included == 2
        assert report.total_jobs_created == 80
        assert report.total_investment == 300_000
        assert report.total_social_value == 1_000_000
        assert report.sroi == pytest.approx(1_000_000 / 300_000)
        assert report.periods == ["2026_Q1", "2026_Q2"]

    def test_stakeholder_count_reflected(self):
        m = make_metrics()
        stakeholders = [make_stakeholder("GABC1"), make_stakeholder("GABC2")]
        report = aggregate_impact_report([m], stakeholders)
        assert report.stakeholder_count == 2

    def test_deadweight_reduces_sroi(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        report_no_dw = aggregate_impact_report([m])
        report_dw = aggregate_impact_report([m], deadweight_bps=2000)
        assert report_dw.sroi < report_no_dw.sroi

    def test_average_diversity(self):
        m1 = make_metrics("2026_Q1", diversity_women_bps=4000)
        m2 = make_metrics("2026_Q2", diversity_women_bps=5000)
        report = aggregate_impact_report([m1, m2])
        assert report.avg_diversity_women_bps == 4500

    def test_generated_at_passthrough(self):
        m = make_metrics()
        report = aggregate_impact_report([m], generated_at=1_700_000_000)
        assert report.generated_at == 1_700_000_000

    def test_zero_investment_raises(self):
        m = make_metrics(total_investment=0, total_social_value=100_000)
        with pytest.raises(ValueError):
            aggregate_impact_report([m])


# ---------------------------------------------------------------------------
# SocialImpactAnalyzer
# ---------------------------------------------------------------------------


class TestSocialImpactAnalyzer:
    # ── Metrics CRUD ──────────────────────────────────────────────────────

    def test_initial_state(self):
        analyzer = SocialImpactAnalyzer([])
        assert analyzer.period_count == 0
        assert analyzer.periods == []

    def test_add_metrics(self):
        analyzer = SocialImpactAnalyzer([])
        m = make_metrics("2026_Q1")
        analyzer.add_metrics(m)
        assert analyzer.period_count == 1
        assert "2026_Q1" in analyzer.periods

    def test_add_duplicate_period_raises(self):
        m1 = make_metrics("2026_Q1")
        m2 = make_metrics("2026_Q1")
        analyzer = SocialImpactAnalyzer([m1])
        with pytest.raises(ValueError, match="already exist"):
            analyzer.add_metrics(m2)

    def test_get_metrics_found(self):
        m = make_metrics("2026_Q1")
        analyzer = SocialImpactAnalyzer([m])
        assert analyzer.get_metrics("2026_Q1").jobs_created == m.jobs_created

    def test_get_metrics_not_found(self):
        analyzer = SocialImpactAnalyzer([])
        with pytest.raises(KeyError):
            analyzer.get_metrics("2026_Q1")

    # ── Stakeholder CRUD ──────────────────────────────────────────────────

    def test_add_and_get_stakeholder(self):
        analyzer = SocialImpactAnalyzer([])
        s = make_stakeholder("GABC1", category="community")
        analyzer.add_stakeholder(s)
        assert analyzer.stakeholder_count() == 1
        retrieved = analyzer.get_stakeholder("GABC1")
        assert retrieved.category == "community"

    def test_add_duplicate_stakeholder_raises(self):
        s = make_stakeholder("GABC1")
        analyzer = SocialImpactAnalyzer([], [s])
        with pytest.raises(ValueError, match="already registered"):
            analyzer.add_stakeholder(s)

    def test_get_stakeholder_not_found(self):
        analyzer = SocialImpactAnalyzer([])
        with pytest.raises(KeyError):
            analyzer.get_stakeholder("GNONE")

    def test_stakeholders_by_category(self):
        s1 = make_stakeholder("GABC1", category="community")
        s2 = make_stakeholder("GABC2", category="worker")
        s3 = make_stakeholder("GABC3", category="community")
        analyzer = SocialImpactAnalyzer([], [s1, s2, s3])
        grouped = analyzer.stakeholders_by_category()
        assert len(grouped["community"]) == 2
        assert len(grouped["worker"]) == 1

    # ── SROI ──────────────────────────────────────────────────────────────

    def test_sroi_basic(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        analyzer = SocialImpactAnalyzer([m])
        assert analyzer.sroi() == pytest.approx(3.5)

    def test_sroi_bps(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        analyzer = SocialImpactAnalyzer([m])
        assert analyzer.sroi_bps() == 35_000

    def test_sroi_with_deadweight(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        analyzer = SocialImpactAnalyzer([m])
        sroi_no_dw = analyzer.sroi()
        sroi_with_dw = analyzer.sroi(deadweight_bps=2000)
        assert sroi_with_dw < sroi_no_dw

    def test_sroi_zero_investment_raises(self):
        m = make_metrics(total_investment=0, total_social_value=100_000)
        analyzer = SocialImpactAnalyzer([m])
        with pytest.raises(ValueError):
            analyzer.sroi()

    # ── Diversity ─────────────────────────────────────────────────────────

    def test_diversity(self):
        m = make_metrics(diversity_women_bps=4500, diversity_underrepresented_bps=3000)
        analyzer = SocialImpactAnalyzer([m])
        d = analyzer.diversity()
        assert d["women_pct"] == pytest.approx(45.0)
        assert d["underrepresented_pct"] == pytest.approx(30.0)

    # ── Community investment ──────────────────────────────────────────────

    def test_community_investment(self):
        m = make_metrics(community_investment=100_000, community_beneficiaries=500)
        analyzer = SocialImpactAnalyzer([m])
        ci = analyzer.community_investment()
        assert ci["total_investment"] == 100_000
        assert ci["total_beneficiaries"] == 500
        assert ci["cost_per_beneficiary"] == pytest.approx(200.0)

    # ── Labour compliance ─────────────────────────────────────────────────

    def test_labour_compliance(self):
        m = make_metrics(
            labour_violations_remediated=4,
            collective_bargaining_agreements=2,
            human_rights_assessment_done=True,
        )
        analyzer = SocialImpactAnalyzer([m])
        lc = analyzer.labour_compliance()
        assert lc["total_violations_remediated"] == 4
        assert lc["total_cba_agreements"] == 2
        assert lc["human_rights_assessment_rate_pct"] == pytest.approx(100.0)

    # ── Job creation ──────────────────────────────────────────────────────

    def test_job_creation_summary(self):
        m1 = make_metrics("2026_Q1", jobs_created=50, training_positions=10)
        m2 = make_metrics("2026_Q2", jobs_created=30, training_positions=5)
        analyzer = SocialImpactAnalyzer([m1, m2])
        jc = analyzer.job_creation_summary()
        assert jc["total_jobs_created"] == 80
        assert jc["total_training_positions"] == 15
        assert jc["avg_jobs_per_period"] == pytest.approx(40.0)
        assert jc["period_count"] == 2

    def test_job_creation_empty(self):
        analyzer = SocialImpactAnalyzer([])
        jc = analyzer.job_creation_summary()
        assert jc["total_jobs_created"] == 0
        assert jc["avg_jobs_per_period"] == 0.0

    # ── Weighted stakeholder impact ───────────────────────────────────────

    def test_weighted_impact_by_category(self):
        # 1 community stakeholder (weight 3000), 1 worker stakeholder (weight 7000)
        # Total weight = 10_000
        # Total social value = 700_000
        # Community share = 3000/10000 * 700_000 = 210_000
        # Worker share    = 7000/10000 * 700_000 = 490_000
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        s1 = make_stakeholder("GABC1", category="community", weight_bps=3000)
        s2 = make_stakeholder("GABC2", category="worker", weight_bps=7000)
        analyzer = SocialImpactAnalyzer([m], [s1, s2])
        result = analyzer.weighted_impact_by_stakeholder_category()
        assert result["community"] == pytest.approx(210_000.0)
        assert result["worker"] == pytest.approx(490_000.0)

    def test_weighted_impact_no_stakeholders(self):
        m = make_metrics()
        analyzer = SocialImpactAnalyzer([m])
        assert analyzer.weighted_impact_by_stakeholder_category() == {}

    def test_weighted_impact_no_metrics(self):
        s = make_stakeholder("GABC1")
        analyzer = SocialImpactAnalyzer([], [s])
        assert analyzer.weighted_impact_by_stakeholder_category() == {}

    def test_weighted_impact_zero_weight(self):
        m = make_metrics()
        s = make_stakeholder("GABC1", weight_bps=0)
        analyzer = SocialImpactAnalyzer([m], [s])
        assert analyzer.weighted_impact_by_stakeholder_category() == {}

    # ── Full impact report ────────────────────────────────────────────────

    def test_impact_report_basic(self):
        m = make_metrics(
            jobs_created=50,
            total_investment=200_000,
            total_social_value=700_000,
        )
        s = make_stakeholder("GABC1")
        analyzer = SocialImpactAnalyzer([m], [s])
        report = analyzer.impact_report(generated_at=1_700_000_000)
        assert report.periods_included == 1
        assert report.total_jobs_created == 50
        assert report.sroi == pytest.approx(3.5)
        assert report.sroi_bps == 35_000
        assert report.stakeholder_count == 1
        assert report.generated_at == 1_700_000_000

    def test_impact_report_with_deadweight(self):
        m = make_metrics(total_investment=200_000, total_social_value=700_000)
        analyzer = SocialImpactAnalyzer([m])
        report = analyzer.impact_report(deadweight_bps=2000)
        # effective value = 700_000 * 0.8 = 560_000; SROI = 2.8
        assert report.sroi == pytest.approx(2.8)

    def test_impact_report_to_json_valid(self):
        m = make_metrics()
        analyzer = SocialImpactAnalyzer([m])
        report = analyzer.impact_report()
        j = report.to_json()
        parsed = json.loads(j)
        assert "sroi" in parsed
        assert "total_jobs_created" in parsed

    def test_impact_report_multiple_periods_aggregated(self):
        m1 = make_metrics("2026_Q1", jobs_created=50, total_investment=200_000, total_social_value=700_000)
        m2 = make_metrics("2026_Q2", jobs_created=30, total_investment=100_000, total_social_value=250_000)
        analyzer = SocialImpactAnalyzer([m1, m2])
        report = analyzer.impact_report()
        assert report.periods_included == 2
        assert report.total_jobs_created == 80
        assert report.total_investment == 300_000
        assert report.total_social_value == 950_000
        assert report.sroi == pytest.approx(950_000 / 300_000)

    def test_impact_report_zero_investment_raises(self):
        m = make_metrics(total_investment=0, total_social_value=100_000)
        analyzer = SocialImpactAnalyzer([m])
        with pytest.raises(ValueError):
            analyzer.impact_report()
