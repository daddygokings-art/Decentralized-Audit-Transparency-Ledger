import { MigrationDefinition, DatabaseAdapter } from '../types';

export const migration: MigrationDefinition = {
  id: '005_dispute_votes_relational_table',
  version: '005',
  name: 'Create dedicated relational table and indexes for dispute votes aggregation',
  phase: 'standard',
  description: 'Enables high-performance relational querying, filtering, and aggregation of DAO dispute votes',
  sqlUp: `
    CREATE TABLE IF NOT EXISTS dispute_votes (
      id VARCHAR(64) PRIMARY KEY,
      dispute_id BIGINT NOT NULL,
      juror VARCHAR(64) NOT NULL,
      vote VARCHAR(32) NOT NULL,
      stake BIGINT NOT NULL DEFAULT 0,
      reward BIGINT NOT NULL DEFAULT 0,
      tx_hash VARCHAR(66),
      signature TEXT,
      rationale TEXT,
      voted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS idx_dispute_votes_dispute_id ON dispute_votes(dispute_id);
    CREATE INDEX IF NOT EXISTS idx_dispute_votes_juror ON dispute_votes(juror);
    CREATE INDEX IF NOT EXISTS idx_dispute_votes_dispute_vote ON dispute_votes(dispute_id, vote);
    CREATE UNIQUE INDEX IF NOT EXISTS idx_dispute_votes_unique_dispute_juror ON dispute_votes(dispute_id, juror);
    CREATE INDEX IF NOT EXISTS idx_dispute_votes_voted_at ON dispute_votes(voted_at);

    CREATE OR REPLACE VIEW dispute_votes_aggregation AS
    SELECT
      dispute_id,
      vote,
      COUNT(*) AS total_votes,
      SUM(stake) AS total_stake,
      SUM(reward) AS total_reward,
      MIN(voted_at) AS first_vote_at,
      MAX(voted_at) AS last_vote_at
    FROM dispute_votes
    GROUP BY dispute_id, vote;
  `,
  sqlDown: `
    DROP VIEW IF EXISTS dispute_votes_aggregation;
    DROP TABLE IF EXISTS dispute_votes;
  `,
  up: async (adapter: DatabaseAdapter) => {},
  down: async (adapter: DatabaseAdapter) => {},
};
