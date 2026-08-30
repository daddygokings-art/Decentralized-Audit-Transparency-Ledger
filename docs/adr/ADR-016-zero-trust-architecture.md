# ADR-016: Zero-Trust Architecture Implementation

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-27 |
| **Deciders** | Security Team & Architecture Board |

## Context

Perimeter-based security models are insufficient for modern decentralized ledger infrastructure. AuditLedger requires verified workload identities, continuous session evaluation, device posture checks, network microsegmentation, and capability-based least privilege.

## Decision

1. **On-Chain Module (`src/zero_trust.rs`)**:
   - Implemented `TrustTier`, `NetworkSegment`, `WorkloadIdentity`, `DevicePosture`, `ContinuousSession`, and `CapabilityGrant`.
   - Enforce segment boundary traversal validation and continuous session freshness.

2. **Off-Chain Security Package (`packages/security/src/zerotrust/`)**:
   - SPIFFE identity parser and validator.
   - Device trust posture scoring engine.
   - Continuous verification session manager with automated idle/risk revocation.
   - Capability-based authorization middleware.

3. **Kubernetes Microsegmentation (`infra/k8s/zero-trust/`)**:
   - Default-deny network policies.
   - Istio AuthorizationPolicies enforcing SPIFFE principal checking and mTLS.

## Consequences

- Eliminates implicit trust across all components.
- Enforces mutual authentication and encryption on every network hop.
