package compliance.baseline.regulatory_baselines_test

import future.keywords.if
import data.compliance.baseline.regulatory_baselines as rb

test_compliant_baseline if {
    input_data := {
        "current_config": {"min_multisig_quorum": 3},
        "baseline_config": {"min_multisig_quorum": 3},
        "evaluation_stats": {"compliance_score_pct": 98.5},
        "events": [
            {"id": "e1", "topic": "trade_compliance"}
        ]
    }

    res := rb.compliant with input as input_data
    res == true
}

test_quorum_drift if {
    input_data := {
        "current_config": {"min_multisig_quorum": 1},
        "baseline_config": {"min_multisig_quorum": 3},
        "evaluation_stats": {"compliance_score_pct": 99.0},
        "events": []
    }

    findings := rb.drift_findings with input as input_data
    count(findings) == 1
    findings[_].drift_id == "DRIFT-CFG-001"
}

test_compliance_score_drift if {
    input_data := {
        "current_config": {"min_multisig_quorum": 3},
        "baseline_config": {"min_multisig_quorum": 3},
        "evaluation_stats": {"compliance_score_pct": 89.0},
        "events": []
    }

    findings := rb.drift_findings with input as input_data
    count(findings) == 1
    findings[_].drift_id == "DRIFT-METRIC-002"
}

test_unregistered_event_drift if {
    input_data := {
        "current_config": {"min_multisig_quorum": 3},
        "baseline_config": {"min_multisig_quorum": 3},
        "evaluation_stats": {"compliance_score_pct": 98.0},
        "events": [
            {"id": "unknown-1", "topic": "rogue_unregistered_stream"}
        ]
    }

    findings := rb.drift_findings with input as input_data
    count(findings) == 1
    findings[_].drift_id == "DRIFT-SCHEMA-003"
}
