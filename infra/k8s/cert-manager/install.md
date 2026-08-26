# cert-manager installation

cert-manager is installed via its official Helm chart, not managed as raw
manifests in this repo (its CRDs are large and versioned upstream).

```bash
helm repo add jetstack https://charts.jetstack.io
helm repo update

helm install cert-manager jetstack/cert-manager \
  --namespace audit-ledger-certs --create-namespace \
  --version v1.15.3 \
  --set crds.enabled=true \
  --set prometheus.enabled=true \
  --set prometheus.servicemonitor.enabled=true
```

After the controller is up, apply the issuers and certificates in this
directory in order:

```bash
kubectl apply -f infra/k8s/cert-manager/cluster-issuer-letsencrypt-staging.yaml
kubectl apply -f infra/k8s/cert-manager/cluster-issuer-letsencrypt-prod.yaml
kubectl apply -f infra/k8s/cert-manager/cluster-issuer-internal-ca.yaml
kubectl apply -f infra/k8s/cert-manager/certificates.yaml
kubectl apply -f infra/k8s/monitoring/cert-expiry-alerts.yaml
```

## Why two Let's Encrypt issuers

`letsencrypt-staging` is the default for every `Certificate` while
testing DNS-01/HTTP-01 solvers, PR preview environments, and new domains
— Let's Encrypt's production issuer has tight rate limits (50
certs/registered domain/week) and staging shares the same ACME flow
without counting against them. Only promote a `Certificate`'s
`issuerRef` to `letsencrypt-prod` once staging has issued successfully
for that exact set of DNS names.

## Bootstrap order (breaking the Vault/cert-manager circular dependency)

`internal-ca-vault` issues certs *by calling Vault*, but Vault's own
listener cert (`vault-tls` in `certificates.yaml`) is issued *by that
same issuer* — Vault can't serve TLS until it has a cert, and the issuer
can't reach Vault over TLS until Vault is serving it. Break the cycle
once, at cluster bring-up, then never again:

1. Install cert-manager and apply the Let's Encrypt issuers.
2. Generate Vault's first listener cert manually (`vault write -f
   pki/issue/internal-mtls common_name=vault.audit-ledger-secrets.svc`
   against a Vault instance temporarily run with `tls_disable = 1`, or a
   short-lived self-signed cert), install the Vault Helm release with it.
3. Unseal Vault, enable the `pki` mount and `cert-manager` Kubernetes
   auth role (see the prerequisite block in
   `cluster-issuer-internal-ca.yaml`), then apply that ClusterIssuer.
4. Apply `certificates.yaml`'s `vault-tls` Certificate — cert-manager now
   takes over Vault's cert going forward, including all renewals.

## Renewal

cert-manager renews automatically at 2/3 of a certificate's lifetime by
default (for a 90-day Let's Encrypt cert, that's ~day 60). No manual
renewal step is needed; `infra/k8s/monitoring/cert-expiry-alerts.yaml`
exists to catch the case where automatic renewal itself is broken
(DNS/ACME account/quota problems), not to replace it.

## Validation and rollback

cert-manager's own `Certificate` status conditions (`Ready`) are the
validation signal — a `Certificate` that fails to renew keeps serving
the last-issued cert until its actual expiry (cert-manager never swaps
in a broken cert), so "rollback" here means alerting before expiry
rather than reverting material. `infra/k8s/monitoring/cert-expiry-alerts.yaml`
fires if a `Certificate`'s expiry falls under 14 days, which for a
60-day renewal target gives ~46 days of buffer to fix a broken issuer
before anything actually expires.
