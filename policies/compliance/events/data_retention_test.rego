package compliance.events.data_retention_test

import future.keywords.if
import data.compliance.events.data_retention as dr

test_compliant_data_retention if {
    events := [
        {
            "id": "evt-dr-1",
            "topic": "data_retention",
            "action": "erasure_requested",
            "payload": {
                "pending_days": 12,
                "erasure_completed": true,
                "legal_hold": false
            }
        },
        {
            "id": "evt-dr-2",
            "topic": "data_retention",
            "action": "data_purged",
            "payload": {
                "has_legal_hold": false
            }
        }
    ]

    res := dr.compliant with input as {"events": events}
    res == true
}

test_erasure_sla_violation if {
    events := [
        {
            "id": "evt-dr-bad-1",
            "topic": "data_retention",
            "action": "erasure_requested",
            "payload": {
                "pending_days": 45,
                "erasure_completed": false,
                "legal_hold": false
            }
        }
    ]

    violations := dr.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "DR-001"
}

test_legal_hold_deletion_violation if {
    events := [
        {
            "id": "evt-dr-bad-2",
            "topic": "data_retention",
            "action": "data_purged",
            "payload": {
                "has_legal_hold": true
            }
        }
    ]

    violations := dr.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "DR-002"
}
