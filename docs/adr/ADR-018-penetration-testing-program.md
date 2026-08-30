# ADR-018: Continuous Penetration Testing Program, Rules of Engagement, and SLA Enforcement

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-27 |
| **Deciders** | Security Steering Committee & Engineering Leads |

## Context

To safeguard assets and maintain regulator confidence, AuditLedger requires a formalized penetration testing program that combines continuous automated assessments with annual third-party external audits, clear rules of engagement, vetted vendor management, and strict remediation SLAs.

## Decision

1. **Establish Multi-Tier Penetration Testing Program**:
   - Scope covers Soroban smart contracts, APIs, relayers, Vault, and Kubernetes infrastructure.
   - Formalized Rules of Engagement (RoE) with 24/7 emergency escalation protocols for critical findings.
2. **Vendor Management & Rotation**:
   - Strict qualification criteria (CREST/OSCP/smart contract specialists).
   - Mandatory vendor rotation every 2 years to prevent cognitive bias.
3. **Finding Tracking & Retesting SLAs**:
   - Strict remediation deadlines: Critical (48h), High (7d), Medium (30d), Low (90d).
   - Mandatory auditor retesting sign-off before closing any finding issue.
4. **Annual Testing Schedule**:
   - Four quarterly specialized engagements covering APIs, smart contracts, cloud infrastructure, and red teaming.

## Consequences

- Systematic identification of zero-day vulnerabilities prior to adversary exploitation.
- Accountable, SLA-driven remediation with verifiable retesting trails.
