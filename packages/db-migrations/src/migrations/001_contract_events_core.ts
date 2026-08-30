import { MigrationDefinition, DatabaseAdapter } from '../types';

export const migration: MigrationDefinition = {
  id: '001_contract_events_core',
  version: '001',
  name: 'Create contract events core table and indexes',
  phase: 'standard',
  sqlUp: `
    CREATE TABLE IF NOT EXISTS contract_events (
      id VARCHAR(64) PRIMARY KEY,
      contract_id VARCHAR(64) NOT NULL,
      sequence_num BIGINT NOT NULL,
      ledger_seq BIGINT NOT NULL,
      tx_hash VARCHAR(66) NOT NULL,
      event_type VARCHAR(64) NOT NULL,
      submitter VARCHAR(64) NOT NULL,
      metadata TEXT NOT NULL,
      event_hash VARCHAR(66) NOT NULL,
      signature TEXT,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      status VARCHAR(32) NOT NULL DEFAULT 'indexed'
    );
    CREATE INDEX IF NOT EXISTS idx_events_contract_seq ON contract_events(contract_id, sequence_num);
    CREATE INDEX IF NOT EXISTS idx_events_type_ledger ON contract_events(event_type, ledger_seq);
    CREATE INDEX IF NOT EXISTS idx_events_submitter_created ON contract_events(submitter, created_at);
    CREATE INDEX IF NOT EXISTS idx_events_hash ON contract_events(event_hash);
  `,
  sqlDown: `
    DROP TABLE IF EXISTS contract_events;
  `,
  up: async (adapter: DatabaseAdapter) => {
    // Up hook
  },
  down: async (adapter: DatabaseAdapter) => {
    // Down hook
  },
};
