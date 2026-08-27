# Regulator Audit Trail - System Architecture

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     REGULATOR PORTAL (Web UI)                        │
│  ┌──────────────────┬──────────────────┬──────────────────┐          │
│  │  Audit Trail     │  DSA Management  │  Compliance      │          │
│  │  Viewer          │  Interface       │  Reports         │          │
│  └──────────────────┴──────────────────┴──────────────────┘          │
│  ┌──────────────────┬──────────────────────────────────────┐          │
│  │  Export Tools    │  Tamper-Evidence Verification       │          │
│  └──────────────────┴──────────────────────────────────────┘          │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ HTTPS
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│              REST API BACKEND (Node.js/Express)                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Authentication Middleware (JWT)                               │  │
│  └────────────────────────────────────────────────────────────────┘  │
│  ┌─────────────────┬──────────────────┬─────────────────────────┐    │
│  │  Audit Trail    │  DSA Management  │  Compliance Validation  │    │
│  │  Endpoints      │  Endpoints       │  Endpoints              │    │
│  │ (11 endpoints)  │                  │                         │    │
│  └─────────────────┴──────────────────┴─────────────────────────┘    │
│  ┌─────────────────┬──────────────────┬─────────────────────────┐    │
│  │  Contract RPC   │  Database Layer  │  Caching Layer         │    │
│  │  Client         │  (PostgreSQL)    │  (Redis optional)      │    │
│  └─────────────────┴──────────────────┴─────────────────────────┘    │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ Stellar RPC      │ │ PostgreSQL DB    │ │ Event Cache      │
│ (Soroban)        │ │                  │ │ (Redis)          │
│                  │ │ - DSAs           │ │                  │
│ Query/Verify     │ │ - Reports        │ │ Recent events    │
│ Events           │ │ - Audit logs     │ │ Merkle proofs    │
└──────────────────┘ └──────────────────┘ └──────────────────┘
         ▲
         │
┌────────┴────────────────────────────────────────────────────────────┐
│              SMART CONTRACT (Soroban on Stellar)                     │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Regulator Audit Trail Contract                               │  │
│  │  ┌──────────────────────────────────────────────────────────┐ │  │
│  │  │  Event Storage & Management                             │ │  │
│  │  │  - Immutable event log (with hash chain)                │ │  │
│  │  │  - Per-type event indices                              │ │  │
│  │  │  - Hash chain validation                               │ │  │
│  │  └──────────────────────────────────────────────────────────┘ │  │
│  │  ┌──────────────────────────────────────────────────────────┐ │  │
│  │  │  Regulatory Classification                              │ │  │
│  │  │  - Event compliance classes                            │ │  │
│  │  │  - ISA 3000 mappings                                   │ │  │
│  │  │  - SOC2 mappings                                       │ │  │
│  │  └──────────────────────────────────────────────────────────┘ │  │
│  │  ┌──────────────────────────────────────────────────────────┐ │  │
│  │  │  Data Sharing Agreements                                │ │  │
│  │  │  - DSA storage & validation                            │ │  │
│  │  │  - Role-based access control                           │ │  │
│  │  │  - Signature verification                              │ │  │
│  │  └──────────────────────────────────────────────────────────┘ │  │
│  │  ┌──────────────────────────────────────────────────────────┐ │  │
│  │  │  Tamper-Evidence & Verification                         │ │  │
│  │  │  - Hash chain continuity checks                        │ │  │
│  │  │  - Immutability proofs                                 │ │  │
│  │  │  - Archive root verification                           │ │  │
│  │  └──────────────────────────────────────────────────────────┘ │  │
│  │  ┌──────────────────────────────────────────────────────────┐ │  │
│  │  │  Compliance Validators                                  │ │  │
│  │  │  - ISA 3000 control validation                         │ │  │
│  │  │  - SOC2 criterion validation                           │ │  │
│  │  │  - Compliance scoring                                  │ │  │
│  │  └──────────────────────────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow Diagram

### Audit Trail Query Flow
```
Regulator Portal (Frontend)
    │
    ├─ User submits query with filters
    ├─ (time range, event types, submitter, sensitivity)
    │
    ▼
REST API (/regulator/audit-trails)
    │
    ├─ Authenticate JWT token
    ├─ Validate DSA permissions
    ├─ Check filter parameters
    │
    ▼
Smart Contract Query
    │
    ├─ Fetch events from immutable log
    ├─ Filter by compliance class
    ├─ Filter by sensitivity level
    ├─ Apply DSA restrictions
    │
    ▼
Database/Cache Layer
    │
    ├─ Cache recent events
    ├─ Index by event type
    ├─ Store DSA permissions
    │
    ▼
Result Assembly
    │
    ├─ Format event details
    ├─ Include regulatory metadata
    ├─ Compute pagination
    │
    ▼
Regulator Portal (Frontend)
    │
    └─ Display audit trail with disclosure options
```

