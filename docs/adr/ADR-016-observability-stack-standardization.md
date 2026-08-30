# ADR-016: Contract Event Observability Stack Standardization

## Status
Accepted

## Context
As the Decentralized Audit Transparency Ledger expands with high-volume contract event ingestion, cross-chain bridging to EVM networks, and multi-database persistence, we require a standardized, end-to-end observability architecture. Operators and engineers need unified tracing, metrics, logging, alerting rules, and incident runbooks to maintain high availability and meet SLAs.

## Decision
We standardized the observability stack around the open-standard CNCF observability ecosystem:
1. **OpenTelemetry SDK (`@audit-ledger/observability`)**: Standardized tracer and meter providers with W3C `traceparent` context propagation, error-priority sampling (100% trace capture on errors), and semantic attributes (`audit.contract_id`, `audit.event_type`, `audit.event_hash`, `bridge.target_chain`).
2. **Prometheus RED & USE Metrics**: Standardized Rate, Errors, and Duration metrics (`audit_event_ingestion_total`, `audit_errors_total`, `audit_event_ingestion_duration_seconds`, `audit_event_verification_total`, `audit_db_query_duration_seconds`).
3. **Loki Structured JSON Logging**: Structured JSON logger formatting with automatic trace correlation (`trace_id`, `span_id`), severity levels, and automated PII/key redaction.
4. **Tempo Distributed Tracing**: Distributed trace ingestion with TraceQL support, trace-to-logs cross-datasource linking, and exemplar trace visualization in Grafana.
5. **Grafana Unified Dashboards**: Pre-provisioned dashboards linking RED metrics, live Loki logs, and Tempo distributed traces.
6. **Prometheus Alerting Rules & Runbook Integration**: Formal alert definitions with direct links to actionable step-by-step incident remediation runbooks in `docs/runbooks/`.

## Consequences
- Every contract event, API request, and bridge transaction can be traced end-to-end with correlated logs and metrics.
- Alerts provide instant root-cause diagnostics and actionable runbook links, reducing MTTR (Mean Time to Resolution).
