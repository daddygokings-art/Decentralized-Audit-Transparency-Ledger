#![cfg(test)]

extern crate std;

use crate::capacity_planning::*;
use proptest::prelude::*;
use soroban_sdk::{Address, Env, Symbol, Vec as SorobanVec};
use std::panic;

fn test_metric(env: &Env, index: u32) -> CapacityMetricRecord {
    CapacityMetricRecord {
        timestamp: 1000 + index as u64,
        current_tps: index,
        peak_tps: index,
        storage_bytes_used: index as u64 * 1000,
        active_submitters: index % 100,
        gas_consumed_stroops: index as u64 * 10,
        cpu_utilization_basis_pts: 50,
        memory_mb_used: index,
    }
}

fn test_tier(env: &Env) -> QuotaTier {
    QuotaTier {
        tier_id: 1,
        tier_name: Symbol::new(env, "standard"),
        max_daily_events: 50_000,
        max_burst_tps: 100,
        storage_quota_bytes: 1_000_000_000,
        max_batch_size: 10_000,
        cost_per_m_events_stroops: 5_000_000,
    }
}

fn test_policy(env: &Env) -> CapacityPolicy {
    CapacityPolicy {
        auto_scaling_enabled: true,
        target_cpu_percent: 70,
        target_tps_per_replica: 1_000,
        cooldown_period_seconds: 300,
        min_replicas: 3,
        max_replicas: 100,
        last_scaled_at: env.ledger().timestamp(),
    }
}

fn setup_quota(env: &Env, admin: &Address, submitter: &Address) {
    CapacityPlanning::set_quota_tier(env, admin.clone(), test_tier(env));
    CapacityPlanning::set_submitter_quota(env, admin.clone(), submitter.clone(), 1);
}

// ── Unit / chaos tests ─────────────────────────────────────────────────

#[test]
fn test_telemetry_ring_buffer_bounds() {
    let env = Env::default();
    env.mock_all_auths();
    let caller = Address::random(&env);

    let total: u32 = 1500;
    for i in 0..total {
        CapacityPlanning::record_telemetry(&env, caller.clone(), test_metric(&env, i));
    }

    let count: u32 = env
        .storage()
        .temporary()
        .get(&CapacityStorageKey::TelemetryCount)
        .unwrap_or(0);
    assert_eq!(count, total, "telemetry count must equal records written");

    // Latest record lives at (count - 1) % 1000.
    let latest_idx = (total - 1) % 1000;
    assert!(env
        .storage()
        .temporary()
        .has(&CapacityStorageKey::PackedTelemetrySnapshot(latest_idx)));

    // Slot 0 was overwritten by record 1000.
    assert!(env
        .storage()
        .temporary()
        .has(&CapacityStorageKey::PackedTelemetrySnapshot(0)));
    assert!(!env
        .storage()
        .persistent()
        .has(&CapacityStorageKey::TelemetrySnapshot(0)));
}

#[test]
fn test_legacy_telemetry_count_migrates_to_temporary_storage() {
    let env = Env::default();
    env.mock_all_auths();
    let caller = Address::random(&env);
    let count_key = CapacityStorageKey::TelemetryCount;
    let snapshot_key = CapacityStorageKey::TelemetrySnapshot(5);

    env.storage().persistent().set(&count_key, &5u32);
    env.storage()
        .persistent()
        .set(&snapshot_key, &test_metric(&env, 5));

    CapacityPlanning::record_telemetry(&env, caller, test_metric(&env, 6));

    assert_eq!(env.storage().temporary().get::<_, u32>(&count_key), Some(6));
    assert!(env
        .storage()
        .temporary()
        .has(&CapacityStorageKey::PackedTelemetrySnapshot(5)));
    assert!(!env.storage().persistent().has(&count_key));
    assert!(!env.storage().persistent().has(&snapshot_key));
    assert!(extend_ttl(&env, count_key));
}

