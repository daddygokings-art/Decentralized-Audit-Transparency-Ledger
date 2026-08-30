package auditledger.compliance

import rego.v1

frameworks := {"soc2", "iso27001", "pci-dss", "gdpr", "mica"}

# A production transfer must use a recognized mechanism and document measures.
deny contains "transfer mechanism is missing" if {
    input.kind == "cross_border_transfer"
    not input.mechanism
}

deny contains "transfer mechanism is not approved" if {
    input.kind == "cross_border_transfer"
    not input.mechanism in {"adequacy", "scc", "bcr", "certification"}
}

deny contains "high-risk transfer requires supplementary measures" if {
    input.kind == "cross_border_transfer"
    input.risk == "high"
    count(input.supplementary_measures) == 0
}

# Requests must be verified and have an auditable fulfillment lifecycle.
deny contains "rights request is not verified" if {
    input.kind == "data_subject_request"
    input.verified != true
}

deny contains "rights request has invalid status" if {
    input.kind == "data_subject_request"
    not input.status in {"received", "in_progress", "fulfilled", "rejected"}
}

# CI policy input can require every changed control to name its framework.
deny contains "control has unsupported framework" if {
    input.kind == "control"
    some framework in input.frameworks
    not framework in frameworks
}
