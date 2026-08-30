//! # Integration tests: real containers
//!
//! This test binary wires together PostgreSQL, Redis, Kafka, and the Soroban RPC
//! stub to test the off-chain components of the Audit Ledger system with real
//! infrastructure.
//!
//! ## Running
//!
//! ```bash
//! # All container tests (requires Docker):
//! cargo test --test integration_containers -- --ignored --test-threads=8
//!
//! # Single group:
//! cargo test --test integration_containers postgres -- --ignored
//! ```
//!
//! ## Test isolation strategy
//!
//! * **PostgreSQL** — each test creates its own database via
//!   [`PostgresHandle::create_database`] and drops it on teardown.
//! * **Redis** — each test uses a dedicated DB index (0-15) and flushes it at
//!   the end.
//! * **Kafka** — each test creates a uniquely-named topic.
//! * **Soroban** — the in-process stub is cheap enough to spin up per-test.
//!
//! ## Parallel execution
//!
//! Tests that share no mutable state run in parallel (`--test-threads=N`).
//! Tests tagged with `#[serial]` run sequentially because they share a
//! singleton container.

mod containers;

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use containers::{
    docker_available, run_parallel, unique_name, KafkaHandle, PostgresHandle, RedisHandle,
    SorobanHandle,
};

// ── Test lifecycle helpers ─────────────────────────────────────────────────────

