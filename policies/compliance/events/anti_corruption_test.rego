package compliance.events.anti_corruption_test

import future.keywords.if
import data.compliance.events.anti_corruption as ac

test_compliant_events if {
    events := [
        {
            "id": "evt-001",
            "topic": "anti_corruption",
            "action": "incident_reported",
            "payload": {
                "severity": "CRITICAL",
                "assigned_officer": "OFFICER_ADDR_123"
            }
        },
        {
            "id": "evt-002",
            "topic": "anti_corruption",
            "action": "whistleblower_submitted",
            "payload": {
                "encrypted_identity": "0xenc12345",
                "is_anonymous": true
            }
        },
        {
            "id": "evt-003",
            "topic": "anti_corruption",
            "action": "gift_registered",
            "payload": {
                "value_usd": 150,
                "pre_approved": false
            }
        }
    ]

    res := ac.compliant with input as {"events": events}
    res == true
}

test_unassigned_incident_violation if {
    events := [
        {
            "id": "evt-bad-001",
            "topic": "anti_corruption",
            "action": "incident_reported",
            "payload": {
                "severity": "HIGH",
                "assigned_officer": null
            }
        }
    ]

    violations := ac.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "AC-001"
}

test_unprotected_whistleblower_violation if {
    events := [
        {
            "id": "evt-bad-002",
            "topic": "anti_corruption",
            "action": "whistleblower_submitted",
            "payload": {
                "encrypted_identity": null,
                "is_anonymous": false
            }
        }
    ]

    violations := ac.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "AC-002"
}

test_unapproved_gift_violation if {
    events := [
        {
            "id": "evt-bad-003",
            "topic": "anti_corruption",
            "action": "gift_registered",
            "payload": {
                "value_usd": 500,
                "pre_approved": false
            }
        }
    ]

    violations := ac.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "AC-003"
}
