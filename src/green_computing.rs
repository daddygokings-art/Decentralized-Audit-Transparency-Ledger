//! # Green Computing & Carbon Tracking (Issue #508)
//!
//! This module provides immutable on-chain tracking of the carbon footprint for
//! infrastructure and contract operations, plus tooling for green scheduling,
//! carbon-aware region selection, and carbon-aware auto-scaling decisions.
//!
//! ## Key Concepts
//!
//! ### Carbon Intensity
//! Carbon intensity (g CO₂e / kWh) varies by region and time. A lower value
//! means each unit of compute emits less greenhouse gas.
//!
//! ### Energy Metrics
//! Infrastructure energy consumption is recorded in **milli-watt-hours (mWh)**
//! (i.e. Wh × 10⁻³) to preserve precision without floating-point arithmetic.
//!
//! ### Carbon Footprint
//! Footprints are tracked in **micro-g CO₂e** (μg CO₂e = 10⁻⁶ g CO₂e) to
//! represent sub-gram emissions for individual contract operations.
//!
//! ### Green Score
//! A 0–100 score summarising how "green" a workload or region is at a point in
//! time. Higher is better. It factors in renewable energy percentage, current
//! carbon intensity, and PUE (Power Usage Effectiveness).
//!
//! ## Storage Key Scheme
//!
//! | Key | Description |
//! |-----|-------------|
//! | `GreenConfig` | Global configuration (owner, thresholds) |
//! | `RegionProfile(region)` | Carbon intensity & renewable % for a region |
//! | `FootprintRecord(id)` | Individual operation carbon record |
//! | `FootprintCount` | Total records stored |
//! | `ScalingPolicy(policy_id)` | Carbon-aware auto-scaling rule |
//! | `ScalingPolicyCount` | Number of scaling policies |
//! | `SchedulingWindow(window_id)` | Green scheduling time window |
//! | `SchedulingWindowCount` | Number of scheduling windows |
//! | `CarbonBudget(owner)` | Per-owner carbon budget |
//! | `RegionRanking` | Ordered list of regions by green score |
//! | `AggregateStats` | Cumulative totals for reporting |

#![allow(dead_code)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error,
    Address, Bytes, Env, Symbol, Vec,
};

// ── Error codes ───────────────────────────────────────────────────────────────
// Range 4000–4099 to avoid collision with other modules.

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GreenError {
    /// Module has not been initialized.
    NotInitialized = 4000,
    /// Module is already initialized.
    AlreadyInitialized = 4001,
    /// Caller is not the owner.
    NotOwner = 4002,
    /// Region profile not found.
    RegionNotFound = 4003,
    /// Region profile already registered.
    RegionAlreadyExists = 4004,
    /// Footprint record not found.
    FootprintNotFound = 4005,
    /// Invalid carbon intensity value (must be > 0).
    InvalidCarbonIntensity = 4006,
    /// Invalid energy value.
    InvalidEnergyValue = 4007,
    /// Invalid green score (must be 0–100).
    InvalidGreenScore = 4008,
    /// Renewable percentage out of range (0–100).
    InvalidRenewablePercent = 4009,
    /// Scaling policy not found.
    PolicyNotFound = 4010,
    /// Scaling policy already exists.
    PolicyAlreadyExists = 4011,
    /// Invalid scale factor.
    InvalidScaleFactor = 4012,
    /// Scheduling window not found.
    WindowNotFound = 4013,
    /// Invalid time window (start must be before end).
    InvalidTimeWindow = 4014,
    /// Carbon budget not found for owner.
    BudgetNotFound = 4015,
    /// Carbon budget exceeded.
    BudgetExceeded = 4016,
    /// Invalid budget amount.
    InvalidBudgetAmount = 4017,
    /// PUE value out of range (must be ≥ 100, representing 1.00×).
    InvalidPue = 4018,
    /// Operation type symbol is too long.
    OperationTypeTooLong = 4019,
    /// Region symbol is too long.
    RegionTooLong = 4020,
}

// ── Enums ─────────────────────────────────────────────────────────────────────

/// The energy source mix used for a workload.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnergySourceMix {
    /// 100 % renewable (solar, wind, hydro, geothermal, …).
    FullyRenewable,
    /// Majority renewable (> 50 %).
    MajorityRenewable,
    /// Mixed grid (10–50 % renewable).
    MixedGrid,
    /// Fossil-fuel dominant (< 10 % renewable).
    FossilDominant,
    /// Unknown / not reported.
    Unknown,
}

/// Reason a scaling action was triggered.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalingTrigger {
    /// Carbon intensity dropped below the green threshold.
    CarbonIntensityLow,
    /// Carbon intensity rose above the red threshold.
    CarbonIntensityHigh,
    /// Green score crossed a policy threshold.
    GreenScoreThreshold,
    /// Manual administrative override.
    ManualOverride,
    /// Renewable energy percentage changed.
    RenewablePercentChange,
}

/// Direction of a scaling action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalingDirection {
    /// Scale up (increase capacity / compute).
    ScaleUp,
    /// Scale down (reduce capacity / compute).
    ScaleDown,
    /// No change recommended.
    Hold,
}

/// The type of operation whose footprint is being recorded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationCategory {
    /// Smart contract invocation.
    ContractInvocation,
    /// Event emission (Soroban event publishing).
    EventEmission,
    /// Data storage write.
    StorageWrite,
    /// Data storage read.
    StorageRead,
    /// Off-chain compute (API server, bridge relayer, etc.).
    OffChainCompute,
    /// Network data transfer.
    NetworkTransfer,
    /// Scheduled / batch processing.
    BatchProcessing,
    /// Custom / other.
    Custom,
}

