#![no_std]

use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, contracttype};

/// Represents supported CBDC pilots globally.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CBDCPilot {
    /// European Central Bank Digital Euro (€-CBDC)
    DigitalEuro = 0,
    /// U.S. Digital Dollar Pilot (USD-CBDC)
    DigitalDollar = 1,
    /// Chinese Digital Yuan / e-CNY (¥-CBDC)
    eCNY = 2,
    /// Bahamas Sand Dollar (BSD-CBDC)
    SandDollar = 3,
}

impl CBDCPilot {
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            CBDCPilot::DigitalEuro => Symbol::new(env, "EUR"),
            CBDCPilot::DigitalDollar => Symbol::new(env, "USD"),
            CBDCPilot::eCNY => Symbol::new(env, "CNY"),
            CBDCPilot::SandDollar => Symbol::new(env, "BSD"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CBDCPilot::DigitalEuro => "DIGITAL_EURO",
            CBDCPilot::DigitalDollar => "DIGITAL_DOLLAR",
            CBDCPilot::eCNY => "E_CNY",
            CBDCPilot::SandDollar => "SAND_DOLLAR",
        }
    }

    pub fn currency_code(&self) -> &'static str {
        match self {
            CBDCPilot::DigitalEuro => "EUR",
            CBDCPilot::DigitalDollar => "USD",
            CBDCPilot::eCNY => "CNY",
            CBDCPilot::SandDollar => "BSD",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "EUR" => Some(CBDCPilot::DigitalEuro),
            "USD" => Some(CBDCPilot::DigitalDollar),
            "CNY" => Some(CBDCPilot::eCNY),
            "BSD" => Some(CBDCPilot::SandDollar),
            _ => None,
        }
    }
}

/// Interoperability framework for cross-CBDC operations.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InteropProtocol {
    /// Direct peer-to-peer atomic swap between two CBDCs
    AtomicSwap = 0,
    /// Multi-step settlement via a neutral hub intermediary
    HubAndSpoke = 1,
    /// Standardized messaging protocol (ISO 20022)
    ISO20022 = 2,
    /// Cross-border payment standard using instant settlement
    CBPR = 3,
}

impl InteropProtocol {
    /// Short on-chain code for this protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            InteropProtocol::AtomicSwap => "ATOMIC_SWAP",
            InteropProtocol::HubAndSpoke => "HUB_SPOKE",
            InteropProtocol::ISO20022 => "ISO_20022",
            InteropProtocol::CBPR => "CBPR",
        }
    }

    pub fn as_symbol(&self, env: &Env) -> Symbol {
        Symbol::new(env, self.as_str())
    }

    pub fn version(&self) -> &'static str {
        match self {
            InteropProtocol::AtomicSwap => "1.0",
            InteropProtocol::HubAndSpoke => "1.0",
            InteropProtocol::ISO20022 => "20.2",
            InteropProtocol::CBPR => "1.0",
        }
    }
}

/// Privacy tier classification for CBDC transactions.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PrivacyTier {
    /// Fully public: submitter, amount, recipient visible on-chain
    Public = 0,
    /// Semi-private: amounts encrypted, addresses visible
    Pseudonymous = 1,
    /// Private: all sensitive data encrypted, only hash visible
    Private = 2,
    /// Regulatory: encrypted with access for central bank regulators only
    RegulatoryConfidential = 3,
}

impl PrivacyTier {
    /// Short on-chain code for this privacy tier.
    pub fn as_str(&self) -> &'static str {
        match self {
            PrivacyTier::Public => "PUBLIC",
            PrivacyTier::Pseudonymous => "PSEUDO",
            PrivacyTier::Private => "PRIVATE",
            PrivacyTier::RegulatoryConfidential => "REGUL",
        }
    }

    pub fn as_symbol(&self, env: &Env) -> Symbol {
        Symbol::new(env, self.as_str())
    }

    pub fn requires_encryption(&self) -> bool {
        matches!(
            self,
            PrivacyTier::Pseudonymous
                | PrivacyTier::Private
                | PrivacyTier::RegulatoryConfidential
        )
    }

    pub fn visibility_level(&self) -> u8 {
        *self as u8
    }
}

/// Offline transaction status.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OfflineStatus {
    /// Transaction created offline, pending reconciliation
    PendingReconciliation = 0,
    /// Successfully reconciled and settled on-chain
    Reconciled = 1,
    /// Failed reconciliation (conflict or validation error)
    FailedReconciliation = 2,
    /// Marked for dispute or reversal
    Disputed = 3,
}

