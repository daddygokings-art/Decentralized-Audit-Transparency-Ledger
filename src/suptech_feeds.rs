#![no_std]

use crate::suptech_types::{DataFeed, DataFeedType};
use soroban_sdk::{contracterror, contracttype, Bytes, BytesN, Env, Vec};
use soroban_sdk::xdr::ToXdr;

/// Maximum payload retained or hashed by a feed operation.
pub const MAX_FEED_PAYLOAD_BYTES: u32 = 4_096;
/// Maximum feed identifiers retained for one publisher.
pub const MAX_PUBLISHER_FEEDS: u32 = 64;
/// Conservative XDR ceiling for a maximum-size data point.
pub const MAX_DATA_POINT_XDR_BYTES: u32 = 5_120;

/// Deterministic validation and capacity failures for feed operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SuptechFeedError {
    EmptyPayload = 1,
    PayloadTooLarge = 2,
    FeedInactive = 3,
    SubscriptionInactive = 4,
    FeedTypeMismatch = 5,
    SubscriptionCountOverflow = 6,
    PublisherFeedLimitReached = 7,
}

/// Resource bounds for maximum-size values:
/// - A data point contains at most 4,144 bytes of field data (4,096 payload
///   + 32-byte hash + timestamp, feed type, and payload length); its XDR form
///   is capped at 5,120 bytes, excluding ledger keys.
/// - A publisher retains at most 64 fixed 32-byte feed IDs (2,048 raw bytes).
/// - Payload hashing is O(n) and processes at most 4,104 input bytes, so the
///   host work is bounded by 65 SHA-256 blocks per call.

/// Storage tier classification for keys managed by this module.
///
/// Retention SLA drives the tier:
/// - `Instance`: hot, contract-lifetime config (bumped with the contract instance).
/// - `Persistent`: archival-critical audit records that must survive indefinitely.
/// - `Temporary`: high-churn telemetry that may be pruned once stale.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum StorageTier {
    Instance = 0,
    Persistent = 1,
    Temporary = 2,
}

/// Ledger TTL policy (in ledgers) applied per storage tier.
///
/// `threshold` is the minimum remaining TTL before a bump is required;
/// `extend_to` is the target TTL after a bump.
pub const INSTANCE_TTL_THRESHOLD: u32 = 100_000;
pub const INSTANCE_TTL_EXTEND_TO: u32 = 500_000;
pub const PERSISTENT_TTL_THRESHOLD: u32 = 200_000;
pub const PERSISTENT_TTL_EXTEND_TO: u32 = 1_000_000;
pub const TEMPORARY_TTL_THRESHOLD: u32 = 10_000;
pub const TEMPORARY_TTL_EXTEND_TO: u32 = 50_000;

/// Represents a data point in a feed stream.
///
/// Field order is chosen to minimize XDR padding: fixed-width scalars
/// (`u64`, `u32`) are packed first, followed by variable-width `Bytes`
/// and the fixed `BytesN<32>` hash.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPoint {
    /// Timestamp of data point
    pub timestamp: u64,
    /// Feed type
    pub feed_type: u32, // DataFeedType as u32
    /// Data hash for integrity
    pub data_hash: BytesN<32>,
    /// Data payload
    pub payload: Bytes,
}

/// Feed subscription record.
///
/// Scalars are packed ahead of variable-width fields to reduce XDR padding.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedSubscription {
    /// Subscription ID
    pub subscription_id: BytesN<32>,
    /// Feed ID
    pub feed_id: BytesN<32>,
    /// Subscription created at
    pub created_at: u64,
    /// Last data received timestamp
    pub last_data_received: Option<u64>,
    /// Data point count received
    pub data_point_count: u32,
    /// Is active
    pub is_active: bool,
    /// Subscriber address
    pub subscriber: soroban_sdk::Address,
    /// Filter criteria (optional)
    pub filter_criteria: Bytes,
}

