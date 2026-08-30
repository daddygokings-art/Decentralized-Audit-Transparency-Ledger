# ADR-015 — Service Mesh and Zero-Trust Networking

| Field       | Value                              |
|-------------|------------------------------------|
| Status      | Accepted                           |
| Date        | 2026-08-29                         |
| Issue       | #506                               |
| Authors     | audit-ledger contributors          |
| Supersedes  | —                                  |
| Related     | ADR-012 (cert-manager/TLS), ADR-011 (Vault secrets) |

---

## Context

The audit-ledger stack runs multiple services in Kubernetes (REST API, GraphQL
API, WebSocket stream, UI, metrics exporter, bridge relayer, notifier).
Before this ADR, service-to-service communication inside the cluster:

- Used plaintext HTTP/TCP (no encryption at rest on the wire)
- Had no machine identity — any pod could reach any other pod
- Had no fine-grained access control below the NetworkPolicy level
- Produced no mesh-level observability (topology, latency per call path)

The Stellar/Soroban contract enforces an immutable on-chain audit trail.
The off-chain services must meet the same standard: every call between
components must be authenticated, encrypted, and logged.

This ADR records the decision to adopt a **service mesh** providing:

1. **Mutual TLS (mTLS)** for all east-west traffic — encryption + identity
2. **Zero-trust AuthN/AuthZ** — deny-all by default, explicit allow per call path
3. **Traffic management** — canary splits, retries, timeouts, circuit breaking
4. **Observability** — request-level metrics, distributed traces, topology graph

---

## Decision

### Primary mesh: Istio

Istio was chosen as the primary service mesh for the audit-ledger stack.

Resources in `infra/k8s/service-mesh/`:

| File | Purpose |
|------|---------|
| `istio-operator.yaml` | IstioOperator CR — control-plane installation |
| `namespace-injection.yaml` | Sidecar injection label for `audit-ledger` namespace |
| `peer-authentication.yaml` | `PeerAuthentication` — mTLS STRICT cluster-wide |
| `authorization-policies.yaml` | `AuthorizationPolicy` — deny-all + per-service ALLOW |
| `gateway.yaml` | `Gateway` — TLS termination at the ingress edge |
| `traffic/virtual-services.yaml` | `VirtualService` — routing, retries, timeouts, canary weights |
| `traffic/destination-rules.yaml` | `DestinationRule` — ISTIO_MUTUAL TLS, subsets, outlier detection |
| `observability/telemetry.yaml` | `Telemetry` — Prometheus metrics + Jaeger tracing |
| `observability/service-monitors.yaml` | `ServiceMonitor` — Prometheus scrape targets |
| `observability/kiali.yaml` | `Kiali` CR — topology UI |

### Alternative mesh: Linkerd

Linkerd resources are provided in `infra/k8s/service-mesh/linkerd/` as a
**drop-in alternative** for teams that prefer a lighter-weight mesh.

Linkerd was modelled because:

- Its control plane is ~5× smaller in CPU/memory than Istio's
- It uses Rust-based proxies (memory-safe, relevant to this Rust-first project)
- Its policy model (`Server` + `ServerAuthorization`) is simpler to audit

Linkerd is not the default choice because:

- It lacks native traffic-splitting (VirtualService equivalent) without the
  SMI `TrafficSplit` CRD, which adds an extra dependency
- Kiali does not support Linkerd natively — a separate Grafana/Buoyant Cloud
  setup is needed for equivalent topology visibility
- The team has more operational experience with Istio

### Network Policies

`infra/k8s/network-policies/` provides Kubernetes `NetworkPolicy` resources
as an **independent defence-in-depth layer** that does not rely on the mesh.
They operate at L3/L4 and are enforced by the CNI plugin (Cilium or
Calico), providing protection even if a mesh sidecar is missing or bypassed.

---

## Alternatives Considered

### No service mesh (NetworkPolicies only)

NetworkPolicies operate at L3/L4 only. They cannot:

- Enforce mTLS (no L7 awareness)
- Control access by HTTP method or path
- Provide per-request metrics or distributed traces

Rejected: insufficient security and observability for an audit-grade system.

### Cilium (eBPF-native mesh)

Cilium with Hubble and the Cilium Service Mesh offers similar capabilities
without sidecars (eBPF kernel module instead). Advantages:

- No sidecar overhead — latency and CPU savings
- Deep eBPF-based network visibility via Hubble

Not chosen because:

- Requires a specific CNI (cannot be layered on an arbitrary CNI)
- Hubble's mTLS implementation (node-to-node transparent encryption) does
  not provide workload-level SPIFFE identity out of the box
- Operational knowledge investment higher than Istio for this team

Left as a future consideration for a dedicated Cilium ADR.

### Consul Connect

