# Responsible Sourcing Integration & Blockchain Verification Guide

## Overview

This guide explains how to integrate the Responsible Sourcing module with the main Audit Ledger contract for complete supply chain transparency and blockchain-verified consumer claims.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              Responsible Sourcing Module                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Certifications │ Shipments │ Audits │ Consumer Claims │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────┬──────────────────────────────────────────┘
                  │ Logs all events
                  ▼
┌─────────────────────────────────────────────────────────────┐
│            Audit Ledger Contract                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Immutable Event Log │ Timestamp Chain │ Verification │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────┬──────────────────────────────────────────┘
                  │ Published to Blockchain
                  ▼
┌─────────────────────────────────────────────────────────────┐
│         Stellar Network (Immutable Record)                   │
│  Contract State | Event Stream | Cryptographic Proofs       │
└─────────────────────────────────────────────────────────────┘
```

## Integration Points

### 1. Event Logging to Audit Ledger

All sourcing events should be logged to the main audit ledger for permanent verification:

```rust
// Log certification issued
let cert_event_data = encode_certification_event(
    &cert_id,
    &authority,
    &material_description,
    cert.scheme,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    authority.clone(),
    Symbol::new(&env, "sourcing_certification_issued"),
    cert_event_data,
    None,                              // category
    Some(Symbol::new(&env, "certification")), // sub_event_type
    false,                             // force deduplication
);

// Correlate the two systems
store_audit_ledger_link(&env, cert_id, audit_event_id);
```

### 2. Shipment Creation Logging

```rust
// Log shipment creation
let shipment_event_data = encode_shipment_event(
    &shipment_id,
    &cert_id,
    quantity,
    &unit,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    creator.clone(),
    Symbol::new(&env, "sourcing_shipment_created"),
    shipment_event_data,
    Some(Symbol::new(&env, "supply_chain")),
    Some(Symbol::new(&env, "shipment_created")),
    false,
);
```

### 3. Custody Transfer Logging

```rust
// Log custody transfer
let custody_event_data = encode_custody_event(
    &shipment_id,
    &from,
    &to,
    &location,
    transfer_seq,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    from.clone(),
    Symbol::new(&env, "sourcing_custody_transfer"),
    custody_event_data,
    Some(Symbol::new(&env, "supply_chain")),
    Some(Symbol::new(&env, "custody_transfer")),
    false,
);

// Optionally link parent event for event chaining
if let Some(prev_transfer_event) = get_previous_custody_event(&env, shipment_id) {
    audit_ledger.link_events(
        env.clone(),
        audit_event_id,
        prev_transfer_event, // parent_event_id
    );
}
```

### 4. Traceability Checkpoint Logging

```rust
// Log traceability checkpoint
let checkpoint_event_data = encode_checkpoint_event(
    &shipment_id,
    checkpoint_seq,
    &party,
    &location,
    &metadata,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    party.clone(),
    Symbol::new(&env, "sourcing_checkpoint"),
    checkpoint_event_data,
    Some(Symbol::new(&env, "traceability")),
    Some(Symbol::new(&env, "checkpoint")),
    false,
);
```

### 5. Audit Report Logging

```rust
// Log audit report
let audit_event_data = encode_audit_report_event(
    &report_id,
    &cert_id,
    standards_covered,
    shipments_audited,
    &findings,
    compliance_status,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    auditor.clone(),
    Symbol::new(&env, "sourcing_audit_report"),
    audit_event_data,
    Some(Symbol::new(&env, "compliance")),
    Some(Symbol::new(&env, "audit_report")),
    false,
);
```

### 6. Consumer Claim Logging

```rust
// Log consumer claim with verification result
let claim_verification = verify_consumer_claim(&env, claim_id);
let claim_event_data = encode_claim_event(
    &claim_id,
    &claimer,
    &claim,
    &cert_id,
    claim_verification,
);

