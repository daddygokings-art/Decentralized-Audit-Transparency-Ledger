//! # Contract Event Capacity Planning and Auto-Scaling Module
//!
//! Provides on-chain capacity telemetry recording, multi-tenant quota tier management,
//! predictive scaling recommendation anchoring, and resource utilization auditing
//! for the Decentralized Audit Transparency Ledger.
//!
//! ## Core Features:
//! - **Capacity Telemetry Ledger**: Periodic snapshots of TPS, storage growth, gas consumption, and compute load.
//! - **Multi-Tenant Quota Tiers**: Dynamic daily event quotas, burst TPS limits, and per-submitter rate-limiting.
//! - **Predictive Scaling Recommendations**: On-chain anchoring of ML-driven replica and resource sizing decisions.
//! - **Cost Accounting & Optimization**: Gas and storage cost tracking per submitter tier.
//!
//! ## Storage Tiers (retention SLA)
//! - **Persistent**: archival-critical records (quota tiers, submitter quotas, latest
//!   recommendation, policy, telemetry count). These are TTL-extended by the
//!   permissionless [`CapacityPlanning::extend_ttl`] helper.
//! - **Temporary**: high-churn telemetry snapshots in the ring buffer. They are
//!   intentionally short-lived and are not TTL-extended.
//! - **Instance**: reserved for future per-instance config; none currently used.

#![no_std]
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, Address, BytesN, Env, Symbol,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CapacityError {
    /// Quota tier not found
    QuotaTierNotFound = 7001,
    /// Submitter daily event quota exceeded
    QuotaExceeded = 7002,
    /// Submitter is currently throttled due to burst rate limit
    BurstRateExceeded = 7003,
    /// Caller is not authorized as capacity administrator
    UnauthorizedAdmin = 7004,
    /// Invalid recommendation parameters
    InvalidRecommendation = 7005,
    /// Telemetry capacity limit reached
    TelemetryCapacityExceeded = 7006,
    /// Policy parameter out of allowable bounds
    InvalidPolicyBounds = 7007,
}

