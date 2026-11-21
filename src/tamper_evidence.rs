//! Tamper-Evidence Verification System for Audit Trails
//!
//! Implements cryptographic chain verification to detect tampering,
//! prove event immutability, and validate audit trail integrity.
//!
//! # Invariants
//!
//! * **Single host.** Every Soroban object produced here is allocated on the
//!   caller's [`Env`]. Mixing objects from different hosts traps with
//!   `mis-tagged object reference`, so no function constructs its own `Env`.
//! * **Fail closed.** Malformed input (empty chains, out-of-range indices,
//!   unsupported algorithms, invalid configuration) is reported as a typed
//!   [`TamperEvidenceError`] instead of being coerced into a "valid" result.
//! * **Recompute, never trust.** Link validity is derived from the hashes
//!   themselves; caller-supplied `valid` flags are outputs, not inputs.
//! * **Observable.** Every verification outcome is published as a structured
//!   contract event so off-chain monitors can alert on tampering without
//!   replaying the computation.
//! * **Replay-resistant signatures.** A [`SignedWitness`] is bound by its
//!   signed digest to a domain tag, the network id, the verifying contract,
//!   the signer, a per-signer strictly increasing nonce and an expiry ledger.
//!   It therefore cannot be replayed on another network, against another
//!   contract, twice on the same contract, or after it expires.
//! * **No equivocation.** Once a root has been accepted for an event index,
//!   a different root for the same index is rejected, whoever signed it.
//! * **Second-preimage-safe Merkle proofs.** Leaves and interior nodes are
//!   hashed under distinct prefixes (RFC 6962 style) and archive proofs are
//!   verified by reconstructing the root, never by trusting a flag.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracterror, contractevent, contracttype, Bytes, BytesN, Env, Vec};

/// Identifier of the SHA-256 hash algorithm in [`TamperEvidenceConfig`].
pub const HASH_ALGORITHM_SHA256: u32 = 0;

/// Domain-separation tag prefixed to every signed witness digest.
pub const WITNESS_DOMAIN_TAG: &[u8] = b"DATL/tamper-evidence/witness/v1";
/// Domain-separation tag for the public witness commitment hash.
pub const COMMITMENT_DOMAIN_TAG: &[u8] = b"DATL/tamper-evidence/commitment/v1";
/// Upper bound on how far ahead a witness may set its expiry, bounding the
/// window in which a leaked-but-unsubmitted witness stays usable
/// (~1 day at 5s ledgers).
pub const MAX_WITNESS_VALIDITY_LEDGERS: u32 = 17_280;
/// Signatures from one validator set are deduplicated within each 10,000-ledger epoch.
pub const WITNESS_EPOCH_LENGTH_LEDGERS: u32 = 10_000;
/// Bounds roster fingerprinting and the temporary canonical-sort buffer.
pub const MAX_VALIDATORS_PER_SET: usize = 64;
/// A `u32` leaf index can address at most 2^32 leaves.
pub const MAX_MERKLE_DEPTH: u32 = 32;
/// Maximum number of links verified in one call. Bounds worst-case CPU and
/// keeps a single invocation comfortably inside Soroban's per-transaction
/// instruction budget.
pub const MAX_CHAIN_BATCH: u32 = 2_048;
/// Maximum number of compromised indices reported individually. The
/// mismatch *count* is always exact; past this cap only the first indices
/// are listed and [`ChainVerification::indices_truncated`] is set. Collecting
/// them in a fixed stack buffer avoids the O(n²) copying of growing a host
/// `Vec` one `push_back` at a time.
pub const MAX_REPORTED_COMPROMISED: usize = 64;

const MERKLE_LEAF_PREFIX: u8 = 0x00;
const MERKLE_NODE_PREFIX: u8 = 0x01;
const MERKLE_EXPIRY_PREFIX: u8 = 0x02;
const VALIDATOR_SET_DOMAIN_TAG: &[u8] = b"DATL/tamper-evidence/validator-set/v1";

/// Typed failures surfaced by the tamper-evidence subsystem.
///
/// Discriminants are part of the public ABI: never renumber an existing
/// variant, only append new ones.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TamperEvidenceError {
    /// A chain verification was requested over zero links.
    EmptyChain = 1,
    /// An event index does not fall inside the chain it is checked against.
    IndexOutOfRange = 2,
    /// `immutability_threshold` must be at least 1.
    InvalidThreshold = 3,
    /// The configured hash algorithm is not supported by the host.
    UnsupportedHashAlgorithm = 4,
    /// Link indices are not strictly sequential (gap, reorder or duplicate),
    /// which indicates deleted or injected events.
    NonSequentialIndex = 5,
    /// An archive proof was built without any Merkle siblings.
    EmptyMerklePath = 6,
    /// The witness signer is not in the trusted validator set.
    UnauthorizedSigner = 7,
    /// The current ledger is past the witness's `valid_until_ledger`.
    WitnessExpired = 8,
    /// `valid_until_ledger` exceeds [`MAX_WITNESS_VALIDITY_LEDGERS`] from now.
    ExpiryTooFar = 9,
    /// The nonce was already used (or is lower than one already used) by this signer.
    ReplayedNonce = 10,
    /// The witness's `commitment_hash` does not match its contents.
    CommitmentMismatch = 11,
    /// The witness claims a timestamp later than the current ledger.
    FutureTimestamp = 12,
    /// A different root was already accepted for this event index.
    Equivocation = 13,
    /// The Merkle path is deeper than [`MAX_MERKLE_DEPTH`].
    MerklePathTooLong = 14,
    /// A batch exceeds [`MAX_CHAIN_BATCH`] links.
    BatchTooLarge = 15,
    /// Parallel input vectors have different lengths.
    LengthMismatch = 16,
    /// This validator already signed for this validator set in the current epoch.
    RedundantSignature = 17,
    /// The record's committed expiry ledger has not been reached.
    RecordNotExpired = 18,
    /// The proof root is not pinned by an accepted validator witness.
    UnanchoredProofRoot = 19,
    /// A different proof is already committed for this record index.
    ProofCommitmentConflict = 20,
    /// The validator roster exceeds [`MAX_VALIDATORS_PER_SET`].
    ValidatorSetTooLarge = 21,
}

/// Published once per chain verification.
#[contractevent(topics = ["tamper_ev", "verified"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainVerifiedEvent {
    #[topic]
    pub valid: bool,
    pub events_verified: u32,
    pub mismatches: u32,
    pub integrity_score: u32,
}

/// Published when a verification finds at least one compromised link.
#[contractevent(topics = ["tamper_ev", "detected"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TamperDetectedEvent {
    #[topic]
    pub first_compromised_index: u32,
    pub mismatches: u32,
    pub events_verified: u32,
}

/// Published when an operation is rejected, so monitors see failed attempts
/// and not only successful verifications.
#[contractevent(topics = ["tamper_ev", "rejected"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRejectedEvent {
    #[topic]
    pub error_code: u32,
}

/// Published when a signed witness is accepted.
#[contractevent(topics = ["tamper_ev", "witness"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessAcceptedEvent {
    #[topic]
    pub event_index: u32,
    pub signer: BytesN<32>,
    pub nonce: u64,
    pub root: BytesN<32>,
}

/// Published when a validator signs a root that conflicts with an accepted one.
/// Carries both roots so the equivocation can be slashed or escalated off-chain.
#[contractevent(topics = ["tamper_ev", "equivocation"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquivocationDetectedEvent {
    #[topic]
    pub event_index: u32,
    pub signer: BytesN<32>,
    pub accepted_root: BytesN<32>,
    pub conflicting_root: BytesN<32>,
}

/// Result of a chain verification operation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainVerification {
    /// Whether the entire chain from event to root is valid
    pub valid: bool,
    /// Number of events verified in the chain
    pub events_verified: u32,
    /// Number of hash mismatches found
    pub mismatches: u32,
    /// Integrity score (0 = invalid, 100 = perfect)
    pub integrity_score: u32, // 0-100
    /// List of event indices with hash mismatches
    pub compromised_indices: Vec<u32>,
    /// Whether `compromised_indices` was capped at [`MAX_REPORTED_COMPROMISED`]
    pub indices_truncated: bool,
}

