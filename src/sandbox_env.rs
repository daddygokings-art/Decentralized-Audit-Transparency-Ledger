#![no_std]

use crate::sandbox_types::*;
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Vec};

/// Isolated sandbox environment for controlled testing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxInstance {
    /// Sandbox ID
    pub sandbox_id: BytesN<32>,
    /// Participant ID
    pub participant_id: BytesN<32>,
    /// Environment level
    pub environment: u8, // SandboxEnvironment as u8
    /// Relaxed requirements active
    pub relaxed_requirements: RelaxedRequirements,
    /// Daily volume used
    pub daily_volume_used: u128,
    /// Daily volume limit
    pub daily_volume_limit: u128,
    /// Is isolated (true) or using shadow ledger (false)
    pub is_fully_isolated: bool,
    /// Transaction count
    pub transaction_count: u32,
    /// Sandbox state hash
    pub state_hash: BytesN<32>,
}

/// Transaction attempt result in sandbox.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransactionApprovalStatus {
    /// Approved and executed
    Approved = 0,
    /// Rejected - limit exceeded
    LimitExceeded = 1,
    /// Rejected - compliance check failed
    ComplianceFailed = 2,
    /// Rejected - invalid transaction
    InvalidTransaction = 3,
    /// Pending review
    PendingReview = 4,
}

/// Sandbox environment manager.
pub struct EnvironmentManager;

impl EnvironmentManager {
    /// Create sandbox instance for participant
    pub fn create_sandbox_instance(
        env: &Env,
        participant_id: BytesN<32>,
        environment: SandboxEnvironment,
    ) -> Result<SandboxInstance, &'static str> {
        let sandbox_id = Self::compute_sandbox_id(env, &participant_id);
        let relaxed_reqs = match environment {
            SandboxEnvironment::Level1PoC => RelaxedRequirements::new_level1(),
            SandboxEnvironment::Level2Beta => RelaxedRequirements::new_level2(),
            SandboxEnvironment::Level3Production => RelaxedRequirements::new_level3(),
        };

        let daily_limit = environment.max_daily_volume();

        Ok(SandboxInstance {
            sandbox_id,
            participant_id,
            environment: environment as u8,
            relaxed_requirements: relaxed_reqs,
            daily_volume_used: 0,
            daily_volume_limit: daily_limit,
            is_fully_isolated: true,
            transaction_count: 0,
            state_hash: BytesN::zero(),
        })
    }

    /// Compute sandbox ID
    pub fn compute_sandbox_id(env: &Env, participant_id: &BytesN<32>) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, participant_id.as_ref()));
        input.append(&Bytes::from_slice(env, b"SANDBOX"));

        env.crypto().sha256(&input)
    }

    /// Check transaction against sandbox limits
    pub fn check_transaction_limits(
        sandbox: &SandboxInstance,
        transaction_amount: u128,
    ) -> Result<TransactionApprovalStatus, &'static str> {
        // Get environment for max transaction check
        let env_level = match sandbox.environment {
            e if e == SandboxEnvironment::Level1PoC as u8 => SandboxEnvironment::Level1PoC,
            e if e == SandboxEnvironment::Level2Beta as u8 => SandboxEnvironment::Level2Beta,
            e if e == SandboxEnvironment::Level3Production as u8 => SandboxEnvironment::Level3Production,
            _ => return Err("Invalid environment"),
        };

        let max_tx = env_level.max_transaction_amount();
        if transaction_amount > max_tx {
            return Ok(TransactionApprovalStatus::LimitExceeded);
        }

        if sandbox.daily_volume_used + transaction_amount > sandbox.daily_volume_limit {
            return Ok(TransactionApprovalStatus::LimitExceeded);
        }

        Ok(TransactionApprovalStatus::Approved)
    }

    /// Execute transaction in sandbox
    pub fn execute_sandbox_transaction(
        sandbox: &mut SandboxInstance,
        amount: u128,
    ) -> Result<TransactionApprovalStatus, &'static str> {
        // Check limits first
        let approval = Self::check_transaction_limits(sandbox, amount)?;

        if approval != TransactionApprovalStatus::Approved {
            return Ok(approval);
        }

        // Apply to sandbox
        sandbox.daily_volume_used = sandbox.daily_volume_used.saturating_add(amount);
        sandbox.transaction_count += 1;

        Ok(TransactionApprovalStatus::Approved)
    }

    /// Reset daily limits
    pub fn reset_daily_limits(sandbox: &mut SandboxInstance) {
        sandbox.daily_volume_used = 0;
    }

    /// Compute compliance score for sandbox usage (0-100)
    pub fn compute_compliance_score(
        sandbox: &SandboxInstance,
        failed_checks: u32,
        total_checks: u32,
    ) -> u32 {
        if total_checks == 0 {
            return 100;
        }

        let passed = total_checks.saturating_sub(failed_checks);
        ((passed as u64 * 100) / total_checks as u64) as u32
    }

    /// Check if relaxed requirements are being abused
    pub fn detect_abuse(
        sandbox: &SandboxInstance,
        transaction_count: u32,
        failed_compliance: u32,
    ) -> bool {
        // Simple abuse detection: high volume + many compliance failures
        let abuse_ratio = if transaction_count > 0 {
            (failed_compliance as u64 * 100) / transaction_count as u64
        } else {
            0
        };

        abuse_ratio > 30 // More than 30% failure rate indicates potential abuse
    }

    /// Get required compliance checks for environment
    pub fn get_required_compliance_checks(environment: SandboxEnvironment) -> u32 {
        environment.required_compliance_checks()
    }

    /// Update sandbox state hash
    pub fn update_state_hash(env: &Env, sandbox: &mut SandboxInstance) {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, sandbox.sandbox_id.as_ref()));
        input.append(&Bytes::from_slice(env, &sandbox.daily_volume_used.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &sandbox.transaction_count.to_le_bytes()));

        sandbox.state_hash = env.crypto().sha256(&input);
    }
}

