# ADR-014: Compliance Automation for SOC 2, ISO 27001, PCI DSS, GDPR, MiCA

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-26 |
| **Deciders** | Security team |

---

## Context

The repo already has extensive *on-chain* compliance smart-contract
modules (`src/anti_corruption.rs`, `src/export_controls.rs`,
`src/trade_compliance.rs`, `src/esg_reporting.rs`,
`src/responsible_sourcing.rs`) — regulatory business logic for
AuditLedger's *users*. There was no equivalent for compliance *of the
AuditLedger system itself*: no cross-framework control inventory, no
automated evidence collection, and no defined process for giving an
external auditor access.

---

## Decision

Build a control-matrix-driven compliance automation layer, kept
deliberately separate in naming and purpose from the on-chain modules
(see `docs/compliance/README.md`'s explicit distinction):

- **Control mapping**: `docs/compliance/control-matrix.yaml` is the
  single source of truth — one control (e.g. `CTRL-SEC-01`, secrets
  rotation) mapped to every framework clause it satisfies across SOC 2,
  ISO 27001, PCI DSS, GDPR, and MiCA. Loosely modeled on NIST OSCAL's
  control/implementation split but kept flat YAML for readability over
  full OSCAL XML/JSON.
- **Evidence collection**: `scripts/compliance/collect-evidence.sh` pulls
  current state for every `automated: true` control (rotation-state
  ledger, cert-manager status, vuln metrics) into dated JSON snapshots,
  run weekly by `.github/workflows/compliance-evidence.yml`.
- **Reporting**: `scripts/compliance/generate-compliance-report.py`
  derives a per-framework Markdown coverage report from the matrix plus
  evidence freshness — flagging evidence older than 8 days as a gap.
- **Auditor access**: `docs/compliance/auditor-access.md` — read-only
  repo access, time-boxed to the engagement, no cluster/Vault
  credentials ever issued to an auditor.
- **Continuous compliance**: evidence collection running weekly (rather
  than generated once before an audit) is the mechanism — it surfaces a
  broken collector or a control that's silently stopped being automated
  before an auditor would find it manually.

Per-framework narrative docs (`docs/compliance/soc2.md`, `iso27001.md`,
`pci-dss.md`, `gdpr.md`, `mica.md`) capture what each framework needs
beyond the technical control mapping (e.g. SOC 2 personnel/vendor
evidence, PCI DSS scoping caveats, MiCA's CASP-level obligations that
sit outside this repo entirely).

---

## Consequences

### Positive
- One control matrix serves five frameworks — adding a sixth framework
  is adding a column, not rebuilding the mapping.
- Evidence is generated, not hand-assembled before each audit, giving a
  continuous trail rather than a point-in-time snapshot.
- Explicit separation from the on-chain `compliance` contract modules
  avoids confusing "AuditLedger helps its users comply with X" (on-chain
  modules) with "AuditLedger's own infrastructure complies with X" (this
  layer).

### Negative
- The control matrix requires ongoing maintenance — a new control or a
  changed framework clause number needs a matrix edit, or the mapping
  silently goes stale.
- Several controls (privacy/DPIA, incident response, RBAC) remain
  manual by design — this automation collects evidence, it doesn't
  automate the underlying process for controls that are inherently
  human-judgment-driven.
- This layer documents *technical* controls; it explicitly does not
  cover organizational requirements (personnel checks, vendor
  assessments, legal/regulatory authorization) that a real audit also
  needs — each framework doc says so to avoid overstating coverage.

### Mitigations
- `generate-compliance-report.py`'s staleness check
  (evidence older than 8 days) is the safety net against matrix/evidence
  drift going unnoticed.
- Framework docs explicitly scope what they do and don't cover (see
  `pci-dss.md`'s scoping note, `mica.md`'s "what this mapping does not
  cover" section) rather than implying full compliance is automated.

---

## Alternatives Considered

| Option | Why not chosen |
|--------|-----------------|
| Full OSCAL (NIST) machine-readable format | More tooling/ecosystem compatibility, but significantly more verbose for a project this size with no existing OSCAL tooling; flat YAML is easier to review in a PR diff. |
| A paid GRC platform (Vanta, Drata) | No existing tooling standardization on a paid platform (per the open-source-defaults decision in [ADR-013](ADR-013-vulnerability-management-lifecycle.md)); a repo-native control matrix stays inspectable and versioned alongside the code it describes. |
| Per-framework compliance docs only, no machine-readable matrix | Would drift immediately — five separate hand-maintained documents with no single source of truth was the original problem, not a fix for it. |
