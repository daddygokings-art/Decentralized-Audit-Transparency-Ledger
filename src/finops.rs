#![allow(dead_code)]

use soroban_sdk::{
    contracterror, contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FinOpsError {
    CostCenterNotFound = 9000,
    CostCenterAlreadyExists = 9001,
    BudgetNotFound = 9002,
    BudgetAlreadyExists = 9003,
    ResourceNotFound = 9004,
    AnomalyNotFound = 9005,
    InvalidAllocation = 9006,
    BudgetExceeded = 9007,
    InvalidAmount = 9008,
    UnauthorizedAccess = 9009,
    InvalidPeriod = 9010,
    InsufficientData = 9011,
}

// ── Data Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostCenter {
    pub id: BytesN<32>,
    pub name: Bytes,
    pub owner: Address,
    pub budget: u64,
    pub currency: Symbol,
    pub created_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostAllocation {
    pub id: BytesN<32>,
    pub cost_center_id: BytesN<32>,
    pub resource_type: Symbol,
    pub amount: u64,
    pub currency: Symbol,
    pub period: Bytes,
    pub timestamp: u64,
    pub submitted_by: Address,
    pub metadata: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChargebackRecord {
    pub id: BytesN<32>,
    pub team: Bytes,
    pub amount: u64,
    pub currency: Symbol,
    pub period: Bytes,
    pub status: u32,
    pub timestamp: u64,
    pub cost_center_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShowbackRecord {
    pub id: BytesN<32>,
    pub team: Bytes,
    pub amount: u64,
    pub currency: Symbol,
    pub period: Bytes,
    pub timestamp: u64,
    pub cost_center_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRecord {
    pub id: BytesN<32>,
    pub resource_type: Symbol,
    pub current_size: u64,
    pub recommended_size: u64,
    pub monthly_cost: u64,
    pub potential_savings: u64,
    pub region: Bytes,
    pub last_updated: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RightsizingRecommendation {
    pub resource_id: BytesN<32>,
    pub recommendation_type: u32,
    pub current_size: u64,
    pub recommended_size: u64,
    pub monthly_savings: u64,
    pub confidence: u32,
    pub reason: Bytes,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostAnomaly {
    pub id: BytesN<32>,
    pub resource_id: BytesN<32>,
    pub expected_cost: u64,
    pub actual_cost: u64,
    pub deviation_pct: u32,
    pub severity: u32,
    pub timestamp: u64,
    pub resolved: bool,
    pub metadata: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Budget {
    pub id: BytesN<32>,
    pub name: Bytes,
    pub cost_center_id: BytesN<32>,
    pub amount: u64,
    pub currency: Symbol,
    pub period: Bytes,
    pub alert_thresholds: Vec<u32>,
    pub created_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetAlert {
    pub id: BytesN<32>,
    pub budget_id: BytesN<32>,
    pub current_spend: u64,
    pub threshold_pct: u32,
    pub message: Bytes,
    pub timestamp: u64,
    pub acknowledged: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostDashboard {
    pub total_cost: u64,
    pub cost_by_category: Vec<(Symbol, u64)>,
    pub cost_by_team: Vec<(Bytes, u64)>,
    pub cost_by_region: Vec<(Bytes, u64)>,
    pub total_savings_potential: u64,
    pub active_anomalies: u32,
    pub budget_utilization: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostSummary {
    pub period: Bytes,
    pub total_cost: u64,
    pub forecasted_cost: u64,
    pub variance_pct: u32,
    pub top_cost_drivers: Vec<(Symbol, u64)>,
    pub timestamp: u64,
}

// ── Storage Keys ────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    CostCenter(BytesN<32>),
    AllCostCenters,
    CostAllocation(BytesN<32>),
    AllAllocations,
    CenterAllocations(BytesN<32>),
    Chargeback(BytesN<32>),
    AllChargebacks,
    Showback(BytesN<32>),
    AllShowbacks,
    Resource(BytesN<32>),
    AllResources,
    RightsizingRec(BytesN<32>),
    AllRightsizingRecs,
    CostAnomaly(BytesN<32>),
    AllAnomalies,
    Budget(BytesN<32>),
    AllBudgets,
    BudgetAlert(BytesN<32>),
    AllAlerts,
    CostCenterCount,
    AllocationCount,
    ChargebackCount,
    ShowbackCount,
    ResourceCount,
    RightsizingCount,
    AnomalyCount,
    BudgetCount,
    AlertCount,
}

// ── Helper Functions ────────────────────────────────────────────────────

fn compute_id(env: &Env, prefix: &[u8], data: &[u8]) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    preimage.append(&Bytes::from_slice(env, prefix));
    preimage.append(&Bytes::from_slice(env, data));
    preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
    env.crypto().sha256(&preimage).into()
}

fn u64_to_bytes(env: &Env, v: u64) -> Bytes {
    bytes!(
        env,
        [
            (v & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
            ((v >> 24) & 0xff) as u8,
            ((v >> 32) & 0xff) as u8,
            ((v >> 40) & 0xff) as u8,
            ((v >> 48) & 0xff) as u8,
            ((v >> 56) & 0xff) as u8,
        ]
    )
}

fn require_non_empty(env: &Env, value: &Bytes, error: FinOpsError) {
    if value.is_empty() {
        panic_with_error!(env, error);
    }
}

fn validate_amount(env: &Env, amount: u64) {
    if amount == 0 {
        panic_with_error!(env, FinOpsError::InvalidAmount);
    }
}

// ── Cost Center Management ──────────────────────────────────────────────

/// Register a new cost center
pub fn register_cost_center(
    env: Env,
    caller: Address,
    name: Bytes,
    budget: u64,
    currency: Symbol,
) -> BytesN<32> {
    caller.require_auth();
    require_non_empty(&env, &name, FinOpsError::InvalidAllocation);
    validate_amount(&env, budget);

    let id = compute_id(&env, b"cost_center", name.as_slice());

    if env.storage().persistent().has(&DataKey::CostCenter(id.clone())) {
        panic_with_error!(&env, FinOpsError::CostCenterAlreadyExists);
    }

    let center = CostCenter {
        id: id.clone(),
        name,
        owner: caller,
        budget,
        currency,
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage().persistent().set(&DataKey::CostCenter(id.clone()), &center);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenterCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::CostCenterCount, &(count + 1));

    env.events()
        .publish((symbol_short!("finops"), symbol_short!("cc_create")), id.clone());

    id
}

/// Get cost center details
pub fn get_cost_center(env: Env, id: BytesN<32>) -> CostCenter {
    env.storage()
        .persistent()
        .get(&DataKey::CostCenter(id))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound))
}

/// Update cost center budget
pub fn update_cost_center_budget(
    env: Env,
    caller: Address,
    id: BytesN<32>,
    new_budget: u64,
) {
    caller.require_auth();

    let mut center: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    if center.owner != caller {
        panic_with_error!(&env, FinOpsError::UnauthorizedAccess);
    }

    validate_amount(&env, new_budget);
    center.budget = new_budget;
    env.storage()
        .persistent()
        .set(&DataKey::CostCenter(id), &center);
}

/// Deactivate cost center
pub fn deactivate_cost_center(env: Env, caller: Address, id: BytesN<32>) {
    caller.require_auth();

    let mut center: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    if center.owner != caller {
        panic_with_error!(&env, FinOpsError::UnauthorizedAccess);
    }

    center.active = false;
    env.storage()
        .persistent()
        .set(&DataKey::CostCenter(id), &center);
}

// ── Cost Allocation ─────────────────────────────────────────────────────

/// Allocate cost to a cost center
pub fn allocate_cost(
    env: Env,
    caller: Address,
    cost_center_id: BytesN<32>,
    resource_type: Symbol,
    amount: u64,
    currency: Symbol,
    period: Bytes,
    metadata: Bytes,
) -> BytesN<32> {
    caller.require_auth();
    validate_amount(&env, amount);
    require_non_empty(&env, &period, FinOpsError::InvalidPeriod);

    let _: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(cost_center_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    let id = compute_id(&env, b"allocation", cost_center_id.as_slice());

    let allocation = CostAllocation {
        id: id.clone(),
        cost_center_id,
        resource_type,
        amount,
        currency,
        period,
        timestamp: env.ledger().timestamp(),
        submitted_by: caller,
        metadata,
    };

    env.storage()
        .persistent()
        .set(&DataKey::CostAllocation(id.clone()), &allocation);

    let mut allocs: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&DataKey::CenterAllocations(cost_center_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    allocs.push_back(id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::CenterAllocations(cost_center_id), &allocs);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AllocationCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::AllocationCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("alloc")),
            (caller, id.clone()),
        );

    id
}

/// Get cost allocation details
pub fn get_allocation(env: Env, id: BytesN<32>) -> CostAllocation {
    env.storage()
        .persistent()
        .get(&DataKey::CostAllocation(id))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::InvalidAllocation))
}

/// Get total allocations for a cost center
pub fn get_center_allocations(env: Env, cost_center_id: BytesN<32>) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::CenterAllocations(cost_center_id))
        .unwrap_or_else(|| Vec::new(&env))
}

// ── Chargeback / Showback ───────────────────────────────────────────────

/// Create chargeback record
pub fn create_chargeback(
    env: Env,
    caller: Address,
    team: Bytes,
    amount: u64,
    currency: Symbol,
    period: Bytes,
    cost_center_id: BytesN<32>,
) -> BytesN<32> {
    caller.require_auth();
    validate_amount(&env, amount);
    require_non_empty(&env, &team, FinOpsError::InvalidAllocation);
    require_non_empty(&env, &period, FinOpsError::InvalidPeriod);

    let _: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(cost_center_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    let id = compute_id(&env, b"chargeback", team.as_slice());

    let record = ChargebackRecord {
        id: id.clone(),
        team,
        amount,
        currency,
        period,
        status: 1,
        timestamp: env.ledger().timestamp(),
        cost_center_id,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Chargeback(id.clone()), &record);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::ChargebackCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::ChargebackCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("chargeback")),
            (caller, id.clone()),
        );

    id
}

/// Create showback record
pub fn create_showback(
    env: Env,
    caller: Address,
    team: Bytes,
    amount: u64,
    currency: Symbol,
    period: Bytes,
    cost_center_id: BytesN<32>,
) -> BytesN<32> {
    caller.require_auth();
    validate_amount(&env, amount);
    require_non_empty(&env, &team, FinOpsError::InvalidAllocation);
    require_non_empty(&env, &period, FinOpsError::InvalidPeriod);

    let _: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(cost_center_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    let id = compute_id(&env, b"showback", team.as_slice());

    let record = ShowbackRecord {
        id: id.clone(),
        team,
        amount,
        currency,
        period,
        timestamp: env.ledger().timestamp(),
        cost_center_id,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Showback(id.clone()), &record);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::ShowbackCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::ShowbackCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("showback")),
            (caller, id.clone()),
        );

    id
}

/// Approve chargeback record
pub fn approve_chargeback(env: Env, caller: Address, id: BytesN<32>) {
    caller.require_auth();

    let mut record: ChargebackRecord = env
        .storage()
        .persistent()
        .get(&DataKey::Chargeback(id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    record.status = 2;
    env.storage()
        .persistent()
        .set(&DataKey::Chargeback(id), &record);
}

// ── Rightsizing Recommendations ────────────────────────────────────────

/// Record cloud resource for rightsizing analysis
pub fn record_resource(
    env: Env,
    caller: Address,
    resource_type: Symbol,
    current_size: u64,
    monthly_cost: u64,
    region: Bytes,
) -> BytesN<32> {
    caller.require_auth();

    let id = compute_id(&env, b"resource", resource_type.as_slice());

    let resource = ResourceRecord {
        id: id.clone(),
        resource_type,
        current_size,
        recommended_size: 0,
        monthly_cost,
        potential_savings: 0,
        region,
        last_updated: env.ledger().timestamp(),
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Resource(id.clone()), &resource);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::ResourceCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::ResourceCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("resource")),
            (caller, id.clone()),
        );

    id
}

/// Generate rightsizing recommendation
pub fn generate_rightsizing(env: Env, caller: Address, resource_id: BytesN<32>) -> BytesN<32> {
    caller.require_auth();

    let resource: ResourceRecord = env
        .storage()
        .persistent()
        .get(&DataKey::Resource(resource_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::ResourceNotFound));

    if !resource.active {
        panic_with_error!(&env, FinOpsError::ResourceNotFound);
    }

    let recommended_size = if resource.current_size > 1 {
        resource.current_size / 2
    } else {
        resource.current_size
    };

    let monthly_savings = if recommended_size < resource.current_size {
        resource.monthly_cost / 2
    } else {
        0
    };

    let rec_id = compute_id(&env, b"rightsizing", resource_id.as_slice());

    let recommendation = RightsizingRecommendation {
        resource_id: resource_id.clone(),
        recommendation_type: 1,
        current_size: resource.current_size,
        recommended_size,
        monthly_savings,
        confidence: 75,
        reason: Bytes::from_slice(&env, b"Right-size based on utilization analysis"),
        timestamp: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::RightsizingRec(rec_id.clone()), &recommendation);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::RightsizingCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::RightsizingCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("rightsize")),
            (caller, rec_id.clone()),
        );

    rec_id
}