let audit_event_id = audit_ledger.log_event(
    env.clone(),
    claimer.clone(),
    Symbol::new(&env, "sourcing_consumer_claim"),
    claim_event_data,
    Some(Symbol::new(&env, "claims")),
    Some(Symbol::new(&env, "consumer_claim")),
    false,
);
```

## Event Encoding Format (JSON)

### Certification Event

```json
{
  "event_type": "certification_issued",
  "cert_id": "sha256_hash_hex",
  "scheme": "RJC",
  "authority": "stellar_address",
  "material": "gold",
  "origin": "ResponsiblyMined",
  "standards": ["ThirdPartyAudit", "ChainOfCustody"],
  "issued_at": 1724073600,
  "expires_at": 0,
  "metadata": "base64_encoded_metadata"
}
```

### Shipment Event

```json
{
  "event_type": "shipment_created",
  "shipment_id": "sha256_hash_hex",
  "cert_id": "sha256_hash_hex",
  "quantity": 100,
  "unit": "oz",
  "creator": "stellar_address",
  "created_at": 1724073600,
  "custody_verified": true
}
```

### Custody Transfer Event

```json
{
  "event_type": "custody_transfer",
  "shipment_id": "sha256_hash_hex",
  "transfer_seq": 0,
  "from": "stellar_address",
  "to": "stellar_address",
  "transferred_at": 1724074000,
  "location": "refinery_facility",
  "transfer_proof": "sha256_hash_hex",
  "signature": "base64_encoded_96_bytes"
}
```

### Traceability Checkpoint Event

```json
{
  "event_type": "checkpoint",
  "shipment_id": "sha256_hash_hex",
  "checkpoint_seq": 0,
  "party": "stellar_address",
  "checkpoint_at": 1724074400,
  "location": "distribution_center",
  "checkpoint_hash": "sha256_hash_hex",
  "prev_checkpoint_hash": "sha256_hash_hex",
  "metadata": {
    "status": "in_transit",
    "handler": "distributor_name"
  }
}
```

### Audit Report Event

```json
{
  "event_type": "audit_report",
  "report_id": "sha256_hash_hex",
  "cert_id": "sha256_hash_hex",
  "auditor": "stellar_address",
  "audited_at": 1724075000,
  "standards_covered": ["ThirdPartyAudit", "ChainOfCustody", "DueDiligence"],
  "shipments_audited": 5,
  "compliance_status": "compliant",
  "findings_summary": "all_standards_met"
}
```

### Consumer Claim Event

```json
{
  "event_type": "consumer_claim",
  "claim_id": "sha256_hash_hex",
  "claimer": "stellar_address",
  "claim": "100% ethically sourced gold",
  "cert_id": "sha256_hash_hex",
  "audit_ids": ["sha256_hash_hex_1", "sha256_hash_hex_2"],
  "claimed_at": 1724075400,
  "verification_status": "verified",
  "claim_hash": "sha256_hash_hex"
}
```

## Event Chaining for Supply Chain Relationships

Events can be linked to show causality and relationships:

```rust
/// Link a custody transfer event to its preceding checkpoint
pub fn chain_custody_to_checkpoint(
    env: Env,
    shipment_id: BytesN<32>,
    custody_event_id: BytesN<32>,
    checkpoint_event_id: BytesN<32>,
) {
    // Record the relationship
    audit_ledger.log_event_with_parent(
        env.clone(),
        custody_event_id,
        Some(checkpoint_event_id), // parent_event_id
    );
}

