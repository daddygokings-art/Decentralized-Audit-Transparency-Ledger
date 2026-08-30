# Contract Event Runbook Automation & Validation

This document provides a detailed overview of the operational runbook automation engine, validation framework, and playbooks for mission-critical ledger operations.

---

## 1. Architecture & Execution Lifecycle

The Runbook Automation Framework allows operators to execute automated operational workflows with deterministic pre-checks, dry-run simulation, step-level timeouts, and automatic rollback on failure.

```
┌────────────────────────────────────────────────────────┐
│               Operator / CI Orchestrator               │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│                 Runbook Validator                      │
│   • Precondition check   • Parameter bounds check       │
│   • DAG dependency cycle • Rollback coverage check     │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│                   Runbook Runner                       │
│  [Step 1] Pre-checks & State Capture                   │
│  [Step 2] Stage & Ingestion Buffer                     │
│  [Step 3] On-Chain Transaction Execution               │
│  [Step 4] Health Assertions & Verification             │
│                                                        │
│  * On Failure ──► Step-by-Step Automated Rollback      │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│        Stellar Soroban Contract Audit Trail            │
│   (src/runbook_automation.rs - SHA-256 Digest Record)  │
└────────────────────────────────────────────────────────┘
```

---

## 2. Operational Playbooks

### 1. Contract Pause (`contract-pause`)
- **Use Case**: Emergency circuit-breaker triggered during severe ledger anomaly or security incident.
- **Steps**:
  1. Verify operator governance authorization.
  2. Buffer incoming bridge relayer queues into persistent storage.
  3. Submit `AuditLedger.pause()` on-chain.
  4. Query ledger state across RPC nodes to confirm frozen state.
- **Rollback**: Submits `unpause()` and flushes queued event buffer.

### 2. Storage & Throughput Cap Increase (`cap-increase`)
- **Use Case**: Dynamic capacity expansion when event volume nears current storage quotas.
- **Steps**:
  1. Inspect headroom and assert safety bounds ($< 3\times$ previous cap).
  2. Execute `set_global_max_logs` on Soroban contract.
  3. Scale relayer worker concurrency and update Prometheus alerting thresholds.
  4. Emit synthetic probe event to verify end-to-end ingestion latency.
- **Rollback**: Restores original cap limits and scales down worker pool.

### 3. Schema Update & Version Migration (`schema-update`)
- **Use Case**: Zero-downtime event schema evolution for new compliance fields.
- **Steps**:
  1. Verify forward and backward schema compatibility.
  2. Stage schema and commit SHA-256 schema digest on-chain.
  3. Execute dual-read validation suite across sample transactions.
  4. Promote new schema version as active standard.
- **Rollback**: Deregisters staged schema version and reverts active version pointer.

### 4. Cross-Chain Bridge Failover (`bridge-failover`)
- **Use Case**: Leader-follower failover when primary relayer stalls or becomes partitioned.
- **Steps**:
  1. Detect heartbeat timeout and verify backup relayer sync status.
  2. Pause primary relayer ingestion and acquire distributed fencing token.
  3. Reconcile uncommitted batches and re-sync sequence nonces.
  4. Promote backup relayer to primary on the Soroban contract.
  5. Verify cross-chain Merkle proof attestation on EVM Verifier contract.
- **Rollback**: Restores primary relayer once underlying issue is resolved.

---

## 3. CLI & Automation Usage

```bash
# List all registered runbooks
./scripts/runbook-runner.sh list

# Validate runbook schema and parameters
./scripts/runbook-runner.sh validate contract-pause

# Run dry-run simulation
./scripts/runbook-runner.sh dry-run cap-increase '{"newMaxLogs": 50000}'

# Execute live runbook
./scripts/runbook-runner.sh execute bridge-failover '{"newRelayerAddress": "GBACKUP_RELAYER_KEY"}'
```
