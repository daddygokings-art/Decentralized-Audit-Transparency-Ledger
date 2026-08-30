# Kubernetes Security Controls

This bundle implements issues #551, #552, and #554 for workloads in the `audit-ledger` namespace.

## Prerequisites

- Kyverno installed with its admission webhook configured with `failurePolicy: Fail`.
- Cilium installed with mutual-authentication support enabled.
- SPIRE server and agents installed in the cluster, with Cilium configured to use the SPIRE trust domain and bundle endpoint.
- The `audit-ledger` namespace and workloads labeled with `app.kubernetes.io/part-of: audit-ledger`.
- Workload templates labeled `networking.audit-ledger.io/policy: enforced` and with `app.kubernetes.io/name` and `app.kubernetes.io/component` labels.
- Images published to `ghcr.io/daddygokings-art` by the protected `master` workflow.

Install the upstream operators according to their versioned documentation, then apply this bundle:

```bash
kubectl apply -k infra/k8s/security
kubectl -n audit-ledger get cnp audit-ledger-zero-trust
kubectl get clusterpolicy audit-ledger-verify-images audit-ledger-pod-baseline audit-ledger-require-network-policy
```

The signing workflow publishes immutable SHA-tagged images and creates keyless Sigstore signatures. Kyverno verifies the GitHub Actions OIDC issuer, the repository workflow identity, and the Rekor transparency-log entry before admitting a pod. Digest mutation prevents a tag from changing after admission.

## SPIFFE/SPIRE identity

Configure a SPIRE registration entry for each service account used by an AuditLedger workload. Use a distinct identity per service, for example:

```text
spiffe://audit-ledger.example/workload/audit-ledger/relayer
spiffe://audit-ledger.example/workload/audit-ledger/rest
spiffe://audit-ledger.example/workload/audit-ledger/metrics-exporter
```

Bind each identity to its Kubernetes namespace and service account, distribute the SPIRE trust bundle to Cilium, and confirm that Cilium reports the authentication policy as enforced. Do not use a shared wildcard identity for the namespace.

The Cilium policy defaults to deny for selected endpoints and permits only authenticated in-namespace service traffic, DNS, and HTTPS egress. Add an explicit narrow egress rule before deploying any workload that requires another destination.

## Operational checks

```bash
kubectl -n audit-ledger describe ciliumnetworkpolicy audit-ledger-zero-trust
kubectl get events -n audit-ledger --field-selector reason=PolicyViolation
kubectl get policyreport -A -l app.kubernetes.io/part-of=audit-ledger
```

Incident handling, including signature failures and bridge compromise, is documented in [the contract-event incident playbook](../../../docs/security/contract-event-incident-response.md).