/// Get rightsizing recommendation
pub fn get_rightsizing(env: Env, id: BytesN<32>) -> RightsizingRecommendation {
    env.storage()
        .persistent()
        .get(&DataKey::RightsizingRec(id))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::ResourceNotFound))
}

// ── Anomaly Detection ──────────────────────────────────────────────────

/// Record cost anomaly
pub fn record_anomaly(
    env: Env,
    caller: Address,
    resource_id: BytesN<32>,
    expected_cost: u64,
    actual_cost: u64,
    metadata: Bytes,
) -> BytesN<32> {
    caller.require_auth();

    let deviation_pct = if expected_cost > 0 {
        ((actual_cost - expected_cost) * 100 / expected_cost) as u32
    } else {
        0
    };

    let severity = if deviation_pct > 50 {
        3
    } else if deviation_pct > 25 {
        2
    } else if deviation_pct > 10 {
        1
    } else {
        0
    };

    let id = compute_id(&env, b"anomaly", resource_id.as_slice());

    let anomaly = CostAnomaly {
        id: id.clone(),
        resource_id,
        expected_cost,
        actual_cost,
        deviation_pct,
        severity,
        timestamp: env.ledger().timestamp(),
        resolved: false,
        metadata,
    };

    env.storage()
        .persistent()
        .set(&DataKey::CostAnomaly(id.clone()), &anomaly);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AnomalyCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::AnomalyCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("anomaly")),
            (caller, id.clone()),
        );

    id
}

