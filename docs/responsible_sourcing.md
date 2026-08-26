# Responsible Sourcing Verification Module

## Overview

The Responsible Sourcing Verification module provides comprehensive on-chain certification, audit, and traceability systems for precious metals, minerals, and conflict-free materials. It implements standards from major certification schemes and enables blockchain-anchored consumer claims.

## Key Features

### 1. **Multi-Scheme Certification Support**

- **RJC (Responsible Jewellery Council)** — Gold, silver, diamond, and gemstone traceability
- **LBMA (London Bullion Market Association)** — Precious metals compliance standards
- **RMI (Responsible Minerals Initiative)** — Conflict-free minerals certification
- **ISO 9001** — Quality management systems
- **ISO 14001** — Environmental management systems
- **Custom schemes** — User-defined certification frameworks

### 2. **Audit Standards Coverage**

- **Third-Party Independent Audits** — Verified by recognized certification authorities
- **Chain of Custody (CoC)** — Cryptographic custody transfer tracking
- **Due Diligence Audits** — Compliance verification
- **Conflict Minerals Audits** — OECD due diligence requirements
- **Environmental Audits** — ESG/sustainability compliance
- **Social Responsibility Audits** — Labor and human rights standards

### 3. **Supply Chain Traceability**

- **Shipment Tracking** — Content-addressed shipment IDs with cryptographic hashing
- **Custody Transfers** — Signed custody handoff records with timestamp verification
- **Checkpoints** — Tamper-evident waypoints along supply chain with hash chaining
- **Material Origin Records** — Extraction location, date, and authority verification
- **Full Path Reconstruction** — Complete supply chain path from source to consumer

### 4. **Consumer Claim Verification**

- **Claim Submission** — Retailers/brands submit claims (e.g., "100% responsibly sourced")
- **Claim Backing** — Claims require supporting certifications and audit reports
- **Claim Verification** — On-chain verification against supporting documentation
- **Conflict Detection** — Automatic detection of claim contradictions

### 5. **Conflict Minerals Detection**

- **Material Alerts** — Registry of conflict-prone materials
- **Automatic Flagging** — Materials flagged during certification
- **Supply Chain Alerts** — Warnings when conflict materials detected

## Data Structures

### Certification

```rust
pub struct Certification {
    pub id: BytesN<32>,                    // Content-addressed ID
    pub scheme: u32,                       // RJC, LBMA, RMI, etc.
    pub authority: Address,                // Issuing certification body
    pub material_description: Bytes,       // "Gold", "Diamonds", etc.
    pub issued_at: u64,                    // Issuance timestamp
    pub expires_at: u64,                   // Expiration (0 = no expiry)
    pub status: u32,                       // 0=pending, 1=active, 2=suspended, 3=revoked
    pub audit_standards: Vec<u32>,         // Standards covered
    pub origin: u32,                       // ResponsiblyMined, Recycled, etc.
    pub metadata_hash: BytesN<32>,         // Content hash
}
```

**Status Values:**
- `0` — Pending (awaiting finalization)
- `1` — Active (valid and enforceable)
- `2` — Suspended (temporarily invalid, may be restored)
- `3` — Revoked (permanently invalid)

### Shipment

```rust
pub struct Shipment {
    pub id: BytesN<32>,                    // Content-addressed shipment ID
    pub certification_id: BytesN<32>,      // Supporting certification
    pub quantity: u64,                     // Amount (e.g., troy ounces)
    pub unit: Bytes,                       // Unit of measurement
    pub created_at: u64,                   // Creation timestamp
    pub last_transfer_at: u64,             // Last custody transfer timestamp
    pub current_custodian: Address,        // Current holder
    pub shipment_hash: BytesN<32>,         // Content hash
    pub custody_verified: bool,            // All custody transfers verified
}
```

### Custody Transfer

```rust
pub struct CustodyTransfer {
    pub id: u32,                           // Sequential index
    pub shipment_id: BytesN<32>,           // Shipment being transferred
    pub from: Address,                     // Previous custodian
    pub to: Address,                       // New custodian
    pub transferred_at: u64,               // Transfer timestamp
    pub transfer_proof: BytesN<32>,        // Cryptographic proof
    pub signature: Bytes,                  // Signature (96 bytes)
    pub location: Bytes,                   // Transfer location/facility
}
```

**Transfer Proof Computation:**
```
transfer_proof = sha256(shipment_id || from || to || timestamp)
```

### Traceability Checkpoint

```rust
pub struct TraceabilityCheckpoint {
    pub index: u32,                        // Sequential index
    pub shipment_id: BytesN<32>,           // Associated shipment
    pub party: Address,                    // Party at checkpoint
    pub checkpoint_at: u64,                // Timestamp
    pub location: Bytes,                   // Facility/location
    pub metadata: Bytes,                   // JSON event data
    pub checkpoint_hash: BytesN<32>,       // Content hash
    pub prev_checkpoint_hash: BytesN<32>,  // Previous checkpoint hash (chain)
}
```

