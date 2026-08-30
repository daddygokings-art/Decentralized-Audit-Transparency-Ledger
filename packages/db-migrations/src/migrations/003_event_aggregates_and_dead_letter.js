"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.migration = void 0;
exports.migration = {
    id: '003_event_aggregates_and_dead_letter',
    version: '003',
    name: 'Create daily aggregates and dead letter queue tables',
    phase: 'standard',
    sqlUp: `
    CREATE TABLE IF NOT EXISTS event_daily_stats (
      date VARCHAR(10) NOT NULL,
      contract_id VARCHAR(64) NOT NULL,
      event_type VARCHAR(64) NOT NULL,
      total_events BIGINT NOT NULL DEFAULT 0,
      total_bytes BIGINT NOT NULL DEFAULT 0,
      unique_submitters INT NOT NULL DEFAULT 0,
      updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      PRIMARY KEY (date, contract_id, event_type)
    );

    CREATE TABLE IF NOT EXISTS event_dead_letter_queue (
      id VARCHAR(64) PRIMARY KEY,
      event_hash VARCHAR(66) NOT NULL,
      contract_id VARCHAR(64) NOT NULL,
      payload TEXT NOT NULL,
      error_message TEXT NOT NULL,
      error_stack TEXT,
      retry_count INT NOT NULL DEFAULT 0,
      status VARCHAR(32) NOT NULL DEFAULT 'failed',
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      last_retry_at TIMESTAMPTZ
    );
    CREATE INDEX IF NOT EXISTS idx_dlq_status_retry ON event_dead_letter_queue(status, retry_count);
  `,
    sqlDown: `
    DROP TABLE IF EXISTS event_dead_letter_queue;
    DROP TABLE IF EXISTS event_daily_stats;
  `,
    up: async (adapter) => { },
    down: async (adapter) => { },
};
