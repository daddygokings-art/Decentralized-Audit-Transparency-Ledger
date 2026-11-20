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
//!
//! # Error propagation & structured events
//!
//! Every [`InteropError`] has a stable numeric [`code`](InteropError::code)
//! and a short on-chain [`symbol`](InteropError::as_symbol). Lifecycle events
//! are typed `#[contractevent]` structs ([`SettlementStatusChanged`],
//! [`AttestationRecorded`], [`SettlementFinalized`]) so their schema appears
//! in the contract spec and indexers can decode them without guessing tuple
//! layouts. Topic layouts are unchanged from the previous untyped events.
//!
//! [`InteropManager::observe`] wraps any fallible result for `?` propagation
//! and emits an [`InteropFailure`] event on error. Note that Soroban discards
//! contract events from an invocation that ultimately fails, so failure
//! events are only persisted when the caller handles the error and the
//! invocation succeeds (e.g. a batch processor that skips bad items); in a
//! failing invocation the typed error code itself is the signal.
//!
//! # Signature verification & replay protection
//!
//! Cross-ledger settlement approvals are ed25519 signatures over a
//! [`SettlementAuthorization`]. The signed message is the SHA-256
//! [`authorization_digest`](InteropManager::authorization_digest) of a
//! fixed-layout payload that binds:
//!
//! * a versioned domain tag ([`AUTH_DOMAIN_TAG`]) — no cross-protocol reuse;
//! * the network id — no cross-network (testnet ↔ mainnet) replay;
//! * the verifying contract address — no cross-deployment replay;
//! * the settlement id — no cross-settlement replay;
//! * the signer key, a per-signer nonce and an expiry timestamp.
//!
//! Nonces are strictly monotonic per signer and stored in `Persistent`
//! storage, so a consumed signature (or any older one) can never be accepted
//! again, and an equivocating signer cannot authorize two settlements with the
//! same nonce. Archived persistent entries cannot be read as "missing" by the
//! host, so TTL expiry cannot resurrect old nonces. Expiry is capped at
//! [`MAX_AUTH_VALIDITY_SECS`] to bound the exposure window of a leaked
//! signature.
//!
//! [`InteropManager::verify_quorum`] checks every attestation (membership,
//! duplicates, settlement binding, expiry, nonce) before any signature is
//! verified and before any nonce is written, so a rejected quorum never
//! leaves partial state behind. Invalid ed25519 signatures trap the host
//! (Soroban has no non-aborting verify), which aborts and rolls back the
//! whole invocation.

use crate::cbdc_types::{CBDCTransaction, InteropProtocol};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contracterror, contractevent, contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol,
};

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
/// Versioned domain-separation tag prefixed to every signed authorization.
pub const AUTH_DOMAIN_TAG: [u8; 16] = *b"CBDC-IOP-AUTH-V1";
/// Maximum lifetime of a settlement authorization (1 hour).
pub const MAX_AUTH_VALIDITY_SECS: u64 = 3_600;
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
    /// Authorization expiry is in the past.
    AuthorizationExpired = 30,
    /// Authorization expiry exceeds [`MAX_AUTH_VALIDITY_SECS`] from now.
    AuthorizationValidityTooLong = 31,
    /// Nonce is not strictly greater than the signer's last consumed nonce.
    NonceReplayed = 32,
    /// Fewer distinct valid attestations than the quorum threshold.
    QuorumNotReached = 33,
    /// Signed authorization is bound to a different settlement id.
    SettlementMismatch = 34,
    /// Attestation bundle is longer than [`MAX_VALIDATORS`].
    TooManyAttestations = 35,
}

impl InteropError {
    /// Stable numeric code, identical to the on-chain contract error code.
    pub fn code(&self) -> u32 {
        *self as u32
    }

