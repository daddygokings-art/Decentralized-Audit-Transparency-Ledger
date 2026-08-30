package auditledger.compliance

import rego.v1

test_high_risk_transfer_requires_measures if {
    count(deny) == 0 with input as {"kind": "cross_border_transfer", "mechanism": "scc", "risk": "high", "supplementary_measures": ["encryption-at-rest"]}
}

test_high_risk_transfer_without_measures_is_denied if {
    "high-risk transfer requires supplementary measures" in deny with input as {"kind": "cross_border_transfer", "mechanism": "scc", "risk": "high", "supplementary_measures": []}
}

test_unverified_request_is_denied if {
    "rights request is not verified" in deny with input as {"kind": "data_subject_request", "verified": false, "status": "received"}
}

test_verified_request_is_accepted if {
    count(deny) == 0 with input as {"kind": "data_subject_request", "verified": true, "status": "fulfilled"}
}