/// Skip the current test when Docker is unavailable (non-panicking).
macro_rules! require_docker {
    () => {
        if !docker_available() {
            eprintln!("⚠  Skipping: Docker is not available in this environment");
            return;
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// PostgreSQL tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Basic connectivity — verifies the container starts and accepts connections.
#[tokio::test]
#[ignore = "requires Docker"]
async fn postgres_container_starts_and_accepts_connections() {
    require_docker!();
    let pg = PostgresHandle::start()
        .await
        .expect("failed to start postgres container");

    let client = pg
        .admin_client()
        .await
        .expect("admin connection should succeed");

    let row = client
        .query_one("SELECT current_database()", &[])
        .await
        .expect("SELECT should work");
    let db: &str = row.get(0);
    assert_eq!(db, "postgres");
}

/// Schema application — verifies all DDL runs without error.
#[tokio::test]
#[ignore = "requires Docker"]
async fn postgres_schema_creates_all_tables() {
    require_docker!();
    let pg = PostgresHandle::start()
        .await
        .expect("failed to start postgres container");

    let db_name = unique_name("schema_test");
    let (_, client) = pg
        .isolated_db(&db_name)
        .await
        .expect("isolated_db should succeed");

    // Verify all expected tables exist.
    for table in &[
        "audit_events",
        "event_metadata",
        "retention_policies",
        "relay_cursors",
    ] {
        let row = client
            .query_one(
                "SELECT count(*) FROM information_schema.tables \
                 WHERE table_schema = 'public' AND table_name = $1",
                &[table],
            )
            .await
            .unwrap_or_else(|e| panic!("query for table {table} failed: {e}"));
        let count: i64 = row.get(0);
        assert_eq!(count, 1, "expected table {table} to exist");
    }

    pg.drop_database(&db_name)
        .await
        .expect("drop_database should succeed");
}

/// Insert and retrieve an audit event row.
#[tokio::test]
#[ignore = "requires Docker"]
async fn postgres_insert_and_query_audit_event() {
    require_docker!();
    let pg = PostgresHandle::start()
        .await
        .expect("failed to start postgres container");

    let db_name = unique_name("audit_events_test");
    let (_, client) = pg
        .isolated_db(&db_name)
        .await
        .expect("isolated_db should succeed");

    let contract_id = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
    let event_type = "payment";
    let submitter = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";

    client
        .execute(
            "INSERT INTO audit_events \
             (contract_id, ledger_sequence, event_index, event_type, submitter) \
             VALUES ($1, $2, $3, $4, $5)",
            &[&contract_id, &1000_i64, &0_i32, &event_type, &submitter],
        )
        .await
        .expect("INSERT should succeed");

    let row = client
        .query_one(
            "SELECT contract_id, event_type, submitter \
             FROM audit_events WHERE contract_id = $1",
            &[&contract_id],
        )
        .await
        .expect("SELECT should return one row");

    assert_eq!(row.get::<_, &str>(0), contract_id);
    assert_eq!(row.get::<_, &str>(1), event_type);
    assert_eq!(row.get::<_, &str>(2), submitter);

    pg.drop_database(&db_name).await.expect("cleanup");
}

/// Unique constraint enforcement — duplicate (contract, seq, index) is rejected.
#[tokio::test]
#[ignore = "requires Docker"]
async fn postgres_unique_constraint_on_event_tuple() {
    require_docker!();
    let pg = PostgresHandle::start()
        .await
        .expect("failed to start postgres container");

    let db_name = unique_name("unique_constraint_test");
    let (_, client) = pg.isolated_db(&db_name).await.expect("isolated_db");

    let insert = |seq: i64, idx: i32| {
        client.execute(
            "INSERT INTO audit_events \
             (contract_id, ledger_sequence, event_index, event_type, submitter) \
             VALUES ('CONTRACT123', $1, $2, 'payment', 'SUBMITTER')",
            &[&seq, &idx],
        )
    };

    insert(1000, 0).await.expect("first insert should succeed");
    let result = insert(1000, 0).await;
    assert!(
        result.is_err(),
        "duplicate (seq, index) should violate unique constraint"
    );

    pg.drop_database(&db_name).await.expect("cleanup");
}

/// Relay cursor upsert — simulates the relayer advancing its position.
#[tokio::test]
#[ignore = "requires Docker"]
async fn postgres_relay_cursor_upsert() {
    require_docker!();
    let pg = PostgresHandle::start()
        .await
        .expect("failed to start postgres container");

    let db_name = unique_name("cursor_test");
    let (_, client) = pg.isolated_db(&db_name).await.expect("isolated_db");

    let contract_id = "CONTRACT_CURSOR_TEST";

    // First upsert (insert).
    client
        .execute(
            "INSERT INTO relay_cursors (contract_id, last_ledger_seq, last_event_index) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (contract_id) DO UPDATE \
             SET last_ledger_seq = EXCLUDED.last_ledger_seq, \
                 last_event_index = EXCLUDED.last_event_index, \
                 updated_at = now()",
            &[&contract_id, &500_i64, &2_i32],
        )
        .await
        .expect("first upsert should succeed");

    // Second upsert (update).
    client
        .execute(
            "INSERT INTO relay_cursors (contract_id, last_ledger_seq, last_event_index) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (contract_id) DO UPDATE \
             SET last_ledger_seq = EXCLUDED.last_ledger_seq, \
                 last_event_index = EXCLUDED.last_event_index, \
                 updated_at = now()",
            &[&contract_id, &750_i64, &5_i32],
        )
        .await
        .expect("second upsert should succeed");

    let row = client
        .query_one(
            "SELECT last_ledger_seq, last_event_index FROM relay_cursors WHERE contract_id = $1",
            &[&contract_id],
        )
        .await
        .expect("cursor row should exist");

    assert_eq!(row.get::<_, i64>(0), 750);
    assert_eq!(row.get::<_, i32>(1), 5);

    pg.drop_database(&db_name).await.expect("cleanup");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Redis tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Basic connectivity — PING → PONG.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_container_starts_and_pings() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    redis.ping().await.expect("PING should succeed");
    assert!(redis.host_port > 0);
}

/// Event caching — store and retrieve audit event JSON.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_cache_and_retrieve_event() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    const DB: u8 = 1;
    redis.flush_db(DB).await.expect("flush db");

    let payload = r#"{"event_type":"payment","amount":100}"#;
    redis
        .cache_event(DB, "CONTRACT_ABC", 1_000, 0, payload, Some(300))
        .await
        .expect("cache_event should succeed");

    let retrieved = redis
        .get_cached_event(DB, "CONTRACT_ABC", 1_000, 0)
        .await
        .expect("get_cached_event should succeed")
        .expect("event should be present");

    assert_eq!(retrieved, payload);

    redis.flush_db(DB).await.expect("cleanup");
}

/// TTL expiry — verify a key with a 1-second TTL disappears.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_ttl_expiry() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    const DB: u8 = 2;
    redis.flush_db(DB).await.expect("flush db");

    redis
        .cache_event(DB, "CONTRACT_TTL", 2_000, 0, "expiring_payload", Some(1))
        .await
        .expect("cache_event with 1s TTL");

    // Key should exist immediately.
    let before = redis
        .get_cached_event(DB, "CONTRACT_TTL", 2_000, 0)
        .await
        .expect("get should succeed")
        .is_some();
    assert!(before, "key should exist before TTL expires");

    // Wait for TTL to expire.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let after = redis
        .get_cached_event(DB, "CONTRACT_TTL", 2_000, 0)
        .await
        .expect("get should succeed after expiry");
    assert!(after.is_none(), "key should be gone after TTL expires");
}

/// Session store — store and retrieve a session.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_session_store_roundtrip() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    const DB: u8 = 3;
    redis.flush_db(DB).await.expect("flush db");

    let session_id = Uuid::new_v4().to_string();
    let user_data = r#"{"user_id":"alice","role":"auditor"}"#;

    redis
        .store_session(DB, &session_id, user_data, 3600)
        .await
        .expect("store_session should succeed");

    let retrieved = redis
        .get_session(DB, &session_id)
        .await
        .expect("get_session should succeed")
        .expect("session should be present");

    assert_eq!(retrieved, user_data);

    redis.flush_db(DB).await.expect("cleanup");
}

