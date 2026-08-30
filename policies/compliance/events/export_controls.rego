package compliance.events.export_controls

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

# Rule 1: Denied Party Transaction Screening (BIS Entity List / OFAC SDN)
# Contract events recording transfers or authorizations involving flagged entities are prohibited
violations contains violation if {
    event := input.events[_]
    event.topic == "export_controls"
    event.action in ["shipment_authorized", "license_issued", "asset_transferred"]
    event.payload.screening_status == "DENIED_PARTY_MATCH"
    not event.payload.override_authorized
    violation := {
        "rule_id": "EC-001",
        "title": "Prohibited Denied Party Transaction",
        "framework": "EAR / OFAC Sanctions / BIS",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Export transaction '%v' involves a flagged entity without lawful override", [event.id])
    }
}

# Rule 2: Dual-Use Item License Verification (ITAR / EAR Commerce Control List)
# Dual-use classification requires a verified license number before export clearance
violations contains violation if {
    event := input.events[_]
    event.topic == "export_controls"
    event.action == "customs_declaration"
    event.payload.is_dual_use == true
    not event.payload.license_number
    violation := {
        "rule_id": "EC-002",
        "title": "Unlicensed Dual-Use Commodity Export",
        "framework": "ITAR 22 CFR 120-130 / EAR 15 CFR 730-774",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Export clearance '%v' marked dual-use but lacks regulatory export license number", [event.id])
    }
}

# Rule 3: Embargoed Destination Restrictions
# Comprehensive embargo jurisdictions cannot receive dual-use or defense commodities
violations contains violation if {
    event := input.events[_]
    event.topic == "export_controls"
    event.action in ["customs_declaration", "shipment_authorized"]
    destination := upper(event.payload.destination_country)
    destination in ["KP", "IR", "SY", "CU", "RU_SANCTIONED_REGION"]
    not event.payload.humanitarian_exemption
    violation := {
        "rule_id": "EC-003",
        "title": "Embargoed Jurisdiction Export Violation",
        "framework": "UN Sanctions / OFAC / EU Restrictive Measures",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Export event '%v' targets embargoed jurisdiction '%v' without valid humanitarian exemption", [event.id, destination])
    }
}

metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
