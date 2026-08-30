# Real-Time Contract Event Analytics with ClickHouse & Apache Druid

## Overview

This module implements real-time analytical event processing, sub-second query execution, pre-aggregated rollups, and visualization tool integration for the Decentralized Audit Transparency Ledger.

## Architecture

```
                                  ┌───────────────────────────────┐
                                  │   Grafana / Superset / BI     │
                                  └───────────────▲───────────────┘
                                                  │ (Sub-second SQL)
┌─────────────────┐       ┌───────────────────────┴───────────────────────┐
│ Soroban Ledger  │       │       Real-Time Analytics Engine              │
│ (Audit Events)  ├───►───┤   (bridge/analytics/realtime-engine.ts)       │
└─────────────────┘       └───────┬───────────────────────────────┬───────┘
                                  │                               │
                       (Micro-batch Ingestion)          (Supervisor Stream)
                                  ▼                               ▼
                      ┌───────────────────────┐       ┌───────────────────────┐
                      │      ClickHouse       │       │     Apache Druid      │
                      │  - ReplacingMergeTree │       │  - Native Timeseries  │
                      │  - AggregatingRollups │       │  - HyperUnique HLL    │
                      └───────────────────────┘       └───────────────────────┘
```

## Features

1. **Sub-Second Querying**:
   - Sub-second analytical queries for real-time throughput (TPS), latency percentiles (P50, P95, P99), and submitter frequency.
   - Column-oriented storage with `ReplacingMergeTree` table engine.

2. **Continuous Rollups**:
   - Hourly and daily pre-aggregated materialized views (`audit_events_hourly_rollup`).
   - HyperLogLog distinct submitter approximations with `uniqState`/`uniqMerge`.

3. **Apache Druid Integration**:
   - Kafka and streaming ingestion supervisor specifications.
   - Native JSON timeseries, topN, and groupBy queries.

4. **Visualization Tool Connectors**:
   - Pre-built Grafana dashboard at `monitoring/grafana/dashboards/realtime-analytics.json`.
   - REST analytics API server providing `/api/v1/analytics/realtime/summary` and `/api/v1/analytics/query`.

## API Endpoints

- `GET /api/v1/analytics/realtime/summary`: Get live TPS, latency percentiles, and event breakdown.
- `GET /api/v1/analytics/rollup`: Query time-bucketed rollups.
- `POST /api/v1/analytics/query`: Execute sub-second analytical SQL queries.
- `POST /api/v1/analytics/visualization/grafana`: Query datasource adapter for Grafana.
- `GET /api/v1/analytics/health`: Analytics sink health check.