### Selective Disclosure Proof Flow
```
Regulator requests disclosure proof for Event #42
    │
    ▼
API: POST /regulator/selective-disclosure
    │
    ├─ Validate regulator authorization
    ├─ Check DSA for event type access
    ├─ Parse allowed fields
    │
    ▼
ProofBuilder (disclosure.rs)
    │
    ├─ Fetch all event fields
    ├─ Create field hashes
    ├─ Build Merkle tree
    │
    ▼
Merkle Tree Construction
    │
    ├─ Hash leaf nodes (disclosed fields only)
    ├─ Combine hashes pairwise
    ├─ Calculate intermediate roots
    │
    ▼
Proof Generation
    │
    ├─ Compute root from disclosed fields
    ├─ Include sibling hashes
    ├─ Include position markers
    │
    ▼
Verification (optional)
    │
    ├─ Verify disclosed_root using proof
    ├─ Ensure field inclusion
    ├─ Validate against complete_root
    │
    ▼
Return Proof to Regulator
    │
    └─ {
        event_index,
        disclosed_fields: ["timestamp", "event_type"],
        merkle_proof: [...],
        verified: true
      }
```

### Tamper-Evidence Verification Flow
```
Regulator checks event integrity
    │
    ▼
API: GET /regulator/tamper-evidence/:eventIndex
    │
    ├─ Validate DSA permissions
    ├─ Fetch event and neighbors
    │
    ▼
TamperEvidenceHelper (tamper_evidence.rs)
    │
    ├─ Verify event hash correctness
    │   └─ Recompute hash and compare
    │
    ├─ Check chain continuity
    │   └─ Verify prev_event.hash == current_event.prev_hash
    │
    ├─ Verify immutability
    │   └─ Count subsequent events referencing this hash
    │
    ├─ Calculate integrity score
    │   └─ (valid_events / total_events) * 100
    │
    ▼
Results
    │
    ├─ Chain Valid: true/false
    ├─ Hash Mismatch Count: N
    ├─ Integrity Score: 0-100%
    ├─ Immutable: true/false (if threshold met)
    │
    ▼
Return to Regulator
    └─ Tamper-evidence verification results
```

### Compliance Report Generation Flow
```
Regulator requests ISA 3000 report
    │
    ▼
API: POST /regulator/compliance-reports
    │
    ├─ Validate regulator role (officer/admin required)
    ├─ Parse audit parameters
    │   └─ Standard, subject, objectives to test
    │
    ▼
ISA3000Validator (compliance_validators.rs)
    │
    ├─ Load control objectives
    ├─ For each control:
    │   ├─ Query events demonstrating control
    │   ├─ Count evidence items
    │   ├─ Verify minimum evidence threshold
    │   ├─ Assess control operating effectiveness
    │
    ├─ Aggregate results
    │   ├─ Total controls tested
    │   ├─ Controls operating effectively
    │   ├─ Controls with deficiencies
    │
    ├─ Calculate compliance score
    │   └─ (operating / tested) * 100
    │
    ▼
Report Generation
    │
    ├─ Create ComplianceAuditReport
    ├─ Store in database
    ├─ Log in audit trail
    │
    ▼
Return to Regulator
    │
    └─ Report with:
       ├─ Compliance score
       ├─ Control assessment details
       ├─ Audit evidence references
       └─ Recommendations for deficiencies
```

## Module Relationships

```
┌─────────────────────────────────────────────────────────────┐
│                    regulator.rs                              │
│  Core data structures and enums                             │
│  - ComplianceStandard                                       │
│  - RegulatorRole                                            │
│  - SensitivityLevel                                         │
│  - RegulatoryEventClass                                     │
│  - TamperProof                                              │
│  - SelectiveDisclosureProof                                 │
│  - DataSharingAgreement                                     │
└────────────────────────┬────────────────────────────────────┘
                         │ Uses all types
       ┌─────────────────┼─────────────────┐
       ▼                 ▼                 ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────────┐
│regulator_    │ │disclosure.rs │ │data_sharing.rs   │
│events.rs     │ │              │ │                  │
│              │ │Merkle tree   │ │DSA validation    │
│Event class   │ │Field proofs  │ │Role-based access │
│mappings      │ │Verification  │ │Event logging     │
└──────┬───────┘ └──────┬───────┘ └────────┬─────────┘
       │                │                  │
       └────────────────┼──────────────────┘
                        │ Depends on
                        ▼
┌──────────────────────────────────────────┐
│   compliance_validators.rs               │
│   - ISA3000Validator                     │
│   - SOC2Validator                        │
│   - Control validation                   │
│   - Compliance scoring                   │
│   - Report generation                    │
└──────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│   tamper_evidence.rs                     │
│   - Chain verification                   │
│   - Integrity scoring                    │
│   - Immutability proofs                  │
│   - Archive verification                 │
└──────────────────────────────────────────┘
```

## Storage Model

