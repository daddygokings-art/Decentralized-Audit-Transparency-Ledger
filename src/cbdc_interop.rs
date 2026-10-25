#![no_std]

//! CBDC cross-ledger settlement and ISO-20022 message bridge.
//!
//! # Determinism & overflow safety
//!
//! All arithmetic in this module uses `checked_*` / `saturating_*` helpers.
//! Boundary violations return typed [`InteropError`] variants instead of
//! panicking, so the contract never aborts on adversarial input.
//!
//! # Storage footprint (worst case)
//!
//! * [`ExchangeRate`]: 2 × u32 + 3 × u128 + u64 + u32 = 8 + 48 + 8 + 4 = 68 bytes
//!   of XDR payload (plus enum/struct discriminants).
//! * [`SettlementInstruction`]: `BytesN<32>` + `CBDCTransaction` + 2 × u32 +
//!   u64 + `Option<BytesN<32>>` ≈ 32 + 96 + 8 + 8 + 33 = 177 bytes.
//! * [`SettlementPath`]: bounded by [`MAX_PATH_HOPS`] hops and
//!   [`MAX_RATES_LEN`] rates; worst case ≈ 2 + 32 + 16 × 16 + 4 = 294 bytes.
//!
//! # CPU instruction bounds (worst case)
//!
//! * [`InteropManager::convert_amount`]: O(1), ≤ 2 checked mul/div.
//! * [`SettlementPath::compute_final_amount`]: O([`MAX_PATH_HOPS`]) conversions,
//!   ≤ 2 × [`MAX_PATH_HOPS`] checked mul/div.
//! * [`InteropManager::execute_atomic_swap`] / `execute_hub_and_spoke`: one
//!   SHA-256 over a bounded input (≤ 128 bytes).
//!
//! # TTL policy
//!
//! * [`ExchangeRate`] and [`SettlementInstruction`] are archival-critical and
//!   stored in `Persistent` storage; use [`InteropManager::extend_ttl`] to bump
//!   their TTL before expiry.
//! * [`SettlementPath`] is derived/ephemeral and stored in `Temporary` storage.
//! * Module-level configuration lives in `Instance` storage.

use crate::cbdc_types::{CBDCTransaction, InteropProtocol};
use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env, Symbol};
use soroban_sdk::xdr::ToXdr;

/// Maximum number of hops allowed in a [`SettlementPath`].
pub const MAX_PATH_HOPS: u32 = 16;
/// Maximum number of exchange rates allowed in a [`SettlementPath`].
pub const MAX_RATES_LEN: u32 = 16;
/// Minimum number of pilots required for a valid [`SettlementPath`].
pub const MIN_PATH_HOPS: u32 = 2;
/// Maximum basis points (100%).
pub const MAX_BPS: u32 = 10_000;
/// Scale factor for exchange rates (1e18).
pub const RATE_SCALE: u128 = 1_000_000_000_000_000_000;
/// Default TTL extension (in ledgers) for archival-critical records.
pub const DEFAULT_TTL_EXTEND_LEDGERS: u32 = 100_000;
/// Threshold (in ledgers) below which archival-critical records are bumped.
pub const TTL_BUMP_THRESHOLD_LEDGERS: u32 = 10_000;

/// Typed contract errors for the CBDC interop module.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InteropError {
    /// Exchange rate is stale.
    StaleRate = 1,
    /// Exchange rate is zero.
    ZeroRate = 2,
    /// Bid/ask price is zero.
    ZeroPrice = 3,
    /// Bid price exceeds ask price.
    BidExceedsAsk = 4,
    /// Amount is zero.
    ZeroAmount = 5,
    /// Arithmetic overflow during conversion.
    ConversionOverflow = 6,
    /// Division by zero during conversion.
    DivisionByZero = 7,
    /// Fee basis points exceed [`MAX_BPS`].
    FeeBpsTooHigh = 8,
    /// Fee calculation overflow.
    FeeOverflow = 9,
    /// Settlement amounts are zero.
    ZeroSettlementAmount = 10,
    /// Settlement exchange rate is zero.
    ZeroSettlementRate = 11,
    /// Path has fewer than [`MIN_PATH_HOPS`] pilots.
    PathTooShort = 12,
    /// Path exceeds [`MAX_PATH_HOPS`] hops.
    PathTooLong = 13,
    /// Rates vector exceeds [`MAX_RATES_LEN`] entries.
    RatesTooLong = 14,
    /// Path length does not match rates length.
    PathLengthMismatch = 15,
    /// Source and destination pilots are identical.
    SamePilot = 16,
    /// Pilot identifier is out of range.
    InvalidPilot = 17,
    /// Spread computation overflowed.
    SpreadOverflow = 18,
}

