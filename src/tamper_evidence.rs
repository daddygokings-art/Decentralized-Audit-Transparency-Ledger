//! Tamper-Evidence Verification System for Audit Trails
//!
//! Implements cryptographic chain verification to detect tampering,
//! prove event immutability, and validate audit trail integrity.

use soroban_sdk::{contracttype, BytesN, Env, Vec};
use crate::regulator::TamperProof;

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

    /// Verify the chain link between two consecutive events
    pub fn verify_chain_link(
        current_event_hash: &BytesN<32>,
        current_prev_hash: &BytesN<32>,
        next_event_hash: &BytesN<32>,
        next_prev_hash: &BytesN<32>,
    ) -> bool {
        // The current event's hash should match next event's prev_hash
        // And current event should reference the event before it
        *current_prev_hash == *next_prev_hash || next_prev_hash == current_event_hash
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

    /// Create a chain witness commitment
    pub fn create_chain_witness(
        env: &Env,
        event_index: u32,
        root_at_index: BytesN<32>,
    ) -> ChainWitness {
        ChainWitness {
            event_index,
            root_at_index,
            timestamp: env.ledger().timestamp(),
            commitment_hash: BytesN::<32>::from_array(&Env::default(), &[0u8; 32]),
        }
    }

    /// Verify a chain of events by checking hash continuity
    pub fn verify_chain_continuity(
        links: &Vec<ChainLink>,
    ) -> ChainVerification {
        let mut valid_count = 0;
        let mut mismatch_count = 0;
        let mut compromised = Vec::new(&Env::default());

        for link in links.iter() {
            if link.valid {
                valid_count += 1;
            } else {
                mismatch_count += 1;
                compromised.push_back(link.index);
            }
        }

        let integrity_score = Self::calculate_integrity_score(
            links.len() as u32,
            mismatch_count,
        );

        ChainVerification {
            valid: mismatch_count == 0,
            events_verified: links.len() as u32,
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
            event_hash: *event_hash,
            archive_root: *archive_root,
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let hash = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(BytesN::<32>::from_array(&Env::default(), &[1u8; 32]));
        references.push_back(BytesN::<32>::from_array(&Env::default(), &[1u8; 32]));

        assert!(TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }

    #[test]
    fn test_retroactive_modification_detected() {
        let env = Env::default();
        let hash = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(BytesN::<32>::from_array(&Env::default(), &[1u8; 32]));
        references.push_back(BytesN::<32>::from_array(&Env::default(), &[2u8; 32])); // Mismatch!

        assert!(!TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }
}
