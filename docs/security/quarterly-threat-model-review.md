# Quarterly Threat Model Review Process

## Cadence & Triggers

Threat models must be kept continuously fresh and are formally reviewed on a **quarterly schedule**:
- **Q1**: January 15
- **Q2**: April 15
- **Q3**: July 15
- **Q4**: October 15

### Out-of-Band Review Triggers
In addition to quarterly reviews, an immediate threat model review is triggered upon:
1. Introduction of new smart contract modules or state-breaking upgrades.
2. Integration of new blockchain bridges or settlement layers.
3. High/Critical CVE disclosure in direct dependencies.
4. Security incident or penetration test finding disclosing an unmodeled attack path.

## Review Checklist

- [ ] Verify all external entrypoints are documented in the Attack Surface Inventory.
- [ ] Evaluate STRIDE threat coverage for new/modified components.
- [ ] Validate that all High/Critical risks have verified mitigations.
- [ ] Review residual risk scores and ensure none exceed the risk tolerance threshold.
- [ ] Run automated threat model validator (`tools/threat-modeling/validate_threat_coverage.py`).
- [ ] Update review timestamp and log signatures in `docs/security/stride-pasta-threat-model.md`.

## RACI Matrix

| Role | Responsibility |
|------|----------------|
| **Security Lead** | Accountable (A) - Signs off on threat model & risk ratings |
| **Lead Architect** | Responsible (R) - Updates component DFDs and technical specifications |
| **DevOps / Infra** | Consulted (C) - Validates cloud/K8s boundaries and access rules |
| **Repo Maintainers** | Informed (I) - Receives quarterly threat report summary |
