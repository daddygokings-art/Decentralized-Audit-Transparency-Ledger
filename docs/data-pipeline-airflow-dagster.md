# Contract Event Data Pipeline with Airflow and Dagster (#523)

This module implements production-grade data pipelines for extracting, validating, aggregating, and engineering features from contract events using:
- **Apache Airflow**: Workflow DAG scheduling, custom operators, SLA management, and warehouse loading.
- **Dagster**: Software-Defined Assets (SDA), lineage graphs, automated dependency resolution, and declarative scheduling.
- **Data Quality Engine**: Automated schema conformance, null checks, and Great Expectations suites.
- **Warehouse Loaders**: Multi-warehouse loading for Snowflake, Google BigQuery, ClickHouse, and DuckDB.

---

## Pipeline Architecture

```
Stellar Soroban RPC
        │
        ▼
┌───────────────────────────────────────┐
│   AuditLedgerExtractOperator (DAG)    │
└───────────────────┬───────────────────┘
                    │
                    ▼
┌───────────────────────────────────────┐
│       DataQualityCheckOperator        │ ── (Quality Metrics) ──► On-Chain Attestation
└───────────────────┬───────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌──────────────┐        ┌──────────────┐
│ Daily Rollup │        │ ML Feature   │
│ Aggregations │        │ Engineering  │
└───────┬──────┘        └───────┬──────┘
        │                       │
        ▼                       ▼
┌───────────────────────────────────────┐
│  Warehouse Loader (Snowflake/BigQuery)│
└───────────────────────────────────────┘
```

---

## Soroban On-Chain Attestation (`src/pipeline_attestation.rs`)

- `record_pipeline_run`: Records execution start/end timestamps, processed record volume, and status.
- `attest_data_quality`: Publishes cryptographic proofs of dataset quality validation passes.
- `update_warehouse_checkpoint`: Sets watermark ledger sequences for sync progress tracking.