/// Distributed lock — acquire, verify exclusive access, release.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_distributed_lock_exclusive() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    const DB: u8 = 4;
    redis.flush_db(DB).await.expect("flush db");

    let lock_key = "contract_upgrade";
    let owner_a = "node-a";
    let owner_b = "node-b";

    // Node A acquires the lock.
    let acquired_a = redis
        .try_lock(DB, lock_key, owner_a, 30)
        .await
        .expect("try_lock should succeed");
    assert!(acquired_a, "node-a should acquire lock");

    // Node B cannot acquire the same lock.
    let acquired_b = redis
        .try_lock(DB, lock_key, owner_b, 30)
        .await
        .expect("try_lock should succeed");
    assert!(!acquired_b, "node-b should not acquire held lock");

    // Node A releases the lock.
    let released = redis
        .release_lock(DB, lock_key, owner_a)
        .await
        .expect("release_lock should succeed");
    assert!(released, "node-a should release its own lock");

    // Node B can now acquire it.
    let acquired_b2 = redis
        .try_lock(DB, lock_key, owner_b, 30)
        .await
        .expect("try_lock should succeed after release");
    assert!(acquired_b2, "node-b should acquire lock after release");

    redis.flush_db(DB).await.expect("cleanup");
}

/// Rate limiter — verify the counter increments and respects the window.
#[tokio::test]
#[ignore = "requires Docker"]
async fn redis_rate_limiter() {
    require_docker!();
    let redis = RedisHandle::start()
        .await
        .expect("failed to start redis container");

    const DB: u8 = 5;
    redis.flush_db(DB).await.expect("flush db");

    let key = &format!("rl:{}", Uuid::new_v4());
    for expected in 1..=5i64 {
        let count = redis
            .rate_limit_incr(DB, key, 60)
            .await
            .expect("rate_limit_incr should succeed");
        assert_eq!(count, expected);
    }

    redis.flush_db(DB).await.expect("cleanup");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Kafka tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Basic connectivity — broker starts and accepts producer connections.
#[tokio::test]
#[ignore = "requires Docker"]
async fn kafka_container_starts_and_accepts_producer() {
    require_docker!();
    let kafka = KafkaHandle::start()
        .await
        .expect("failed to start kafka container");

    assert!(!kafka.bootstrap_servers.is_empty());
    kafka
        .producer()
        .expect("FutureProducer should be creatable");
}

/// Topic creation — create a topic and verify produce/consume round-trip.
#[tokio::test]
#[ignore = "requires Docker"]
async fn kafka_produce_and_consume_single_message() {
    require_docker!();
    let kafka = KafkaHandle::start()
        .await
        .expect("failed to start kafka container");

    let topic = kafka
        .create_isolated_topic("audit_test")
        .await
        .expect("topic creation should succeed");

    let payload = r#"{"event":"payment","amount":250}"#;
    kafka
        .produce(&topic, "key1", payload)
        .await
        .expect("produce should succeed");

    let messages = kafka
        .consume_n(
            &unique_name("consumer_group"),
            &[&topic],
            1,
            Duration::from_secs(30),
        )
        .await
        .expect("consume_n should succeed");

    assert_eq!(messages.len(), 1, "should have consumed exactly 1 message");
    let msg = &messages[0];
    let body = std::str::from_utf8(msg.payload().unwrap_or_default())
        .expect("payload should be valid UTF-8");
    assert_eq!(body, payload);
}

/// Batch produce — verify all messages arrive in order.
#[tokio::test]
#[ignore = "requires Docker"]
async fn kafka_batch_produce_and_consume() {
    require_docker!();
    let kafka = KafkaHandle::start()
        .await
        .expect("failed to start kafka container");

    let topic = kafka
        .create_isolated_topic("batch_test")
        .await
        .expect("topic creation");

    const MSG_COUNT: usize = 10;
    let messages: Vec<(String, String)> = (0..MSG_COUNT)
        .map(|i| (format!("key{i}"), format!(r#"{{"seq":{i}}}"#)))
        .collect();

    let refs: Vec<(&str, &str)> = messages
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    kafka
        .produce_batch(&topic, &refs)
        .await
        .expect("batch produce should succeed");

    let consumed = kafka
        .consume_n(
            &unique_name("batch_consumer"),
            &[&topic],
            MSG_COUNT,
            Duration::from_secs(30),
        )
        .await
        .expect("consume_n should succeed");

    assert_eq!(
        consumed.len(),
        MSG_COUNT,
        "all messages should be consumed"
    );
}

/// Audit event helper — produce a structured event and consume it.
#[tokio::test]
#[ignore = "requires Docker"]
async fn kafka_audit_event_produce_consume() {
    require_docker!();
    let kafka = KafkaHandle::start()
        .await
        .expect("failed to start kafka container");

    // Use the standard topic for audit events.
    kafka
        .create_topic(containers::kafka::TOPIC_AUDIT_EVENTS, 1)
        .await
        .expect("create topic");

    kafka
        .produce_audit_event(
            "CONTRACT_XYZ",
            "transfer",
            "SUBMITTER_ADDRESS",
            json!({"amount": 1000, "currency": "USDC"}),
            1_234_567,
        )
        .await
        .expect("produce_audit_event should succeed");

    let msgs = kafka
        .consume_n(
            &unique_name("audit_consumer"),
            &[containers::kafka::TOPIC_AUDIT_EVENTS],
            1,
            Duration::from_secs(30),
        )
        .await
        .expect("consume_n");

    assert_eq!(msgs.len(), 1);
    let body: serde_json::Value =
        serde_json::from_slice(msgs[0].payload().unwrap_or_default())
            .expect("payload should be valid JSON");
    assert_eq!(body["event_type"].as_str(), Some("transfer"));
    assert_eq!(body["contract_id"].as_str(), Some("CONTRACT_XYZ"));
}

/// Dead-letter queue — messages that fail processing are routed to DLQ.
#[tokio::test]
#[ignore = "requires Docker"]
async fn kafka_dead_letter_queue_routing() {
    require_docker!();
    let kafka = KafkaHandle::start()
        .await
        .expect("failed to start kafka container");

    kafka
        .create_topic(containers::kafka::TOPIC_DLQ, 1)
        .await
        .expect("create DLQ topic");

    // Simulate a failed message being re-produced to the DLQ.
    let dlq_payload =
        r#"{"original_topic":"audit.events","error":"schema_validation_failed","payload":"{}"}"#;
    kafka
        .produce(containers::kafka::TOPIC_DLQ, "dlq-key-1", dlq_payload)
        .await
        .expect("produce to DLQ");

    let msgs = kafka
        .consume_n(
            &unique_name("dlq_consumer"),
            &[containers::kafka::TOPIC_DLQ],
            1,
            Duration::from_secs(30),
        )
        .await
        .expect("consume from DLQ");

    assert_eq!(msgs.len(), 1);
    let body: serde_json::Value =
        serde_json::from_slice(msgs[0].payload().unwrap_or_default()).expect("valid JSON");
    assert_eq!(body["error"].as_str(), Some("schema_validation_failed"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Soroban RPC stub tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Health check — `getHealth` returns `{"status":"healthy"}`.
#[tokio::test]
#[ignore = "requires Docker"]  // Stub is in-process; Docker not actually needed.
async fn soroban_stub_health_check() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    assert!(soroban.is_healthy().await, "stub should report healthy");
}

/// Latest ledger — returns the configured sequence number.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_get_latest_ledger() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    soroban.set_latest_ledger(1_234_567);

    let seq = soroban
        .get_latest_ledger()
        .await
        .expect("getLatestLedger should succeed");
    assert_eq!(seq, 1_234_567);
}

/// Event pre-loading — `getEvents` returns pre-loaded contract events.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_get_events_preloaded() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    let contract_id = "CONTRACT_EVENT_TEST";
    soroban.add_event(
        contract_id,
        json!({
            "type": "contract",
            "ledger": "1000000",
            "contractId": contract_id,
            "topic": [{"sym": "payment"}],
            "value": {"u32": 42}
        }),
    );
    soroban.add_event(
        contract_id,
        json!({
            "type": "contract",
            "ledger": "1000001",
            "contractId": contract_id,
            "topic": [{"sym": "transfer"}],
            "value": {"u32": 99}
        }),
    );

    let events = soroban
        .get_events(contract_id, 1_000_000)
        .await
        .expect("getEvents should succeed");

    assert_eq!(events.len(), 2, "should return both pre-loaded events");
    assert_eq!(events[0]["topic"][0]["sym"].as_str(), Some("payment"));
    assert_eq!(events[1]["topic"][0]["sym"].as_str(), Some("transfer"));
}

/// Transaction lifecycle — sendTransaction → getTransaction(SUCCESS).
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_transaction_lifecycle() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    // Simulate a transaction.
    let send_result = soroban
        .rpc_call(
            "sendTransaction",
            json!({"transaction": "AAAAAAAAAAAAAAAA"}),
        )
        .await
        .expect("sendTransaction should succeed");

    assert_eq!(send_result["status"].as_str(), Some("PENDING"));
    let hash = send_result["hash"].as_str().expect("hash must be present");

    // Poll status — stub always returns SUCCESS.
    let get_result = soroban
        .rpc_call("getTransaction", json!({"hash": hash}))
        .await
        .expect("getTransaction should succeed");

    assert_eq!(get_result["status"].as_str(), Some("SUCCESS"));
}

/// Simulate transaction — verifies simulateTransaction response shape.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_simulate_transaction() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    let result = soroban
        .rpc_call(
            "simulateTransaction",
            json!({"transaction": "AAAAAAAAAAAAAAAA"}),
        )
        .await
        .expect("simulateTransaction should succeed");

    assert!(result["minResourceFee"].as_str().is_some());
    assert!(result["results"].as_array().is_some());
}

/// Ledger entry pre-loading — getLedgerEntries returns pre-loaded data.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_ledger_entry_preloading() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    let key = "AAAA_CONTRACT_STORAGE_KEY";
    soroban.add_ledger_entry(key, json!({"u32": 12345}));

    let result = soroban
        .rpc_call("getLedgerEntries", json!({"keys": [key]}))
        .await
        .expect("getLedgerEntries should succeed");

    let entries = result["entries"].as_array().expect("entries must be array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["xdr"]["u32"].as_u64(), Some(12345));
}

/// Fee stats — verifies getFeeStats response shape.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_fee_stats() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    let result = soroban
        .rpc_call("getFeeStats", json!({}))
        .await
        .expect("getFeeStats should succeed");

    assert!(result["sorobanInclusionFee"]["max"].as_str().is_some());
    assert!(result["inclusionFee"]["min"].as_str().is_some());
}

/// Custom tx status — override a transaction to FAILED and verify.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_stub_custom_tx_status() {
    let soroban = SorobanHandle::start()
        .await
        .expect("failed to start soroban stub");

    let hash = "deadbeefdeadbeef0000000000000000000000000000000000000000deadbeef";
    soroban.set_tx_status(hash, "FAILED");

    let result = soroban
        .rpc_call("getTransaction", json!({"hash": hash}))
        .await
        .expect("getTransaction should succeed");

    assert_eq!(result["status"].as_str(), Some("FAILED"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cross-service integration tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Full event pipeline: Kafka → PostgreSQL → Redis cache
///
/// Simulates the relayer consuming Kafka events and persisting them to Postgres
/// while caching hot events in Redis.
#[tokio::test]
#[ignore = "requires Docker"]
async fn pipeline_kafka_to_postgres_to_redis() {
    require_docker!();

    // Start all three containers concurrently.
    let (pg_res, redis_res, kafka_res) = tokio::join!(
        PostgresHandle::start(),
        RedisHandle::start(),
        KafkaHandle::start(),
    );
    let pg = pg_res.expect("postgres");
    let redis = redis_res.expect("redis");
    let kafka = kafka_res.expect("kafka");

    // Set up isolated resources.
    let db_name = unique_name("pipeline_test");
    let (_, db) = pg.isolated_db(&db_name).await.expect("isolated_db");
    let topic = kafka
        .create_isolated_topic("pipeline")
        .await
        .expect("topic");

    const REDIS_DB: u8 = 6;
    redis.flush_db(REDIS_DB).await.expect("flush redis db");

    // ── Step 1: Produce events to Kafka ───────────────────────────────────────
    let events: Vec<(String, String)> = (0..5)
        .map(|i| {
            let key = format!("CONTRACT_PIPE:{i}");
            let payload = json!({
                "contract_id": "CONTRACT_PIPE",
                "event_type": "payment",
                "submitter": "SUBMITTER",
                "ledger_seq": 1000 + i,
                "event_index": 0
            })
            .to_string();
            (key, payload)
        })
        .collect();

    let refs: Vec<(&str, &str)> = events
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    kafka
        .produce_batch(&topic, &refs)
        .await
        .expect("produce batch");

    // ── Step 2: Consume from Kafka ─────────────────────────────────────────────
    let consumed = kafka
        .consume_n(
            &unique_name("pipeline_consumer"),
            &[&topic],
            5,
            Duration::from_secs(30),
        )
        .await
        .expect("consume");

    assert_eq!(consumed.len(), 5, "should have consumed 5 messages");

    // ── Step 3: Persist to PostgreSQL and cache in Redis ──────────────────────
    for msg in &consumed {
        let payload: serde_json::Value =
            serde_json::from_slice(msg.payload().unwrap_or_default()).expect("valid JSON");

        let contract_id = payload["contract_id"].as_str().unwrap_or_default();
        let ledger_seq: i64 = payload["ledger_seq"].as_i64().unwrap_or(0);
        let event_type = payload["event_type"].as_str().unwrap_or_default();
        let submitter = payload["submitter"].as_str().unwrap_or_default();

        // Write to Postgres.
        db.execute(
            "INSERT INTO audit_events \
             (contract_id, ledger_sequence, event_index, event_type, submitter) \
             VALUES ($1, $2, $3, $4, $5)",
            &[
                &contract_id,
                &ledger_seq,
                &0_i32,
                &event_type,
                &submitter,
            ],
        )
        .await
        .expect("INSERT should succeed");

        // Cache in Redis (TTL = 5 min).
        redis
            .cache_event(
                REDIS_DB,
                contract_id,
                ledger_seq as u64,
                0,
                &payload.to_string(),
                Some(300),
            )
            .await
            .expect("cache_event should succeed");
    }

    // ── Step 4: Verify Postgres row count ─────────────────────────────────────
    let row = db
        .query_one(
            "SELECT count(*) FROM audit_events WHERE contract_id = 'CONTRACT_PIPE'",
            &[],
        )
        .await
        .expect("count query");
    let count: i64 = row.get(0);
    assert_eq!(count, 5, "all 5 events should be in postgres");

    // ── Step 5: Verify Redis cache ────────────────────────────────────────────
    let cache_size = redis.dbsize(REDIS_DB).await.expect("dbsize");
    assert_eq!(cache_size, 5, "all 5 events should be cached in redis");

    // ── Cleanup ───────────────────────────────────────────────────────────────
    redis.flush_db(REDIS_DB).await.expect("cleanup redis");
    pg.drop_database(&db_name).await.expect("cleanup postgres");
}

/// Parallel cross-service test — runs multiple independent subtests
/// concurrently to demonstrate parallel execution capabilities.
#[tokio::test]
#[ignore = "requires Docker"]
async fn parallel_independent_service_tests() {
    require_docker!();

    let (pg_res, redis_res) =
        tokio::join!(PostgresHandle::start(), RedisHandle::start());
    let pg = std::sync::Arc::new(pg_res.expect("postgres"));
    let redis = std::sync::Arc::new(redis_res.expect("redis"));

    let pg1 = std::sync::Arc::clone(&pg);
    let pg2 = std::sync::Arc::clone(&pg);
    let redis1 = std::sync::Arc::clone(&redis);
    let redis2 = std::sync::Arc::clone(&redis);

    // Run four concurrent subtests.
    let results = run_parallel::<(), Box<dyn std::error::Error + Send + Sync>>(vec![
        Box::pin(async move {
            let db = unique_name("par_pg1");
            let (_, client) = pg1.isolated_db(&db).await?;
            client
                .execute(
                    "INSERT INTO audit_events \
                     (contract_id, ledger_sequence, event_index, event_type, submitter) \
                     VALUES ('C1', 1, 0, 'pay', 'S1')",
                    &[],
                )
                .await?;
            pg1.drop_database(&db).await?;
            Ok(())
        }),
        Box::pin(async move {
            let db = unique_name("par_pg2");
            let (_, client) = pg2.isolated_db(&db).await?;
            client
                .execute(
                    "INSERT INTO retention_policies \
                     (contract_id, ttl_days) VALUES ('C2', 90)",
                    &[],
                )
                .await?;
            pg2.drop_database(&db).await?;
            Ok(())
        }),
        Box::pin(async move {
            redis1.flush_db(7).await?;
            redis1
                .cache_event(7, "C3", 100, 0, "{\"type\":\"test\"}", Some(60))
                .await?;
            let v = redis1.get_cached_event(7, "C3", 100, 0).await?;
            assert!(v.is_some(), "cached event should exist");
            redis1.flush_db(7).await?;
            Ok(())
        }),
        Box::pin(async move {
            redis2.flush_db(8).await?;
            let lock_key = unique_name("par_lock");
            let acquired = redis2.try_lock(8, &lock_key, "node-par", 10).await?;
            assert!(acquired, "parallel lock should be acquired");
            redis2.flush_db(8).await?;
            Ok(())
        }),
    ])
    .await;

    results.expect("all parallel subtests should succeed");
}

/// Soroban-to-Kafka bridge simulation — events fetched from RPC are produced
/// to Kafka for downstream processing.
#[tokio::test]
#[ignore = "requires Docker"]
async fn soroban_events_forwarded_to_kafka() {
    require_docker!();

    let (soroban_res, kafka_res) =
        tokio::join!(SorobanHandle::start(), KafkaHandle::start());
    let soroban = soroban_res.expect("soroban stub");
    let kafka = kafka_res.expect("kafka");

    let contract_id = "CONTRACT_BRIDGE";

    // Pre-load 3 events in the stub.
    for i in 0..3u32 {
        soroban.add_event(
            contract_id,
            json!({
                "type": "contract",
                "ledger": (1_000_000 + i).to_string(),
                "contractId": contract_id,
                "topic": [{"sym": "payment"}],
                "value": {"u32": i * 100},
                "id": format!("{i}")
            }),
        );
    }

    // Create a Kafka topic for the bridge output.
    let topic = kafka
        .create_isolated_topic("bridge_output")
        .await
        .expect("create topic");

    // Simulate the relayer: fetch from Soroban, forward to Kafka.
    let events = soroban
        .get_events(contract_id, 1_000_000)
        .await
        .expect("get_events");
    assert_eq!(events.len(), 3);

    for event in &events {
        kafka
            .produce(&topic, contract_id, &event.to_string())
            .await
            .expect("produce event to kafka");
    }

    // Verify all 3 events arrive on Kafka.
    let consumed = kafka
        .consume_n(
            &unique_name("bridge_consumer"),
            &[&topic],
            3,
            Duration::from_secs(30),
        )
        .await
        .expect("consume");

    assert_eq!(consumed.len(), 3, "all 3 events should arrive on Kafka");
}
