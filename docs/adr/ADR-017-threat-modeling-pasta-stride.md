# ADR-017: STRIDE and PASTA Threat Modeling with Quarterly Review Cadence

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-27 |
| **Deciders** | Core Security Team & Maintainers |

## Context

Decentralized audit ledger components interact with untrusted clients, external blockchains, and microservices. Without systematic threat modeling, vulnerabilities may arise from unanticipated attack vectors or architecture drift.

## Decision

1. **Adopt Combined STRIDE & PASTA Methodology**:
   - Apply STRIDE across all 5 architectural tiers (Smart Contracts, Relayers, APIs, Notifiers, Infrastructure).
   - Execute the 7-stage PASTA risk-centric simulation framework to ensure business and compliance alignment.
2. **Document Attack Surfaces and Security Requirements**:
   - Maintain an Attack Surface Inventory and Security Requirements Traceability Matrix (SRTM).
3. **Automate Review Cadence**:
   - Enforce a formal quarterly review cycle with automated validation tooling and GitHub Actions.

## Consequences

- Systematic identification and mitigation of threats across all system layers.
- Verified residual risk posture maintained below acceptable thresholds.