**Hash Chain Verification:**
Each checkpoint links to the previous one, forming a tamper-evident chain:
```
checkpoint_hash = sha256(prev_hash || index || party || location || metadata || timestamp)
```

### Audit Report

```rust
pub struct AuditReport {
    pub id: BytesN<32>,                    // Content-addressed report ID
    pub certification_id: BytesN<32>,      // Certification audited
    pub auditor: Address,                  // Auditing authority
    pub audited_at: u64,                   // Audit timestamp
    pub standards_covered: Vec<u32>,       // Standards verified
    pub shipments_audited: u32,            // Number of shipments audited
    pub findings: Bytes,                   // JSON findings summary
    pub compliance_status: u32,            // 0=non-compliant, 1=compliant, 2=with_findings
    pub report_hash: BytesN<32>,           // Content hash
    pub finalized: bool,                   // Immutable once finalized
}
```

### Consumer Claim

```rust
pub struct ConsumerClaim {
    pub id: BytesN<32>,                    // Claim ID
    pub claimer: Address,                  // Claiming party (retailer/brand)
    pub claim: Bytes,                      // Claim text
    pub supporting_certification: BytesN<32>, // Primary certification
    pub supporting_audits: Vec<BytesN<32>>,   // Audit reports
    pub claimed_at: u64,                   // Claim timestamp
    pub verification_status: u32,          // 0=pending, 1=verified, 2=disputed
    pub claim_hash: BytesN<32>,            // Content hash
}
```

### Material Origin Record

```rust
pub struct MaterialOriginRecord {
    pub id: BytesN<32>,                    // Record ID
    pub material_type: Bytes,              // "gold", "silver", "diamond"
    pub origin_location: Bytes,            // Mine/source location
    pub extraction_date: u64,              // Extraction timestamp
    pub extraction_authority: Address,     // Mine operator/certifier
    pub conflict_free: bool,               // Verified non-conflict
    pub legally_sourced: bool,             // Legal extraction verification
    pub environmentally_compliant: bool,   // ESG compliance
    pub documentation_hash: BytesN<32>,    // Supporting docs hash
}
```

## API Reference

### Initialization

```rust
pub fn initialize(env: Env, owner: Address)
```

Initialize the module with the contract owner.

### Certifier Management

```rust
pub fn register_certifier(env: Env, caller: Address, certifier: Address)
pub fn revoke_certifier(env: Env, caller: Address, certifier: Address)
pub fn is_certifier_approved(env: Env, certifier: Address) -> bool
```

Manage authorized certification authorities. Only the contract owner can register/revoke certifiers.

### Certification Management

```rust
pub fn issue_certification(
    env: Env,
    authority: Address,
    scheme: u32,
    material_description: Bytes,
    expires_at: u64,
    audit_standards: Vec<u32>,
    origin: u32,
    metadata: Bytes,
) -> BytesN<32>

pub fn get_certification(env: Env, cert_id: BytesN<32>) -> Certification
pub fn revoke_certification(env: Env, authority: Address, cert_id: BytesN<32>)
```

**Parameters:**
- `scheme` — Certification scheme (1=RJC, 2=LBMA, 3=RMI, etc.)
- `audit_standards` — Applicable standards (1=ThirdPartyAudit, 2=CoC, 3=DueDiligence, etc.)
- `origin` — Material origin (1=ResponsiblyMined, 2=Recycled, 3=PostConsumer, etc.)

### Shipment Tracking

```rust
pub fn create_shipment(
    env: Env,
    creator: Address,
    certification_id: BytesN<32>,
    quantity: u64,
    unit: Bytes,
) -> BytesN<32>

pub fn get_shipment(env: Env, shipment_id: BytesN<32>) -> Shipment
```

### Chain of Custody

```rust
pub fn transfer_custody(
    env: Env,
    from: Address,
    to: Address,
    shipment_id: BytesN<32>,
    location: Bytes,
    signature: Bytes,  // 96 bytes: pubkey[32] || sig[64]
) -> u32

pub fn get_custody_transfer(env: Env, shipment_id: BytesN<32>, seq: u32) -> CustodyTransfer
pub fn verify_custody_chain(env: Env, shipment_id: BytesN<32>) -> bool
```

**Signature Format:**
The signature is 96 bytes:
- Bytes 0-31: Public key (Ed25519)
- Bytes 32-95: Signature (Ed25519)

The signature should sign the transfer proof: `sha256(shipment_id || from || to || timestamp)`.

### Traceability

