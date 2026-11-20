# Regulator-Specific Audit Trails

## Overview

The Regulator Audit Trail system provides immutable, tamper-evident, and queryable audit logs for regulatory compliance. It implements selective disclosure for privacy-preserving audits, data sharing agreements for controlled access, and compliance validation for ISA 3000 and SOC2 standards.

## Core Features

### 1. Immutable Event Logging
- Events are stored with cryptographic hash chains
- Genesis event starts with a zero hash
- Each event references the previous event's hash
- Tampering is detected by hash mismatches

### 2. Tamper-Evidence Verification
- **Chain Verification**: Validate hash continuity from event to present
- **Immutability Proofs**: Prove an event cannot be modified (by subsequent references)
- **Archive Proofs**: Merkle proofs for archived events
- **Integrity Scoring**: Quantify chain health (0-100%)

### 3. Selective Disclosure
- Disclose only specific fields without revealing full metadata
- Merkle tree-based proofs of field inclusion
- Zero-knowledge compliance proofs (prove criteria satisfaction without data)
- Field-level access control

### 4. Data Sharing Agreements (DSA)
- Contract-based agreements between entities and regulators
- Role-based access control (Auditor, Officer, Admin)
- Compliance standard restrictions (ISA 3000, SOC2, GDPR, SOX)
- Sensitivity level filtering
- Signature validation

### 5. Compliance Standards
- **ISA 3000**: International Standard on Assurance Engagements
  - CC6.1: Segregation of duties
  - CC6.2: Exception handling
  - CC7.1: Change prevention
  - CC9.1: Monitoring & reconciliation
  - A1.1: Authorization & access control

- **SOC2**: Service Organization Control Framework
  - CC6.1-CC6.2: Logical & physical access
  - CC7.1: Change management
  - A1.1: Availability commitments
  - PI1.1: Processing integrity

### 6. Regulator Portal
- Web-based portal for accessing audit trails
- Role-based access control
- Real-time audit trail queries
- Compliance report generation
- Selective disclosure proof generation
- Tamper-evidence verification
- Data export (CSV, JSON)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│           Regulator Portal (Frontend)                    │
│  - Dashboard with audit trail queries                   │
│  - DSA management interface                             │
│  - Compliance report viewing                            │
│  - Data export functionality                            │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────┴────────────────────────────────────────┐
│           REST API Backend                               │
│  - Audit trail queries with filtering                   │
│  - Selective disclosure proof generation                │
│  - Tamper-evidence verification                         │
│  - DSA management (CRUD)                                │
│  - Compliance report generation                         │
│  - Data export                                          │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────┴────────────────────────────────────────┐
│           Smart Contract (Soroban)                       │
│  - Immutable event storage                              │
│  - Hash chain validation                                │
│  - DSA enforcement                                      │
│  - Compliance rule execution                            │
│  - Signature verification                               │
└─────────────────────────────────────────────────────────┘
```

## Smart Contract Modules

### `regulator.rs`
Core data structures:
- `ComplianceStandard`: Enum of supported standards
- `RegulatorRole`: Role hierarchy (Auditor < Officer < Admin)
- `SensitivityLevel`: Data classification (Public < Internal < Confidential < Restricted)
- `RegulatoryEventClass`: Event compliance classification
- `TamperProof`: Cryptographic proof of event integrity
- `SelectiveDisclosureProof`: Proof of field inclusion
- `DataSharingAgreement`: DSA contract
- `AccessRequest`: Request for audit data access
- `ComplianceReport`: Generated audit report

### `regulator_events.rs`
Compliance event classification:
- `ISA3000Objectives`: Control objective implementations
- `SOC2Criteria`: Criterion implementations
- `ComplianceEventType`: Standard event types for logging

### `disclosure.rs`
Selective disclosure implementation:
- `ProofBuilder`: Merkle tree construction
- `DisclosureHelper`: Proof generation and verification
- `FieldDisclosureProof`: Individual field proof
- `DisclosureConfig`: Authorization configuration

### `data_sharing.rs`
DSA framework:
- `DSABuilder`: Fluent DSA construction
- `DSAHelper`: Validation and decision logic
- `AccessDecision`: Approval/rejection decisions
- `DSAEventLog`: Event logging

### `tamper_evidence.rs`
Chain verification:
- `ChainVerification`: Validation result
- `ChainLink`: Individual link verification
- `ImmutabilityProof`: Proof of event immutability
- `TamperEvidenceHelper`: Verification logic

### `compliance_validators.rs`
Compliance rule execution:
- `ISA3000Validator`: ISA 3000 validation
- `SOC2Validator`: SOC2 validation
- `ControlValidationResult`: Control assessment
- `ComplianceAuditReport`: Compliance report

## REST API Endpoints

### Audit Trail Queries

**GET /regulator/audit-trails**
Query audit trail with filtering
```
Query Parameters:
- startTime: number (timestamp)
- endTime: number (timestamp)
- eventTypes: string[] (event type filters)
- submitter: string (address filter)
- minSensitivity: "public" | "internal" | "confidential" | "restricted"
- onlyControlEvents: boolean
- limit: number (max 1000)
- offset: number