/// Link a consumer claim to supporting audit reports
pub fn chain_claim_to_audits(
    env: Env,
    claim_event_id: BytesN<32>,
    audit_event_ids: Vec<BytesN<32>>,
) {
    for audit_id in audit_event_ids {
        audit_ledger.log_event_with_parent(
            env.clone(),
            audit_event_id,
            Some(claim_event_id), // Audit supports claim
        );
    }
}
```

## Verification Workflows

### 1. Full Supply Chain Verification

```rust
pub fn verify_supply_chain(
    env: Env,
    shipment_id: BytesN<32>,
) -> SupplyChainVerification {
    // 1. Verify chain of custody
    let custody_valid = ResponsibleSourcing::verify_custody_chain(
        env.clone(),
        shipment_id.clone(),
    );

    // 2. Verify traceability checkpoints
    let traceability_valid = ResponsibleSourcing::verify_traceability_chain(
        env.clone(),
        shipment_id.clone(),
    );

    // 3. Verify audit ledger entries
    let path = ResponsibleSourcing::get_traceability_path(
        env.clone(),
        shipment_id.clone(),
    );
    
    let mut audit_entries_valid = true;
    for checkpoint in path {
        let audit_event_id = get_audit_event_for_checkpoint(
            &env,
            shipment_id.clone(),
            checkpoint.index,
        );
        
        if let Some(event_id) = audit_event_id {
            let event = audit_ledger.get_event(env.clone(), event_id);
            // Verify event hash chain is valid
            audit_entries_valid = audit_entries_valid && event.event_hash != BytesN::from_array(&env, &[0u8; 32]);
        }
    }

    // 4. Get full audit trail
    let audit_trail = get_shipment_audit_trail(&env, shipment_id.clone());

    SupplyChainVerification {
        shipment_id,
        custody_valid,
        traceability_valid,
        audit_entries_valid,
        audit_trail,
    }
}
```

### 2. Consumer Claim Verification with Blockchain Proof

```rust
pub fn verify_claim_with_blockchain_proof(
    env: Env,
    claim_id: BytesN<32>,
) -> ClaimVerificationProof {
    // 1. Get the claim
    let claim = ResponsibleSourcing::get_consumer_claim(env.clone(), claim_id.clone());

    // 2. Verify claim data
    let claim_verified = ResponsibleSourcing::verify_consumer_claim(
        env.clone(),
        claim_id.clone(),
    );

    // 3. Get audit ledger entry for this claim
    let claim_audit_event_id = get_audit_event_for_claim(&env, claim_id.clone());

    // 4. Get blockchain proof from audit ledger
    let audit_event = audit_ledger.get_event(env.clone(), claim_audit_event_id.clone());

    // 5. Construct full verification proof
    ClaimVerificationProof {
        claim_id,
        claim_verified,
        supporting_cert: claim.supporting_certification,
        supporting_audits: claim.supporting_audits,
        audit_ledger_event_id: claim_audit_event_id,
        blockchain_timestamp: audit_event.timestamp,
        blockchain_hash: audit_event.event_hash,
        full_proof: generate_merkle_proof(&env, audit_event),
    }
}
```

### 3. Generate Consumer Certificate/QR Code

```rust
pub struct ConsumerCertificate {
    pub product_id: Bytes,
    pub claim: Bytes,
    pub verification_url: Bytes,
    pub blockchain_proof: Bytes,
    pub qr_code_data: Bytes,
}

