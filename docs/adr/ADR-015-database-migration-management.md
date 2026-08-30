# ADR-015: Contract Event Database Migration Management

## Status
Accepted

## Context
The Decentralized Audit Transparency Ledger indexes high-throughput on-chain contract events from Soroban / Stellar and cross-chain verification records from EVM networks. As the platform evolves, the database schema requires versioned migrations, transactional rollbacks, dry-run safety validation, and zero-downtime schema evolution (expand/contract pattern) without locking tables during production deployments. Multi-database support is required across PostgreSQL, SQLite (for local embedded development/testing), and MySQL.

## Decision
We implemented `@audit-ledger/db-migrations`, a database migration framework supporting:
1. **Versioned Migrations with Checksum Verification**: Every migration includes sequential versioning, up/down scripts, and SHA-256 content checksums stored in `_contract_event_migrations`. Checksums prevent undetected schema drift or post-deployment tampering.
2. **Atomic Transactional Execution & Stepwise Rollbacks**: Migrations run inside isolated transactions with automatic rollback on error. Non-transactional flags are provided for specific operations (e.g. `CREATE INDEX CONCURRENTLY` in PostgreSQL). Rollbacks can be executed by step count or to a specific target version.
3. **Distributed Advisory Locking**: Prevents concurrent migration execution across multiple container replicas using PostgreSQL advisory locks (`pg_advisory_lock`), MySQL table locks (`GET_LOCK`), and SQLite file locks.
4. **Dry-Run Validation & Lock Risk Analysis**: Static AST and regex analysis detects dangerous table locks (e.g. `ALTER TABLE ADD COLUMN NOT NULL` without `DEFAULT`, non-concurrent index creation, direct column drops).
5. **Zero-Downtime Deployment Engine (Expand/Contract)**: Provides automated scaffolding for 3-phase schema evolution:
   - *Phase 1 (Expand)*: Add new columns/tables/views with dual-write synchronization triggers.
   - *Phase 2 (Backfill)*: Batched, throttled historical data backfill with cursor pagination to avoid lock contention.
   - *Phase 3 (Contract)*: Safe removal of deprecated columns after all services have transitioned.

## Consequences
- Database changes are fully version-controlled, auditable, and idempotent.
- Production deployments can proceed with zero downtime.
- Multi-database adapters allow running unit/integration tests with in-memory SQLite and production with PostgreSQL.
