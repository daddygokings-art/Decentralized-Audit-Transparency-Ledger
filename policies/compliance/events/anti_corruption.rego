package compliance.events.anti_corruption

import future.keywords.in
import future.keywords.if

# Default compliance decision: allow if no violations
default allow := false
default compliant := false

# Evaluate compliance
compliant if {
    count(violations) == 0
}

allow if {
    compliant
}

# Rule 1: Bribery Incident threshold validation (FCPA / UK Bribery Act)
# Severity threshold: Any high-severity or critical bribe report requires an active compliance officer assignment
violations contains violation if {
    event := input.events[_]
    event.topic == "anti_corruption"
    event.action in ["incident_reported", "bribe_flagged"]
    event.payload.severity in ["HIGH", "CRITICAL"]
    not event.payload.assigned_officer
    violation := {
        "rule_id": "AC-001",
        "title": "Unassigned High-Severity Anti-Corruption Incident",
        "framework": "ISO 37001 / FCPA",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Anti-corruption incident '%v' with severity '%v' missing assigned compliance officer", [event.id, event.payload.severity])
    }
}

# Rule 2: Whistleblower report integrity and protection
# Whistleblower reports must have encrypted identity or designated anonymous hash
violations contains violation if {
    event := input.events[_]
    event.topic == "anti_corruption"
    event.action == "whistleblower_submitted"
    not event.payload.encrypted_identity
    not event.payload.is_anonymous
    violation := {
        "rule_id": "AC-002",
        "title": "Unprotected Whistleblower Identity",
        "framework": "EU Whistleblower Protection Directive 2019/1937",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Whistleblower report '%v' does not have encryption or anonymous flag enabled", [event.id])
    }
}

# Rule 3: Gift & Hospitality Registry Limits (OECD Anti-Bribery Convention)
# Gifts exceeding maximum threshold ($250 equivalent) without prior compliance pre-approval
violations contains violation if {
    event := input.events[_]
    event.topic == "anti_corruption"
    event.action == "gift_registered"
    event.payload.value_usd > 250
    not event.payload.pre_approved
    violation := {
        "rule_id": "AC-003",
        "title": "Unapproved High-Value Gift/Hospitality",
        "framework": "UK Bribery Act Section 7 / FCPA",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Gift registration '%v' exceeds $250 USD limit ($%v) without compliance pre-approval", [event.id, event.payload.value_usd])
    }
}

# Rule 4: Investigation Resolution SLA
# Open investigations cannot remain unresolved beyond 90 days without audit status update
violations contains violation if {
    event := input.events[_]
    event.topic == "anti_corruption"
    event.action == "investigation_status"
    event.payload.status == "OPEN"
    event.payload.days_open > 90
    not event.payload.extension_approved
    violation := {
        "rule_id": "AC-004",
        "title": "Anti-Corruption Investigation SLA Breach",
        "framework": "ISO 37001 Clause 9.2",
        "severity": "MEDIUM",
        "event_id": event.id,
        "message": sprintf("Investigation '%v' has been open for %v days without approved extension", [event.id, event.payload.days_open])
    }
}

# Collect all compliance metrics
metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
