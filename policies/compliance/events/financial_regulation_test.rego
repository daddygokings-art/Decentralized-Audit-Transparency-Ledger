package compliance.events.financial_regulation_test

import future.keywords.if
import data.compliance.events.financial_regulation as fin

test_compliant_financial_events if {
    events := [
        {
            "id": "evt-fin-1",
            "topic": "stablecoin_reserves",
            "action": "reserve_attestation",
            "payload": {
                "asset_id": "USDC_VAULT",
                "reserve_ratio": 1.02
            }
        },
        {
            "id": "evt-fin-2",
            "topic": "rwa_asset",
            "action": "transfer_settled",
            "payload": {
                "amount_usd": 15000,
                "travel_rule_compliant": true,
                "ctr_reported": true
            }
        }
    ]

    res := fin.compliant with input as {"events": events}
    res == true
}

test_reserve_backing_violation if {
    events := [
        {
            "id": "evt-fin-bad-1",
            "topic": "stablecoin_reserves",
            "action": "reserve_attestation",
            "payload": {
                "asset_id": "USDT_VAULT",
                "reserve_ratio": 0.94
            }
        }
    ]

    violations := fin.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "FIN-001"
}

test_travel_rule_violation if {
    events := [
        {
            "id": "evt-fin-bad-2",
            "topic": "rwa_asset",
            "action": "transfer_settled",
            "payload": {
                "amount_usd": 2500,
                "travel_rule_compliant": false,
                "ctr_reported": false
            }
        }
    ]

    violations := fin.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "FIN-002"
}

test_ctr_reporting_violation if {
    events := [
        {
            "id": "evt-fin-bad-3",
            "topic": "rwa_asset",
            "action": "transfer_settled",
            "payload": {
                "amount_usd": 50000,
                "travel_rule_compliant": true,
                "ctr_reported": false
            }
        }
    ]

    violations := fin.violations with input as {"events": events}
    count(violations) == 1
    violations[_].rule_id == "FIN-003"
}