impl OfflineStatus {
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            OfflineStatus::PendingReconciliation => Symbol::new(env, "PENDING"),
            OfflineStatus::Reconciled => Symbol::new(env, "RECON"),
            OfflineStatus::FailedReconciliation => Symbol::new(env, "FAILED"),
            OfflineStatus::Disputed => Symbol::new(env, "DISPUTE"),
        }
    }

    pub fn is_settled(&self) -> bool {
        matches!(self, OfflineStatus::Reconciled)
    }
}

/// Represents a CBDC transaction with cross-pilot interoperability.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBDCTransaction {
    /// Unique transaction ID (generated offline or on-chain)
    pub tx_id: BytesN<32>,
    /// Source CBDC pilot
    pub source_pilot: u32, // CBDCPilot as u8
    /// Destination CBDC pilot
    pub dest_pilot: u32, // CBDCPilot as u8
    /// Sending account address
    pub from: Address,
    /// Receiving account address
    pub to: Address,
    /// Amount in source pilot's base unit
    pub amount_source: u128,
    /// Amount in destination pilot's base unit (computed post-conversion)
    pub amount_dest: u128,
    /// Exchange rate applied (in fixed-point: rate * 1e18)
    pub exchange_rate: u128,
    /// Timestamp of transaction creation
    pub timestamp: u64,
    /// Interoperability protocol used
    pub protocol: u32, // InteropProtocol as u8
    /// Privacy tier for this transaction
    pub privacy_tier: u32, // PrivacyTier as u8
    /// Optional offline status
    pub offline_status: Option<u32>, // OfflineStatus
    /// Transaction metadata
    pub metadata: Bytes,
}

impl CBDCTransaction {
    /// Computes content-hash for transaction (similar to audit log events)
    pub fn compute_hash(&self, prev_hash: &BytesN<32>) -> BytesN<32> {
        
        let mut input = soroban_sdk::Bytes::new(prev_hash.env());

        // Append serializable fields for hashing
        input.append(&self.tx_id.to_bytes());
        input.append(&Bytes::from_slice(&self.tx_id.env(), &self.source_pilot.to_le_bytes()));
        input.append(&Bytes::from_slice(&self.tx_id.env(), &self.dest_pilot.to_le_bytes()));
        input.append(&Bytes::from_slice(
            &self.tx_id.env(),
            &self.amount_source.to_le_bytes(),
        ));
        input.append(&Bytes::from_slice(
            &self.tx_id.env(),
            &self.amount_dest.to_le_bytes(),
        ));
        input.append(&Bytes::from_slice(
            &self.tx_id.env(),
            &self.exchange_rate.to_le_bytes(),
        ));
        input.append(&Bytes::from_slice(&self.tx_id.env(), &self.timestamp.to_le_bytes()));
        input.append(&self.metadata);

        env.crypto().sha256(&input)
    }
}

/// Represents batch settlement for offline transactions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchSettlement {
    /// Batch ID
    pub batch_id: BytesN<32>,
    /// List of transaction IDs in this batch
    pub transaction_ids: soroban_sdk::Vec<BytesN<32>>,
    /// Total batch amount (source pilot)
    pub total_amount: u128,
    /// Settlement status
    pub settlement_status: u32, // OfflineStatus as u8
    /// Timestamp of batch creation
    pub created_at: u64,
    /// Timestamp of settlement
    pub settled_at: Option<u64>,
}

/// Deterministic, typed error variants for CBDC interoperability operations.
///
/// These variants are returned by `src/cbdc_interop.rs` instead of panicking
/// when boundary conditions, malformed payloads, or arithmetic overflow are
/// encountered during high-throughput Soroban ledger execution.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CBDCInteropError {
    /// Arithmetic operation would overflow the target integer type.
    ArithmeticOverflow = 0,
    /// Arithmetic operation would underflow the target integer type.
    ArithmeticUnderflow = 1,
    /// Division or modulo by zero was attempted.
    DivisionByZero = 2,
    /// A collection exceeded its explicit maximum allowed length.
    CollectionTooLong = 3,
    /// A collection was empty where at least one element is required.
    EmptyCollection = 4,
    /// A payload was malformed or could not be decoded.
    MalformedPayload = 5,
    /// A payload exceeded the maximum allowed byte length.
    PayloadTooLarge = 6,
    /// An amount was outside the configured `[min_tx_amount, max_tx_amount]` bounds.
    AmountOutOfBounds = 7,
    /// An exchange rate was zero or otherwise invalid.
    InvalidExchangeRate = 8,
    /// A supplied enum discriminant did not map to a known variant.
    InvalidEnumDiscriminant = 9,
    /// A timestamp was earlier than the referenced lower bound.
    InvalidTimestamp = 10,
    /// A batch settlement referenced more transactions than permitted.
    BatchSizeExceeded = 11,
    /// A batch settlement total did not match the sum of its entries.
    BatchTotalMismatch = 12,
    /// The requested operation is not permitted for the current state.
    InvalidState = 13,
}

