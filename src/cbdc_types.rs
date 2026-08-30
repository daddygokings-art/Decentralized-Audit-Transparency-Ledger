#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol};

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
    pub fn as_symbol(&self) -> Symbol {
        match self {
            CBDCPilot::DigitalEuro => Symbol::new(&[b"EUR"]),
            CBDCPilot::DigitalDollar => Symbol::new(&[b"USD"]),
            CBDCPilot::eCNY => Symbol::new(&[b"CNY"]),
            CBDCPilot::SandDollar => Symbol::new(&[b"BSD"]),
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
    pub fn as_symbol(&self) -> Symbol {
        match self {
            InteropProtocol::AtomicSwap => Symbol::new(&[b"ATOMIC_SWAP"]),
            InteropProtocol::HubAndSpoke => Symbol::new(&[b"HUB_SPOKE"]),
            InteropProtocol::ISO20022 => Symbol::new(&[b"ISO_20022"]),
            InteropProtocol::CBPR => Symbol::new(&[b"CBPR"]),
        }
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
    pub fn as_symbol(&self) -> Symbol {
        match self {
            PrivacyTier::Public => Symbol::new(&[b"PUBLIC"]),
            PrivacyTier::Pseudonymous => Symbol::new(&[b"PSEUDO"]),
            PrivacyTier::Private => Symbol::new(&[b"PRIVATE"]),
            PrivacyTier::RegulatoryConfidential => Symbol::new(&[b"REGUL"]),
        }
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
    pub fn as_symbol(&self) -> Symbol {
        match self {
            OfflineStatus::PendingReconciliation => Symbol::new(&[b"PENDING"]),
            OfflineStatus::Reconciled => Symbol::new(&[b"RECON"]),
            OfflineStatus::FailedReconciliation => Symbol::new(&[b"FAILED"]),
            OfflineStatus::Disputed => Symbol::new(&[b"DISPUTE"]),
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
    pub source_pilot: u8, // CBDCPilot as u8
    /// Destination CBDC pilot
    pub dest_pilot: u8, // CBDCPilot as u8
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
    pub protocol: u8, // InteropProtocol as u8
    /// Privacy tier for this transaction
    pub privacy_tier: u8, // PrivacyTier as u8
    /// Optional offline status
    pub offline_status: Option<u8>, // OfflineStatus as u8
    /// Transaction metadata
    pub metadata: Bytes,
}

impl CBDCTransaction {
    /// Computes content-hash for transaction (similar to audit log events)
    pub fn compute_hash(&self, prev_hash: &BytesN<32>) -> BytesN<32> {
        
        let mut input = soroban_sdk::Bytes::new(prev_hash.env());

        // Append serializable fields for hashing
        input.append(&Bytes::from_slice(
            &self.tx_id.env(),
            self.tx_id.as_ref(),
        ));
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

        self.tx_id.env().crypto().sha256(&input)
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
    pub settlement_status: u8, // OfflineStatus as u8
    /// Timestamp of batch creation
    pub created_at: u64,
    /// Timestamp of settlement
    pub settled_at: Option<u64>,
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
}
