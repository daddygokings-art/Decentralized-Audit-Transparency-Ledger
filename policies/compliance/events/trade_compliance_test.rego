package compliance.events.trade_compliance_test

import future.keywords.if
import data.compliance.events.trade_compliance as tc

test_compliant_trade_events if {
    events := [
        {
            "id": "evt-tc-1",
            "topic": "trade_compliance",
            "action": "shipment_dispatched",
            "payload": {
                "certificate_of_origin_hash": "0xcoo87216a",
                "origin_verified": true,
                "hs_code": "8471300000",
                "declared_value_usd": 10000,
                "benchmark_value_usd": 10500
            }
        }
    ]

    res := tc.compliant with input as {"events": events}
    res == true
}

test_missing_coo_violation if {
    events := [
        {
            "id": "evt-tc-bad-1",
            "topic": "trade_compliance",
            "action": "shipment_dispatched",
            "payload": {
                "certificate_of_origin_hash": null,
                "origin_verified": false
            }
        }
    ]

    violations := tc.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "TC-001"
}

test_invalid_hs_code_violation if {
    events := [
        {
            "id": "evt-tc-bad-2",
            "topic": "trade_compliance",
            "action": "tariff_assessment",
            "payload": {
                "hs_code": "INVALID_ABC"
            }
        }
    ]

    violations := tc.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "TC-002"
}

test_undervaluation_violation if {
    events := [
        {
            "id": "evt-tc-bad-3",
            "topic": "trade_compliance",
            "action": "customs_declaration",
            "payload": {
                "hs_code": "847130",
                "declared_value_usd": 2000,
                "benchmark_value_usd": 10000,
                "valuation_variance_justified": false
            }
        }
    ]

    violations := tc.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "TC-003"
}