Consul's service mesh offers HashiCorp Vault integration as a first-class
feature. Given that this project already uses Vault, this was appealing.
Not chosen because the existing Vault integration (cert-manager + CSI) is
already working and adding Consul adds a third control-plane component.

---

## Zero-Trust Design Principles Applied

| Principle | Implementation |
|-----------|----------------|
| Never trust, always verify | `PeerAuthentication: STRICT` rejects any non-mTLS connection |
| Least privilege | `AuthorizationPolicy: deny-all` baseline; explicit ALLOW per call path |
| Assume breach | NetworkPolicies at L3/L4 + mesh policy at L7 — two independent layers |
| Verify explicitly | SPIFFE X.509 SVIDs issued per workload by istiod CA |
| Encrypt everywhere | ISTIO_MUTUAL TLS on all `DestinationRule` hosts |

---

## Traffic Management

### Canary rollout

VirtualServices split traffic between `stable` and `canary` pod subsets
(keyed by `app.kubernetes.io/track` label). The default weights are
`stable: 100, canary: 0`. To roll out a canary:

```bash
kubectl patch virtualservice api-rest -n audit-ledger \
  --type=json \
  -p='[{"op":"replace","path":"/spec/http/1/route/1/weight","value":10},
       {"op":"replace","path":"/spec/http/1/route/0/weight","value":90}]'
```

### Retry policy

Read paths (GET): 3 retries, 3 s per-try timeout, retry on
`connect-failure,reset,retriable-4xx,gateway-error`.

Write paths: 1 attempt (retrying non-idempotent writes risks duplicate events).

### Circuit breaking

DestinationRules include outlier detection (5 consecutive 5xx errors in 30 s
→ eject pod for 30 s, up to 50 % of the pool). This prevents a single
degraded instance from accepting traffic during an incident.

---

## Observability

| Signal | Source | Consumer |
|--------|--------|----------|
| Request metrics | Envoy sidecar `:15090/stats/prometheus` | Prometheus → Grafana |
| mTLS certificate metrics | istiod `:15014` | Prometheus |
| Distributed traces | Envoy → Zipkin wire format | Jaeger |
| Service topology | Prometheus + Kiali | Kiali UI |
| Access logs | Envoy → stdout (JSON) | log aggregator (Loki/Fluentd) |

---

## Consequences

### Positive

- All east-west traffic is encrypted and mutually authenticated
- Pod identity is cryptographically verified (SPIFFE X.509 SVIDs)
- Call graph, latency histograms, and error rates are observable
- Canary and blue-green releases are possible without code changes
- NetworkPolicies provide a fallback if mesh sidecars are evicted

### Negative / Trade-offs

- ~10–15 ms added latency per hop (Envoy double-proxying) — acceptable for
  audit ledger workloads; Stellar RPC roundtrip dominates latency
- Sidecar injection adds ~50 MB RSS per pod — acceptable for current scale
- Increased cluster complexity; operators need Istio knowledge
- `istioctl` must be installed for control-plane operations

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Sidecar upgrade disruption | Rolling restart via `kubectl rollout restart` |
| CA compromise | Vault PKI as external CA (replace istiod CA in production) |
| Policy misconfiguration locks out services | Staging environment validates policies before prod |
| Linkerd files unused/stale | Clearly scoped to `linkerd/` subdirectory; CI lint prevents invalid YAML |

---

## Implementation Checklist

- [x] `infra/k8s/network-policies/` — NetworkPolicies (L3/L4)
- [x] `infra/k8s/service-mesh/istio-operator.yaml` — Istio control plane
- [x] `infra/k8s/service-mesh/namespace-injection.yaml` — sidecar injection
- [x] `infra/k8s/service-mesh/peer-authentication.yaml` — mTLS STRICT
- [x] `infra/k8s/service-mesh/authorization-policies.yaml` — L7 AuthZ
- [x] `infra/k8s/service-mesh/gateway.yaml` — ingress TLS termination
- [x] `infra/k8s/service-mesh/traffic/` — VirtualService + DestinationRule
- [x] `infra/k8s/service-mesh/observability/` — Telemetry + ServiceMonitor + Kiali
- [x] `infra/k8s/service-mesh/linkerd/` — Linkerd alternative
- [x] `docs/adr/ADR-015-service-mesh.md` — this document
- [x] `docs/network-policies.md` — operational guide

## References

- [Istio Security Concepts](https://istio.io/latest/docs/concepts/security/)
- [Istio Traffic Management](https://istio.io/latest/docs/concepts/traffic-management/)
- [Linkerd Policy](https://linkerd.io/2.14/features/server-policy/)
- [NIST SP 800-207 Zero Trust Architecture](https://csrc.nist.gov/publications/detail/sp/800-207/final)
- [SPIFFE/SPIRE](https://spiffe.io/)