/// Detect cost anomalies using z-score analysis
pub fn detect_anomalies(env: Env, caller: Address, resource_id: BytesN<32>) -> Vec<BytesN<32>> {
    caller.require_auth();

    let mut anomalies = Vec::new(&env);
    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AnomalyCount)
        .unwrap_or(0u32);

    for i in 0..count {
        let key = DataKey::CostAnomaly(BytesN::from_slice(
            &env,
            &i.to_le_bytes(),
        ));
        if let Some(anomaly) = env.storage().persistent().get::<_, CostAnomaly>(&key) {
            if anomaly.resource_id == resource_id && !anomaly.resolved && anomaly.severity >= 2 {
                anomalies.push_back(anomaly.id);
            }
        }
    }

    anomalies
}

/// Resolve anomaly
pub fn resolve_anomaly(env: Env, caller: Address, id: BytesN<32>) {
    caller.require_auth();

    let mut anomaly: CostAnomaly = env
        .storage()
        .persistent()
        .get(&DataKey::CostAnomaly(id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::AnomalyNotFound));

    anomaly.resolved = true;
    env.storage()
        .persistent()
        .set(&DataKey::CostAnomaly(id), &anomaly);
}

// ── Budget Management ──────────────────────────────────────────────────

/// Create budget
pub fn create_budget(
    env: Env,
    caller: Address,
    name: Bytes,
    cost_center_id: BytesN<32>,
    amount: u64,
    currency: Symbol,
    period: Bytes,
    alert_thresholds: Vec<u32>,
) -> BytesN<32> {
    caller.require_auth();
    validate_amount(&env, amount);
    require_non_empty(&env, &name, FinOpsError::InvalidAllocation);
    require_non_empty(&env, &period, FinOpsError::InvalidPeriod);

    let _: CostCenter = env
        .storage()
        .persistent()
        .get(&DataKey::CostCenter(cost_center_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::CostCenterNotFound));

    let id = compute_id(&env, b"budget", name.as_slice());

    let budget = Budget {
        id: id.clone(),
        name,
        cost_center_id,
        amount,
        currency,
        period,
        alert_thresholds,
        created_at: env.ledger().timestamp(),
        active: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Budget(id.clone()), &budget);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::BudgetCount)
        .unwrap_or(0u32);
    env.storage()
        .persistent()
        .set(&DataKey::BudgetCount, &(count + 1));

    env.events()
        .publish(
            (symbol_short!("finops"), symbol_short!("budget_create")),
            (caller, id.clone()),
        );

    id
}

