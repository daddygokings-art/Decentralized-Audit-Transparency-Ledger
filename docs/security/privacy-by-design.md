# Privacy by Design

## Overview

This document defines the privacy architecture for the Decentralized Audit & Transparency Ledger. It applies the seven foundational principles of Privacy by Design (PbD) to the AuditLedger contract, off-chain services, SDKs, and operational procedures.

## Privacy Principles

### 1. Data Minimization

**Policy:** Only store the data strictly necessary to fulfill the audit trail purpose.

**Implementation:**

| Data Element | Justification | Retention |
|-------------|---------------|-----------|
| `event_type` (Symbol) | Required for filtering and categorization | Indefinite (on-chain) |
| `submitter` (Address) | Required for attribution and non-repudiation | Indefinite (on-chain) |
| `timestamp` (u64) | Required for ordering and time-based queries | Indefinite (on-chain) |
| `metadata` (Bytes) | Opaque payload; schema-constrained | Configurable TTL |
| `category` (Symbol) | Optional classification | Indefinite (on-chain) |
| `sub_event_type` (Option<Symbol>) | Optional hierarchical classification | Indefinite (on-chain) |

**Guidelines for metadata authors:**
- Do not include direct identifiers (names, emails, phone numbers) unless legally required
- Use opaque references (e.g., internal IDs) instead of PII
- Keep metadata under the configured `global_metadata_max_size` (default: 1 KB)
- Consider off-chain encryption for sensitive fields before embedding in metadata

### 2. Purpose Limitation

**Policy:** Data collected for audit logging must only be used for audit and compliance purposes.

**Allowed uses:**
- Immutable audit trail verification
- Regulatory compliance reporting
- Internal governance and accountability
- Security incident investigation

**Prohibited uses:**
- Commercial marketing or profiling
- Unauthorized data sharing with third parties
- Automated decision-making beyond fraud detection

### 3. Storage Limitation

**Policy:** Event data is not retained longer than necessary.

**On-chain retention:**
- Events are immutable once written to the Soroban ledger
- TTL-based cleanup is available via `set_event_ttl(ttl_ledgers)` which extends persistent storage expiry
- Off-chain consumers should implement their own archival and purging policies

**Off-chain retention:**
- Export files should be classified and stored per data classification policy
- Automated purge jobs should run for off-chain copies beyond the retention period
- Metrics and logs should be rotated per the logging configuration

### 4. Accuracy

**Policy:** Event data must be accurate and, where necessary, kept up to date.

**Mechanisms:**
- Content-addressed event IDs (`sha256` of event fields) ensure immutability and detect tampering
- Hash chain links each event to its predecessor for tamper evidence
- Event versioning (`EventVersion` struct) preserves historical corrections without mutating original data
- Governance actions emit typed events for auditability

**Correction process:**
1. Authorized submitter identifies discrepancy
2. Owner reviews and validates the correction
3. New version is appended via `update_event()` with full version history
4. Original event remains accessible via `get_event_history()`

### 5. Integrity and Confidentiality

**Policy:** Appropriate security controls protect event data from unauthorized access, alteration, or disclosure.

**Integrity controls:**
- Append-only log structure prevents retroactive modification
- Content-addressed IDs make tampering detectable
- SHA-256 hash chain provides tamper evidence
- `require_auth()` checks on all governance functions

**Confidentiality controls:**
- All events are public by design on the Soroban network
- Sensitive metadata should be encrypted off-chain before submission
- Off-chain services use scoped API keys with minimal permissions
- Secrets are never embedded in the contract or committed to the repository

### 6. Accountability

**Policy:** Data controllers and processors are accountable for compliance with privacy principles.

**Accountability measures:**

| Measure | Implementation |
|---------|---------------|
| Audit logging | All governance actions emit typed Soroban events |
| Access control | `require_auth()` on all write operations |
| Role separation | Owner, submitter, and verifier roles are distinct |
| Incident response | See `docs/security/vulnerability-reporting.md` |
| Privacy reviews | See DPIA process below |
| Data mapping | See data flow diagram below |

## Data Flow and Processing Map

```
┌──────────────┐      ┌───────────────┐      ┌──────────────┐
│  Submitter   │─────▶│  AuditLedger  │─────▶│  Off-chain   │
│  (Client)    │ auth │  Contract     │ emit │  Services    │
└──────────────┘      └───────────────┘      └──────────────┘
       │                      │                      │
       │ 1. Authenticate      │ 2. Store event       │ 3. Index/export
       │    via Stellar       │    on-chain          │    events
       │    transaction       │                      │
       │                      │ 4. Emit Soroban      │ 5. Alert /
       │                      │    events            │    notify
       ▼                      ▼                      ▼
┌──────────────┐      ┌───────────────┐      ┌──────────────┐
│  Stellar     │      │  Prometheus   │      │  Webhook /   │
│  Consensus   │      │  Metrics      │      │  Export      │
└──────────────┘      └───────────────┘      └──────────────┘
```