### Smart Contract Storage
```
DataKey:EventRegulatoryClass(BytesN<32>) 
  → RegulatoryEventClass {
      standard: ComplianceStandard,
      control_code: Symbol,
      demonstrates_control: bool,
      retention_ledgers: u32,
      sensitivity: SensitivityLevel
    }

DataKey::EventDataSharingAgreement(BytesN<32>) 
  → DataSharingAgreement {
      id: BytesN<32>,
      data_provider: Address,
      regulator_address: Address,
      standards: Vec<ComplianceStandard>,
      allowed_event_types: Vec<Symbol>,
      role: RegulatorRole,
      min_sensitivity: SensitivityLevel,
      active: bool,
      signatures: (BytesN<64>, BytesN<64>)
    }

DataKey::AccessRequests(BytesN<32>)
  → AccessRequest {
      id: BytesN<32>,
      requester: Address,
      data_owner: Address,
      standard: ComplianceStandard,
      event_types: Vec<Symbol>,
      status: u32,
      created_at: u64,
      resolved_at: u64
    }
```

### Database Storage (PostgreSQL)
```
Tables:
  - data_sharing_agreements
    ├── id (PK)
    ├── data_provider
    ├── regulator_address
    ├── standards (JSON)
    ├── allowed_event_types (JSON)
    ├── role
    ├── status
    ├── active
    ├── created_at
    └── created_by

  - compliance_reports
    ├── id (PK)
    ├── standard
    ├── audit_subject
    ├── issuer
    ├── generated_at
    ├── status
    ├── events_examined
    ├── controls_operating
    ├── controls_deficient
    ├── compliance_score
    └── created_at

  - audit_event_classifications
    ├── event_hash (FK)
    ├── standard
    ├── control_code
    ├── demonstrates_control
    ├── retention_ledgers
    ├── sensitivity
    └── stored_at
```

## API Request/Response Cycles

### Query Audit Trail
```
Request:
GET /regulator/audit-trails?startTime=X&eventTypes=access_control,auth

Process:
1. JWT validation → RegulatorContext
2. DSA permission check
3. Contract RPC query
4. Filter & pagination
5. Format results

Response:
{
  query: { ... },
  regulatorId: "REG_001",
  standards: ["ISA3000", "SOC2"],
  entries: [
    {
      eventIndex: 0,
      eventHash: "0xabc...",
      timestamp: 1724000000,
      eventType: "access_control",
      submitter: "GA...",
      sensitivity: "confidential",
      controlEvent: true
    }
  ],
  totalCount: 100,
  hasMore: true
}
```

### Generate Disclosure Proof
```
Request:
POST /regulator/selective-disclosure
{
  eventIndex: 42,
  allowedFields: ["timestamp", "eventType", "submitter"],
  regulatorId: "REG_001"
}

Process:
1. Regulator authorization check
2. DSA validation for event type
3. ProofBuilder initialization
4. Merkle tree construction
5. Root computation
6. Proof generation

Response:
{
  eventIndex: 42,
  disclosedRoot: "0xcde...",
  completeRoot: "0xfgh...",
  disclosedFields: ["timestamp", "eventType", "submitter"],
  merkleProof: ["0x...", "0x...", ...],
  verified: true
}
```

## Compliance Audit Trail

Every regulator action is logged:
```
{
  timestamp: u64,
  action: "query_audit_trail" | "generate_proof" | "verify_chain" | ...,
  regulator_id: string,
  dsa_id: BytesN<32>,
  event_indices: [u32],
  result: "success" | "access_denied" | "invalid_proof",
  details: {
    events_accessed: u32,
    fields_disclosed: u32,
    chain_valid: bool,
    compliance_verified: bool
  }
}
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Query audit trail | O(n) | n = events in range |
| Generate disclosure proof | O(log m) | m = fields per event |
| Verify tamper-evidence | O(n) | n = events in chain |
| Validate DSA | O(1) | Direct contract storage lookup |
| Compliance scoring | O(c) | c = controls tested |
| Archive verification | O(log a) | a = archived events |

## Security Boundaries

```
┌───────────────────────────────────────┐
│ Untrusted Input (Regulator Frontend)   │
└──────────────────┬────────────────────┘
                   │
                   ▼
┌───────────────────────────────────────┐
│ Input Validation Layer (Middleware)    │
│ - Sanitize filters                    │
│ - Validate JWT                        │
│ - Check DSA permissions               │
└──────────────────┬────────────────────┘
                   │
                   ▼
┌───────────────────────────────────────┐
│ Smart Contract Boundary                │
│ - Signature verification              │
│ - Hash chain validation               │
│ - Immutability checks                 │
│ - Audit logging                       │
└──────────────────┬────────────────────┘
                   │
                   ▼
┌───────────────────────────────────────┐
│ Trusted Storage (Immutable)            │
│ - Event log (blockchain)              │
│ - Hash chains                         │
└───────────────────────────────────────┘
```

## Scalability Considerations

- Event log indexing by type and submitter for O(1) lookups
- Paginated API responses (default 100, max 1000)
- Merkle proofs reduce disclosure payload to O(log n)
- Archive pruning for long-term retention
- Caching layer for recent events
- Read replicas for query distribution

