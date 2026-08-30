//! # Redis container fixture
//!
//! Provides a [`RedisHandle`] that wraps a running `redis:7-alpine` container
//! together with helpers for:
//!
//! * Key-value caching (event metadata cache, contract state snapshots)
//! * Session store operations
//! * Pub/Sub channels (used by the notifier service)
//! * Per-test database isolation via Redis DB indices 0-15

use std::time::Duration;

use redis::{aio::ConnectionManager, AsyncCommands, Client};
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::redis::Redis;

use super::{wait_until_ready, CONTAINER_POLL_INTERVAL, CONTAINER_READY_TIMEOUT};

// ── Image constants ────────────────────────────────────────────────────────────

const REDIS_PORT: u16 = 6379;

// ── Public handle ─────────────────────────────────────────────────────────────

/// A running Redis container.
///
/// Dropping this struct stops and removes the container.
pub struct RedisHandle {
    /// Underlying container (kept alive by ownership).
    _container: ContainerAsync<Redis>,
    /// Host-side port mapped to the container's 6379.
    pub host_port: u16,
    /// Base URL without database suffix: `redis://127.0.0.1:<port>`.
    pub url: String,
}

impl RedisHandle {
    // ── Lifecycle ──────────────────────────────────────────────────────────────

    /// Start a Redis container and wait until it is ready.
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let container = Redis::default().start().await?;
        let host_port = container.get_host_port_ipv4(REDIS_PORT).await?;
        let url = format!("redis://127.0.0.1:{host_port}");

        let handle = Self {
            _container: container,
            host_port,
            url: url.clone(),
        };

        // Wait until PING succeeds.
        let attempts = (CONTAINER_READY_TIMEOUT.as_millis()
            / CONTAINER_POLL_INTERVAL.as_millis()) as u32;

