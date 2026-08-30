# Security Chaos Engineering

## Overview

Security chaos engineering validates that the AuditLedger contract and off-chain stack can detect, respond to, and recover from security failures. This document describes the chaos test suite, execution procedures, and expected outcomes.

## Principles

1. **Hypothesis-driven:** Each chaos test states a hypothesis about system behavior under failure conditions.
2. **Blast radius limited:** Tests run against isolated testnet environments or local mocks.
3. **Automated detection:** Monitoring and alerting verify that failures are detected.
4. **Measurable recovery:** Recovery time and data integrity are quantified.

## Chaos Categories

### 1. Certificate Expiry

**Hypothesis:** The system detects expired TLS certificates and alerts before service disruption.

**Tests:**
- Rotate TLS certificate for REST API and verify old certificate is rejected
- Test webhook endpoints with expired certificates
- Verify certificate expiry alerts fire before actual expiry

**Recovery validation:**
- Automated certificate rotation restores service
- Alert is cleared after rotation

### 2. Key Rotation

**Hypothesis:** Owner key rotation does not disrupt contract operations and old key is immediately denied.

**Tests:**
```rust
// See src/chaos_tests.rs
chaos_key_rotation_old_key_denied
chaos_key_rotation_events_preserved
chaos_key_rotation_multiple_rotations
chaos_recovery_after_key_rotation
```

**Recovery validation:**
- New owner can perform all governance functions
- Old owner cannot perform any governance functions
- Events logged before and after rotation remain accessible
- Hash chain integrity is preserved

### 3. Permission Changes

**Hypothesis:** Permission changes (pause, block, allowlist) take effect immediately and can be reversed.

**Tests:**
```rust
// See src/chaos_tests.rs
chaos_permission_change_pause_blocks_writes
chaos_permission_change_governance_denied_while_paused
chaos_permission_change_submitter_blocklist
chaos_permission_change_allowlist_mode
```

**Recovery validation:**
- Pause/unpause cycle preserves all events
- Block/unblock cycle restores submitter access
- Allowlist mode correctly filters submitters
- Governance functions remain blocked while paused

### 4. Network Partition

**Hypothesis:** Network partition between off-chain services and contract does not corrupt state.

**Tests:**
- Simulate RPC unavailability and verify retry behavior
- Test event queuing during network partition
- Verify events are not duplicated after partition heals

**Recovery validation:**
- Events queued during partition are flushed on reconnection
- No duplicate events appear after partition heals
- Hash chain remains valid after recovery

### 5. Dependency Failure

**Hypothesis:** Off-chain service failures do not corrupt on-chain state or data consistency.

**Tests:**
- Metrics exporter unavailable: verify contract continues operating
- REST API unavailable: verify contract continues operating
- Database failure in off-chain service: verify no state corruption
- Webhook delivery failure: verify event is logged but webhook retries

**Recovery validation:**
- Off-chain services resume from last known state
- No events are lost or duplicated
- Metrics catch up after service restart

## Kubernetes Litmus Chaos

For Kubernetes environments, the repository includes a LitmusChaos setup under `infra/k8s/litmus` that exercises pod failure, network partition, CPU saturation, memory saturation, and automated recovery validation for the AuditLedger workloads in the `audit-ledger` namespace.

```bash
# Install Litmus in the cluster, then apply the repo config
kubectl apply -f https://raw.githubusercontent.com/litmuschaos/litmus/master/litmus-operator.yaml
kubectl apply -k infra/k8s/litmus

# Trigger the chaos workflow
kubectl get chaosengine -n litmus
kubectl describe chaosengine audit-ledger-chaos-engine -n litmus
```

### Included experiments
- `pod-delete` for pod failure injection
- `pod-network-chaos` for network partition simulation
- `pod-cpu-hog` for resource exhaustion under CPU pressure
- `pod-memory-hog` for memory exhaustion conditions
- `audit-ledger-recovery-validation` to confirm the application recovers within the expected window

### Recovery gates
- The engine monitors `app.kubernetes.io/part-of=audit-ledger`
- Recovery validation fails if the app does not become healthy within 60 seconds
- Each experiment is recorded with a `ChaosResult` for automated verification and auditability

## Running Chaos Tests

### Prerequisites

```bash
# Rust test suite
cargo test --features chaos

# Or run specific chaos tests
cargo test chaos_key_rotation
cargo test chaos_permission_change
cargo test chaos_recovery
```

### Testnet Chaos

```bash
# Deploy to isolated testnet
export SOROBAN_SECRET_KEY="<testnet_key>"
./scripts/deploy_testnet.sh

# Run chaos test suite against live testnet
cargo test chaos -- --nocapture
```

### CI Integration

Chaos tests run in CI as part of the security pipeline:

```yaml
# .github/workflows/chaos.yml
name: Chaos Engineering

on:
  schedule:
    - cron: '0 2 * * 1'  # Weekly on Monday
  workflow_dispatch:

jobs:
  chaos:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test chaos -- --nocapture
```

## Monitoring Chaos Tests

### Metrics to Track

| Metric | Description | Threshold |
|--------|-------------|-----------|
| Recovery time | Time from failure to normal operation | < 60s |
| Data loss | Events lost during chaos | 0 |
| Duplicate events | Events duplicated after recovery | 0 |
| Detection time | Time from failure to alert | < 30s |
| False positive rate | Alerts fired without actual failure | < 1% |

### Alert Validation

Each chaos test should validate that:
1. The expected alert fires
2. The alert contains sufficient context for investigation
3. The alert severity matches the actual impact

## Chaos Test Inventory

| Test | Category | Hypothesis | Status |
|------|----------|------------|--------|
| `chaos_key_rotation_old_key_denied` | Key Rotation | Old key denied after rotation | Pass |
| `chaos_key_rotation_events_preserved` | Key Rotation | Events preserved after rotation | Pass |
| `chaos_key_rotation_multiple_rotations` | Key Rotation | Multiple rotations maintain security | Pass |
| `chaos_permission_change_pause_blocks_writes` | Permissions | Pause blocks writes | Pass |
| `chaos_permission_change_governance_denied_while_paused` | Permissions | Governance denied while paused | Pass |
| `chaos_permission_change_submitter_blocklist` | Permissions | Blocklist prevents submission | Pass |
| `chaos_permission_change_allowlist_mode` | Permissions | Allowlist mode filters correctly | Pass |
| `chaos_metadata_schema_change_enforces_constraint` | Permissions | Schema changes enforce constraints | Pass |
| `chaos_event_cap_change_removal` | Permissions | Cap removal restores logging | Pass |
| `chaos_ttl_configuration_change` | Storage | TTL changes apply correctly | Pass |
| `chaos_nonce_configuration_change` | Auth | Nonce config changes work | Pass |
| `chaos_rate_limit_change` | Auth | Rate limits apply immediately | Pass |
| `chaos_recovery_after_pause_unpause` | Recovery | Recovery after pause/unpause | Pass |
| `chaos_recovery_after_key_rotation` | Recovery | Recovery after key rotation | Pass |
| `chaos_recovery_after_cap_removal` | Recovery | Recovery after cap removal | Pass |

## References

- [Principles of Chaos Engineering](https://principlesofchaos.org/)
- [Gremlin Chaos Engineering](https://www.gremlin.com/chaos-engineering/)
- [Netflix Chaos Engineering](https://netflixtechblog.com/tagged/chaos-engineering)
- [LitmusChaos Documentation](https://docs.litmuschaos.io/)