// ── Data Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapacityMetricRecord {
    pub timestamp: u64,
    pub current_tps: u32,
    pub peak_tps: u32,
    pub storage_bytes_used: u64,
    pub active_submitters: u32,
    pub gas_consumed_stroops: u64,
    pub cpu_utilization_basis_pts: u32, // 10000 = 100%
    pub memory_mb_used: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaTier {
    pub tier_id: u32,
    pub tier_name: Symbol,
    pub max_daily_events: u64,
    pub max_burst_tps: u32,
    pub storage_quota_bytes: u64,
    pub max_batch_size: u32,
    pub cost_per_m_events_stroops: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitterQuotaRecord {
    pub submitter: Address,
    pub tier_id: u32,
    pub events_in_current_window: u64,
    pub window_start: u64,
    pub is_throttled: bool,
    pub total_cost_billed_stroops: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingRecommendationRecord {
    pub timestamp: u64,
    pub recommended_min_replicas: u32,
    pub recommended_max_replicas: u32,
    pub recommended_cpu_millicores: u32,
    pub recommended_memory_mb: u32,
    pub forecast_horizon_hours: u32,
    pub confidence_score_percent: u32,
    pub estimated_monthly_cost_usd: u32,
    pub model_digest: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapacityPolicy {
    pub auto_scaling_enabled: bool,
    pub target_cpu_percent: u32,
    pub target_tps_per_replica: u32,
    pub cooldown_period_seconds: u32,
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub last_scaled_at: u64,
}

// ── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapacityStorageKey {
    TelemetrySnapshot(u32),
    TelemetryCount,
    Tier(u32),
    SubmitterQuota(Address),
    LatestRecommendation,
    Policy,
}

/// Deterministic bounds to cap on-chain storage, memory and CPU usage.
const MAX_TELEMETRY_SNAPSHOTS: u32 = 1000;
const MAX_BATCH_SIZE: u32 = 10_000;
const MAX_REPLICAS: u32 = 10_000;
const MAX_CPU_BASIS_PTS: u32 = 10_000; // 100.00%
const MAX_TPS: u32 = 1_000_000;
const MAX_MEMORY_MB: u32 = 1_000_000;
const MAX_FORECAST_HOURS: u32 = 8_760; // 1 year
const MAX_CONFIDENCE_PCT: u32 = 100;

/// TTL extension policy for archival-critical persistent records.
///
/// `TTL_THRESHOLD_LEDGERS` is the minimum remaining TTL before a bump is
/// performed; `TTL_EXTEND_LEDGERS` is the amount added on each bump. Both are
/// sized so that active audit records never lapse between maintenance windows.
const TTL_THRESHOLD_LEDGERS: u32 = 100_000;
const TTL_EXTEND_LEDGERS: u32 = 500_000;

// ── Capacity Planning Functions ──────────────────────────────────────────

pub struct CapacityPlanning;

impl CapacityPlanning {
    /// Record a point-in-time capacity and resource telemetry metric
    pub fn record_telemetry(
        env: &Env,
        caller: Address,
        metric: CapacityMetricRecord,
    ) {
        caller.require_auth();

        // Reject malformed metric payloads.
        if metric.current_tps > MAX_TPS
            || metric.peak_tps > MAX_TPS
            || metric.cpu_utilization_basis_pts > MAX_CPU_BASIS_PTS
            || metric.memory_mb_used > MAX_MEMORY_MB
        {
            panic_with_error!(env, CapacityError::InvalidRecommendation);
        }

        let count: u32 = env
            .storage()
            .persistent()
            .get(&CapacityStorageKey::TelemetryCount)
            .unwrap_or(0);

        // Ring buffer: deterministic, bounded snapshot count.
        // Snapshots are high-churn and use the Temporary tier to avoid rent.
        let next_idx = count % MAX_TELEMETRY_SNAPSHOTS;
        let key = CapacityStorageKey::TelemetrySnapshot(next_idx);
        env.storage().temporary().set(&key, &metric);

        let next_count = count
            .checked_add(1)
            .unwrap_or(MAX_TELEMETRY_SNAPSHOTS);
        let count_key = CapacityStorageKey::TelemetryCount;
        env.storage().persistent().set(&count_key, &next_count);
        Self::bump_persistent_ttl(env, &count_key);
    }

    /// Register or update a multi-tenant quota tier
    pub fn set_quota_tier(
        env: &Env,
        admin: Address,
        tier: QuotaTier,
    ) {
        admin.require_auth();

        // Reject malformed / out-of-bounds tier parameters.
        if tier.max_daily_events == 0
            || tier.max_burst_tps > MAX_TPS
            || tier.storage_quota_bytes == 0
            || tier.max_batch_size == 0
            || tier.max_batch_size > MAX_BATCH_SIZE
            || tier.cost_per_m_events_stroops == 0
        {
            panic_with_error!(env, CapacityError::InvalidPolicyBounds);
        }

        let key = CapacityStorageKey::Tier(tier.tier_id);
        env.storage().persistent().set(&key, &tier);
        Self::bump_persistent_ttl(env, &key);
    }

    /// Assign a submitter to a quota tier
    pub fn set_submitter_quota(
        env: &Env,
        admin: Address,
        submitter: Address,
        tier_id: u32,
    ) {
        admin.require_auth();

        let tier_key = CapacityStorageKey::Tier(tier_id);
        if !env.storage().persistent().has(&tier_key) {
            panic_with_error!(env, CapacityError::QuotaTierNotFound);
        }

        let record = SubmitterQuotaRecord {
            submitter: submitter.clone(),
            tier_id,
            events_in_current_window: 0,
            window_start: env.ledger().timestamp(),
            is_throttled: false,
            total_cost_billed_stroops: 0,
        };

        let sub_key = CapacityStorageKey::SubmitterQuota(submitter);
        env.storage().persistent().set(&sub_key, &record);
        Self::bump_persistent_ttl(env, &sub_key);
    }

    /// Check and consume quota for a submitter logging batch events
    pub fn consume_quota(
        env: &Env,
        submitter: Address,
        event_count: u32,
    ) -> bool {
        submitter.require_auth();

        // Reject empty or oversized batches to bound per-call work.
        if event_count == 0 || event_count > MAX_BATCH_SIZE {
            panic_with_error!(env, CapacityError::InvalidRecommendation);
        }

        let sub_key = CapacityStorageKey::SubmitterQuota(submitter.clone());
        let mut sub_record: SubmitterQuotaRecord = match env.storage().persistent().get(&sub_key) {
            Some(r) => r,
            None => {
                // Default Tier 1 (Standard)
                SubmitterQuotaRecord {
                    submitter: submitter.clone(),
                    tier_id: 1,
                    events_in_current_window: 0,
                    window_start: env.ledger().timestamp(),
                    is_throttled: false,
                    total_cost_billed_stroops: 0,
                }
            }
        };

        // ── TTL helpers ──────────────────────────────────────────────────

        /// Permissionless TTL extension for archival-critical persistent records.
        ///
        /// Anyone may call this to keep active audit records from being archived.
        /// Only persistent keys are extended; temporary telemetry snapshots are
        /// intentionally left to expire.
        pub fn extend_ttl(env: &Env, key: CapacityStorageKey) {
            match key {
                CapacityStorageKey::TelemetrySnapshot(_) => {
                    // Temporary tier: no archival guarantee, no bump.
                }
                _ => Self::bump_persistent_ttl(env, &key),
            }
        }

        /// Extend the TTL of a persistent key if it is below the threshold.
        fn bump_persistent_ttl(env: &Env, key: &CapacityStorageKey) {
            let storage = env.storage().persistent();
            if storage.has(key) {
                storage.extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_LEDGERS);
            }
        }
    }
}
