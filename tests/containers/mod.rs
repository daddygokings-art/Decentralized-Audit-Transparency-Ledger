//! # Container lifecycle management for integration tests
//!
//! This module provides shared infrastructure for spinning up real Docker
//! containers in tests, keeping them alive for the duration of a test, and
//! tearing them down cleanly afterwards.
//!
//! ## Design choices
//!
//! * Each container type lives in its own sub-module so that test authors only
//!   pull in the dependencies they actually need.
//! * The [`ContainerHandle`] wrapper holds a running container together with any
//!   derived connection information (URL, port, …) so it can be passed around as
//!   a single value.
//! * Containers are started lazily via [`tokio::sync::OnceCell`] singletons when
//!   tests want to share one container across the whole process (e.g. schema
//!   creation is expensive).  Each test can alternatively request its own
//!   per-test container for full isolation.
//! * A global timeout guard prevents CI from hanging if Docker is unavailable.

pub mod kafka;
pub mod postgres;
pub mod redis;
pub mod soroban;

use std::time::Duration;

// ── Re-exports for convenience ─────────────────────────────────────────────────

pub use kafka::KafkaHandle;
pub use postgres::PostgresHandle;
pub use redis::RedisHandle;
pub use soroban::SorobanHandle;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum time to wait for any container to become ready.
pub const CONTAINER_READY_TIMEOUT: Duration = Duration::from_secs(120);

/// How long to sleep between readiness-poll attempts.
pub const CONTAINER_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Default Docker network used by all test containers (created on first use).
#[allow(dead_code)]
pub const TEST_NETWORK: &str = "audit_ledger_test";

// ── Parallel execution helpers ─────────────────────────────────────────────────

/// Run a collection of async closures concurrently and collect results.
///
/// Any single failure causes the whole batch to fail.
///
/// # Example
/// ```rust,ignore
/// run_parallel(vec![
///     Box::pin(async { test_postgres_write().await }),
///     Box::pin(async { test_redis_set().await }),
/// ]).await.expect("parallel tests failed");
/// ```
pub async fn run_parallel<T, E>(
    futs: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>>,
) -> Result<Vec<T>, E>
where
    T: Send + 'static,
    E: Send + std::fmt::Debug + 'static,
{
    use futures::future::try_join_all;
    try_join_all(futs).await
}

// ── Environment helpers ────────────────────────────────────────────────────────

/// Returns `true` if Docker is available and tests should run.
///
/// Skips (returns `false`) in environments where Docker is not present so that
/// unit-test-only CI steps do not fail.
pub fn docker_available() -> bool {
    std::process::Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Macro that skips the test when Docker is unavailable.
///
/// ```rust,ignore
/// #[tokio::test]
/// async fn test_with_postgres() {
///     require_docker!();
///     // ... test body
/// }
/// ```
#[macro_export]
macro_rules! require_docker {
    () => {
        if !$crate::containers::docker_available() {
            eprintln!("Skipping test: Docker not available in this environment");
            return;
        }
    };
}

// ── Retry helper ──────────────────────────────────────────────────────────────

/// Poll `check` up to `max_attempts` times with `interval` between each try.
///
/// Returns `Ok(())` as soon as `check` succeeds, or `Err` with the last error
/// after all attempts are exhausted.
pub async fn wait_until_ready<F, Fut, E>(
    max_attempts: u32,
    interval: Duration,
    mut check: F,
) -> Result<(), E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
    E: std::fmt::Debug,
{
    let mut last_err = None;
    for attempt in 0..max_attempts {
        match check().await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing_or_eprintln(attempt, max_attempts, &e);
                last_err = Some(e);
                tokio::time::sleep(interval).await;
            }
        }
    }
    Err(last_err.unwrap())
}

fn tracing_or_eprintln<E: std::fmt::Debug>(attempt: u32, max: u32, err: &E) {
    eprintln!(
        "[containers] readiness check attempt {}/{} failed: {:?}",
        attempt + 1,
        max,
        err
    );
}

// ── Test isolation helpers ─────────────────────────────────────────────────────

/// Generate a unique name prefix for per-test resources (databases, topics, …).
///
/// Uses the thread name when available, falling back to a UUID.
pub fn unique_name(prefix: &str) -> String {
    let id = uuid::Uuid::new_v4()
        .to_string()
        .replace('-', "")
        .chars()
        .take(8)
        .collect::<String>();
    format!("{prefix}_{id}")
}
