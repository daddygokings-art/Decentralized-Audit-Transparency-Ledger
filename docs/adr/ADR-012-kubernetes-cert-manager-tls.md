# ADR-012: Adopt Kubernetes + cert-manager for TLS Certificate Management

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-08-26 |
| **Deciders** | Security team |

---

## Context

This repository deploys today via Docker Compose only
(`docker-compose.yml`, `docker-compose.prod.yml`) — there is no existing
Kubernetes footprint. `cert-manager`, the de facto standard for
automated TLS issuance/renewal, is a Kubernetes controller and has no
meaningful Docker-Compose-native equivalent with the same feature set
(pluggable issuers, ACME, private CA integration, Prometheus metrics).

Achieving automated certificate issuance, renewal, and monitoring as
requested required either (a) introducing a Kubernetes deployment path
specifically to host cert-manager, (b) staying Compose-only and using a
Compose-native ACME solution (Traefik/Caddy built-in ACME) instead of
cert-manager literally, or (c) documentation/scaffolding only, with no
live cluster.

---

## Decision

Introduce a new Kubernetes deployment path (`infra/k8s/`) **specifically
to host cert-manager** (and, per [ADR-011](ADR-011-secrets-management-vault.md),
Vault) — additive to, not a replacement for, the existing Docker Compose
deployment. `infra/k8s/cert-manager/` defines:

- `letsencrypt-staging` / `letsencrypt-prod` ClusterIssuers (ACME, HTTP-01
  for public ingress hosts, DNS-01 via Route53 for internal-only names),
- a custom `internal-ca-vault` ClusterIssuer backed by Vault's PKI
  secrets engine for internal mTLS, tying certificate issuance to the
  same Vault instance used for secrets rotation,
- `Certificate` resources per service with short `renewBefore` windows
  and `rotationPolicy: Always` on private keys,
- Prometheus alerting on expiry/renewal-stall
  (`infra/k8s/monitoring/cert-expiry-alerts.yaml`).

See [certificate-management.md](../security/certificate-management.md)
for the operational detail and `infra/k8s/cert-manager/install.md` for
the Vault/cert-manager circular-bootstrap sequencing.

---

## Consequences

### Positive
- Fully automated issuance and renewal — no manual certificate handling
  for either public (Let's Encrypt) or internal (Vault PKI) TLS.
- A single custom-issuer pattern (Vault PKI via cert-manager's Vault
  issuer type) covers "custom issuers" while reusing infrastructure
  already justified by ADR-011, rather than standing up a second,
  unrelated internal CA system.
- Expiry monitoring with a 14-day alert threshold against a 30-day
  renewal trigger gives ~2 weeks of buffer to fix a broken issuer before
  any outage.

### Negative
- This is the first Kubernetes footprint in the repo — it adds an
  operational surface (a cluster) that didn't previously need to exist
  for a Compose-deployed stack, and Compose and Kubernetes deployment
  paths now both exist and must be kept from drifting apart in
  documentation.
- Two Let's Encrypt issuers (staging/prod) plus a Vault-backed issuer is
  more moving parts than a single static-cert approach.

### Mitigations
- `infra/k8s/` is scoped narrowly to what secrets/TLS/vuln/compliance
  automation actually needs (namespaces `audit-ledger-secrets`,
  `audit-ledger-certs`, `audit-ledger`) — it is not a full migration of
  the existing relayer/metrics-exporter/prometheus/grafana/rest/ui stack
  off Docker Compose, which is out of scope for this change.
- `install.md` documents the staging-before-prod promotion rule
  explicitly so Let's Encrypt's production rate limits (50
  certs/domain/week) are never hit by test issuance.

---

## Alternatives Considered

| Option | Why not chosen |
|--------|-----------------|
| Compose-native ACME (Traefik/Caddy) | Meets "automated Let's Encrypt renewal" but not "cert-manager," "custom issuers" (no clean private-CA/Vault-PKI integration), or Kubernetes-native renewal monitoring as requested. |
| Docs/scaffolding only, no live cluster | Would not deliver working renewal automation or monitoring — just a design doc. Rejected since a Kubernetes path was worth introducing precisely because cert-manager is the tool actually requested. |