```rust
pub fn record_checkpoint(
    env: Env,
    party: Address,
    shipment_id: BytesN<32>,
    location: Bytes,
    metadata: Bytes,  // JSON
) -> u32

pub fn get_checkpoint(env: Env, shipment_id: BytesN<32>, index: u32) -> TraceabilityCheckpoint
pub fn verify_traceability_chain(env: Env, shipment_id: BytesN<32>) -> bool
pub fn get_traceability_path(env: Env, shipment_id: BytesN<32>) -> Vec<TraceabilityCheckpoint>
```

### Material Origin

```rust
pub fn record_material_origin(
    env: Env,
    authority: Address,
    material_type: Bytes,
    origin_location: Bytes,
    extraction_date: u64,
    conflict_free: bool,
    legally_sourced: bool,
    environmentally_compliant: bool,
    documentation: Bytes,
) -> BytesN<32>

pub fn get_material_origin(env: Env, origin_id: BytesN<32>) -> MaterialOriginRecord
```

### Audit Reporting

```rust
pub fn file_audit_report(
    env: Env,
    auditor: Address,
    certification_id: BytesN<32>,
    standards_covered: Vec<u32>,
    shipments_audited: u32,
    findings: Bytes,       // JSON
    compliance_status: u32, // 0=non-compliant, 1=compliant, 2=with_findings
) -> BytesN<32>

pub fn get_audit_report(env: Env, report_id: BytesN<32>) -> AuditReport
```

### Consumer Claims

```rust
pub fn submit_consumer_claim(
    env: Env,
    claimer: Address,
    claim: Bytes,
    supporting_certification: BytesN<32>,
    supporting_audits: Vec<BytesN<32>>,
) -> BytesN<32>

pub fn get_consumer_claim(env: Env, claim_id: BytesN<32>) -> ConsumerClaim
pub fn verify_consumer_claim(env: Env, claim_id: BytesN<32>) -> bool
```

### Conflict Minerals

```rust
pub fn register_conflict_alert(env: Env, caller: Address, material: Bytes)
pub fn is_conflict_material(env: Env, material: Bytes) -> bool
```

## Usage Examples

### 1. Issuing a Responsible Sourcing Certification

```rust
// Register certifier (owner-only)
ResponsibleSourcing::register_certifier(
    env.clone(),
    owner.clone(),
    rjc_authority.clone(),
);

// Issue RJC certification for recycled gold
let cert_id = ResponsibleSourcing::issue_certification(
    env.clone(),
    rjc_authority.clone(),
    1,                    // RJC scheme
    b"recycled_gold",
    0,                    // No expiry
    vec![1, 2, 3],        // Audit standards
    2,                    // Recycled origin
    b"certification_data",
);
```

### 2. Tracking a Shipment Through Supply Chain

```rust
// Create shipment at source
let shipment_id = ResponsibleSourcing::create_shipment(
    env.clone(),
    mine_operator.clone(),
    cert_id.clone(),
    100,                  // 100 oz
    b"oz",
);

// Record checkpoint at refinery
ResponsibleSourcing::record_checkpoint(
    env.clone(),
    refinery.clone(),
    shipment_id.clone(),
    b"refinery_facility",
    b"{\"process\": \"refined\", \"yield\": \"98%\"}",
);

// Record checkpoint at distribution
ResponsibleSourcing::record_checkpoint(
    env.clone(),
    distributor.clone(),
    shipment_id.clone(),
    b"distribution_center",
    b"{\"status\": \"in_transit\"}",
);

// Verify full traceability chain
assert!(ResponsibleSourcing::verify_traceability_chain(env.clone(), shipment_id));
```

### 3. Verifying Chain of Custody

```rust
// Transfer from mine operator to refinery
ResponsibleSourcing::transfer_custody(
    env.clone(),
    mine_operator.clone(),
    refinery.clone(),
    shipment_id.clone(),
    b"refinery_location",
    signature_96_bytes,  // Signed by mine_operator
);

// Verify chain integrity
assert!(ResponsibleSourcing::verify_custody_chain(env.clone(), shipment_id));
```

### 4. Filing and Auditing

```rust
// File audit report
let report_id = ResponsibleSourcing::file_audit_report(
    env.clone(),
    auditor.clone(),
    cert_id.clone(),
    vec![1, 2, 3],        // Standards covered
    5,                    // Shipments audited
    b"{\"findings\": \"all_compliant\"}",
    1,                    // Compliant
);

// Submit consumer claim backed by audit
let claim_id = ResponsibleSourcing::submit_consumer_claim(
    env.clone(),
    retailer.clone(),
    b"100% ethically sourced and audited",
    cert_id.clone(),
    vec![report_id.clone()],
);

// Verify claim
assert!(ResponsibleSourcing::verify_consumer_claim(env.clone(), claim_id));
```

