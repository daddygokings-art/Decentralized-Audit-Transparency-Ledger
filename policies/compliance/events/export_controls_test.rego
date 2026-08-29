package compliance.events.export_controls_test

import future.keywords.if
import data.compliance.events.export_controls as ec

test_compliant_export_events if {
    events := [
        {
            "id": "evt-ec-1",
            "topic": "export_controls",
            "action": "customs_declaration",
            "payload": {
                "screening_status": "CLEAR",
                "is_dual_use": true,
                "license_number": "LIC-2026-US-89211",
                "destination_country": "DE"
            }
        },
        {
            "id": "evt-ec-2",
            "topic": "export_controls",
            "action": "shipment_authorized",
            "payload": {
                "screening_status": "CLEAR",
                "is_dual_use": false,
                "destination_country": "JP"
            }
        }
    ]

    res := ec.compliant with input as {"events": events}
    res == true
}

test_denied_party_violation if {
    events := [
        {
            "id": "evt-ec-bad-1",
            "topic": "export_controls",
            "action": "shipment_authorized",
            "payload": {
                "screening_status": "DENIED_PARTY_MATCH",
                "override_authorized": false,
                "destination_country": "SG"
            }
        }
    ]

    violations := ec.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "EC-001"
}

test_unlicensed_dual_use_violation if {
    events := [
        {
            "id": "evt-ec-bad-2",
            "topic": "export_controls",
            "action": "customs_declaration",
            "payload": {
                "screening_status": "CLEAR",
                "is_dual_use": true,
                "license_number": null,
                "destination_country": "FR"
            }
        }
    ]

    violations := ec.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "EC-002"
}

test_embargoed_destination_violation if {
    events := [
        {
            "id": "evt-ec-bad-3",
            "topic": "export_controls",
            "action": "customs_declaration",
            "payload": {
                "screening_status": "CLEAR",
                "is_dual_use": false,
                "destination_country": "KP",
                "humanitarian_exemption": false
            }
        }
    ]

    violations := ec.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "EC-003"
}
