# ADR-015: Contract Event Audit and Compliance Automation for SOX, GDPR, HIPAA, and MiCA

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-27 |
| **Deciders** | Architecture & Security Team |

## Context

AuditLedger requires a unified, continuous audit and compliance framework capable of ingesting contract events, continuously evaluating compliance controls, enforcing runtime policies, and generating audit-ready reports for SOX, GDPR, HIPAA, and MiCA standards.

## Decision

1. **On-Chain Module (`src/event_compliance_automation.rs`)**:
   - Implement `ComplianceFramework`, `ComplianceControl`, `EvidenceRecord`, `PolicyRule`, `ControlEvaluationResult`, and `ComplianceAuditReport`.
   - Provide standard baseline control presets for SOX, GDPR, HIPAA, and MiCA.
   - Enforce runtime policy checks for access authorization, encryption requirements, and legal holds.

2. **Automated Evidence Collection**:
   - Continuous ingestion of contract events into categorized evidence records.
   - Threshold-based control health monitoring (Passed, Warning, Deficient, InsufficientEvidence).

3. **Audit-Ready Reporting**:
   - Deterministic report generation with cryptographic verification digest seals.
   - Standalone documentation and matrix mapping in `docs/compliance/`.

## Consequences

- Continuous automated compliance replaces costly manual pre-audit preparation.
- System surfaces non-compliance and policy violations in real-time.
