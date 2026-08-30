"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.migration = void 0;
exports.migration = {
    id: '002_event_partitions_and_verification',
    version: '002',
    name: 'Create cross-chain verification store and topics tables',
    phase: 'standard',
    sqlUp: `
    CREATE TABLE IF NOT EXISTS event_verifications (
      id VARCHAR(64) PRIMARY KEY,
      event_id VARCHAR(64) NOT NULL,
      event_hash VARCHAR(66) NOT NULL,
      target_chain VARCHAR(32) NOT NULL,
      verifier_address VARCHAR(64) NOT NULL,
      status VARCHAR(32) NOT NULL DEFAULT 'pending',
      proof_data TEXT,
      relay_tx_hash VARCHAR(66),
      verified_at TIMESTAMPTZ,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
    CREATE INDEX IF NOT EXISTS idx_verif_event_hash ON event_verifications(event_hash);
    CREATE INDEX IF NOT EXISTS idx_verif_chain_status ON event_verifications(target_chain, status);

    CREATE TABLE IF NOT EXISTS event_topics (
      id VARCHAR(64) PRIMARY KEY,
      event_id VARCHAR(64) NOT NULL,
      topic VARCHAR(128) NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
    CREATE INDEX IF NOT EXISTS idx_topics_topic ON event_topics(topic);
  `,
    sqlDown: `
    DROP TABLE IF EXISTS event_topics;
    DROP TABLE IF EXISTS event_verifications;
  `,
    up: async (adapter) => { },
    down: async (adapter) => { },
};