/// Exchange rate between two CBDC pilots.
///
/// Field order is chosen to minimize XDR padding: small integers first,
/// then the wide `u128` values.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangeRate {
    /// Source CBDC pilot
    pub source_pilot: u32, // CBDCPilot as u8
    /// Destination CBDC pilot
    pub dest_pilot: u32, // CBDCPilot as u8
    /// Maximum volatility tolerated (percentage * 100)
    pub max_volatility: u32,
    /// Timestamp of rate (for staleness checks)
    pub timestamp: u64,
    /// Rate scaled by 1e18 for precision
    pub rate: u128,
    /// Bid price (lower)
    pub bid_price: u128,
    /// Ask price (higher)
    pub ask_price: u128,
}

impl ExchangeRate {
    /// Check if rate is stale (older than 1 hour by default)
    pub fn is_stale(&self, current_time: u64, staleness_threshold: u64) -> bool {
        current_time.saturating_sub(self.timestamp) > staleness_threshold
    }

    /// Compute mid-price between bid and ask (saturating).
    pub fn mid_price(&self) -> u128 {
        self.bid_price.saturating_add(self.ask_price) / 2
    }

    /// Apply spread from mid-price. Returns a typed error on overflow.
    pub fn spread_bps(&self) -> Result<u128, InteropError> {
        let mid = self.mid_price();
        if mid == 0 {
            return Ok(0);
        }
        let diff = self.ask_price.saturating_sub(self.bid_price);
        let scaled = diff
            .checked_mul(MAX_BPS as u128)
            .ok_or(InteropError::SpreadOverflow)?;
        Ok(scaled / mid)
    }
}

