"""Tests for sdk/python/audit_ledger/modern_slavery.py.

Covers modern slavery risk assessment, supply chain mapping, training tracking,
due diligence investigations, policy management, and compliance reporting.
"""

from __future__ import annotations

import json
import sys
import types

import pytest

# Bypass broken __init__.py by creating minimal package stub
if "audit_ledger" not in sys.modules:
    _pkg = types.ModuleType("audit_ledger")
    _pkg.__path__ = ["audit_ledger"]  # type: ignore[assignment]
    _pkg.__package__ = "audit_ledger"
    _pkg.__spec__ = None  # type: ignore[assignment]
    sys.modules["audit_ledger"] = _pkg

from audit_ledger.modern_slavery import (
    DueDiligenceRecord,
    MSAPolicy,
    MSAReport,
    ModernSlaveryAnalyzer,
    RiskAssessment,
    SupplyChainNode,
    TrainingRecord,
    build_compliance_report,
    calculate_risk_score,
    remediation_progress,
    supply_chain_risk_summary,
    training_effectiveness,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def make_assessment(
    assessment_id: str = "2026_q1",
    scope: str = "global",
    risk_level: int = 1,
    high_risk_areas: int = 3,
    stakeholder_consultation_done: bool = True,
) -> RiskAssessment:
    """Build a minimal valid RiskAssessment."""
    return RiskAssessment(
        assessment_id=assessment_id,
        scope=scope,
        risk_level=risk_level,
        high_risk_areas=high_risk_areas,
        stakeholder_consultation_done=stakeholder_consultation_done,
    )


def make_node(
    supplier_id: str = "supplier_001",
    name: str = "Test Supplier",
    country: str = "CN",
    risk_level: int = 1,
    audited: bool = True,
) -> SupplyChainNode:
    """Build a minimal valid SupplyChainNode."""
    return SupplyChainNode(
        supplier_id=supplier_id,
        name=name,
        country=country,
        risk_level=risk_level,
        audited=audited,
    )


def make_training(
    training_id: str = "train_001",
    topic: str = "msa_awareness",
    attendees: int = 100,
    risk_assessment_covered: bool = True,
    due_diligence_covered: bool = True,
) -> TrainingRecord:
    """Build a minimal valid TrainingRecord."""
    return TrainingRecord(
        training_id=training_id,
        topic=topic,
        attendees=attendees,
        risk_assessment_covered=risk_assessment_covered,
        due_diligence_covered=due_diligence_covered,
    )


def make_dd_record(
    record_id: str = "dd_001",
    subject: str = "supplier_001",
    scope: str = "labour_practices",
    risk_level: int = 1,
    corrective_actions_required: int = 2,
    corrective_actions_completed_pct: int = 50,
) -> DueDiligenceRecord:
    """Build a minimal valid DueDiligenceRecord."""
    return DueDiligenceRecord(
        record_id=record_id,
        subject=subject,
        scope=scope,
        risk_level=risk_level,
        corrective_actions_required=corrective_actions_required,
        corrective_actions_completed_pct=corrective_actions_completed_pct,
    )


def make_policy(
    policy_id: str = "policy_001",
    version: int = 1,
    scope: str = "global",
    stakeholder_input_included: bool = True,
) -> MSAPolicy:
    """Build a minimal valid MSAPolicy."""
    return MSAPolicy(
        policy_id=policy_id,
        version=version,
        scope=scope,
        stakeholder_input_included=stakeholder_input_included,
    )


# ---------------------------------------------------------------------------
# RiskAssessment
# ---------------------------------------------------------------------------


class TestRiskAssessment:
    def test_defaults(self):
        a = RiskAssessment(assessment_id="test")
        assert a.risk_level == 0
        assert a.high_risk_areas == 0
        assert a.stakeholder_consultation_done is False

    def test_from_dict(self):
        d = {
            "assessment_id": "2026_q1",
            "risk_level": 2,
            "high_risk_areas": 5,
            "stakeholder_consultation_done": True,
        }
        a = RiskAssessment.from_dict(d)
        assert a.assessment_id == "2026_q1"
        assert a.risk_level == 2
        assert a.high_risk_areas == 5

    def test_to_dict_roundtrip(self):
        a = make_assessment()
        d = a.to_dict()
        a2 = RiskAssessment.from_dict(d)
        assert a2.assessment_id == a.assessment_id
        assert a2.risk_level == a.risk_level


# ---------------------------------------------------------------------------
# SupplyChainNode
# ---------------------------------------------------------------------------


class TestSupplyChainNode:
    def test_from_dict(self):
        d = {
            "supplier_id": "supp_001",
            "name": "Test Inc",
            "country": "VN",
            "risk_level": 2,
            "audited": True,
        }
        n = SupplyChainNode.from_dict(d)
        assert n.supplier_id == "supp_001"
        assert n.risk_level == 2
        assert n.audited is True

    def test_to_dict_roundtrip(self):
        n = make_node()
        d = n.to_dict()
        n2 = SupplyChainNode.from_dict(d)
        assert n2.supplier_id == n.supplier_id
        assert n2.risk_level == n.risk_level


# ---------------------------------------------------------------------------
# TrainingRecord
# ---------------------------------------------------------------------------


class TestTrainingRecord:
    def test_defaults(self):
        t = TrainingRecord(training_id="t1")
        assert t.attendees == 0
        assert t.risk_assessment_covered is False

    def test_from_dict(self):
        d = {
            "training_id": "train_001",
            "attendees": 50,
            "risk_assessment_covered": True,
            "due_diligence_covered": False,
        }
        t = TrainingRecord.from_dict(d)
        assert t.attendees == 50
        assert t.risk_assessment_covered is True


# ---------------------------------------------------------------------------
# DueDiligenceRecord
# ---------------------------------------------------------------------------


class TestDueDiligenceRecord:
    def test_from_dict(self):
        d = {
            "record_id": "dd_001",
            "subject": "supplier_001",
            "risk_level": 2,
            "corrective_actions_completed_pct": 75,
        }
        dd = DueDiligenceRecord.from_dict(d)
        assert dd.record_id == "dd_001"
        assert dd.risk_level == 2
        assert dd.corrective_actions_completed_pct == 75


# ---------------------------------------------------------------------------
# MSAPolicy
# ---------------------------------------------------------------------------


class TestMSAPolicy:
    def test_from_dict(self):
        d = {
            "policy_id": "pol_001",
            "version": 2,
            "stakeholder_input_included": True,
        }
        p = MSAPolicy.from_dict(d)
        assert p.policy_id == "pol_001"
        assert p.version == 2
        assert p.stakeholder_input_included is True


# ---------------------------------------------------------------------------
# MSAReport
# ---------------------------------------------------------------------------


class TestMSAReport:
    def test_to_json_valid(self):
        r = MSAReport(
            generated_at=1_700_000_000,
            assessments_count=2,
            max_risk_level=2,
        )
        j = r.to_json()
        parsed = json.loads(j)
        assert parsed["assessments_count"] == 2
        assert parsed["max_risk_level"] == 2


# ---------------------------------------------------------------------------
# calculate_risk_score
# ---------------------------------------------------------------------------


class TestCalculateRiskScore:
    def test_empty_assessments_returns_zero(self):
        assert calculate_risk_score([]) == 0.0

    def test_single_low_risk_assessment(self):
        a = make_assessment(risk_level=0, high_risk_areas=0)
        score = calculate_risk_score([a])
        assert 0 <= score <= 3  # Low risk

    def test_single_high_risk_assessment(self):
        a = make_assessment(risk_level=3, high_risk_areas=5)
        score = calculate_risk_score([a])
        assert score > 5  # Higher score for critical risk

    def test_consultation_reduces_score(self):
        a1 = make_assessment(
            assessment_id="a1",
            risk_level=2,
            stakeholder_consultation_done=True,
        )
        a2 = make_assessment(
            assessment_id="a2",
            risk_level=2,
            stakeholder_consultation_done=True,
        )
        # Both with consultation should reduce score
        score_consulted = calculate_risk_score([a1, a2])
        a_no_consult = make_assessment(risk_level=2, stakeholder_consultation_done=False)
        score_no_consult = calculate_risk_score([a_no_consult])
        assert score_consulted < score_no_consult

    def test_invalid_weight_raises(self):
        a = make_assessment()
        with pytest.raises(ValueError):
            calculate_risk_score([a], weight_stakeholder_consultation=1.5)


# ---------------------------------------------------------------------------
# supply_chain_risk_summary
# ---------------------------------------------------------------------------


class TestSupplyChainRiskSummary:
    def test_empty_nodes(self):
        result = supply_chain_risk_summary([])
        assert result["total_suppliers"] == 0
        assert result["audited_count"] == 0

    def test_mixed_risk_levels(self):
        n1 = make_node("s1", risk_level=0)  # Low
        n2 = make_node("s2", risk_level=2)  # High
        n3 = make_node("s3", risk_level=3)  # Critical
        result = supply_chain_risk_summary([n1, n2, n3])
        assert result["total_suppliers"] == 3
        assert result["high_or_critical_count"] == 2
        assert result["by_risk_level"][0] == 1
        assert result["by_risk_level"][2] == 1
        assert result["by_risk_level"][3] == 1

    def test_audit_rate(self):
        n1 = make_node("s1", audited=True)
        n2 = make_node("s2", audited=False)
        result = supply_chain_risk_summary([n1, n2])
        assert result["audited_count"] == 1
        assert result["audit_rate_pct"] == 50.0


# ---------------------------------------------------------------------------
# training_effectiveness
# ---------------------------------------------------------------------------


class TestTrainingEffectiveness:
    def test_empty_records(self):
        result = training_effectiveness([])
        assert result["total_personnel_trained"] == 0
        assert result["total_sessions"] == 0

    def test_single_session(self):
        t = make_training(attendees=100, risk_assessment_covered=True)
        result = training_effectiveness([t])
        assert result["total_personnel_trained"] == 100
        assert result["total_sessions"] == 1
        assert result["avg_attendees_per_session"] == 100.0
        assert result["risk_assessment_covered_pct"] == 100.0

    def test_mixed_coverage(self):
        t1 = make_training(
            training_id="t1",
            attendees=50,
            risk_assessment_covered=True,
            due_diligence_covered=False,
        )
        t2 = make_training(
            training_id="t2",
            attendees=30,
            risk_assessment_covered=False,
            due_diligence_covered=True,
        )
        result = training_effectiveness([t1, t2])
        assert result["total_personnel_trained"] == 80
        assert result["risk_assessment_covered_pct"] == 50.0
        assert result["due_diligence_covered_pct"] == 50.0


# ---------------------------------------------------------------------------
# remediation_progress
# ---------------------------------------------------------------------------


class TestRemediationProgress:
    def test_empty_records(self):
        result = remediation_progress([])
        assert result["total_actions_identified"] == 0

    def test_fully_remediated(self):
        dd = make_dd_record(
            corrective_actions_required=3,
            corrective_actions_completed_pct=100,
        )
        result = remediation_progress([dd])
        assert result["total_actions_identified"] == 3
        assert result["fully_completed"] == 1
        assert result["partially_completed"] == 0
        assert result["not_started"] == 0

    def test_partial_remediation(self):
        dd1 = make_dd_record(
            record_id="dd1",
            corrective_actions_required=2,
            corrective_actions_completed_pct=50,
        )
        dd2 = make_dd_record(
            record_id="dd2",
            corrective_actions_required=3,
            corrective_actions_completed_pct=0,
        )
        result = remediation_progress([dd1, dd2])
        assert result["total_actions_identified"] == 5
        assert result["partially_completed"] == 1
        assert result["not_started"] == 1
        assert result["avg_completion_pct"] == 25.0


# ---------------------------------------------------------------------------
# ModernSlaveryAnalyzer
# ---------------------------------------------------------------------------


class TestModernSlaveryAnalyzer:
    def test_initial_state(self):
        analyzer = ModernSlaveryAnalyzer([])
        assert analyzer.total_personnel_trained() == 0
        assert analyzer.policy_count() == 0

    def test_risk_score(self):
        a = make_assessment(risk_level=1, high_risk_areas=2)
        analyzer = ModernSlaveryAnalyzer([a])
        score = analyzer.risk_score()
        assert 0 <= score <= 10

    def test_supply_chain_summary(self):
        n1 = make_node("s1", risk_level=2)
        n2 = make_node("s2", risk_level=0, audited=False)
        analyzer = ModernSlaveryAnalyzer([], [n1, n2])
        summary = analyzer.supply_chain_summary()
        assert summary["total_suppliers"] == 2
        assert summary["high_or_critical_count"] == 1
        assert summary["audited_count"] == 1

    def test_high_risk_suppliers(self):
        n1 = make_node("s1", risk_level=2)
        n2 = make_node("s2", risk_level=0)
        analyzer = ModernSlaveryAnalyzer([], [n1, n2])
        high = analyzer.high_risk_suppliers()
        assert len(high) == 1
        assert high[0].supplier_id == "s1"

    def test_unaudited_suppliers(self):
        n1 = make_node("s1", audited=True)
        n2 = make_node("s2", audited=False)
        analyzer = ModernSlaveryAnalyzer([], [n1, n2])
        unaudited = analyzer.unaudited_suppliers()
        assert len(unaudited) == 1
        assert unaudited[0].supplier_id == "s2"

    def test_training_summary(self):
        t = make_training(attendees=200)
        analyzer = ModernSlaveryAnalyzer([], [], [t])
        summary = analyzer.training_summary()
        assert summary["total_personnel_trained"] == 200

    def test_remediation_summary(self):
        dd = make_dd_record(corrective_actions_completed_pct=75)
        analyzer = ModernSlaveryAnalyzer([], [], [], [dd])
        summary = analyzer.remediation_summary()
        assert summary["avg_completion_pct"] == 75.0

    def test_policies_with_stakeholder_input(self):
        p1 = make_policy(policy_id="p1", stakeholder_input_included=True)
        p2 = make_policy(policy_id="p2", stakeholder_input_included=False)
        analyzer = ModernSlaveryAnalyzer([], [], [], [], [p1, p2])
        with_input = analyzer.policies_with_stakeholder_input()
        assert len(with_input) == 1
        assert with_input[0].policy_id == "p1"

    def test_add_assessment(self):
        analyzer = ModernSlaveryAnalyzer([])
        a = make_assessment(assessment_id="new")
        analyzer.add_assessment(a)
        summary = analyzer.risk_summary()
        assert summary["assessment_count"] == 1

    def test_compliance_report(self):
        a = make_assessment()
        n = make_node()
        t = make_training()
        dd = make_dd_record()
        p = make_policy()
        analyzer = ModernSlaveryAnalyzer([a], [n], [t], [dd], [p])
        report = analyzer.compliance_report(generated_at=1_700_000_000)
        assert report.assessments_count == 1
        assert report.supply_chain_nodes == 1
        assert report.total_trained_personnel == 100
        assert report.due_diligence_investigations == 1
        assert report.active_policies == 1


# ---------------------------------------------------------------------------
# build_compliance_report
# ---------------------------------------------------------------------------


class TestBuildComplianceReport:
    def test_empty_data(self):
        report = build_compliance_report([])
        assert report.assessments_count == 0
        assert report.max_risk_level == 0

    def test_full_aggregation(self):
        assessments = [make_assessment(risk_level=2)]
        nodes = [make_node(risk_level=2), make_node("s2", risk_level=0)]
        trainings = [make_training(attendees=150)]
        dd = [make_dd_record(corrective_actions_required=5, corrective_actions_completed_pct=100)]
        policies = [make_policy()]
        report = build_compliance_report(
            assessments, nodes, trainings, dd, policies, generated_at=1_700_000_000
        )
        assert report.assessments_count == 1
        assert report.max_risk_level == 2
        assert report.supply_chain_nodes == 2
        assert report.high_risk_suppliers == 1
        assert report.total_trained_personnel == 150
        assert report.due_diligence_investigations == 1
        assert report.total_corrective_actions == 5
        assert report.corrective_actions_completion_pct == 100
        assert report.active_policies == 1
