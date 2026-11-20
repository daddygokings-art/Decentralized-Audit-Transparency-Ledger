//! CBDC cross-ledger settlement and ISO-20022 message bridge.
//!
//! # Validator attestation quorum (race-free settlement finalization)
//!
//! A settlement in [`SettlementStatus::AwaitingDestination`] is finalized by
//! a configured validator set attesting the destination-ledger confirmation
//! hash via [`InteropManager::attest_settlement`]. Several validators may
//! submit their attestations in the same ledger and in any order, so the
//! attestation path enforces the following invariants:
//!
//! * **INV-ATT-1 (one vote per validator)** – a validator is counted at most
//!   once per settlement. A replayed attestation returns
//!   [`InteropError::DuplicateAttestation`] and a conflicting one returns
//!   [`InteropError::Equivocation`]; neither mutates state.
//! * **INV-ATT-2 (no conflicting finality)** – the quorum must be a strict
//!   majority of the validator set (`2 * quorum > validators`), so two
//!   different confirmation hashes can never both reach quorum.
//! * **INV-ATT-3 (exactly-once finalization)** – the transition to
//!   [`SettlementStatus::Completed`] (or [`SettlementStatus::Failed`] when no
//!   hash can still reach quorum) happens exactly once and emits exactly one
//!   finalization event. Terminal statuses are absorbing: a late attestation
//!   or failure report can never flip a finalized settlement.
//! * **INV-ATT-4 (order independence)** – attestations are stored in a
//!   canonical order (sorted by validator, tallies sorted by hash), and the
//!   attestation digest is computed over that canonical form. Any
//!   interleaving of the same set of attestations yields a byte-identical
//!   [`AttestationState`], settlement record and digest.
//!
//! Each attestation is authenticated with `validator.require_auth()`.
//! Configuring the validator set is an administrative operation; the
//! embedding contract is responsible for gating
//! [`InteropManager::configure_validators`] behind its governance check.
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
/// Maximum number of validators in the attestation set. Bounds the size of
/// [`AttestationState`] and the cost of a single attestation call.
pub const MAX_VALIDATORS: u32 = 32;

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
    /// Attesting address is not a member of the configured validator set.
    NotAValidator = 19,
    /// The validator already attested this exact confirmation hash.
    DuplicateAttestation = 20,
    /// The validator already attested a *different* confirmation hash.
    Equivocation = 21,
    /// No settlement instruction is stored under the given ID.
    SettlementNotFound = 22,
    /// A settlement instruction already exists under the given ID.
    SettlementAlreadyExists = 23,
    /// The requested settlement status transition is not allowed.
    InvalidStatusTransition = 24,
    /// Quorum must satisfy `validators / 2 < quorum <= validators`.
    InvalidQuorum = 25,
    /// Validator set is empty, exceeds [`MAX_VALIDATORS`], or has duplicates.
    InvalidValidatorSet = 26,
    /// Settlement is not in a status that accepts attestations.
    NotAwaitingAttestation = 27,
    /// The stored status value does not map to a [`SettlementStatus`].
    InvalidStatus = 28,
    /// No validator set has been configured.
    ValidatorsNotConfigured = 29,
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

    /// Decode the `u32` status stored in [`SettlementInstruction::status`].
    pub fn from_u32(value: u32) -> Option<SettlementStatus> {
        match value {
            0 => Some(SettlementStatus::Created),
            1 => Some(SettlementStatus::InitiatedSource),
            2 => Some(SettlementStatus::ConfirmedSource),
            3 => Some(SettlementStatus::AwaitingDestination),
            4 => Some(SettlementStatus::Completed),
            5 => Some(SettlementStatus::Failed),
            _ => None,
        }
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

    /// Determine settlement status transition validity.
    ///
    /// Terminal statuses ([`SettlementStatus::Completed`] and
    /// [`SettlementStatus::Failed`]) are absorbing: no transition out of them
    /// is valid, including `Completed -> Failed` and `Failed -> Failed`. This
    /// prevents a late failure report racing a quorum finalization from
    /// overwriting an already-confirmed cross-ledger settlement.
    pub fn is_valid_status_transition(
        from: SettlementStatus,
        to: SettlementStatus,
    ) -> bool {
        if from.is_terminal() {
            return false;
        }
        matches!(
            (from, to),
            (SettlementStatus::Created, SettlementStatus::InitiatedSource)
                | (SettlementStatus::InitiatedSource, SettlementStatus::ConfirmedSource)
                | (SettlementStatus::ConfirmedSource, SettlementStatus::AwaitingDestination)
                | (SettlementStatus::AwaitingDestination, SettlementStatus::Completed)
                // Any non-terminal state may fail.
                | (_, SettlementStatus::Failed)
        )
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

// ════════════════════════════════════════════════════════════════════════════
// Validator attestation quorum
// ════════════════════════════════════════════════════════════════════════════

/// Storage keys for the stateful settlement / attestation layer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteropDataKey {
    /// `Instance`: canonical (sorted, de-duplicated) validator set.
    Validators,
    /// `Instance`: attestation quorum threshold.
    AttestationQuorum,
    /// `Persistent`: [`SettlementInstruction`] by settlement ID.
    Settlement(BytesN<32>),
    /// `Persistent`: [`AttestationState`] by settlement ID.
    Attestations(BytesN<32>),
}

/// A single validator's vote for a destination-ledger confirmation hash.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatorAttestation {
    /// Attesting validator.
    pub validator: Address,
    /// Destination-ledger confirmation hash the validator vouches for.
    pub confirmation_hash: BytesN<32>,
}