## Security Considerations

### 1. **Cryptographic Integrity**

- All entities (certifications, shipments, checkpoints, claims) are content-addressed via SHA-256
- Chain of custody transfers include signed proofs
- Hash chains ensure tamper detection at any point

### 2. **Authorization**

- Only registered certifiers can issue certifications and audit reports
- Custody transfers require authorization from the current custodian
- Claims must be backed by active, compliant certifications

### 3. **Immutability**

- Once a checkpoint is recorded, it cannot be modified
- Hash chain linkage makes modification detectable
- Audit reports are finalized and immutable

### 4. **Timestamp Validation**

- All records include timestamps for temporal ordering
- Supply chain events must occur in logical order
- Retroactive modification is cryptographically detectable

## Compliance & Standards

### Supported Standards

| Standard | Code | Description |
|----------|------|-------------|
| RJC | 1 | Responsible Jewellery Council |
| LBMA | 2 | London Bullion Market Association |
| RMI | 3 | Responsible Minerals Initiative |
| ISO 9001 | 4 | Quality Management Systems |
| ISO 14001 | 5 | Environmental Management |
| Custom | 6 | User-defined schemes |

### Audit Standards

| Standard | Code | Focus |
|----------|------|-------|
| Third-Party Audit | 1 | Independent verification |
| Chain of Custody | 2 | Supply chain integrity |
| Due Diligence | 3 | Regulatory compliance |
| Conflict Minerals | 4 | OECD requirements |
| Environmental | 5 | ESG compliance |
| Social Responsibility | 6 | Labor & human rights |

### Material Origins

| Origin | Code | Description |
|--------|------|-------------|
| ResponsiblyMined | 1 | Ethical mining operations |
| Recycled | 2 | Post-industrial recycling |
| PostConsumer | 3 | Consumer product recovery |
| ConflictFreeVerified | 4 | Verified non-conflict |
| Unknown | 5 | Unverified source |

## Events

The module emits the following events:

```rust
// Certifier events
("certifier_registered", certifier_address)
("certifier_revoked", certifier_address)

// Certification events
("certification_issued", cert_id, authority, scheme)
("certification_revoked", cert_id, authority)

// Shipment events
("shipment_created", shipment_id, cert_id, quantity)

// Custody events
("custody_transferred", shipment_id, from, to, seq)

// Traceability events
("checkpoint_recorded", shipment_id, seq, party)

// Audit events
("audit_report_filed", report_id, cert_id, auditor)

// Consumer claim events
("consumer_claim_submitted", claim_id, claimer, cert_id)

// Conflict events
("conflict_alert_registered", material)

// Material origin events
("material_origin_recorded", origin_id, authority)
```

## Integration with Main Audit Ledger

The Responsible Sourcing module integrates with the main `AuditLedger` contract through:

1. **Event Logging** — All sourcing events can be logged as audit trail entries
2. **Certification Verification** — Certificates can be anchored to audit ledger checkpoints
3. **Supply Chain Auditing** — Complete supply chain audits logged with tamper-proof timestamps
4. **Consumer Transparency** — Claims verified against the immutable audit trail

### Example Integration

```rust
// Log certification event to audit ledger
let cert_event_id = AuditLedger::log_event(
    env.clone(),
    certifier.clone(),
    Symbol::new(&env, "certification_issued"),
    cert_metadata_bytes,
);

// Log shipment tracking to audit ledger
let shipment_event_id = AuditLedger::log_event(
    env.clone(),
    mine_operator.clone(),
    Symbol::new(&env, "shipment_created"),
    shipment_metadata_bytes,
);

// Verify chain integrity across both systems
assert!(ResponsibleSourcing::verify_traceability_chain(env.clone(), shipment_id));
assert!(AuditLedger::verify_integrity(env.clone()));
```

## Cost Optimization

### Gas Efficiency

- **Indexed Queries** — O(1) lookup for certifications, shipments, and reports
- **Packed Storage** — Material origins and audit records use compressed encoding
- **Lazy Verification** — Hash chains verified on-demand, not computed on every write

### Storage Optimization

- **Content-Addressed IDs** — Single ID lookup for all entity types
- **Hash Chain Compression** — Each checkpoint stores only previous hash link (32 bytes)
- **Sparse Indexing** — Only active shipments and certifications indexed

## Future Enhancements

1. **Batch Certification** — Issue multiple certifications in single transaction
2. **Automated Compliance Checking** — Smart contracts that auto-verify standards
3. **Cross-Border Integration** — Connect with international supply chain systems
4. **ML-Based Anomaly Detection** — Detect suspicious supply chain patterns
5. **Oracle Integration** — Real-time compliance data from external sources
6. **Multi-Chain Support** — Bridge to other blockchains for interoperability
