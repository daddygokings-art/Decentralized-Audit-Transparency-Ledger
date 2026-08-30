package compliance.events.security_integrity

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

# Rule 1: Multi-Signature Governance Authorization (SOC 2 CC6.1 / CC6.3)
# Critical contract administrative actions (owner addition, contract pause, emergency upgrade) require multi-sig quorum
violations contains violation if {
    event := input.events[_]
    event.topic in ["governance", "admin", "core_ledger"]
    event.action in ["owner_added", "owner_removed", "contract_paused", "contract_upgraded"]
    event.payload.signatures_count < event.payload.required_quorum
    violation := {
        "rule_id": "SEC-001",
        "title": "Administrative Action Below Multi-Sig Quorum",
        "framework": "SOC 2 CC6.1 / CC6.3 / ISO 27001 A.9.4",
        "severity": "CRITICAL",
        "event_id": event.id,
        "message": sprintf("Admin action '%v' executed with %v signatures, required quorum is %v", [event.action, event.payload.signatures_count, event.payload.required_quorum])
    }
}

# Rule 2: Tamper Evidence & Cryptographic Hash Linkage
# Events recording archive, Merkle tree batching, or state checkpoints must contain valid SHA-256 / Blake3 hash
violations contains violation if {
    event := input.events[_]
    event.topic in ["tamper_evidence", "core_ledger", "bridge"]
    event.action in ["events_archived", "batch_sealed", "checkpoint_created"]
    hash_val := event.payload.state_hash
    not regex.match("^(0x)?[a-fA-F0-9]{64}$", hash_val)
    violation := {
        "rule_id": "SEC-002",
        "title": "Invalid Cryptographic State Hash",
        "framework": "ISO 27001 A.10.1 (Cryptographic Controls) / SOC 2 CC6.6",
        "severity": "HIGH",
        "event_id": event.id,
        "message": sprintf("State checkpoint event '%v' has invalid cryptographic hash '%v'", [event.id, hash_val])
    }
}

metrics := {
    "total_events_evaluated": count(input.events),
    "total_violations": count(violations),
    "compliant": compliant
}
