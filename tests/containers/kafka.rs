//! # Kafka container fixture
//!
//! Provides a [`KafkaHandle`] that wraps a running Kafka container (using the
//! KRaft mode image from `testcontainers-modules`) together with typed helpers
//! for creating topics, producing audit events, and consuming them.
//!
//! ## Architecture
//!
//! The testcontainers Kafka module uses Confluent's `cp-kafka` image in KRaft
//! mode (no Zookeeper required for single-broker test setups).
//!
//! Topic names are namespaced with a unique prefix so that parallel tests do not
//! interfere with each other even when they share the same broker.

use std::time::Duration;

use rdkafka::{
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    client::DefaultClientContext,
    config::ClientConfig,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::OwnedMessage,
    producer::{FutureProducer, FutureRecord},
    Message,
};
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::kafka::Kafka;

use super::{unique_name, wait_until_ready, CONTAINER_POLL_INTERVAL, CONTAINER_READY_TIMEOUT};

// ── Image constants ────────────────────────────────────────────────────────────

const KAFKA_PORT: u16 = 9092;

// ── Standard topic names ───────────────────────────────────────────────────────

/// Audit events emitted by the smart-contract relayer.
pub const TOPIC_AUDIT_EVENTS: &str = "audit.events";
/// Governance actions (cap changes, ownership transfers).
pub const TOPIC_GOVERNANCE: &str = "audit.governance";
/// Dead-letter queue for events that could not be processed.
pub const TOPIC_DLQ: &str = "audit.dlq";

// ── Public handle ─────────────────────────────────────────────────────────────

/// A running Kafka broker (KRaft mode, single node) with helpers for tests.
pub struct KafkaHandle {
    _container: ContainerAsync<Kafka>,
    pub bootstrap_servers: String,
    pub host_port: u16,
}

impl KafkaHandle {
    // ── Lifecycle ──────────────────────────────────────────────────────────────

    /// Start a Kafka container and wait until the broker is reachable.
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let container = Kafka::default().start().await?;
        let host_port = container.get_host_port_ipv4(KAFKA_PORT).await?;
        let bootstrap_servers = format!("localhost:{host_port}");

        let handle = Self {
            _container: container,
            bootstrap_servers: bootstrap_servers.clone(),
            host_port,
        };

        // Wait until Kafka metadata is accessible via the producer API.
        let attempts = (CONTAINER_READY_TIMEOUT.as_millis()
            / CONTAINER_POLL_INTERVAL.as_millis()) as u32;

        wait_until_ready(attempts, CONTAINER_POLL_INTERVAL, || {
            let bs = bootstrap_servers.clone();
            async move {
                // Attempt to create a transient producer — rdkafka does
                // metadata fetching on first use.
                let producer: Result<FutureProducer, _> = ClientConfig::new()
                    .set("bootstrap.servers", &bs)
                    .set("message.timeout.ms", "3000")
                    .create();
                producer
                    .map(|_| ())
                    .map_err(|e| format!("kafka broker not ready: {e}"))
            }
        })
        .await
        .map_err(|e| format!("kafka container never became ready: {e}"))?;

