#![no_std]

use crate::cbdc_types::{CBDCPilot, CBDCTransaction, InteropProtocol, OfflineStatus, PrivacyTier};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Represents a CBDC transaction event logged to the audit trail.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBDCEvent {
    /// Global event index
    pub event_index: u32,
    /// Timestamp of event logging
    pub timestamp: u64,
    /// CBDC transaction details
    pub transaction: CBDCTransaction,
    /// Event type (e.g., "transaction", "settlement", "reconciliation")
    pub event_type: Symbol,
    /// Event status (success, pending, failed)
    pub status: Symbol,
    /// Optional error message if failed
    pub error_message: Option<Bytes>,
}

impl CBDCEvent {
    pub fn transaction_success(
        env: &Env,
        event_index: u32,
        timestamp: u64,
        transaction: CBDCTransaction,
        event_type: Symbol,
    ) -> Self {
        CBDCEvent {
            event_index,
            timestamp,
            transaction,
            event_type,
            status: Symbol::new(env, "success"),
            error_message: None,
        }
    }

    pub fn transaction_failed(
        env: &Env,
        event_index: u32,
        timestamp: u64,
        transaction: CBDCTransaction,
        event_type: Symbol,
        error: Bytes,
    ) -> Self {
        CBDCEvent {
            event_index,
            timestamp,
            transaction,
            event_type,
            status: Symbol::new(env, "failed"),
            error_message: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status == Symbol::new(self.transaction.tx_id.env(), "success")
    }
}

/// Configuration for CBDC event logging.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBDCEventConfig {
    /// Maximum number of CBDC events to store
    pub max_events: u32,
    /// Current CBDC event count
    pub event_count: u32,
    /// Whether to log all transactions (true) or only settlements (false)
    pub log_all_transactions: bool,
    /// Event retention period in seconds (0 = unlimited)
    pub retention_seconds: u64,
}

impl CBDCEventConfig {
    pub fn default() -> Self {
        CBDCEventConfig {
            max_events: 100_000,
            event_count: 0,
            log_all_transactions: true,
            retention_seconds: 0,
        }
    }

    pub fn is_full(&self) -> bool {
        self.event_count >= self.max_events
    }

    pub fn can_log_more(&self) -> bool {
        !self.is_full()
    }

    pub fn increment_count(&mut self) -> Result<(), &'static str> {
        if self.is_full() {
            return Err("CBDC event log is full");
        }
        self.event_count += 1;
        Ok(())
    }
}

/// Transaction event type classification.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransactionEventType {
    /// Standard cross-CBDC transaction
    CrossCBDCTransfer = 0,
    /// Settlement of offline transactions
    BatchSettlement = 1,
    /// Exchange rate update
    ExchangeRateUpdate = 2,
    /// Reconciliation of offline batches
    Reconciliation = 3,
    /// Dispute or reversal of transaction
    DisputeReversal = 4,
}

impl TransactionEventType {
    pub fn as_symbol(&self) -> Symbol {
        match self {
            TransactionEventType::CrossCBDCTransfer => Symbol::new(&[b"XFER"]),
            TransactionEventType::BatchSettlement => Symbol::new(&[b"BATCH"]),
            TransactionEventType::ExchangeRateUpdate => Symbol::new(&[b"RATE"]),
            TransactionEventType::Reconciliation => Symbol::new(&[b"RECON"]),
            TransactionEventType::DisputeReversal => Symbol::new(&[b"DISPUTE"]),
        }
    }
}

/// Summary statistics for CBDC events.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBDCEventStats {
    /// Total successful transactions logged
    pub total_success: u32,
    /// Total failed transactions logged
    pub total_failed: u32,
    /// Total amount transferred (in source pilot units)
    pub total_volume: u128,
    /// Most recent transaction timestamp
    pub last_transaction_timestamp: u64,
    /// Events per CBDC pilot
    pub events_per_pilot: soroban_sdk::Vec<(u8, u32)>, // (pilot_id, count)
}

impl CBDCEventStats {
    pub fn new(env: &Env) -> Self {
        CBDCEventStats {
            total_success: 0,
            total_failed: 0,
            total_volume: 0,
            last_transaction_timestamp: 0,
            events_per_pilot: Vec::new(env),
        }
    }

    pub fn record_success(&mut self, amount: u128, timestamp: u64) {
        self.total_success += 1;
        self.total_volume = self.total_volume.saturating_add(amount);
        self.last_transaction_timestamp = timestamp;
    }

    pub fn record_failure(&mut self) {
        self.total_failed += 1;
    }

    pub fn success_rate(&self) -> u32 {
        let total = self.total_success + self.total_failed;
        if total == 0 {
            return 100;
        }
        ((self.total_success as u64 * 100) / total as u64) as u32
    }
}

/// CBDC transaction logger implementation.
pub struct CBDCLogger;