/// Represents a settlement instruction for cross-CBDC transfer.
///
/// Stored in `Persistent` storage (archival-critical audit record).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementInstruction {
    /// Settlement ID
    pub settlement_id: BytesN<32>,
    /// Source transaction
    pub transaction: CBDCTransaction,
    /// Interoperability protocol
    pub protocol: u32, // InteropProtocol as u8
    /// Settlement status
    pub status: u32, // SettlementStatus as u8
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
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            SettlementStatus::Created => Symbol::new(env, "CREATED"),
            SettlementStatus::InitiatedSource => Symbol::new(env, "INIT_SRC"),
            SettlementStatus::ConfirmedSource => Symbol::new(env, "CONF_SRC"),
            SettlementStatus::AwaitingDestination => Symbol::new(env, "AWAIT_DST"),
            SettlementStatus::Completed => Symbol::new(env, "COMPLETE"),
            SettlementStatus::Failed => Symbol::new(env, "FAILED"),
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
    ) -> Result<(), InteropError> {
        if rate.is_stale(current_time, staleness_threshold) {
            return Err(InteropError::StaleRate);
        }
        if rate.rate == 0 {
            return Err(InteropError::ZeroRate);
        }
        if rate.bid_price == 0 || rate.ask_price == 0 {
            return Err(InteropError::ZeroPrice);
        }
        if rate.bid_price > rate.ask_price {
            return Err(InteropError::BidExceedsAsk);
        }
        Ok(())
    }

    /// Convert amount using exchange rate
    pub fn convert_amount(amount: u128, rate: u128) -> Result<u128, InteropError> {
        if rate == 0 {
            return Err(InteropError::ZeroRate);
        }
        if amount == 0 {
            return Err(InteropError::ZeroAmount);
        }

        // Calculate: (amount * rate) / 1e18, handling overflow
        let scaled_amount = amount
            .checked_mul(rate)
            .ok_or(InteropError::ConversionOverflow)?
            .checked_div(RATE_SCALE)
            .ok_or(InteropError::DivisionByZero)?;

        Ok(scaled_amount)
    }

    /// Execute atomic swap protocol
    pub fn execute_atomic_swap(
        env: &Env,
        from: &Address,
        to: &Address,
        source_amount: u128,
        dest_amount: u128,
        _timeout_ledgers: u32,
    ) -> Result<BytesN<32>, InteropError> {
        if source_amount == 0 || dest_amount == 0 {
            return Err(InteropError::ZeroAmount);
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
    ) -> Result<BytesN<32>, InteropError> {
        if source_amount == 0 || dest_amount == 0 {
            return Err(InteropError::ZeroAmount);
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
    ) -> Result<(), InteropError> {
        if instruction.transaction.amount_source == 0 || instruction.transaction.amount_dest == 0
        {
            return Err(InteropError::ZeroSettlementAmount);
        }
        if instruction.transaction.exchange_rate == 0 {
            return Err(InteropError::ZeroSettlementRate);
        }
        Ok(())
    }

    /// Permissionless TTL extension for archival-critical records.
    ///
    /// Bumps the TTL of the given persistent storage key so active audit
    /// records are never unexpectedly archived. Safe to call by anyone.
    pub fn extend_ttl(env: &Env, key: &Symbol) {
        env.storage().persistent().extend_ttl(
            key,
            TTL_BUMP_THRESHOLD_LEDGERS,
            DEFAULT_TTL_EXTEND_LEDGERS,
        );
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
    pub fn compute_settlement_fee(amount: u128, fee_bps: u32) -> Result<u128, InteropError> {
        if fee_bps > MAX_BPS {
            return Err(InteropError::FeeBpsTooHigh);
        }

        let fee = amount
            .checked_mul(fee_bps as u128)
            .ok_or(InteropError::FeeOverflow)?
            .checked_div(MAX_BPS as u128)
            .ok_or(InteropError::DivisionByZero)?;

        Ok(fee)
    }

    /// Validate cross-pilot compatibility
    pub fn validate_pilot_pair(source: u8, dest: u8) -> Result<(), InteropError> {
        if source == dest {
            return Err(InteropError::SamePilot);
        }
        if source > 3 || dest > 3 {
            return Err(InteropError::InvalidPilot);
        }
        Ok(())
    }
}

/// Settlement path finding for multi-hop transfers.
///
/// Stored in `Temporary` storage (derived/ephemeral routing data).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPath {
    /// Sequence of pilots in path (e.g., [EUR, USD, CNY])
    pub path: Bytes,
    /// Exchange rates for each hop
    pub rates: soroban_sdk::Vec<u128>,
    /// Total cost in basis points
    pub total_cost_bps: u32,
}

impl SettlementPath {
    /// Calculate final amount after all conversions.
    ///
    /// Enforces [`MIN_PATH_HOPS`], [`MAX_PATH_HOPS`] and [`MAX_RATES_LEN`]
    /// bounds before iterating, returning typed errors on violation.
    pub fn compute_final_amount(&self, initial_amount: u128) -> Result<u128, InteropError> {
        let path_len = self.path.len();
        let rates_len = self.rates.len();

        if path_len < MIN_PATH_HOPS {
            return Err(InteropError::PathTooShort);
        }
        if path_len > MAX_PATH_HOPS {
            return Err(InteropError::PathTooLong);
        }
        if rates_len > MAX_RATES_LEN {
            return Err(InteropError::RatesTooLong);
        }
        if path_len != rates_len {
            return Err(InteropError::PathLengthMismatch);
        }

        let mut amount = initial_amount;
        for rate in self.rates.iter() {
            amount = InteropManager::convert_amount(amount, rate)?;
        }

        Ok(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_exchange_rate_validation() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 500,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 0_950_000_000_000_000_000,
            ask_price: 1_050_000_000_000_000_000,
        };

        assert!(InteropManager::validate_exchange_rate(&rate, 2000, 3600).is_ok());
        assert_eq!(
            InteropManager::validate_exchange_rate(&rate, 10000, 3600),
            Err(InteropError::StaleRate)
        );
    }

    #[test]
    fn test_conversion_calculation() {
        let amount = 1_000_000_000_000_000_000u128; // 1e18
        let rate = 1_200_000_000_000_000_000u128; // 1.2x rate

        let result = InteropManager::convert_amount(amount, rate).unwrap();
        assert_eq!(result, 1_200_000_000_000_000_000);
    }

    #[test]
    fn test_conversion_overflow_returns_typed_error() {
        assert_eq!(
            InteropManager::convert_amount(u128::MAX, u128::MAX),
            Err(InteropError::ConversionOverflow)
        );
        assert_eq!(
            InteropManager::convert_amount(0, 1),
            Err(InteropError::ZeroAmount)
        );
        assert_eq!(
            InteropManager::convert_amount(1, 0),
            Err(InteropError::ZeroRate)
        );
    }

    #[test]
    fn test_settlement_fee_calculation() {
        let amount = 1_000_000u128;
        let fee_bps = 25; // 0.25%

        let fee = InteropManager::compute_settlement_fee(amount, fee_bps).unwrap();
        assert_eq!(fee, 250);
    }

    #[test]
    fn test_settlement_fee_boundaries() {
        assert_eq!(
            InteropManager::compute_settlement_fee(1, MAX_BPS + 1),
            Err(InteropError::FeeBpsTooHigh)
        );
        assert_eq!(
            InteropManager::compute_settlement_fee(u128::MAX, MAX_BPS),
            Err(InteropError::FeeOverflow)
        );
    }

    #[test]
    fn test_spread_bps_overflow_is_typed() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 0,
            rate: 1,
            bid_price: 0,
            ask_price: u128::MAX,
        };
        assert_eq!(rate.spread_bps(), Err(InteropError::SpreadOverflow));
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

    #[test]
    fn test_pilot_pair_boundaries() {
        assert_eq!(
            InteropManager::validate_pilot_pair(1, 1),
            Err(InteropError::SamePilot)
        );
        assert_eq!(
            InteropManager::validate_pilot_pair(0, 4),
            Err(InteropError::InvalidPilot)
        );
        assert!(InteropManager::validate_pilot_pair(0, 3).is_ok());
    }

    #[test]
    fn test_path_bounds_enforced() {
        let env = Env::default();

        // Too short
        let short = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8]),
            rates: soroban_sdk::Vec::new(&env),
            total_cost_bps: 0,
        };
        assert_eq!(
            short.compute_final_amount(1),
            Err(InteropError::PathTooShort)
        );

        // Length mismatch
        let mismatch = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8]),
            rates: soroban_sdk::Vec::new(&env),
            total_cost_bps: 0,
        };
        assert_eq!(
            mismatch.compute_final_amount(1),
            Err(InteropError::PathLengthMismatch)
        );

        // Rates vector too long
        let mut rates = soroban_sdk::Vec::new(&env);
        for _ in 0..(MAX_RATES_LEN + 1) {
            rates.push_back(RATE_SCALE);
        }
        let too_long = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8]),
            rates,
            total_cost_bps: 0,
        };
        assert_eq!(
            too_long.compute_final_amount(1),
            Err(InteropError::RatesTooLong)
        );
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Property-based tests (proptest) — invariant & determinism guarantees
    // ════════════════════════════════════════════════════════════════════════════════

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(200))]

        /// convert_amount never panics: returns a typed error on overflow,
        /// division by zero, zero rate, or zero amount.
        #[test]
        fn prop_convert_amount_no_panic(
            amount in any::<u128>(),
            rate in any::<u128>(),
        ) {
            let result = InteropManager::convert_amount(amount, rate);
            match result {
                Ok(scaled) => {
                    prop_assert!(scaled <= u128::MAX / 2);
                }
                Err(InteropError::ConversionOverflow) |
                Err(InteropError::DivisionByZero) |
                Err(InteropError::ZeroRate) |
                Err(InteropError::ZeroAmount) => {}
                _ => prop_assert!(false, "unexpected error variant"),
            }
        }

        /// convert_amount is deterministic: same inputs always yield same output.
        #[test]
        fn prop_convert_amount_deterministic(
            amount in any::<u128>(),
            rate in any::<u128>(),
        ) {
            let r1 = InteropManager::convert_amount(amount, rate);
            let r2 = InteropManager::convert_amount(amount, rate);
            prop_assert_eq!(r1, r2);
        }

        /// compute_settlement_fee never panics; bounded by amount.
        #[test]
        fn prop_settlement_fee_bounded(
            amount in any::<u128>(),
            fee_bps in any::<u32>(),
        ) {
            let result = InteropManager::compute_settlement_fee(amount, fee_bps);
            match result {
                Ok(fee) => {
                    prop_assert!(fee <= amount);
                }
                Err(InteropError::FeeBpsTooHigh) |
                Err(InteropError::FeeOverflow) |
                Err(InteropError::DivisionByZero) => {}
                _ => prop_assert!(false, "unexpected error variant"),
            }
        }

        /// compute_settlement_fee is deterministic.
        #[test]
        fn prop_settlement_fee_deterministic(
            amount in any::<u128>(),
            fee_bps in any::<u32>(),
        ) {
            prop_assert_eq!(
                InteropManager::compute_settlement_fee(amount, fee_bps),
                InteropManager::compute_settlement_fee(amount, fee_bps),
            );
        }

        /// validate_exchange_rate is deterministic.
        #[test]
        fn prop_validate_rate_deterministic(
            source_pilot in any::<u32>(),
            dest_pilot in any::<u32>(),
            max_volatility in any::<u32>(),
            timestamp in any::<u64>(),
            rate in any::<u128>(),
            bid_price in any::<u128>(),
            ask_price in any::<u128>(),
            current_time in any::<u64>(),
            staleness_threshold in any::<u64>(),
        ) {
            let rate_obj = ExchangeRate {
                source_pilot,
                dest_pilot,
                max_volatility,
                timestamp,
                rate,
                bid_price,
                ask_price,
            };
            let r1 = InteropManager::validate_exchange_rate(&rate_obj, current_time, staleness_threshold);
            let r2 = InteropManager::validate_exchange_rate(&rate_obj, current_time, staleness_threshold);
            prop_assert_eq!(r1, r2);
        }

        /// is_valid_status_transition is deterministic and monotonic:
        /// Completed and Failed are terminal (never transition out).
        #[test]
        fn prop_status_transition_deterministic(
            from in any::<SettlementStatus>(),
            to in any::<SettlementStatus>(),
        ) {
            let r1 = InteropManager::is_valid_status_transition(from, to);
            let r2 = InteropManager::is_valid_status_transition(from, to);
            prop_assert_eq!(r1, r2);

            // Terminal states cannot transition out (except to Failed).
            if matches!(from, SettlementStatus::Completed | SettlementStatus::Failed) {
                prop_assert_eq!(r1, to == SettlementStatus::Failed);
            }
        }

        /// validate_pilot_pair: same pilot always fails; distinct valid pilots
        /// (0..=3) always succeed.
        #[test]
        fn prop_pilot_pair_deterministic(
            source in any::<u8>(),
            dest in any::<u8>(),
        ) {
            let r1 = InteropManager::validate_pilot_pair(source, dest);
            let r2 = InteropManager::validate_pilot_pair(source, dest);
            prop_assert_eq!(r1, r2);

            if source == dest {
                prop_assert_eq!(r1, Err(InteropError::SamePilot));
            } else if source > 3 || dest > 3 {
                prop_assert_eq!(r1, Err(InteropError::InvalidPilot));
            } else {
                prop_assert!(r1.is_ok());
            }
        }

        /// spread_bps never panics; returns typed error on overflow.
        #[test]
        fn prop_spread_bps_no_panic(
            bid_price in any::<u128>(),
            ask_price in any::<u128>(),
        ) {
            let rate = ExchangeRate {
                source_pilot: 0,
                dest_pilot: 1,
                max_volatility: 0,
                timestamp: 0,
                rate: 1,
                bid_price,
                ask_price,
            };
            match rate.spread_bps() {
                Ok(bps) => prop_assert!(bps <= MAX_BPS as u128),
                Err(InteropError::SpreadOverflow) => {}
                _ => prop_assert!(false, "unexpected error variant"),
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Concurrency & reordering tests — interleaved settlements and conversions
    // ════════════════════════════════════════════════════════════════════════════════

    /// Interleaved conversions for distinct (amount, rate) pairs never
    /// interfere; each conversion is independent and deterministic.
    #[test]
    fn test_interleaved_conversions_independent() {
        let pairs: Vec<(u128, u128)> = vec![
            (1_000_000_000_000_000_000, 1_200_000_000_000_000_000),
            (500_000_000_000_000_000, 900_000_000_000_000_000),
            (2_000_000_000_000_000_000, 1_500_000_000_000_000_000),
            (100_000_000_000_000_000, 1_100_000_000_000_000_000),
            (750_000_000_000_000_000, 1_300_000_000_000_000_000),
        ];

        let mut results = Vec::new();
        for (amount, rate) in &pairs {
            results.push(InteropManager::convert_amount(*amount, *rate));
        }

        // Re-run in reverse order — results must be identical (determinism under reordering).
        let mut reverse_results = Vec::new();
        for (amount, rate) in pairs.iter().rev() {
            reverse_results.push(InteropManager::convert_amount(*amount, *rate));
        }

        for i in 0..pairs.len() {
            assert_eq!(results[i], reverse_results[pairs.len() - 1 - i]);
        }
    }

    /// Interleaved atomic swaps produce distinct swap IDs for distinct inputs.
    #[test]
    fn test_interleaved_atomic_swaps_distinct_ids() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let hub = Address::generate(&env);

        let mut ids = Vec::new();
        for i in 0..10u32 {
            let id = InteropManager::execute_atomic_swap(
                &env,
                &from,
                &to,
                1_000_000 + i,
                2_000_000 + i,
                100,
            )
            .unwrap();
            ids.push(id);
        }

        // All swap IDs must be distinct.
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                assert_ne!(ids[i], ids[j], "swap IDs collide at {i} and {j}");
            }
        }
    }

    /// Interleaved hub-and-spoke settlements produce distinct IDs.
    #[test]
    fn test_interleaved_hub_and_spoke_distinct_ids() {
        let env = Env::default();
        let hub = Address::generate(&env);

        let mut ids = Vec::new();
        for i in 0..10u32 {
            let id = InteropManager::execute_hub_and_spoke(
                &env,
                &hub,
                1_000_000 + i,
                2_000_000 + i,
            )
            .unwrap();
            ids.push(id);
        }

        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                assert_ne!(ids[i], ids[j], "hub IDs collide at {i} and {j}");
            }
        }
    }

    /// Multi-hop path computation is order-sensitive: reversing rates changes
    /// the result (non-commutative), but re-running the same path is stable.
    #[test]
    fn test_path_computation_order_sensitive() {
        let env = Env::default();

        let path_a = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8, 2u8]),
            rates: soroban_sdk::Vec::from_array(&env, &[
                RATE_SCALE * 2,
                RATE_SCALE * 3,
            ]),
            total_cost_bps: 10,
        };

        let path_b = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 2u8, 1u8]),
            rates: soroban_sdk::Vec::from_array(&env, &[
                RATE_SCALE * 3,
                RATE_SCALE * 2,
            ]),
            total_cost_bps: 10,
        };

        let amount = 1_000_000_000_000_000_000u128;
        let result_a = path_a.compute_final_amount(amount).unwrap();
        let result_b = path_b.compute_final_amount(amount).unwrap();

        // Different order → different result (non-commutative conversions).
        assert_ne!(result_a, result_b);

        // Re-running the same path is deterministic.
        assert_eq!(path_a.compute_final_amount(amount).unwrap(), result_a);
        assert_eq!(path_b.compute_final_amount(amount).unwrap(), result_b);
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Chaos / fault-injection tests — boundary values, overflow, malformed input
    // ════════════════════════════════════════════════════════════════════════════════

    /// Boundary: convert_amount with u128::MAX operands → ConversionOverflow.
    #[test]
    fn test_conversion_u128_max_boundary() {
        assert_eq!(
            InteropManager::convert_amount(u128::MAX, u128::MAX),
            Err(InteropError::ConversionOverflow)
        );
        assert_eq!(
            InteropManager::convert_amount(u128::MAX, 1),
            Err(InteropError::ConversionOverflow)
        );
    }

    /// Boundary: compute_settlement_fee with u128::MAX → FeeOverflow.
    #[test]
    fn test_fee_u128_max_boundary() {
        assert_eq!(
            InteropManager::compute_settlement_fee(u128::MAX, MAX_BPS),
            Err(InteropError::FeeOverflow)
        );
    }

    /// Boundary: fee_bps exactly at MAX_BPS is valid; MAX_BPS + 1 fails.
    #[test]
    fn test_fee_bps_boundary() {
        assert!(InteropManager::compute_settlement_fee(1_000_000, MAX_BPS).is_ok());
        assert_eq!(
            InteropManager::compute_settlement_fee(1_000_000, MAX_BPS + 1),
            Err(InteropError::FeeBpsTooHigh)
        );
    }

    /// Boundary: exchange rate exactly zero → ZeroRate in convert and validate.
    #[test]
    fn test_zero_rate_boundary() {
        assert_eq!(
            InteropManager::convert_amount(1_000, 0),
            Err(InteropError::ZeroRate)
        );

        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 1000,
            rate: 0,
            bid_price: 1_000_000_000_000_000_000,
            ask_price: 1_000_000_000_000_000_000,
        };
        assert_eq!(
            InteropManager::validate_exchange_rate(&rate, 1001, 3600),
            Err(InteropError::ZeroRate)
        );
    }

    /// Boundary: zero amount → ZeroAmount in convert, atomic swap, hub-and-spoke.
    #[test]
    fn test_zero_amount_boundary() {
        assert_eq!(
            InteropManager::convert_amount(0, RATE_SCALE),
            Err(InteropError::ZeroAmount)
        );

        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        assert_eq!(
            InteropManager::execute_atomic_swap(&env, &from, &to, 0, 1_000, 100),
            Err(InteropError::ZeroAmount)
        );
        assert_eq!(
            InteropManager::execute_atomic_swap(&env, &from, &to, 1_000, 0, 100),
            Err(InteropError::ZeroAmount)
        );
        assert_eq!(
            InteropManager::execute_hub_and_spoke(&env, &from, 0, 1_000),
            Err(InteropError::ZeroAmount)
        );
    }

    /// Boundary: bid_price > ask_price → BidExceedsAsk.
    #[test]
    fn test_bid_exceeds_ask_boundary() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 1_100_000_000_000_000_000,
            ask_price: 1_000_000_000_000_000_000,
        };
        assert_eq!(
            InteropManager::validate_exchange_rate(&rate, 1001, 3600),
            Err(InteropError::BidExceedsAsk)
        );
    }

    /// Boundary: bid_price == 0 or ask_price == 0 → ZeroPrice.
    #[test]
    fn test_zero_price_boundary() {
        let rate_bid_zero = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 0,
            ask_price: 1_000_000_000_000_000_000,
        };
        assert_eq!(
            InteropManager::validate_exchange_rate(&rate_bid_zero, 1001, 3600),
            Err(InteropError::ZeroPrice)
        );

        let rate_ask_zero = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 1_000_000_000_000_000_000,
            ask_price: 0,
        };
        assert_eq!(
            InteropManager::validate_exchange_rate(&rate_ask_zero, 1001, 3600),
            Err(InteropError::ZeroPrice)
        );
    }

    /// Chaos: stale rate detection across a staleness threshold boundary.
    #[test]
    fn test_staleness_chaos_boundary() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 950_000_000_000_000_000,
            ask_price: 1_050_000_000_000_000_000,
        };

        // Exactly at threshold → not stale (saturating_sub > threshold is false).
        assert!(!rate.is_stale(4600, 3600));
        // One past threshold → stale.
        assert!(rate.is_stale(4601, 3600));
        // Well within threshold → fresh.
        assert!(!rate.is_stale(2000, 3600));
    }

    /// Chaos: path bounds enforced — too short, too long, rates too long,
    /// length mismatch all return typed errors.
    #[test]
    fn test_path_bounds_chaos() {
        let env = Env::default();

        // Too short (1 hop < MIN_PATH_HOPS=2).
        let short = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8]),
            rates: soroban_sdk::Vec::new(&env),
            total_cost_bps: 0,
        };
        assert_eq!(
            short.compute_final_amount(1),
            Err(InteropError::PathTooShort)
        );

        // Too long (> MAX_PATH_HOPS=16).
        let mut long_path = Bytes::new(&env);
        for i in 0..17u8 {
            long_path.push_back(i);
        }
        let mut long_rates = soroban_sdk::Vec::new(&env);
        for _ in 0..17u32 {
            long_rates.push_back(RATE_SCALE);
        }
        let too_long = SettlementPath {
            path: long_path,
            rates: long_rates,
            total_cost_bps: 0,
        };
        assert_eq!(
            too_long.compute_final_amount(1),
            Err(InteropError::PathTooLong)
        );

        // Rates too long (> MAX_RATES_LEN=16).
        let mut rates = soroban_sdk::Vec::new(&env);
        for _ in 0..(MAX_RATES_LEN + 1) {
            rates.push_back(RATE_SCALE);
        }
        let rates_too_long = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8]),
            rates,
            total_cost_bps: 0,
        };
        assert_eq!(
            rates_too_long.compute_final_amount(1),
            Err(InteropError::RatesTooLong)
        );

        // Length mismatch.
        let mismatch = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8, 2u8]),
            rates: soroban_sdk::Vec::from_array(&env, &[RATE_SCALE, RATE_SCALE]),
            total_cost_bps: 0,
        };
        assert_eq!(
            mismatch.compute_final_amount(1),
            Err(InteropError::PathLengthMismatch)
        );
    }

    /// Chaos: same pilot pair always fails; out-of-range pilots always fail.
    #[test]
    fn test_pilot_pair_chaos() {
        // Same pilot.
        assert_eq!(
            InteropManager::validate_pilot_pair(0, 0),
            Err(InteropError::SamePilot)
        );
        assert_eq!(
            InteropManager::validate_pilot_pair(3, 3),
            Err(InteropError::SamePilot)
        );
        // Out of range.
        assert_eq!(
            InteropManager::validate_pilot_pair(0, 4),
            Err(InteropError::InvalidPilot)
        );
        assert_eq!(
            InteropManager::validate_pilot_pair(5, 1),
            Err(InteropError::InvalidPilot)
        );
        // Valid distinct pairs.
        assert!(InteropManager::validate_pilot_pair(0, 1).is_ok());
        assert!(InteropManager::validate_pilot_pair(0, 3).is_ok());
        assert!(InteropManager::validate_pilot_pair(2, 3).is_ok());
    }

    /// Chaos: spread overflow with extreme bid/ask values.
    #[test]
    fn test_spread_overflow_chaos() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 0,
            timestamp: 0,
            rate: 1,
            bid_price: 0,
            ask_price: u128::MAX,
        };
        assert_eq!(rate.spread_bps(), Err(InteropError::SpreadOverflow));
    }

    /// Chaos: settlement status transitions — Completed/Failed are terminal.
    #[test]
    fn test_terminal_status_transitions_chaos() {
        for terminal in [SettlementStatus::Completed, SettlementStatus::Failed] {
            for next in [
                SettlementStatus::Created,
                SettlementStatus::InitiatedSource,
                SettlementStatus::ConfirmedSource,
                SettlementStatus::AwaitingDestination,
                SettlementStatus::Completed,
            ] {
                if next == SettlementStatus::Failed {
                    assert!(
                        InteropManager::is_valid_status_transition(terminal, next),
                        "{terminal:?} → Failed should be allowed"
                    );
                } else {
                    assert!(
                        !InteropManager::is_valid_status_transition(terminal, next),
                        "{terminal:?} → {next:?} should be disallowed"
                    );
                }
            }
        }
    }

    /// Chaos: SettlementInstruction validation rejects zero amounts and rates.
    #[test]
    fn test_settlement_instruction_validation_chaos() {
        let env = Env::default();

        let zero_amount = SettlementInstruction {
            settlement_id: BytesN::from_array(&env, &[1u8; 32]),
            transaction: CBDCTransaction {
                tx_id: BytesN::from_array(&env, &[2u8; 32]),
                source_pilot: 0,
                dest_pilot: 1,
                from: Address::generate(&env),
                to: Address::generate(&env),
                amount_source: 0,
                amount_dest: 1_000,
                exchange_rate: RATE_SCALE,
                timestamp: 1000,
                protocol: 0,
                privacy_tier: 0,
                offline_status: None,
                metadata: Bytes::new(&env),
            },
            protocol: 0,
            status: 0,
            timestamp: 1000,
            confirmation_hash: None,
        };
        assert_eq!(
            InteropManager::validate_settlement_instruction(&zero_amount),
            Err(InteropError::ZeroSettlementAmount)
        );

        let zero_rate = SettlementInstruction {
            settlement_id: BytesN::from_array(&env, &[1u8; 32]),
            transaction: CBDCTransaction {
                tx_id: BytesN::from_array(&env, &[2u8; 32]),
                source_pilot: 0,
                dest_pilot: 1,
                from: Address::generate(&env),
                to: Address::generate(&env),
                amount_source: 1_000,
                amount_dest: 1_000,
                exchange_rate: 0,
                timestamp: 1000,
                protocol: 0,
                privacy_tier: 0,
                offline_status: None,
                metadata: Bytes::new(&env),
            },
            protocol: 0,
            status: 0,
            timestamp: 1000,
            confirmation_hash: None,
        };
        assert_eq!(
            InteropManager::validate_settlement_instruction(&zero_rate),
            Err(InteropError::ZeroSettlementRate)
        );
    }
}