#[test]
fn test_quota_consumption_respects_daily_limit() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);
    let submitter = Address::random(&env);
    setup_quota(&env, &admin, &submitter);

    let batch = 10_000u32;
    for _ in 0..5 {
        assert!(CapacityPlanning::consume_quota(&env, submitter.clone(), batch));
    }

    let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
    assert_eq!(record.events_in_current_window, 50_000);
    assert!(!record.is_throttled);

    // The 6th batch would exceed the daily limit.
    let result = panic::catch_unwind(|| {
        CapacityPlanning::consume_quota(&env, submitter.clone(), batch)
    });
    assert!(result.is_err(), "over-quota consumption must be rejected");

    let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
    assert!(record.is_throttled, "throttle flag must persist after rejection");

    // Further consumption in the same window keeps failing.
    let result = panic::catch_unwind(|| {
        CapacityPlanning::consume_quota(&env, submitter.clone(), 1)
    });
    assert!(result.is_err());

    // Advancing the ledger by more than 24 hours resets the window.
    env.ledger().set_timestamp(env.ledger().timestamp() + 86401);
    assert!(CapacityPlanning::consume_quota(&env, submitter.clone(), batch));
    let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
    assert!(!record.is_throttled);
    assert_eq!(record.events_in_current_window, 10_000);
}

#[test]
fn test_quota_cost_accounting_is_deterministic() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);
    let submitter = Address::random(&env);
    setup_quota(&env, &admin, &submitter);

    let batches: [u32; 4] = [1_000, 2_000, 3_000, 4_000];
    let mut expected_cost: u64 = 0;
    for batch in batches {
        assert!(CapacityPlanning::consume_quota(&env, submitter.clone(), batch));
        expected_cost = expected_cost.saturating_add((batch as u64 * 5_000_000) / 1_000_000);
    }

    let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
    assert_eq!(
        record.events_in_current_window,
        10_000,
        "consumed events must accumulate exactly"
    );
    assert_eq!(
        record.total_cost_billed_stroops, expected_cost,
        "billed cost must match the deterministic rate formula"
    );
}

#[test]
fn test_missing_tier_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);
    let submitter = Address::random(&env);

    let result = panic::catch_unwind(|| {
        CapacityPlanning::set_submitter_quota(&env, admin.clone(), submitter.clone(), 999)
    });
    assert!(result.is_err(), "unknown quota tier must be rejected");
}

#[test]
fn test_invalid_recommendation_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let reporter = Address::random(&env);

    let mut rec = ScalingRecommendationRecord {
        timestamp: env.ledger().timestamp(),
        recommended_min_replicas: 0,
        recommended_max_replicas: 5,
        recommended_cpu_millicores: 1_000,
        recommended_memory_mb: 1024,
        forecast_horizon_hours: 24,
        confidence_score_percent: 90,
        estimated_monthly_cost_usd: 100,
        model_digest: BytesN::from_array(&env, [0u8; 32]),
    };
    let result = panic::catch_unwind(|| {
        CapacityPlanning::record_recommendation(&env, reporter.clone(), rec.clone())
    });
    assert!(result.is_err(), "zero min replicas must be rejected");

    rec.recommended_min_replicas = 5;
    rec.recommended_max_replicas = 2;
    let result = panic::catch_unwind(|| {
        CapacityPlanning::record_recommendation(&env, reporter.clone(), rec.clone())
    });
    assert!(result.is_err(), "max below min must be rejected");
}

#[test]
fn test_valid_recommendation_round_trips() {
    let env = Env::default();
    env.mock_all_auths();
    let reporter = Address::random(&env);

    let rec = ScalingRecommendationRecord {
        timestamp: env.ledger().timestamp(),
        recommended_min_replicas: 3,
        recommended_max_replicas: 10,
        recommended_cpu_millicores: 1_000,
        recommended_memory_mb: 4096,
        forecast_horizon_hours: 24,
        confidence_score_percent: 95,
        estimated_monthly_cost_usd: 500,
        model_digest: BytesN::from_array(&env, [1u8; 32]),
    };
    CapacityPlanning::record_recommendation(&env, reporter.clone(), rec.clone());

    let stored = CapacityPlanning::get_latest_recommendation(&env).unwrap();
    assert_eq!(stored, rec);
    assert_eq!(stored.recommended_min_replicas, 3);
    assert_eq!(stored.recommended_max_replicas, 10);
}