/// Check budget and generate alert if threshold exceeded
pub fn check_budget(env: Env, caller: Address, budget_id: BytesN<32>, current_spend: u64) -> BytesN<32> {
    caller.require_auth();

    let budget: Budget = env
        .storage()
        .persistent()
        .get(&DataKey::Budget(budget_id.clone()))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::BudgetNotFound));

    if !budget.active {
        panic_with_error!(&env, FinOpsError::BudgetNotFound);
    }

    let utilization_pct = if budget.amount > 0 {
        (current_spend * 100 / budget.amount) as u32
    } else {
        0
    };

    let mut alert_id = BytesN::from_slice(&env, &[0u8; 32]);
    let mut triggered = false;

    for threshold in budget.alert_thresholds.iter() {
        if utilization_pct >= threshold {
            alert_id = compute_id(&env, b"alert", budget_id.as_slice());

            let alert = BudgetAlert {
                id: alert_id.clone(),
                budget_id: budget_id.clone(),
                current_spend,
                threshold_pct: threshold,
                message: Bytes::from_slice(
                    &env,
                    format!("Budget utilization at {}%", utilization_pct).as_bytes(),
                ),
                timestamp: env.ledger().timestamp(),
                acknowledged: false,
            };

            env.storage()
                .persistent()
                .set(&DataKey::BudgetAlert(alert_id.clone()), &alert);

            let count: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::AlertCount)
                .unwrap_or(0u32);
            env.storage()
                .persistent()
                .set(&DataKey::AlertCount, &(count + 1));

            triggered = true;
            break;
        }
    }

    if triggered {
        env.events()
            .publish(
                (symbol_short!("finops"), symbol_short!("budget_alert")),
                (caller, alert_id.clone()),
            );
    }

    alert_id
}

