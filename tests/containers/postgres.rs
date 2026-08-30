//! # PostgreSQL container fixture
//!
//! Provides a [`PostgresHandle`] that wraps a running `postgres:16-alpine`
//! container together with a ready-to-use connection pool.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::containers::postgres::PostgresHandle;
//!
//! let pg = PostgresHandle::start().await.unwrap();
//!
//! // Create a per-test isolated database
//! let db_name = pg.create_database("my_test_db").await.unwrap();
//! let client  = pg.connect(&db_name).await.unwrap();
//!
//! client.execute("INSERT INTO audit_events ...", &[]).await.unwrap();
//! // `pg` (and therefore the container) is dropped at end of scope
//! ```

use std::time::Duration;

use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use tokio_postgres::{Client, NoTls};

use super::{wait_until_ready, CONTAINER_POLL_INTERVAL, CONTAINER_READY_TIMEOUT};

// ── PostgreSQL image constants ─────────────────────────────────────────────────

/// Default superuser credentials (test-only, never production values).
const PG_USER: &str = "postgres";
const PG_PASSWORD: &str = "test_secret";
const PG_DB: &str = "postgres";

// ── Public handle ─────────────────────────────────────────────────────────────

/// A running PostgreSQL container with helpers for test isolation.
///
/// Dropping this struct stops and removes the container.
pub struct PostgresHandle {
    /// The underlying testcontainers container (kept alive by ownership).
    _container: ContainerAsync<Postgres>,
    /// Host-side port mapped to the container's 5432.
    pub host_port: u16,
    /// Connection string for the default `postgres` database.
    pub connection_string: String,
}

impl PostgresHandle {
    // ── Lifecycle ──────────────────────────────────────────────────────────────

    /// Start a fresh PostgreSQL container and wait until it accepts connections.
    ///
    /// # Errors
    /// Returns an error if the container fails to start or is not ready within
    /// [`CONTAINER_READY_TIMEOUT`].
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let image = Postgres::default()
            .with_user(PG_USER)
            .with_password(PG_PASSWORD)
            .with_db_name(PG_DB);

        let container = image.start().await?;
        let host_port = container.get_host_port_ipv4(5432).await?;

        let connection_string = format!(
            "host=localhost port={host_port} user={PG_USER} password={PG_PASSWORD} dbname={PG_DB}"
        );

        let handle = Self {
            _container: container,
            host_port,
            connection_string: connection_string.clone(),
        };

        // Wait until Postgres is ready to accept connections.
        let attempts = (CONTAINER_READY_TIMEOUT.as_millis()
            / CONTAINER_POLL_INTERVAL.as_millis()) as u32;

        let conn_str = connection_string.clone();
        wait_until_ready(attempts, CONTAINER_POLL_INTERVAL, || {
            let cs = conn_str.clone();
            async move {
                tokio_postgres::connect(&cs, NoTls)
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("postgres not ready: {e}"))
            }
        })
        .await
        .map_err(|e| format!("postgres container never became ready: {e}"))?;

        Ok(handle)
    }

    // ── Database isolation ─────────────────────────────────────────────────────

    /// Create a fresh database for one test and return its name.
    ///
    /// The caller should use [`Self::connect`] with the returned name to obtain
    /// an isolated connection.
    pub async fn create_database(
        &self,
        name: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let (client, conn) = tokio_postgres::connect(&self.connection_string, NoTls).await?;
        tokio::spawn(async move { conn.await });

        // Database names cannot be parameterized — sanitize explicitly.
        let safe_name = sanitize_identifier(name);
        client
            .execute(&format!("CREATE DATABASE \"{safe_name}\""), &[])
            .await?;
        Ok(safe_name)
    }

    /// Connect to a named database and return an authenticated client.
    pub async fn connect(
        &self,
        db_name: &str,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        let cs = format!(
            "host=localhost port={} user={PG_USER} password={PG_PASSWORD} dbname={db_name}",
            self.host_port
        );
        let (client, conn) = tokio_postgres::connect(&cs, NoTls).await?;
        // Drive the connection on a background task.
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("[postgres] connection error: {e}");
            }
        });
        Ok(client)
    }

    /// Connect to the default `postgres` database (useful for admin operations).
    pub async fn admin_client(&self) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        self.connect(PG_DB).await
    }

    // ── Schema helpers ─────────────────────────────────────────────────────────

    /// Apply the audit-ledger off-chain schema to `db_name`.
    ///
    /// Creates the `audit_events`, `event_metadata`, and `retention_policies`
    /// tables that the off-chain indexer and API layer use.
    pub async fn apply_schema(
        &self,
        db_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.connect(db_name).await?;

        client
            .batch_execute(AUDIT_LEDGER_SCHEMA)
            .await
            .map_err(Into::into)
    }

    /// Drop a test database.  Call this in test teardown to keep the container
    /// clean when reusing it across multiple tests.
    pub async fn drop_database(
        &self,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.admin_client().await?;
        let safe_name = sanitize_identifier(name);
        // Terminate any remaining connections before dropping.
        client
            .execute(
                "SELECT pg_terminate_backend(pid) \
                 FROM pg_stat_activity \
                 WHERE datname = $1 AND pid <> pg_backend_pid()",
                &[&safe_name],
            )
            .await?;
        client
            .execute(&format!("DROP DATABASE IF EXISTS \"{safe_name}\""), &[])
            .await?;
        Ok(())
    }

    /// Convenience: create a fresh isolated database, apply the full schema, and
    /// return an open client — all in one call.
    pub async fn isolated_db(
        &self,
        name: &str,
    ) -> Result<(String, Client), Box<dyn std::error::Error + Send + Sync>> {
        let db_name = self.create_database(name).await?;
        self.apply_schema(&db_name).await?;
        let client = self.connect(&db_name).await?;
        Ok((db_name, client))
    }
}

