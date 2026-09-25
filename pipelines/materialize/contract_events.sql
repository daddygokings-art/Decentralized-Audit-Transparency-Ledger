-- Materialize contract event streaming SQL (issue #433)
--
-- The connection is expected to be provisioned by the deployment environment:
--   CREATE CONNECTION contract_event_kafka TO KAFKA (...);
-- Events use a compact JSON payload. The views below are intentionally plain
-- SQL so they can be queried by a dashboard or another application.

CREATE SOURCE contract_event_source
  FROM KAFKA CONNECTION contract_event_kafka (TOPIC 'contract-events');

CREATE TABLE contract_events (
  event_id text NOT NULL,
  contract_id text NOT NULL,
  event_type text NOT NULL,
  submitter text NOT NULL,
  occurred_at timestamp NOT NULL,
  metadata jsonb NOT NULL,
  event_hash text NOT NULL
) FROM contract_event_source;

-- Optional reference data. A separate catalog/registry pipeline can write here.
CREATE TABLE contract_registry (
  contract_id text NOT NULL,
  contract_name text NOT NULL,
  network text NOT NULL
);

-- One-minute live rollup for dashboards.
CREATE MATERIALIZED VIEW event_counts_by_minute AS
SELECT
  date_trunc('minute', occurred_at) AS window_start,
  event_type,
  count(*) AS event_count
FROM contract_events
GROUP BY window_start, event_type;

-- Distinct submitters per hour and event type.
CREATE MATERIALIZED VIEW submitters_by_hour AS
SELECT
  date_trunc('hour', occurred_at) AS window_start,
  event_type,
  count(DISTINCT submitter) AS unique_submitters,
  count(*) AS event_count
FROM contract_events
GROUP BY window_start, event_type;

-- Join events to the contract catalog for a real-time operational dashboard.
CREATE MATERIALIZED VIEW event_contract_activity AS
SELECT
  e.occurred_at,
  e.event_id,
  e.event_type,
  e.submitter,
  e.event_hash,
  coalesce(c.contract_name, e.contract_id) AS contract_name,
  coalesce(c.network, 'unknown') AS network
FROM contract_events AS e
LEFT JOIN contract_registry AS c USING (contract_id);

-- Query-friendly view for a dashboard's event volume panel.
CREATE VIEW dashboard_event_volume AS
SELECT
  window_start,
  sum(event_count) AS total_events
FROM event_counts_by_minute
GROUP BY window_start
ORDER BY window_start DESC;
