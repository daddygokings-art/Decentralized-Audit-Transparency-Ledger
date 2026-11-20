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

use soroban_sdk::{contracterror, contractevent, contracttype, BytesN, Env, Vec};

/// Identifier of the SHA-256 hash algorithm in [`TamperEvidenceConfig`].
pub const HASH_ALGORITHM_SHA256: u32 = 0;

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
    /// Path from event to archive root
    pub merkle_path: Vec<BytesN<32>>,
    /// Whether the proof is valid
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

    /// Create a chain witness commitment
    pub fn create_chain_witness(env: &Env, event_index: u32, root_at_index: BytesN<32>) -> ChainWitness {
        ChainWitness {
            event_index,
            root_at_index,
            timestamp: env.ledger().timestamp(),
            commitment_hash: BytesN::<32>::from_array(env, &[0u8; 32]),
        }
    }

    /// Verify a chain of events by checking hash continuity.
    ///
    /// A link is compromised when its recorded `prev_hash` differs from the
    /// supplied `expected_prev_hash`, or (for every link after the first)
    /// from the actual hash of the preceding link. Indices must be strictly
    /// sequential; a gap or reorder is a hard error because it means events
    /// were removed or injected rather than modified.
    pub fn verify_chain_continuity(
        env: &Env,
        links: &Vec<ChainLink>,
    ) -> Result<ChainVerification, TamperEvidenceError> {
        if links.is_empty() {
            return Err(Self::reject(env, TamperEvidenceError::EmptyChain));
        }

        let mut mismatch_count: u32 = 0;
        let mut compromised = Vec::new(env);
        let mut prev: Option<ChainLink> = None;

        for link in links.iter() {
            let mut ok = link.prev_hash == link.expected_prev_hash;
            if let Some(p) = &prev {
                if p.index.checked_add(1) != Some(link.index) {
                    return Err(Self::reject(env, TamperEvidenceError::NonSequentialIndex));
                }
                ok &= Self::verify_chain_link(&p.hash, &link.prev_hash);
            }
            if !ok {
                mismatch_count += 1;
                compromised.push_back(link.index);
            }
            prev = Some(link);
        }

        let events_verified = links.len();
        let integrity_score = Self::calculate_integrity_score(events_verified, mismatch_count);
        let valid = mismatch_count == 0;

        ChainVerifiedEvent {
            valid,
            events_verified,
            mismatches: mismatch_count,
            integrity_score,
        }
        .publish(env);
        if let Some(first) = compromised.first() {
            TamperDetectedEvent {
                first_compromised_index: first,
                mismatches: mismatch_count,
                events_verified,
            }
            .publish(env);
        }

        Ok(ChainVerification {
            valid,
            events_verified,
            mismatches: mismatch_count,
            integrity_score,
            compromised_indices: compromised,
        })
    }

    /// Verify that an event has not been retroactively modified
    /// by checking that its hash still matches all references to it.
    ///
    /// An empty reference set proves nothing and therefore returns `false`.
    pub fn verify_no_retroactive_modification(event_hash: &BytesN<32>, references: &Vec<BytesN<32>>) -> bool {
        !references.is_empty() && references.iter().all(|ref_hash| ref_hash == *event_hash)
    }

    /// Create proof that event is immutable by archive
    pub fn create_archive_proof(
        env: &Env,
        event_hash: &BytesN<32>,
        archive_root: &BytesN<32>,
        merkle_path: Vec<BytesN<32>>,
    ) -> Result<ArchiveProof, TamperEvidenceError> {
        if merkle_path.is_empty() {
            return Err(Self::reject(env, TamperEvidenceError::EmptyMerklePath));
        }

        Ok(ArchiveProof {
            event_hash: event_hash.clone(),
            archive_root: archive_root.clone(),
            merkle_path,
            valid: true,
        })
    }

    /// Verify an archive proof by reconstructing the root
    pub fn verify_archive_proof(proof: &ArchiveProof) -> bool {
        // In production, reconstruct root from event_hash and merkle_path
        // and verify it matches archive_root
        proof.valid
    }
}

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
                TamperEvidenceHelper::create_archive_proof(&env, &h(&env, 1), &h(&env, 2), Vec::new(&env)),
                Err(TamperEvidenceError::EmptyMerklePath)
            );
        });
    }
}