// ── Structs ───────────────────────────────────────────────────────────────────

/// Global green computing configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GreenConfig {
    /// Contract owner (governance authority).
    pub owner: Address,
    /// Carbon intensity threshold below which a region is considered "green"
    /// (g CO₂e / kWh × 10⁻³, i.e. milli-g CO₂e / kWh).
    pub green_intensity_threshold_mg: u32,
    /// Carbon intensity threshold above which workloads should be shifted
    /// (same unit as above).
    pub red_intensity_threshold_mg: u32,
    /// Default renewable energy % required to qualify as "green" (0–100).
    pub min_renewable_percent: u32,
    /// Whether per-owner carbon budgets are enforced on `record_footprint`.
    pub enforce_budgets: bool,
    /// Timestamp of last configuration update.
    pub updated_at: u64,
}

/// Carbon intensity and renewable energy profile for a geographic region.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionProfile {
    /// Region identifier (e.g. `us-east-1`, `eu-west-1`).
    pub region_id: Symbol,
    /// Current carbon intensity in milli-g CO₂e / kWh (× 10⁻³ g/kWh).
    pub carbon_intensity_mg_per_kwh: u32,
    /// Renewable energy percentage (0–100).
    pub renewable_percent: u32,
    /// PUE (Power Usage Effectiveness) × 100; e.g. 140 = PUE 1.40.
    /// Must be ≥ 100 (PUE can never be below 1.0).
    pub pue_x100: u32,
    /// Pre-computed green score (0–100). Higher is better.
    pub green_score: u32,
    /// Dominant energy source mix.
    pub energy_mix: EnergySourceMix,
    /// Human-readable region name (e.g. "US East (N. Virginia)").
    pub display_name: Bytes,
    /// Ledger timestamp of last update.
    pub updated_at: u64,
    /// Whether this region is currently accepting green workload shifts.
    pub accepts_green_shifts: bool,
}

/// Carbon footprint record for a single operation or batch.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintRecord {
    /// Sequential record ID.
    pub id: u32,
    /// Address that triggered the operation.
    pub submitter: Address,
    /// Region where the operation ran.
    pub region_id: Symbol,
    /// High-level category of the operation.
    pub category: OperationCategory,
    /// Caller-supplied operation type label (max 10 chars as Symbol).
    pub operation_type: Symbol,
    /// Energy consumed in milli-Wh (Wh × 10⁻³).
    pub energy_mwh: u32,
    /// Carbon emitted in micro-g CO₂e (g CO₂e × 10⁻⁶).
    pub carbon_ug_co2e: u64,
    /// Carbon intensity at time of operation (milli-g CO₂e / kWh).
    pub carbon_intensity_mg: u32,
    /// Renewable energy percentage of the region at the time.
    pub renewable_percent: u32,
    /// Green score of the region at the time.
    pub green_score: u32,
    /// Optional metadata / context.
    pub metadata: Bytes,
    /// Ledger timestamp.
    pub recorded_at: u64,
}

/// Carbon-aware auto-scaling policy.
///
/// When the region's carbon intensity crosses a threshold, the policy
/// recommends scaling the workload up or down.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingPolicy {
    /// Sequential policy ID.
    pub policy_id: u32,
    /// Policy owner / workload identifier.
    pub owner: Address,
    /// Human-readable policy name.
    pub name: Bytes,
    /// Region this policy applies to (or empty for global).
    pub region_id: Symbol,
    /// Carbon intensity (mg/kWh) below which we scale up.
    pub scale_up_below_mg: u32,
    /// Carbon intensity (mg/kWh) above which we scale down.
    pub scale_down_above_mg: u32,
    /// Scale factor when scaling up (× 100; e.g. 150 = 1.5×).
    pub scale_up_factor_x100: u32,
    /// Scale factor when scaling down (× 100; e.g. 50 = 0.5×).
    pub scale_down_factor_x100: u32,
    /// Minimum green score required to allow scale-up (0–100).
    pub min_green_score_for_scale_up: u32,
    /// Whether the policy is currently active.
    pub active: bool,
    /// Ledger timestamp of creation.
    pub created_at: u64,
    /// Ledger timestamp of last update.
    pub updated_at: u64,
}

/// Result of evaluating a scaling policy against current region conditions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalingDecision {
    /// Policy that was evaluated.
    pub policy_id: u32,
    /// Region evaluated.
    pub region_id: Symbol,
    /// Recommended scaling direction.
    pub direction: ScalingDirection,
    /// Recommended scale factor (× 100).
    pub factor_x100: u32,
    /// Why the decision was made.
    pub trigger: ScalingTrigger,
    /// Current carbon intensity at decision time.
    pub current_intensity_mg: u32,
    /// Current green score at decision time.
    pub current_green_score: u32,
    /// Ledger timestamp of the decision.
    pub decided_at: u64,
}