pub fn generate_consumer_certificate(
    env: Env,
    product_id: Bytes,
    claim_id: BytesN<32>,
) -> ConsumerCertificate {
    let verification = verify_claim_with_blockchain_proof(env.clone(), claim_id.clone());
    
    let blockchain_proof = encode_blockchain_proof(&verification);
    
    let verification_url = format!(
        "https://verify.responsiblesourcing.xyz/claims/{}",
        hex::encode(claim_id.to_vec())
    );

    let qr_code_data = generate_qr_code(
        format!("{}?proof={}", verification_url, blockchain_proof).as_bytes(),
    );

    ConsumerCertificate {
        product_id,
        claim: get_consumer_claim(env.clone(), claim_id).claim,
        verification_url: Bytes::from_slice(&env, verification_url.as_bytes()),
        blockchain_proof: Bytes::from_slice(&env, &blockchain_proof),
        qr_code_data,
    }
}
```

## Off-Chain Verification

Off-chain consumers can verify claims using:

### 1. Contract Query Endpoint

```typescript
async function verifyClaim(claimId: string): Promise<ClaimVerification> {
    // Query Soroban contract
    const claim = await contract.invoke({
        method: 'get_consumer_claim',
        args: [claimId],
    });

    const verified = await contract.invoke({
        method: 'verify_consumer_claim',
        args: [claimId],
    });

    // Query audit ledger for blockchain proof
    const auditEvent = await auditLedgerContract.invoke({
        method: 'get_event_by_order',
        args: [eventIndex],
    });

    return {
        claim,
        verified,
        blockchainTimestamp: auditEvent.timestamp,
        blockchainHash: auditEvent.event_hash,
        contractAddress: CONTRACT_ID,
        network: 'testnet', // or 'public'
    };
}
```

### 2. Direct Blockchain Verification

```typescript
async function verifyBlockchainProof(proof: BlockchainProof): Promise<boolean> {
    // 1. Verify event hash chain
    let expectedPrevHash = proof.prevEventHash;
    for (const event of proof.eventChain) {
        const computedHash = sha256(
            event.id +
            expectedPrevHash +
            event.index +
            event.timestamp
        );
        
        if (computedHash !== event.event_hash) {
            return false;
        }
        
        expectedPrevHash = event.event_hash;
    }

    // 2. Verify claim supports
    const certification = await contract.invoke({
        method: 'get_certification',
        args: [proof.certificationId],
    });

    if (certification.status !== 1) {
        return false; // Not active
    }

    // 3. Verify all audit reports
    for (const auditId of proof.auditIds) {
        const report = await contract.invoke({
            method: 'get_audit_report',
            args: [auditId],
        });

        if (!report.finalized || report.compliance_status === 0) {
            return false;
        }
    }

    return true;
}
```

## Integration Checklist

- [ ] Deploy Responsible Sourcing contract
- [ ] Deploy/Reference Audit Ledger contract
- [ ] Initialize Responsible Sourcing with owner
- [ ] Register certification authorities
- [ ] Implement event encoding functions
- [ ] Set up event logging to Audit Ledger
- [ ] Create event chaining logic
- [ ] Implement supply chain verification
- [ ] Build consumer claim verification
- [ ] Create QR code generation service
- [ ] Build off-chain verification API
- [ ] Test end-to-end workflow
- [ ] Deploy monitoring and alerting
- [ ] Create consumer-facing verification portal

## Performance Considerations

### Query Optimization

```rust
// Batch query for multiple shipments
pub fn batch_verify_shipments(
    env: Env,
    shipment_ids: Vec<BytesN<32>>,
) -> Vec<ShipmentVerification> {
    shipment_ids
        .iter()
        .map(|id| verify_supply_chain(env.clone(), id.clone()))
        .collect()
}
```

### Pagination for Large Supply Chains

```rust
pub fn get_traceability_path_paginated(
    env: Env,
    shipment_id: BytesN<32>,
    start_index: u32,
    limit: u32,
) -> Vec<TraceabilityCheckpoint> {
    let count = env.storage()
        .instance()
        .get(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()))
        .unwrap_or(0);

    let end = (start_index + limit).min(count);
    let mut checkpoints = Vec::new(&env);
    
    for i in start_index..end {
        if let Some(checkpoint) = env.storage().instance()
            .get::<_, TraceabilityCheckpoint>(
                &ResponsibleSourcingKey::TraceabilityCheckpoint(shipment_id.clone(), i)
            ) {
            checkpoints.push_back(checkpoint);
        }
    }
    
    checkpoints
}
```

## Monitoring & Alerts

### Key Metrics

- Certifications issued per day
- Shipments tracked
- Custody transfers per shipment (avg)
- Chain of custody break rate
- Audit report turnaround time
- Consumer claims verification rate
- Conflict materials detected

### Alert Triggers

```rust
pub fn check_alerts(env: Env, shipment_id: BytesN<32>) {
    let shipment = ResponsibleSourcing::get_shipment(env.clone(), shipment_id.clone());

    // Alert 1: Custody chain broken
    if !ResponsibleSourcing::verify_custody_chain(env.clone(), shipment_id.clone()) {
        emit_alert("CUSTODY_CHAIN_BROKEN", shipment_id);
    }

    // Alert 2: Traceability chain broken
    if !ResponsibleSourcing::verify_traceability_chain(env.clone(), shipment_id.clone()) {
        emit_alert("TRACEABILITY_CHAIN_BROKEN", shipment_id);
    }

    // Alert 3: Certification expired
    let cert = ResponsibleSourcing::get_certification(env.clone(), shipment.certification_id);
    if cert.expires_at > 0 && cert.expires_at < env.ledger().timestamp() {
        emit_alert("CERTIFICATION_EXPIRED", shipment_id);
    }

    // Alert 4: Conflict materials detected
    if ResponsibleSourcing::is_conflict_material(env.clone(), cert.material_description.clone()) {
        emit_alert("CONFLICT_MATERIAL_DETECTED", shipment_id);
    }
}
```

## Conclusion

The Responsible Sourcing module provides a comprehensive, blockchain-verified system for supply chain transparency. By integrating with the Audit Ledger, all sourcing data is immutably recorded and cryptographically verifiable, enabling consumer confidence in responsible sourcing claims.