// ── Schema DDL ────────────────────────────────────────────────────────────────

/// Off-chain PostgreSQL schema for the Audit Ledger.
const AUDIT_LEDGER_SCHEMA: &str = r#"
-- Core event archive table.
-- Mirrors on-chain events pulled by the metrics-exporter / relayer.
CREATE TABLE IF NOT EXISTS audit_events (
    id              UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_id     VARCHAR(64)     NOT NULL,
    ledger_sequence BIGINT          NOT NULL,
    event_index     INTEGER         NOT NULL,
    event_type      VARCHAR(128)    NOT NULL,
    submitter       VARCHAR(64)     NOT NULL,
    metadata_hash   BYTEA,
    metadata_raw    JSONB,
    tx_hash         BYTEA,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT now(),
    UNIQUE (contract_id, ledger_sequence, event_index)
);

CREATE INDEX IF NOT EXISTS idx_audit_events_contract
    ON audit_events (contract_id, ledger_sequence DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_type
    ON audit_events (event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_submitter
    ON audit_events (submitter, created_at DESC);

-- Extended metadata stored separately to avoid row-size limits.
CREATE TABLE IF NOT EXISTS event_metadata (
    event_id    UUID        NOT NULL REFERENCES audit_events(id) ON DELETE CASCADE,
    key         TEXT        NOT NULL,
    value       JSONB,
    PRIMARY KEY (event_id, key)
);

-- Retention policy configuration persisted off-chain.
CREATE TABLE IF NOT EXISTS retention_policies (
    id              SERIAL      PRIMARY KEY,
    contract_id     VARCHAR(64) NOT NULL UNIQUE,
    ttl_days        INTEGER     NOT NULL DEFAULT 365,
    archive_enabled BOOLEAN     NOT NULL DEFAULT TRUE,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Off-chain relay cursor: tracks the last processed ledger per contract.
CREATE TABLE IF NOT EXISTS relay_cursors (
    contract_id         VARCHAR(64)     PRIMARY KEY,
    last_ledger_seq     BIGINT          NOT NULL DEFAULT 0,
    last_event_index    INTEGER         NOT NULL DEFAULT -1,
    updated_at          TIMESTAMPTZ     NOT NULL DEFAULT now()
);
"#;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Sanitize a string so it can safely be used as a PostgreSQL identifier.
///
/// Only ASCII alphanumeric and underscore characters are allowed; everything
/// else is stripped.  Maximum length is truncated to 63 bytes (Postgres limit).
fn sanitize_identifier(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(63)
        .collect()
}

// ── Startup wait helper exposed to other modules ───────────────────────────────

/// Wait up to `timeout` for a TCP port on localhost to accept a connection.
///
/// Used as a fallback readiness probe when the higher-level client library
/// is not available (e.g. before a schema is applied).
pub async fn wait_for_port(port: u16, timeout: Duration) -> Result<(), String> {
    let interval = Duration::from_millis(200);
    let attempts = (timeout.as_millis() / interval.as_millis()) as u32;
    wait_until_ready(attempts, interval, || async move {
        tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .map(|_| ())
            .map_err(|e| format!("port {port} not open: {e}"))
    })
    .await
}
