package compliance.events.security_integrity_test

import future.keywords.if
import data.compliance.events.security_integrity as sec

test_compliant_security_events if {
    events := [
        {
            "id": "evt-sec-1",
            "topic": "governance",
            "action": "contract_paused",
            "payload": {
                "signatures_count": 3,
                "required_quorum": 3
            }
        },
        {
            "id": "evt-sec-2",
            "topic": "tamper_evidence",
            "action": "batch_sealed",
            "payload": {
                "state_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            }
        }
    ]

    res := sec.compliant with input as {"events": events}
    res == true
}

test_quorum_violation if {
    events := [
        {
            "id": "evt-sec-bad-1",
            "topic": "governance",
            "action": "contract_upgraded",
            "payload": {
                "signatures_count": 1,
                "required_quorum": 3
            }
        }
    ]

    violations := sec.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "SEC-001"
}

test_hash_format_violation if {
    events := [
        {
            "id": "evt-sec-bad-2",
            "topic": "tamper_evidence",
            "action": "batch_sealed",
            "payload": {
                "state_hash": "bad-hash"
            }
        }
    ]

    violations := sec.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "SEC-002"
}