    /// Short on-chain symbol (≤ 9 chars, built without a host allocation)
    /// used in structured failure events.
    pub fn as_symbol(&self) -> Symbol {
        match self {
            InteropError::StaleRate => symbol_short!("STALE"),
            InteropError::ZeroRate => symbol_short!("ZERO_RATE"),
            InteropError::ZeroPrice => symbol_short!("ZERO_PX"),
            InteropError::BidExceedsAsk => symbol_short!("BID_ASK"),
            InteropError::ZeroAmount => symbol_short!("ZERO_AMT"),
            InteropError::ConversionOverflow => symbol_short!("CONV_OVF"),
            InteropError::DivisionByZero => symbol_short!("DIV_ZERO"),
            InteropError::FeeBpsTooHigh => symbol_short!("FEE_BPS"),
            InteropError::FeeOverflow => symbol_short!("FEE_OVF"),
            InteropError::ZeroSettlementAmount => symbol_short!("ZERO_STL"),
            InteropError::ZeroSettlementRate => symbol_short!("ZERO_SRT"),
            InteropError::PathTooShort => symbol_short!("PATH_SHRT"),
            InteropError::PathTooLong => symbol_short!("PATH_LONG"),
            InteropError::RatesTooLong => symbol_short!("RATE_LONG"),
            InteropError::PathLengthMismatch => symbol_short!("PATH_MISM"),
            InteropError::SamePilot => symbol_short!("SAME_PLT"),
            InteropError::InvalidPilot => symbol_short!("BAD_PLT"),
            InteropError::SpreadOverflow => symbol_short!("SPRD_OVF"),
            InteropError::NotAValidator => symbol_short!("NOT_VALID"),
            InteropError::DuplicateAttestation => symbol_short!("DUP_ATT"),
            InteropError::Equivocation => symbol_short!("EQUIVOC"),
            InteropError::SettlementNotFound => symbol_short!("STL_NONE"),
            InteropError::SettlementAlreadyExists => symbol_short!("STL_EXIST"),
            InteropError::InvalidStatusTransition => symbol_short!("BAD_TRANS"),
            InteropError::InvalidQuorum => symbol_short!("BAD_QRM"),
            InteropError::InvalidValidatorSet => symbol_short!("BAD_VSET"),
            InteropError::NotAwaitingAttestation => symbol_short!("NOT_AWAIT"),
            InteropError::InvalidStatus => symbol_short!("BAD_STAT"),
            InteropError::ValidatorsNotConfigured => symbol_short!("NO_VSET"),
            InteropError::AuthorizationExpired => symbol_short!("AUTH_EXP"),
            InteropError::AuthorizationValidityTooLong => symbol_short!("AUTH_LONG"),
            InteropError::NonceReplayed => symbol_short!("REPLAY"),
            InteropError::QuorumNotReached => symbol_short!("NO_QUORUM"),
            InteropError::SettlementMismatch => symbol_short!("STL_MISM"),
            InteropError::TooManyAttestations => symbol_short!("TOO_MANY"),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Structured events
// ════════════════════════════════════════════════════════════════════════════

/// Emitted by [`InteropManager::observe`] when an observed operation fails.
///
/// Topics: `["cbdc_err", operation]`.
#[contractevent(topics = ["cbdc_err"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteropFailure {
    /// Logical operation that failed (e.g. `convert`, `attest`).
    #[topic]
    pub operation: Symbol,
    /// Stable numeric [`InteropError`] code.
    pub error_code: u32,
    /// Short symbolic [`InteropError`] name.
    pub error: Symbol,
    /// Ledger timestamp at the moment of failure.
    pub timestamp: u64,
}

/// Emitted when a signed settlement authorization is accepted and its nonce
/// consumed.
///
/// Topics: `["cbdc_auth", "accepted", signer]`.
#[contractevent(topics = ["cbdc_auth", "accepted"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationAccepted {
    /// ed25519 public key of the signer.
    #[topic]
    pub signer: BytesN<32>,
    /// Settlement the signature authorizes.
    pub settlement_id: BytesN<32>,
    /// Nonce consumed by this authorization.
    pub nonce: u64,
}

// ════════════════════════════════════════════════════════════════════════════
// Signed authorizations
// ════════════════════════════════════════════════════════════════════════════

/// The statement a signer (bank, validator, settlement agent) approves.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementAuthorization {
    /// Settlement being authorized.
    pub settlement_id: BytesN<32>,
    /// ed25519 public key of the signer.
    pub signer: BytesN<32>,
    /// Strictly increasing per-signer nonce; the first valid nonce is 1.
    pub nonce: u64,
    /// Ledger timestamp (seconds) after which the signature is void.
    pub expires_at: u64,
}

/// A [`SettlementAuthorization`] together with its ed25519 signature over
/// [`InteropManager::authorization_digest`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedAuthorization {
    pub auth: SettlementAuthorization,
    pub signature: BytesN<64>,
}

