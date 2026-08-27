#![no_std]

use crate::suptech_types::{DataFeed, DataFeedType};
use soroban_sdk::{contracttype, Bytes, BytesN, Env, Symbol, Vec};

/// Represents a data point in a feed stream.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPoint {
    /// Timestamp of data point
    pub timestamp: u64,
    /// Feed type
    pub feed_type: u8, // DataFeedType as u8
    /// Data payload
    pub payload: Bytes,
    /// Data hash for integrity
    pub data_hash: BytesN<32>,
}

/// Feed subscription record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedSubscription {
    /// Subscription ID
    pub subscription_id: BytesN<32>,
    /// Feed ID
    pub feed_id: BytesN<32>,
    /// Subscriber address
    pub subscriber: soroban_sdk::Address,
    /// Filter criteria (optional)
    pub filter_criteria: Bytes,
    /// Subscription created at
    pub created_at: u64,
    /// Is active
    pub is_active: bool,
    /// Last data received timestamp
    pub last_data_received: Option<u64>,
    /// Data point count received
    pub data_point_count: u32,
}

/// Feed publishing configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedPublisher {
    /// Publisher ID
    pub publisher_id: BytesN<32>,
    /// Publisher address
    pub address: soroban_sdk::Address,
    /// Feeds published
    pub feeds_published: Vec<BytesN<32>>,
    /// Is authorized
    pub is_authorized: bool,
    /// Created at
    pub created_at: u64,
    /// Total data points published
    pub total_data_points: u32,
}

/// Real-time feed manager.
pub struct FeedManager;

impl FeedManager {
    /// Create a new data feed
    pub fn create_feed(
        env: &Env,
        feed_type: DataFeedType,
        initial_data: Bytes,
    ) -> Result<DataFeed, &'static str> {
        if initial_data.is_empty() {
            return Err("Initial data cannot be empty");
        }

        let feed_id = Self::compute_feed_id(env, feed_type);
        let now = env.ledger().timestamp();

        Ok(DataFeed {
            feed_id,
            feed_type: feed_type as u8,
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
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, feed_type.as_symbol().to_string().as_bytes()));
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        sha256(&input)
    }

    /// Publish data point to feed
    pub fn publish_data_point(
        env: &Env,
        feed: &mut DataFeed,
        new_data: Bytes,
    ) -> Result<DataPoint, &'static str> {
        if !feed.is_active {
            return Err("Feed is not active");
        }

        if new_data.is_empty() {
            return Err("Data payload cannot be empty");
        }

        let now = env.ledger().timestamp();
        let data_hash = Self::compute_data_hash(env, &new_data, now);

        // Update feed
        feed.current_data = new_data.clone();
        feed.last_updated = now;

        Ok(DataPoint {
            timestamp: now,
            feed_type: feed.feed_type,
            payload: new_data,
            data_hash,
        })
    }

    /// Compute data point hash
    pub fn compute_data_hash(env: &Env, data: &Bytes, timestamp: u64) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(data);
        input.append(&Bytes::from_slice(env, &timestamp.to_le_bytes()));

        sha256(&input)
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
    ) -> Result<FeedSubscription, &'static str> {
        let subscription_id = Self::compute_subscription_id(env, &feed_id, &subscriber);

        Ok(FeedSubscription {
            subscription_id,
            feed_id,
            subscriber,
            filter_criteria: Bytes::new(env),
            created_at: env.ledger().timestamp(),
            is_active: true,
            last_data_received: None,
            data_point_count: 0,
        })
    }

    /// Compute subscription ID
    pub fn compute_subscription_id(
        env: &Env,
        feed_id: &BytesN<32>,
        subscriber: &soroban_sdk::Address,
    ) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, feed_id.as_ref()));
        input.append(&Bytes::from_slice(env, subscriber.to_xdr().as_ref()));

        sha256(&input)
    }

    /// Record data point receipt
    pub fn record_data_receipt(
        subscription: &mut FeedSubscription,
        timestamp: u64,
    ) -> Result<(), &'static str> {
        if !subscription.is_active {
            return Err("Subscription is not active");
        }

        subscription.last_data_received = Some(timestamp);
        subscription.data_point_count = subscription.data_point_count.saturating_add(1);

        Ok(())
    }

    /// Validate data point against feed
    pub fn validate_data_point(
        data_point: &DataPoint,
        feed: &DataFeed,
    ) -> Result<(), &'static str> {
        if data_point.feed_type != feed.feed_type {
            return Err("Data point feed type mismatch");
        }

        if data_point.payload.is_empty() {
            return Err("Data point payload cannot be empty");
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
    ) -> Result<FeedPublisher, &'static str> {
        let publisher_id = Self::compute_publisher_id(env, &publisher_address);

        Ok(FeedPublisher {
            publisher_id,
            address: publisher_address,
            feeds_published: Vec::new(env),
            is_authorized: true,
            created_at: env.ledger().timestamp(),
            total_data_points: 0,
        })
    }

    /// Compute publisher ID
    pub fn compute_publisher_id(
        env: &Env,
        address: &soroban_sdk::Address,
    ) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, address.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, b"PUBLISHER"));

        sha256(&input)
    }

    /// Deactivate feed
    pub fn deactivate_feed(feed: &mut DataFeed) {
        feed.is_active = false;
    }

    /// Reactivate feed
    pub fn reactivate_feed(feed: &mut DataFeed, current_time: u64) -> Result<(), &'static str> {
        feed.is_active = true;
        feed.last_updated = current_time;
        Ok(())
    }
}