/// Single chain link verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainLink {
    /// Index in the event sequence
    pub index: u32,
    /// Hash of this event
    pub hash: BytesN<32>,
    /// Hash of the previous event, as recorded in this event
    pub prev_hash: BytesN<32>,
    /// Expected previous hash (the actual hash of the preceding event)
    pub expected_prev_hash: BytesN<32>,
    /// Whether this link is valid. Ignored on input: validity is always
    /// recomputed from the hashes by [`TamperEvidenceHelper::verify_chain_continuity`].
    pub valid: bool,
}

/// Proof that an event has not been modified
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutabilityProof {
    /// Event index being proven immutable
    pub event_index: u32,
    /// Hash of the event
    pub event_hash: BytesN<32>,
    /// Number of subsequent events that (transitively) commit to this event's hash
    pub references: u32,
    /// Length of the hash chain from this event to the most recent event, inclusive
    pub chain_length: u32,
    /// Whether immutability is proven (by subsequent references)
    pub immutable: bool,
}

/// Archive proof for events older than retention period
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProof {
    /// Hash of the archived event
    pub event_hash: BytesN<32>,
    /// Root hash of the archive merkle tree
    pub archive_root: BytesN<32>,
    /// Position of the event among the archive's leaves; bit `i` selects
    /// whether the node at depth `i` is a right (1) or left (0) child.
    pub leaf_index: u32,
    /// Sibling hashes from the leaf up to (excluding) the archive root
    pub merkle_path: Vec<BytesN<32>>,
    /// Whether the proof is valid
    pub valid: bool,
}

/// Merkle inclusion proof that a record's expiration was committed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpirationProof {
    /// Sequential event index being proven expired.
    pub event_index: u32,
    /// Hash of the expired event.
    pub event_hash: BytesN<32>,
    /// Ledger sequence at which the expiration was recorded.
    pub expired_at_ledger: u32,
    /// Validator-accepted root of the expiration commitment tree.
    pub expiry_root: BytesN<32>,
    /// Position of this commitment among the tree's leaves.
    pub leaf_index: u32,
    /// Sibling hashes from the leaf up to (excluding) the root.
    pub merkle_path: Vec<BytesN<32>>,
    /// Whether the proof was valid when constructed. Ignored during verification.
    pub valid: bool,
}

/// Witness for hash chain commitment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainWitness {
    /// Event index this witness covers
    pub event_index: u32,
    /// Merkle root of events up to this point
    pub root_at_index: BytesN<32>,
    /// Timestamp of this witness
    pub timestamp: u64,
    /// Public commitment of this root (e.g., in blockchain)
    pub commitment_hash: BytesN<32>,
}

/// A [`ChainWitness`] co-signed by a validator, with replay protection.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedWitness {
    pub witness: ChainWitness,
    /// Ed25519 public key of the signing validator
    pub signer: BytesN<32>,
    /// Per-signer nonce; must be strictly greater than the signer's last accepted nonce
    pub nonce: u64,
    /// Last ledger sequence at which this witness may be accepted
    pub valid_until_ledger: u32,
    /// Ed25519 signature over [`TamperEvidenceHelper::witness_digest`]
    pub signature: BytesN<64>,
}

/// Persistent storage keys owned by the tamper-evidence subsystem.
///
/// Persistent (not temporary) storage is deliberate: a temporary entry that
/// expired would read back as absent and silently re-enable replays, whereas
/// an archived persistent entry must be restored before it can be read.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TamperEvidenceKey {
    /// Highest nonce accepted from a signer
    SignerNonce(BytesN<32>),
    /// Root accepted for an event index
    AcceptedRoot(u32),
    /// Validator/set/epoch tuple already used to accept a witness.
    ValidatorEpochSigner(BytesN<32>, BytesN<32>, u32),
    /// Proof committed for an archived event index.
    CommittedArchiveProof(u32),
    /// Persistent proof commitment for an expired event index.
    CommittedExpiryProof(u32),
}

/// Configuration for tamper-evidence validation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TamperEvidenceConfig {
    /// Minimum chain length to consider immutable
    pub immutability_threshold: u32,
    /// Whether to verify all chains or sample
    pub verify_all: bool,
    /// Algorithm for hash verification (only [`HASH_ALGORITHM_SHA256`] is supported)
    pub hash_algorithm: u32,
    /// Enable archive root verification
    pub verify_archives: bool,
}

impl TamperEvidenceConfig {
    /// Validate the configuration, failing closed on unsupported settings.
    pub fn validate(&self) -> Result<(), TamperEvidenceError> {
        if self.immutability_threshold == 0 {
            return Err(TamperEvidenceError::InvalidThreshold);
        }
        if self.hash_algorithm != HASH_ALGORITHM_SHA256 {
            return Err(TamperEvidenceError::UnsupportedHashAlgorithm);
        }
        Ok(())
    }
}

/// Allocation-free accumulator for compromised link indices.
struct Tally {
    mismatches: u32,
    indices: [u32; MAX_REPORTED_COMPROMISED],
    reported: usize,
}

impl Tally {
    fn new() -> Self {
        Self {
            mismatches: 0,
            indices: [0; MAX_REPORTED_COMPROMISED],
            reported: 0,
        }
    }

    fn flag(&mut self, index: u32) {
        self.mismatches += 1;
        if self.reported < MAX_REPORTED_COMPROMISED {
            self.indices[self.reported] = index;
            self.reported += 1;
        }
    }

    /// Build the result with a single host `Vec` allocation and publish events.
    fn finish(self, env: &Env, events_verified: u32) -> ChainVerification {
        let mismatches = self.mismatches;
        let integrity_score = TamperEvidenceHelper::calculate_integrity_score(events_verified, mismatches);
        let valid = mismatches == 0;

        ChainVerifiedEvent {
            valid,
            events_verified,
            mismatches,
            integrity_score,
        }
        .publish(env);
        if !valid {
            TamperDetectedEvent {
                first_compromised_index: self.indices[0],
                mismatches,
                events_verified,
            }
            .publish(env);
        }

        ChainVerification {
            valid,
            events_verified,
            mismatches,
            integrity_score,
            compromised_indices: Vec::from_slice(env, &self.indices[..self.reported]),
            indices_truncated: mismatches as usize > self.reported,
        }
    }
}

/// Helper functions for tamper-evidence verification
pub struct TamperEvidenceHelper;

impl TamperEvidenceHelper {
    /// Publish a rejection event and hand the error back for `?`/`map_err` chaining.
    pub fn reject(env: &Env, error: TamperEvidenceError) -> TamperEvidenceError {
        VerificationRejectedEvent {
            error_code: error as u32,
        }
        .publish(env);
        error
    }

    /// Verify a single event's hash is correct
    pub fn verify_event_hash(event_hash: &BytesN<32>, expected_hash: &BytesN<32>) -> bool {
        event_hash == expected_hash
    }

    /// Verify the chain link between two consecutive events: the later
    /// event must commit to the exact hash of the earlier one.
    pub fn verify_chain_link(current_event_hash: &BytesN<32>, next_prev_hash: &BytesN<32>) -> bool {
        next_prev_hash == current_event_hash
    }

    /// Calculate the integrity score for a chain
    pub fn calculate_integrity_score(total_events: u32, mismatches: u32) -> u32 {
        if total_events == 0 {
            100
        } else {
            let valid_events = total_events.saturating_sub(mismatches);
            ((valid_events as u64 * 100) / (total_events as u64)) as u32
        }
    }

    /// Verify event immutability based on subsequent references
    pub fn verify_immutability(
        env: &Env,
        event_hash: &BytesN<32>,
        event_index: u32,
        total_events: u32,
        config: &TamperEvidenceConfig,
    ) -> Result<ImmutabilityProof, TamperEvidenceError> {
        config.validate().map_err(|e| Self::reject(env, e))?;
        if event_index >= total_events {
            return Err(Self::reject(env, TamperEvidenceError::IndexOutOfRange));
        }

        let chain_length = total_events - event_index;
        Ok(ImmutabilityProof {
            event_index,
            event_hash: event_hash.clone(),
            references: chain_length - 1,
            chain_length,
            immutable: chain_length >= config.immutability_threshold,
        })
    }

