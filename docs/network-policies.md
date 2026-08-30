# Network Policies & Service Mesh — Operational Guide

This guide covers day-to-day operations for the audit-ledger zero-trust
networking layer: Kubernetes NetworkPolicies (L3/L4) and the Istio service
mesh (L7).

See [docs/adr/ADR-015-service-mesh.md](adr/ADR-015-service-mesh.md) for the
architecture decision record and rationale.

---

## Table of Contents

1. [Directory Layout](#directory-layout)
2. [Prerequisites](#prerequisites)
3. [Initial Installation](#initial-installation)
4. [Applying Network Policies](#applying-network-policies)
5. [Applying the Service Mesh](#applying-the-service-mesh)
6. [Verifying mTLS](#verifying-mtls)
7. [Traffic Splitting (Canary Rollouts)](#traffic-splitting-canary-rollouts)
8. [Observability](#observability)
9. [Linkerd Alternative](#linkerd-alternative)
10. [Troubleshooting](#troubleshooting)
11. [Adding a New Service](#adding-a-new-service)

---

## Directory Layout

```
infra/k8s/
├── network-policies/
│   ├── default-deny.yaml          # deny-all for audit-ledger, secrets, certs namespaces
│   ├── allow-workloads.yaml       # per-service allow rules (7 services)
│   ├── allow-vault.yaml           # Vault + cert-manager allow rules
│   └── kustomization.yaml
└── service-mesh/
    ├── istio-operator.yaml        # Istio control-plane installation manifest
    ├── namespace-injection.yaml   # sidecar injection label
    ├── peer-authentication.yaml   # mTLS STRICT
    ├── authorization-policies.yaml# deny-all + per-service ALLOW (L7)
    ├── gateway.yaml               # TLS termination at the ingress edge
    ├── kustomization.yaml
    ├── traffic/
    │   ├── virtual-services.yaml  # routing, retries, timeouts, canary weights
    │   ├── destination-rules.yaml # ISTIO_MUTUAL TLS, subsets, outlier detection
    │   └── kustomization.yaml
    ├── observability/
    │   ├── telemetry.yaml         # Prometheus metrics + Jaeger tracing
    │   ├── service-monitors.yaml  # Prometheus scrape targets
    │   ├── kiali.yaml             # Kiali topology UI
    │   └── kustomization.yaml
    └── linkerd/                   # Linkerd alternative (not applied by default)
        ├── namespace-annotation.yaml
        ├── servers.yaml
        └── kustomization.yaml
```

---

## Prerequisites

| Tool | Minimum version | Install |
|------|-----------------|---------|
| `kubectl` | 1.28 | https://kubernetes.io/docs/tasks/tools/ |
| `istioctl` | 1.21 | https://istio.io/latest/docs/setup/getting-started/#download |
| `helm` | 3.14 | https://helm.sh/docs/intro/install/ |
| Prometheus Operator | 0.73 | `helm install ... kube-prometheus-stack` |
| cert-manager | 1.14 | see `infra/k8s/cert-manager/install.md` |

---

## Initial Installation

### 1. Install Istio

```bash
# Download and install istioctl (version pinned to match istio-operator.yaml)
curl -L https://istio.io/downloadIstio | ISTIO_VERSION=1.21.0 sh -
export PATH="$PWD/istio-1.21.0/bin:$PATH"

# Install the control plane using the operator manifest
istioctl install -f infra/k8s/service-mesh/istio-operator.yaml --verify

# Confirm all Istio components are running
kubectl get pods -n istio-system
kubectl get pods -n istio-ingress
```

### 2. Install Kiali operator (optional but recommended)

```bash
helm repo add kiali https://kiali.org/helm-charts
helm repo update
helm install kiali-operator kiali/kiali-operator \
  -n kiali-operator --create-namespace \
  --version 1.86.0
```

### 3. Apply the full stack

```bash
# Network policies first (independent of mesh)
kubectl apply -k infra/k8s/network-policies/

# Service mesh (Istio)
kubectl apply -k infra/k8s/service-mesh/

# Restart existing pods to inject sidecars
kubectl rollout restart deployment -n audit-ledger
```

---

## Applying Network Policies

NetworkPolicies are managed independently of the service mesh. Apply them
even if you are not using a mesh.

```bash
# Apply all policies at once
kubectl apply -k infra/k8s/network-policies/

# Verify policies were created
kubectl get networkpolicies -n audit-ledger
kubectl get networkpolicies -n audit-ledger-secrets
kubectl get networkpolicies -n audit-ledger-certs
```

Expected output in `audit-ledger`:
```
NAME                      POD-SELECTOR
default-deny-all          <none>
allow-api-rest            app.kubernetes.io/component=api-rest
allow-api-graphql         app.kubernetes.io/component=api-graphql
allow-api-ws              app.kubernetes.io/component=api-ws
allow-ui                  app.kubernetes.io/component=ui
allow-metrics-exporter    app.kubernetes.io/component=metrics-exporter
allow-notifier            app.kubernetes.io/component=notifier
allow-relayer             app.kubernetes.io/component=relayer
```

### Testing connectivity

```bash
# From a debug pod, verify that a blocked path is denied:
kubectl run nettest --image=curlimages/curl --rm -it \
  --namespace=audit-ledger -- \
  curl -v http://api-rest.audit-ledger.svc.cluster.local:3002/healthz

# From a permitted pod (e.g. ui → api-rest), verify it succeeds:
kubectl exec -n audit-ledger deploy/ui -- \
  curl -sv http://api-rest.audit-ledger.svc.cluster.local:3002/healthz
```

---

## Applying the Service Mesh

```bash
# Apply all mesh resources
kubectl apply -k infra/k8s/service-mesh/

# Check PeerAuthentication
kubectl get peerauthentication -n audit-ledger
kubectl get peerauthentication -n istio-ingress

# Check AuthorizationPolicies
kubectl get authorizationpolicy -n audit-ledger

# Check VirtualServices and DestinationRules
kubectl get virtualservices,destinationrules -n audit-ledger
```

---

## Verifying mTLS

```bash
# Check mTLS status for all pods in the namespace
istioctl x check-inject -n audit-ledger

# Verify a specific pod has a sidecar
kubectl get pod -n audit-ledger -l app.kubernetes.io/component=api-rest \
  -o jsonpath='{.items[0].spec.containers[*].name}'
# Expected: api-rest istio-proxy

# Confirm STRICT mTLS is active (no PERMISSIVE connections)
istioctl authn tls-check -n audit-ledger \
  api-rest.audit-ledger.svc.cluster.local

# Inspect the certificate issued to a pod
istioctl proxy-config secret -n audit-ledger \
  deploy/api-rest --output json | jq '.[0].secret.tlsCertificate'
```

The output should show:
- `TLS_VERSION_1_3` or `TLS_VERSION_1_2`
- `CLIENT_AND_SERVER` (mutual authentication)
- A valid SPIFFE URI SAN: `spiffe://cluster.local/ns/audit-ledger/sa/api-rest`

---

## Traffic Splitting (Canary Rollouts)

Canary deployments use the `app.kubernetes.io/track` label on pods.

### Deploy a canary

```bash
# Tag the canary deployment
kubectl patch deployment api-rest-canary -n audit-ledger \
  --type=json \
  -p='[{"op":"add","path":"/spec/template/metadata/labels/app.kubernetes.io~1track","value":"canary"}]'

# Shift 10 % of traffic to the canary
kubectl patch virtualservice api-rest -n audit-ledger \
  --type=json \
  -p='[
    {"op":"replace","path":"/spec/http/1/route/0/weight","value":90},
    {"op":"replace","path":"/spec/http/1/route/1/weight","value":10}
  ]'

# Monitor error rates and latency in Kiali or Grafana
# If healthy, promote to 100 %
kubectl patch virtualservice api-rest -n audit-ledger \
  --type=json \
  -p='[
    {"op":"replace","path":"/spec/http/1/route/0/weight","value":100},
    {"op":"replace","path":"/spec/http/1/route/1/weight","value":0}
  ]'
```

### Rollback

```bash
# Instantly return all traffic to stable
kubectl patch virtualservice api-rest -n audit-ledger \
  --type=json \
  -p='[
    {"op":"replace","path":"/spec/http/1/route/0/weight","value":100},
    {"op":"replace","path":"/spec/http/1/route/1/weight","value":0}
  ]'
```

---

## Observability

### Kiali (topology UI)

After installing the Kiali CR:
```bash
# Port-forward if no ingress is configured
kubectl port-forward svc/kiali -n istio-system 20001:20001

# Open http://localhost:20001/kiali
```

Use the Graph view to see real-time traffic flow, mTLS lock icons, and
error rates for each service-to-service edge.

### Prometheus metrics

Istio sidecar metrics are scraped via the `ServiceMonitor` resources in
`observability/service-monitors.yaml`. Key metrics:

| Metric | Description |
|--------|-------------|
| `istio_requests_total` | Request count by source, destination, response code |
| `istio_request_duration_milliseconds` | Latency histogram per call path |
| `istio_tcp_connections_opened_total` | TCP connection count |
| `pilot_xds_push_time` | Control-plane xDS push latency |

### Distributed tracing (Jaeger)

```bash
kubectl port-forward svc/jaeger-query -n monitoring 16686:16686
# Open http://localhost:16686
# Search for service: audit-ledger or component: api-rest
```

Trace sampling is set to 100 % in staging. Lower it in production:

```bash
kubectl patch telemetry audit-ledger-telemetry -n audit-ledger \
  --type=json \
  -p='[{"op":"replace","path":"/spec/tracing/0/randomSamplingPercentage","value":1.0}]'
```

---

## Linkerd Alternative

If you prefer Linkerd over Istio:

```bash
# Install Linkerd
curl -sL run.linkerd.io/install | sh
linkerd install --crds | kubectl apply -f -
linkerd install | kubectl apply -f -
linkerd check

# Apply Linkerd-specific resources (do NOT apply Istio resources as well)
kubectl apply -k infra/k8s/service-mesh/linkerd/

# Verify proxy injection
linkerd check --proxy -n audit-ledger

# Inspect traffic (Linkerd equivalent of Kiali)
linkerd viz install | kubectl apply -f -
linkerd viz dashboard
```

The `linkerd/servers.yaml` defines a `Server` and `ServerAuthorization` for
each service port, enforcing the same deny-all + explicit-allow policy as
the Istio `AuthorizationPolicy` resources.

---

## Troubleshooting

### Pod cannot reach another pod after NetworkPolicy applied

```bash
# Check which policies apply to the source pod
kubectl describe networkpolicy -n audit-ledger

# Temporarily add a broad allow rule for debugging (REMOVE AFTER):
kubectl run debug-allow --image=nicolaka/netshoot --rm -it -n audit-ledger -- bash
```

### mTLS handshake failures (503 / PEER_CERT_NOT_PROVIDED)

```bash
# Check the peer-authentication status
kubectl describe peerauthentication default-mtls-strict -n audit-ledger

# Check if the destination pod has a sidecar
kubectl get pod -n audit-ledger -o wide \
  -l app.kubernetes.io/component=api-graphql

# Examine proxy config for the destination
istioctl proxy-config listener -n audit-ledger deploy/api-graphql
```

### AuthorizationPolicy denying legitimate traffic (403)

```bash
# Check the access logs (Envoy JSON logs)
kubectl logs -n audit-ledger deploy/api-rest -c istio-proxy | \
  jq 'select(.response_code == 403)'

# Temporarily switch to AUDIT mode to trace denials without blocking:
kubectl patch authorizationpolicy allow-api-rest -n audit-ledger \
  --type=json \
  -p='[{"op":"replace","path":"/spec/action","value":"AUDIT"}]'
# Check the Envoy access log for the AUDIT entries, then switch back to ALLOW.
```

### VirtualService not taking effect

```bash
# Confirm the VirtualService is bound to the correct gateway and hosts
kubectl describe virtualservice api-rest -n audit-ledger

# Force a config push
istioctl proxy-config routes -n audit-ledger deploy/api-rest
```

---

## Adding a New Service

1. **NetworkPolicy** — add a block in `allow-workloads.yaml` with `podSelector`
   matching the new component label, and explicit ingress/egress ports.

2. **AuthorizationPolicy** — add an `AuthorizationPolicy` in
   `authorization-policies.yaml` with the correct SPIFFE principal for each
   allowed caller.

3. **VirtualService** — add a route block in `traffic/virtual-services.yaml`
   with timeout and retry settings appropriate for the new service.

4. **DestinationRule** — add a `DestinationRule` in
   `traffic/destination-rules.yaml` with `ISTIO_MUTUAL` TLS and
   `stable`/`canary` subsets.

5. **Linkerd** (if using Linkerd) — add a `Server` and `ServerAuthorization`
   pair in `linkerd/servers.yaml`.

6. Apply changes:
   ```bash
   kubectl apply -k infra/k8s/network-policies/
   kubectl apply -k infra/k8s/service-mesh/
   ```

7. Verify with `istioctl authn tls-check` and check Kiali graph for the new
   service edge.
