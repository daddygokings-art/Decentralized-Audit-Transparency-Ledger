# Contract Event Data Lake: Apache Iceberg & Delta Lake

## Overview

This module introduces scalable, immutable, and performant data lake capabilities for the Decentralized Audit Transparency Ledger using **Apache Iceberg (v2)** and **Delta Lake**.

## Core Capabilities

1. **ACID Transactions & OCC**:
   - Serialized commits guaranteed via Optimistic Concurrency Control (OCC) anchored on the Soroban smart contract (`src/data_lake.rs`).
   - Atomic multi-file writes with rollback protection and manifest lists.

2. **Time Travel**:
   - Query table state at any point in historical time (`AS OF TIMESTAMP`) or specific commit versions (`AS OF VERSION`).
   - Snapshot diffing enables precise audit delta generation between any two checkpoints.

3. **Schema Evolution**:
   - Backward and forward compatibility verification.
   - Column additions, safe type promotions (e.g. `int` -> `long`), and nullable conversions without table rewrites.

4. **Query Engine Federation**:
   - Connectors for Trino, Presto, DuckDB, and Apache Spark.
   - Predicate pushdown and partition pruning on `event_type` and date dimensions.

## API Endpoints

- `GET /api/v1/datalake/snapshots`: Retrieve snapshot history for Iceberg and Delta formats.
- `GET /api/v1/datalake/timetravel`: Resolve historical snapshots given a timestamp or version.
- `GET /api/v1/datalake/schema/history`: Inspect full schema evolution genealogy.
- `POST /api/v1/datalake/schema/evolve`: Propose and register an evolved schema version.
- `GET /api/v1/datalake/health`: Storage health check.
