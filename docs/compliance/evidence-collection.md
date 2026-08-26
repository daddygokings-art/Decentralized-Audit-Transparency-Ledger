# Evidence Collection

## What gets collected

`scripts/compliance/collect-evidence.sh` iterates every `automated:
true` control in `control-matrix.yaml` and writes a dated JSON snapshot
per control:

| Control | Evidence source |
|---|---|
| CTRL-SEC-01 (secrets rotation) | `scripts/secrets-rotation` state ledger — last recorded rotation per secret, timestamped |
| CTRL-SEC-02 (TLS) | `kubectl get certificates -A` — Ready status and expiry per Certificate |
| CTRL-SEC-03 (vulnerability mgmt) | `vuln-metrics.json` + active entries from `exceptions.yaml` |
| CTRL-SEC-04 (dependency review) | Pointer to CI run history (not embedded — avoids putting a GitHub token in evidence bundles) |
| CTRL-AUD-01 (audit trail) | Pointer to `scripts/deploy-verify.sh` for an on-demand integrity check |

Controls marked `automated: false` (CTRL-PRIV-01, CTRL-IR-01,
CTRL-ACC-01) are not collected here — their evidence *is* the referenced
document (`docs/security/privacy-by-design.md`,
`docs/security/vulnerability-reporting.md`,
`docs/adr/ADR-006-rbac-implementation.md`), reviewed and updated
manually.

## Schedule and retention

`.github/workflows/compliance-evidence.yml` runs weekly, uploading the
`evidence/` directory and generated `compliance-reports/*.md` as a
90-day-retention artifact. 90 days is chosen to comfortably span a
quarterly audit cadence without keeping an unbounded, ever-growing
history in CI storage — for a SOC 2 Type II observation period (6-12
months), the evidence trail needed is the accumulated weekly snapshots
across that period, so a production deployment should also ship
`evidence/` to durable storage (e.g. an object store bucket) rather than
relying on CI artifact retention alone. That export step is
intentionally left to the deploying organization's existing backup
tooling (see `tools/backup/`) rather than duplicated here.

## Integrity

Evidence JSON is generated, not hand-edited — `collect-evidence.sh` is
the only writer. Combined with running in CI (where the workflow run log
itself is an independent, timestamped record of *when* each snapshot was
taken), this gives an auditor a chain from "the control matrix claims
this control is automated" to "here is proof it actually ran, and here
is what it found, on this date."