impl CBDCInteropError {
    /// Stable numeric code for this error, suitable for on-chain storage.
    pub fn code(&self) -> u32 {
        *self as u32
    }

    /// Short on-chain symbol for this error.
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            CBDCInteropError::ArithmeticOverflow => Symbol::new(env, "OVRFLOW"),
            CBDCInteropError::ArithmeticUnderflow => Symbol::new(env, "UNDRFLOW"),
            CBDCInteropError::DivisionByZero => Symbol::new(env, "DIV_ZERO"),
            CBDCInteropError::CollectionTooLong => Symbol::new(env, "TOO_LONG"),
            CBDCInteropError::EmptyCollection => Symbol::new(env, "EMPTY"),
            CBDCInteropError::MalformedPayload => Symbol::new(env, "MALFORMED"),
            CBDCInteropError::PayloadTooLarge => Symbol::new(env, "TOO_LARGE"),
            CBDCInteropError::AmountOutOfBounds => Symbol::new(env, "AMT_BOUND"),
            CBDCInteropError::InvalidExchangeRate => Symbol::new(env, "BAD_RATE"),
            CBDCInteropError::InvalidEnumDiscriminant => Symbol::new(env, "BAD_ENUM"),
            CBDCInteropError::InvalidTimestamp => Symbol::new(env, "BAD_TIME"),
            CBDCInteropError::BatchSizeExceeded => Symbol::new(env, "BATCH_MAX"),
            CBDCInteropError::BatchTotalMismatch => Symbol::new(env, "BATCH_SUM"),
            CBDCInteropError::InvalidState => Symbol::new(env, "BAD_STATE"),
        }
    }
}

/// Explicit maximum collection lengths enforced before writing state entries.
///
/// These bounds cap storage footprint and CPU instruction usage for the
/// worst-case payload accepted by `src/cbdc_interop.rs`.
pub mod limits {
    /// Maximum number of transaction IDs permitted in a single batch settlement.
    pub const MAX_BATCH_TRANSACTIONS: u32 = 1_000;
    /// Maximum byte length of transaction metadata.
    pub const MAX_METADATA_BYTES: u32 = 4_096;
    /// Maximum byte length of an ISO-20022 message payload.
    pub const MAX_ISO20022_PAYLOAD_BYTES: u32 = 16_384;
    /// Maximum number of entries in a single cross-ledger settlement leg list.
    pub const MAX_SETTLEMENT_LEGS: u32 = 64;
}

/// Configuration for CBDC interoperability operations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBDCConfig {
    /// Maximum amount per transaction
    pub max_tx_amount: u128,
    /// Minimum amount per transaction
    pub min_tx_amount: u128,
    /// Exchange rate update interval (in seconds)
    pub exchange_rate_update_interval: u64,
    /// Maximum offline batch size
    pub max_batch_size: u32,
    /// Whether offline mode is enabled
    pub offline_mode_enabled: bool,
}

impl CBDCConfig {
    pub fn default() -> Self {
        CBDCConfig {
            max_tx_amount: 1_000_000_00, // 1M units
            min_tx_amount: 1_00, // 1 unit
            exchange_rate_update_interval: 3600, // 1 hour
            max_batch_size: 1000,
            offline_mode_enabled: true,
        }
    }

    pub fn is_valid_amount(&self, amount: u128) -> bool {
        amount >= self.min_tx_amount && amount <= self.max_tx_amount
    }

    /// Validates an amount, returning a typed error instead of panicking.
    pub fn check_amount(&self, amount: u128) -> Result<(), CBDCInteropError> {
        if amount < self.min_tx_amount || amount > self.max_tx_amount {
            return Err(CBDCInteropError::AmountOutOfBounds);
        }
        Ok(())
    }

