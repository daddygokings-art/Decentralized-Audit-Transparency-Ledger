/// Parametric Insurance for Data Integrity
///
/// Provides automatic payouts based on oracle-verified triggers:
/// - Data loss events
/// - Corruption detection
/// - Availability failures
/// - Event delivery failures
///
/// Architecture:
/// 1. Policy Creation: Define coverage, limits, premiums
/// 2. Capital Pools: Aggregate underwriter capital
/// 3. Claims: Automatic payout on oracle trigger
/// 4. Settlement: Direct transfers without manual intervention

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Policy types (parametric triggers)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum PolicyType {
    /// Data loss: events dropped or not logged
    DataLoss = 0,
    /// Data corruption: integrity check failed
    DataCorruption = 1,
    /// Availability: service unavailable >N minutes
    Availability = 2,
    /// Bridge latency: response time exceeds threshold
    BridgeLatency = 3,
    /// Custom parametric trigger
    Custom = 4,
}

/// Policy status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum PolicyStatus {
    /// Policy active, premium payments on track
    Active = 0,
    /// Policy lapsed (premium unpaid)
    Lapsed = 1,
    /// Policy cancelled by holder
    Cancelled = 2,
    /// Policy matured (no longer accepting claims)
    Matured = 3,
}

/// Insurance policy
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    /// Unique policy ID
    pub policy_id: u64,
    /// Policy type (data loss, corruption, etc.)
    pub policy_type: PolicyType,
    /// Holder/buyer of policy
    pub holder: Address,
    /// Coverage amount (payout if triggered)
    pub coverage_amount: u128,
    /// Annual premium (in stroops)
    pub annual_premium: u128,
    /// Premium payment frequency (ledgers)
    pub premium_frequency_ledgers: u32,
    /// Last premium payment ledger
    pub last_premium_ledger: u32,
    /// Policy expiration ledger
    pub expiration_ledger: u32,
    /// Maximum claims per year
    pub max_claims_per_year: u32,
    /// Claims filed this year
    pub claims_filed_this_year: u32,
    /// Deductible (holder pays first)
    pub deductible: u128,
    /// Current policy status
    pub status: PolicyStatus,
    /// Oracle contract for verification
    pub oracle_address: Address,
    /// Claim trigger threshold/parameter
    pub trigger_parameter: u128,
    /// Capital pool backing this policy
    pub capital_pool_id: u64,
}

/// Capital pool for insurance
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapitalPool {
    /// Unique pool ID
    pub pool_id: u64,
    /// Pool manager/owner
    pub manager: Address,
    /// Total capital contributed
    pub total_capital: u128,
    /// Capital currently available for claims
    pub available_capital: u128,
    /// Capital reserved for existing claims
    pub reserved_capital: u128,
    /// Minimum pool size
    pub minimum_capital: u128,
    /// Share of premiums pool receives
    pub premium_share_bps: u32,
    /// Policies covered by this pool
    pub policy_ids: Vec<u64>,
    /// Total claims paid
    pub total_claims_paid: u128,
    /// Pool fee in basis points
    pub pool_fee_bps: u32,
}

/// Insurance claim (payout event)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    /// Unique claim ID
    pub claim_id: u64,
    /// Policy this claim is for
    pub policy_id: u64,
    /// Claim holder
    pub claimant: Address,
    /// Coverage amount requested
    pub coverage_requested: u128,
    /// Actual verified loss (oracle-determined)
    pub verified_loss: u128,
    /// Deductible applied
    pub deductible_applied: u128,
    /// Payout amount (coverage - deductible)
    pub payout_amount: u128,
    /// Status (pending, approved, rejected, paid)
    pub status: u32,
    /// Oracle verification data
    pub oracle_verification: Bytes,
    /// Ledger claim was filed
    pub filed_ledger: u32,
    /// Ledger claim was resolved
    pub resolved_ledger: u32,
    /// Transaction hash of settlement
    pub settlement_tx_hash: Option<Bytes>,
}