**Data controllers:**
- Contract owner (governance decisions)
- Off-chain service operators (data processing)

**Data processors:**
- Bridge relayer (proof generation)
- Metrics exporter (metrics aggregation)
- REST API (data serving)
- Webhook dispatch (event notification)

## Data Protection Impact Assessment (DPIA) Process

### When a DPIA is Required

A DPIA is required when:
- New event types or metadata schemas are introduced that could increase privacy risk
- Off-chain processing changes introduce new data flows or third-party integrations
- Contract upgrades change data handling or storage mechanisms
- Regulatory requirements mandate formal privacy assessment

### DPIA Template

```markdown
# Data Protection Impact Assessment: [Feature/Change Name]

## Assessment Metadata
- **Date:** YYYY-MM-DD
- **Assessor:** [Name/Role]
- **Reviewer:** [Name/Role]
- **Version:** [Feature version or commit hash]

## Description
[Brief description of the feature or change]

## Data Inventory
| Data Item | Source | Purpose | Retention | Storage Location |
|-----------|--------|---------|-----------|-----------------|
| [field]   | [origin] | [why] | [how long] | [where] |

## Necessity and Proportionality
- [ ] Data is limited to what is necessary
- [ ] Retention period is justified
- [ ] No less intrusive alternative exists

## Privacy Risks
| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| [risk] | [L/M/H] | [L/M/H/C] | [control] |

## Compliance
- [ ] GDPR Article 5 principles addressed
- [ ] Data subject rights considered (access, rectification, erasure)
- [ ] Data breach notification procedure defined

## Approval
- [ ] Assessor sign-off
- [ ] Privacy review sign-off
- [ ] Owner/governance approval
```

### DPIA Process Steps

1. **Initiation:** Developer or product owner submits DPIA request
2. **Data Mapping:** Identify all data flows, storage locations, and processors
3. **Risk Assessment:** Evaluate privacy risks against the seven PbD principles
4. **Mitigation:** Define technical and procedural controls
5. **Review:** Privacy officer reviews and approves
6. **Implementation:** Deploy with documented controls
7. **Monitoring:** Periodic review of controls and risk posture

## Privacy Review Checklist

### Before Merging Code

- [ ] No PII in event metadata unless explicitly required and documented
- [ ] Metadata size limits are enforced
- [ ] New event types do not introduce unexpected data categories
- [ ] Off-chain services handle data per the data flow map
- [ ] Secrets are not logged or exposed in error messages
- [ ] API responses do not leak more data than necessary
- [ ] Webhook payloads do not contain sensitive unencrypted data

### Before Release

- [ ] DPIA completed for new features
- [ ] Privacy documentation updated
- [ ] Data retention policies reviewed
- [ ] Incident response plan tested
- [ ] Third-party processor agreements reviewed

## Subject Access and Data Rights

Since the AuditLedger contract is immutable and public, certain GDPR data subject rights have technical limitations:

| Right | Feasibility | Approach |
|-------|-------------|----------|
| Right to access | Partial | Events can be read via RPC; submitter can verify their own events |
| Right to rectification | Partial | New versions can be appended; original is preserved |
| Right to erasure | Not feasible on-chain | Off-chain copies should be purged per retention policy |
| Right to restrict processing | Feasible | Pause or restrict event logging for affected types |
| Right to data portability | Feasible | Export via `get_events_by_submitter()` or off-chain APIs |
| Right to object | Not applicable | Data is not used for automated decision-making |

## Incident Response

Privacy incidents (unauthorized access, data breach, misconfiguration) should follow the process in `docs/security/vulnerability-reporting.md` with additional steps:

1. Assess scope of personal data exposure
2. Notify affected data subjects within 72 hours if required
3. Notify the Data Protection Authority (DPA) if required by jurisdiction
4. Document remediation steps and timeline
5. Review and update controls to prevent recurrence

## References

- [ISO/IEC 27701:2019 - Privacy Information Management](https://www.iso.org/standard/71670.html)
- [GDPR Article 5 - Principles](https://gdpr-info.eu/art-5-gdpr/)
- [NIST Privacy Framework](https://www.nist.gov/privacy-framework)
- [OWASP Privacy by Design](https://owasp.org/www-project-privacy-by-design/)
