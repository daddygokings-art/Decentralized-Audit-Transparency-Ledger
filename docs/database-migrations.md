# Contract Event Database Migration Guide

This guide describes how to manage contract event database schemas, run migrations, perform rollbacks, validate migrations in dry-run mode, and execute zero-downtime schema deployments.

## Architecture & Features

The `@audit-ledger/db-migrations` package manages the relational schema for contract events, cross-chain verifications, daily aggregates, and dead letter queues.

- **Supported Databases**: PostgreSQL, SQLite, MySQL.
- **Tracking Table**: `_contract_event_migrations` (tracks migration ID, version, SHA-256 checksum, batch number, execution duration, and timestamp).
- **Distributed Locks**: Advisory locks (`pg_advisory_lock` in Postgres, `GET_LOCK` in MySQL) prevent race conditions in distributed deployments.
- **Zero-Downtime Engine**: Lock risk analyzer and Expand-Contract planner.

## CLI Commands

Run the CLI using `node dist/cli.js` or `npm run migrate`:

```bash
# Check migration status and checksum integrity
audit-migrate status

# Apply pending migrations
audit-migrate up

# Apply next N migrations
audit-migrate up --steps 1

# Roll back the latest migration batch
audit-migrate down

# Roll back N steps
audit-migrate down --steps 1

# Dry-run validation (checks lock risks without applying changes)
audit-migrate dry-run

# Generate a 3-phase zero-downtime deployment plan
audit-migrate zero-downtime-plan --table contract_events --old-column metadata --new-column metadata_v2 --type JSONB
```

## Built-in Migrations

1. `001_contract_events_core`: Core table for indexed contract events with compound indexes (`idx_events_contract_seq`, `idx_events_type_ledger`, `idx_events_submitter_created`, `idx_events_hash`).
2. `002_event_partitions_and_verification`: Verification status table and event topics.
3. `003_event_aggregates_and_dead_letter`: Daily stats aggregates and ingestion dead letter queue.
4. `004_zero_downtime_payload_expansion`: Safe backward-compatible schema expansion example.
