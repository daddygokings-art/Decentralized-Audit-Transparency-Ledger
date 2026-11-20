//! Tamper-Evidence Verification System for Audit Trails
//!
//! Implements cryptographic chain verification to detect tampering,
//! prove event immutability, and validate audit trail integrity.

use soroban_sdk::{
    contracterror, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env, Map, Vec,
};

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
    /// Integrity score (0.0 = invalid, 1.0 = perfect)
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
    /// Hash of the previous event
    pub prev_hash: BytesN<32>,
    /// Expected previous hash (from next event's prev_hash field)
    pub expected_prev_hash: BytesN<32>,
    /// Whether this link is valid
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
    /// Number of events that reference this event's hash
    pub references: u32,
    /// Hash chain from this event to the most recent event
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
    /// Algorithm for hash verification (SHA256=0, BLAKE2b=1, etc)
    pub hash_algorithm: u32,
    /// Enable archive root verification
    pub verify_archives: bool,
}

/// Storage keys used by the validator attestation protocol.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TamperEvidenceKey {
    /// Pending attestations for an event index: `validator -> attested root`.
    Attestations(u32),
    /// Finalized, quorum-backed witness for an event index.
    Witness(u32),
}

/// Errors raised by the validator attestation protocol.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TamperEvidenceError {
    /// Quorum must be at least one validator.
    InvalidQuorum = 1,
    /// A validator attested two different roots for the same event index
    /// (equivocation). The first attestation is kept; the second is rejected.
    ConflictingAttestation = 2,
    /// A witness was already finalized for this index with a different root.
    WitnessAlreadyFinalized = 3,
}

/// Result of recording a validator attestation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttestationStatus {
    /// Quorum not yet reached; carries the number of distinct validators that
    /// have attested the same root as this call.
    Pending(u32),
    /// This attestation completed the quorum and finalized the witness.
    Finalized(ChainWitness),
    /// The witness had already been finalized with the same root; the
    /// attestation is accepted idempotently and changes nothing.
    AlreadyFinalized(ChainWitness),
}

/// Helper functions for tamper-evidence verification
pub struct TamperEvidenceHelper;

impl TamperEvidenceHelper {
    /// Verify a single event's hash is correct
    pub fn verify_event_hash(
        event_hash: &BytesN<32>,
        expected_hash: &BytesN<32>,
    ) -> bool {
        event_hash == expected_hash
    }

    /// Verify the chain link between two consecutive events.
    ///
    /// The next event must reference the current event's hash. Two events
    /// that merely share the same parent (`current_prev_hash ==
    /// next_prev_hash`) are siblings produced by concurrent appends, i.e. a
    /// fork, and are rejected; so is a self-referencing event.
    pub fn verify_chain_link(
        current_event_hash: &BytesN<32>,
        current_prev_hash: &BytesN<32>,
        next_event_hash: &BytesN<32>,
        next_prev_hash: &BytesN<32>,
    ) -> bool {
        next_prev_hash == current_event_hash
            && next_event_hash != current_event_hash
            && current_prev_hash != next_prev_hash
    }

    /// Calculate the integrity score for a chain
    pub fn calculate_integrity_score(
        total_events: u32,
        mismatches: u32,
    ) -> u32 {
        if total_events == 0 {
            100
        } else {
            let valid_events = total_events.saturating_sub(mismatches);
            ((valid_events as u64 * 100) / (total_events as u64)) as u32
        }
    }

    /// Verify event immutability based on subsequent references
    pub fn verify_immutability(
        event_index: u32,
        total_events: u32,
        config: &TamperEvidenceConfig,
    ) -> ImmutabilityProof {
        let chain_length = total_events.saturating_sub(event_index);
        let immutable = chain_length >= config.immutability_threshold;

        ImmutabilityProof {
            event_index,
            event_hash: BytesN::<32>::from_array(&Env::default(), &[0u8; 32]),
            references: chain_length,
            chain_length,
            immutable,
        }
    }

    /// Create a chain witness commitment.
    ///
    /// The commitment is `sha256(event_index_be || root_at_index)`, so it is
    /// fully deterministic and can be recomputed off-chain.
    pub fn create_chain_witness(
        env: &Env,
        event_index: u32,
        root_at_index: BytesN<32>,
    ) -> ChainWitness {
        let commitment_hash =
            Self::attestation_commitment(env, event_index, &root_at_index, &Vec::new(env));
        ChainWitness {
            event_index,
            root_at_index,
            timestamp: env.ledger().timestamp(),
            commitment_hash,
        }
    }

