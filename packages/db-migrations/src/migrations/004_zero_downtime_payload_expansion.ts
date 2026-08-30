import { MigrationDefinition, DatabaseAdapter } from '../types';

export const migration: MigrationDefinition = {
  id: '004_zero_downtime_payload_expansion',
  version: '004',
  name: 'Zero-downtime metadata and schema expansion',
  phase: 'expand',
  description: 'Adds compressed_payload and schema_version columns backward-compatibly',
  sqlUp: `
    ALTER TABLE contract_events ADD COLUMN compressed_payload TEXT;
    ALTER TABLE contract_events ADD COLUMN schema_version INT DEFAULT 1;
  `,
  sqlDown: `
    ALTER TABLE contract_events DROP COLUMN IF EXISTS schema_version;
    ALTER TABLE contract_events DROP COLUMN IF EXISTS compressed_payload;
  `,
  up: async (adapter: DatabaseAdapter) => {},
  down: async (adapter: DatabaseAdapter) => {},
};