/// Vote count for a single confirmation hash.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationTally {
    /// Confirmation hash being voted on.
    pub confirmation_hash: BytesN<32>,
    /// Number of distinct validators that attested this hash.
    pub votes: u32,
}

/// Attestation state for one settlement.
///
/// The validator set and quorum are snapshotted when the settlement enters
/// [`SettlementStatus::AwaitingDestination`], so reconfiguring validators
/// never races with in-flight settlements. All collections are kept in a
/// canonical order so the state is independent of attestation arrival order.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationState {
    /// Validator set snapshot (sorted).
    pub validators: soroban_sdk::Vec<Address>,
    /// Quorum snapshot.
    pub quorum: u32,
    /// Recorded attestations, sorted by validator address.
    pub attestations: soroban_sdk::Vec<ValidatorAttestation>,
    /// Per-hash vote tallies, sorted by confirmation hash.
    pub tallies: soroban_sdk::Vec<AttestationTally>,
    /// Confirmation hash that reached quorum, if any.
    pub finalized_hash: Option<BytesN<32>>,
    /// SHA-256 digest over the canonical state (see [`InteropManager::attestation_digest`]).
    pub digest: BytesN<32>,
}

/// Result of a successful [`InteropManager::attest_settlement`] call.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AttestationOutcome {
    /// Vote recorded; settlement still awaiting quorum.
    Recorded = 0,
    /// This vote completed the quorum; settlement is now `Completed`.
    Finalized = 1,
    /// No hash can reach quorum any more; settlement is now `Failed`.
    Failed = 2,
    /// Settlement was already terminal; vote recorded for audit only.
    LateRecorded = 3,
}

impl InteropManager {
    /// Configure the validator set and attestation quorum.
    ///
    /// The set must be non-empty, contain at most [`MAX_VALIDATORS`] distinct
    /// addresses, and `quorum` must be a strict majority
    /// (`validators / 2 < quorum <= validators`) so that two conflicting
    /// confirmation hashes can never both reach quorum (INV-ATT-2).
    ///
    /// Settlements already awaiting attestation keep their snapshot.
    pub fn configure_validators(
        env: &Env,
        validators: &soroban_sdk::Vec<Address>,
        quorum: u32,
    ) -> Result<(), InteropError> {
        let n = validators.len();
        if n == 0 || n > MAX_VALIDATORS {
            return Err(InteropError::InvalidValidatorSet);
        }
        let mut sorted: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(env);
        for v in validators.iter() {
            let pos = match sorted.binary_search(&v) {
                Ok(_) => return Err(InteropError::InvalidValidatorSet),
                Err(pos) => pos,
            };
            sorted.insert(pos, v);
        }
        // Strict majority: 2q > n and q <= n. `n <= MAX_VALIDATORS` so the
        // multiplication cannot overflow.
        if quorum > n || quorum.saturating_mul(2) <= n {
            return Err(InteropError::InvalidQuorum);
        }
        env.storage().instance().set(&InteropDataKey::Validators, &sorted);
        env.storage()
            .instance()
            .set(&InteropDataKey::AttestationQuorum, &quorum);
        Ok(())
    }

    /// Currently configured validator set (sorted), if any.
    pub fn get_validators(env: &Env) -> Option<soroban_sdk::Vec<Address>> {
        env.storage().instance().get(&InteropDataKey::Validators)
    }

    /// Currently configured attestation quorum, if any.
    pub fn get_attestation_quorum(env: &Env) -> Option<u32> {
        env.storage().instance().get(&InteropDataKey::AttestationQuorum)
    }

    /// Store a new settlement instruction. It must pass
    /// [`Self::validate_settlement_instruction`], be in
    /// [`SettlementStatus::Created`], and its ID must be unused.
    pub fn register_settlement(
        env: &Env,
        instruction: &SettlementInstruction,
    ) -> Result<(), InteropError> {
        Self::validate_settlement_instruction(instruction)?;
        if instruction.status != SettlementStatus::Created as u32 {
            return Err(InteropError::InvalidStatusTransition);
        }
        let key = InteropDataKey::Settlement(instruction.settlement_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(InteropError::SettlementAlreadyExists);
        }
        Self::store_settlement(env, instruction);
        Ok(())
    }

    /// Read a stored settlement instruction.
    pub fn get_settlement(env: &Env, settlement_id: &BytesN<32>) -> Option<SettlementInstruction> {
        env.storage()
            .persistent()
            .get(&InteropDataKey::Settlement(settlement_id.clone()))
    }

    /// Read the attestation state of a settlement.
    pub fn get_attestation_state(env: &Env, settlement_id: &BytesN<32>) -> Option<AttestationState> {
        env.storage()
            .persistent()
            .get(&InteropDataKey::Attestations(settlement_id.clone()))
    }

