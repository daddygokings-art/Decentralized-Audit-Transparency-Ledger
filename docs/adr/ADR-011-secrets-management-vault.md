# ADR-011: Adopt HashiCorp Vault for Secrets Management

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-26 |
| **Deciders** | Security team |

---

## Context

Database passwords, API keys, and signing keys were previously
long-lived plaintext values passed via `.env` (see `.env.example`) or CI
secrets, with no rotation mechanism. `docs/disaster-recovery.md` and
[ADR-006](ADR-006-rbac-implementation.md) both already recommended
adopting a secrets manager "for production" without committing to one.
This ADR closes that gap.

Options considered: HashiCorp Vault (self-hosted), a cloud provider's
managed secrets service (AWS Secrets Manager / GCP Secret Manager /
Azure Key Vault), or continuing repo-native scripted rotation of `.env`
values with no external service.

---

## Decision

Adopt **HashiCorp Vault**, self-hosted in the Kubernetes cluster
introduced by [ADR-012](ADR-012-kubernetes-cert-manager-tls.md), using:

- the `database` secrets engine for DB credential rotation (static role
  for connection-pool-friendly rotation, dynamic creds available for
  anything that can tolerate short leases),
- `kv-v2` for API keys (versioned, enabling rollback without a separate
  backup mechanism),
- `transit` for signing keys (versioned keys, so in-flight
  tokens/signatures don't break on rotation).

Rotation is driven by Kubernetes CronJobs
(`infra/k8s/vault/rotation-cronjobs.yaml`) calling scripts in
`scripts/secrets-rotation/`, each of which validates the new material
before considering rotation complete and rolls back automatically on
validation failure. See [secrets-rotation.md](../security/secrets-rotation.md)
for the full flow.

No cloud provider is assumed elsewhere in this repo (deployment today is
Docker Compose, not tied to AWS/GCP/Azure), so a cloud-native secrets
manager would have forced a cloud commitment this ADR isn't the place to
make. Vault also unifies secrets *and* internal PKI (see
[ADR-012](ADR-012-kubernetes-cert-manager-tls.md)'s `internal-ca-vault`
issuer) under one system rather than two.

---

## Consequences

### Positive
- No long-lived plaintext credentials at rest; every rotated secret has
  a bounded lifetime.
- Rotation failures are caught by automated validation before they can
  cause an outage, with automatic rollback.
- Versioned storage (`kv-v2`, `transit`) makes rollback a read of prior
  state rather than a separate backup/restore system.

### Negative
- Introduces a new stateful, highly-available service (Vault itself)
  that must be operated correctly — a misconfigured or sealed Vault
  becomes a single point of failure for every secret-dependent workload.
- Requires the new Kubernetes deployment path (ADR-012) that didn't
  previously exist for this repo's Docker-Compose-only deployment model.

### Mitigations
- Vault runs in HA mode with Raft storage and cloud-KMS auto-unseal
  (`infra/k8s/vault/values.yaml`) — no manual unseal step in the normal
  path.
- `infra/k8s/monitoring/secrets-rotation-alerts.yaml`'s `VaultSealed`
  alert pages on-call immediately if auto-unseal doesn't recover Vault
  within a minute.
- Migration off `.env`-based secrets is incremental per-secret (see
  "Migration" in [secrets-rotation.md](../security/secrets-rotation.md)),
  not a single cutover — existing services keep working while each
  secret moves over.

---

## Alternatives Considered

| Option | Why not chosen |
|--------|-----------------|
| Cloud provider secrets manager | Would force a specific cloud dependency this repo doesn't otherwise have; revisit if/when the project standardizes on one cloud. |
| Repo-native scripted `.env` rotation, no external service | No versioned rollback primitive, no dynamic/short-lived credentials, and rotation state would live in CI secrets with no audit trail. |