    /// Deterministic public commitment over a witness's contents:
    /// `sha256(COMMITMENT_DOMAIN_TAG || index_be || root || timestamp_be)`.
    pub fn compute_witness_commitment(
        env: &Env,
        event_index: u32,
        root_at_index: &BytesN<32>,
        timestamp: u64,
    ) -> BytesN<32> {
        let mut buf = Bytes::from_slice(env, COMMITMENT_DOMAIN_TAG);
        buf.extend_from_array(&event_index.to_be_bytes());
        buf.append(&root_at_index.clone().into());
        buf.extend_from_array(&timestamp.to_be_bytes());
        env.crypto().sha256(&buf).into()
    }

    /// Create a chain witness commitment
    pub fn create_chain_witness(env: &Env, event_index: u32, root_at_index: BytesN<32>) -> ChainWitness {
        let timestamp = env.ledger().timestamp();
        let commitment_hash = Self::compute_witness_commitment(env, event_index, &root_at_index, timestamp);
        ChainWitness {
            event_index,
            root_at_index,
            timestamp,
            commitment_hash,
        }
    }

    /// The exact 32-byte message a validator signs for a [`SignedWitness`].
    ///
    /// `sha256(WITNESS_DOMAIN_TAG || network_id || contract_address_xdr ||
    /// signer || nonce_be || valid_until_be || event_index_be || root ||
    /// timestamp_be || commitment_hash)`
    ///
    /// Binding the network id and the verifying contract address prevents
    /// cross-network and cross-contract replay; the nonce and expiry prevent
    /// replay on the same contract.
    pub fn witness_digest(
        env: &Env,
        witness: &ChainWitness,
        signer: &BytesN<32>,
        nonce: u64,
        valid_until_ledger: u32,
    ) -> BytesN<32> {
        let mut buf = Bytes::from_slice(env, WITNESS_DOMAIN_TAG);
        buf.append(&env.ledger().network_id().into());
        buf.append(&env.current_contract_address().to_xdr(env));
        buf.append(&signer.clone().into());
        buf.extend_from_array(&nonce.to_be_bytes());
        buf.extend_from_array(&valid_until_ledger.to_be_bytes());
        buf.extend_from_array(&witness.event_index.to_be_bytes());
        buf.append(&witness.root_at_index.clone().into());
        buf.extend_from_array(&witness.timestamp.to_be_bytes());
        buf.append(&witness.commitment_hash.clone().into());
        env.crypto().sha256(&buf).into()
    }