/// Green scheduling window: a time period in which green compute is preferred.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulingWindow {
    /// Sequential window ID.
    pub window_id: u32,
    /// Region this window applies to.
    pub region_id: Symbol,
    /// Ledger timestamp when the window opens.
    pub start_ts: u64,
    /// Ledger timestamp when the window closes.
    pub end_ts: u64,
    /// Expected carbon intensity during the window (mg/kWh).
    pub expected_intensity_mg: u32,
    /// Expected renewable percentage during the window.
    pub expected_renewable_percent: u32,
    /// Minimum green score expected.
    pub min_green_score: u32,
    /// Whether the window is still valid / not expired.
    pub active: bool,
    /// Who created the window.
    pub created_by: Address,
    /// Ledger timestamp of creation.
    pub created_at: u64,
}

/// Per-owner carbon budget for enforcing emission limits.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarbonBudget {
    /// Budget owner.
    pub owner: Address,
    /// Total budget in micro-g CO₂e.
    pub budget_ug_co2e: u64,
    /// Amount consumed so far in micro-g CO₂e.
    pub consumed_ug_co2e: u64,
    /// Budget period start timestamp.
    pub period_start: u64,
    /// Budget period end timestamp (0 = no expiry).
    pub period_end: u64,
    /// Whether the budget has been exhausted.
    pub exhausted: bool,
    /// Timestamp of last consumption update.
    pub updated_at: u64,
}

/// Aggregate carbon and energy statistics across all recorded operations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregateStats {
    /// Total operations recorded.
    pub total_operations: u32,
    /// Total energy consumed across all operations (milli-Wh).
    pub total_energy_mwh: u64,
    /// Total carbon emitted across all operations (micro-g CO₂e).
    pub total_carbon_ug_co2e: u64,
    /// Average green score across all recorded operations (0–100).
    pub avg_green_score: u32,
    /// Number of operations that ran in green (below threshold) regions.
    pub green_operations: u32,
    /// Number of operations that ran in red (above threshold) regions.
    pub red_operations: u32,
    /// Timestamp of last update.
    pub updated_at: u64,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum GreenDataKey {
    /// Global configuration.
    GreenConfig,
    /// Region profile: (region_id) → RegionProfile.
    RegionProfile(Symbol),
    /// Ordered list of region IDs sorted by green score (best first).
    RegionRanking,
    /// Footprint record by ID: (id) → FootprintRecord.
    FootprintRecord(u32),
    /// Total footprint records stored.
    FootprintCount,
    /// Scaling policy by ID: (policy_id) → ScalingPolicy.
    ScalingPolicy(u32),
    /// Total scaling policies.
    ScalingPolicyCount,
    /// Scheduling window by ID: (window_id) → SchedulingWindow.
    SchedulingWindow(u32),
    /// Total scheduling windows.
    SchedulingWindowCount,
    /// Per-owner carbon budget: (owner) → CarbonBudget.
    CarbonBudget(Address),
    /// Aggregate stats across all operations.
    AggregateStats,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct GreenComputingContract;

// ── Internal helpers ──────────────────────────────────────────────────────────

fn get_config(env: &Env) -> Option<GreenConfig> {
    env.storage()
        .instance()
        .get::<_, GreenConfig>(&GreenDataKey::GreenConfig)
}

fn require_owner(env: &Env, caller: &Address) {
    let config = get_config(env)
        .unwrap_or_else(|| panic_with_error!(env, GreenError::NotInitialized));
    if config.owner != *caller {
        panic_with_error!(env, GreenError::NotOwner);
    }
}

fn require_initialized(env: &Env) -> GreenConfig {
    get_config(env).unwrap_or_else(|| panic_with_error!(env, GreenError::NotInitialized))
}

/// Compute a green score (0–100) from carbon intensity, renewable %, and PUE.
///
/// Formula (all weights sum to 1.0):
/// - Renewable component (40 %): renewable_percent * 0.40
/// - Intensity component (40 %): max(0, (1 - intensity / 1000)) * 40
///   (assumes 1000 mg/kWh as a "worst case" grid; clamps at 0)
/// - PUE component (20 %): max(0, (1 - (pue - 100) / 200)) * 20
///   (PUE of 1.00 → full score; PUE of 3.00 → zero; linear between)
///
/// All arithmetic is integer-only to be WASM-safe.
pub fn compute_green_score(
    carbon_intensity_mg: u32,
    renewable_percent: u32,
    pue_x100: u32,
) -> u32 {
    // Renewable component: 0–40
    let renew_clamped = renewable_percent.min(100);
    let renew_score = renew_clamped * 40 / 100; // 0..40

    // Intensity component: 0–40
    // reference_worst = 1000 mg/kWh
    let intensity_score = if carbon_intensity_mg >= 1000 {
        0u32
    } else {
        (1000 - carbon_intensity_mg) * 40 / 1000
    };

    // PUE component: 0–20
    // PUE of 100 (1.00×) → 20; PUE of 300 (3.00×) → 0; linear
    let pue_floor = pue_x100.max(100);
    let pue_excess = pue_floor - 100; // 0..∞
    let pue_score = if pue_excess >= 200 {
        0u32
    } else {
        (200 - pue_excess) * 20 / 200
    };

    renew_score + intensity_score + pue_score
}

fn get_footprint_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&GreenDataKey::FootprintCount)
        .unwrap_or(0)
}

fn get_scaling_policy_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&GreenDataKey::ScalingPolicyCount)
        .unwrap_or(0)
}

fn get_scheduling_window_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&GreenDataKey::SchedulingWindowCount)
        .unwrap_or(0)
}