/// Statistics for feed performance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedStatistics {
    /// Average update frequency (actual vs expected)
    pub avg_update_interval: u64,
    /// Total data points published
    pub total_data_points: u32,
    /// Number of subscribers
    pub subscriber_count: u32,
    /// Data quality score (0-100)
    pub quality_score: u32,
    /// Last 24h uptime percentage
    pub uptime_24h: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_creation() {
        let env = soroban_sdk::Env::default();
        let data = Bytes::from_slice(&env, b"test_data");

        let feed =
            FeedManager::create_feed(&env, DataFeedType::TransactionStream, data).unwrap();
        assert!(feed.is_active);
        assert_eq!(feed.subscriber_count, 0);
        assert_eq!(feed.update_frequency, 1);
    }

    #[test]
    fn test_data_freshness() {
        let env = soroban_sdk::Env::default();
        let mut feed = DataFeed {
            feed_id: BytesN::zero(),
            feed_type: DataFeedType::BalanceSnapshot as u8,
            current_data: Bytes::from_slice(&env, b"data"),
            last_updated: 1000,
            update_frequency: 300,
            subscriber_count: 0,
            is_active: true,
            metadata: Bytes::new(&env),
        };

        assert!(FeedManager::is_data_fresh(&feed, 1200)); // 200 < 300
        assert!(!FeedManager::is_data_fresh(&feed, 1400)); // 400 > 300
    }

    #[test]
    fn test_data_quality_score() {
        let env = soroban_sdk::Env::default();
        let feed = DataFeed {
            feed_id: BytesN::zero(),
            feed_type: DataFeedType::TransactionStream as u8,
            current_data: Bytes::from_slice(&env, b"data"),
            last_updated: 1000,
            update_frequency: 300,
            subscriber_count: 10,
            is_active: true,
            metadata: Bytes::new(&env),
        };

        // Fresh, active, some subscribers
        let score = FeedManager::compute_data_quality_score(&feed, 1200, 5);
        assert!(score > 80);

        // Stale
        let score = FeedManager::compute_data_quality_score(&feed, 1400, 5);
        assert!(score < 100);
    }

    #[test]
    fn test_subscription_creation() {
        let env = soroban_sdk::Env::default();
        let feed_id = BytesN::zero();
        let subscriber = soroban_sdk::Address::generate(&env);

        let sub =
            FeedManager::create_subscription(&env, feed_id, subscriber.clone()).unwrap();
        assert!(sub.is_active);
        assert_eq!(sub.data_point_count, 0);
        assert_eq!(sub.last_data_received, None);
    }
}