    /// Highest nonce accepted from `signer`, if any.
    pub fn last_nonce(env: &Env, signer: &BytesN<32>) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&TamperEvidenceKey::SignerNonce(signer.clone()))
    }

    /// Root accepted for `event_index`, if any.
    pub fn accepted_root(env: &Env, event_index: u32) -> Option<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&TamperEvidenceKey::AcceptedRoot(event_index))
    }

    /// Fingerprint a validator set independently of caller-provided ordering.
    pub fn validator_set_fingerprint(
        env: &Env,
        trusted_signers: &Vec<BytesN<32>>,
    ) -> Result<BytesN<32>, TamperEvidenceError> {
        if trusted_signers.len() as usize > MAX_VALIDATORS_PER_SET {
            return Err(TamperEvidenceError::ValidatorSetTooLarge);
        }
        let mut keys = [[0u8; 32]; MAX_VALIDATORS_PER_SET];
        let mut len = 0usize;
        for signer in trusted_signers.iter() {
            keys[len] = signer.to_array();
            len += 1;
        }
        keys[..len].sort_unstable();
        let unique_len = (0..len).filter(|i| *i == 0 || keys[*i] != keys[*i - 1]).count();

        let mut buf = Bytes::from_slice(env, VALIDATOR_SET_DOMAIN_TAG);
        buf.extend_from_array(&(unique_len as u32).to_be_bytes());
        for i in 0..len {
            if i == 0 || keys[i] != keys[i - 1] {
                buf.extend_from_array(&keys[i]);
            }
        }
        Ok(env.crypto().sha256(&buf).into())
    }

    /// Verify a validator-signed witness and record it.
    ///
    /// All cheap, stateful checks run before the signature check so that
    /// replayed, expired or unauthorized submissions are rejected with a
    /// typed error without paying for ed25519 verification. Note that the
    /// host *traps* (rather than returning an error) on an invalid signature;
    /// the invocation aborts and no state from this call is persisted.
    ///
    /// On success the signer's nonce is consumed and the root is pinned for
    /// `event_index`. Re-submitting the same root for an index (e.g. a second
    /// validator co-signing) is accepted; a conflicting root is
    /// [`TamperEvidenceError::Equivocation`].
    pub fn accept_signed_witness(
        env: &Env,
        signed: &SignedWitness,
        trusted_signers: &Vec<BytesN<32>>,
    ) -> Result<(), TamperEvidenceError> {
        let fail = |e| Err(Self::reject(env, e));
        let w = &signed.witness;
        let ledger = env.ledger();

        if !trusted_signers.contains(&signed.signer) {
            return fail(TamperEvidenceError::UnauthorizedSigner);
        }
        if ledger.sequence() > signed.valid_until_ledger {
            return fail(TamperEvidenceError::WitnessExpired);
        }
        if signed.valid_until_ledger - ledger.sequence() > MAX_WITNESS_VALIDITY_LEDGERS {
            return fail(TamperEvidenceError::ExpiryTooFar);
        }
        if w.timestamp > ledger.timestamp() {
            return fail(TamperEvidenceError::FutureTimestamp);
        }
        if matches!(Self::last_nonce(env, &signed.signer), Some(last) if signed.nonce <= last) {
            return fail(TamperEvidenceError::ReplayedNonce);
        }
        let commitment = Self::compute_witness_commitment(env, w.event_index, &w.root_at_index, w.timestamp);
        if commitment != w.commitment_hash {
            return fail(TamperEvidenceError::CommitmentMismatch);
        }
        let epoch = ledger.sequence() / WITNESS_EPOCH_LENGTH_LEDGERS;
        let set_fingerprint = match Self::validator_set_fingerprint(env, trusted_signers) {
            Ok(fingerprint) => fingerprint,
            Err(error) => return fail(error),
        };
        let epoch_key = TamperEvidenceKey::ValidatorEpochSigner(set_fingerprint, signed.signer.clone(), epoch);
        let already_signed_this_epoch = env.storage().persistent().has(&epoch_key);
        if already_signed_this_epoch
            && !matches!(Self::accepted_root(env, w.event_index), Some(root) if root != w.root_at_index)
        {
            return fail(TamperEvidenceError::RedundantSignature);
        }

        let digest = Self::witness_digest(env, w, &signed.signer, signed.nonce, signed.valid_until_ledger);
        env.crypto()
            .ed25519_verify(&signed.signer, &digest.into(), &signed.signature);

        // Equivocation is only meaningful for an authentic signature, so it is
        // checked after verification: an unsigned conflicting root is noise,
        // a signed one is evidence of a Byzantine validator.
        if let Some(accepted) = Self::accepted_root(env, w.event_index) {
            if accepted != w.root_at_index {
                EquivocationDetectedEvent {
                    event_index: w.event_index,
                    signer: signed.signer.clone(),
                    accepted_root: accepted,
                    conflicting_root: w.root_at_index.clone(),
                }
                .publish(env);
                return fail(TamperEvidenceError::Equivocation);
            }
        }
        if already_signed_this_epoch {
            return fail(TamperEvidenceError::RedundantSignature);
        }

        let storage = env.storage().persistent();
        storage.set(&TamperEvidenceKey::SignerNonce(signed.signer.clone()), &signed.nonce);
        storage.set(&TamperEvidenceKey::AcceptedRoot(w.event_index), &w.root_at_index);
        storage.set(&epoch_key, &true);

        WitnessAcceptedEvent {
            event_index: w.event_index,
            signer: signed.signer.clone(),
            nonce: signed.nonce,
            root: w.root_at_index.clone(),
        }
        .publish(env);
        Ok(())
    }

    /// Reject empty or oversized batches before doing any per-link work.
    fn check_batch(env: &Env, len: u32) -> Result<(), TamperEvidenceError> {
        if len == 0 {
            return Err(Self::reject(env, TamperEvidenceError::EmptyChain));
        }
        if len > MAX_CHAIN_BATCH {
            return Err(Self::reject(env, TamperEvidenceError::BatchTooLarge));
        }
        Ok(())
    }

    /// Verify a chain of events by checking hash continuity.
    ///
    /// A link is compromised when its recorded `prev_hash` differs from the
    /// supplied `expected_prev_hash`, or (for every link after the first)
    /// from the actual hash of the preceding link. Indices must be strictly
    /// sequential; a gap or reorder is a hard error because it means events
    /// were removed or injected rather than modified.
    ///
    /// For large batches prefer [`Self::verify_hash_chain`]: decoding each
    /// `ChainLink` struct dominates the cost of this entry point.
    pub fn verify_chain_continuity(
        env: &Env,
        links: &Vec<ChainLink>,
    ) -> Result<ChainVerification, TamperEvidenceError> {
        let len = links.len();
        Self::check_batch(env, len)?;

        let mut tally = Tally::new();
        // Only the previous link's index and hash are needed; keeping just
        // those avoids holding a whole decoded struct across iterations.
        let mut prev: Option<(u32, BytesN<32>)> = None;

        for link in links.iter() {
            let ok = match &prev {
                None => link.prev_hash == link.expected_prev_hash,
                Some((prev_index, prev_hash)) => {
                    if prev_index.checked_add(1) != Some(link.index) {
                        return Err(Self::reject(env, TamperEvidenceError::NonSequentialIndex));
                    }
                    // Both checks always run on honest input; short-circuiting
                    // only saves work on links that are already compromised.
                    Self::verify_chain_link(prev_hash, &link.prev_hash) && link.prev_hash == link.expected_prev_hash
                }
            };
            if !ok {
                tally.flag(link.index);
            }
            prev = Some((link.index, link.hash));
        }

        Ok(tally.finish(env, len))
    }

    /// Compact, large-batch chain verification.
    ///
    /// Link `k` (absolute index `first_index + k`) records `event_hashes[k]`
    /// and back-pointer `prev_hashes[k]`. It is compromised when that
    /// back-pointer differs from the hash of its predecessor, where the
    /// predecessor of the first link is the externally trusted `anchor`
    /// (e.g. a root pinned by a signed witness).
    ///
    /// Taking two flat `Vec<BytesN<32>>` instead of `Vec<ChainLink>` skips
    /// per-link struct decoding and does one host comparison per link, which
    /// cuts metered CPU by ~45% on large batches (1.84M -> 1.02M instructions
    /// for 1,024 links).
    pub fn verify_hash_chain(
        env: &Env,
        first_index: u32,
        anchor: &BytesN<32>,
        event_hashes: &Vec<BytesN<32>>,
        prev_hashes: &Vec<BytesN<32>>,
    ) -> Result<ChainVerification, TamperEvidenceError> {
        let len = event_hashes.len();
        Self::check_batch(env, len)?;
        if prev_hashes.len() != len {
            return Err(Self::reject(env, TamperEvidenceError::LengthMismatch));
        }
        if first_index.checked_add(len - 1).is_none() {
            return Err(Self::reject(env, TamperEvidenceError::IndexOutOfRange));
        }

        let mut tally = Tally::new();
        let mut expected_prev = anchor.clone();
        for (k, (hash, prev)) in event_hashes.iter().zip(prev_hashes.iter()).enumerate() {
            if !Self::verify_chain_link(&expected_prev, &prev) {
                tally.flag(first_index + k as u32);
            }
            expected_prev = hash;
        }

        Ok(tally.finish(env, len))
    }

    /// Verify that an event has not been retroactively modified
    /// by checking that its hash still matches all references to it.
    ///
    /// An empty reference set proves nothing and therefore returns `false`.
    pub fn verify_no_retroactive_modification(event_hash: &BytesN<32>, references: &Vec<BytesN<32>>) -> bool {
        !references.is_empty() && references.iter().all(|ref_hash| ref_hash == *event_hash)
    }

    /// Merkle leaf hash: `sha256(0x00 || event_hash)`.
    pub fn merkle_leaf(env: &Env, event_hash: &BytesN<32>) -> BytesN<32> {
        let mut buf = Bytes::from_array(env, &[MERKLE_LEAF_PREFIX]);
        buf.append(&event_hash.clone().into());
        env.crypto().sha256(&buf).into()
    }

    /// Merkle interior node hash: `sha256(0x01 || left || right)`.
    pub fn merkle_node(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
        let mut buf = Bytes::from_array(env, &[MERKLE_NODE_PREFIX]);
        buf.append(&left.clone().into());
        buf.append(&right.clone().into());
        env.crypto().sha256(&buf).into()
    }

    /// Reconstruct the Merkle root for `event_hash` at `leaf_index` from its sibling path.
    pub fn compute_merkle_root(
        env: &Env,
        event_hash: &BytesN<32>,
        leaf_index: u32,
        merkle_path: &Vec<BytesN<32>>,
    ) -> Result<BytesN<32>, TamperEvidenceError> {
        Self::compute_merkle_root_from_leaf(env, Self::merkle_leaf(env, event_hash), leaf_index, merkle_path)
    }

    /// Create proof that event is immutable by archive. The returned
    /// `valid` flag reflects an actual root reconstruction.
    pub fn create_archive_proof(
        env: &Env,
        event_hash: &BytesN<32>,
        archive_root: &BytesN<32>,
        leaf_index: u32,
        merkle_path: Vec<BytesN<32>>,
    ) -> Result<ArchiveProof, TamperEvidenceError> {
        let root =
            Self::compute_merkle_root(env, event_hash, leaf_index, &merkle_path).map_err(|e| Self::reject(env, e))?;

        Ok(ArchiveProof {
            event_hash: event_hash.clone(),
            archive_root: archive_root.clone(),
            leaf_index,
            merkle_path,
            valid: root == *archive_root,
        })
    }

    /// Verify an archive proof by reconstructing the root. The proof's own
    /// `valid` flag is ignored, since anyone can set it.
    pub fn verify_archive_proof(env: &Env, proof: &ArchiveProof) -> bool {
        matches!(
            Self::compute_merkle_root(env, &proof.event_hash, proof.leaf_index, &proof.merkle_path),
            Ok(root) if root == proof.archive_root
        )
    }

    /// Hash an expiration commitment leaf. Its distinct prefix and explicit
    /// index/deadline prevent archive leaves or altered expiry claims aliasing it.
    pub fn expiration_leaf(
        env: &Env,
        event_index: u32,
        event_hash: &BytesN<32>,
        expired_at_ledger: u32,
    ) -> BytesN<32> {
        let mut buf = Bytes::from_array(env, &[MERKLE_EXPIRY_PREFIX]);
        buf.extend_from_array(&event_index.to_be_bytes());
        buf.append(&event_hash.clone().into());
        buf.extend_from_array(&expired_at_ledger.to_be_bytes());
        env.crypto().sha256(&buf).into()
    }

    /// Reconstruct a Merkle root from an already-hashed leaf.
    fn compute_merkle_root_from_leaf(
        env: &Env,
        mut node: BytesN<32>,
        leaf_index: u32,
        merkle_path: &Vec<BytesN<32>>,
    ) -> Result<BytesN<32>, TamperEvidenceError> {
        let depth = merkle_path.len();
        if depth == 0 {
            return Err(TamperEvidenceError::EmptyMerklePath);
        }
        if depth > MAX_MERKLE_DEPTH {
            return Err(TamperEvidenceError::MerklePathTooLong);
        }
        if depth < MAX_MERKLE_DEPTH && (leaf_index >> depth) != 0 {
            return Err(TamperEvidenceError::IndexOutOfRange);
        }

        for (level, sibling) in merkle_path.iter().enumerate() {
            node = if (leaf_index >> level) & 1 == 0 {
                Self::merkle_node(env, &node, &sibling)
            } else {
                Self::merkle_node(env, &sibling, &node)
            };
        }
        Ok(node)
    }

    /// Build a proof that a committed expiry leaf is included in `expiry_root`.
    pub fn create_expiration_proof(
        env: &Env,
        event_index: u32,
        event_hash: &BytesN<32>,
        expired_at_ledger: u32,
        expiry_root: &BytesN<32>,
        leaf_index: u32,
        merkle_path: Vec<BytesN<32>>,
    ) -> Result<ExpirationProof, TamperEvidenceError> {
        if env.ledger().sequence() < expired_at_ledger {
            return Err(Self::reject(env, TamperEvidenceError::RecordNotExpired));
        }
        let leaf = Self::expiration_leaf(env, event_index, event_hash, expired_at_ledger);
        let root = Self::compute_merkle_root_from_leaf(env, leaf, leaf_index, &merkle_path)
            .map_err(|e| Self::reject(env, e))?;
        Ok(ExpirationProof {
            event_index,
            event_hash: event_hash.clone(),
            expired_at_ledger,
            expiry_root: expiry_root.clone(),
            leaf_index,
            merkle_path,
            valid: root == *expiry_root,
        })
    }

    /// Verify both the Merkle inclusion and that the committed expiry ledger
    /// has arrived. The proof's `valid` flag is never trusted.
    pub fn verify_expiration_proof(env: &Env, proof: &ExpirationProof) -> bool {
        if env.ledger().sequence() < proof.expired_at_ledger {
            return false;
        }
        let leaf = Self::expiration_leaf(env, proof.event_index, &proof.event_hash, proof.expired_at_ledger);
        matches!(
            Self::compute_merkle_root_from_leaf(env, leaf, proof.leaf_index, &proof.merkle_path),
            Ok(root) if root == proof.expiry_root
        )
    }

    /// Persist an archive proof only when its root was accepted by a validator witness.
    pub fn commit_archive_proof(
        env: &Env,
        event_index: u32,
        proof: &ArchiveProof,
    ) -> Result<(), TamperEvidenceError> {
        if !Self::verify_archive_proof(env, proof) {
            return Err(Self::reject(env, TamperEvidenceError::CommitmentMismatch));
        }
        if Self::accepted_root(env, event_index).as_ref() != Some(&proof.archive_root) {
            return Err(Self::reject(env, TamperEvidenceError::UnanchoredProofRoot));
        }
        let key = TamperEvidenceKey::CommittedArchiveProof(event_index);
        if let Some(existing) = env.storage().persistent().get::<_, ArchiveProof>(&key) {
            if &existing != proof {
                return Err(Self::reject(env, TamperEvidenceError::ProofCommitmentConflict));
            }
            return Ok(());
        }
        env.storage().persistent().set(&key, proof);
        Ok(())
    }

    /// Persist an expiry proof only when its root was accepted by a validator witness.
    pub fn commit_expiration_proof(env: &Env, proof: &ExpirationProof) -> Result<(), TamperEvidenceError> {
        if !Self::verify_expiration_proof(env, proof) {
            return Err(Self::reject(env, TamperEvidenceError::CommitmentMismatch));
        }
        if Self::accepted_root(env, proof.event_index).as_ref() != Some(&proof.expiry_root) {
            return Err(Self::reject(env, TamperEvidenceError::UnanchoredProofRoot));
        }
        let key = TamperEvidenceKey::CommittedExpiryProof(proof.event_index);
        if let Some(existing) = env.storage().persistent().get::<_, ExpirationProof>(&key) {
            if &existing != proof {
                return Err(Self::reject(env, TamperEvidenceError::ProofCommitmentConflict));
            }
            return Ok(());
        }
        env.storage().persistent().set(&key, proof);
        Ok(())
    }

    /// Retrieve the persistent proof commitment for an archived event.
    pub fn committed_archive_proof(env: &Env, event_index: u32) -> Option<ArchiveProof> {
        env.storage()
            .persistent()
            .get(&TamperEvidenceKey::CommittedArchiveProof(event_index))
    }

    /// Retrieve the persistent proof commitment for an expired event.
    pub fn committed_expiration_proof(env: &Env, event_index: u32) -> Option<ExpirationProof> {
        env.storage()
            .persistent()
            .get(&TamperEvidenceKey::CommittedExpiryProof(event_index))
    }
}

