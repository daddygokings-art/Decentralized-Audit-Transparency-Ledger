#![no_std]

use crate::cbdc_types::{CBDCPilot, CBDCTransaction, InteropProtocol};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Exchange rate between two CBDC pilots.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangeRate {
    /// Source CBDC pilot
    pub source_pilot: u8, // CBDCPilot as u8
    /// Destination CBDC pilot
    pub dest_pilot: u8, // CBDCPilot as u8
    /// Rate scaled by 1e18 for precision
    pub rate: u128,
    /// Timestamp of rate (for staleness checks)
    pub timestamp: u64,
    /// Bid price (lower)
    pub bid_price: u128,
    /// Ask price (higher)
    pub ask_price: u128,
    /// Maximum volatility tolerated (percentage * 100)
    pub max_volatility: u32,
}

impl ExchangeRate {
    /// Check if rate is stale (older than 1 hour by default)
    pub fn is_stale(&self, current_time: u64, staleness_threshold: u64) -> bool {
        current_time.saturating_sub(self.timestamp) > staleness_threshold
    }

    /// Compute mid-price between bid and ask
    pub fn mid_price(&self) -> u128 {
        (self.bid_price.saturating_add(self.ask_price)) / 2
    }

    /// Apply spread from mid-price
    pub fn spread_bps(&self) -> u128 {
        if self.mid_price() == 0 {
            return 0;
        }
        ((self.ask_price - self.bid_price) * 10000) / self.mid_price()
    }
}

/// Represents a settlement instruction for cross-CBDC transfer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementInstruction {
    /// Settlement ID
    pub settlement_id: BytesN<32>,
    /// Source transaction
    pub transaction: CBDCTransaction,
    /// Interoperability protocol
    pub protocol: u8, // InteropProtocol as u8
    /// Settlement status
    pub status: u8, // SettlementStatus as u8
    /// Settlement timestamp
    pub timestamp: u64,
    /// Confirmation hash from destination ledger
    pub confirmation_hash: Option<BytesN<32>>,
}

/// Settlement status enumeration.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementStatus {
    /// Instruction created, awaiting execution
    Created = 0,
    /// Settlement initiated on source ledger
    InitiatedSource = 1,
    /// Settlement confirmed on source ledger
    ConfirmedSource = 2,
    /// Settlement awaiting confirmation on destination
    AwaitingDestination = 3,
    /// Settlement completed on both ledgers
    Completed = 4,
    /// Settlement failed or disputed
    Failed = 5,
}

impl SettlementStatus {
    pub fn as_symbol(&self) -> Symbol {
        match self {
            SettlementStatus::Created => Symbol::new(&[b"CREATED"]),
            SettlementStatus::InitiatedSource => Symbol::new(&[b"INIT_SRC"]),
            SettlementStatus::ConfirmedSource => Symbol::new(&[b"CONF_SRC"]),
            SettlementStatus::AwaitingDestination => Symbol::new(&[b"AWAIT_DST"]),
            SettlementStatus::Completed => Symbol::new(&[b"COMPLETE"]),
            SettlementStatus::Failed => Symbol::new(&[b"FAILED"]),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SettlementStatus::Completed | SettlementStatus::Failed)
    }
}

/// Interoperability operation manager.
pub struct InteropManager;

