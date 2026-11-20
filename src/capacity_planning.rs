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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedCapacityMetric {
    timestamp: u64,
    packed_rates: u64,
    storage_bytes_used: u64,
    active_submitters: u32,
    gas_consumed_stroops: u64,
    memory_mb_used: u32,
}

impl From<CapacityMetricRecord> for PackedCapacityMetric {
    fn from(record: CapacityMetricRecord) -> Self {
        Self {
            timestamp: record.timestamp,
            packed_rates: record.current_tps as u64
                | ((record.peak_tps as u64) << 20)
                | ((record.cpu_utilization_basis_pts as u64) << 40),
            storage_bytes_used: record.storage_bytes_used,
            active_submitters: record.active_submitters,
            gas_consumed_stroops: record.gas_consumed_stroops,
            memory_mb_used: record.memory_mb_used,
        }
    }
}

impl From<PackedCapacityMetric> for CapacityMetricRecord {
    fn from(record: PackedCapacityMetric) -> Self {
        Self {
            timestamp: record.timestamp,
            current_tps: (record.packed_rates & 0xFFFFF) as u32,
            peak_tps: ((record.packed_rates >> 20) & 0xFFFFF) as u32,
            storage_bytes_used: record.storage_bytes_used,
            active_submitters: record.active_submitters,
            gas_consumed_stroops: record.gas_consumed_stroops,
            cpu_utilization_basis_pts: ((record.packed_rates >> 40) & 0x3FFF) as u32,
            memory_mb_used: record.memory_mb_used,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedQuotaTier {
    tier_and_burst: u64,
    tier_name: Symbol,
    max_daily_events: u64,
    storage_quota_bytes: u64,
    max_batch_size: u32,
    cost_per_m_events_stroops: u64,
}

impl From<QuotaTier> for PackedQuotaTier {
    fn from(tier: QuotaTier) -> Self {
        Self {
            tier_and_burst: tier.tier_id as u64 | ((tier.max_burst_tps as u64) << 32),
            tier_name: tier.tier_name,
            max_daily_events: tier.max_daily_events,
            storage_quota_bytes: tier.storage_quota_bytes,
            max_batch_size: tier.max_batch_size,
            cost_per_m_events_stroops: tier.cost_per_m_events_stroops,
        }
    }
}

impl From<PackedQuotaTier> for QuotaTier {
    fn from(tier: PackedQuotaTier) -> Self {
        Self {
            tier_id: tier.tier_and_burst as u32,
            tier_name: tier.tier_name,
            max_daily_events: tier.max_daily_events,
            max_burst_tps: (tier.tier_and_burst >> 32) as u32,
            storage_quota_bytes: tier.storage_quota_bytes,
            max_batch_size: tier.max_batch_size,
            cost_per_m_events_stroops: tier.cost_per_m_events_stroops,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedSubmitterQuotaRecord {
    submitter: Address,
    tier_and_throttle: u64,
    events_in_current_window: u64,
    window_start: u64,
    total_cost_billed_stroops: u64,
}

impl From<SubmitterQuotaRecord> for PackedSubmitterQuotaRecord {
    fn from(record: SubmitterQuotaRecord) -> Self {
        Self {
            submitter: record.submitter,
            tier_and_throttle: record.tier_id as u64 | ((record.is_throttled as u64) << 32),
            events_in_current_window: record.events_in_current_window,
            window_start: record.window_start,
            total_cost_billed_stroops: record.total_cost_billed_stroops,
        }
    }
}

impl From<PackedSubmitterQuotaRecord> for SubmitterQuotaRecord {
    fn from(record: PackedSubmitterQuotaRecord) -> Self {
        Self {
            submitter: record.submitter,
            tier_id: record.tier_and_throttle as u32,
            events_in_current_window: record.events_in_current_window,
            window_start: record.window_start,
            is_throttled: (record.tier_and_throttle >> 32) & 1 == 1,
            total_cost_billed_stroops: record.total_cost_billed_stroops,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedScalingRecommendationRecord {
    timestamp: u64,
    replicas: u32,
    recommended_cpu_millicores: u32,
    recommended_memory_mb: u32,
    forecast_horizon_hours: u32,
    confidence_score_percent: u32,
    estimated_monthly_cost_usd: u32,
    model_digest: BytesN<32>,
}

impl From<ScalingRecommendationRecord> for PackedScalingRecommendationRecord {
    fn from(record: ScalingRecommendationRecord) -> Self {
        Self {
            timestamp: record.timestamp,
            replicas: record.recommended_min_replicas | (record.recommended_max_replicas << 14),
            recommended_cpu_millicores: record.recommended_cpu_millicores,
            recommended_memory_mb: record.recommended_memory_mb,
            forecast_horizon_hours: record.forecast_horizon_hours,
            confidence_score_percent: record.confidence_score_percent,
            estimated_monthly_cost_usd: record.estimated_monthly_cost_usd,
            model_digest: record.model_digest,
        }
    }
}

impl From<PackedScalingRecommendationRecord> for ScalingRecommendationRecord {
    fn from(record: PackedScalingRecommendationRecord) -> Self {
        Self {
            timestamp: record.timestamp,
            recommended_min_replicas: record.replicas & 0x3FFF,
            recommended_max_replicas: (record.replicas >> 14) & 0x3FFF,
            recommended_cpu_millicores: record.recommended_cpu_millicores,
            recommended_memory_mb: record.recommended_memory_mb,
            forecast_horizon_hours: record.forecast_horizon_hours,
            confidence_score_percent: record.confidence_score_percent,
            estimated_monthly_cost_usd: record.estimated_monthly_cost_usd,
            model_digest: record.model_digest,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedCapacityPolicy {
    scaling_flags: u64,
    target_tps_per_replica: u32,
    cooldown_period_seconds: u32,
    last_scaled_at: u64,
}

impl From<CapacityPolicy> for PackedCapacityPolicy {
    fn from(policy: CapacityPolicy) -> Self {
        Self {
            scaling_flags: policy.auto_scaling_enabled as u64
                | ((policy.target_cpu_percent as u64) << 1)
                | ((policy.min_replicas as u64) << 15)
                | ((policy.max_replicas as u64) << 29),
            target_tps_per_replica: policy.target_tps_per_replica,
            cooldown_period_seconds: policy.cooldown_period_seconds,
            last_scaled_at: policy.last_scaled_at,
        }
    }
}

impl From<PackedCapacityPolicy> for CapacityPolicy {
    fn from(policy: PackedCapacityPolicy) -> Self {
        Self {
            auto_scaling_enabled: policy.scaling_flags & 1 == 1,
            target_cpu_percent: ((policy.scaling_flags >> 1) & 0x3FFF) as u32,
            target_tps_per_replica: policy.target_tps_per_replica,
            cooldown_period_seconds: policy.cooldown_period_seconds,
            min_replicas: ((policy.scaling_flags >> 15) & 0x3FFF) as u32,
            max_replicas: ((policy.scaling_flags >> 29) & 0x3FFF) as u32,
            last_scaled_at: policy.last_scaled_at,
        }
    }
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
    PackedTelemetrySnapshot(u32),
    PackedTier(u32),
    PackedSubmitterQuota(Address),
    PackedLatestRecommendation,
    PackedPolicy,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CapacityStorageTier {
    Persistent,
    Temporary,
    Instance,
}

pub const TTL_BUMP_THRESHOLD_LEDGERS: u32 = 17_280;
pub const PERSISTENT_TTL_TARGET_LEDGERS: u32 = 518_400;
pub const TEMPORARY_TTL_TARGET_LEDGERS: u32 = 120_960;
pub const INSTANCE_TTL_TARGET_LEDGERS: u32 = 518_400;

pub fn storage_tier_for(key: &CapacityStorageKey) -> CapacityStorageTier {
    match key {
        CapacityStorageKey::TelemetrySnapshot(_) | CapacityStorageKey::Policy => {
            CapacityStorageTier::Persistent
        }
        CapacityStorageKey::TelemetryCount | CapacityStorageKey::PackedTelemetrySnapshot(_) => {
            CapacityStorageTier::Temporary
        }
        CapacityStorageKey::PackedPolicy => CapacityStorageTier::Instance,
        CapacityStorageKey::Tier(_)
        | CapacityStorageKey::SubmitterQuota(_)
        | CapacityStorageKey::LatestRecommendation
        | CapacityStorageKey::PackedTier(_)
        | CapacityStorageKey::PackedSubmitterQuota(_)
        | CapacityStorageKey::PackedLatestRecommendation => CapacityStorageTier::Persistent,
    }
}

/// Permissionlessly extend the TTL of an active capacity-planning record.
/// Legacy keys are included so existing deployments can renew entries while
/// their values are lazily migrated to the compact layout.
pub fn extend_ttl(env: &Env, key: CapacityStorageKey) -> bool {
    let persistent = env.storage().persistent();
    let temporary = env.storage().temporary();
    match key {
        CapacityStorageKey::TelemetrySnapshot(index) => {
            let legacy = CapacityStorageKey::TelemetrySnapshot(index);
            let packed = CapacityStorageKey::PackedTelemetrySnapshot(index);
            let mut found = false;
            if persistent.has(&legacy) {
                persistent.extend_ttl(
                    &legacy,
                    TTL_BUMP_THRESHOLD_LEDGERS,
                    PERSISTENT_TTL_TARGET_LEDGERS,
                );
                found = true;
            }
            if temporary.has(&packed) {
                temporary.extend_ttl(
                    &packed,
                    TTL_BUMP_THRESHOLD_LEDGERS,
                    TEMPORARY_TTL_TARGET_LEDGERS,
                );
                found = true;
            }
            found
        }
        CapacityStorageKey::TelemetryCount => {
            let legacy = CapacityStorageKey::TelemetryCount;
            let mut found = false;
            if persistent.has(&legacy) {
                persistent.extend_ttl(
                    &legacy,
                    TTL_BUMP_THRESHOLD_LEDGERS,
                    PERSISTENT_TTL_TARGET_LEDGERS,
                );
                found = true;
            }
            if temporary.has(&legacy) {
                temporary.extend_ttl(
                    &legacy,
                    TTL_BUMP_THRESHOLD_LEDGERS,
                    TEMPORARY_TTL_TARGET_LEDGERS,
                );
                found = true;
            }
            found
        }
        CapacityStorageKey::Tier(id) => extend_persistent_pair(
            env,
            &CapacityStorageKey::Tier(id),
            &CapacityStorageKey::PackedTier(id),
        ),
        CapacityStorageKey::SubmitterQuota(address) => extend_persistent_pair(
            env,
            &CapacityStorageKey::SubmitterQuota(address.clone()),
            &CapacityStorageKey::PackedSubmitterQuota(address),
        ),
        CapacityStorageKey::LatestRecommendation => extend_persistent_pair(
            env,
            &CapacityStorageKey::LatestRecommendation,
            &CapacityStorageKey::PackedLatestRecommendation,
        ),
        CapacityStorageKey::Policy => {
            let mut found = false;
            if persistent.has(&CapacityStorageKey::Policy) {
                persistent.extend_ttl(
                    &CapacityStorageKey::Policy,
                    TTL_BUMP_THRESHOLD_LEDGERS,
                    PERSISTENT_TTL_TARGET_LEDGERS,
                );
                found = true;
            }
            found | extend_instance_policy(env)
        }
        CapacityStorageKey::PackedTelemetrySnapshot(index) => {
            extend_temporary(env, &CapacityStorageKey::PackedTelemetrySnapshot(index))
        }
        CapacityStorageKey::PackedTier(id) => {
            extend_persistent(env, &CapacityStorageKey::PackedTier(id))
        }
        CapacityStorageKey::PackedSubmitterQuota(address) => {
            extend_persistent(env, &CapacityStorageKey::PackedSubmitterQuota(address))
        }
        CapacityStorageKey::PackedLatestRecommendation => {
            extend_persistent(env, &CapacityStorageKey::PackedLatestRecommendation)
        }
        CapacityStorageKey::PackedPolicy => extend_instance_policy(env),
    }
}

fn extend_persistent_pair(env: &Env, legacy: &CapacityStorageKey, packed: &CapacityStorageKey) -> bool {
    extend_persistent(env, legacy) | extend_persistent(env, packed)
}

fn extend_persistent(env: &Env, key: &CapacityStorageKey) -> bool {
    let storage = env.storage().persistent();
    if storage.has(key) {
        storage.extend_ttl(key, TTL_BUMP_THRESHOLD_LEDGERS, PERSISTENT_TTL_TARGET_LEDGERS);
        true
    } else {
        false
    }
}

fn extend_temporary(env: &Env, key: &CapacityStorageKey) -> bool {
    let storage = env.storage().temporary();
    if storage.has(key) {
        storage.extend_ttl(key, TTL_BUMP_THRESHOLD_LEDGERS, TEMPORARY_TTL_TARGET_LEDGERS);
        true
    } else {
        false
    }
}

fn extend_instance_policy(env: &Env) -> bool {
    let storage = env.storage().instance();
    if storage.has(&CapacityStorageKey::PackedPolicy) {
        storage.extend_ttl(TTL_BUMP_THRESHOLD_LEDGERS, INSTANCE_TTL_TARGET_LEDGERS);
        true
    } else {
        false
    }
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

        let count_key = CapacityStorageKey::TelemetryCount;
        let temporary = env.storage().temporary();
        let persistent = env.storage().persistent();
        let count: u32 = if let Some(count) = temporary.get(&count_key) {
            count
        } else if let Some(count) = persistent.get(&count_key) {
            temporary.set(&count_key, &count);
            temporary.extend_ttl(
                &count_key,
                TTL_BUMP_THRESHOLD_LEDGERS,
                TEMPORARY_TTL_TARGET_LEDGERS,
            );
            persistent.remove(&count_key);
            count
        } else {
            0
        };

        // Ring buffer: deterministic, bounded snapshot count.
        let next_idx = count % MAX_TELEMETRY_SNAPSHOTS;
        let key = CapacityStorageKey::PackedTelemetrySnapshot(next_idx);
        temporary.set(&key, &PackedCapacityMetric::from(metric));
        temporary.extend_ttl(
            &key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            TEMPORARY_TTL_TARGET_LEDGERS,
        );
        persistent.remove(&CapacityStorageKey::TelemetrySnapshot(next_idx));

        let next_count = count.saturating_add(1);
        temporary.set(&count_key, &next_count);
        temporary.extend_ttl(
            &count_key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            TEMPORARY_TTL_TARGET_LEDGERS,
        );
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

        let legacy_key = CapacityStorageKey::Tier(tier.tier_id);
        let packed_key = CapacityStorageKey::PackedTier(tier.tier_id);
        let storage = env.storage().persistent();
        storage.set(&packed_key, &PackedQuotaTier::from(tier));
        storage.extend_ttl(
            &packed_key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            PERSISTENT_TTL_TARGET_LEDGERS,
        );
        storage.remove(&legacy_key);
    }

    /// Assign a submitter to a quota tier
    pub fn set_submitter_quota(
        env: &Env,
        admin: Address,
        submitter: Address,
        tier_id: u32,
    ) {
        admin.require_auth();

        if read_quota_tier(env, tier_id).is_none() {
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

        store_submitter_quota(env, &record);
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
        let packed_sub_key = CapacityStorageKey::PackedSubmitterQuota(submitter.clone());
        let persistent = env.storage().persistent();
        let mut sub_record: SubmitterQuotaRecord = match persistent
            .get::<_, PackedSubmitterQuotaRecord>(&packed_sub_key)
        {
            Some(record) => {
                extend_ttl(env, packed_sub_key);
                record.into()
            }
            None => match persistent.get::<_, SubmitterQuotaRecord>(&sub_key) {
                Some(record) => {
                    persistent.set(&packed_sub_key, &PackedSubmitterQuotaRecord::from(record.clone()));
                    persistent.extend_ttl(
                        &packed_sub_key,
                        TTL_BUMP_THRESHOLD_LEDGERS,
                        PERSISTENT_TTL_TARGET_LEDGERS,
                    );
                    persistent.remove(&sub_key);
                    record
                }
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
            },
        };

        let tier: QuotaTier = read_quota_tier(env, sub_record.tier_id).unwrap_or(QuotaTier {
            tier_id: 1,
            tier_name: Symbol::new(env, "standard"),
            max_daily_events: 100_000,
            max_burst_tps: 50,
            storage_quota_bytes: 1_000_000_000,
            max_batch_size: 100,
            cost_per_m_events_stroops: 5_000_000,
        });

        // Enforce per-batch size cap from the tier's own policy.
        if event_count > tier.max_batch_size {
            panic_with_error!(env, CapacityError::InvalidRecommendation);
        }

        let now = env.ledger().timestamp();
        // 24-hour sliding window reset
        if now >= sub_record.window_start + 86400 {
            sub_record.events_in_current_window = 0;
            sub_record.window_start = now;
            sub_record.is_throttled = false;
        }

        let added = event_count as u64;
        let projected = sub_record
            .events_in_current_window
            .checked_add(added)
            .unwrap_or(u64::MAX);

        if projected > tier.max_daily_events {
            sub_record.is_throttled = true;
            store_submitter_quota(env, &sub_record);
            panic_with_error!(env, CapacityError::QuotaExceeded);
        }

        sub_record.events_in_current_window = projected;

        // Overflow-safe batch cost: (event_count * cost_per_m) / 1_000_000
        let batch_cost = added
            .checked_mul(tier.cost_per_m_events_stroops)
            .and_then(|v| v.checked_div(1_000_000))
            .unwrap_or(0);

        let new_total = sub_record
            .total_cost_billed_stroops
            .checked_add(batch_cost)
            .unwrap_or(u64::MAX);
        sub_record.total_cost_billed_stroops = new_total;

        store_submitter_quota(env, &sub_record);
        true
    }

    /// Record predictive scaling recommendation generated by ML forecasting model
    pub fn record_recommendation(
        env: &Env,
        reporter: Address,
        rec: ScalingRecommendationRecord,
    ) {
        reporter.require_auth();

        if rec.recommended_min_replicas == 0
            || rec.recommended_max_replicas < rec.recommended_min_replicas
            || rec.recommended_max_replicas > MAX_REPLICAS
            || rec.recommended_cpu_millicores == 0
            || rec.recommended_memory_mb > MAX_MEMORY_MB
            || rec.forecast_horizon_hours == 0
            || rec.forecast_horizon_hours > MAX_FORECAST_HOURS
            || rec.confidence_score_percent > MAX_CONFIDENCE_PCT
        {
            panic_with_error!(env, CapacityError::InvalidRecommendation);
        }

        let key = CapacityStorageKey::PackedLatestRecommendation;
        let storage = env.storage().persistent();
        storage.set(&key, &PackedScalingRecommendationRecord::from(rec));
        storage.extend_ttl(
            &key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            PERSISTENT_TTL_TARGET_LEDGERS,
        );
        storage.remove(&CapacityStorageKey::LatestRecommendation);
    }

    /// Configure auto-scaling policy parameters
    pub fn update_policy(
        env: &Env,
        admin: Address,
        policy: CapacityPolicy,
    ) {
        admin.require_auth();

        if policy.min_replicas == 0
            || policy.max_replicas < policy.min_replicas
            || policy.max_replicas > MAX_REPLICAS
            || policy.min_replicas > MAX_REPLICAS
            || policy.target_cpu_percent > MAX_CPU_BASIS_PTS
            || policy.cooldown_period_seconds == 0
        {
            panic_with_error!(env, CapacityError::InvalidPolicyBounds);
        }

        let key = CapacityStorageKey::PackedPolicy;
        let storage = env.storage().instance();
        storage.set(&key, &PackedCapacityPolicy::from(policy));
        storage.extend_ttl(TTL_BUMP_THRESHOLD_LEDGERS, INSTANCE_TTL_TARGET_LEDGERS);
        env.storage().persistent().remove(&CapacityStorageKey::Policy);
    }

    /// Query latest scaling recommendation
    pub fn get_latest_recommendation(env: &Env) -> Option<ScalingRecommendationRecord> {
        let storage = env.storage().persistent();
        if let Some(record) = storage.get::<_, PackedScalingRecommendationRecord>(
            &CapacityStorageKey::PackedLatestRecommendation,
        ) {
            extend_ttl(env, CapacityStorageKey::PackedLatestRecommendation);
            Some(record.into())
        } else if let Some(record) =
            storage.get::<_, ScalingRecommendationRecord>(&CapacityStorageKey::LatestRecommendation)
        {
            storage.set(
                &CapacityStorageKey::PackedLatestRecommendation,
                &PackedScalingRecommendationRecord::from(record.clone()),
            );
            storage.extend_ttl(
                &CapacityStorageKey::PackedLatestRecommendation,
                TTL_BUMP_THRESHOLD_LEDGERS,
                PERSISTENT_TTL_TARGET_LEDGERS,
            );
            storage.remove(&CapacityStorageKey::LatestRecommendation);
            Some(record)
        } else {
            None
        }
    }

    /// Query active auto-scaling policy
    pub fn get_policy(env: &Env) -> Option<CapacityPolicy> {
        let instance = env.storage().instance();
        if let Some(policy) = instance.get::<_, PackedCapacityPolicy>(&CapacityStorageKey::PackedPolicy) {
            extend_ttl(env, CapacityStorageKey::PackedPolicy);
            Some(policy.into())
        } else if let Some(policy) = env
            .storage()
            .persistent()
            .get::<_, CapacityPolicy>(&CapacityStorageKey::Policy)
        {
            instance.set(
                &CapacityStorageKey::PackedPolicy,
                &PackedCapacityPolicy::from(policy.clone()),
            );
            instance.extend_ttl(TTL_BUMP_THRESHOLD_LEDGERS, INSTANCE_TTL_TARGET_LEDGERS);
            env.storage().persistent().remove(&CapacityStorageKey::Policy);
            Some(policy)
        } else {
            None
        }
    }

    /// Query submitter quota record
    pub fn get_submitter_quota(env: &Env, submitter: Address) -> Option<SubmitterQuotaRecord> {
        read_submitter_quota(env, submitter)
    }

    /// Query quota tier definition
    pub fn get_quota_tier(env: &Env, tier_id: u32) -> Option<QuotaTier> {
        read_quota_tier(env, tier_id)
    }
}

fn read_quota_tier(env: &Env, tier_id: u32) -> Option<QuotaTier> {
    let storage = env.storage().persistent();
    let packed_key = CapacityStorageKey::PackedTier(tier_id);
    if let Some(tier) = storage.get::<_, PackedQuotaTier>(&packed_key) {
        extend_ttl(env, packed_key);
        Some(tier.into())
    } else if let Some(tier) = storage.get::<_, QuotaTier>(&CapacityStorageKey::Tier(tier_id)) {
        storage.set(&packed_key, &PackedQuotaTier::from(tier.clone()));
        storage.extend_ttl(
            &packed_key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            PERSISTENT_TTL_TARGET_LEDGERS,
        );
        storage.remove(&CapacityStorageKey::Tier(tier_id));
        Some(tier)
    } else {
        None
    }
}

fn store_submitter_quota(env: &Env, record: &SubmitterQuotaRecord) {
    let legacy_key = CapacityStorageKey::SubmitterQuota(record.submitter.clone());
    let packed_key = CapacityStorageKey::PackedSubmitterQuota(record.submitter.clone());
    let storage = env.storage().persistent();
    storage.set(&packed_key, &PackedSubmitterQuotaRecord::from(record.clone()));
    storage.extend_ttl(
        &packed_key,
        TTL_BUMP_THRESHOLD_LEDGERS,
        PERSISTENT_TTL_TARGET_LEDGERS,
    );
    storage.remove(&legacy_key);
}

fn read_submitter_quota(env: &Env, submitter: Address) -> Option<SubmitterQuotaRecord> {
    let legacy_key = CapacityStorageKey::SubmitterQuota(submitter.clone());
    let packed_key = CapacityStorageKey::PackedSubmitterQuota(submitter);
    let storage = env.storage().persistent();
    if let Some(record) = storage.get::<_, PackedSubmitterQuotaRecord>(&packed_key) {
        extend_ttl(env, packed_key);
        Some(record.into())
    } else if let Some(record) = storage.get::<_, SubmitterQuotaRecord>(&legacy_key) {
        storage.set(&packed_key, &PackedSubmitterQuotaRecord::from(record.clone()));
        storage.extend_ttl(
            &packed_key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            PERSISTENT_TTL_TARGET_LEDGERS,
        );
        storage.remove(&legacy_key);
        Some(record)
    } else {
        None
    }
}