#[test]
fn test_invalid_policy_bounds_are_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);

    let mut policy = test_policy(&env);
    policy.min_replicas = 0;
    let result = panic::catch_unwind(|| CapacityPlanning::update_policy(&env, admin.clone(), policy.clone()));
    assert!(result.is_err(), "zero min replicas must be rejected");

    policy.min_replicas = 10;
    policy.max_replicas = 2;
    let result = panic::catch_unwind(|| CapacityPlanning::update_policy(&env, admin.clone(), policy.clone()));
    assert!(result.is_err(), "max below min must be rejected");
}

#[test]
fn test_valid_policy_round_trips() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);

    let policy = test_policy(&env);
    CapacityPlanning::update_policy(&env, admin.clone(), policy.clone());

    let stored = CapacityPlanning::get_policy(&env).unwrap();
    assert_eq!(stored, policy);
}

// ── Property-based tests ───────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { cases: 150, .. ProptestConfig::default() })]

    #[test]
    fn prop_quota_never_exceeds_daily_limit(
        batches in prop::collection::vec(any::<u32>(), 1..=60),
        advances in prop::collection::vec(0u64..=1000, 1..=60),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::random(&env);
        let submitter = Address::random(&env);
        setup_quota(&env, &admin, &submitter);

        let mut now = env.ledger().timestamp();
        let mut expected_events: u64 = 0;
        let mut expected_cost: u64 = 0;

        for (batch, advance) in batches.iter().zip(advances.iter()) {
            now = now.saturating_add(*advance);
            env.ledger().set_timestamp(now);

            let result = panic::catch_unwind(|| {
                CapacityPlanning::consume_quota(&env, submitter.clone(), *batch)
            });

            if let Ok(true) = result {
                expected_events = expected_events.saturating_add(*batch as u64);
                expected_cost = expected_cost.saturating_add((*batch as u64 * 5_000_000) / 1_000_000);
            }

            let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
            // Invariant: window consumption may never exceed the configured daily cap.
            assert!(
                record.events_in_current_window <= 50_000,
                "daily quota must never be exceeded after a successful consume"
            );
        }

        // With sub-day advances the window never resets, so counters match exactly.
        let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
        assert_eq!(record.events_in_current_window, expected_events);
        assert_eq!(record.total_cost_billed_stroops, expected_cost);
    }
}

// ── Concurrency / interleaving tests ───────────────────────────────────

#[test]
fn test_interleaved_submitters_each_respect_quota() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::random(&env);

    let submitters: SorobanVec<Address> = SorobanVec::new(&env);
    for i in 0..5u32 {
        let submitter = Address::random(&env);
        submitters.push_back(submitter.clone());
        CapacityPlanning::set_quota_tier(&env, admin.clone(), test_tier(&env));
        CapacityPlanning::set_submitter_quota(&env, admin.clone(), submitter, 1);
    }

    // Interleave consumption across submitters.
    for round in 0..20u32 {
        for i in 0..submitters.len() {
            let submitter = submitters.get(i).unwrap();
            let _ = panic::catch_unwind(|| {
                CapacityPlanning::consume_quota(&env, submitter.clone(), 1_000)
            });
        }
    }

    // Invariant: every submitter stayed within its daily limit.
    for i in 0..submitters.len() {
        let submitter = submitters.get(i).unwrap();
        let record = CapacityPlanning::get_submitter_quota(&env, submitter.clone()).unwrap();
        assert!(
            record.events_in_current_window <= 50_000,
            "interleaved submitter must respect its daily quota"
        );
    }
}