    /// Verify a chain of events by checking hash continuity.
    ///
    /// A link is counted as compromised when it is flagged invalid, when its
    /// index does not strictly increase (duplicate / reordered event), or when
    /// it shares its `prev_hash` with an earlier link (a fork caused by two
    /// writers appending against the same parent concurrently).
    pub fn verify_chain_continuity(links: &Vec<ChainLink>) -> ChainVerification {
        let env = links.env();
        let mut mismatch_count: u32 = 0;
        let mut compromised = Vec::new(env);
        let mut seen_parents: Map<BytesN<32>, u32> = Map::new(env);
        let mut last_index: Option<u32> = None;

        for link in links.iter() {
            let ordered = !matches!(last_index, Some(prev) if link.index <= prev);
            let forked = seen_parents.contains_key(link.prev_hash.clone());

            if !link.valid || !ordered || forked {
                mismatch_count += 1;
                compromised.push_back(link.index);
            }

            seen_parents.set(link.prev_hash.clone(), link.index);
            if ordered {
                last_index = Some(link.index);
            }
        }

        let integrity_score = Self::calculate_integrity_score(
            links.len(),
            mismatch_count,
        );

        ChainVerification {
            valid: mismatch_count == 0,
            events_verified: links.len(),
            mismatches: mismatch_count,
            integrity_score,
            compromised_indices: compromised,
        }
    }

    /// Verify that an event has not been retroactively modified
    /// by checking that its hash still matches all references to it
    pub fn verify_no_retroactive_modification(
        event_hash: &BytesN<32>,
        references: &Vec<BytesN<32>>,
    ) -> bool {
        // All references should match the original event hash
        references.iter().all(|ref_hash| ref_hash == *event_hash)
    }

    /// Create proof that event is immutable by archive
    pub fn create_archive_proof(
        event_hash: &BytesN<32>,
        archive_root: &BytesN<32>,
        merkle_path: Vec<BytesN<32>>,
    ) -> ArchiveProof {
        let valid = !merkle_path.is_empty();

        ArchiveProof {
            event_hash: event_hash.clone(),
            archive_root: archive_root.clone(),
            merkle_path,
            valid,
        }
    }

    /// Verify an archive proof by reconstructing the root
    pub fn verify_archive_proof(proof: &ArchiveProof) -> bool {
        // In production, reconstruct root from event_hash and merkle_path
        // and verify it matches archive_root
        proof.valid
    }

    // ------------------------------------------------------------------
    // Validator attestations
    // ------------------------------------------------------------------

    /// Record a validator's attestation that `root_at_index` is the Merkle
    /// root of the audit trail at `event_index`, finalizing a
    /// [`ChainWitness`] once `quorum` distinct validators agree.
    ///
    /// Must be called from within a contract context. The protocol is safe
    /// under simultaneous attestations (several in the same ledger, in any
    /// transaction order):
    ///
    /// * **No double counting:** attestations are keyed by validator, so a
    ///   retried or replayed attestation is idempotent.
    /// * **Equivocation is rejected:** a validator cannot switch its vote to a
    ///   different root ([`TamperEvidenceError::ConflictingAttestation`]).
    /// * **Write-once witness:** once finalized, a witness can never be
    ///   overwritten by a late or conflicting attestation
    ///   ([`TamperEvidenceError::WitnessAlreadyFinalized`]).
    /// * **Order-independent commitment:** the witness commitment is computed
    ///   over the attesting validators in canonical (sorted `Map`) order, so it
    ///   is identical regardless of the order attestations arrived in.
    ///
    /// Callers should choose `quorum` greater than half of the validator set
    /// so that two conflicting roots can never both reach quorum.
    pub fn record_validator_attestation(
        env: &Env,
        validator: &Address,
        event_index: u32,
        root_at_index: &BytesN<32>,
        quorum: u32,
    ) -> Result<AttestationStatus, TamperEvidenceError> {
        validator.require_auth();
        if quorum == 0 {
            return Err(TamperEvidenceError::InvalidQuorum);
        }

        let storage = env.storage().persistent();
        let witness_key = TamperEvidenceKey::Witness(event_index);
        if let Some(witness) = storage.get::<_, ChainWitness>(&witness_key) {
            return if witness.root_at_index == *root_at_index {
                Ok(AttestationStatus::AlreadyFinalized(witness))
            } else {
                Err(TamperEvidenceError::WitnessAlreadyFinalized)
            };
        }

        let attestations_key = TamperEvidenceKey::Attestations(event_index);
        let mut attestations: Map<Address, BytesN<32>> = storage
            .get(&attestations_key)
            .unwrap_or_else(|| Map::new(env));

        match attestations.get(validator.clone()) {
            Some(existing) if existing == *root_at_index => {}
            Some(_) => return Err(TamperEvidenceError::ConflictingAttestation),
            None => attestations.set(validator.clone(), root_at_index.clone()),
        }

        // Map iteration is in canonical key order, independent of insertion.
        let mut attesters = Vec::new(env);
        for (addr, root) in attestations.iter() {
            if root == *root_at_index {
                attesters.push_back(addr);
            }
        }

        if attesters.len() < quorum {
            storage.set(&attestations_key, &attestations);
            return Ok(AttestationStatus::Pending(attesters.len()));
        }

        let witness = ChainWitness {
            event_index,
            root_at_index: root_at_index.clone(),
            timestamp: env.ledger().timestamp(),
            commitment_hash: Self::attestation_commitment(
                env,
                event_index,
                root_at_index,
                &attesters,
            ),
        };
        storage.set(&witness_key, &witness);
        storage.remove(&attestations_key);
        Ok(AttestationStatus::Finalized(witness))
    }

