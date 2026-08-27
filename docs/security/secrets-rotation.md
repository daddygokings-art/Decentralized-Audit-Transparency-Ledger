# Secrets Rotation

| | |
|---|---|
| Status | Active |
| Owner | Security team |
| Related | [../adr/ADR-011-secrets-management-vault.md](../adr/ADR-011-secrets-management-vault.md) |

All database passwords, API keys, and signing keys are managed by
HashiCorp Vault (`infra/k8s/vault/`) and rotated on a schedule by
Kubernetes CronJobs, never stored as long-lived plaintext in `.env`,
CI secrets, or application config. This does not change how the
existing off-chain services read secrets today — see "Migration" below.

## Rotation schedules

| Secret class | Vault engine | Cadence | CronJob |
|---|---|---|---|
| Database credentials | `database` (static role) | Weekly, Mon 02:00 UTC | `rotate-db-credentials` |
| API keys (webhooks, SDK publish, integrations) | `kv-v2` | Every 90 days | `rotate-api-keys` |
| Signing keys (event signing, JWT) | `transit` | Every 30 days | `rotate-signing-keys` |
| TLS certificates | cert-manager | Automatic, ~2/3 of cert lifetime | see [certificate-management.md](certificate-management.md) |

Schedules are staggered (different days/hours) so a Vault outage during
one rotation window doesn't compound with another secret class rotating
at the same time.

## How each rotation works

Scripts live in `scripts/secrets-rotation/`, shared helpers in
`common.sh`. Every `rotate-*.sh` follows the same flow:

1. Authenticate to Vault via Kubernetes auth (the CronJob's own
   ServiceAccount token — no Vault token is ever stored in the cluster
   at rest).
2. Snapshot the current material.
3. Trigger Vault's native rotation for that engine (`database/rotate-role`,
   KV `put` of a fresh value, `transit/keys/.../rotate`).
4. Confirm the material actually changed.
5. Record rotation state (`record_rotation_state`) — both a JSONL audit
   trail under `STATE_DIR` and a Prometheus metric via the
   node_exporter textfile collector, so
   `infra/k8s/monitoring/secrets-rotation-alerts.yaml` can alert on
   staleness without querying Vault directly.
6. Hand off to `validate-rotation.sh`.

## Validation

`validate-rotation.sh` proves the *new* material actually works before
a rotation is considered successful:

- **Database**: connects with the new credential (`psql ... SELECT 1`).
- **API key**: confirms the value is present and well-formed in Vault.
- **Signing key**: performs a sign/verify round-trip through `transit`.

A validation failure calls `rollback-rotation.sh` and exits non-zero —
the CronJob shows `Failed`, which `SecretRotationJobFailed` in
`infra/k8s/monitoring/secrets-rotation-alerts.yaml` pages on-call for.
Rotation failures are never silent.

## Rollback

Rollback strategy differs by secret class, because each Vault engine
has different rollback primitives:

- **Database**: Vault's `database` engine has no "restore prior
  password" operation, so rollback re-rotates to a *fresh* known-good
  credential rather than trying to recover the discarded one, and pages
  on-call — a DB rotation failing validation usually means the database
  itself is unreachable or misconfigured, which a credential rollback
  alone won't fix.
- **API key**: KV v2 keeps prior versions by default; rollback reads the
  previous version and writes it back as current, soft-deleting the bad
  version (never hard-destroyed, so it's available for incident
  forensics).
- **Signing key**: `transit` can't un-rotate a key version, so rollback
  forces `min_encryption_version` back to the previous version — new
  signing resumes using the known-good key, while the bad version stays
  valid for *verification* so nothing already signed with it breaks.

## Migration from `.env`-based secrets

`.env.example` today lists plaintext-in-CI-secret values
(`SOROBAN_SECRET_KEY`, `SLACK_WEBHOOK`, etc.). Migrating a given secret
to Vault-managed rotation is: create the Vault path/role, point the
consuming service at the Vault Agent sidecar or CSI secrets-store driver
(both enabled in `infra/k8s/vault/values.yaml`) instead of the env var,
then remove the plaintext value from wherever it was stored. This is
tracked incrementally per-secret, not as a single cutover — see the
tracking issue referenced in [ADR-011](../adr/ADR-011-secrets-management-vault.md).

## References

- `infra/k8s/vault/` — Vault Helm values, policies, CronJobs
- `scripts/secrets-rotation/` — rotation/validation/rollback scripts
- [ADR-011: Adopt Vault for secrets management](../adr/ADR-011-secrets-management-vault.md)
