# ADR-017: Contract Event Business Metrics and KPIs Engine

## Status
Accepted

## Context
Understanding adoption, ecosystem health, decentralized participation, event volume trajectories, cross-chain bridge utilization, and developer API traction requires automated business metric calculation, statistical anomaly detection, and executive reporting.

## Decision
We implemented `@audit-ledger/business-metrics`, an automated KPI calculation and reporting engine featuring:
1. **Submitter Dynamics & Decentralization Tracking**: Computes DAU, WAU, MAU, retention cohorts, and the Gini coefficient to monitor decentralization and prevent monopoly submitter concentration.
2. **Growth Engine & Z-Score Anomaly Detection**: Tracks multi-period growth (DoD, WoW, MoM) and triggers anomaly alerts when submission volume deviates $|Z| \ge 2.5$ standard deviations from moving baseline.
3. **Cross-Chain Bridge & Governance Telemetry**: Tracks bridged USD volume, verification latencies, proof cache hit ratios, voter turnout, and dispute resolution performance.
4. **API Developer Adoption SLAs**: Tracks developer tokens, protocol splits (REST vs GraphQL vs WS), tier utilization, and p95 latency compliance.
5. **Executive Dashboards & Reporting**: Pre-provisioned Grafana executive dashboards, markdown briefing generators, and Prometheus exposition.

## Consequences
- Executive leadership and ecosystem stakeholders have instant transparency into platform growth and adoption.
- Automatic anomaly detection pinpoints unexpected traffic changes early.