fn get_aggregate_stats(env: &Env) -> AggregateStats {
    env.storage()
        .instance()
        .get::<_, AggregateStats>(&GreenDataKey::AggregateStats)
        .unwrap_or(AggregateStats {
            total_operations: 0,
            total_energy_mwh: 0,
            total_carbon_ug_co2e: 0,
            avg_green_score: 0,
            green_operations: 0,
            red_operations: 0,
            updated_at: 0,
        })
}

// ── Contract implementation ───────────────────────────────────────────────────

#[contractimpl]
impl GreenComputingContract {
    // ── Initialization ────────────────────────────────────────────────────────

    /// Initialize the green computing module.
    ///
    /// # Arguments
    /// * `owner`                      – Governance authority.
    /// * `green_intensity_threshold_mg` – mg CO₂e/kWh below which a region is "green".
    /// * `red_intensity_threshold_mg`   – mg CO₂e/kWh above which shifts are recommended.
    /// * `min_renewable_percent`       – Minimum renewable % to qualify as green (0–100).
    /// * `enforce_budgets`             – Whether to enforce per-owner carbon budgets.
    ///
    /// # Errors
    /// * `AlreadyInitialized` – Called more than once.
    pub fn initialize(
        env: Env,
        owner: Address,
        green_intensity_threshold_mg: u32,
        red_intensity_threshold_mg: u32,
        min_renewable_percent: u32,
        enforce_budgets: bool,
    ) -> GreenConfig {
        owner.require_auth();

        if get_config(&env).is_some() {
            panic_with_error!(&env, GreenError::AlreadyInitialized);
        }

        if min_renewable_percent > 100 {
            panic_with_error!(&env, GreenError::InvalidRenewablePercent);
        }

        let now = env.ledger().timestamp();
        let config = GreenConfig {
            owner: owner.clone(),
            green_intensity_threshold_mg,
            red_intensity_threshold_mg,
            min_renewable_percent,
            enforce_budgets,
            updated_at: now,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::GreenConfig, &config);

        env.events().publish(
            (Symbol::new(&env, "green"), Symbol::new(&env, "init")),
            (owner,),
        );

        config
    }

    // ── Region management ─────────────────────────────────────────────────────

    /// Register or update a regional carbon intensity profile.
    ///
    /// The green score is computed automatically from the provided values.
    ///
    /// # Arguments
    /// * `caller`                   – Must be the owner.
    /// * `region_id`                – Region Symbol identifier.
    /// * `carbon_intensity_mg`      – Current carbon intensity (mg CO₂e / kWh).
    /// * `renewable_percent`        – Renewable energy % (0–100).
    /// * `pue_x100`                 – PUE × 100 (≥ 100).
    /// * `energy_mix`               – Dominant energy source mix.
    /// * `display_name`             – Human-readable name bytes.
    /// * `accepts_green_shifts`     – Whether this region accepts workload migrations.
    ///
    /// # Errors
    /// * `NotOwner`                 – Caller is not the contract owner.
    /// * `InvalidCarbonIntensity`   – `carbon_intensity_mg` is zero.
    /// * `InvalidRenewablePercent`  – `renewable_percent` > 100.
    /// * `InvalidPue`               – `pue_x100` < 100.
    pub fn upsert_region(
        env: Env,
        caller: Address,
        region_id: Symbol,
        carbon_intensity_mg: u32,
        renewable_percent: u32,
        pue_x100: u32,
        energy_mix: EnergySourceMix,
        display_name: Bytes,
        accepts_green_shifts: bool,
    ) -> RegionProfile {
        caller.require_auth();
        require_owner(&env, &caller);

        if carbon_intensity_mg == 0 {
            panic_with_error!(&env, GreenError::InvalidCarbonIntensity);
        }
        if renewable_percent > 100 {
            panic_with_error!(&env, GreenError::InvalidRenewablePercent);
        }
        if pue_x100 < 100 {
            panic_with_error!(&env, GreenError::InvalidPue);
        }

        let green_score = compute_green_score(carbon_intensity_mg, renewable_percent, pue_x100);

        let profile = RegionProfile {
            region_id: region_id.clone(),
            carbon_intensity_mg_per_kwh: carbon_intensity_mg,
            renewable_percent,
            pue_x100,
            green_score,
            energy_mix,
            display_name,
            updated_at: env.ledger().timestamp(),
            accepts_green_shifts,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::RegionProfile(region_id.clone()), &profile);

        // Rebuild the region ranking list by updating or inserting this region.
        // We keep a Vec<Symbol> sorted descending by green score.
        let mut ranking: Vec<Symbol> = env
            .storage()
            .instance()
            .get::<_, Vec<Symbol>>(&GreenDataKey::RegionRanking)
            .unwrap_or_else(|| Vec::new(&env));

        // Remove stale entry for this region (if any).
        let mut new_ranking: Vec<Symbol> = Vec::new(&env);
        for i in 0..ranking.len() {
            let r = ranking.get(i).unwrap();
            if r != region_id {
                new_ranking.push_back(r);
            }
        }

        // Insert in the correct sorted position (descending green score).
        let mut inserted = false;
        let mut final_ranking: Vec<Symbol> = Vec::new(&env);
        for i in 0..new_ranking.len() {
            let r = new_ranking.get(i).unwrap();
            if !inserted {
                let other_score: u32 = env
                    .storage()
                    .instance()
                    .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(r.clone()))
                    .map(|p| p.green_score)
                    .unwrap_or(0);
                if green_score >= other_score {
                    final_ranking.push_back(region_id.clone());
                    inserted = true;
                }
            }
            final_ranking.push_back(r);
        }
        if !inserted {
            final_ranking.push_back(region_id.clone());
        }

