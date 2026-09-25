# Materialize Contract Event Streaming

This directory contains the dependency-free SQL layer for issue #433. It
consumes contract events from the `contract-events` Kafka topic and maintains
live Materialize relations for dashboards and alerting.

## Deploy

Provision the `contract_event_kafka` connection in the target Materialize
environment, then apply the SQL file with the normal Materialize CLI or
PostgreSQL-compatible client:

```bash
materialize sql-client --host "$MATERIALIZE_HOST" \
  --port "$MATERIALIZE_PORT" --username "$MATERIALIZE_USER" \
  --password "$MATERIALIZE_PASSWORD" --database "$MATERIALIZE_DATABASE" \
  -f pipelines/materialize/contract_events.sql
```

The `contract_registry` table is optional reference data. A catalog service
can insert contract names and networks into it; events remain visible when a
registry row is missing.

## Relations

- `event_counts_by_minute`: live event counts grouped by type and minute.
- `submitters_by_hour`: live distinct-submitter and event totals.
- `event_contract_activity`: event/catalog join for operational dashboards.
- `dashboard_event_volume`: a small query target for a real-time volume panel.

The definitions use only Materialize SQL and do not add a runtime dependency
to the SDK or contract.