impl CBDCLogger {
    /// Log a successful CBDC transaction
    pub fn log_transaction_success(
        env: &Env,
        event_index: u32,
        transaction: CBDCTransaction,
        event_type: TransactionEventType,
    ) -> CBDCEvent {
        let timestamp = env.ledger().timestamp();
        CBDCEvent::transaction_success(
            env,
            event_index,
            timestamp,
            transaction,
            event_type.as_symbol(),
        )
    }

    /// Log a failed CBDC transaction
    pub fn log_transaction_failed(
        env: &Env,
        event_index: u32,
        transaction: CBDCTransaction,
        event_type: TransactionEventType,
        error_msg: &str,
    ) -> CBDCEvent {
        let timestamp = env.ledger().timestamp();
        let error_bytes = Bytes::from_slice(env, error_msg.as_bytes());
        CBDCEvent::transaction_failed(
            env,
            event_index,
            timestamp,
            transaction,
            event_type.as_symbol(),
            error_bytes,
        )
    }

    /// Create metadata for event logging
    pub fn create_metadata(
        env: &Env,
        source: CBDCPilot,
        dest: CBDCPilot,
        protocol: InteropProtocol,
        privacy_tier: PrivacyTier,
    ) -> Bytes {
        // Serialize CBDC context into metadata bytes
        let mut metadata = Bytes::new(env);

        // Append pilot codes
        metadata.append(&Bytes::from_slice(env, source.currency_code().as_bytes()));
        metadata.append(&Bytes::from_slice(env, b":"));
        metadata.append(&Bytes::from_slice(env, dest.currency_code().as_bytes()));
        metadata.append(&Bytes::from_slice(env, b":"));

        // Append protocol info
        metadata.append(&Bytes::from_slice(env, protocol.as_symbol().to_string().as_bytes()));
        metadata.append(&Bytes::from_slice(env, b":"));

        // Append privacy tier
        metadata.append(&Bytes::from_slice(
            env,
            privacy_tier.as_symbol().to_string().as_bytes(),
        ));

        metadata
    }

    /// Validate transaction before logging
    pub fn validate_transaction(tx: &CBDCTransaction) -> Result<(), &'static str> {
        if tx.amount_source == 0 {
            return Err("Transaction amount cannot be zero");
        }
        if tx.amount_dest == 0 {
            return Err("Destination amount cannot be zero");
        }
        if tx.exchange_rate == 0 {
            return Err("Exchange rate cannot be zero");
        }
        if tx.source_pilot == tx.dest_pilot {
            return Err("Source and destination pilots cannot be the same");
        }
        Ok(())
    }

    /// Extract pilot from transaction
    pub fn get_source_pilot(tx: &CBDCTransaction) -> Result<CBDCPilot, &'static str> {
        match tx.source_pilot {
            0 => Ok(CBDCPilot::DigitalEuro),
            1 => Ok(CBDCPilot::DigitalDollar),
            2 => Ok(CBDCPilot::eCNY),
            3 => Ok(CBDCPilot::SandDollar),
            _ => Err("Invalid source pilot"),
        }
    }

    /// Extract destination pilot from transaction
    pub fn get_dest_pilot(tx: &CBDCTransaction) -> Result<CBDCPilot, &'static str> {
        match tx.dest_pilot {
            0 => Ok(CBDCPilot::DigitalEuro),
            1 => Ok(CBDCPilot::DigitalDollar),
            2 => Ok(CBDCPilot::eCNY),
            3 => Ok(CBDCPilot::SandDollar),
            _ => Err("Invalid destination pilot"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbdc_event_config_full() {
        let mut config = CBDCEventConfig::default();
        assert!(config.can_log_more());

        config.event_count = config.max_events;
        assert!(!config.can_log_more());
        assert!(config.is_full());
    }

    #[test]
    fn test_cbdc_event_config_increment() {
        let mut config = CBDCEventConfig::default();
        assert!(config.increment_count().is_ok());
        assert_eq!(config.event_count, 1);

        config.event_count = config.max_events;
        assert!(config.increment_count().is_err());
    }

    #[test]
    fn test_cbdc_event_stats_success_rate() {
        let config = CBDCEventConfig::default();
        let mut stats = CBDCEventStats::new(&soroban_sdk::Env::default());

        stats.total_success = 80;
        stats.total_failed = 20;
        assert_eq!(stats.success_rate(), 80);

        stats.total_success = 0;
        stats.total_failed = 0;
        assert_eq!(stats.success_rate(), 100);
    }

    #[test]
    fn test_transaction_event_type_conversions() {
        assert_eq!(
            TransactionEventType::CrossCBDCTransfer.as_symbol().to_string(),
            "XFER"
        );
        assert_eq!(
            TransactionEventType::BatchSettlement.as_symbol().to_string(),
            "BATCH"
        );
    }
}