/// Feed publishing configuration.
///
/// Scalars are packed ahead of variable-width fields to reduce XDR padding.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedPublisher {
    /// Publisher ID
    pub publisher_id: BytesN<32>,
    /// Created at
    pub created_at: u64,
    /// Total data points published
    pub total_data_points: u32,
    /// Is authorized
    pub is_authorized: bool,
    /// Publisher address
    pub address: soroban_sdk::Address,
    /// Feeds published
    pub feeds_published: Vec<BytesN<32>>,
}

/// Real-time feed manager.
pub struct FeedManager;

impl FeedManager {
    /// Create a new data feed
    pub fn create_feed(
        env: &Env,
        feed_type: DataFeedType,
        initial_data: Bytes,
    ) -> Result<DataFeed, SuptechFeedError> {
        if initial_data.is_empty() {
            return Err(SuptechFeedError::EmptyPayload);
        }
        if initial_data.len() > MAX_FEED_PAYLOAD_BYTES {
            return Err(SuptechFeedError::PayloadTooLarge);
        }

        let feed_id = Self::compute_feed_id(env, feed_type);
        let now = env.ledger().timestamp();

        Ok(DataFeed {
            feed_id,
            feed_type: feed_type as u32,
            current_data: initial_data,
            last_updated: now,
            update_frequency: feed_type.update_frequency_seconds(),
            subscriber_count: 0,
            is_active: true,
            metadata: Bytes::new(env),
        })
    }

