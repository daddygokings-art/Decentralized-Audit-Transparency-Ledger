//! Comprehensive tests for green computing and carbon tracking (Issue #508).
//!
//! Tests cover:
//! - Module initialization and configuration
//! - Region profile management and green-score computation
//! - Region ranking and recommendation
//! - Carbon footprint recording and carbon emission math
//! - Aggregate statistics
//! - Carbon-aware auto-scaling policies
//! - Green scheduling windows
//! - Carbon budget management and enforcement
//! - Error cases and boundary conditions

#[cfg(test)]
mod tests {
    use crate::green_computing::{
        compute_green_score, EnergySourceMix, GreenComputingContract, GreenComputingContractClient,
        OperationCategory, ScalingDirection,
    };
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Bytes, Env, Symbol};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn setup() -> (Env, GreenComputingContractClient<'static>, Address) {
        let env = Env::default();
        let cid = env.register(GreenComputingContract, ());
        let client = GreenComputingContractClient::new(&env, &cid);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    fn init(
        env: &Env,
        client: &GreenComputingContractClient,
        owner: &Address,
    ) {
        env.mock_all_auths();
        client.initialize(owner, &300, &600, &30, &false);
    }

    /// Register a test region with sensible defaults.
    fn add_region(
        env: &Env,
        client: &GreenComputingContractClient,
        owner: &Address,
        region_id: Symbol,
        intensity: u32,
        renewable: u32,
    ) {
        env.mock_all_auths();
        client.upsert_region(
            owner,
            &region_id,
            &intensity,
            &renewable,
            &120,
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(env, b"Test Region"),
            &true,
        );
    }

    // ── 1. compute_green_score unit tests ─────────────────────────────────────

    #[test]
    fn test_green_score_max_renewable_low_intensity_good_pue() {
        // 100 % renewable, 0 mg/kWh intensity, PUE 1.00 → score = 40 + 40 + 20 = 100
        let score = compute_green_score(1, 100, 100);
        // intensity=1, just above 0 → (999/1000)*40 ≈ 39; renew=40; pue=20 → 99
        assert!(score >= 99);
    }

    #[test]
    fn test_green_score_perfect_components() {
        // intensity near 0, 100% renewable, PUE 1.00
        // renew: 100*40/100 = 40
        // intensity: (1000-1)*40/1000 = 39
        // pue: (200-0)*20/200 = 20
        // total = 99
        let score = compute_green_score(1, 100, 100);
        assert_eq!(score, 99);
    }

    #[test]
    fn test_green_score_fossil_dominant_high_intensity_bad_pue() {
        // 0 % renewable, 1000 mg/kWh intensity (worst), PUE 3.00
        // renew: 0; intensity: 0; pue: 0
        let score = compute_green_score(1000, 0, 300);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_green_score_mid_renewable() {
        // 50 % renewable, 500 mg/kWh, PUE 1.50
        // renew: 50*40/100 = 20
        // intensity: (1000-500)*40/1000 = 20
        // pue: (200-50)*20/200 = 15
        let score = compute_green_score(500, 50, 150);
        assert_eq!(score, 55);
    }

    #[test]
    fn test_green_score_intensity_clamped_at_zero_above_1000() {
        let score = compute_green_score(2000, 50, 150);
        // intensity component = 0 (clamped)
        // renew = 20, pue = 15
        assert_eq!(score, 35);
    }

    #[test]
    fn test_green_score_pue_clamped_at_zero_above_300() {
        let score = compute_green_score(500, 50, 400);
        // pue component = 0 (clamped)
        // renew = 20, intensity = 20
        assert_eq!(score, 40);
    }

    // ── 2. Initialization tests ───────────────────────────────────────────────

    #[test]
    fn test_initialize_success() {
        let (env, client, owner) = setup();
        env.mock_all_auths();
        let config = client.initialize(&owner, &300, &600, &30, &false);
        assert_eq!(config.owner, owner);
        assert_eq!(config.green_intensity_threshold_mg, 300);
        assert_eq!(config.red_intensity_threshold_mg, 600);
        assert_eq!(config.min_renewable_percent, 30);
        assert!(!config.enforce_budgets);
    }

    #[test]
    #[should_panic]
    fn test_initialize_twice_fails() {
        let (env, client, owner) = setup();
        env.mock_all_auths();
        client.initialize(&owner, &300, &600, &30, &false);
        client.initialize(&owner, &300, &600, &30, &false);
    }

    #[test]
    #[should_panic]
    fn test_initialize_invalid_renewable_percent() {
        let (env, client, owner) = setup();
        env.mock_all_auths();
        client.initialize(&owner, &300, &600, &101, &false);
    }

    // ── 3. Region management tests ────────────────────────────────────────────

    #[test]
    fn test_upsert_region_creates_profile() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        let profile = client.upsert_region(
            &owner,
            &symbol_short!("eu_west"),
            &200,
            &80,
            &110,
            &EnergySourceMix::MajorityRenewable,
            &Bytes::from_slice(&env, b"EU West"),
            &true,
        );

        assert_eq!(profile.region_id, symbol_short!("eu_west"));
        assert_eq!(profile.carbon_intensity_mg_per_kwh, 200);
        assert_eq!(profile.renewable_percent, 80);
        assert_eq!(profile.pue_x100, 110);
        assert!(profile.accepts_green_shifts);
        // green_score: renew=32, intensity=(800*40/1000)=32, pue=(200-10)*20/200=19 → 83
        assert_eq!(profile.green_score, 83);
    }

    #[test]
    fn test_upsert_region_updates_existing() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.upsert_region(
            &owner,
            &symbol_short!("us_east"),
            &500,
            &30,
            &140,
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(&env, b"US East"),
            &true,
        );

        // Update it with better numbers
        let updated = client.upsert_region(
            &owner,
            &symbol_short!("us_east"),
            &200,
            &70,
            &120,
            &EnergySourceMix::MajorityRenewable,
            &Bytes::from_slice(&env, b"US East Updated"),
            &true,
        );

        assert_eq!(updated.carbon_intensity_mg_per_kwh, 200);
        assert_eq!(updated.renewable_percent, 70);
    }

    #[test]
    #[should_panic]
    fn test_upsert_region_zero_intensity_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.upsert_region(
            &owner,
            &symbol_short!("bad"),
            &0, // invalid
            &50,
            &120,
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(&env, b"bad"),
            &true,
        );
    }

    #[test]
    #[should_panic]
    fn test_upsert_region_renewable_over_100_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.upsert_region(
            &owner,
            &symbol_short!("bad"),
            &200,
            &101, // invalid
            &120,
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(&env, b"bad"),
            &true,
        );
    }

    #[test]
    #[should_panic]
    fn test_upsert_region_pue_below_100_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.upsert_region(
            &owner,
            &symbol_short!("bad"),
            &200,
            &50,
            &99, // invalid, PUE < 1.0
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(&env, b"bad"),
            &true,
        );
    }

    #[test]
    #[should_panic]
    fn test_upsert_region_non_owner_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let not_owner = Address::generate(&env);
        env.mock_all_auths();
        client.upsert_region(
            &not_owner, // not the owner
            &symbol_short!("bad"),
            &200,
            &50,
            &120,
            &EnergySourceMix::MixedGrid,
            &Bytes::from_slice(&env, b"bad"),
            &true,
        );
    }

    #[test]
    fn test_get_region_success() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("ap_se"), 400, 40);

        let profile = client.get_region(&symbol_short!("ap_se"));
        assert_eq!(profile.region_id, symbol_short!("ap_se"));
        assert_eq!(profile.carbon_intensity_mg_per_kwh, 400);
    }

    #[test]
    #[should_panic]
    fn test_get_region_not_found_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        client.get_region(&symbol_short!("ghost"));
    }

    // ── 4. Region ranking tests ───────────────────────────────────────────────

    #[test]
    fn test_ranked_regions_sorted_by_green_score() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        // Add regions with known scores
        // eu_west: intensity=200, renewable=80, pue=110 → score=83
        // us_east: intensity=600, renewable=20, pue=180 → lower score
        // ap_se:   intensity=400, renewable=50, pue=140 → mid score
        env.mock_all_auths();
        client.upsert_region(&owner, &symbol_short!("eu_west"), &200, &80, &110,
            &EnergySourceMix::MajorityRenewable, &Bytes::from_slice(&env, b"EU"), &true);
        client.upsert_region(&owner, &symbol_short!("us_east"), &600, &20, &180,
            &EnergySourceMix::FossilDominant, &Bytes::from_slice(&env, b"US"), &true);
        client.upsert_region(&owner, &symbol_short!("ap_se"), &400, &50, &140,
            &EnergySourceMix::MixedGrid, &Bytes::from_slice(&env, b"AP"), &true);

        let ranked = client.ranked_regions();
        assert_eq!(ranked.len(), 3);
        // First region should be greenest (eu_west)
        assert_eq!(ranked.get(0).unwrap(), symbol_short!("eu_west"));
        // Last should be dirtiest (us_east)
        assert_eq!(ranked.get(2).unwrap(), symbol_short!("us_east"));
    }

    #[test]
    fn test_recommend_region_returns_greenest_accepting_shifts() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        // eu_west: score=83, accepts shifts
        client.upsert_region(&owner, &symbol_short!("eu_west"), &200, &80, &110,
            &EnergySourceMix::MajorityRenewable, &Bytes::from_slice(&env, b"EU"), &true);
        // us_west: very green but does NOT accept shifts
        client.upsert_region(&owner, &symbol_short!("us_west"), &50, &100, &100,
            &EnergySourceMix::FullyRenewable, &Bytes::from_slice(&env, b"US West"), &false);

        // us_west would be greenest but doesn't accept shifts → eu_west recommended
        let rec = client.recommend_region(&50);
        assert_eq!(rec.len(), 1);
        assert_eq!(rec.get(0).unwrap(), symbol_short!("eu_west"));
    }

    #[test]
    fn test_recommend_region_returns_empty_if_none_qualifies() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        // Score will be low; require 95 minimum
        client.upsert_region(&owner, &symbol_short!("us_east"), &600, &20, &180,
            &EnergySourceMix::FossilDominant, &Bytes::from_slice(&env, b"US"), &true);

        let rec = client.recommend_region(&95);
        assert_eq!(rec.len(), 0);
    }

    // ── 5. Footprint recording tests ──────────────────────────────────────────

    #[test]
    fn test_record_footprint_basic() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("us_east"), 500, 30);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        let id = client.record_footprint(
            &submitter,
            &symbol_short!("us_east"),
            &OperationCategory::ContractInvocation,
            &symbol_short!("invoke"),
            &100,
            &Bytes::from_slice(&env, b"test"),
        );

        assert_eq!(id, 0);
        assert_eq!(client.total_footprints(), 1);
    }

    #[test]
    fn test_record_footprint_carbon_math() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        // intensity = 500 mg/kWh
        add_region(&env, &client, &owner, symbol_short!("us_east"), 500, 30);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        let id = client.record_footprint(
            &submitter,
            &symbol_short!("us_east"),
            &OperationCategory::ContractInvocation,
            &symbol_short!("invoke"),
            &1000, // 1000 mWh = 1 Wh
            &Bytes::from_slice(&env, b""),
        );

        let record = client.get_footprint(&id);
        // carbon_ug = 1000 * 500 = 500_000 μg CO₂e
        assert_eq!(record.carbon_ug_co2e, 500_000);
        assert_eq!(record.energy_mwh, 1000);
    }

    #[test]
    fn test_record_multiple_footprints_sequential_ids() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let submitter = Address::generate(&env);
        env.mock_all_auths();

        for _ in 0..5 {
            client.record_footprint(
                &submitter,
                &symbol_short!("eu_west"),
                &OperationCategory::EventEmission,
                &symbol_short!("emit"),
                &50,
                &Bytes::from_slice(&env, b""),
            );
        }

        assert_eq!(client.total_footprints(), 5);
        // IDs are sequential
        assert_eq!(client.get_footprint(&0).id, 0);
        assert_eq!(client.get_footprint(&4).id, 4);
    }

    #[test]
    #[should_panic]
    fn test_record_footprint_zero_energy_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("us_east"), 500, 30);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        client.record_footprint(
            &submitter,
            &symbol_short!("us_east"),
            &OperationCategory::StorageWrite,
            &symbol_short!("write"),
            &0, // invalid
            &Bytes::from_slice(&env, b""),
        );
    }

    #[test]
    #[should_panic]
    fn test_record_footprint_unknown_region_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        client.record_footprint(
            &submitter,
            &symbol_short!("ghost"),
            &OperationCategory::ContractInvocation,
            &symbol_short!("invoke"),
            &100,
            &Bytes::from_slice(&env, b""),
        );
    }

    #[test]
    #[should_panic]
    fn test_get_footprint_not_found_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        client.get_footprint(&99);
    }

    // ── 6. Aggregate statistics tests ─────────────────────────────────────────

    #[test]
    fn test_aggregate_stats_initial_zero() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let stats = client.get_aggregate_stats();
        assert_eq!(stats.total_operations, 0);
        assert_eq!(stats.total_energy_mwh, 0);
        assert_eq!(stats.total_carbon_ug_co2e, 0);
    }

    #[test]
    fn test_aggregate_stats_after_records() {
        let (env, client, owner) = setup();
        // green threshold = 300, red = 600
        init(&env, &client, &owner);
        // intensity = 200 (below green threshold → counts as green)
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let submitter = Address::generate(&env);
        env.mock_all_auths();

        client.record_footprint(
            &submitter, &symbol_short!("eu_west"),
            &OperationCategory::ContractInvocation, &symbol_short!("invoke"),
            &100, &Bytes::from_slice(&env, b""),
        );
        client.record_footprint(
            &submitter, &symbol_short!("eu_west"),
            &OperationCategory::EventEmission, &symbol_short!("emit"),
            &200, &Bytes::from_slice(&env, b""),
        );

        let stats = client.get_aggregate_stats();
        assert_eq!(stats.total_operations, 2);
        assert_eq!(stats.total_energy_mwh, 300); // 100 + 200
        // carbon = (100 * 200) + (200 * 200) = 20000 + 40000 = 60000 μg
        assert_eq!(stats.total_carbon_ug_co2e, 60_000);
        // Both ops ran in green region (intensity 200 <= threshold 300)
        assert_eq!(stats.green_operations, 2);
        assert_eq!(stats.red_operations, 0);
    }

    #[test]
    fn test_aggregate_stats_red_operations_counted() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner); // red threshold = 600
        // intensity = 700 (above red threshold)
        add_region(&env, &client, &owner, symbol_short!("dirty"), 700, 5);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        client.record_footprint(
            &submitter, &symbol_short!("dirty"),
            &OperationCategory::BatchProcessing, &symbol_short!("batch"),
            &500, &Bytes::from_slice(&env, b""),
        );

        let stats = client.get_aggregate_stats();
        assert_eq!(stats.red_operations, 1);
        assert_eq!(stats.green_operations, 0);
    }

    // ── 7. Scaling policy tests ───────────────────────────────────────────────

    #[test]
    fn test_create_scaling_policy() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"my-policy"),
            &symbol_short!("eu_west"),
            &300, // scale up below 300
            &600, // scale down above 600
            &150, // 1.5×
            &50,  // 0.5×
            &60,  // min green score 60
        );

        assert_eq!(policy.policy_id, 0);
        assert!(policy.active);
        assert_eq!(policy.scale_up_below_mg, 300);
        assert_eq!(policy.scale_down_above_mg, 600);
    }

    #[test]
    fn test_evaluate_scaling_policy_scale_up() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        // intensity = 200 (below scale_up threshold of 300)
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"up-policy"),
            &symbol_short!("eu_west"),
            &300, // scale up below 300
            &600, // scale down above 600
            &150, // 1.5×
            &50,  // 0.5×
            &60,  // min green score 60 (eu_west score is 83 ≥ 60)
        );

        let decision = client.evaluate_scaling_policy(&policy.policy_id);
        assert_eq!(decision.direction, ScalingDirection::ScaleUp);
        assert_eq!(decision.factor_x100, 150);
        assert_eq!(decision.current_intensity_mg, 200);
    }

    #[test]
    fn test_evaluate_scaling_policy_scale_down() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        // intensity = 700 (above scale_down threshold of 600)
        add_region(&env, &client, &owner, symbol_short!("dirty"), 700, 5);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"down-policy"),
            &symbol_short!("dirty"),
            &300, &600, &150, &50, &60,
        );

        let decision = client.evaluate_scaling_policy(&policy.policy_id);
        assert_eq!(decision.direction, ScalingDirection::ScaleDown);
        assert_eq!(decision.factor_x100, 50);
    }

    #[test]
    fn test_evaluate_scaling_policy_hold() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        // intensity = 400 (between 300 and 600 → hold)
        add_region(&env, &client, &owner, symbol_short!("mid"), 400, 50);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"hold-policy"),
            &symbol_short!("mid"),
            &300, &600, &150, &50, &60,
        );

        let decision = client.evaluate_scaling_policy(&policy.policy_id);
        assert_eq!(decision.direction, ScalingDirection::Hold);
    }

    #[test]
    fn test_evaluate_scaling_policy_hold_when_green_score_too_low() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        // intensity = 200 (below scale-up threshold) but low renewable → low score
        add_region(&env, &client, &owner, symbol_short!("lowgs"), 200, 5);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"score-policy"),
            &symbol_short!("lowgs"),
            &300, // scale up below 300
            &600,
            &150,
            &50,
            &80, // min score 80 (region score will be < 80 with 5% renewable)
        );

        let decision = client.evaluate_scaling_policy(&policy.policy_id);
        // intensity is low enough but green score is insufficient → Hold
        assert_eq!(decision.direction, ScalingDirection::Hold);
    }

    #[test]
    fn test_deactivate_scaling_policy() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        let policy = client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"p"),
            &symbol_short!("eu_west"),
            &300, &600, &150, &50, &60,
        );

        client.deactivate_scaling_policy(&owner, &policy.policy_id);
        let updated = client.get_scaling_policy(&policy.policy_id);
        assert!(!updated.active);
    }

    #[test]
    #[should_panic]
    fn test_create_scaling_policy_zero_factor_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"bad"),
            &symbol_short!("eu_west"),
            &300, &600,
            &0, // invalid scale factor
            &50, &60,
        );
    }

    #[test]
    #[should_panic]
    fn test_create_scaling_policy_unknown_region_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.create_scaling_policy(
            &owner,
            &Bytes::from_slice(&env, b"bad"),
            &symbol_short!("ghost"),
            &300, &600, &150, &50, &60,
        );
    }

    #[test]
    #[should_panic]
    fn test_evaluate_nonexistent_policy_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        client.evaluate_scaling_policy(&99);
    }

    // ── 8. Green scheduling window tests ─────────────────────────────────────

    #[test]
    fn test_register_scheduling_window() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        let window = client.register_scheduling_window(
            &owner,
            &symbol_short!("eu_west"),
            &1000,
            &2000,
            &150,
            &85,
            &75,
        );

        assert_eq!(window.window_id, 0);
        assert_eq!(window.region_id, symbol_short!("eu_west"));
        assert_eq!(window.start_ts, 1000);
        assert_eq!(window.end_ts, 2000);
        assert!(window.active);
    }

    #[test]
    fn test_find_green_windows_overlap() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        // Window 0: 1000–2000
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &1000, &2000, &150, &85, &75);
        // Window 1: 3000–4000 (no overlap with 500–1500)
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &3000, &4000, &150, &85, &75);
        // Window 2: 1500–2500 (overlaps with 1200–1800)
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &1500, &2500, &150, &85, &75);

        // Query: 1200–1800 should overlap windows 0 and 2
        let result = client.find_green_windows(&symbol_short!("eu_west"), &1200, &1800);
        assert_eq!(result.len(), 2);
        // Should contain IDs 0 and 2
        let ids: soroban_sdk::Vec<u32> = result;
        let mut found_0 = false;
        let mut found_2 = false;
        for i in 0..ids.len() {
            match ids.get(i).unwrap() {
                0 => found_0 = true,
                2 => found_2 = true,
                _ => {}
            }
        }
        assert!(found_0);
        assert!(found_2);
    }

    #[test]
    fn test_find_green_windows_no_overlap() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &5000, &6000, &150, &85, &75);

        let result = client.find_green_windows(&symbol_short!("eu_west"), &1000, &2000);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_find_green_windows_wrong_region_returns_empty() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);
        add_region(&env, &client, &owner, symbol_short!("us_east"), 500, 30);

        env.mock_all_auths();
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &1000, &2000, &150, &85, &75);

        // Query for us_east but window belongs to eu_west
        let result = client.find_green_windows(&symbol_short!("us_east"), &500, &3000);
        assert_eq!(result.len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_register_window_invalid_time_range_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        // start >= end → invalid
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &2000, &1000, &150, &85, &75);
    }

    #[test]
    #[should_panic]
    fn test_register_window_unknown_region_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.register_scheduling_window(&owner, &symbol_short!("ghost"),
            &1000, &2000, &150, &85, &75);
    }

    #[test]
    fn test_get_scheduling_window_success() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        env.mock_all_auths();
        client.register_scheduling_window(&owner, &symbol_short!("eu_west"),
            &1000, &2000, &150, &85, &75);

        let w = client.get_scheduling_window(&0);
        assert_eq!(w.start_ts, 1000);
        assert_eq!(w.min_green_score, 75);
    }

    #[test]
    #[should_panic]
    fn test_get_scheduling_window_not_found_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        client.get_scheduling_window(&99);
    }

    // ── 9. Carbon budget tests ────────────────────────────────────────────────

    #[test]
    fn test_set_carbon_budget() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let user = Address::generate(&env);
        env.mock_all_auths();
        let budget = client.set_carbon_budget(
            &owner, &user, &1_000_000, &0, &0,
        );

        assert_eq!(budget.owner, user);
        assert_eq!(budget.budget_ug_co2e, 1_000_000);
        assert_eq!(budget.consumed_ug_co2e, 0);
        assert!(!budget.exhausted);
    }

    #[test]
    fn test_get_carbon_budget() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let user = Address::generate(&env);
        env.mock_all_auths();
        client.set_carbon_budget(&owner, &user, &500_000, &0, &0);

        let b = client.get_carbon_budget(&user);
        assert_eq!(b.budget_ug_co2e, 500_000);
    }

    #[test]
    #[should_panic]
    fn test_get_carbon_budget_not_found_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let unknown = Address::generate(&env);
        env.mock_all_auths();
        client.get_carbon_budget(&unknown);
    }

    #[test]
    #[should_panic]
    fn test_set_carbon_budget_zero_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let user = Address::generate(&env);
        env.mock_all_auths();
        client.set_carbon_budget(&owner, &user, &0, &0, &0); // invalid
    }

    #[test]
    fn test_carbon_budget_enforced_on_record_footprint() {
        let (env, client, owner) = setup();
        // enable budget enforcement
        env.mock_all_auths();
        client.initialize(&owner, &300, &600, &30, &true);

        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let user = Address::generate(&env);
        env.mock_all_auths();
        // budget = 15000 μg CO₂e
        // each op: 100 mWh * 200 mg/kWh = 20000 μg → exceeds budget on first call
        // so set a budget that fits exactly one 5_000 μg op
        // 5000 μg = 25 mWh * 200 mg/kWh
        client.set_carbon_budget(&owner, &user, &5_000, &0, &0);

        // First op: 25 mWh * 200 = 5000 μg → exactly at limit
        client.record_footprint(
            &user, &symbol_short!("eu_west"),
            &OperationCategory::ContractInvocation, &symbol_short!("invoke"),
            &25, &Bytes::from_slice(&env, b""),
        );

        // Budget consumed = 5000 = limit → exhausted
        let b = client.get_carbon_budget(&user);
        assert!(b.exhausted);
    }

    #[test]
    #[should_panic]
    fn test_carbon_budget_exceeded_panics() {
        let (env, client, owner) = setup();
        env.mock_all_auths();
        client.initialize(&owner, &300, &600, &30, &true);

        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let user = Address::generate(&env);
        env.mock_all_auths();
        // tiny budget: 1000 μg
        client.set_carbon_budget(&owner, &user, &1000, &0, &0);

        // op: 100 mWh * 200 = 20000 μg → far exceeds budget
        client.record_footprint(
            &user, &symbol_short!("eu_west"),
            &OperationCategory::ContractInvocation, &symbol_short!("invoke"),
            &100, &Bytes::from_slice(&env, b""),
        );
    }

    #[test]
    fn test_budget_not_enforced_when_disabled() {
        let (env, client, owner) = setup();
        // enforce_budgets = false
        env.mock_all_auths();
        client.initialize(&owner, &300, &600, &30, &false);

        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let user = Address::generate(&env);
        env.mock_all_auths();
        // Set a tiny budget but enforcement is off
        client.set_carbon_budget(&owner, &user, &1, &0, &0);

        // Should succeed even though emission would exceed budget
        client.record_footprint(
            &user, &symbol_short!("eu_west"),
            &OperationCategory::ContractInvocation, &symbol_short!("invoke"),
            &10000, &Bytes::from_slice(&env, b""),
        );
        assert_eq!(client.total_footprints(), 1);
    }

    // ── 10. Configuration update tests ───────────────────────────────────────

    #[test]
    fn test_update_config() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        let config = client.update_config(&owner, &100, &400, &50, &true);
        assert_eq!(config.green_intensity_threshold_mg, 100);
        assert_eq!(config.red_intensity_threshold_mg, 400);
        assert_eq!(config.min_renewable_percent, 50);
        assert!(config.enforce_budgets);
    }

    #[test]
    #[should_panic]
    fn test_update_config_non_owner_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        let not_owner = Address::generate(&env);
        env.mock_all_auths();
        client.update_config(&not_owner, &100, &400, &50, &false);
    }

    #[test]
    #[should_panic]
    fn test_update_config_invalid_renewable_fails() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        client.update_config(&owner, &100, &400, &101, &false); // > 100
    }

    #[test]
    fn test_get_config_returns_current() {
        let (env, client, owner) = setup();
        env.mock_all_auths();
        client.initialize(&owner, &250, &550, &25, &true);

        let config = client.get_config();
        assert_eq!(config.green_intensity_threshold_mg, 250);
        assert_eq!(config.red_intensity_threshold_mg, 550);
        assert!(config.enforce_budgets);
    }

    // ── 11. Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn test_total_footprints_zero_initially() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        assert_eq!(client.total_footprints(), 0);
    }

    #[test]
    fn test_ranked_regions_empty_initially() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        let ranked = client.ranked_regions();
        assert_eq!(ranked.len(), 0);
    }

    #[test]
    fn test_recommend_region_empty_when_no_regions() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        let rec = client.recommend_region(&0);
        assert_eq!(rec.len(), 0);
    }

    #[test]
    fn test_multiple_operation_categories() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 200, 80);

        let submitter = Address::generate(&env);
        env.mock_all_auths();

        let categories = [
            OperationCategory::ContractInvocation,
            OperationCategory::EventEmission,
            OperationCategory::StorageWrite,
            OperationCategory::StorageRead,
            OperationCategory::OffChainCompute,
            OperationCategory::NetworkTransfer,
            OperationCategory::BatchProcessing,
            OperationCategory::Custom,
        ];

        for cat in categories {
            client.record_footprint(
                &submitter,
                &symbol_short!("eu_west"),
                &cat,
                &symbol_short!("op"),
                &10,
                &Bytes::from_slice(&env, b""),
            );
        }

        assert_eq!(client.total_footprints(), 8);
    }

    #[test]
    fn test_region_with_100_percent_renewable_is_fully_renewable() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);

        env.mock_all_auths();
        let profile = client.upsert_region(
            &owner,
            &symbol_short!("solar"),
            &10,
            &100,
            &100,
            &EnergySourceMix::FullyRenewable,
            &Bytes::from_slice(&env, b"Solar Farm"),
            &true,
        );

        // Very high green score expected
        assert!(profile.green_score >= 95);
    }

    #[test]
    fn test_carbon_intensity_stored_in_footprint_record() {
        let (env, client, owner) = setup();
        init(&env, &client, &owner);
        add_region(&env, &client, &owner, symbol_short!("eu_west"), 250, 70);

        let submitter = Address::generate(&env);
        env.mock_all_auths();
        let id = client.record_footprint(
            &submitter,
            &symbol_short!("eu_west"),
            &OperationCategory::ContractInvocation,
            &symbol_short!("invoke"),
            &100,
            &Bytes::from_slice(&env, b"context"),
        );

        let record = client.get_footprint(&id);
        assert_eq!(record.carbon_intensity_mg, 250);
        assert_eq!(record.renewable_percent, 70);
        assert_eq!(record.submitter, submitter);
        assert_eq!(record.region_id, symbol_short!("eu_west"));
    }
}