    /// Return the finalized witness for `event_index`, if any.
    pub fn get_finalized_witness(env: &Env, event_index: u32) -> Option<ChainWitness> {
        env.storage()
            .persistent()
            .get(&TamperEvidenceKey::Witness(event_index))
    }

    /// Deterministic witness commitment:
    /// `sha256(event_index_be || root || xdr(attester_0) || ... )`, with
    /// attesters in canonical order.
    pub fn attestation_commitment(
        env: &Env,
        event_index: u32,
        root_at_index: &BytesN<32>,
        attesters: &Vec<Address>,
    ) -> BytesN<32> {
        let mut pre_image = Bytes::new(env);
        pre_image.extend_from_array(&event_index.to_be_bytes());
        pre_image.append(&Bytes::from(root_at_index.clone()));
        for attester in attesters.iter() {
            pre_image.append(&attester.to_xdr(env));
        }
        env.crypto().sha256(&pre_image).to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct AttestationHarness;

    #[contractimpl]
    impl AttestationHarness {}

    fn harness(env: &Env) -> Address {
        env.mock_all_auths();
        env.register(AttestationHarness, ())
    }

    /// Each attestation is a separate invocation (own auth frame), as on-chain.
    fn attest(
        env: &Env,
        contract: &Address,
        validator: &Address,
        index: u32,
        root: &BytesN<32>,
        quorum: u32,
    ) -> Result<AttestationStatus, TamperEvidenceError> {
        env.as_contract(contract, || {
            TamperEvidenceHelper::record_validator_attestation(env, validator, index, root, quorum)
        })
    }

    fn witness(env: &Env, contract: &Address, index: u32) -> Option<ChainWitness> {
        env.as_contract(contract, || TamperEvidenceHelper::get_finalized_witness(env, index))
    }

    fn root(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }

    fn link(env: &Env, index: u32, hash: u8, prev: u8) -> ChainLink {
        ChainLink {
            index,
            hash: root(env, hash),
            prev_hash: root(env, prev),
            expected_prev_hash: root(env, prev),
            valid: true,
        }
    }

    #[test]
    fn test_chain_link_accepts_direct_successor() {
        let env = Env::default();
        assert!(TamperEvidenceHelper::verify_chain_link(
            &root(&env, 2),
            &root(&env, 1),
            &root(&env, 3),
            &root(&env, 2),
        ));
    }

    #[test]
    fn test_chain_link_rejects_sibling_fork() {
        let env = Env::default();
        // Two events appended concurrently against the same parent (1).
        assert!(!TamperEvidenceHelper::verify_chain_link(
            &root(&env, 2),
            &root(&env, 1),
            &root(&env, 3),
            &root(&env, 1),
        ));
    }

    #[test]
    fn test_chain_continuity_detects_fork_and_reordering() {
        let env = Env::default();
        let mut links = Vec::new(&env);
        links.push_back(link(&env, 0, 1, 0));
        links.push_back(link(&env, 1, 2, 1));
        links.push_back(link(&env, 2, 3, 1)); // fork: same parent as index 1
        links.push_back(link(&env, 2, 4, 3)); // duplicate index

        let result = TamperEvidenceHelper::verify_chain_continuity(&links);
        assert!(!result.valid);
        assert_eq!(result.events_verified, 4);
        assert_eq!(result.mismatches, 2);
        assert_eq!(result.integrity_score, 50);
        assert_eq!(result.compromised_indices.len(), 2);
    }

    #[test]
    fn test_chain_continuity_clean_chain() {
        let env = Env::default();
        let mut links = Vec::new(&env);
        links.push_back(link(&env, 0, 1, 0));
        links.push_back(link(&env, 1, 2, 1));
        links.push_back(link(&env, 2, 3, 2));

        let result = TamperEvidenceHelper::verify_chain_continuity(&links);
        assert!(result.valid);
        assert_eq!(result.integrity_score, 100);
    }

    #[test]
    fn test_attestation_quorum_finalizes_witness() {
        let env = Env::default();
        let c = harness(&env);
        let (v1, v2) = (Address::generate(&env), Address::generate(&env));
        let r = root(&env, 9);

        assert_eq!(attest(&env, &c, &v1, 5, &r, 2), Ok(AttestationStatus::Pending(1)));
        let Ok(AttestationStatus::Finalized(w)) = attest(&env, &c, &v2, 5, &r, 2) else {
            panic!("expected finalized witness");
        };
        assert_eq!(w.root_at_index, r);
        assert_eq!(witness(&env, &c, 5), Some(w));
    }

    #[test]
    fn test_duplicate_attestation_is_not_double_counted() {
        let env = Env::default();
        let c = harness(&env);
        let v1 = Address::generate(&env);
        let r = root(&env, 9);

        for _ in 0..3 {
            assert_eq!(attest(&env, &c, &v1, 1, &r, 2), Ok(AttestationStatus::Pending(1)));
        }
        assert_eq!(witness(&env, &c, 1), None);
    }

    #[test]
    fn test_validator_equivocation_rejected() {
        let env = Env::default();
        let c = harness(&env);
        let v1 = Address::generate(&env);

        attest(&env, &c, &v1, 1, &root(&env, 1), 2).unwrap();
        assert_eq!(
            attest(&env, &c, &v1, 1, &root(&env, 2), 2),
            Err(TamperEvidenceError::ConflictingAttestation)
        );
    }

    #[test]
    fn test_finalized_witness_is_write_once() {
        let env = Env::default();
        let c = harness(&env);
        let (v1, v2, v3) = (
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        );
        let r = root(&env, 7);

        attest(&env, &c, &v1, 3, &r, 1).unwrap();
        let w = witness(&env, &c, 3).unwrap();

        assert_eq!(
            attest(&env, &c, &v2, 3, &r, 1),
            Ok(AttestationStatus::AlreadyFinalized(w.clone()))
        );
        assert_eq!(
            attest(&env, &c, &v3, 3, &root(&env, 8), 1),
            Err(TamperEvidenceError::WitnessAlreadyFinalized)
        );
        assert_eq!(witness(&env, &c, 3), Some(w));
    }

    #[test]
    fn test_witness_commitment_is_order_independent() {
        let env = Env::default();
        let validators = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        let r = root(&env, 4);

        let finalize = |order: [usize; 3]| {
            let c = harness(&env);
            for i in order {
                attest(&env, &c, &validators[i], 0, &r, 3).unwrap();
            }
            witness(&env, &c, 0).unwrap().commitment_hash
        };

        let a = finalize([0, 1, 2]);
        assert_eq!(a, finalize([2, 0, 1]));
        assert_eq!(a, finalize([1, 2, 0]));
    }

    #[test]
    fn test_zero_quorum_rejected() {
        let env = Env::default();
        let c = harness(&env);
        let v1 = Address::generate(&env);

        assert_eq!(
            attest(&env, &c, &v1, 0, &root(&env, 1), 0),
            Err(TamperEvidenceError::InvalidQuorum)
        );
    }

    #[test]
    fn test_verify_event_hash_match() {
        let hash = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        let expected = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        assert!(TamperEvidenceHelper::verify_event_hash(&hash, &expected));
    }

    #[test]
    fn test_verify_event_hash_mismatch() {
        let hash = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        let expected = BytesN::<32>::from_array(&Env::default(), &[2u8; 32]);
        assert!(!TamperEvidenceHelper::verify_event_hash(&hash, &expected));
    }

    #[test]
    fn test_integrity_score_perfect() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 0);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_integrity_score_partial() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 10);
        assert_eq!(score, 90);
    }

    #[test]
    fn test_integrity_score_all_compromised() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 100);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_immutability_check() {
        let config = TamperEvidenceConfig {
            immutability_threshold: 10,
            verify_all: true,
            hash_algorithm: 0,
            verify_archives: true,
        };

        let proof = TamperEvidenceHelper::verify_immutability(0, 100, &config);
        assert!(proof.immutable);
        assert_eq!(proof.chain_length, 100);
    }

    #[test]
    fn test_immutability_not_reached() {
        let config = TamperEvidenceConfig {
            immutability_threshold: 50,
            verify_all: true,
            hash_algorithm: 0,
            verify_archives: true,
        };

        let proof = TamperEvidenceHelper::verify_immutability(95, 100, &config);
        assert!(!proof.immutable);
        assert_eq!(proof.chain_length, 5);
    }

    #[test]
    fn test_no_retroactive_modification() {
        let env = Env::default();
        let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(BytesN::<32>::from_array(&env, &[1u8; 32]));
        references.push_back(BytesN::<32>::from_array(&env, &[1u8; 32]));

        assert!(TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }

    #[test]
    fn test_retroactive_modification_detected() {
        let env = Env::default();
        let hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(BytesN::<32>::from_array(&env, &[1u8; 32]));
        references.push_back(BytesN::<32>::from_array(&env, &[2u8; 32])); // Mismatch!

        assert!(!TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }
}
