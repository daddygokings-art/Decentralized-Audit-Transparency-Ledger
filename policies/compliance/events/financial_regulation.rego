package compliance.events.financial_regulation

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

# Rule 1: MiCA Stablecoin Reserve Backing Ratio (MiCA Article 36 / 38)
# Stablecoins and asset-referenced tokens must maintain minimum 100% reserve backing ratio
violations contains violation if {
    event := input.events[_]
    event.topic in ["stablecoin_reserves", "rwa_compliance"]
    event.action in ["reserve_attestation", "rebalance_recorded", "audit_snapshot"]
    event.payload.reserve_ratio < 1.0
    violation := {
        "rule_id": "FIN-001",
        "title": "Insufficient Reserve Asset Backing",
        "framework": "EU MiCA Art. 36(1) / MAS Stablecoin Framework",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Reserve backing ratio of %v is below mandatory 100%% minimum for asset '%v'", [event.payload.reserve_ratio, event.payload.asset_id])
    }
}

# Rule 2: FATF Travel Rule Threshold Identification (FATF Recommendation 16)
# Crypto-asset transfers exceeding threshold ($1,000 USD / EUR equivalent) require originator & beneficiary KYC metadata
violations contains violation if {
    event := input.events[_]
    event.topic in ["rwa_asset", "cbdc_logging", "asset_lifecycle"]
    event.action in ["transfer_settled", "rwa_transferred", "token_minted"]
    event.payload.amount_usd >= 1000
    not event.payload.travel_rule_compliant
    violation := {
        "rule_id": "FIN-002",
        "title": "FATF Travel Rule Identification Missing",
        "framework": "FATF Rec. 16 / FinCEN Travel Rule / EU TFR",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Transfer '%v' of $%v USD missing verified Travel Rule originator/beneficiary metadata", [event.id, event.payload.amount_usd])
    }
}

# Rule 3: AML Large Transaction Reporting (FinCEN BSA Form 8300 / CTR)
# Single or aggregated transactions exceeding $10,000 USD require flagged regulatory CTR submission event
violations contains violation if {
    event := input.events[_]
    event.topic in ["rwa_asset", "defi_auditing", "tax_audit_trail"]
    event.action in ["transfer_settled", "tax_settled", "large_payment"]
    event.payload.amount_usd >= 10000
    not event.payload.ctr_reported
    violation := {
        "rule_id": "FIN-003",
        "title": "Currency Transaction Reporting (CTR) Threshold Breached Without Filing",
        "framework": "FinCEN BSA 31 CFR 1010.311 / EU AMLD6",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("Transaction '%v' amount $%v exceeds $10,000 without mandatory CTR regulatory filing", [event.id, event.payload.amount_usd])
    }
}

metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
