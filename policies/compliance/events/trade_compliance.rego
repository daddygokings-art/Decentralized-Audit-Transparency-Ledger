package compliance.events.trade_compliance

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

# Rule 1: Certificate of Origin Authentication (WTO / Customs Valuation)
# International goods movement contract events must include a cryptographic proof or valid issuer for COO
violations contains violation if {
    event := input.events[_]
    event.topic == "trade_compliance"
    event.action in ["shipment_dispatched", "border_clearance"]
    not event.payload.certificate_of_origin_hash
    not event.payload.origin_verified
    violation := {
        "rule_id": "TC-001",
        "title": "Missing or Unverified Certificate of Origin",
        "framework": "WTO Rules of Origin / WCO SAFE Framework",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Trade event '%v' lacks verified Certificate of Origin proof hash", [event.id])
    }
}

# Rule 2: Tariff Classification (HS Code Format Validation)
# HS Code must adhere to standard 6-10 digit WCO nomenclature
violations contains violation if {
    event := input.events[_]
    event.topic == "trade_compliance"
    event.action in ["tariff_assessment", "customs_declaration"]
    hs_code := event.payload.hs_code
    not regex.match("^[0-9]{6}([0-9]{2}|[0-9]{4})?$", hs_code)
    violation := {
        "rule_id": "TC-002",
        "title": "Invalid Harmonized System (HS) Tariff Code",
        "framework": "WCO Harmonized System Convention",
        "severity": "MEDIUM",
        "event_id": event.id,
        "message": sprintf("Trade event '%v' specifies invalid HS code format '%v'", [event.id, hs_code])
    }
}

# Rule 3: Anti-Dumping / Countervailing Duty Compliance
# Under-declared customs valuations below fair market benchmark without justification
violations contains violation if {
    event := input.events[_]
    event.topic == "trade_compliance"
    event.action == "customs_declaration"
    event.payload.declared_value_usd < event.payload.benchmark_value_usd * 0.5
    not event.payload.valuation_variance_justified
    violation := {
        "rule_id": "TC-003",
        "title": "Suspicious Undervaluation in Customs Declaration",
        "framework": "WTO Agreement on Customs Valuation (GATT Art VII)",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Customs declaration '%v' declared value ($%v) deviates significantly from benchmark ($%v)", [event.id, event.payload.declared_value_usd, event.payload.benchmark_value_usd])
    }
}

metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
