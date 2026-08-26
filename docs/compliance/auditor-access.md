# Auditor Access

Principle: an external auditor gets **read-only** access to evidence and
reports, never write access to the repo, infrastructure, or Vault, and
never a standing credential that outlives the engagement.

## Granting access

1. **Repository**: add the auditor as a GitHub collaborator with the
   `Read` role (or, if the repo is private and org policy requires it,
   route through a dedicated `compliance-audit-readonly` team) — scoped
   to this repository only, not the org.
2. **CI artifacts**: the `compliance-evidence` and
   `vulnerability-management-report` workflow artifacts (evidence
   bundles, compliance reports, vuln metrics) are downloadable by anyone
   with repo read access — no separate grant needed once step 1 is done.
3. **Live systems**: an auditor does **not** get `kubectl` access to the
   cluster or a Vault token. If a specific control's evidence needs
   verification beyond the JSON snapshot (e.g. watching a rotation
   CronJob run live), a team member drives it on a screen-share instead
   of provisioning cluster credentials for the audit window.
4. **Time-boxing**: repository access is granted for the audit
   engagement window plus a short buffer for follow-up questions, then
   revoked — track the grant/revoke dates in the audit engagement's
   tracking issue (label `compliance-audit`), not in this doc.

## What an auditor can see vs. what they're told

Auditors get the same `control-matrix.yaml` and evidence trail the team
uses internally — there is no separate "audit view" with different
content. `generate-compliance-report.py --framework <fw>` is exactly the
tool a team member would run to prepare for the audit; running it for
the auditor is a courtesy, not a different data source.

## Sample requests during a SOC 2 Type II audit

Beyond the continuous evidence trail, a Type II sample typically asks
for specific historical instances (e.g. "show me three rotation events
from Q2 for the database credential"). Since
`scripts/secrets-rotation/common.sh`'s `record_rotation_state` appends
to a JSONL ledger rather than overwriting, the full history is queryable
directly:

```bash
grep -c "" "$STATE_DIR/db-audit-ledger-static.jsonl"   # total rotations recorded
jq -s '.[range(0;3)]' "$STATE_DIR/db-audit-ledger-static.jsonl"  # first 3
```

Ship a redacted export of the relevant ledger file(s) for the sampled
period rather than granting direct access to `STATE_DIR` on the cluster.
