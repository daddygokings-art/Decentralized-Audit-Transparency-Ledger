# Certificate Management

| | |
|---|---|
| Status | Active |
| Owner | Security team |
| Related | [../adr/ADR-012-kubernetes-cert-manager-tls.md](../adr/ADR-012-kubernetes-cert-manager-tls.md), [chaos-engineering.md](chaos-engineering.md) |

TLS certificates are managed by [cert-manager](https://cert-manager.io/)
running in the `audit-ledger-certs` namespace. See
`infra/k8s/cert-manager/install.md` for installation steps and the
Vault/cert-manager bootstrap order.

## Issuers

| Issuer | Type | Use |
|---|---|---|
| `letsencrypt-staging` | ACME (Let's Encrypt staging) | Default for all new `Certificate` resources — validate the DNS-01/HTTP-01 solver works before touching production rate limits. |
| `letsencrypt-prod` | ACME (Let's Encrypt production) | Public-facing certs (`api.audit-ledger.example`, `app.audit-ledger.example`), promoted from staging once issuance is proven. |
| `internal-ca-vault` | Custom (Vault PKI secrets engine) | Internal mTLS — Vault's own listener, service-to-service traffic. Never a public CA target; these names aren't internet-resolvable. |

`letsencrypt-staging`/`letsencrypt-prod` use HTTP-01 for public ingress
hosts and DNS-01 (Route53) for anything under `internal.audit-ledger.example`
that can't serve an HTTP-01 challenge. `internal-ca-vault` is defined in
`infra/k8s/cert-manager/cluster-issuer-internal-ca.yaml` and signs
through the same Vault instance used for [secrets rotation](secrets-rotation.md)
— see `infra/k8s/cert-manager/vault-pki-policy.hcl` for its scoped
permissions.

## Renewal automation

cert-manager renews automatically once a `Certificate` reaches its
`renewBefore` threshold — no manual step. Current settings
(`infra/k8s/cert-manager/certificates.yaml`):

| Certificate | Lifetime | Renews at |
|---|---|---|
| `audit-ledger-api-tls`, `audit-ledger-ui-tls` | 90d (Let's Encrypt max) | 30d remaining |
| `vault-tls` (internal) | 30d | 10d remaining |

Private keys rotate on every renewal (`rotationPolicy: Always`) rather
than being reused across renewals.

## Monitoring

`infra/k8s/monitoring/cert-expiry-alerts.yaml` (PrometheusRule, requires
cert-manager's `prometheus.enabled=true`):

- `CertificateExpiringSoon` — under 14 days to expiry. For a
  30-day-remaining renewal trigger, this only fires if automatic renewal
  itself is broken, giving ~2 weeks to fix the issuer before an actual
  outage.
- `CertificateNotReady` — cert-manager reports the `Certificate` as not
  `Ready` for 30+ minutes.
- `ACMEChallengeFailing` — repeated ACME client errors (DNS/HTTP-01
  solver misconfiguration).
- `CertificateRenewalStalled` — no renewal recorded past the expected
  window.

## Why no manual rollback step

A `Certificate` that fails to renew keeps serving its last-issued,
still-valid cert — cert-manager never swaps in a broken/unissued
certificate. "Rollback" for TLS therefore means *alerting with enough
lead time to fix the issuer*, not reverting certificate material; the
14-day alert threshold against a 30-day renewal trigger is the control
that replaces a rollback step.

## References

- `infra/k8s/cert-manager/install.md` — installation, bootstrap order
- [ADR-012: Adopt Kubernetes + cert-manager for TLS](../adr/ADR-012-kubernetes-cert-manager-tls.md)
- [chaos-engineering.md](chaos-engineering.md) — "Certificate Expiry" chaos test category
