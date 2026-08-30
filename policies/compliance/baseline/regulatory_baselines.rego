package compliance.baseline.regulatory_baselines

import future.keywords.in
import future.keywords.if

default allow := false
default compliant := false

# Combined baseline check across all regulatory standards
compliant if {
    count(drift_findings) == 0
}

allow if {
    compliant
}

# Drift Rule 1: Ledger Configuration Drift
# Contract event configuration cannot decrease required quorum or disable cryptographic verification
drift_findings contains finding if {
    current_config := input.current_config
    baseline_config := input.baseline_config
    current_config.min_multisig_quorum < baseline_config.min_multisig_quorum
    finding := {
        "drift_id": "DRIFT-CFG-001",
        "category": "Governance Quorum Downgrade",
        "severity": "CRITICAL",
        "framework": "SOC 2 CC6.1",
        "current_value": current_config.min_multisig_quorum,
        "baseline_value": baseline_config.min_multisig_quorum,
        "message": sprintf("Governance quorum weakened from baseline %v to %v", [baseline_config.min_multisig_quorum, current_config.min_multisig_quorum])
    }
}

# Drift Rule 2: Compliance Coverage Drift
# Percentage of audited contract events falling below required threshold (95%)
drift_findings contains finding if {
    stats := input.evaluation_stats
    stats.compliance_score_pct < 95.0
    finding := {
        "drift_id": "DRIFT-METRIC-002",
        "category": "Compliance Score Degradation",
        "severity": "HIGH",
        "framework": "ISO 27001 / SOC 2",
        "current_value": stats.compliance_score_pct,
        "baseline_value": 95.0,
        "message": sprintf("Aggregate compliance score dropped to %v%% (baseline threshold: 95.0%%)", [stats.compliance_score_pct])
    }
}

# Drift Rule 3: Unregistered Event Schema Drift
# Detection of novel event types emitted without documented policy controls
drift_findings contains finding if {
    event := input.events[_]
    known_topics := [
        "anti_corruption",
        "export_controls",
        "trade_compliance",
        "data_retention",
        "stablecoin_reserves",
        "rwa_compliance",
        "rwa_asset",
        "cbdc_logging",
        "asset_lifecycle",
        "defi_auditing",
        "tax_audit_trail",
        "governance",
        "admin",
        "core_ledger",
        "tamper_evidence",
        "bridge"
    ]
    not event.topic in known_topics
    finding := {
        "drift_id": "DRIFT-SCHEMA-003",
        "category": "Unregistered Event Schema Drift",
        "severity": "MEDIUM",
        "framework": "SOC 2 CC6.8",
        "current_value": event.topic,
        "baseline_value": "Registered Event Schema",
        "message": sprintf("Event '%v' emitted with undocumented topic '%v'", [event.id, event.topic])
    }
}