    /// Compute deterministic feed ID
    pub fn compute_feed_id(env: &Env, feed_type: DataFeedType) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, feed_type.as_str().as_bytes()));
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        env.crypto().sha256(&input).to_bytes()
    }

    /// Publish data point to feed
    pub fn publish_data_point(
        env: &Env,
        feed: &mut DataFeed,
        new_data: Bytes,
    ) -> Result<DataPoint, SuptechFeedError> {
        if !feed.is_active {
            return Err(SuptechFeedError::FeedInactive);
        }

        if new_data.is_empty() {
            return Err(SuptechFeedError::EmptyPayload);
        }
        if new_data.len() > MAX_FEED_PAYLOAD_BYTES {
            return Err(SuptechFeedError::PayloadTooLarge);
        }

        let now = env.ledger().timestamp();
        let data_hash = Self::compute_data_hash(env, &new_data, now)?;

        // Update feed
        feed.current_data = new_data.clone();
        feed.last_updated = now;

        Ok(DataPoint {
            timestamp: now,
            feed_type: feed.feed_type,
            data_hash,
            payload: new_data,
        })
    }

    /// Compute data point hash
    pub fn compute_data_hash(
        env: &Env,
        data: &Bytes,
        timestamp: u64,
    ) -> Result<BytesN<32>, SuptechFeedError> {
        if data.is_empty() {
            return Err(SuptechFeedError::EmptyPayload);
        }
        if data.len() > MAX_FEED_PAYLOAD_BYTES {
            return Err(SuptechFeedError::PayloadTooLarge);
        }

        let mut input = Bytes::new(env);
        input.append(data);
        input.append(&Bytes::from_slice(env, &timestamp.to_le_bytes()));

        Ok(env.crypto().sha256(&input).to_bytes())
    }

    /// Verify data freshness
    pub fn is_data_fresh(feed: &DataFeed, current_time: u64) -> bool {
        let age = current_time.saturating_sub(feed.last_updated);
        age <= feed.update_frequency
    }

    /// Check if data is stale (overdue for update)
    pub fn is_data_stale(feed: &DataFeed, current_time: u64) -> bool {
        !Self::is_data_fresh(feed, current_time)
    }

    /// Create subscription to feed
    pub fn create_subscription(
        env: &Env,
        feed_id: BytesN<32>,
        subscriber: soroban_sdk::Address,
    ) -> Result<FeedSubscription, SuptechFeedError> {
        let subscription_id = Self::compute_subscription_id(env, &feed_id, &subscriber);

        Ok(FeedSubscription {
            subscription_id,
            feed_id,
            created_at: env.ledger().timestamp(),
            last_data_received: None,
            data_point_count: 0,
            is_active: true,
            subscriber,
            filter_criteria: Bytes::new(env),
        })
    }

    /// Compute subscription ID
    pub fn compute_subscription_id(
        env: &Env,
        feed_id: &BytesN<32>,
        subscriber: &soroban_sdk::Address,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&feed_id.to_bytes());
        input.append(&subscriber.to_xdr(env));

        env.crypto().sha256(&input).to_bytes()
    }

    /// Record data point receipt
    pub fn record_data_receipt(
        subscription: &mut FeedSubscription,
        timestamp: u64,
    ) -> Result<(), SuptechFeedError> {
        if !subscription.is_active {
            return Err(SuptechFeedError::SubscriptionInactive);
        }

        let next_count = subscription
            .data_point_count
            .checked_add(1)
            .ok_or(SuptechFeedError::SubscriptionCountOverflow)?;
        subscription.last_data_received = Some(timestamp);
        subscription.data_point_count = next_count;

        Ok(())
    }

    /// Validate data point against feed
    pub fn validate_data_point(
        data_point: &DataPoint,
        feed: &DataFeed,
    ) -> Result<(), SuptechFeedError> {
        if data_point.feed_type != feed.feed_type {
            return Err(SuptechFeedError::FeedTypeMismatch);
        }

        if data_point.payload.is_empty() {
            return Err(SuptechFeedError::EmptyPayload);
        }
        if data_point.payload.len() > MAX_FEED_PAYLOAD_BYTES {
            return Err(SuptechFeedError::PayloadTooLarge);
        }

        Ok(())
    }

    /// Get feed update lag (seconds behind expected update)
    pub fn get_feed_update_lag(feed: &DataFeed, current_time: u64) -> u64 {
        let age = current_time.saturating_sub(feed.last_updated);
        if age > feed.update_frequency {
            age - feed.update_frequency
        } else {
            0
        }
    }

    /// Check data quality score (0-100)
    pub fn compute_data_quality_score(
        feed: &DataFeed,
        current_time: u64,
        subscriber_count_healthy: u32,
    ) -> u32 {
        let mut score = 100u32;

        // Deduct for staleness
        if !Self::is_data_fresh(feed, current_time) {
            score = score.saturating_sub(20);
        }

        // Deduct for low subscriber count
        if feed.subscriber_count == 0 {
            score = score.saturating_sub(15);
        } else if feed.subscriber_count < subscriber_count_healthy {
            score = score.saturating_sub(10);
        }

        // Deduct if inactive
        if !feed.is_active {
            score = score.saturating_sub(50);
        }

        score
    }

    /// Authorize a publisher
    pub fn authorize_publisher(
        env: &Env,
        publisher_address: soroban_sdk::Address,
    ) -> Result<FeedPublisher, SuptechFeedError> {
        let publisher_id = Self::compute_publisher_id(env, &publisher_address);

        Ok(FeedPublisher {
            publisher_id,
            created_at: env.ledger().timestamp(),
            total_data_points: 0,
            is_authorized: true,
            address: publisher_address,
            feeds_published: Vec::new(env),
        })
    }

    /// Add a feed to the publisher's bounded feed index.
    pub fn add_published_feed(
        publisher: &mut FeedPublisher,
        feed_id: BytesN<32>,
    ) -> Result<(), SuptechFeedError> {
        if publisher.feeds_published.len() >= MAX_PUBLISHER_FEEDS {
            return Err(SuptechFeedError::PublisherFeedLimitReached);
        }
        publisher.feeds_published.push_back(feed_id);
        Ok(())
    }

    /// Compute publisher ID
    pub fn compute_publisher_id(
        env: &Env,
        address: &soroban_sdk::Address,
    ) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(&address.to_xdr(env));
        input.append(&Bytes::from_slice(env, b"PUBLISHER"));

        env.crypto().sha256(&input).to_bytes()
    }
}

/* … truncated 3093 chars — edit only what you need near the top … */
