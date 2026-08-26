# SOC 2 Type II

Trust Services Criteria coverage, mapped from `control-matrix.yaml`
(`frameworks.soc2`). Run `generate-compliance-report.py --framework
soc2` for the current coverage/evidence-staleness table.

| Criteria | Controls |
|---|---|
| CC6 — Logical & physical access controls | CTRL-SEC-01 (secrets rotation), CTRL-SEC-02 (TLS), CTRL-ACC-01 (API RBAC) |
| CC7 — System operations | CTRL-SEC-03 (vulnerability management), CTRL-AUD-01 (audit trail), CTRL-IR-01 (incident response) |
| CC8 — Change management | CTRL-SEC-04 (dependency review) |
| P1 — Privacy | CTRL-PRIV-01 (DPIA) |

## What a SOC 2 auditor will ask for beyond the control matrix

- **Personnel**: background checks, security training records, access
  review sign-offs — organizational, not tracked in this repo.
- **Vendor management**: third-party risk assessments for Vault hosting,
  cloud KMS (auto-unseal), and any managed Kubernetes provider.
- **Change management tickets**: the control matrix shows *that*
  dependency changes are reviewed (CTRL-SEC-04); a SOC 2 Type II sample
  additionally wants specific PRs as evidence — `docs/compliance/auditor-access.md`
  covers giving read access to PR history for the audit period.

See [README.md](README.md) for how evidence is collected and
[auditor-access.md](auditor-access.md) for granting an auditor access.
