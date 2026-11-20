use soroban_sdk::contracterror;

/// Errors surfaced by the tamper-evidence subsystem.
///
/// Partition-related variants are grouped so callers can distinguish
/// transient (retryable) conditions from permanent integrity failures.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Generic, unspecified failure.
    Unknown = 0,
    /// Caller is not authorized for this operation.
    Unauthorized = 1,
    /// Requested record was not found.
    NotFound = 2,
    /// Input failed validation.
    InvalidInput = 3,
    /// Operation attempted in an invalid state.
    InvalidState = 4,
    /// Cryptographic proof failed verification.
    ProofVerificationFailed = 5,
    /// Proof generation produced a non-deterministic result.
    NonDeterministicProof = 6,
    /// Audit chain integrity check failed.
    IntegrityViolation = 7,
    /// A network partition is currently observed; the operation is
    /// transiently unavailable and may be retried once quorum is restored.
    PartitionDetected = 8,
    /// Quorum of peers could not be reached to confirm the operation.
    QuorumUnavailable = 9,
    /// The operation was rejected because the node is in recovery mode.
    RecoveryInProgress = 10,
    /// A recovery checkpoint is stale or inconsistent with the audit chain.
    StaleRecoveryCheckpoint = 11,
    /// Retry budget for a partitioned operation has been exhausted.
    RetryExhausted = 12,
}

impl Error {
    /// Returns `true` when the error is transient and the caller may
    /// safely retry the operation after a backoff (e.g. once the
    /// partition heals or quorum is restored).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::PartitionDetected
                | Error::QuorumUnavailable
                | Error::RecoveryInProgress
                | Error::RetryExhausted
        )
    }

    /// Returns `true` when the error indicates a permanent integrity
    /// or cryptographic failure that must not be retried blindly.
    pub fn is_integrity_failure(&self) -> bool {
        matches!(
            self,
            Error::ProofVerificationFailed
                | Error::NonDeterministicProof
                | Error::IntegrityViolation
                | Error::StaleRecoveryCheckpoint
        )
    }
}
