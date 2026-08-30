# ADR 017: Contract Event Synthetic Monitoring and SLA Tracking Architecture

## Status
Accepted

## Context
The Decentralized Audit Ledger serves enterprise auditing, regulatory reporting (SupTech), stablecoin reserve validation, and compliance-critical workloads. Degradation in RPC nodes, transaction submission congestion, or indexing lag directly impact enterprise SLAs. Passive metrics alone cannot detect silent failures or end-to-end journey degradation. An active synthetic monitoring engine with on-chain and off-chain SLA tracking is required.

## Decision
1. **On-Chain Synthetic Registry & SLA Engine (`src/synthetic_monitoring.rs`)**:
   - Store probe configurations, telemetry executions, sliding-window SLA evaluations, and incident tracking on-chain.
   - Support distinct journey types: `EventSubmission`, `EventQuery`, `GovernanceOperations`, `TokenGatingVerify`, and `ApiHealthCheck`.
   - Track uptime in basis points (bps) and P95 latency thresholds over configurable evaluation windows (e.g. 24h).
2. **Prometheus & Grafana Observability Layer**:
   - Add Prometheus alert rules (`monitoring/prometheus/synthetic_alerts.yml`) for latency spikes, consecutive failures, and SLA breaches.
   - Provision Grafana dashboard (`monitoring/grafana/dashboards/synthetic-monitoring.json`) displaying live uptime gauges, SLA compliance, and latency percentiles.
3. **Automated Prober Service (`scripts/synthetic-monitoring/`)**:
   - Python-based synthetic runner executing simulated transactions and queries at regular intervals, recording telemetry both locally and on-chain.

## Consequences
### Positive
- Sub-minute detection of production outages before users report them.
- Cryptographically verifiable SLA compliance reports for institutional clients.
- Automated incident opening and resolution tracking.

### Tradeoffs
- Generating synthetic transactions consumes minimal testnet/network fee resources.
- Storage bounding required on-chain (implemented via ring buffer of recent executions).
