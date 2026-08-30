# Attack Surfaces and Risk Assessment Matrix

## 1. Attack Surface Inventory

| ID | Component / Interface | Entry Protocol | Authentication Mechanism | Trust Boundary | Risk Rating |
|----|-----------------------|----------------|--------------------------|----------------|-------------|
| `AS-01` | Soroban Contract Invocations | Stellar RPC / Horizon | Stellar Transaction Signature (`require_auth`) | Public -> Blockchain Engine | Medium |
| `AS-02` | REST API Endpoints (`/api/v1/*`) | HTTPS / TLS 1.3 | OAuth2 / Bearer Tokens / API Keys | Public -> API Gateway | High |
| `AS-03` | WebSocket Subscription Feed | WSS / TLS 1.3 | JWT / Session Token | Public -> Notifier Service | Medium |
| `AS-04` | Cross-Chain Relayer Ingress | RPC / JSON-RPC | Relayer Private Key / Merkle Proof | Relayer -> EVM Verifier | High |
| `AS-05` | HashiCorp Vault Transit API | HTTPS / mTLS | Kubernetes ServiceAccount / SPIFFE SVID | Cluster Core -> Vault KMS | Critical |
| `AS-06` | Prometheus Metrics Exporter | HTTP (Internal) | NetworkPolicy Isolation | Core -> Observability | Low |

## 2. Risk Assessment Matrix

```
  +------------------+----------+----------+----------+
  | IMPACT / LIKELIH | Low      | Medium   | High     |
  +------------------+----------+----------+----------+
  | High             | Medium   | High     | Critical |
  | Medium           | Low      | Medium   | High     |
  | Low              | Low      | Low      | Medium   |
  +------------------+----------+----------+----------+
```

### Risk Scoring and Remediation Summary
| Risk ID | Title | Inherent Score | Residual Score | Mitigation Strategy | Status |
|---------|-------|----------------|----------------|---------------------|--------|
| `RSK-01` | Governance Key Compromise | Critical (9.0) | Low (2.5) | 3-of-5 Hardware Multisig + Timelock | Accepted |
| `RSK-02` | Cross-Chain Relayer Spoofing | High (8.2) | Low (2.0) | EVM Merkle Proof Cryptographic Verification | Accepted |
| `RSK-03` | API DDoS / Rate Limit Bypass | High (7.5) | Low (2.2) | Cloudflare WAF + Redis Sliding Window | Accepted |
| `RSK-04` | Sensitive Data Leak on Ledger | High (8.0) | Low (1.8) | Client-side E2E Encryption + Crypto-shredding | Accepted |
| `RSK-05` | Container Lateral Movement | Medium (6.5) | Low (1.5) | K8s Default-Deny NetworkPolicies + mTLS | Accepted |

## 3. Security Requirements Traceability Matrix (SRTM)

| Security Requirement | Threat Mitigated | Technical Enforcement | Verification Test |
|----------------------|------------------|-----------------------|-------------------|
| `REQ-SEC-01`: Events must be append-only and cryptographically chained | `THR-SC-02` | SHA-256 Chaining in `src/lib.rs` | `tests/tamper_evidence_tests.rs` |
| `REQ-SEC-02`: Governance operations must require multisig quorum | `THR-SC-01` | Multisig check in `src/dao_governance.rs` | `tests/governance_tests.rs` |
| `REQ-SEC-03`: Personal data must be crypto-shredded upon verified erasure | `THR-SC-04` | Metadata redaction in `src/data_retention.rs` | `tests/data_retention_tests.rs` |
| `REQ-SEC-04`: Inter-service calls must use mutual TLS | `THR-API-01` | cert-manager + Istio mTLS | `infra/k8s/zero-trust/` |