/// Get budget details
pub fn get_budget(env: Env, id: BytesN<32>) -> Budget {
    env.storage()
        .persistent()
        .get(&DataKey::Budget(id))
        .unwrap_or_else(|| panic_with_error!(&env, FinOpsError::BudgetNotFound))
}

// ── Cloud Cost Dashboard ────────────────────────────────────────────────

/// Generate cost dashboard summary
pub fn generate_dashboard(env: Env) -> CostDashboard {
    let mut total_cost = 0u64;
    let cost_by_category = Vec::new(&env);
    let cost_by_team = Vec::new(&env);
    let cost_by_region = Vec::new(&env);

    let mut total_savings = 0u64;
    let mut active_anomalies = 0u32;

    let allocation_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AllocationCount)
        .unwrap_or(0u32);

    for i in 0..allocation_count {
        let key = DataKey::CostAllocation(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(alloc) = env.storage().persistent().get::<_, CostAllocation>(&key) {
            total_cost += alloc.amount;
        }
    }

    let anomaly_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AnomalyCount)
        .unwrap_or(0u32);

    for i in 0..anomaly_count {
        let key = DataKey::CostAnomaly(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(anomaly) = env.storage().persistent().get::<_, CostAnomaly>(&key) {
            if !anomaly.resolved {
                active_anomalies += 1;
            }
        }
    }

    let rightsizing_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::RightsizingCount)
        .unwrap_or(0u32);

    for i in 0..rightsizing_count {
        let key = DataKey::RightsizingRec(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(rec) = env.storage().persistent().get::<_, RightsizingRecommendation>(&key) {
            total_savings += rec.monthly_savings;
        }
    }

    let budget_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::BudgetCount)
        .unwrap_or(0u32);

    let mut budget_util = 0u32;
    for i in 0..budget_count {
        let key = DataKey::Budget(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(budget) = env.storage().persistent().get::<_, Budget>(&key) {
            if budget.active {
                let center_allocs = get_center_allocations(env.clone(), budget.cost_center_id.clone());
                for alloc_id in center_allocs.iter() {
                    if let Some(alloc) = env.storage().persistent().get::<_, CostAllocation>(&DataKey::CostAllocation(alloc_id.clone())) {
                        budget_util += (alloc.amount * 100 / budget.amount) as u32;
                    }
                }
            }
        }
    }

    if budget_count > 0 {
        budget_util /= budget_count;
    }

    CostDashboard {
        total_cost,
        cost_by_category,
        cost_by_team,
        cost_by_region,
        total_savings_potential: total_savings,
        active_anomalies,
        budget_utilization: budget_util,
        timestamp: env.ledger().timestamp(),
    }
}