Response:
{
  entries: [{
    eventIndex: number,
    eventHash: string,
    timestamp: number,
    eventType: string,
    submitter: string,
    sensitivity: string,
    controlEvent: boolean
  }],
  totalCount: number,
  hasMore: boolean
}
```

**GET /regulator/audit-trails/:eventIndex**
Get specific audit entry with optional disclosure proof

**POST /regulator/selective-disclosure**
Generate selective disclosure proof
```
Request:
{
  eventIndex: number,
  allowedFields: string[],
  regulatorId: string
}

Response:
{
  eventIndex: number,
  disclosedFields: string[],
  merkleProof: string[],
  disclosedRoot: string,
  completeRoot: string,
  verified: boolean
}
```

**GET /regulator/tamper-evidence/:eventIndex**
Verify tamper-evidence chain
```
Response:
{
  eventIndex: number,
  chainValid: boolean,
  previousEventHash: string,
  currentEventHash: string,
  nextEventHash: string,
  hashAlgorithm: string,
  verificationDetails: {
    genesisEvent: boolean,
    isLastEvent: boolean,
    chainIntegrityScore: number,
    intermediateEventCount: number
  }
}
```

### Data Sharing Agreements

**POST /regulator/data-sharing-agreements**
Create DSA

**GET /regulator/data-sharing-agreements**
List active DSAs

**GET /regulator/data-sharing-agreements/:dsaId**
Get DSA details

### Compliance Reports

**POST /regulator/compliance-reports**
Generate compliance report

**GET /regulator/compliance-reports**
List compliance reports

**GET /regulator/compliance-reports/:reportId**
Get report details

### Other Operations

**POST /regulator/export**
Export audit data (CSV, JSON, PDF)

**GET /regulator/access-requests**
List access requests

**POST /regulator/statistics**
Get audit statistics

## Usage Examples

### Query Audit Trail for ISA 3000 Evidence

```typescript
const response = await fetch('/regulator/audit-trails', {
  headers: {
    'Authorization': 'Bearer <token>'
  },
  query: {
    startTime: 1724000000,
    endTime: 1724086400,
    eventTypes: ['access_control', 'authorization_change'],
    onlyControlEvents: true,
    minSensitivity: 'internal'
  }
});
const { entries } = await response.json();
```

### Generate Selective Disclosure Proof

```typescript
const proof = await fetch('/regulator/selective-disclosure', {
  method: 'POST',
  headers: {
    'Authorization': 'Bearer <token>',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    eventIndex: 42,
    allowedFields: ['timestamp', 'eventType', 'submitter'],
    regulatorId: 'REG_001'
  })
});
```

### Verify Event Immutability

```typescript
const verification = await fetch('/regulator/tamper-evidence/42', {
  headers: {
    'Authorization': 'Bearer <token>'
  }
});
const { chainValid, verificationDetails } = await verification.json();
console.log(`Chain Valid: ${chainValid}`);
console.log(`Integrity Score: ${verificationDetails.chainIntegrityScore}`);
```

## Security Considerations

### Authentication & Authorization
- JWT-based authentication for API access
- Role-based access control (RBAC)
- DSA-enforced data access permissions

### Cryptographic Security
- SHA-256 for event hashing
- Merkle proofs for field disclosure
- Signature verification for DSA execution
- Content-addressed event IDs

### Data Protection
- Selective disclosure for privacy
- Sensitivity level classification
- DSA-enforced access control
- Encrypted data transmission (HTTPS)

### Audit Trail Integrity
- Hash chain validation
- Immutability verification
- Retroactive modification detection
- Archive proofs for retention

## Compliance Mapping

### ISA 3000
- **CC6 - Segregation of Duties**: Access control and role assignment events
- **CC7 - Change Management**: Change logs and approval workflows
- **CC9 - Monitoring**: Reconciliation and balance verification events
- **A1 - Authorization**: Policy documents and access control lists

### SOC2
- **Security (CC6-CC8)**: Authentication, access control, change management
- **Availability (A)**: Uptime reports, availability metrics
- **Processing Integrity (PI)**: Transaction logs, reconciliation records
- **Confidentiality (C)**: Data classification, encryption verification
- **Privacy (P)**: GDPR compliance, data handling procedures

### CQRS Projection Engine RBAC (`src/cqrs.rs`)

Every state-mutating CQRS entrypoint needs a `CqrsAuthorization`, and only
`CqrsEngine::authorize` can issue one, after it has checked `src/rbac.rs`.
Owners of an existing deployment are seeded as RBAC Admins on their first CQRS
mutation, provided RBAC has never been initialized in the contract.

| Entrypoint | Operation | Minimum role | ISA 3000 / SOC2 control | Evidence emitted |
|------------|-----------|--------------|-------------------------|------------------|
| `cqrs_log_event` | `log` | Submitter (self only) | A1 Authorization · PI Processing Integrity | `cqrs_audit/log` |
| `cqrs_update_event` | `update` | Auditor (+ owner/multisig) | CC7 Change Management | `cqrs_audit/update` |
| `cqrs_governance` | `governance` | Admin (+ owner/multisig) | CC6 Segregation of Duties · CC7 | `cqrs_audit/governance` |
| `cqrs_create_snapshot` | `snapshot` | Admin | CC9 Monitoring | `cqrs_audit/snapshot` |
| `cqrs_rebuild_projection` | `rebuild` | Admin | PI Processing Integrity · CC9 | `cqrs_audit/rebuild` |
| `cqrs_set_role` / `cqrs_suspend_submitter` / `cqrs_reinstate_submitter` | role admin | Admin | CC6 Segregation of Duties · A1 | `cqrs_audit/set_role`, `/suspend`, `/reinstate` |

Audit event data is `(caller, timestamp, outcome, state_diff_digest)`, where
`outcome` is `ok` or `denied`. A rejected attempt also emits
`cqrs_denied/<op>` with `(caller, timestamp, code)`. The digest is
`sha256(xdr(caller) ‖ xdr(op) ‖ prev_seq ‖ next_seq ‖ timestamp ‖ event_id?)`,
so an examiner can recompute it from the CQRS event store.

| Stable code | `RbacError` | Meaning |
|-------------|-------------|---------|
| 403 | `InsufficientRole` | Caller's role ranks below the operation's minimum (includes revoked/downgraded roles) |
| 423 | `SubmitterSuspended` | Submitter DID is suspended; overrides any assigned role |
| 424 | `UnauthorizedSubmitter` | Caller holds no RBAC role, or tried to log on behalf of another DID |

Positive/negative RBAC matrix, hierarchy precedence, revocation and
suspension tests: `src/rbac_gating_tests.rs` (`cqrs_rbac_matrix`).

## Testing

Comprehensive test coverage in `src/regulator_tests.rs`:
- 50+ test cases covering all modules
- Selective disclosure proof generation/verification
- Tamper-evidence chain validation
- DSA lifecycle and access control
- Compliance validator logic
- Regulator event type validation

## Deployment

### Prerequisites
- Rust toolchain with Soroban SDK 26.1+
- Node.js 20+ for API and frontend
- PostgreSQL or compatible database
- Docker & Docker Compose (recommended)

### Build

```bash
# Build smart contract
cargo build --target wasm32-unknown-unknown --release

# Build API
cd api/rest && npm install && npm run build

# Build frontend
cd ui && npm install && npm run build
```

### Deploy

```bash
# Deploy to Testnet
export SOROBAN_SECRET_KEY="<your_secret_key>"
./scripts/deploy_testnet.sh

# Start services
docker compose up -d
```

## Future Enhancements

- Zero-knowledge proof library integration
- Multi-signature DSA support
- Time-locked disclosure (time-based proofs)
- Blockchain commitment anchoring
- Advanced analytics dashboard
- Machine learning anomaly detection
- Automated compliance checking
- Audit trail versioning
- Event chaining for related events
- Webhook notifications for auditors

## References

- [ISA 3000 Standard](https://www.iaasb.org/publications)
- [SOC 2 Framework](https://www.aicpa.org/soc2)
- [GDPR Compliance](https://gdpr-info.eu/)
- [Soroban SDK Documentation](https://soroban.stellar.org/)