impl InteropManager {
    /// Validate exchange rate for transaction
    pub fn validate_exchange_rate(
        rate: &ExchangeRate,
        current_time: u64,
        staleness_threshold: u64,
    ) -> Result<(), &'static str> {
        if rate.is_stale(current_time, staleness_threshold) {
            return Err("Exchange rate is stale");
        }
        if rate.rate == 0 {
            return Err("Exchange rate is zero");
        }
        if rate.bid_price == 0 || rate.ask_price == 0 {
            return Err("Bid/ask prices cannot be zero");
        }
        if rate.bid_price > rate.ask_price {
            return Err("Bid price cannot exceed ask price");
        }
        Ok(())
    }

    /// Convert amount using exchange rate
    pub fn convert_amount(amount: u128, rate: u128) -> Result<u128, &'static str> {
        if rate == 0 {
            return Err("Exchange rate cannot be zero");
        }
        if amount == 0 {
            return Err("Amount cannot be zero");
        }

        // Calculate: (amount * rate) / 1e18, handling overflow
        let scaled_amount = amount
            .checked_mul(rate)
            .ok_or("Conversion overflow")?
            .checked_div(1_000_000_000_000_000_000)
            .ok_or("Division error")?;

        Ok(scaled_amount)
    }

    /// Execute atomic swap protocol
    pub fn execute_atomic_swap(
        env: &Env,
        from: &Address,
        to: &Address,
        source_amount: u128,
        dest_amount: u128,
        timeout_ledgers: u32,
    ) -> Result<BytesN<32>, &'static str> {
        if source_amount == 0 || dest_amount == 0 {
            return Err("Amounts cannot be zero");
        }

        // Generate swap ID
                let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, from.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, to.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, &source_amount.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &dest_amount.to_le_bytes()));

        Ok(env.crypto().sha256(&input))
    }

    /// Execute hub-and-spoke settlement
    pub fn execute_hub_and_spoke(
        env: &Env,
        hub_address: &Address,
        source_amount: u128,
        dest_amount: u128,
    ) -> Result<BytesN<32>, &'static str> {
        if source_amount == 0 || dest_amount == 0 {
            return Err("Amounts cannot be zero");
        }

        // Generate settlement ID
                let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, hub_address.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, &source_amount.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &dest_amount.to_le_bytes()));

        Ok(env.crypto().sha256(&input))
    }

    /// Validate settlement instruction
    pub fn validate_settlement_instruction(
        instruction: &SettlementInstruction,
    ) -> Result<(), &'static str> {
        if instruction.transaction.amount_source == 0 || instruction.transaction.amount_dest == 0
        {
            return Err("Settlement amounts cannot be zero");
        }
        if instruction.transaction.exchange_rate == 0 {
            return Err("Settlement exchange rate cannot be zero");
        }
        Ok(())
    }

    /// Get protocol version string
    pub fn protocol_version(protocol: InteropProtocol) -> &'static str {
        protocol.version()
    }

    /// Determine settlement status transition validity
    pub fn is_valid_status_transition(
        from: SettlementStatus,
        to: SettlementStatus,
    ) -> bool {
        match (from, to) {
            (SettlementStatus::Created, SettlementStatus::InitiatedSource) => true,
            (SettlementStatus::InitiatedSource, SettlementStatus::ConfirmedSource) => true,
            (SettlementStatus::ConfirmedSource, SettlementStatus::AwaitingDestination) => true,
            (SettlementStatus::AwaitingDestination, SettlementStatus::Completed) => true,
            (SettlementStatus::AwaitingDestination, SettlementStatus::Failed) => true,
            (_, SettlementStatus::Failed) => true, // Can fail from any state
            _ => false,
        }
    }

    /// Compute settlement fee based on amount
    pub fn compute_settlement_fee(amount: u128, fee_bps: u32) -> Result<u128, &'static str> {
        if fee_bps > 10000 {
            return Err("Fee basis points cannot exceed 10000");
        }

        let fee = (amount as u128)
            .checked_mul(fee_bps as u128)
            .ok_or("Fee calculation overflow")?
            .checked_div(10000)
            .ok_or("Fee division error")?;

        Ok(fee)
    }

    /// Validate cross-pilot compatibility
    pub fn validate_pilot_pair(source: u8, dest: u8) -> Result<(), &'static str> {
        if source == dest {
            return Err("Source and destination pilots must differ");
        }
        if source > 3 || dest > 3 {
            return Err("Invalid pilot identifier");
        }
        Ok(())
    }
}

/// Settlement path finding for multi-hop transfers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPath {
    /// Sequence of pilots in path (e.g., [EUR, USD, CNY])
    pub path: soroban_sdk::Vec<u8>,
    /// Exchange rates for each hop
    pub rates: soroban_sdk::Vec<u128>,
    /// Total cost in basis points
    pub total_cost_bps: u32,
}

impl SettlementPath {
    /// Calculate final amount after all conversions
    pub fn compute_final_amount(&self, initial_amount: u128) -> Result<u128, &'static str> {
        if self.path.len() < 2 {
            return Err("Path must have at least 2 pilots");
        }
        if self.path.len() as u32 != self.rates.len() as u32 {
            return Err("Path length mismatch");
        }

        let mut amount = initial_amount;
        for rate in self.rates.iter() {
            amount = InteropManager::convert_amount(amount, *rate)?;
        }

        Ok(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_rate_validation() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            rate: 1_000_000_000_000_000_000,
            timestamp: 1000,
            bid_price: 0_950_000_000_000_000_000,
            ask_price: 1_050_000_000_000_000_000,
            max_volatility: 500,
        };

        assert!(InteropManager::validate_exchange_rate(&rate, 2000, 3600).is_ok());
        assert!(InteropManager::validate_exchange_rate(&rate, 10000, 3600).is_err()); // stale
    }

    #[test]
    fn test_conversion_calculation() {
        let amount = 1_000_000_000_000_000_000u128; // 1e18
        let rate = 1_200_000_000_000_000_000u128; // 1.2x rate

        let result = InteropManager::convert_amount(amount, rate).unwrap();
        assert_eq!(result, 1_200_000_000_000_000_000);
    }

    #[test]
    fn test_settlement_fee_calculation() {
        let amount = 1_000_000u128;
        let fee_bps = 25; // 0.25%

        let fee = InteropManager::compute_settlement_fee(amount, fee_bps).unwrap();
        assert_eq!(fee, 250);
    }

    #[test]
    fn test_status_transitions() {
        assert!(InteropManager::is_valid_status_transition(
            SettlementStatus::Created,
            SettlementStatus::InitiatedSource
        ));
        assert!(!InteropManager::is_valid_status_transition(
            SettlementStatus::Created,
            SettlementStatus::Completed
        ));
        assert!(InteropManager::is_valid_status_transition(
            SettlementStatus::AwaitingDestination,
            SettlementStatus::Failed
        ));
    }
}