    /// Validates a batch size against both the config and the hard limit.
    pub fn check_batch_size(&self, size: u32) -> Result<(), CBDCInteropError> {
        if size > self.max_batch_size || size > limits::MAX_BATCH_TRANSACTIONS {
            return Err(CBDCInteropError::BatchSizeExceeded);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbdc_pilot_conversions() {
        assert_eq!(CBDCPilot::DigitalEuro.currency_code(), "EUR");
        assert_eq!(CBDCPilot::DigitalDollar.currency_code(), "USD");
        assert_eq!(CBDCPilot::eCNY.currency_code(), "CNY");
        assert_eq!(CBDCPilot::SandDollar.currency_code(), "BSD");
    }

    #[test]
    fn test_cbdc_pilot_from_code() {
        assert_eq!(CBDCPilot::from_code("EUR"), Some(CBDCPilot::DigitalEuro));
        assert_eq!(CBDCPilot::from_code("USD"), Some(CBDCPilot::DigitalDollar));
        assert_eq!(CBDCPilot::from_code("CNY"), Some(CBDCPilot::eCNY));
        assert_eq!(CBDCPilot::from_code("BSD"), Some(CBDCPilot::SandDollar));
        assert_eq!(CBDCPilot::from_code("GBP"), None);
    }

    #[test]
    fn test_privacy_tier_encryption_requirement() {
        assert!(!PrivacyTier::Public.requires_encryption());
        assert!(PrivacyTier::Pseudonymous.requires_encryption());
        assert!(PrivacyTier::Private.requires_encryption());
        assert!(PrivacyTier::RegulatoryConfidential.requires_encryption());
    }

    #[test]
    fn test_offline_status() {
        assert!(!OfflineStatus::PendingReconciliation.is_settled());
        assert!(OfflineStatus::Reconciled.is_settled());
        assert!(!OfflineStatus::FailedReconciliation.is_settled());
        assert!(!OfflineStatus::Disputed.is_settled());
    }

    #[test]
    fn test_cbdc_config_validation() {
        let config = CBDCConfig::default();
        assert!(config.is_valid_amount(100));
        assert!(!config.is_valid_amount(0));
        assert!(!config.is_valid_amount(2_000_000_00));
    }

    #[test]
    fn test_interop_error_codes_are_stable() {
        assert_eq!(CBDCInteropError::ArithmeticOverflow.code(), 0);
        assert_eq!(CBDCInteropError::CollectionTooLong.code(), 3);
        assert_eq!(CBDCInteropError::MalformedPayload.code(), 5);
        assert_eq!(CBDCInteropError::InvalidState.code(), 13);
    }

    #[test]
    fn test_check_amount_boundaries() {
        let config = CBDCConfig::default();
        assert_eq!(config.check_amount(config.min_tx_amount), Ok(()));
        assert_eq!(config.check_amount(config.max_tx_amount), Ok(()));
        assert_eq!(
            config.check_amount(config.min_tx_amount - 1),
            Err(CBDCInteropError::AmountOutOfBounds)
        );
        assert_eq!(
            config.check_amount(config.max_tx_amount + 1),
            Err(CBDCInteropError::AmountOutOfBounds)
        );
        assert_eq!(
            config.check_amount(u128::MAX),
            Err(CBDCInteropError::AmountOutOfBounds)
        );
    }

    #[test]
    fn test_check_batch_size_boundaries() {
        let config = CBDCConfig::default();
        assert_eq!(config.check_batch_size(0), Ok(()));
        assert_eq!(config.check_batch_size(config.max_batch_size), Ok(()));
        assert_eq!(
            config.check_batch_size(config.max_batch_size + 1),
            Err(CBDCInteropError::BatchSizeExceeded)
        );
        assert_eq!(
            config.check_batch_size(u32::MAX),
            Err(CBDCInteropError::BatchSizeExceeded)
        );
    }

    #[test]
    fn test_limits_are_bounded() {
        assert!(limits::MAX_BATCH_TRANSACTIONS > 0);
        assert!(limits::MAX_METADATA_BYTES > 0);
        assert!(limits::MAX_ISO20022_PAYLOAD_BYTES > 0);
        assert!(limits::MAX_SETTLEMENT_LEGS > 0);
    }
}