#[cfg(test)]
#[path = "tamper_evidence_fuzz_tests.rs"]
mod fuzz_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::{contract, Address};

    #[contract]
    struct Probe;

    /// Events can only be published from inside a contract frame.
    fn setup() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(Probe, ());
        (env, id)
    }

    fn h(env: &Env, b: u8) -> BytesN<32> {
        BytesN::from_array(env, &[b; 32])
    }

    fn config(threshold: u32) -> TamperEvidenceConfig {
        TamperEvidenceConfig {
            immutability_threshold: threshold,
            verify_all: true,
            hash_algorithm: HASH_ALGORITHM_SHA256,
            verify_archives: true,
        }
    }

    /// Build an untampered chain of `n` links with hashes 1..=n.
    fn chain(env: &Env, n: u8) -> Vec<ChainLink> {
        let mut links = Vec::new(env);
        for i in 0..n {
            links.push_back(ChainLink {
                index: i as u32,
                hash: h(env, i + 1),
                prev_hash: h(env, i),
                expected_prev_hash: h(env, i),
                valid: false,
            });
        }
        links
    }

    #[test]
    fn test_verify_event_hash_match() {
        let env = Env::default();
        assert!(TamperEvidenceHelper::verify_event_hash(&h(&env, 1), &h(&env, 1)));
    }

    #[test]
    fn test_verify_event_hash_mismatch() {
        let env = Env::default();
        assert!(!TamperEvidenceHelper::verify_event_hash(&h(&env, 1), &h(&env, 2)));
    }

    #[test]
    fn test_chain_link_requires_exact_commitment() {
        let env = Env::default();
        assert!(TamperEvidenceHelper::verify_chain_link(&h(&env, 1), &h(&env, 1)));
        assert!(!TamperEvidenceHelper::verify_chain_link(&h(&env, 1), &h(&env, 2)));
    }

    #[test]
    fn test_integrity_score_perfect() {
        assert_eq!(TamperEvidenceHelper::calculate_integrity_score(100, 0), 100);
    }

    #[test]
    fn test_integrity_score_partial() {
        assert_eq!(TamperEvidenceHelper::calculate_integrity_score(100, 10), 90);
    }

    #[test]
    fn test_integrity_score_all_compromised() {
        assert_eq!(TamperEvidenceHelper::calculate_integrity_score(100, 100), 0);
    }

    #[test]
    fn test_immutability_check() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let proof = TamperEvidenceHelper::verify_immutability(&env, &h(&env, 7), 0, 100, &config(10)).unwrap();
            assert!(proof.immutable);
            assert_eq!(proof.chain_length, 100);
            assert_eq!(proof.references, 99);
            assert_eq!(proof.event_hash, h(&env, 7));
        });
    }

    #[test]
    fn test_immutability_not_reached() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let proof = TamperEvidenceHelper::verify_immutability(&env, &h(&env, 7), 95, 100, &config(50)).unwrap();
            assert!(!proof.immutable);
            assert_eq!(proof.chain_length, 5);
        });
    }

    #[test]
    fn test_immutability_rejects_out_of_range_index() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let err = TamperEvidenceHelper::verify_immutability(&env, &h(&env, 7), 100, 100, &config(1)).unwrap_err();
            assert_eq!(err, TamperEvidenceError::IndexOutOfRange);
        });
        // The rejection is observable off-chain.
        assert_eq!(env.events().all().events().len(), 1);
    }

    #[test]
    fn test_config_validation() {
        assert_eq!(config(0).validate(), Err(TamperEvidenceError::InvalidThreshold));
        let mut c = config(1);
        c.hash_algorithm = 1;
        assert_eq!(c.validate(), Err(TamperEvidenceError::UnsupportedHashAlgorithm));
        assert_eq!(config(1).validate(), Ok(()));
    }

    #[test]
    fn test_chain_continuity_clean_chain() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let v = TamperEvidenceHelper::verify_chain_continuity(&env, &chain(&env, 5)).unwrap();
            assert!(v.valid);
            assert_eq!(v.events_verified, 5);
            assert_eq!(v.integrity_score, 100);
            assert!(v.compromised_indices.is_empty());
        });
        // Only the summary event: no tamper alert on a clean chain.
        assert_eq!(env.events().all().events().len(), 1);
    }

    #[test]
    fn test_chain_continuity_ignores_forged_valid_flag() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let mut links = chain(&env, 4);
            let mut l = links.get(2).unwrap();
            l.prev_hash = h(&env, 0xEE); // break the link…
            l.valid = true; // …while claiming it is fine
            links.set(2, l);

            let v = TamperEvidenceHelper::verify_chain_continuity(&env, &links).unwrap();
            assert!(!v.valid);
            assert_eq!(v.mismatches, 1);
            assert_eq!(v.compromised_indices, Vec::from_array(&env, [2u32]));
        });
        // Summary + tamper alert.
        assert_eq!(env.events().all().events().len(), 2);
    }

    #[test]
    fn test_chain_continuity_detects_substituted_event() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let mut links = chain(&env, 4);
            let mut l = links.get(1).unwrap();
            l.hash = h(&env, 0xAA); // content of event 1 rewritten
            links.set(1, l);

            let v = TamperEvidenceHelper::verify_chain_continuity(&env, &links).unwrap();
            // Event 2 no longer commits to event 1's (new) hash.
            assert_eq!(v.compromised_indices, Vec::from_array(&env, [2u32]));
        });
    }

    #[test]
    fn test_chain_continuity_rejects_empty_and_gaps() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let empty = Vec::new(&env);
            assert_eq!(
                TamperEvidenceHelper::verify_chain_continuity(&env, &empty),
                Err(TamperEvidenceError::EmptyChain)
            );

            let mut links = chain(&env, 3);
            links.remove(1); // deleted event
            assert_eq!(
                TamperEvidenceHelper::verify_chain_continuity(&env, &links),
                Err(TamperEvidenceError::NonSequentialIndex)
            );
        });
    }

    #[test]
    fn test_no_retroactive_modification() {
        let env = Env::default();
        let refs = Vec::from_array(&env, [h(&env, 1), h(&env, 1)]);
        assert!(TamperEvidenceHelper::verify_no_retroactive_modification(
            &h(&env, 1),
            &refs
        ));
    }

    #[test]
    fn test_retroactive_modification_detected() {
        let env = Env::default();
        let refs = Vec::from_array(&env, [h(&env, 1), h(&env, 2)]);
        assert!(!TamperEvidenceHelper::verify_no_retroactive_modification(
            &h(&env, 1),
            &refs
        ));
    }

    #[test]
    fn test_no_references_proves_nothing() {
        let env = Env::default();
        let refs = Vec::new(&env);
        assert!(!TamperEvidenceHelper::verify_no_retroactive_modification(
            &h(&env, 1),
            &refs
        ));
    }

    #[test]
    fn test_archive_proof_requires_path() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            assert_eq!(
                TamperEvidenceHelper::create_archive_proof(&env, &h(&env, 1), &h(&env, 2), 0, Vec::new(&env)),
                Err(TamperEvidenceError::EmptyMerklePath)
            );
        });
    }

    /// Four-leaf tree over events 1..=4.
    fn tree(env: &Env) -> (Vec<BytesN<32>>, BytesN<32>) {
        let l: Vec<BytesN<32>> = Vec::from_array(
            env,
            [1u8, 2, 3, 4].map(|b| TamperEvidenceHelper::merkle_leaf(env, &h(env, b))),
        );
        let n01 = TamperEvidenceHelper::merkle_node(env, &l.get(0).unwrap(), &l.get(1).unwrap());
        let n23 = TamperEvidenceHelper::merkle_node(env, &l.get(2).unwrap(), &l.get(3).unwrap());
        let root = TamperEvidenceHelper::merkle_node(env, &n01, &n23);
        (Vec::from_array(env, [l.get(3).unwrap(), n01]), root) // path for leaf 2 (event 3)
    }

    #[test]
    fn test_archive_proof_reconstructs_root() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let (path, root) = tree(&env);
            let proof = TamperEvidenceHelper::create_archive_proof(&env, &h(&env, 3), &root, 2, path).unwrap();
            assert!(proof.valid);
            assert!(TamperEvidenceHelper::verify_archive_proof(&env, &proof));

            // Wrong event, wrong position, or a forged `valid` flag all fail.
            let mut bad = proof.clone();
            bad.event_hash = h(&env, 4);
            bad.valid = true;
            assert!(!TamperEvidenceHelper::verify_archive_proof(&env, &bad));
            let mut bad = proof.clone();
            bad.leaf_index = 3;
            assert!(!TamperEvidenceHelper::verify_archive_proof(&env, &bad));
            // Index bits above the tree depth are rejected, not ignored.
            let mut bad = proof;
            bad.leaf_index = 2 | (1 << 2);
            assert!(!TamperEvidenceHelper::verify_archive_proof(&env, &bad));
        });
    }

    #[test]
    fn test_merkle_leaf_and_node_are_domain_separated() {
        let env = Env::default();
        // An interior node must never be presentable as a leaf.
        let a = h(&env, 1);
        let mut concat = Bytes::from_array(&env, &[MERKLE_NODE_PREFIX]);
        concat.append(&a.clone().into());
        concat.append(&a.clone().into());
        let node = TamperEvidenceHelper::merkle_node(&env, &a, &a);
        let as_leaf: BytesN<32> = env.crypto().sha256(&concat).into();
        assert_eq!(node, as_leaf);
        assert_ne!(TamperEvidenceHelper::merkle_leaf(&env, &node), node);
    }

    #[test]
    fn test_witness_commitment_is_bound_to_contents() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let w = TamperEvidenceHelper::create_chain_witness(&env, 5, h(&env, 9));
            assert_ne!(w.commitment_hash, BytesN::from_array(&env, &[0u8; 32]));
            assert_eq!(
                w.commitment_hash,
                TamperEvidenceHelper::compute_witness_commitment(&env, 5, &h(&env, 9), w.timestamp)
            );
            assert_ne!(
                w.commitment_hash,
                TamperEvidenceHelper::compute_witness_commitment(&env, 6, &h(&env, 9), w.timestamp)
            );
        });
    }
}