/// Underwriter stake in capital pool
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolShare {
    /// Underwriter address
    pub underwriter: Address,
    /// Pool ID
    pub pool_id: u64,
    /// Capital contributed
    pub capital_contributed: u128,
    /// Share percentage (basis points)
    pub share_bps: u32,
    /// Unrealized losses (claims against this stake)
    pub unrealized_losses: u128,
    /// Realized losses (paid out)
    pub realized_losses: u128,
    /// Rewards earned
    pub rewards_earned: u128,
}

/// Insurance configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceConfig {
    /// Base token (XLM address)
    pub base_token: Address,
    /// Minimum policy coverage
    pub min_coverage: u128,
    /// Maximum policy coverage
    pub max_coverage: u128,
    /// Minimum pool capital
    pub min_pool_capital: u128,
    /// Solvency ratio (basis points, e.g., 2000 = 20%)
    pub solvency_ratio_bps: u32,
    /// Claim processing fee in basis points
    pub claim_fee_bps: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum InsuranceKey {
    /// Owner/admin
    Owner,
    /// Configuration
    Config,
    /// Policy details: u64 (policy_id) → Policy
    Policy(u64),
    /// Next policy ID
    PolicyCounter,
    /// Claim details: u64 (claim_id) → Claim
    Claim(u64),
    /// Next claim ID
    ClaimCounter,
    /// Capital pool: u64 (pool_id) → CapitalPool
    CapitalPool(u64),
    /// Next pool ID
    PoolCounter,
    /// Underwriter shares: (Address, u64) → PoolShare
    PoolShare(Address, u64),
    /// User policies: Address → Vec<u64>
    UserPolicies(Address),
    /// Pool policies: u64 (pool_id) → Vec<u64>
    PoolPolicies(u64),
    /// Total premium collected
    TotalPremiumCollected,
    /// Premium earned by pool: u64 (pool_id) → u128
    PoolPremiumEarned(u64),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InsuranceError {
    /// Unauthorized
    Unauthorized = 1,
    /// Policy not found
    PolicyNotFound = 2,
    /// Invalid coverage amount
    InvalidCoverage = 3,
    /// Insufficient capital
    InsufficientCapital = 4,
    /// Premium payment due
    PremiumPaymentDue = 5,
    /// Policy expired
    PolicyExpired = 6,
    /// Claim limit exceeded
    ClaimLimitExceeded = 7,
    /// Claim not found
    ClaimNotFound = 8,
    /// Pool not found
    PoolNotFound = 9,
    /// Insufficient solvency
    InsufficientSolvency = 10,
    /// Oracle verification failed
    OracleVerificationFailed = 11,
    /// Already claimed for event
    AlreadyClaimed = 12,
    /// Invalid policy status
    InvalidPolicyStatus = 13,
    /// Pool minimum not met
    PoolMinimumNotMet = 14,
    /// Underwriter insufficient stake
    InsufficientUnderwriterStake = 15,
}

// ============================================================================
// Core Insurance Contract
// ============================================================================

#[contract]
pub struct ParametricInsurance;

#[contractimpl]
impl ParametricInsurance {
    /// Initialize insurance system (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        base_token: Address,
        min_coverage: u128,
        max_coverage: u128,
        min_pool_capital: u128,
        solvency_ratio_bps: u32,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&InsuranceKey::Owner) {
            panic_with_error!(&env, InsuranceError::Unauthorized);
        }

        let config = InsuranceConfig {
            base_token,
            min_coverage,
            max_coverage,
            min_pool_capital,
            solvency_ratio_bps,
            claim_fee_bps: 100, // Default 1%
        };

        env.storage().instance().set(&InsuranceKey::Owner, &owner);
        env.storage().instance().set(&InsuranceKey::Config, &config);
        env.storage()
            .instance()
            .set(&InsuranceKey::PolicyCounter, &0u64);
        env.storage()
            .instance()
            .set(&InsuranceKey::ClaimCounter, &0u64);
        env.storage()
            .instance()
            .set(&InsuranceKey::PoolCounter, &0u64);

        log!(
            &env,
            "ParametricInsurance: initialized - min_coverage={}, max_coverage={}",
            min_coverage,
            max_coverage
        );
    }

    // ========================================================================
    // Policy Management
    // ========================================================================

    /// Purchase insurance policy
    pub fn purchase_policy(
        env: Env,
        policy_type: PolicyType,
        coverage_amount: u128,
        annual_premium: u128,
        duration_ledgers: u32,
        max_claims_per_year: u32,
        deductible: u128,
        oracle_address: Address,
        trigger_parameter: u128,
        capital_pool_id: u64,
    ) -> u64 {
        let holder = env.invoker();
        let config = Self::get_config(&env);
        let current_ledger = env.ledger().sequence();

        // Validate coverage amount
        if coverage_amount < config.min_coverage || coverage_amount > config.max_coverage {
            panic_with_error!(&env, InsuranceError::InvalidCoverage);
        }

        // Verify pool exists and has capacity
        let mut pool = Self::get_pool_or_panic(&env, capital_pool_id);
        if pool.available_capital < coverage_amount {
            panic_with_error!(&env, InsuranceError::InsufficientCapital);
        }

        let policy_id = Self::get_next_policy_id(&env);

        let policy = Policy {
            policy_id,
            policy_type,
            holder: holder.clone(),
            coverage_amount,
            annual_premium,
            premium_frequency_ledgers: 52_560, // ~weekly in ledgers
            last_premium_ledger: current_ledger,
            expiration_ledger: current_ledger + duration_ledgers,
            max_claims_per_year,
            claims_filed_this_year: 0,
            deductible,
            status: PolicyStatus::Active,
            oracle_address,
            trigger_parameter,
            capital_pool_id,
        };

        // TODO: Collect first premium from holder
        // Transfer annual_premium from holder to pool

        // Reserve coverage amount from pool
        pool.available_capital -= coverage_amount;
        pool.reserved_capital += coverage_amount;
        pool.policy_ids.push_back(policy_id);

        env.storage()
            .instance()
            .set(&InsuranceKey::Policy(policy_id), &policy);
        env.storage()
            .instance()
            .set(&InsuranceKey::CapitalPool(capital_pool_id), &pool);

        // Track policy for holder
        let mut user_policies = env
            .storage()
            .instance()
            .get::<_, Vec<u64>>(&InsuranceKey::UserPolicies(holder.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        user_policies.push_back(policy_id);
        env.storage()
            .instance()
            .set(&InsuranceKey::UserPolicies(holder.clone()), &user_policies);

        log!(
            &env,
            "ParametricInsurance: policy purchased - id={}, type={}, holder={}, coverage={}",
            policy_id,
            policy_type as u32,
            holder,
            coverage_amount
        );

        policy_id
    }

    /// Pay premium to keep policy active
    pub fn pay_premium(env: Env, policy_id: u64, premium_amount: u128) {
        let payer = env.invoker();
        let mut policy = Self::get_policy_or_panic(&env, policy_id);

        // Verify payer is policy holder
        if payer != policy.holder {
            panic_with_error!(&env, InsuranceError::Unauthorized);
        }

        // Check policy not expired
        let current_ledger = env.ledger().sequence();
        if current_ledger > policy.expiration_ledger {
            panic_with_error!(&env, InsuranceError::PolicyExpired);
        }

        // TODO: Collect premium from payer
        // Transfer premium_amount from payer to pool

        // Update policy state
        policy.last_premium_ledger = current_ledger;
        policy.status = PolicyStatus::Active;

        // Distribute premium to pool
        let mut pool = Self::get_pool_or_panic(&env, policy.capital_pool_id);
        let pool_premium_share = (premium_amount * pool.premium_share_bps as u128) / 10_000;
        let current_earned = env
            .storage()
            .instance()
            .get::<_, u128>(&InsuranceKey::PoolPremiumEarned(pool.pool_id))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&InsuranceKey::PoolPremiumEarned(pool.pool_id), &(current_earned + pool_premium_share));

        env.storage()
            .instance()
            .set(&InsuranceKey::Policy(policy_id), &policy);

        log!(
            &env,
            "ParametricInsurance: premium paid - policy={}, amount={}",
            policy_id,
            premium_amount
        );
    }

    /// Cancel policy and return reserved capital
    pub fn cancel_policy(env: Env, policy_id: u64) {
        let caller = env.invoker();
        let mut policy = Self::get_policy_or_panic(&env, policy_id);

        if caller != policy.holder {
            panic_with_error!(&env, InsuranceError::Unauthorized);
        }

        // Return reserved capital to pool
        let mut pool = Self::get_pool_or_panic(&env, policy.capital_pool_id);
        pool.available_capital += policy.coverage_amount;
        pool.reserved_capital -= policy.coverage_amount;

        policy.status = PolicyStatus::Cancelled;

        env.storage()
            .instance()
            .set(&InsuranceKey::Policy(policy_id), &policy);
        env.storage()
            .instance()
            .set(&InsuranceKey::CapitalPool(policy.capital_pool_id), &pool);

        log!(
            &env,
            "ParametricInsurance: policy cancelled - id={}",
            policy_id
        );
    }

    // ========================================================================
    // Claims Management
    // ========================================================================

    /// File claim on policy
    pub fn file_claim(
        env: Env,
        policy_id: u64,
        oracle_verification: Bytes,
    ) -> u64 {
        let claimant = env.invoker();
        let mut policy = Self::get_policy_or_panic(&env, policy_id);

        // Validate policy
        if policy.status != PolicyStatus::Active {
            panic_with_error!(&env, InsuranceError::InvalidPolicyStatus);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger > policy.expiration_ledger {
            panic_with_error!(&env, InsuranceError::PolicyExpired);
        }

        // Check claim limit
        if policy.claims_filed_this_year >= policy.max_claims_per_year {
            panic_with_error!(&env, InsuranceError::ClaimLimitExceeded);
        }

        let claim_id = Self::get_next_claim_id(&env);

        // TODO: Call oracle to verify event and get verified loss
        let verified_loss = policy.coverage_amount; // Placeholder

        // Calculate payout
        let deductible_to_apply = verified_loss.min(policy.deductible);
        let payout_amount = verified_loss.saturating_sub(deductible_to_apply);

        let claim = Claim {
            claim_id,
            policy_id,
            claimant: claimant.clone(),
            coverage_requested: policy.coverage_amount,
            verified_loss,
            deductible_applied: deductible_to_apply,
            payout_amount,
            status: 1, // Approved
            oracle_verification,
            filed_ledger: current_ledger,
            resolved_ledger: current_ledger,
            settlement_tx_hash: None,
        };

        // Update policy
        policy.claims_filed_this_year += 1;

        // Process payout immediately
        let mut pool = Self::get_pool_or_panic(&env, policy.capital_pool_id);

        // Verify pool solvency
        let config = Self::get_config(&env);
        let required_reserve = (pool.total_capital * config.solvency_ratio_bps as u128) / 10_000;
        if pool.available_capital < (required_reserve + payout_amount) {
            panic_with_error!(&env, InsuranceError::InsufficientSolvency);
        }

        // Pay out claim
        pool.available_capital -= payout_amount;
        pool.total_claims_paid += payout_amount;
        pool.reserved_capital -= verified_loss;

        // TODO: Transfer payout_amount to claimant
        // Transfer base_token from pool to claimant

        env.storage()
            .instance()
            .set(&InsuranceKey::Claim(claim_id), &claim);
        env.storage()
            .instance()
            .set(&InsuranceKey::Policy(policy_id), &policy);
        env.storage()
            .instance()
            .set(&InsuranceKey::CapitalPool(policy.capital_pool_id), &pool);

        log!(
            &env,
            "ParametricInsurance: claim filed and paid - id={}, policy={}, payout={}",
            claim_id,
            policy_id,
            payout_amount
        );

        claim_id
    }

    // ========================================================================
    // Capital Pool Management
    // ========================================================================

    /// Create capital pool
    pub fn create_capital_pool(
        env: Env,
        manager: Address,
        minimum_capital: u128,
        premium_share_bps: u32,
    ) -> u64 {
        manager.require_auth();

        let pool_id = Self::get_next_pool_id(&env);

        let pool = CapitalPool {
            pool_id,
            manager,
            total_capital: 0,
            available_capital: 0,
            reserved_capital: 0,
            minimum_capital,
            premium_share_bps,
            policy_ids: Vec::new(&env),
            total_claims_paid: 0,
            pool_fee_bps: 500, // Default 5%
        };

        env.storage()
            .instance()
            .set(&InsuranceKey::CapitalPool(pool_id), &pool);

        log!(
            &env,
            "ParametricInsurance: capital pool created - id={}, manager={}, min_capital={}",
            pool_id,
            manager,
            minimum_capital
        );

        pool_id
    }

    /// Contribute capital to pool
    pub fn contribute_to_pool(
        env: Env,
        pool_id: u64,
        amount: u128,
    ) {
        let underwriter = env.invoker();
        let mut pool = Self::get_pool_or_panic(&env, pool_id);

        // TODO: Collect capital from underwriter
        // Transfer amount from underwriter to pool

        let total_before = pool.total_capital;
        pool.total_capital += amount;
        pool.available_capital += amount;

        // Calculate and record share
        let share_bps = if total_before > 0 {
            ((pool.total_capital - total_before) * 10_000) / pool.total_capital
        } else {
            10_000 // First contributor gets 100%
        };

        let mut pool_share = env
            .storage()
            .instance()
            .get::<_, PoolShare>(&InsuranceKey::PoolShare(underwriter.clone(), pool_id))
            .unwrap_or(PoolShare {
                underwriter: underwriter.clone(),
                pool_id,
                capital_contributed: 0,
                share_bps: 0,
                unrealized_losses: 0,
                realized_losses: 0,
                rewards_earned: 0,
            });

        pool_share.capital_contributed += amount;
        pool_share.share_bps = share_bps;

        env.storage()
            .instance()
            .set(&InsuranceKey::CapitalPool(pool_id), &pool);
        env.storage()
            .instance()
            .set(&InsuranceKey::PoolShare(underwriter.clone(), pool_id), &pool_share);

        log!(
            &env,
            "ParametricInsurance: capital contributed - pool={}, underwriter={}, amount={}",
            pool_id,
            underwriter,
            amount
        );
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get policy details
    pub fn get_policy(env: Env, policy_id: u64) -> Option<Policy> {
        env.storage()
            .instance()
            .get(&InsuranceKey::Policy(policy_id))
    }

    /// Get claim details
    pub fn get_claim(env: Env, claim_id: u64) -> Option<Claim> {
        env.storage()
            .instance()
            .get(&InsuranceKey::Claim(claim_id))
    }

    /// Get capital pool details
    pub fn get_capital_pool(env: Env, pool_id: u64) -> Option<CapitalPool> {
        env.storage()
            .instance()
            .get(&InsuranceKey::CapitalPool(pool_id))
    }

    /// Get insurance configuration
    pub fn get_insurance_config(env: Env) -> InsuranceConfig {
        Self::get_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_config(env: &Env) -> InsuranceConfig {
        env.storage()
            .instance()
            .get(&InsuranceKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, InsuranceError::Unauthorized))
    }

    fn get_policy_or_panic(env: &Env, policy_id: u64) -> Policy {
        env.storage()
            .instance()
            .get(&InsuranceKey::Policy(policy_id))
            .unwrap_or_else(|| panic_with_error!(env, InsuranceError::PolicyNotFound))
    }

    fn get_pool_or_panic(env: &Env, pool_id: u64) -> CapitalPool {
        env.storage()
            .instance()
            .get(&InsuranceKey::CapitalPool(pool_id))
            .unwrap_or_else(|| panic_with_error!(env, InsuranceError::PoolNotFound))
    }

    fn get_next_policy_id(env: &Env) -> u64 {
        let current = env
            .storage()
            .instance()
            .get::<_, u64>(&InsuranceKey::PolicyCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&InsuranceKey::PolicyCounter, &(current + 1));

        current
    }

    fn get_next_claim_id(env: &Env) -> u64 {
        let current = env
            .storage()
            .instance()
            .get::<_, u64>(&InsuranceKey::ClaimCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&InsuranceKey::ClaimCounter, &(current + 1));

        current
    }

    fn get_next_pool_id(env: &Env) -> u64 {
        let current = env
            .storage()
            .instance()
            .get::<_, u64>(&InsuranceKey::PoolCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&InsuranceKey::PoolCounter, &(current + 1));

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_creation() {
        // Tests for policy purchase
    }

    #[test]
    fn test_claim_filing() {
        // Tests for claim process
    }

    #[test]
    fn test_pool_management() {
        // Tests for capital pools
    }
}
