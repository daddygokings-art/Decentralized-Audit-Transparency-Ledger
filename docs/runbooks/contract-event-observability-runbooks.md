# Contract Event Observability Incident Runbooks

This document contains step-by-step diagnostic and remediation runbooks for alerts triggered by the contract event observability stack.

---

## 1. `ContractEventIngestionLagHigh`
- **Severity**: Critical
- **Trigger**: `audit_ingestion_lag_ledgers > 20` for > 3 minutes.
- **Description**: The event ingestion service is falling behind the Stellar/Soroban network ledger tip.

### Diagnostics
```bash
# 1. Check ingestion service logs for RPC connection timeouts
docker logs -n 100 audit-ledger-metrics-exporter | grep -E "ERROR|timeout|ECONNREFUSED"

# 2. Check Soroban RPC endpoint response latency
curl -w "@scripts/curl-format.txt" -o /dev/null -s -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
  https://soroban-testnet.stellar.org

# 3. Check database write latency in Grafana
# Visit: http://grafana:3000/d/audit-ledger-unified-observability
```

### Remediation
1. **RPC Rate Limiting**: If RPC returns 429 / timeouts, switch to fallback RPC endpoint via `STELLAR_RPC` environment variable.
2. **Database Contention**: Check if long-running migrations or lock contentions are holding locks on `contract_events`. Run `SELECT pid, query, age(clock_timestamp(), query_start) FROM pg_stat_activity WHERE state != 'idle';`.
3. **Restart Ingestion Worker**: If worker is stuck in an unhandled event loop: `docker restart audit-ledger-metrics-exporter`.

---

## 2. `ContractEventProcessingErrorRateSpike`
- **Severity**: Critical
- **Trigger**: `sum(rate(audit_errors_total[5m])) / sum(rate(audit_event_ingestion_total[5m])) > 0.05`.
- **Description**: More than 5% of incoming events are failing schema validation, hashing, or database insertion.

### Diagnostics
```bash
# 1. Query Loki for error logs containing trace IDs
# LogQL: {service=~"audit-.+"} |= "error" | json

# 2. Check error breakdown in Prometheus
# Expression: sum(rate(audit_errors_total[5m])) by (component, error_type)
```

### Remediation
1. Inspect the most recent error trace in Tempo using the `trace_id` extracted from Loki error logs.
2. If invalid event schema was emitted from contract, verify whether a new contract version was deployed without database schema migration.
3. If database schema mismatch, apply pending migrations with `npm run migrate up`.

---

## 3. `BridgeRelayerVerificationFailure`
- **Severity**: Critical
- **Trigger**: `rate(audit_event_verification_total{status="failed"}[5m]) > 0.02`.
- **Description**: EVM Verifier contract rejected inclusion proofs or transaction reverted.

### Diagnostics
```bash
# Check relayer error recovery logs and dead letter queue
docker logs audit-ledger-relayer | grep -E "exhausted retries|dead letter queue"
```

### Remediation
1. Check EVM target chain gas price and relayer account balance.
2. Verify that the relayer's signing key matches the authorized verifier address configured on the EVM `Verifier.sol` contract.
3. Reprocess dead letter queue events once gas/key issue is resolved.

---

## 4. `DatabaseMigrationLockTimeout`
- **Severity**: Warning
- **Trigger**: `rate(audit_errors_total{error_type="migration_lock_timeout"}[5m]) > 0`.
- **Description**: A deployment migration worker failed to acquire advisory lock due to another active process holding the lock.

### Diagnostics
```bash
# In PostgreSQL, check active advisory locks
SELECT pid, locktype, mode, granted FROM pg_locks WHERE locktype = 'advisory';
```

### Remediation
1. Identify the worker holding the lock. If stale/orphaned from a crashed container, terminate connection: `SELECT pg_terminate_backend(<pid>);`.
2. Re-run migration validation: `audit-migrate validate`.

---

## 5. `EventDeadLetterQueueGrowing`
- **Severity**: Warning
- **Trigger**: `audit_dead_letter_queue_size > 50` for > 5 minutes.
- **Description**: Failed events accumulating in `event_dead_letter_queue`.

### Diagnostics
```bash
# Inspect dead letter queue records
SELECT id, contract_id, error_message, retry_count, created_at FROM event_dead_letter_queue ORDER BY created_at DESC LIMIT 10;
```

### Remediation
1. Identify root cause of failure (malformed payload, signature mismatch, temporary RPC outage).
2. Trigger batch DLQ replay worker after fix is deployed.