    /// Advance a settlement along the status machine.
    ///
    /// Entering [`SettlementStatus::AwaitingDestination`] snapshots the
    /// current validator set and quorum. [`SettlementStatus::Completed`] can
    /// only be reached through [`Self::attest_settlement`] reaching quorum, so
    /// requesting it here returns [`InteropError::InvalidStatusTransition`].
    /// Moving to [`SettlementStatus::Failed`] after the settlement was
    /// finalized is rejected because terminal statuses are absorbing.
    pub fn advance_settlement_status(
        env: &Env,
        settlement_id: &BytesN<32>,
        to: SettlementStatus,
    ) -> Result<(), InteropError> {
        let mut instruction =
            Self::get_settlement(env, settlement_id).ok_or(InteropError::SettlementNotFound)?;
        let from = SettlementStatus::from_u32(instruction.status).ok_or(InteropError::InvalidStatus)?;
        if to == SettlementStatus::Completed || !Self::is_valid_status_transition(from, to) {
            return Err(InteropError::InvalidStatusTransition);
        }

        if to == SettlementStatus::AwaitingDestination {
            let validators = Self::get_validators(env).ok_or(InteropError::ValidatorsNotConfigured)?;
            let quorum =
                Self::get_attestation_quorum(env).ok_or(InteropError::ValidatorsNotConfigured)?;
            let mut state = AttestationState {
                validators,
                quorum,
                attestations: soroban_sdk::Vec::new(env),
                tallies: soroban_sdk::Vec::new(env),
                finalized_hash: None,
                digest: BytesN::from_array(env, &[0u8; 32]),
            };
            state.digest = Self::attestation_digest(env, settlement_id, &state);
            Self::store_attestation_state(env, settlement_id, &state);
        }

        instruction.status = to as u32;
        Self::store_settlement(env, &instruction);
        env.events().publish(
            (Symbol::new(env, "cbdc_settle"), to.as_symbol(env)),
            settlement_id.clone(),
        );
        Ok(())
    }

    /// Record a validator attestation for a settlement awaiting destination
    /// confirmation, finalizing it exactly once when quorum is reached.
    ///
    /// Concurrency guarantees (see module docs, INV-ATT-1..4):
    /// * the validator must authorize the call and belong to the snapshot set;
    /// * replays return [`InteropError::DuplicateAttestation`] and conflicting
    ///   votes return [`InteropError::Equivocation`], leaving state untouched;
    /// * the `Completed` / `Failed` transition and its `final` event happen
    ///   exactly once; attestations arriving after that are recorded with
    ///   [`AttestationOutcome::LateRecorded`] and never change the status.
    pub fn attest_settlement(
        env: &Env,
        validator: &Address,
        settlement_id: &BytesN<32>,
        confirmation_hash: &BytesN<32>,
    ) -> Result<AttestationOutcome, InteropError> {
        validator.require_auth();

        let mut instruction =
            Self::get_settlement(env, settlement_id).ok_or(InteropError::SettlementNotFound)?;
        let status = SettlementStatus::from_u32(instruction.status).ok_or(InteropError::InvalidStatus)?;
        if !matches!(
            status,
            SettlementStatus::AwaitingDestination | SettlementStatus::Completed | SettlementStatus::Failed
        ) {
            return Err(InteropError::NotAwaitingAttestation);
        }
        // A terminal settlement that never entered AwaitingDestination has no
        // attestation snapshot and therefore accepts no votes.
        let mut state = Self::get_attestation_state(env, settlement_id)
            .ok_or(InteropError::NotAwaitingAttestation)?;

        if state.validators.binary_search(validator).is_err() {
            return Err(InteropError::NotAValidator);
        }

        // INV-ATT-1: one vote per validator, kept in canonical order.
        let vote = ValidatorAttestation {
            validator: validator.clone(),
            confirmation_hash: confirmation_hash.clone(),
        };
        let mut insert_at = state.attestations.len();
        for (i, existing) in state.attestations.iter().enumerate() {
            if existing.validator == *validator {
                return Err(if existing.confirmation_hash == *confirmation_hash {
                    InteropError::DuplicateAttestation
                } else {
                    InteropError::Equivocation
                });
            }
            if existing.validator > *validator {
                insert_at = i as u32;
                break;
            }
        }
        state.attestations.insert(insert_at, vote);

        // Tally update, kept sorted by confirmation hash.
        let mut votes = 1u32;
        let mut tally_at = state.tallies.len();
        let mut found = false;
        for (i, tally) in state.tallies.iter().enumerate() {
            if tally.confirmation_hash == *confirmation_hash {
                votes = tally.votes.checked_add(1).ok_or(InteropError::ConversionOverflow)?;
                state.tallies.set(
                    i as u32,
                    AttestationTally {
                        confirmation_hash: confirmation_hash.clone(),
                        votes,
                    },
                );
                found = true;
                break;
            }
            if tally.confirmation_hash > *confirmation_hash {
                tally_at = i as u32;
                break;
            }
        }
        if !found {
            state.tallies.insert(
                tally_at,
                AttestationTally {
                    confirmation_hash: confirmation_hash.clone(),
                    votes,
                },
            );
        }

        // INV-ATT-3: decide the transition at most once.
        let outcome = if status.is_terminal() {
            AttestationOutcome::LateRecorded
        } else if votes >= state.quorum {
            instruction.status = SettlementStatus::Completed as u32;
            instruction.confirmation_hash = Some(confirmation_hash.clone());
            state.finalized_hash = Some(confirmation_hash.clone());
            AttestationOutcome::Finalized
        } else {
            let remaining = state.validators.len().saturating_sub(state.attestations.len());
            let mut max_votes = 0u32;
            for tally in state.tallies.iter() {
                max_votes = max_votes.max(tally.votes);
            }
            if max_votes.saturating_add(remaining) < state.quorum {
                instruction.status = SettlementStatus::Failed as u32;
                AttestationOutcome::Failed
            } else {
                AttestationOutcome::Recorded
            }
        };

        state.digest = Self::attestation_digest(env, settlement_id, &state);
        Self::store_attestation_state(env, settlement_id, &state);
        if matches!(outcome, AttestationOutcome::Finalized | AttestationOutcome::Failed) {
            Self::store_settlement(env, &instruction);
        }

        env.events().publish(
            (Symbol::new(env, "cbdc_att"), Symbol::new(env, "attest")),
            (settlement_id.clone(), validator.clone(), confirmation_hash.clone(), votes),
        );
        if matches!(outcome, AttestationOutcome::Finalized | AttestationOutcome::Failed) {
            env.events().publish(
                (Symbol::new(env, "cbdc_att"), Symbol::new(env, "final")),
                (
                    settlement_id.clone(),
                    instruction.status,
                    state.finalized_hash.clone(),
                    state.digest.clone(),
                ),
            );
        }

        Ok(outcome)
    }

