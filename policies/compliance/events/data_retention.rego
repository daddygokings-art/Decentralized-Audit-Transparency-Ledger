package compliance.events.data_retention

import future.keywords.in
import future.keywords.if

default allow := false
default compliant := false

compliant if {
    count(violations) == 0
}

allow if {
    compliant
}

# Rule 1: Right to Be Forgotten / Erasure SLA (GDPR Article 17)
# Erasure requests must be fulfilled or cryptographically anonymized within 30 days
violations contains violation if {
    event := input.events[_]
    event.topic == "data_retention"
    event.action == "erasure_requested"
    event.payload.pending_days > 30
    not event.payload.erasure_completed
    not event.payload.legal_hold
    violation := {
        "rule_id": "DR-001",
        "title": "GDPR Article 17 Erasure SLA Breach",
        "framework": "GDPR Art. 17 / CCPA",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Data erasure request '%v' pending for %v days without completion or active legal hold", [event.id, event.payload.pending_days])
    }
}

# Rule 2: Unauthorized Deletion during Legal Hold (SOC 2 / ISO 27001)
# Data subject to an active legal hold cannot be purged or expunged
violations contains violation if {
    event := input.events[_]
    event.topic == "data_retention"
    event.action in ["data_purged", "record_deleted"]
    event.payload.has_legal_hold == true
    violation := {
        "rule_id": "DR-002",
        "title": "Unlawful Deletion Under Active Legal Hold",
        "framework": "SOC 2 CC6.5 / ISO 27001 A.18.1.3",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Ledger data '%v' purged while under active legal hold", [event.id])
    }
}

# Rule 3: Data Retention Horizon Expiry
# Retained personal data exceeding maximal legal retention limit without renewal justification
violations contains violation if {
    event := input.events[_]
    event.topic == "data_retention"
    event.action == "retention_policy_check"
    event.payload.retention_years > event.payload.max_allowed_years
    not event.payload.extension_justification
    violation := {
        "rule_id": "DR-003",
        "title": "Retention Limit Exceeded Without Justification",
        "framework": "GDPR Art. 5(1)(e) (Storage Limitation)",
        "severity": "MEDIUM",
        "event_id": event.id,
        "message": sprintf("Record retention of %v years exceeds maximum %v years for dataset '%v'", [event.payload.retention_years, event.payload.max_allowed_years, event.id])
    }
}

metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