#[cfg(test)]
mod metering_tests {
    //! Host-budget regression tests. The Soroban budget is deterministic for
    //! a given SDK/host version, so these ceilings catch accidental
    //! regressions (e.g. reintroducing per-mismatch `push_back`) without
    //! being flaky. Ceilings carry ~2x headroom over measured values.
    use super::*;
    use soroban_sdk::{contract, Address};

    #[contract]
    struct Probe;

    fn hb(env: &Env, i: u32) -> BytesN<32> {
        let mut a = [0x11u8; 32];
        a[..4].copy_from_slice(&i.to_be_bytes());
        BytesN::from_array(env, &a)
    }

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(Probe, ());
        (env, id)
    }

    fn links(env: &Env, n: u32, tamper_all: bool) -> Vec<ChainLink> {
        let mut v = Vec::new(env);
        for k in 0..n {
            let prev = if tamper_all { hb(env, u32::MAX) } else { hb(env, k) };
            v.push_back(ChainLink {
                index: k,
                hash: hb(env, k + 1),
                prev_hash: prev,
                expected_prev_hash: hb(env, k),
                valid: true,
            });
        }
        v
    }

    fn flat(env: &Env, n: u32, tamper_all: bool) -> (Vec<BytesN<32>>, Vec<BytesN<32>>) {
        let (mut hashes, mut prevs) = (Vec::new(env), Vec::new(env));
        for k in 0..n {
            hashes.push_back(hb(env, k + 1));
            prevs.push_back(if tamper_all { hb(env, u32::MAX) } else { hb(env, k) });
        }
        (hashes, prevs)
    }

    /// Returns (cpu, mem) consumed by `f` alone.
    fn metered<R>(env: &Env, id: &Address, f: impl FnOnce() -> R) -> (R, u64, u64) {
        env.cost_estimate().budget().reset_unlimited();
        let r = env.as_contract(id, f);
        let b = env.cost_estimate().budget();
        (r, b.cpu_instruction_cost(), b.memory_bytes_cost())
    }

    #[test]
    fn fully_tampered_batch_memory_is_bounded() {
        let (env, id) = setup();
        let input = links(&env, 1_024, true);
        let (v, _, mem) = metered(&env, &id, || {
            TamperEvidenceHelper::verify_chain_continuity(&env, &input)
        });
        let v = v.unwrap();
        assert_eq!(v.mismatches, 1_024);
        assert_eq!(v.compromised_indices.len() as usize, MAX_REPORTED_COMPROMISED);
        assert!(v.indices_truncated);
        // Previously ~4.3 MB (quadratic in mismatches).
        assert!(mem < 64_000, "mem {mem}");
    }

    #[test]
    fn memory_does_not_grow_with_mismatch_count() {
        let (env, id) = setup();
        let few = links(&env, 128, true);
        let many = links(&env, 2_048, true);
        let (_, _, m_few) = metered(&env, &id, || TamperEvidenceHelper::verify_chain_continuity(&env, &few));
        let (_, _, m_many) = metered(&env, &id, || TamperEvidenceHelper::verify_chain_continuity(&env, &many));
        // 16x the mismatches, same footprint.
        assert_eq!(m_few, m_many);
    }

    #[test]
    fn chain_continuity_cpu_ceiling() {
        let (env, id) = setup();
        let input = links(&env, 1_024, false);
        let (_, cpu, _) = metered(&env, &id, || {
            TamperEvidenceHelper::verify_chain_continuity(&env, &input)
        });
        // Previously ~1.84M for this input.
        assert!(cpu < 3_000_000, "cpu {cpu}");
    }

    #[test]
    fn compact_path_is_cheaper_and_agrees() {
        let (env, id) = setup();
        for tamper_all in [false, true] {
            let input = links(&env, 1_024, tamper_all);
            let (hashes, prevs) = flat(&env, 1_024, tamper_all);
            let (a, cpu_links, _) = metered(&env, &id, || {
                TamperEvidenceHelper::verify_chain_continuity(&env, &input)
            });
            let (b, cpu_flat, _) = metered(&env, &id, || {
                TamperEvidenceHelper::verify_hash_chain(&env, 0, &hb(&env, 0), &hashes, &prevs)
            });
            assert_eq!(a.unwrap(), b.unwrap());
            // At least 30% cheaper (measured ~45%).
            assert!(cpu_flat * 10 < cpu_links * 7, "flat {cpu_flat} links {cpu_links}");
        }
    }

    #[test]
    fn max_batch_fits_and_oversize_is_rejected() {
        let (env, id) = setup();
        let (hashes, prevs) = flat(&env, MAX_CHAIN_BATCH, false);
        let (v, cpu, _) = metered(&env, &id, || {
            TamperEvidenceHelper::verify_hash_chain(&env, 0, &hb(&env, 0), &hashes, &prevs)
        });
        assert!(v.unwrap().valid);
        assert!(cpu < 5_000_000, "cpu {cpu}");

        let (mut big_h, mut big_p) = (hashes, prevs);
        big_h.push_back(hb(&env, MAX_CHAIN_BATCH + 1));
        big_p.push_back(hb(&env, MAX_CHAIN_BATCH));
        let r = env.as_contract(&id, || {
            TamperEvidenceHelper::verify_hash_chain(&env, 0, &hb(&env, 0), &big_h, &big_p)
        });
        assert_eq!(r, Err(TamperEvidenceError::BatchTooLarge));
    }

    #[test]
    fn compact_path_validates_inputs() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let (h, p) = flat(&env, 4, false);
            let mut short = p.clone();
            short.pop_back();
            assert_eq!(
                TamperEvidenceHelper::verify_hash_chain(&env, 0, &hb(&env, 0), &h, &short),
                Err(TamperEvidenceError::LengthMismatch)
            );
            assert_eq!(
                TamperEvidenceHelper::verify_hash_chain(&env, u32::MAX, &hb(&env, 0), &h, &p),
                Err(TamperEvidenceError::IndexOutOfRange)
            );
            let empty = Vec::new(&env);
            assert_eq!(
                TamperEvidenceHelper::verify_hash_chain(&env, 0, &hb(&env, 0), &empty, &empty),
                Err(TamperEvidenceError::EmptyChain)
            );
            // Absolute indices are reported relative to `first_index`.
            let mut bad = p;
            bad.set(2, hb(&env, 0xDEAD));
            let v = TamperEvidenceHelper::verify_hash_chain(&env, 100, &hb(&env, 0), &h, &bad).unwrap();
            assert_eq!(v.compromised_indices, Vec::from_array(&env, [102u32]));
        });
    }
}