        env.storage()
            .instance()
            .set(&GreenDataKey::RegionRanking, &final_ranking);

        env.events().publish(
            (Symbol::new(&env, "green"), Symbol::new(&env, "region_upsert")),
            (caller, region_id, green_score),
        );

        profile
    }

    /// Get a region profile by ID.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `RegionNotFound` – Region ID not registered.
    pub fn get_region(env: Env, region_id: Symbol) -> RegionProfile {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(region_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::RegionNotFound))
    }

    /// Return all region IDs ranked by green score (best first).
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    pub fn ranked_regions(env: Env) -> Vec<Symbol> {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, Vec<Symbol>>(&GreenDataKey::RegionRanking)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return the greenest region that accepts workload shifts.
    ///
    /// Iterates the ranking list and returns the first region that
    /// `accepts_green_shifts == true` and has a green score ≥ `min_score`.
    ///
    /// Returns `None` (encoded as 0 results in the Vec) when none qualifies.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    pub fn recommend_region(env: Env, min_score: u32) -> Vec<Symbol> {
        require_initialized(&env);
        let ranking: Vec<Symbol> = env
            .storage()
            .instance()
            .get::<_, Vec<Symbol>>(&GreenDataKey::RegionRanking)
            .unwrap_or_else(|| Vec::new(&env));

        let mut result: Vec<Symbol> = Vec::new(&env);
        for i in 0..ranking.len() {
            let r = ranking.get(i).unwrap();
            if let Some(profile) = env
                .storage()
                .instance()
                .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(r.clone()))
            {
                if profile.accepts_green_shifts && profile.green_score >= min_score {
                    result.push_back(r);
                    break;
                }
            }
        }
        result
    }

    // ── Footprint recording ───────────────────────────────────────────────────

    /// Record the carbon footprint of a single operation or batch.
    ///
    /// The carbon emission is computed as:
    /// ```text
    /// carbon_ug = energy_mwh * carbon_intensity_mg / 1000
    /// ```
    /// where `energy_mwh` is milli-Wh and `carbon_intensity_mg` is mg CO₂e/kWh.
    /// The result is in micro-g CO₂e (μg CO₂e).
    ///
    /// If `enforce_budgets` is enabled and the caller has a budget, the
    /// consumed amount is checked against it.
    ///
    /// # Arguments
    /// * `submitter`       – Address of the operation submitter.
    /// * `region_id`       – Region where the operation ran.
    /// * `category`        – Operation category.
    /// * `operation_type`  – Short symbol for the operation type (max 10 chars).
    /// * `energy_mwh`      – Energy consumed (milli-Wh).
    /// * `metadata`        – Optional context bytes.
    ///
    /// # Returns
    /// Sequential `FootprintRecord` ID.
    ///
    /// # Errors
    /// * `NotInitialized`     – Module not initialized.
    /// * `RegionNotFound`     – Region not registered.
    /// * `InvalidEnergyValue` – `energy_mwh` is zero.
    /// * `BudgetExceeded`     – Caller's carbon budget would be exceeded.
    pub fn record_footprint(
        env: Env,
        submitter: Address,
        region_id: Symbol,
        category: OperationCategory,
        operation_type: Symbol,
        energy_mwh: u32,
        metadata: Bytes,
    ) -> u32 {
        submitter.require_auth();
        let config = require_initialized(&env);

        if energy_mwh == 0 {
            panic_with_error!(&env, GreenError::InvalidEnergyValue);
        }

        let profile: RegionProfile = env
            .storage()
            .instance()
            .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(region_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::RegionNotFound));

        // carbon_ug = energy_mwh (mWh) * intensity (mg/kWh) / 1000
        // Units: mWh * mg/kWh = mWh * mg / (1000 Wh) = mg * mWh / 1000 Wh
        //        = mg/1000 * (Wh/Wh) ... simplified:
        //        1 kWh = 1000 Wh = 1_000_000 mWh
        //        energy_mwh / 1_000_000 kWh * intensity_mg g/kWh * 1_000_000 μg/g
        //        = energy_mwh * intensity_mg (μg CO₂e)
        let carbon_ug_co2e: u64 =
            (energy_mwh as u64) * (profile.carbon_intensity_mg_per_kwh as u64);

        // Budget enforcement
        if config.enforce_budgets {
            if let Some(mut budget) = env
                .storage()
                .instance()
                .get::<_, CarbonBudget>(&GreenDataKey::CarbonBudget(submitter.clone()))
            {
                if budget.exhausted {
                    panic_with_error!(&env, GreenError::BudgetExceeded);
                }
                let new_consumed = budget.consumed_ug_co2e.saturating_add(carbon_ug_co2e);
                if new_consumed > budget.budget_ug_co2e {
                    panic_with_error!(&env, GreenError::BudgetExceeded);
                }
                budget.consumed_ug_co2e = new_consumed;
                budget.exhausted = new_consumed >= budget.budget_ug_co2e;
                budget.updated_at = env.ledger().timestamp();
                env.storage()
                    .instance()
                    .set(&GreenDataKey::CarbonBudget(submitter.clone()), &budget);
            }
        }

        let record_id = get_footprint_count(&env);
        let now = env.ledger().timestamp();

        let record = FootprintRecord {
            id: record_id,
            submitter: submitter.clone(),
            region_id: region_id.clone(),
            category,
            operation_type: operation_type.clone(),
            energy_mwh,
            carbon_ug_co2e,
            carbon_intensity_mg: profile.carbon_intensity_mg_per_kwh,
            renewable_percent: profile.renewable_percent,
            green_score: profile.green_score,
            metadata,
            recorded_at: now,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::FootprintRecord(record_id), &record);
        env.storage()
            .instance()
            .set(&GreenDataKey::FootprintCount, &(record_id + 1));

        // Update aggregate stats
        let mut stats = get_aggregate_stats(&env);
        stats.total_operations += 1;
        stats.total_energy_mwh = stats.total_energy_mwh.saturating_add(energy_mwh as u64);
        stats.total_carbon_ug_co2e = stats
            .total_carbon_ug_co2e
            .saturating_add(carbon_ug_co2e);
        // Running average of green score
        let prev_total = (stats.avg_green_score as u64)
            * (stats.total_operations.saturating_sub(1) as u64);
        stats.avg_green_score =
            ((prev_total + profile.green_score as u64) / stats.total_operations as u64) as u32;
        if profile.carbon_intensity_mg_per_kwh <= config.green_intensity_threshold_mg {
            stats.green_operations += 1;
        } else if profile.carbon_intensity_mg_per_kwh >= config.red_intensity_threshold_mg {
            stats.red_operations += 1;
        }
        stats.updated_at = now;
        env.storage()
            .instance()
            .set(&GreenDataKey::AggregateStats, &stats);

        env.events().publish(
            (Symbol::new(&env, "green"), Symbol::new(&env, "footprint")),
            (submitter, region_id, carbon_ug_co2e),
        );

        record_id
    }

    /// Get a footprint record by its sequential ID.
    ///
    /// # Errors
    /// * `NotInitialized`   – Module not initialized.
    /// * `FootprintNotFound` – No record with this ID.
    pub fn get_footprint(env: Env, record_id: u32) -> FootprintRecord {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, FootprintRecord>(&GreenDataKey::FootprintRecord(record_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::FootprintNotFound))
    }

    /// Return the total number of footprint records stored.
    pub fn total_footprints(env: Env) -> u32 {
        require_initialized(&env);
        get_footprint_count(&env)
    }

    // ── Aggregate statistics ──────────────────────────────────────────────────

    /// Return cumulative carbon and energy statistics across all recorded operations.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    pub fn get_aggregate_stats(env: Env) -> AggregateStats {
        require_initialized(&env);
        get_aggregate_stats(&env)
    }

    // ── Carbon-aware auto-scaling ─────────────────────────────────────────────

    /// Create a carbon-aware auto-scaling policy.
    ///
    /// # Arguments
    /// * `caller`                     – Policy owner; must be authenticated.
    /// * `name`                       – Policy name bytes.
    /// * `region_id`                  – Region this policy targets.
    /// * `scale_up_below_mg`          – Scale up when intensity drops below this.
    /// * `scale_down_above_mg`        – Scale down when intensity rises above this.
    /// * `scale_up_factor_x100`       – Scale factor when scaling up (× 100).
    /// * `scale_down_factor_x100`     – Scale factor when scaling down (× 100).
    /// * `min_green_score_for_scale_up` – Minimum green score to allow scale-up.
    ///
    /// # Errors
    /// * `NotInitialized`   – Module not initialized.
    /// * `InvalidScaleFactor` – A scale factor is zero.
    /// * `RegionNotFound`   – Region not registered.
    pub fn create_scaling_policy(
        env: Env,
        caller: Address,
        name: Bytes,
        region_id: Symbol,
        scale_up_below_mg: u32,
        scale_down_above_mg: u32,
        scale_up_factor_x100: u32,
        scale_down_factor_x100: u32,
        min_green_score_for_scale_up: u32,
    ) -> ScalingPolicy {
        caller.require_auth();
        require_initialized(&env);

        if scale_up_factor_x100 == 0 || scale_down_factor_x100 == 0 {
            panic_with_error!(&env, GreenError::InvalidScaleFactor);
        }

        // Verify region exists
        if env
            .storage()
            .instance()
            .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(region_id.clone()))
            .is_none()
        {
            panic_with_error!(&env, GreenError::RegionNotFound);
        }

        if min_green_score_for_scale_up > 100 {
            panic_with_error!(&env, GreenError::InvalidGreenScore);
        }

        let policy_id = get_scaling_policy_count(&env);
        let now = env.ledger().timestamp();

        let policy = ScalingPolicy {
            policy_id,
            owner: caller.clone(),
            name,
            region_id: region_id.clone(),
            scale_up_below_mg,
            scale_down_above_mg,
            scale_up_factor_x100,
            scale_down_factor_x100,
            min_green_score_for_scale_up,
            active: true,
            created_at: now,
            updated_at: now,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::ScalingPolicy(policy_id), &policy);
        env.storage()
            .instance()
            .set(&GreenDataKey::ScalingPolicyCount, &(policy_id + 1));

        env.events().publish(
            (
                Symbol::new(&env, "green"),
                Symbol::new(&env, "policy_created"),
            ),
            (caller, policy_id, region_id),
        );

        policy
    }

    /// Evaluate a scaling policy against the current region conditions and
    /// return a `ScalingDecision`.
    ///
    /// This is a **read-only** operation — it does not mutate state. It is
    /// intended to be called off-chain (simulation / view) or from an
    /// infrastructure orchestrator.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `PolicyNotFound` – Policy ID not found.
    /// * `RegionNotFound` – Region associated with the policy not found.
    pub fn evaluate_scaling_policy(env: Env, policy_id: u32) -> ScalingDecision {
        require_initialized(&env);

        let policy: ScalingPolicy = env
            .storage()
            .instance()
            .get::<_, ScalingPolicy>(&GreenDataKey::ScalingPolicy(policy_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::PolicyNotFound));

        let profile: RegionProfile = env
            .storage()
            .instance()
            .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(policy.region_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::RegionNotFound));

        let now = env.ledger().timestamp();
        let ci = profile.carbon_intensity_mg_per_kwh;
        let gs = profile.green_score;

        let (direction, factor_x100, trigger) = if ci <= policy.scale_up_below_mg
            && gs >= policy.min_green_score_for_scale_up
        {
            (
                ScalingDirection::ScaleUp,
                policy.scale_up_factor_x100,
                ScalingTrigger::CarbonIntensityLow,
            )
        } else if ci >= policy.scale_down_above_mg {
            (
                ScalingDirection::ScaleDown,
                policy.scale_down_factor_x100,
                ScalingTrigger::CarbonIntensityHigh,
            )
        } else if gs < policy.min_green_score_for_scale_up && ci <= policy.scale_up_below_mg {
            (
                ScalingDirection::Hold,
                100u32,
                ScalingTrigger::GreenScoreThreshold,
            )
        } else {
            (ScalingDirection::Hold, 100u32, ScalingTrigger::GreenScoreThreshold)
        };

        ScalingDecision {
            policy_id,
            region_id: policy.region_id,
            direction,
            factor_x100,
            trigger,
            current_intensity_mg: ci,
            current_green_score: gs,
            decided_at: now,
        }
    }

    /// Get a scaling policy by ID.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `PolicyNotFound` – Policy not found.
    pub fn get_scaling_policy(env: Env, policy_id: u32) -> ScalingPolicy {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, ScalingPolicy>(&GreenDataKey::ScalingPolicy(policy_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::PolicyNotFound))
    }

    /// Deactivate a scaling policy.
    ///
    /// # Arguments
    /// * `caller` – Must be the policy owner.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `PolicyNotFound` – Policy not found.
    /// * `NotOwner`       – Caller is not the policy owner.
    pub fn deactivate_scaling_policy(env: Env, caller: Address, policy_id: u32) {
        caller.require_auth();
        require_initialized(&env);

        let mut policy: ScalingPolicy = env
            .storage()
            .instance()
            .get::<_, ScalingPolicy>(&GreenDataKey::ScalingPolicy(policy_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::PolicyNotFound));

        if policy.owner != caller {
            panic_with_error!(&env, GreenError::NotOwner);
        }

        policy.active = false;
        policy.updated_at = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&GreenDataKey::ScalingPolicy(policy_id), &policy);

        env.events().publish(
            (
                Symbol::new(&env, "green"),
                Symbol::new(&env, "policy_deactivated"),
            ),
            (caller, policy_id),
        );
    }

    // ── Green scheduling ──────────────────────────────────────────────────────

    /// Register a green scheduling window for a region.
    ///
    /// A scheduling window indicates a period when a region is expected to be
    /// particularly green (low carbon intensity / high renewables). Callers
    /// should prefer to run batch workloads during these windows.
    ///
    /// # Arguments
    /// * `caller`                     – Must be the contract owner.
    /// * `region_id`                  – Target region.
    /// * `start_ts`                   – Window start (ledger timestamp).
    /// * `end_ts`                     – Window end (ledger timestamp).
    /// * `expected_intensity_mg`      – Expected carbon intensity during window.
    /// * `expected_renewable_percent` – Expected renewable % during window.
    /// * `min_green_score`            – Minimum green score expected.
    ///
    /// # Errors
    /// * `NotOwner`           – Caller is not the contract owner.
    /// * `RegionNotFound`     – Region not registered.
    /// * `InvalidTimeWindow`  – `start_ts >= end_ts`.
    pub fn register_scheduling_window(
        env: Env,
        caller: Address,
        region_id: Symbol,
        start_ts: u64,
        end_ts: u64,
        expected_intensity_mg: u32,
        expected_renewable_percent: u32,
        min_green_score: u32,
    ) -> SchedulingWindow {
        caller.require_auth();
        require_owner(&env, &caller);

        if start_ts >= end_ts {
            panic_with_error!(&env, GreenError::InvalidTimeWindow);
        }
        if expected_renewable_percent > 100 {
            panic_with_error!(&env, GreenError::InvalidRenewablePercent);
        }
        if min_green_score > 100 {
            panic_with_error!(&env, GreenError::InvalidGreenScore);
        }

        // Verify region exists
        if env
            .storage()
            .instance()
            .get::<_, RegionProfile>(&GreenDataKey::RegionProfile(region_id.clone()))
            .is_none()
        {
            panic_with_error!(&env, GreenError::RegionNotFound);
        }

        let window_id = get_scheduling_window_count(&env);
        let now = env.ledger().timestamp();

        let window = SchedulingWindow {
            window_id,
            region_id: region_id.clone(),
            start_ts,
            end_ts,
            expected_intensity_mg,
            expected_renewable_percent,
            min_green_score,
            active: true,
            created_by: caller.clone(),
            created_at: now,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::SchedulingWindow(window_id), &window);
        env.storage()
            .instance()
            .set(&GreenDataKey::SchedulingWindowCount, &(window_id + 1));

        env.events().publish(
            (
                Symbol::new(&env, "green"),
                Symbol::new(&env, "window_registered"),
            ),
            (caller, window_id, region_id),
        );

        window
    }

    /// Get a scheduling window by ID.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `WindowNotFound` – Window not found.
    pub fn get_scheduling_window(env: Env, window_id: u32) -> SchedulingWindow {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, SchedulingWindow>(&GreenDataKey::SchedulingWindow(window_id))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::WindowNotFound))
    }

    /// Return the IDs of all active scheduling windows for a region that
    /// overlap with the given time range `[query_start, query_end]`.
    ///
    /// Scans all windows linearly — suitable for small-to-medium window counts.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    pub fn find_green_windows(
        env: Env,
        region_id: Symbol,
        query_start: u64,
        query_end: u64,
    ) -> Vec<u32> {
        require_initialized(&env);
        let count = get_scheduling_window_count(&env);
        let mut result: Vec<u32> = Vec::new(&env);

        for i in 0..count {
            if let Some(w) = env
                .storage()
                .instance()
                .get::<_, SchedulingWindow>(&GreenDataKey::SchedulingWindow(i))
            {
                if w.region_id == region_id
                    && w.active
                    && w.end_ts > query_start
                    && w.start_ts < query_end
                {
                    result.push_back(i);
                }
            }
        }
        result
    }

    // ── Carbon budget management ──────────────────────────────────────────────

    /// Set or update a carbon budget for an address.
    ///
    /// Only the contract owner can set budgets. Budgets are enforced during
    /// `record_footprint` when `enforce_budgets` is `true`.
    ///
    /// # Arguments
    /// * `caller`          – Must be the contract owner.
    /// * `owner`           – Address the budget is for.
    /// * `budget_ug_co2e`  – Total budget in micro-g CO₂e.
    /// * `period_start`    – Budget period start timestamp.
    /// * `period_end`      – Budget period end (0 = no expiry).
    ///
    /// # Errors
    /// * `NotOwner`              – Caller is not the contract owner.
    /// * `InvalidBudgetAmount`   – `budget_ug_co2e` is zero.
    pub fn set_carbon_budget(
        env: Env,
        caller: Address,
        owner: Address,
        budget_ug_co2e: u64,
        period_start: u64,
        period_end: u64,
    ) -> CarbonBudget {
        caller.require_auth();
        require_owner(&env, &caller);

        if budget_ug_co2e == 0 {
            panic_with_error!(&env, GreenError::InvalidBudgetAmount);
        }

        let now = env.ledger().timestamp();
        let budget = CarbonBudget {
            owner: owner.clone(),
            budget_ug_co2e,
            consumed_ug_co2e: 0,
            period_start,
            period_end,
            exhausted: false,
            updated_at: now,
        };

        env.storage()
            .instance()
            .set(&GreenDataKey::CarbonBudget(owner.clone()), &budget);

        env.events().publish(
            (
                Symbol::new(&env, "green"),
                Symbol::new(&env, "budget_set"),
            ),
            (caller, owner, budget_ug_co2e),
        );

        budget
    }

    /// Get the carbon budget for an address.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    /// * `BudgetNotFound` – No budget set for this address.
    pub fn get_carbon_budget(env: Env, owner: Address) -> CarbonBudget {
        require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, CarbonBudget>(&GreenDataKey::CarbonBudget(owner))
            .unwrap_or_else(|| panic_with_error!(&env, GreenError::BudgetNotFound))
    }

    // ── Configuration management ──────────────────────────────────────────────

    /// Update global configuration thresholds.
    ///
    /// # Arguments
    /// * `caller`                       – Must be the owner.
    /// * `green_intensity_threshold_mg` – New green threshold.
    /// * `red_intensity_threshold_mg`   – New red threshold.
    /// * `min_renewable_percent`        – New minimum renewable %.
    /// * `enforce_budgets`              – Toggle budget enforcement.
    ///
    /// # Errors
    /// * `NotOwner`                – Caller is not the owner.
    /// * `InvalidRenewablePercent` – `min_renewable_percent` > 100.
    pub fn update_config(
        env: Env,
        caller: Address,
        green_intensity_threshold_mg: u32,
        red_intensity_threshold_mg: u32,
        min_renewable_percent: u32,
        enforce_budgets: bool,
    ) -> GreenConfig {
        caller.require_auth();
        require_owner(&env, &caller);

        if min_renewable_percent > 100 {
            panic_with_error!(&env, GreenError::InvalidRenewablePercent);
        }

        let mut config = require_initialized(&env);
        config.green_intensity_threshold_mg = green_intensity_threshold_mg;
        config.red_intensity_threshold_mg = red_intensity_threshold_mg;
        config.min_renewable_percent = min_renewable_percent;
        config.enforce_budgets = enforce_budgets;
        config.updated_at = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&GreenDataKey::GreenConfig, &config);

        env.events().publish(
            (
                Symbol::new(&env, "green"),
                Symbol::new(&env, "config_updated"),
            ),
            (caller,),
        );

        config
    }

    /// Get the current global configuration.
    ///
    /// # Errors
    /// * `NotInitialized` – Module not initialized.
    pub fn get_config(env: Env) -> GreenConfig {
        require_initialized(&env)
    }
}