    /// Deterministic SHA-256 digest over the canonical attestation state:
    /// `sha256(xdr(settlement_id, validators, quorum, attestations, tallies, finalized_hash))`.
    ///
    /// Because every collection is stored in canonical order, the digest is
    /// identical for every interleaving of the same set of attestations
    /// (INV-ATT-4) and can be recomputed off-chain from the stored state.
    pub fn attestation_digest(
        env: &Env,
        settlement_id: &BytesN<32>,
        state: &AttestationState,
    ) -> BytesN<32> {
        let preimage = (
            settlement_id.clone(),
            state.validators.clone(),
            state.quorum,
            state.attestations.clone(),
            state.tallies.clone(),
            state.finalized_hash.clone(),
        )
            .to_xdr(env);
        env.crypto().sha256(&preimage).to_bytes()
    }

    fn store_settlement(env: &Env, instruction: &SettlementInstruction) {
        let key = InteropDataKey::Settlement(instruction.settlement_id.clone());
        env.storage().persistent().set(&key, instruction);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
    }

    fn store_attestation_state(env: &Env, settlement_id: &BytesN<32>, state: &AttestationState) {
        let key = InteropDataKey::Attestations(settlement_id.clone());
        env.storage().persistent().set(&key, state);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
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
    use alloc::vec::Vec;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;

    fn status_strategy() -> impl Strategy<Value = SettlementStatus> {
        prop::sample::select(&[
            SettlementStatus::Created,
            SettlementStatus::InitiatedSource,
            SettlementStatus::ConfirmedSource,
            SettlementStatus::AwaitingDestination,
            SettlementStatus::Completed,
            SettlementStatus::Failed,
        ][..])
    }

    #[test]
    fn test_exchange_rate_validation() {
        let rate = ExchangeRate {
            source_pilot: 0,
            dest_pilot: 1,
            max_volatility: 500,
            timestamp: 1000,
            rate: 1_000_000_000_000_000_000,
            bid_price: 950_000_000_000_000_000,
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
        assert_eq!(fee, 2_500);
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
            from in status_strategy(),
            to in status_strategy(),
        ) {
            let r1 = InteropManager::is_valid_status_transition(from, to);
            let r2 = InteropManager::is_valid_status_transition(from, to);
            prop_assert_eq!(r1, r2);

            // Terminal states are absorbing: no transition out of them.
            if from.is_terminal() {
                prop_assert!(!r1);
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

        let mut ids = Vec::new();
        for i in 0..10u32 {
            let id = InteropManager::execute_atomic_swap(
                &env,
                &from,
                &to,
                1_000_000 + i as u128,
                2_000_000 + i as u128,
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
                1_000_000 + i as u128,
                2_000_000 + i as u128,
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

    /// Multi-hop path computation is order-sensitive: every hop truncates to
    /// an integer amount, so reordering fractional rates can change the
    /// result, while re-running the same path is always stable.
    #[test]
    fn test_path_computation_order_sensitive() {
        let env = Env::default();
        let one_and_half = RATE_SCALE * 3 / 2;
        let double = RATE_SCALE * 2;

        let path_a = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8]),
            rates: soroban_sdk::Vec::from_array(&env, [one_and_half, double]),
            total_cost_bps: 10,
        };
        let path_b = SettlementPath {
            path: Bytes::from_slice(&env, &[0u8, 1u8]),
            rates: soroban_sdk::Vec::from_array(&env, [double, one_and_half]),
            total_cost_bps: 10,
        };

        // 1 * 1.5 -> 1 (truncated), * 2 -> 2  vs  1 * 2 -> 2, * 1.5 -> 3.
        let result_a = path_a.compute_final_amount(1).unwrap();
        let result_b = path_b.compute_final_amount(1).unwrap();
        assert_eq!(result_a, 2);
        assert_eq!(result_b, 3);

        // Re-running the same path is deterministic.
        assert_eq!(path_a.compute_final_amount(1).unwrap(), result_a);
        assert_eq!(path_b.compute_final_amount(1).unwrap(), result_b);
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
            InteropManager::convert_amount(u128::MAX, 2),
            Err(InteropError::ConversionOverflow)
        );
        // `u128::MAX * 1` does not overflow; it truncates to zero after scaling.
        assert_eq!(InteropManager::convert_amount(u128::MAX, 1), Ok(u128::MAX / RATE_SCALE));
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
            rates: soroban_sdk::Vec::from_array(&env, [RATE_SCALE, RATE_SCALE]),
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
        // Terminal statuses are absorbing — including Completed -> Failed,
        // which previously let a late failure report overwrite a finalized
        // settlement.
        for terminal in [SettlementStatus::Completed, SettlementStatus::Failed] {
            for next in [
                SettlementStatus::Created,
                SettlementStatus::InitiatedSource,
                SettlementStatus::ConfirmedSource,
                SettlementStatus::AwaitingDestination,
                SettlementStatus::Completed,
                SettlementStatus::Failed,
            ] {
                assert!(
                    !InteropManager::is_valid_status_transition(terminal, next),
                    "{terminal:?} → {next:?} should be disallowed"
                );
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

    // ════════════════════════════════════════════════════════════════════════════════
    // Validator attestation quorum — simultaneous attestation race tests
    // ════════════════════════════════════════════════════════════════════════════════

    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::xdr::{ContractEventBody, ScVal};
    use soroban_sdk::{contract, contractimpl};

    /// Empty host contract that provides a storage context for the module.
    #[contract]
    struct InteropHarness;

    #[contractimpl]
    impl InteropHarness {}

    struct Fixture {
        env: Env,
        contract: Address,
        validators: Vec<Address>,
    }

    impl Fixture {
        fn new(n: u32, quorum: u32) -> Self {
            let env = Env::default();
            env.mock_all_auths();
            let contract = env.register(InteropHarness, ());
            let validators: Vec<Address> = (0..n).map(|_| Address::generate(&env)).collect();
            let f = Fixture { env, contract, validators };
            f.run(|env| {
                let mut set = soroban_sdk::Vec::new(env);
                for v in &f.validators {
                    set.push_back(v.clone());
                }
                InteropManager::configure_validators(env, &set, quorum).unwrap();
            });
            f
        }

        fn run<T>(&self, f: impl FnOnce(&Env) -> T) -> T {
            self.env.as_contract(&self.contract, || f(&self.env))
        }

        fn hash(&self, b: u8) -> BytesN<32> {
            BytesN::from_array(&self.env, &[b; 32])
        }

        /// Register a settlement and advance it to `AwaitingDestination`.
        fn open_settlement(&self, id: u8) -> BytesN<32> {
            let settlement_id = self.hash(id);
            let instruction = SettlementInstruction {
                settlement_id: settlement_id.clone(),
                transaction: CBDCTransaction {
                    tx_id: BytesN::from_array(&self.env, &[id ^ 0xFF; 32]),
                    source_pilot: 0,
                    dest_pilot: 1,
                    from: Address::generate(&self.env),
                    to: Address::generate(&self.env),
                    amount_source: 1_000,
                    amount_dest: 1_200,
                    exchange_rate: RATE_SCALE,
                    timestamp: 1_000,
                    protocol: 0,
                    privacy_tier: 0,
                    offline_status: None,
                    metadata: Bytes::new(&self.env),
                },
                protocol: 0,
                status: SettlementStatus::Created as u32,
                timestamp: 1_000,
                confirmation_hash: None,
            };
            self.run(|env| {
                InteropManager::register_settlement(env, &instruction).unwrap();
                for to in [
                    SettlementStatus::InitiatedSource,
                    SettlementStatus::ConfirmedSource,
                    SettlementStatus::AwaitingDestination,
                ] {
                    InteropManager::advance_settlement_status(env, &settlement_id, to).unwrap();
                }
            });
            settlement_id
        }

        fn attest(
            &self,
            validator: usize,
            id: &BytesN<32>,
            hash: &BytesN<32>,
        ) -> Result<AttestationOutcome, InteropError> {
            self.run(|env| {
                InteropManager::attest_settlement(env, &self.validators[validator], id, hash)
            })
        }

        fn state(&self, id: &BytesN<32>) -> AttestationState {
            self.run(|env| InteropManager::get_attestation_state(env, id).unwrap())
        }

        fn status(&self, id: &BytesN<32>) -> SettlementStatus {
            self.run(|env| {
                SettlementStatus::from_u32(InteropManager::get_settlement(env, id).unwrap().status)
                    .unwrap()
            })
        }

        /// Number of contract events whose topics contain `topic`.
        fn count_topic(&self, topic: &str) -> usize {
            self.env
                .events()
                .all()
                .events()
                .iter()
                .filter(|e| match &e.body {
                    ContractEventBody::V0(body) => body.topics.iter().any(
                        |t| matches!(t, ScVal::Symbol(s) if s.0.as_slice() == topic.as_bytes()),
                    ),
                })
                .count()
        }
    }

    #[test]
    fn test_configure_validators_requires_strict_majority_quorum() {
        let env = Env::default();
        let contract = env.register(InteropHarness, ());
        env.as_contract(&contract, || {
            let mut set = soroban_sdk::Vec::new(&env);
            for _ in 0..4 {
                set.push_back(Address::generate(&env));
            }
            // 2 of 4 is not a strict majority: two conflicting hashes could
            // both reach quorum.
            assert_eq!(
                InteropManager::configure_validators(&env, &set, 2),
                Err(InteropError::InvalidQuorum)
            );
            assert_eq!(
                InteropManager::configure_validators(&env, &set, 5),
                Err(InteropError::InvalidQuorum)
            );
            assert_eq!(
                InteropManager::configure_validators(&env, &set, 0),
                Err(InteropError::InvalidQuorum)
            );
            assert!(InteropManager::configure_validators(&env, &set, 3).is_ok());

            // Duplicate validators are rejected.
            let dup = set.get(0).unwrap();
            set.push_back(dup);
            assert_eq!(
                InteropManager::configure_validators(&env, &set, 3),
                Err(InteropError::InvalidValidatorSet)
            );

            // Empty and oversized sets are rejected.
            assert_eq!(
                InteropManager::configure_validators(&env, &soroban_sdk::Vec::new(&env), 1),
                Err(InteropError::InvalidValidatorSet)
            );
            let mut big = soroban_sdk::Vec::new(&env);
            for _ in 0..(MAX_VALIDATORS + 1) {
                big.push_back(Address::generate(&env));
            }
            assert_eq!(
                InteropManager::configure_validators(&env, &big, MAX_VALIDATORS),
                Err(InteropError::InvalidValidatorSet)
            );
        });
    }

    #[test]
    fn test_quorum_finalizes_exactly_once() {
        let f = Fixture::new(5, 3);
        let id = f.open_settlement(1);
        let h = f.hash(0xAA);

        assert_eq!(f.attest(0, &id, &h), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(1, &id, &h), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.status(&id), SettlementStatus::AwaitingDestination);

        assert_eq!(f.attest(2, &id, &h), Ok(AttestationOutcome::Finalized));
        assert_eq!(f.count_topic("final"), 1);
        assert_eq!(f.status(&id), SettlementStatus::Completed);
        let settled = f.run(|env| InteropManager::get_settlement(env, &id).unwrap());
        assert_eq!(settled.confirmation_hash, Some(h.clone()));

        // Simultaneous late attestations from the remaining validators are
        // recorded for audit but never re-finalize or emit another event.
        assert_eq!(f.attest(3, &id, &h), Ok(AttestationOutcome::LateRecorded));
        assert_eq!(f.count_topic("final"), 0);
        assert_eq!(f.attest(4, &id, &f.hash(0xBB)), Ok(AttestationOutcome::LateRecorded));
        assert_eq!(f.count_topic("final"), 0);

        let state = f.state(&id);
        assert_eq!(state.attestations.len(), 5);
        assert_eq!(state.finalized_hash, Some(h.clone()));
        assert_eq!(f.status(&id), SettlementStatus::Completed);
        let settled = f.run(|env| InteropManager::get_settlement(env, &id).unwrap());
        assert_eq!(settled.confirmation_hash, Some(h));
    }

    #[test]
    fn test_duplicate_and_equivocating_attestations_leave_state_untouched() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(2);
        let h1 = f.hash(1);
        let h2 = f.hash(2);

        assert_eq!(f.attest(0, &id, &h1), Ok(AttestationOutcome::Recorded));
        let before = f.state(&id);

        // Replay of the same vote cannot be double counted toward quorum.
        assert_eq!(f.attest(0, &id, &h1), Err(InteropError::DuplicateAttestation));
        // A conflicting vote from the same validator is rejected.
        assert_eq!(f.attest(0, &id, &h2), Err(InteropError::Equivocation));

        assert_eq!(f.state(&id), before);
        assert_eq!(f.status(&id), SettlementStatus::AwaitingDestination);
        assert_eq!(before.tallies.len(), 1);
        assert_eq!(before.tallies.get(0).unwrap().votes, 1);
    }

    #[test]
    fn test_non_validator_and_wrong_status_rejected() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(3);
        let outsider = Address::generate(&f.env);
        let h = f.hash(9);
        assert_eq!(
            f.run(|env| InteropManager::attest_settlement(env, &outsider, &id, &h)),
            Err(InteropError::NotAValidator)
        );

        // Unknown settlement.
        assert_eq!(
            f.attest(0, &f.hash(0x77), &h),
            Err(InteropError::SettlementNotFound)
        );

        // A settlement still in `Created` does not accept attestations.
        let mut fresh = f.run(|env| InteropManager::get_settlement(env, &id).unwrap());
        fresh.settlement_id = f.hash(0x55);
        fresh.status = SettlementStatus::Created as u32;
        f.run(|env| InteropManager::register_settlement(env, &fresh).unwrap());
        assert_eq!(
            f.attest(0, &fresh.settlement_id, &h),
            Err(InteropError::NotAwaitingAttestation)
        );

        // Re-registering the same ID is rejected.
        assert_eq!(
            f.run(|env| InteropManager::register_settlement(env, &fresh)),
            Err(InteropError::SettlementAlreadyExists)
        );
    }

    #[test]
    fn test_conflicting_simultaneous_attestations_cannot_both_finalize() {
        // 5 validators, quorum 3: interleaved votes for two hashes.
        let f = Fixture::new(5, 3);
        let id = f.open_settlement(4);
        let h1 = f.hash(1);
        let h2 = f.hash(2);

        assert_eq!(f.attest(0, &id, &h1), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(1, &id, &h2), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(2, &id, &h2), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(3, &id, &h1), Ok(AttestationOutcome::Recorded));
        // Whichever hash gets the deciding vote wins; the other is locked out.
        assert_eq!(f.attest(4, &id, &h1), Ok(AttestationOutcome::Finalized));
        assert_eq!(f.state(&id).finalized_hash, Some(h1));
        assert_eq!(f.status(&id), SettlementStatus::Completed);
    }

    #[test]
    fn test_split_vote_fails_deterministically_when_quorum_unreachable() {
        // 4 validators, quorum 3. After h1, h2, h3 no hash can reach 3 votes.
        let f = Fixture::new(4, 3);
        let id = f.open_settlement(5);
        assert_eq!(f.attest(0, &id, &f.hash(1)), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(1, &id, &f.hash(2)), Ok(AttestationOutcome::Recorded));
        assert_eq!(f.attest(2, &id, &f.hash(3)), Ok(AttestationOutcome::Failed));
        assert_eq!(f.count_topic("final"), 1);
        assert_eq!(f.status(&id), SettlementStatus::Failed);
        assert_eq!(f.state(&id).finalized_hash, None);

        assert_eq!(f.attest(3, &id, &f.hash(1)), Ok(AttestationOutcome::LateRecorded));
        assert_eq!(f.count_topic("final"), 0);
        assert_eq!(f.status(&id), SettlementStatus::Failed);
    }

    #[test]
    fn test_failure_report_cannot_overwrite_finalized_settlement() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(6);
        let h = f.hash(1);
        f.attest(0, &id, &h).unwrap();
        assert_eq!(f.attest(1, &id, &h), Ok(AttestationOutcome::Finalized));

        // A racing failure report arriving after finalization is rejected.
        assert_eq!(
            f.run(|env| InteropManager::advance_settlement_status(env, &id, SettlementStatus::Failed)),
            Err(InteropError::InvalidStatusTransition)
        );
        assert_eq!(f.status(&id), SettlementStatus::Completed);
    }

    #[test]
    fn test_failure_before_quorum_wins_and_late_votes_do_not_complete() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(7);
        let h = f.hash(1);
        f.attest(0, &id, &h).unwrap();
        f.run(|env| {
            InteropManager::advance_settlement_status(env, &id, SettlementStatus::Failed).unwrap()
        });
        assert_eq!(f.attest(1, &id, &h), Ok(AttestationOutcome::LateRecorded));
        assert_eq!(f.status(&id), SettlementStatus::Failed);
        assert_eq!(f.state(&id).finalized_hash, None);
    }

    #[test]
    fn test_completed_cannot_be_forced_without_quorum() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(8);
        assert_eq!(
            f.run(|env| {
                InteropManager::advance_settlement_status(env, &id, SettlementStatus::Completed)
            }),
            Err(InteropError::InvalidStatusTransition)
        );
        assert_eq!(f.status(&id), SettlementStatus::AwaitingDestination);
    }