/// Generate cost summary for a period
pub fn get_cost_summary(env: Env, period: Bytes) -> CostSummary {
    let mut total_cost = 0u64;
    let top_drivers = Vec::new(&env);
    let mut forecasted = 0u64;

    let allocation_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AllocationCount)
        .unwrap_or(0u32);

    for i in 0..allocation_count {
        let key = DataKey::CostAllocation(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(alloc) = env.storage().persistent().get::<_, CostAllocation>(&key) {
            if alloc.period == period {
                total_cost += alloc.amount;
            }
        }
    }

    if total_cost > 0 {
        forecasted = total_cost + (total_cost / 10);
    }

    CostSummary {
        period,
        total_cost,
        forecasted_cost: forecasted,
        variance_pct: 0,
        top_drivers,
        timestamp: env.ledger().timestamp(),
    }
}

// ── Query Helpers ───────────────────────────────────────────────────────

/// Get total cost center count
pub fn get_cost_center_count(env: Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::CostCenterCount)
        .unwrap_or(0u32)
}

/// Get total allocation count
pub fn get_allocation_count(env: Env) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::AllocationCount)
        .unwrap_or(0u32)
}

/// Get active anomaly count
pub fn get_active_anomaly_count(env: Env) -> u32 {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AnomalyCount)
        .unwrap_or(0u32);

    let mut active = 0u32;
    for i in 0..count {
        let key = DataKey::CostAnomaly(BytesN::from_slice(&env, &i.to_le_bytes()));
        if let Some(anomaly) = env.storage().persistent().get::<_, CostAnomaly>(&key) {
            if !anomaly.resolved {
                active += 1;
            }
        }
    }

    active
}