        Ok(handle)
    }

    // ── Topic management ───────────────────────────────────────────────────────

    /// Create a topic with the given name and number of partitions.
    pub async fn create_topic(
        &self,
        name: &str,
        partitions: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
            .set("bootstrap.servers", &self.bootstrap_servers)
            .create()?;

        let topic = NewTopic::new(name, partitions, TopicReplication::Fixed(1));
        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));
        let results = admin.create_topics(&[topic], &opts).await?;

        for result in results {
            match result {
                Ok(_) => {}
                Err((topic, e)) => {
                    // "already exists" is acceptable in shared-container scenarios.
                    if !format!("{e:?}").contains("TopicAlreadyExists") {
                        return Err(format!("failed to create topic {topic}: {e:?}").into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Create all standard audit-ledger topics with sensible defaults.
    pub async fn create_standard_topics(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.create_topic(TOPIC_AUDIT_EVENTS, 3).await?;
        self.create_topic(TOPIC_GOVERNANCE, 1).await?;
        self.create_topic(TOPIC_DLQ, 1).await?;
        Ok(())
    }

    /// Create a uniquely-named topic for test isolation and return its name.
    pub async fn create_isolated_topic(
        &self,
        prefix: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let name = unique_name(prefix);
        self.create_topic(&name, 1).await?;
        Ok(name)
    }

    // ── Producer helpers ───────────────────────────────────────────────────────

    /// Build an rdkafka `FutureProducer` connected to this broker.
    pub fn producer(&self) -> Result<FutureProducer, Box<dyn std::error::Error + Send + Sync>> {
        ClientConfig::new()
            .set("bootstrap.servers", &self.bootstrap_servers)
            .set("message.timeout.ms", "5000")
            // Idempotent delivery for exactly-once semantics in tests.
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .create()
            .map_err(Into::into)
    }

    /// Produce a single message to `topic` with `key` and JSON `payload`.
    ///
    /// Returns `(partition, offset)` on successful delivery.
    pub async fn produce(
        &self,
        topic: &str,
        key: &str,
        payload: &str,
    ) -> Result<(i32, i64), Box<dyn std::error::Error + Send + Sync>> {
        let producer = self.producer()?;
        let record = FutureRecord::to(topic).payload(payload).key(key);
        // `send` returns `OwnedDeliveryResult` = `Result<(partition, offset), (KafkaError, OwnedMessage)>`
        let (partition, offset) = producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| format!("kafka produce error: {e}"))?;
        Ok((partition, offset))
    }

    /// Produce a batch of messages. Returns after all are delivered.
    pub async fn produce_batch(
        &self,
        topic: &str,
        messages: &[(&str, &str)], // (key, payload) pairs
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let producer = self.producer()?;
        let mut futures = Vec::with_capacity(messages.len());
        for (key, payload) in messages {
            let record = FutureRecord::to(topic).payload(*payload).key(*key);
            futures.push(producer.send(record, Duration::from_secs(5)));
        }
        for f in futures {
            f.await
                .map_err(|(e, _)| format!("kafka batch produce error: {e}"))?;
        }
        Ok(())
    }

    // ── Consumer helpers ───────────────────────────────────────────────────────

    /// Build a `StreamConsumer` for the given consumer group.
    pub fn consumer(
        &self,
        group_id: &str,
    ) -> Result<StreamConsumer, Box<dyn std::error::Error + Send + Sync>> {
        ClientConfig::new()
            .set("bootstrap.servers", &self.bootstrap_servers)
            .set("group.id", group_id)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .set("session.timeout.ms", "10000")
            .create()
            .map_err(Into::into)
    }

    /// Subscribe to `topics` and consume up to `limit` messages.
    ///
    /// Times out after `timeout` if fewer than `limit` messages arrive.
    pub async fn consume_n(
        &self,
        group_id: &str,
        topics: &[&str],
        limit: usize,
        timeout: Duration,
    ) -> Result<Vec<OwnedMessage>, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;
        use rdkafka::consumer::Consumer as _;

        let consumer = self.consumer(group_id)?;
        consumer.subscribe(topics)?;

        let mut messages = Vec::with_capacity(limit);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            if messages.len() >= limit {
                break;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, consumer.stream().next()).await {
                Ok(Some(Ok(msg))) => {
                    consumer.commit_message(&msg, CommitMode::Async)?;
                    messages.push(msg.detach());
                }
                Ok(Some(Err(e))) => return Err(format!("kafka consumer error: {e}").into()),
                Ok(None) | Err(_) => break,
            }
        }
        Ok(messages)
    }

    // ── Audit event helpers ────────────────────────────────────────────────────

    /// Produce a structured audit event to [`TOPIC_AUDIT_EVENTS`].
    pub async fn produce_audit_event(
        &self,
        contract_id: &str,
        event_type: &str,
        submitter: &str,
        metadata: serde_json::Value,
        ledger_seq: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = serde_json::json!({
            "contract_id": contract_id,
            "event_type":  event_type,
            "submitter":   submitter,
            "metadata":    metadata,
            "ledger_seq":  ledger_seq,
            "timestamp":   chrono_now_rfc3339(),
        });
        self.produce(TOPIC_AUDIT_EVENTS, contract_id, &event.to_string())
            .await
            .map(|_| ())
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn chrono_now_rfc3339() -> String {
    // Use a simple timestamp without chrono dependency.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