#[cfg(test)]
mod signature_tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use soroban_sdk::testutils::{Events as _, Ledger as _};
    use soroban_sdk::{contract, Address};

    #[contract]
    struct Probe;

    struct Validator {
        key: SigningKey,
    }

    impl Validator {
        fn new(seed: u8) -> Self {
            Self {
                key: SigningKey::from_bytes(&[seed; 32]),
            }
        }
        fn pk(&self, env: &Env) -> BytesN<32> {
            BytesN::from_array(env, &self.key.verifying_key().to_bytes())
        }
        fn sign(&self, env: &Env, w: &ChainWitness, nonce: u64, valid_until: u32) -> SignedWitness {
            let signer = self.pk(env);
            let digest = TamperEvidenceHelper::witness_digest(env, w, &signer, nonce, valid_until);
            let sig = self.key.sign(&digest.to_array());
            SignedWitness {
                witness: w.clone(),
                signer,
                nonce,
                valid_until_ledger: valid_until,
                signature: BytesN::from_array(env, &sig.to_bytes()),
            }
        }
    }

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.sequence_number = 1_000;
            l.timestamp = 1_700_000_000;
        });
        let id = env.register(Probe, ());
        (env, id)
    }

    fn h(env: &Env, b: u8) -> BytesN<32> {
        BytesN::from_array(env, &[b; 32])
    }

    #[test]
    fn honest_witness_is_accepted_and_pins_root() {
        let (env, id) = setup();
        let v = Validator::new(1);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [v.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            let sw = v.sign(&env, &w, 1, 1_010);
            assert_eq!(TamperEvidenceHelper::accept_signed_witness(&env, &sw, &set), Ok(()));
            assert_eq!(TamperEvidenceHelper::last_nonce(&env, &v.pk(&env)), Some(1));
            assert_eq!(TamperEvidenceHelper::accepted_root(&env, 7), Some(h(&env, 0xAB)));
        });
        assert_eq!(env.events().all().events().len(), 1);
    }

    #[test]
    fn exact_replay_is_rejected() {
        let (env, id) = setup();
        let v = Validator::new(1);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [v.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            let sw = v.sign(&env, &w, 1, 1_010);
            TamperEvidenceHelper::accept_signed_witness(&env, &sw, &set).unwrap();
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &sw, &set),
                Err(TamperEvidenceError::ReplayedNonce)
            );
            // Older nonce for a new witness is also a replay.
            let w2 = TamperEvidenceHelper::create_chain_witness(&env, 8, h(&env, 0xAC));
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &v.sign(&env, &w2, 0, 1_010), &set),
                Err(TamperEvidenceError::ReplayedNonce)
            );
        });
    }

    #[test]
    fn expiry_window_is_enforced() {
        let (env, id) = setup();
        let v = Validator::new(1);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [v.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &v.sign(&env, &w, 1, 999), &set),
                Err(TamperEvidenceError::WitnessExpired)
            );
            let too_far = 1_000 + MAX_WITNESS_VALIDITY_LEDGERS + 1;
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &v.sign(&env, &w, 1, too_far), &set),
                Err(TamperEvidenceError::ExpiryTooFar)
            );
        });
    }

    #[test]
    fn unauthorized_and_inconsistent_witnesses_are_rejected() {
        let (env, id) = setup();
        let trusted = Validator::new(1);
        let outsider = Validator::new(2);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [trusted.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &outsider.sign(&env, &w, 1, 1_010), &set),
                Err(TamperEvidenceError::UnauthorizedSigner)
            );

            let mut forged = w.clone();
            forged.root_at_index = h(&env, 0xCD); // commitment no longer matches
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &trusted.sign(&env, &forged, 1, 1_010), &set),
                Err(TamperEvidenceError::CommitmentMismatch)
            );

            let mut future = w;
            future.timestamp += 1;
            future.commitment_hash =
                TamperEvidenceHelper::compute_witness_commitment(&env, 7, &future.root_at_index, future.timestamp);
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &trusted.sign(&env, &future, 1, 1_010), &set),
                Err(TamperEvidenceError::FutureTimestamp)
            );
        });
    }

    #[test]
    fn equivocation_is_detected_across_signers() {
        let (env, id) = setup();
        let a = Validator::new(1);
        let b = Validator::new(2);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [a.pk(&env), b.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            TamperEvidenceHelper::accept_signed_witness(&env, &a.sign(&env, &w, 1, 1_010), &set).unwrap();
            // Co-signing the same root is fine.
            TamperEvidenceHelper::accept_signed_witness(&env, &b.sign(&env, &w, 1, 1_010), &set).unwrap();
            // A conflicting root for the same index is not.
            let w2 = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xEE));
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(&env, &b.sign(&env, &w2, 2, 1_010), &set),
                Err(TamperEvidenceError::Equivocation)
            );
            assert_eq!(TamperEvidenceHelper::accepted_root(&env, 7), Some(h(&env, 0xAB)));
            // A rejected witness does not burn the nonce.
            assert_eq!(TamperEvidenceHelper::last_nonce(&env, &b.pk(&env)), Some(1));
        });
    }

    #[test]
    fn signatures_are_deduplicated_per_set_and_ledger_epoch() {
        let (env, id) = setup();
        let a = Validator::new(1);
        let b = Validator::new(2);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [a.pk(&env), b.pk(&env)]);
            let first = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            TamperEvidenceHelper::accept_signed_witness(&env, &a.sign(&env, &first, 1, 1_010), &set).unwrap();

            let next = TamperEvidenceHelper::create_chain_witness(&env, 8, h(&env, 0xAC));
            assert_eq!(
                TamperEvidenceHelper::accept_signed_witness(
                    &env,
                    &a.sign(&env, &next, 2, 1_010),
                    &Vec::from_array(&env, [b.pk(&env), a.pk(&env)])
                ),
                Err(TamperEvidenceError::RedundantSignature)
            );
            assert_eq!(TamperEvidenceHelper::last_nonce(&env, &a.pk(&env)), Some(1));

            // A different roster is a different validator set.
            let expanded_set = Vec::from_array(&env, [a.pk(&env), b.pk(&env), Validator::new(3).pk(&env)]);
            TamperEvidenceHelper::accept_signed_witness(&env, &a.sign(&env, &next, 2, 1_010), &expanded_set).unwrap();

            env.ledger().with_mut(|ledger| {
                ledger.sequence_number = WITNESS_EPOCH_LENGTH_LEDGERS;
                ledger.timestamp = ledger.timestamp.saturating_add(5);
            });
            let next_epoch = TamperEvidenceHelper::create_chain_witness(&env, 9, h(&env, 0xAD));
            TamperEvidenceHelper::accept_signed_witness(
                &env,
                &a.sign(&env, &next_epoch, 3, WITNESS_EPOCH_LENGTH_LEDGERS + 10),
                &set,
            )
            .unwrap();
        });
    }

    #[test]
    fn archive_proof_is_persisted_only_after_validator_root_acceptance() {
        let (env, id) = setup();
        let validator = Validator::new(1);
        env.as_contract(&id, || {
            let event_index = 7;
            let event_hash = h(&env, 0xA7);
            let sibling = h(&env, 0xB8);
            let leaf = TamperEvidenceHelper::merkle_leaf(&env, &event_hash);
            let root = TamperEvidenceHelper::merkle_node(&env, &leaf, &sibling);
            let proof = TamperEvidenceHelper::create_archive_proof(
                &env,
                &event_hash,
                &root,
                0,
                Vec::from_array(&env, [sibling]),
            )
            .unwrap();
            assert_eq!(
                TamperEvidenceHelper::commit_archive_proof(&env, event_index, &proof),
                Err(TamperEvidenceError::UnanchoredProofRoot)
            );

            let witness = TamperEvidenceHelper::create_chain_witness(&env, event_index, root);
            let validator_set = Vec::from_array(&env, [validator.pk(&env)]);
            TamperEvidenceHelper::accept_signed_witness(
                &env,
                &validator.sign(&env, &witness, 1, 1_010),
                &validator_set,
            )
            .unwrap();
            TamperEvidenceHelper::commit_archive_proof(&env, event_index, &proof).unwrap();
            assert_eq!(
                TamperEvidenceHelper::committed_archive_proof(&env, event_index),
                Some(proof)
            );
        });
    }

    #[test]
    fn expiration_proofs_require_expired_and_validator_anchored_roots() {
        let (env, id) = setup();
        let validator = Validator::new(1);
        env.as_contract(&id, || {
            let event_index = 7;
            let event_hash = h(&env, 0xA7);
            let expired_at = env.ledger().sequence();
            let sibling = h(&env, 0xB8);
            let leaf = TamperEvidenceHelper::expiration_leaf(&env, event_index, &event_hash, expired_at);
            let root = TamperEvidenceHelper::merkle_node(&env, &leaf, &sibling);
            let path = Vec::from_array(&env, [sibling]);

            let proof = TamperEvidenceHelper::create_expiration_proof(
                &env,
                event_index,
                &event_hash,
                expired_at,
                &root,
                0,
                path,
            )
            .unwrap();
            assert!(proof.valid);
            assert!(TamperEvidenceHelper::verify_expiration_proof(&env, &proof));

            let mut altered = proof.clone();
            altered.event_hash = h(&env, 0xA8);
            altered.valid = true;
            assert!(!TamperEvidenceHelper::verify_expiration_proof(&env, &altered));

            let not_yet_expired = TamperEvidenceHelper::create_expiration_proof(
                &env,
                event_index,
                &event_hash,
                expired_at + 1,
                &root,
                0,
                Vec::from_array(&env, [h(&env, 0xB8)]),
            );
            assert_eq!(not_yet_expired, Err(TamperEvidenceError::RecordNotExpired));

            let witness = TamperEvidenceHelper::create_chain_witness(&env, event_index, root.clone());
            let validator_set = Vec::from_array(&env, [validator.pk(&env)]);
            TamperEvidenceHelper::accept_signed_witness(
                &env,
                &validator.sign(&env, &witness, 1, 1_010),
                &validator_set,
            )
            .unwrap();
            TamperEvidenceHelper::commit_expiration_proof(&env, &proof).unwrap();
            assert_eq!(
                TamperEvidenceHelper::committed_expiration_proof(&env, event_index),
                Some(proof.clone())
            );
            assert_eq!(
                TamperEvidenceHelper::commit_expiration_proof(&env, &altered),
                Err(TamperEvidenceError::CommitmentMismatch)
            );
        });
    }

    #[test]
    fn expiry_proof_rejects_roots_without_validator_witnesses() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            let event_hash = h(&env, 0xA7);
            let expired_at = env.ledger().sequence();
            let sibling = h(&env, 0xB8);
            let leaf = TamperEvidenceHelper::expiration_leaf(&env, 4, &event_hash, expired_at);
            let root = TamperEvidenceHelper::merkle_node(&env, &leaf, &sibling);
            let proof = TamperEvidenceHelper::create_expiration_proof(
                &env,
                4,
                &event_hash,
                expired_at,
                &root,
                0,
                Vec::from_array(&env, [sibling]),
            )
            .unwrap();
            assert_eq!(
                TamperEvidenceHelper::commit_expiration_proof(&env, &proof),
                Err(TamperEvidenceError::UnanchoredProofRoot)
            );
            assert_eq!(TamperEvidenceHelper::committed_expiration_proof(&env, 4), None);
        });
    }

    #[test]
    #[should_panic(expected = "failed ED25519 verification")]
    fn tampered_signature_traps() {
        let (env, id) = setup();
        let v = Validator::new(1);
        env.as_contract(&id, || {
            let set = Vec::from_array(&env, [v.pk(&env)]);
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            let mut sw = v.sign(&env, &w, 1, 1_010);
            sw.nonce = 2; // nonce is covered by the signature
            let _ = TamperEvidenceHelper::accept_signed_witness(&env, &sw, &set);
        });
    }

    #[test]
    #[should_panic(expected = "failed ED25519 verification")]
    fn cross_contract_replay_traps() {
        let (env, id) = setup();
        let other = env.register(Probe, ());
        let v = Validator::new(1);
        let sw = env.as_contract(&id, || {
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            v.sign(&env, &w, 1, 1_010)
        });
        env.as_contract(&other, || {
            let set = Vec::from_array(&env, [v.pk(&env)]);
            let _ = TamperEvidenceHelper::accept_signed_witness(&env, &sw, &set);
        });
    }

    #[test]
    fn digest_is_deterministic_and_network_bound() {
        let (env, id) = setup();
        let v = Validator::new(1);
        env.as_contract(&id, || {
            let w = TamperEvidenceHelper::create_chain_witness(&env, 7, h(&env, 0xAB));
            let d1 = TamperEvidenceHelper::witness_digest(&env, &w, &v.pk(&env), 1, 1_010);
            let d2 = TamperEvidenceHelper::witness_digest(&env, &w, &v.pk(&env), 1, 1_010);
            assert_eq!(d1, d2);
            env.ledger().with_mut(|l| l.network_id = [9u8; 32]);
            let d3 = TamperEvidenceHelper::witness_digest(&env, &w, &v.pk(&env), 1, 1_010);
            assert_ne!(d1, d3);
        });
    }
}
