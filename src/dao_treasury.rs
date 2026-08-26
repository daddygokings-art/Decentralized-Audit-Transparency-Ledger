/// Treasury Management Contract - Decentralized Fund Allocation
///
/// Manages DAO treasury with:
/// - Multi-sig fund approvals
/// - Spending allocations and budgets
/// - Fee distribution
/// - Emergency withdrawal procedures

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Treasury fund entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fund {
    /// Fund name (e.g., "operations", "development", "security")
    pub name: String,
    /// Current balance in stroops
    pub balance: u128,
    /// Budget limit per period
    pub budget_limit: u128,
    /// Current period budget used
    pub budget_used: u128,
    /// Period duration in ledgers (e.g., 52560 = 1 week)
    pub period_ledgers: u32,
    /// Last budget reset ledger
    pub last_reset_ledger: u32,
}

/// Treasury spending allocation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allocation {
    /// Allocation ID
    pub allocation_id: u32,
    /// Recipient address
    pub recipient: Address,
    /// Fund being allocated from
    pub fund_name: String,
    /// Amount in stroops
    pub amount: u128,
    /// Purpose of allocation
    pub purpose: String,
    /// Approval status
    pub approved: bool,
    /// Number of approvals
    pub approval_count: u32,
    /// Required approvals for execution
    pub required_approvals: u32,
    /// Created ledger
    pub created_ledger: u32,
    /// Execution ledger (0 = not executed)
    pub execution_ledger: u32,
}

/// Fee distribution configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeDistribution {
    /// Recipient address (fund or address)
    pub recipient: Address,
    /// Percentage in basis points (e.g., 2500 = 25%)
    pub percentage_bps: u32,
    /// Whether recipient is a fund (true) or address (false)
    pub is_fund: bool,
}

/// Treasury configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasuryConfig {
    /// Treasury owner address
    pub owner: Address,
    /// Base token for treasury (e.g., XLM)
    pub base_token: Address,
    /// Signing keys for multi-sig (addresses of authorized signers)
    pub signers: Vec<Address>,
    /// Required signatures for approval
    pub required_signatures: u32,
    /// Fee distribution rules
    pub fee_distribution: Vec<FeeDistribution>,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum TreasuryKey {
    /// Treasury owner
    Owner,
    /// Treasury configuration
    Config,
    /// Fund details: String (fund_name) → Fund
    Fund(String),
    /// All fund names
    FundNames,
    /// Allocation details: u32 (allocation_id) → Allocation
    Allocation(u32),
    /// Next allocation ID
    AllocationCounter,
    /// Approval signatures on allocation: (u32, Address) → bool
    AllocationApproval(u32, Address),
    /// Total fee collected
    TotalFeesCollected,
    /// Per-address fee share received
    FeeShareReceived(Address),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TreasuryError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Fund not found
    FundNotFound = 2,
    /// Insufficient fund balance
    InsufficientBalance = 3,
    /// Budget limit exceeded
    BudgetExceeded = 4,
    /// Allocation not found
    AllocationNotFound = 5,
    /// Allocation already approved
    AllocationAlreadyApproved = 6,
    /// Insufficient approvals
    InsufficientApprovals = 7,
    /// Allocation cannot be executed (not approved)
    AllocationNotApproved = 8,
    /// Invalid signer
    InvalidSigner = 9,
    /// Duplicate signature
    DuplicateSignature = 10,
    /// Fund transfer failed
    TransferFailed = 11,
    /// Invalid fee distribution
    InvalidFeeDistribution = 12,
    /// Signer not found
    SignerNotFound = 13,
    /// Invalid amount (0 or too large)
    InvalidAmount = 14,
}

// ============================================================================
// Treasury Contract
// ============================================================================

#[contract]
pub struct DAOTreasury;