/// Environment usage statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentStatistics {
    /// Transactions executed
    pub transactions_executed: u32,
    /// Transactions failed
    pub transactions_failed: u32,
    /// Total volume processed
    pub total_volume: u128,
    /// Compliance check score (0-100)
    pub compliance_score: u32,
    /// Usage of daily limits (0-100%)
    pub daily_limit_usage: u32,
}

impl EnvironmentStatistics {
    pub fn success_rate(&self) -> u32 {
        let total = self.transactions_executed.saturating_add(self.transactions_failed);
        if total == 0 {
            return 100;
        }

        ((self.transactions_executed as u64 * 100) / total as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let env = soroban_sdk::Env::default();
        let participant_id = BytesN::zero();

        let sandbox =
            EnvironmentManager::create_sandbox_instance(&env, participant_id, SandboxEnvironment::Level1PoC)
                .unwrap();

        assert!(sandbox.is_fully_isolated);
        assert_eq!(sandbox.transaction_count, 0);
    }

    #[test]
    fn test_transaction_limits() {
        let env = soroban_sdk::Env::default();
        let mut sandbox = EnvironmentManager::create_sandbox_instance(
            &env,
            BytesN::zero(),
            SandboxEnvironment::Level1PoC,
        )
        .unwrap();

        // Valid transaction
        let result =
            EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 1_000_00).unwrap();
        assert_eq!(result, TransactionApprovalStatus::Approved);
        assert_eq!(sandbox.transaction_count, 1);

        // Exceeds daily limit
        let result =
            EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 200_000_00).unwrap();
        assert_eq!(result, TransactionApprovalStatus::LimitExceeded);
    }

    #[test]
    fn test_daily_reset() {
        let env = soroban_sdk::Env::default();
        let mut sandbox = EnvironmentManager::create_sandbox_instance(
            &env,
            BytesN::zero(),
            SandboxEnvironment::Level2Beta,
        )
        .unwrap();

        EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 50_000_00).unwrap();
        assert!(sandbox.daily_volume_used > 0);

        EnvironmentManager::reset_daily_limits(&mut sandbox);
        assert_eq!(sandbox.daily_volume_used, 0);
    }

    #[test]
    fn test_abuse_detection() {
        let env = soroban_sdk::Env::default();
        let sandbox = EnvironmentManager::create_sandbox_instance(
            &env,
            BytesN::zero(),
            SandboxEnvironment::Level1PoC,
        )
        .unwrap();

        // Low failure rate - no abuse
        assert!(!EnvironmentManager::detect_abuse(&sandbox, 100, 10));

        // High failure rate - abuse
        assert!(EnvironmentManager::detect_abuse(&sandbox, 100, 40));
    }
}