        wait_until_ready(attempts, CONTAINER_POLL_INTERVAL, || {
            let u = url.clone();
            async move {
                let client = Client::open(u).map_err(|e| format!("redis open: {e}"))?;
                let mut conn = client
                    .get_async_connection()
                    .await
                    .map_err(|e| format!("redis connect: {e}"))?;
                redis::cmd("PING")
                    .query_async::<String>(&mut conn)
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("redis ping: {e}"))
            }
        })
        .await
        .map_err(|e| format!("redis never became ready: {e}"))?;

        Ok(handle)
    }

    // ── Connection helpers ─────────────────────────────────────────────────────

    /// Return a [`Client`] for the given Redis database index (0-15).
    pub fn client(&self, db: u8) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{}", self.url, db);
        Client::open(url).map_err(Into::into)
    }

    /// Return a multiplexed async connection manager (cheaply cloneable).
    pub async fn connection_manager(
        &self,
        db: u8,
    ) -> Result<ConnectionManager, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{}", self.url, db);
        let client = Client::open(url)?;
        ConnectionManager::new(client).await.map_err(Into::into)
    }

    // ── Test-isolation helpers ─────────────────────────────────────────────────

    /// Flush a specific database index to give each test a clean slate.
    pub async fn flush_db(&self, db: u8) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(db)?;
        let mut conn = client.get_async_connection().await?;
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await
            .map_err(Into::into)
    }

    /// Flush all databases.  Use in global teardown, not per-test teardown.
    pub async fn flush_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(0)?;
        let mut conn = client.get_async_connection().await?;
        redis::cmd("FLUSHALL")
            .query_async::<()>(&mut conn)
            .await
            .map_err(Into::into)
    }

    // ── Cache helpers ──────────────────────────────────────────────────────────

    /// Store an audit event in the cache with an optional TTL.
    ///
    /// Key format: `event:<contract_id>:<ledger_seq>:<index>`
    pub async fn cache_event(
        &self,
        db: u8,
        contract_id: &str,
        ledger_seq: u64,
        index: u32,
        payload: &str,
        ttl_secs: Option<u64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(db).await?;
        let key = format!("event:{contract_id}:{ledger_seq}:{index}");
        if let Some(ttl) = ttl_secs {
            cm.set_ex::<_, _, ()>(&key, payload, ttl).await?;
        } else {
            cm.set::<_, _, ()>(&key, payload).await?;
        }
        Ok(())
    }

    /// Retrieve a cached event.
    pub async fn get_cached_event(
        &self,
        db: u8,
        contract_id: &str,
        ledger_seq: u64,
        index: u32,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(db).await?;
        let key = format!("event:{contract_id}:{ledger_seq}:{index}");
        cm.get::<_, Option<String>>(&key)
            .await
            .map_err(Into::into)
    }

    // ── Session store helpers ──────────────────────────────────────────────────

    /// Store a user session token with an expiry.
    pub async fn store_session(
        &self,
        db: u8,
        session_id: &str,
        user_data: &str,
        ttl_secs: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(db).await?;
        let key = format!("session:{session_id}");
        cm.set_ex::<_, _, ()>(&key, user_data, ttl_secs)
            .await
            .map_err(Into::into)
    }

    /// Look up a session.
    pub async fn get_session(
        &self,
        db: u8,
        session_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(db).await?;
        let key = format!("session:{session_id}");
        cm.get::<_, Option<String>>(&key)
            .await
            .map_err(Into::into)
    }

    // ── Pub/Sub helpers ────────────────────────────────────────────────────────

    /// Publish a message to a channel.
    ///
    /// Returns the number of subscribers that received the message.
    pub async fn publish(
        &self,
        channel: &str,
        message: &str,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(0).await?;
        cm.publish::<_, _, i64>(channel, message)
            .await
            .map_err(Into::into)
    }

    // ── Rate-limiter helpers ───────────────────────────────────────────────────

    /// Increment a rate-limit counter and return the new value.
    ///
    /// Sets the TTL on first increment so the window expires automatically.
    /// Uses a dedicated async connection (not the multiplexed manager) because
    /// `redis::pipe::query_async` requires exclusive mutable access to a
    /// `ConnectionLike` implementor.
    pub async fn rate_limit_incr(
        &self,
        db: u8,
        key: &str,
        window_secs: u64,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(db)?;
        let mut conn = client.get_async_connection().await?;
        // MULTI/EXEC pipe: INCR key + EXPIRE key <window> (EXPIRE result ignored)
        let results: Vec<i64> = redis::pipe()
            .atomic()
            .incr(key, 1i64)
            .expire(key, window_secs as i64)
            .ignore()
            .query_async(&mut conn)
            .await?;
        Ok(results.into_iter().next().unwrap_or(0))
    }

    // ── Distributed lock helpers ───────────────────────────────────────────────

    /// Acquire a simple distributed lock using SET NX EX.
    ///
    /// Returns `true` if the lock was acquired.
    pub async fn try_lock(
        &self,
        db: u8,
        lock_key: &str,
        owner_id: &str,
        ttl_secs: u64,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut cm = self.connection_manager(db).await?;
        let key = format!("lock:{lock_key}");
        // SET key value NX EX ttl
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(owner_id)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut cm)
            .await?;
        Ok(result.is_some())
    }

    /// Release a lock (only if still owned by `owner_id`).
    pub async fn release_lock(
        &self,
        db: u8,
        lock_key: &str,
        owner_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(db)?;
        let mut conn = client.get_async_connection().await?;
        let key = format!("lock:{lock_key}");
        // Lua script: only delete if value matches owner_id
        let script = redis::Script::new(
            r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
            "#,
        );
        let deleted: i64 = script
            .key(&key)
            .arg(owner_id)
            .invoke_async(&mut conn)
            .await?;
        Ok(deleted > 0)
    }

    /// Ping the server to verify it is alive.
    pub async fn ping(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(0)?;
        let mut conn = client.get_async_connection().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Return the number of keys in the given database.
    pub async fn dbsize(&self, db: u8) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client(db)?;
        let mut conn = client.get_async_connection().await?;
        redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(Into::into)
    }
}