    #[test]
    fn test_validator_reconfiguration_does_not_affect_inflight_snapshot() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(9);
        let h = f.hash(1);
        f.attest(0, &id, &h).unwrap();

        // Rotate to a completely new validator set mid-flight.
        f.run(|env| {
            let mut set = soroban_sdk::Vec::new(env);
            set.push_back(Address::generate(env));
            InteropManager::configure_validators(env, &set, 1).unwrap();
        });

        // Old validators still finalize the in-flight settlement.
        assert_eq!(f.attest(1, &id, &h), Ok(AttestationOutcome::Finalized));
        assert_eq!(f.state(&id).validators.len(), 3);
    }

    #[test]
    fn test_awaiting_destination_requires_configured_validators() {
        let env = Env::default();
        let contract = env.register(InteropHarness, ());
        env.as_contract(&contract, || {
            let id = BytesN::from_array(&env, &[1u8; 32]);
            let instruction = SettlementInstruction {
                settlement_id: id.clone(),
                transaction: CBDCTransaction {
                    tx_id: BytesN::from_array(&env, &[2u8; 32]),
                    source_pilot: 0,
                    dest_pilot: 1,
                    from: Address::generate(&env),
                    to: Address::generate(&env),
                    amount_source: 1,
                    amount_dest: 1,
                    exchange_rate: RATE_SCALE,
                    timestamp: 0,
                    protocol: 0,
                    privacy_tier: 0,
                    offline_status: None,
                    metadata: Bytes::new(&env),
                },
                protocol: 0,
                status: SettlementStatus::Created as u32,
                timestamp: 0,
                confirmation_hash: None,
            };
            InteropManager::register_settlement(&env, &instruction).unwrap();
            InteropManager::advance_settlement_status(&env, &id, SettlementStatus::InitiatedSource)
                .unwrap();
            InteropManager::advance_settlement_status(&env, &id, SettlementStatus::ConfirmedSource)
                .unwrap();
            assert_eq!(
                InteropManager::advance_settlement_status(
                    &env,
                    &id,
                    SettlementStatus::AwaitingDestination
                ),
                Err(InteropError::ValidatorsNotConfigured)
            );
            // Skipping states is rejected.
            assert_eq!(
                InteropManager::advance_settlement_status(&env, &id, SettlementStatus::Created),
                Err(InteropError::InvalidStatusTransition)
            );
            // Non-Created registrations are rejected.
            let mut bad = instruction.clone();
            bad.settlement_id = BytesN::from_array(&env, &[3u8; 32]);
            bad.status = SettlementStatus::Completed as u32;
            assert_eq!(
                InteropManager::register_settlement(&env, &bad),
                Err(InteropError::InvalidStatusTransition)
            );
        });
    }

    #[test]
    fn test_status_decoding_round_trips() {
        for s in [
            SettlementStatus::Created,
            SettlementStatus::InitiatedSource,
            SettlementStatus::ConfirmedSource,
            SettlementStatus::AwaitingDestination,
            SettlementStatus::Completed,
            SettlementStatus::Failed,
        ] {
            assert_eq!(SettlementStatus::from_u32(s as u32), Some(s));
        }
        assert_eq!(SettlementStatus::from_u32(6), None);
        assert_eq!(SettlementStatus::from_u32(u32::MAX), None);
    }

    /// Votes for one validator: `None` abstains, `Some(k)` votes hash `k`.
    fn vote_plan_strategy() -> impl Strategy<Value = (u32, Vec<Option<u8>>, Vec<usize>)> {
        (1u32..=7).prop_flat_map(|n| {
            let quorum = n / 2 + 1;
            (
                Just(quorum),
                prop::collection::vec(prop::option::of(0u8..3), n as usize),
            )
                .prop_flat_map(|(quorum, votes)| {
                    let order: Vec<usize> = (0..votes.len()).collect();
                    (Just(quorum), Just(votes), Just(order).prop_shuffle())
                })
        })
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(256))]

        /// INV-ATT-2/3/4: for any set of honest votes, applying them in
        /// validator order and in a random shuffled order yields byte-identical
        /// attestation state, digest and settlement status; at most one hash is
        /// ever finalized and only with at least `quorum` votes.
        #[test]
        fn prop_attestation_order_independence((quorum, votes, order) in vote_plan_strategy()) {
            let n = votes.len() as u32;
            let f = Fixture::new(n, quorum);
            let id_a = f.open_settlement(0xA0);
            let id_b = f.open_settlement(0xB0);

            let mut finals_b = 0usize;
            for (v, vote) in votes.iter().enumerate() {
                if let Some(k) = vote {
                    f.attest(v, &id_a, &f.hash(*k)).unwrap();
                }
            }
            for &v in &order {
                if let Some(k) = votes[v] {
                    f.attest(v, &id_b, &f.hash(k)).unwrap();
                    finals_b += f.count_topic("final");
                }
            }

            let a = f.state(&id_a);
            let b = f.state(&id_b);
            prop_assert_eq!(&a.attestations, &b.attestations);
            prop_assert_eq!(&a.tallies, &b.tallies);
            prop_assert_eq!(&a.finalized_hash, &b.finalized_hash);
            prop_assert_eq!(f.status(&id_a), f.status(&id_b));
            prop_assert!(finals_b <= 1);

            // Digest equivalence: recompute A's digest as if stored under B's ID.
            let a_as_b = f.run(|env| InteropManager::attestation_digest(env, &id_b, &a));
            prop_assert_eq!(a_as_b, b.digest.clone());

            // Finalized hash, if any, has quorum and is unique.
            let reaching: Vec<_> = a.tallies.iter().filter(|t| t.votes >= quorum).collect();
            prop_assert!(reaching.len() <= 1);
            match &a.finalized_hash {
                Some(h) => {
                    prop_assert_eq!(f.status(&id_a), SettlementStatus::Completed);
                    prop_assert_eq!(&reaching[0].confirmation_hash, h);
                }
                None => prop_assert_ne!(f.status(&id_a), SettlementStatus::Completed),
            }
        }
    }
}