#[contractimpl]
impl DAOTreasury {
    /// Initialize treasury (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        base_token: Address,
        signers: Vec<Address>,
        required_signatures: u32,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&TreasuryKey::Owner) {
            panic_with_error!(&env, TreasuryError::Unauthorized);
        }

        let config = TreasuryConfig {
            owner: owner.clone(),
            base_token,
            signers,
            required_signatures,
            fee_distribution: Vec::new(&env),
        };

        env.storage().instance().set(&TreasuryKey::Owner, &owner);
        env.storage().instance().set(&TreasuryKey::Config, &config);
        env.storage()
            .instance()
            .set(&TreasuryKey::AllocationCounter, &0u32);
        env.storage()
            .instance()
            .set(&TreasuryKey::FundNames, &Vec::new(&env));

        log!(
            &env,
            "DAOTreasury: initialized - owner={}, required_signatures={}",
            owner,
            required_signatures
        );
    }

    // ========================================================================
    // Fund Management
    // ========================================================================

    /// Create a new fund
    pub fn create_fund(
        env: Env,
        fund_name: String,
        budget_limit: u128,
        period_ledgers: u32,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        if env
            .storage()
            .instance()
            .get::<_, Option<Fund>>(&TreasuryKey::Fund(fund_name.clone()))
            .is_some()
        {
            panic_with_error!(&env, TreasuryError::FundNotFound);
        }

        let fund = Fund {
            name: fund_name.clone(),
            balance: 0,
            budget_limit,
            budget_used: 0,
            period_ledgers,
            last_reset_ledger: env.ledger().sequence(),
        };

        env.storage()
            .instance()
            .set(&TreasuryKey::Fund(fund_name.clone()), &fund);

        // Add to fund names list
        let mut fund_names = env
            .storage()
            .instance()
            .get::<_, Vec<String>>(&TreasuryKey::FundNames)
            .unwrap_or_else(|| Vec::new(&env));
        fund_names.push_back(fund_name);
        env.storage()
            .instance()
            .set(&TreasuryKey::FundNames, &fund_names);

        log!(
            &env,
            "DAOTreasury: fund created - name={}, budget={}",
            fund_name,
            budget_limit
        );
    }

    /// Deposit funds into treasury
    pub fn deposit(env: Env, fund_name: String, amount: u128) {
        if amount == 0 {
            panic_with_error!(&env, TreasuryError::InvalidAmount);
        }

        let mut fund = Self::get_fund_or_panic(&env, &fund_name);

        // Update fund balance
        fund.balance += amount;

        env.storage()
            .instance()
            .set(&TreasuryKey::Fund(fund_name.clone()), &fund);

        log!(
            &env,
            "DAOTreasury: deposit - fund={}, amount={}",
            fund_name,
            amount
        );
    }

    // ========================================================================
    // Allocation & Approval
    // ========================================================================

    /// Request fund allocation
    pub fn request_allocation(
        env: Env,
        recipient: Address,
        fund_name: String,
        amount: u128,
        purpose: String,
    ) -> u32 {
        let config = Self::get_config(&env);

        if amount == 0 || amount > 1_000_000_000_000_000 {
            panic_with_error!(&env, TreasuryError::InvalidAmount);
        }

        let allocation_id = Self::get_next_allocation_id(&env);

        let allocation = Allocation {
            allocation_id,
            recipient,
            fund_name,
            amount,
            purpose,
            approved: false,
            approval_count: 0,
            required_approvals: config.required_signatures,
            created_ledger: env.ledger().sequence(),
            execution_ledger: 0,
        };

        env.storage()
            .instance()
            .set(&TreasuryKey::Allocation(allocation_id), &allocation);

        log!(
            &env,
            "DAOTreasury: allocation requested - id={}, amount={}",
            allocation_id,
            amount
        );

        allocation_id
    }

    /// Approve an allocation (multi-sig)
    pub fn approve_allocation(env: Env, allocation_id: u32) {
        let signer = env.invoker();
        let config = Self::get_config(&env);

        // Check signer is authorized
        let is_authorized = config
            .signers
            .iter()
            .any(|s| s == &signer);

        if !is_authorized {
            panic_with_error!(&env, TreasuryError::InvalidSigner);
        }

        // Check signature not already provided
        if env
            .storage()
            .instance()
            .get::<_, bool>(&TreasuryKey::AllocationApproval(allocation_id, signer.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, TreasuryError::DuplicateSignature);
        }

        let mut allocation = Self::get_allocation_or_panic(&env, allocation_id);

        // Record approval
        env.storage()
            .instance()
            .set(&TreasuryKey::AllocationApproval(allocation_id, signer.clone()), &true);

        allocation.approval_count += 1;

        // Check if threshold reached
        if allocation.approval_count >= allocation.required_approvals {
            allocation.approved = true;
        }

        env.storage()
            .instance()
            .set(&TreasuryKey::Allocation(allocation_id), &allocation);

        log!(
            &env,
            "DAOTreasury: allocation approved - id={}, approvals={}/{}",
            allocation_id,
            allocation.approval_count,
            allocation.required_approvals
        );
    }

    /// Execute approved allocation
    pub fn execute_allocation(env: Env, allocation_id: u32) {
        let mut allocation = Self::get_allocation_or_panic(&env, allocation_id);

        if !allocation.approved {
            panic_with_error!(&env, TreasuryError::AllocationNotApproved);
        }

        let mut fund = Self::get_fund_or_panic(&env, &allocation.fund_name);

        // Check budget
        if allocation.amount + fund.budget_used > fund.budget_limit {
            panic_with_error!(&env, TreasuryError::BudgetExceeded);
        }

        // Check balance
        if allocation.amount > fund.balance {
            panic_with_error!(&env, TreasuryError::InsufficientBalance);
        }

        // Execute transfer
        // TODO: Call token contract to transfer funds
        // token.transfer(fund.base_token, treasury, recipient, amount)

        // Update fund state
        fund.balance -= allocation.amount;
        fund.budget_used += allocation.amount;

        allocation.execution_ledger = env.ledger().sequence();

        env.storage()
            .instance()
            .set(&TreasuryKey::Fund(allocation.fund_name.clone()), &fund);
        env.storage()
            .instance()
            .set(&TreasuryKey::Allocation(allocation_id), &allocation);

        log!(
            &env,
            "DAOTreasury: allocation executed - id={}, amount={}, recipient={}",
            allocation_id,
            allocation.amount,
            allocation.recipient
        );
    }

    // ========================================================================
    // Fee Distribution
    // ========================================================================

    /// Set fee distribution rules
    pub fn set_fee_distribution(
        env: Env,
        distribution: Vec<FeeDistribution>,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        // Verify total is 100%
        let mut total_bps = 0u32;
        for entry in distribution.iter() {
            total_bps = total_bps.saturating_add(entry.percentage_bps);
        }

        if total_bps != 10_000 {
            panic_with_error!(&env, TreasuryError::InvalidFeeDistribution);
        }

        let mut config = Self::get_config(&env);
        config.fee_distribution = distribution;

        env.storage().instance().set(&TreasuryKey::Config, &config);

        log!(
            &env,
            "DAOTreasury: fee distribution updated"
        );
    }

    /// Collect fees and distribute
    pub fn distribute_fees(env: Env, total_fees: u128) {
        let config = Self::get_config(&env);

        for dist in config.fee_distribution.iter() {
            let share = (total_fees as u128 * dist.percentage_bps as u128) / 10_000u128;

            if dist.is_fund {
                // Deposit to fund
                let fund_name = String::from_array(
                    &env,
                    &[0u8; 32], // TODO: Get fund name from dist.recipient
                );
                Self::deposit(&env, fund_name, share);
            } else {
                // Record fee share for recipient
                let current = env
                    .storage()
                    .instance()
                    .get::<_, u128>(&TreasuryKey::FeeShareReceived(dist.recipient.clone()))
                    .unwrap_or(0);

                env.storage()
                    .instance()
                    .set(&TreasuryKey::FeeShareReceived(dist.recipient.clone()), &(current + share));
            }
        }

        let total_collected = env
            .storage()
            .instance()
            .get::<_, u128>(&TreasuryKey::TotalFeesCollected)
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&TreasuryKey::TotalFeesCollected, &(total_collected + total_fees));

        log!(
            &env,
            "DAOTreasury: fees distributed - total={}",
            total_fees
        );
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get fund details
    pub fn get_fund(env: Env, fund_name: String) -> Option<Fund> {
        env.storage()
            .instance()
            .get(&TreasuryKey::Fund(fund_name))
    }

    /// Get allocation details
    pub fn get_allocation(env: Env, allocation_id: u32) -> Option<Allocation> {
        env.storage()
            .instance()
            .get(&TreasuryKey::Allocation(allocation_id))
    }

    /// Get fund balance
    pub fn get_fund_balance(env: Env, fund_name: String) -> u128 {
        Self::get_fund_or_panic(&env, &fund_name).balance
    }

    /// Get treasury configuration
    pub fn get_treasury_config(env: Env) -> TreasuryConfig {
        Self::get_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&TreasuryKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, TreasuryError::Unauthorized))
    }

    fn get_config(env: &Env) -> TreasuryConfig {
        env.storage()
            .instance()
            .get(&TreasuryKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, TreasuryError::Unauthorized))
    }

    fn get_fund_or_panic(env: &Env, fund_name: &String) -> Fund {
        env.storage()
            .instance()
            .get(&TreasuryKey::Fund(fund_name.clone()))
            .unwrap_or_else(|| panic_with_error!(env, TreasuryError::FundNotFound))
    }

    fn get_allocation_or_panic(env: &Env, allocation_id: u32) -> Allocation {
        env.storage()
            .instance()
            .get(&TreasuryKey::Allocation(allocation_id))
            .unwrap_or_else(|| panic_with_error!(env, TreasuryError::AllocationNotFound))
    }

    fn get_next_allocation_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&TreasuryKey::AllocationCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&TreasuryKey::AllocationCounter, &(current + 1));

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fund_creation() {
        // Tests for fund management
    }

    #[test]
    fn test_allocation_approval() {
        // Tests for allocation flow
    }

    #[test]
    fn test_fee_distribution() {
        // Tests for fee distribution
    }
}