/// Emitted when a stored settlement moves between lifecycle states.
///
/// Topics: `["cbdc_settle", new_status_symbol]` (unchanged layout).
#[contractevent(topics = ["cbdc_settle"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementStatusChanged {
    /// [`SettlementStatus::as_symbol`] of the new status.
    #[topic]
    pub status: Symbol,
    /// Settlement whose status changed.
    pub settlement_id: BytesN<32>,
    /// Previous [`SettlementStatus`] code.
    pub from_status: u32,
    /// New [`SettlementStatus`] code.
    pub to_status: u32,
    /// Ledger timestamp of the transition.
    pub timestamp: u64,
}

/// Emitted for every accepted validator attestation.
///
/// Topics: `["cbdc_att", "attest"]` (unchanged layout).
#[contractevent(topics = ["cbdc_att", "attest"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationRecorded {
    pub settlement_id: BytesN<32>,
    pub validator: Address,
    pub confirmation_hash: BytesN<32>,
    /// Votes for `confirmation_hash` after this attestation.
    pub votes: u32,
}

/// Emitted exactly once when an attested settlement is finalized as
/// `Completed` or `Failed`.
///
/// Topics: `["cbdc_att", "final"]` (unchanged layout).
#[contractevent(topics = ["cbdc_att", "final"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementFinalized {
    pub settlement_id: BytesN<32>,
    /// Final [`SettlementStatus`] code.
    pub status: u32,
    /// Hash that reached quorum, if the settlement completed.
    pub finalized_hash: Option<BytesN<32>>,
    /// Canonical [`AttestationState`] digest at finalization.
    pub digest: BytesN<32>,
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

    /// Stable numeric code as stored in [`SettlementInstruction::status`].
    pub fn code(&self) -> u32 {
        *self as u32
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
        input.append(&from.to_xdr(env));
        input.append(&to.to_xdr(env));
        input.append(&Bytes::from_slice(env, &source_amount.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &dest_amount.to_le_bytes()));

        Ok(env.crypto().sha256(&input).to_bytes())
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
        input.append(&hub_address.to_xdr(env));
        input.append(&Bytes::from_slice(env, &source_amount.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &dest_amount.to_le_bytes()));

        Ok(env.crypto().sha256(&input).to_bytes())
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

    /// Pass a result through unchanged, emitting a structured
    /// [`InteropFailure`] event if it is an error.
    ///
    /// Designed for `?`-style propagation inside contract entrypoints:
    ///
    /// ```ignore
    /// let out = InteropManager::observe(
    ///     &env,
    ///     symbol_short!("convert"),
    ///     InteropManager::convert_amount(amount, rate),
    /// )?;
    /// ```
    ///
    /// Must be called from within a contract frame (events require one). See
    /// the module docs for when failure events are persisted.
    pub fn observe<T>(
        env: &Env,
        operation: Symbol,
        result: Result<T, InteropError>,
    ) -> Result<T, InteropError> {
        if let Err(err) = &result {
            InteropFailure {
                operation,
                error_code: err.code(),
                error: err.as_symbol(),
                timestamp: env.ledger().timestamp(),
            }
            .publish(env);
        }
        result
    }

    // ── Signature verification & replay protection ───────────────────────

    /// Deterministic message digest an off-chain signer must sign.
    ///
    /// Layout (all fixed-width, so the encoding is unambiguous):
    /// `AUTH_DOMAIN_TAG(16) ‖ network_id(32) ‖ xdr(verifier) ‖ settlement_id(32)
    ///  ‖ signer(32) ‖ nonce(u64 BE) ‖ expires_at(u64 BE)`.
    ///
    /// `verifier` is the address of the contract that will check the
    /// signature (normally `env.current_contract_address()`).
    pub fn authorization_digest(
        env: &Env,
        verifier: &Address,
        auth: &SettlementAuthorization,
    ) -> BytesN<32> {
        let mut msg = Bytes::from_array(env, &AUTH_DOMAIN_TAG);
        msg.append(&env.ledger().network_id().into());
        msg.append(&verifier.to_xdr(env));
        msg.append(&auth.settlement_id.clone().into());
        msg.append(&auth.signer.clone().into());
        let mut tail = [0u8; 16];
        tail[..8].copy_from_slice(&auth.nonce.to_be_bytes());
        tail[8..].copy_from_slice(&auth.expires_at.to_be_bytes());
        msg.extend_from_array(&tail);
        env.crypto().sha256(&msg).to_bytes()
    }

    /// Last nonce consumed by `signer` (0 if it has never signed).
    pub fn last_nonce(env: &Env, signer: &BytesN<32>) -> u64 {
        env.storage()
            .persistent()
            .get(&InteropDataKey::SignerNonce(signer.clone()))
            .unwrap_or(0)
    }

    /// Read-only freshness checks: expiry window and nonce monotonicity.
    fn check_freshness(env: &Env, auth: &SettlementAuthorization) -> Result<(), InteropError> {
        let now = env.ledger().timestamp();
        if now > auth.expires_at {
            return Err(InteropError::AuthorizationExpired);
        }
        if auth.expires_at - now > MAX_AUTH_VALIDITY_SECS {
            return Err(InteropError::AuthorizationValidityTooLong);
        }
        if auth.nonce <= Self::last_nonce(env, &auth.signer) {
            return Err(InteropError::NonceReplayed);
        }
        Ok(())
    }

    /// Verify the ed25519 signature. Traps (aborting the invocation) if the
    /// signature is invalid.
    fn verify_signature(env: &Env, signed: &SignedAuthorization) {
        let digest = Self::authorization_digest(env, &env.current_contract_address(), &signed.auth);
        env.crypto()
            .ed25519_verify(&signed.auth.signer, &digest.into(), &signed.signature);
    }

    /// Record the nonce as consumed and emit [`AuthorizationAccepted`].
    fn consume_nonce(env: &Env, auth: &SettlementAuthorization) {
        let key = InteropDataKey::SignerNonce(auth.signer.clone());
        let storage = env.storage().persistent();
        storage.set(&key, &auth.nonce);
        storage.extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
        AuthorizationAccepted {
            signer: auth.signer.clone(),
            settlement_id: auth.settlement_id.clone(),
            nonce: auth.nonce,
        }
        .publish(env);
    }

    /// Verify a single signed authorization for `settlement_id` and consume
    /// its nonce.
    ///
    /// Typed errors are returned (with no state change) for expired,
    /// over-long, replayed or mis-bound authorizations. An invalid signature
    /// traps the host. Must run inside a contract frame.
    pub fn verify_authorization(
        env: &Env,
        settlement_id: &BytesN<32>,
        signed: &SignedAuthorization,
    ) -> Result<(), InteropError> {
        if signed.auth.settlement_id != *settlement_id {
            return Err(InteropError::SettlementMismatch);
        }
        Self::check_freshness(env, &signed.auth)?;
        Self::verify_signature(env, signed);
        Self::consume_nonce(env, &signed.auth);
        Ok(())
    }

    /// Verify that at least `threshold` distinct registered validators signed
    /// `settlement_id`, consuming each attestation's nonce.
    ///
    /// Uses the same set/quorum rules as [`Self::configure_validators`]: the
    /// set must be non-empty, duplicate-free and at most [`MAX_VALIDATORS`]
    /// keys, and `threshold` must be a strict majority
    /// (`n / 2 < threshold <= n`) so two conflicting settlements can never
    /// both gather a quorum from the same set.
    ///
    /// All structural and freshness checks run first and no nonce is written
    /// unless the whole quorum is valid, so a rejected call leaves no partial
    /// state even if the caller swallows the error. Returns the number of
    /// accepted attestations.
    pub fn verify_quorum(
        env: &Env,
        settlement_id: &BytesN<32>,
        validators: &soroban_sdk::Vec<BytesN<32>>,
        threshold: u32,
        attestations: &soroban_sdk::Vec<SignedAuthorization>,
    ) -> Result<u32, InteropError> {
        let n = validators.len();
        if n == 0 || n > MAX_VALIDATORS {
            return Err(InteropError::InvalidValidatorSet);
        }
        if attestations.len() > MAX_VALIDATORS {
            return Err(InteropError::TooManyAttestations);
        }
        if threshold > n || threshold.saturating_mul(2) <= n {
            return Err(InteropError::InvalidQuorum);
        }
        for i in 0..n {
            let v = validators.get_unchecked(i);
            for j in (i + 1)..n {
                if validators.get_unchecked(j) == v {
                    return Err(InteropError::InvalidValidatorSet);
                }
            }
        }

        // Phase 1: structural + freshness checks (read-only).
        let count = attestations.len();
        for i in 0..count {
            let a = attestations.get_unchecked(i).auth;
            if a.settlement_id != *settlement_id {
                return Err(InteropError::SettlementMismatch);
            }
            if !validators.contains(&a.signer) {
                return Err(InteropError::NotAValidator);
            }
            for j in 0..i {
                if attestations.get_unchecked(j).auth.signer == a.signer {
                    return Err(InteropError::DuplicateAttestation);
                }
            }
            Self::check_freshness(env, &a)?;
        }
        if count < threshold {
            return Err(InteropError::QuorumNotReached);
        }

        // Phase 2: cryptographic verification (traps on forgery).
        for signed in attestations.iter() {
            Self::verify_signature(env, &signed);
        }

        // Phase 3: commit nonces only after every check has passed.
        for signed in attestations.iter() {
            Self::consume_nonce(env, &signed.auth);
        }
        Ok(count)
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
    /// `Persistent`: last consumed authorization nonce of an ed25519 signer.
    SignerNonce(BytesN<32>),
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

        instruction.status = to.code();
        Self::store_settlement(env, &instruction);
        SettlementStatusChanged {
            status: to.as_symbol(env),
            settlement_id: settlement_id.clone(),
            from_status: from.code(),
            to_status: to.code(),
            timestamp: env.ledger().timestamp(),
        }
        .publish(env);
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

        AttestationRecorded {
            settlement_id: settlement_id.clone(),
            validator: validator.clone(),
            confirmation_hash: confirmation_hash.clone(),
            votes,
        }
        .publish(env);
        if matches!(outcome, AttestationOutcome::Finalized | AttestationOutcome::Failed) {
            SettlementFinalized {
                settlement_id: settlement_id.clone(),
                status: instruction.status,
                finalized_hash: state.finalized_hash.clone(),
                digest: state.digest.clone(),
            }
            .publish(env);
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

    // ════════════════════════════════════════════════════════════════════════════════
    // Error propagation & structured events
    // ════════════════════════════════════════════════════════════════════════════════

    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::Event as _;

    fn last_event(env: &Env) -> soroban_sdk::xdr::ContractEvent {
        env.events().all().events().last().cloned().expect("no events")
    }

    // ── Signed authorization & replay protection ─────────────────────────

    mod auth {
        use super::*;
        use ed25519_dalek::{Signer, SigningKey};

        pub(super) const NOW: u64 = 1_000_000;

        pub(super) fn key(seed: u8) -> SigningKey {
            SigningKey::from_bytes(&[seed; 32])
        }

        pub(super) fn pubkey(env: &Env, k: &SigningKey) -> BytesN<32> {
            BytesN::from_array(env, &k.verifying_key().to_bytes())
        }

        pub(super) fn sign(
            env: &Env,
            verifier: &Address,
            k: &SigningKey,
            settlement_id: &BytesN<32>,
            nonce: u64,
            expires_at: u64,
        ) -> SignedAuthorization {
            let auth = SettlementAuthorization {
                settlement_id: settlement_id.clone(),
                signer: pubkey(env, k),
                nonce,
                expires_at,
            };
            let digest = InteropManager::authorization_digest(env, verifier, &auth);
            let sig = k.sign(&digest.to_array()).to_bytes();
            SignedAuthorization { auth, signature: BytesN::from_array(env, &sig) }
        }

        pub(super) fn setup() -> (Env, Address, BytesN<32>) {
            let env = Env::default();
            env.ledger().set_timestamp(NOW);
            let id = env.register(InteropHarness, ());
            let sid = BytesN::from_array(&env, &[0xAB; 32]);
            (env, id, sid)
        }
    }

    #[test]
    fn test_valid_authorization_accepted_then_replay_rejected() {
        let (env, id, sid) = auth::setup();
        let k = auth::key(1);
        let signed = auth::sign(&env, &id, &k, &sid, 1, auth::NOW + 60);
        env.as_contract(&id, || {
            assert_eq!(InteropManager::verify_authorization(&env, &sid, &signed), Ok(()));
            assert_eq!(InteropManager::last_nonce(&env, &signed.auth.signer), 1);
            // Exact replay of the same signature.
            assert_eq!(
                InteropManager::verify_authorization(&env, &sid, &signed),
                Err(InteropError::NonceReplayed)
            );
        });
    }

    #[test]
    fn test_nonce_zero_and_rollback_rejected() {
        let (env, id, sid) = auth::setup();
        let k = auth::key(2);
        env.as_contract(&id, || {
            let zero = auth::sign(&env, &id, &k, &sid, 0, auth::NOW + 60);
            assert_eq!(
                InteropManager::verify_authorization(&env, &sid, &zero),
                Err(InteropError::NonceReplayed)
            );
            let n5 = auth::sign(&env, &id, &k, &sid, 5, auth::NOW + 60);
            InteropManager::verify_authorization(&env, &sid, &n5).unwrap();
            let n3 = auth::sign(&env, &id, &k, &sid, 3, auth::NOW + 60);
            assert_eq!(
                InteropManager::verify_authorization(&env, &sid, &n3),
                Err(InteropError::NonceReplayed)
            );
        });
    }

    #[test]
    fn test_expiry_window_enforced() {
        let (env, id, sid) = auth::setup();
        let k = auth::key(3);
        env.as_contract(&id, || {
            let expired = auth::sign(&env, &id, &k, &sid, 1, auth::NOW - 1);
            assert_eq!(
                InteropManager::verify_authorization(&env, &sid, &expired),
                Err(InteropError::AuthorizationExpired)
            );
            let too_long = auth::sign(&env, &id, &k, &sid, 1, auth::NOW + MAX_AUTH_VALIDITY_SECS + 1);
            assert_eq!(
                InteropManager::verify_authorization(&env, &sid, &too_long),
                Err(InteropError::AuthorizationValidityTooLong)
            );
            // Boundaries are inclusive.
            let at_now = auth::sign(&env, &id, &k, &sid, 1, auth::NOW);
            assert_eq!(InteropManager::verify_authorization(&env, &sid, &at_now), Ok(()));
            let at_max = auth::sign(&env, &id, &k, &sid, 2, auth::NOW + MAX_AUTH_VALIDITY_SECS);
            assert_eq!(InteropManager::verify_authorization(&env, &sid, &at_max), Ok(()));
        });
    }

    #[test]
    fn test_cross_settlement_replay_rejected() {
        let (env, id, sid) = auth::setup();
        let other = BytesN::from_array(&env, &[0xCD; 32]);
        let signed = auth::sign(&env, &id, &auth::key(4), &sid, 1, auth::NOW + 60);
        let res = env.as_contract(&id, || InteropManager::verify_authorization(&env, &other, &signed));
        assert_eq!(res, Err(InteropError::SettlementMismatch));
    }

    #[test]
    #[should_panic]
    fn test_forged_signature_traps() {
        let (env, id, sid) = auth::setup();
        let mut signed = auth::sign(&env, &id, &auth::key(5), &sid, 1, auth::NOW + 60);
        // Tamper with the authorized nonce after signing.
        signed.auth.nonce = 2;
        env.as_contract(&id, || {
            let _ = InteropManager::verify_authorization(&env, &sid, &signed);
        });
    }

    #[test]
    #[should_panic]
    fn test_cross_deployment_replay_traps() {
        let (env, id, sid) = auth::setup();
        let other_deployment = env.register(InteropHarness, ());
        // Signed for `id`, submitted to `other_deployment`.
        let signed = auth::sign(&env, &id, &auth::key(6), &sid, 1, auth::NOW + 60);
        env.as_contract(&other_deployment, || {
            let _ = InteropManager::verify_authorization(&env, &sid, &signed);
        });
    }

    #[test]
    fn test_authorization_digest_deterministic_and_bound() {
        let (env, id, sid) = auth::setup();
        let base = SettlementAuthorization {
            settlement_id: sid.clone(),
            signer: auth::pubkey(&env, &auth::key(7)),
            nonce: 1,
            expires_at: 10,
        };
        let d = InteropManager::authorization_digest(&env, &id, &base);
        assert_eq!(d, InteropManager::authorization_digest(&env, &id, &base));

        let mut v = base.clone();
        v.nonce = 2;
        assert_ne!(d, InteropManager::authorization_digest(&env, &id, &v));
        let mut v = base.clone();
        v.expires_at = 11;
        assert_ne!(d, InteropManager::authorization_digest(&env, &id, &v));
        let mut v = base.clone();
        v.settlement_id = BytesN::from_array(&env, &[0; 32]);
        assert_ne!(d, InteropManager::authorization_digest(&env, &id, &v));
        let other = env.register(InteropHarness, ());
        assert_ne!(d, InteropManager::authorization_digest(&env, &other, &base));
    }

    #[test]
    fn test_quorum_happy_path_and_atomic_rejection() {
        let (env, id, sid) = auth::setup();
        let keys: Vec<_> = (10u8..14).map(auth::key).collect();
        let validators = soroban_sdk::Vec::from_iter(&env, keys.iter().map(|k| auth::pubkey(&env, k)));

        env.as_contract(&id, || {
            // 2 of 4 signatures but threshold 3 → rejected, no nonce consumed.
            let mut atts = soroban_sdk::Vec::new(&env);
            atts.push_back(auth::sign(&env, &id, &keys[0], &sid, 1, auth::NOW + 60));
            atts.push_back(auth::sign(&env, &id, &keys[1], &sid, 1, auth::NOW + 60));
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 3, &atts),
                Err(InteropError::QuorumNotReached)
            );
            assert_eq!(InteropManager::last_nonce(&env, &validators.get(0).unwrap()), 0);

            // Third signature reaches the quorum.
            atts.push_back(auth::sign(&env, &id, &keys[2], &sid, 1, auth::NOW + 60));
            assert_eq!(InteropManager::verify_quorum(&env, &sid, &validators, 3, &atts), Ok(3));
            // Whole bundle replayed.
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 3, &atts),
                Err(InteropError::NonceReplayed)
            );
        });
    }

    #[test]
    fn test_quorum_structural_rejections() {
        let (env, id, sid) = auth::setup();
        let keys: Vec<_> = (20u8..23).map(auth::key).collect();
        let validators = soroban_sdk::Vec::from_iter(&env, keys.iter().map(|k| auth::pubkey(&env, k)));
        let outsider = auth::key(99);

        env.as_contract(&id, || {
            let one = |k: &ed25519_dalek::SigningKey| auth::sign(&env, &id, k, &sid, 1, auth::NOW + 60);
            let empty = soroban_sdk::Vec::new(&env);

            // Threshold must be a strict majority of 3: only 2 or 3.
            for bad in [0, 1, 4] {
                assert_eq!(
                    InteropManager::verify_quorum(&env, &sid, &validators, bad, &empty),
                    Err(InteropError::InvalidQuorum)
                );
            }

            // Same validator counted twice.
            let dup = soroban_sdk::Vec::from_array(&env, [one(&keys[0]), one(&keys[0])]);
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 2, &dup),
                Err(InteropError::DuplicateAttestation)
            );

            // Duplicate entry in the validator set itself.
            let bad_set = soroban_sdk::Vec::from_array(
                &env,
                [auth::pubkey(&env, &keys[0]), auth::pubkey(&env, &keys[0])],
            );
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &bad_set, 2, &empty),
                Err(InteropError::InvalidValidatorSet)
            );
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &soroban_sdk::Vec::new(&env), 1, &empty),
                Err(InteropError::InvalidValidatorSet)
            );

            let unknown = soroban_sdk::Vec::from_array(&env, [one(&keys[0]), one(&outsider)]);
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 2, &unknown),
                Err(InteropError::NotAValidator)
            );

            let other_sid = BytesN::from_array(&env, &[1; 32]);
            let wrong = soroban_sdk::Vec::from_array(
                &env,
                [auth::sign(&env, &id, &keys[0], &other_sid, 1, auth::NOW + 60)],
            );
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 2, &wrong),
                Err(InteropError::SettlementMismatch)
            );

            let mut big = soroban_sdk::Vec::new(&env);
            for i in 0..=MAX_VALIDATORS {
                big.push_back(BytesN::from_array(&env, &[i as u8; 32]));
            }
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &big, 20, &empty),
                Err(InteropError::InvalidValidatorSet)
            );
            let flood = soroban_sdk::Vec::from_iter(&env, (0..=MAX_VALIDATORS).map(|_| one(&keys[0])));
            assert_eq!(
                InteropManager::verify_quorum(&env, &sid, &validators, 2, &flood),
                Err(InteropError::TooManyAttestations)
            );
        });
        // None of the rejected calls consumed a nonce.
        env.as_contract(&id, || {
            for k in &keys {
                assert_eq!(InteropManager::last_nonce(&env, &auth::pubkey(&env, k)), 0);
            }
        });
    }

    #[test]
    fn test_error_codes_are_stable_and_symbols_unique() {
        let all = [
            InteropError::StaleRate,
            InteropError::ZeroRate,
            InteropError::ZeroPrice,
            InteropError::BidExceedsAsk,
            InteropError::ZeroAmount,
            InteropError::ConversionOverflow,
            InteropError::DivisionByZero,
            InteropError::FeeBpsTooHigh,
            InteropError::FeeOverflow,
            InteropError::ZeroSettlementAmount,
            InteropError::ZeroSettlementRate,
            InteropError::PathTooShort,
            InteropError::PathTooLong,
            InteropError::RatesTooLong,
            InteropError::PathLengthMismatch,
            InteropError::SamePilot,
            InteropError::InvalidPilot,
            InteropError::SpreadOverflow,
            InteropError::NotAValidator,
            InteropError::DuplicateAttestation,
            InteropError::Equivocation,
            InteropError::SettlementNotFound,
            InteropError::SettlementAlreadyExists,
            InteropError::InvalidStatusTransition,
            InteropError::InvalidQuorum,
            InteropError::InvalidValidatorSet,
            InteropError::NotAwaitingAttestation,
            InteropError::InvalidStatus,
            InteropError::ValidatorsNotConfigured,
            InteropError::AuthorizationExpired,
            InteropError::AuthorizationValidityTooLong,
            InteropError::NonceReplayed,
            InteropError::QuorumNotReached,
            InteropError::SettlementMismatch,
            InteropError::TooManyAttestations,
        ];
        for (i, e) in all.iter().enumerate() {
            assert_eq!(e.code(), i as u32 + 1, "code drift for {e:?}");
            for other in &all[i + 1..] {
                assert_ne!(e.as_symbol(), other.as_symbol(), "{e:?} vs {other:?}");
            }
        }
    }

    #[test]
    fn test_status_code_matches_from_u32() {
        for code in 0..=5u32 {
            assert_eq!(SettlementStatus::from_u32(code).unwrap().code(), code);
        }
    }

    #[test]
    fn test_status_change_event_is_structured() {
        let f = Fixture::new(3, 2);
        f.env.ledger().set_timestamp(4_242);
        let id = f.open_settlement(1);
        let expected = SettlementStatusChanged {
            status: SettlementStatus::AwaitingDestination.as_symbol(&f.env),
            settlement_id: id,
            from_status: SettlementStatus::ConfirmedSource.code(),
            to_status: SettlementStatus::AwaitingDestination.code(),
            timestamp: 4_242,
        };
        assert_eq!(last_event(&f.env), expected.to_xdr(&f.env, &f.contract));
    }

    #[test]
    fn test_attestation_events_are_structured() {
        let f = Fixture::new(3, 2);
        let id = f.open_settlement(2);
        let h = f.hash(0xAA);

        assert_eq!(f.attest(0, &id, &h), Ok(AttestationOutcome::Recorded));
        let recorded = AttestationRecorded {
            settlement_id: id.clone(),
            validator: f.validators[0].clone(),
            confirmation_hash: h.clone(),
            votes: 1,
        };
        assert_eq!(last_event(&f.env), recorded.to_xdr(&f.env, &f.contract));

        assert_eq!(f.attest(1, &id, &h), Ok(AttestationOutcome::Finalized));
        // `events().all()` reflects the latest invocation only; read it first.
        let emitted = last_event(&f.env);
        let finalized = SettlementFinalized {
            settlement_id: id.clone(),
            status: SettlementStatus::Completed.code(),
            finalized_hash: Some(h),
            digest: f.state(&id).digest,
        };
        assert_eq!(emitted, finalized.to_xdr(&f.env, &f.contract));
    }

    #[test]
    fn test_observe_passes_ok_through_silently() {
        let f = Fixture::new(1, 1);
        let before = f.env.events().all().events().len();
        let out = f.run(|env| {
            InteropManager::observe(env, symbol_short!("convert"), InteropManager::convert_amount(10, RATE_SCALE))
        });
        assert_eq!(out, Ok(10));
        assert_eq!(f.env.events().all().events().len(), before);
    }

    #[test]
    fn test_observe_emits_failure_and_supports_question_mark() {
        fn pipeline(env: &Env, amount: u128, rate: u128) -> Result<u128, InteropError> {
            let converted = InteropManager::observe(
                env,
                symbol_short!("convert"),
                InteropManager::convert_amount(amount, rate),
            )?;
            InteropManager::observe(env, symbol_short!("fee"), InteropManager::compute_settlement_fee(converted, 25))
        }
        let f = Fixture::new(1, 1);
        f.env.ledger().set_timestamp(77);
        let res = f.run(|env| pipeline(env, 0, RATE_SCALE));
        assert_eq!(res, Err(InteropError::ZeroAmount));

        let failures: Vec<_> = f
            .env
            .events()
            .all()
            .events()
            .iter()
            .filter(|e| match &e.body {
                ContractEventBody::V0(b) => {
                    matches!(b.topics.first(), Some(ScVal::Symbol(s)) if s.0.as_slice() == b"cbdc_err")
                }
            })
            .cloned()
            .collect();
        // Propagation stopped at the first error: exactly one failure event.
        let expected = InteropFailure {
            operation: symbol_short!("convert"),
            error_code: InteropError::ZeroAmount.code(),
            error: InteropError::ZeroAmount.as_symbol(),
            timestamp: 77,
        };
        assert_eq!(failures, std::vec![expected.to_xdr(&f.env, &f.contract)]);
    }
}
