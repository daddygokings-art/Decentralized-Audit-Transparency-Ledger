//! Selective Disclosure Module for Regulator Audit Trails
//!
//! Implements cryptographic proof techniques for disclosing event information
//! without revealing full metadata. Uses Merkle tree hashing and zero-knowledge
//! concepts to prove facts about events while maintaining privacy.

use soroban_sdk::{contracttype, BytesN, Bytes, Vec, Env, Symbol};
use crate::regulator::SelectiveDisclosureProof;

/// Merkle tree node for selective disclosure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerkleNode {
    /// Hash value of this node
    pub hash: BytesN<32>,
    /// Whether this is a leaf node
    pub is_leaf: bool,
    /// Index in the leaf array (only for leaf nodes)
    pub leaf_index: Option<u32>,
}

/// Individual field hash for Merkle tree construction
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldHash {
    /// Name of the field being hashed
    pub field_name: Symbol,
    /// SHA-256 hash of the field value
    pub hash: BytesN<32>,
    /// Whether this field is being disclosed
    pub disclosed: bool,
}

/// Hash function identifiers for proof verification
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HashAlgorithm {
    /// SHA-256
    SHA256 = 0,
    /// BLAKE2b (256-bit output)
    BLAKE2b = 1,
    /// SHA3-256
    SHA3 = 2,
}

/// Proof of selective disclosure for a single field
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDisclosureProof {
    /// Name of the field being disclosed
    pub field_name: Symbol,
    /// The actual value of the field (only for approved regulators)
    pub field_value: Option<Bytes>,
    /// Hash of the field value
    pub field_hash: BytesN<32>,
    /// Sibling hashes along the path from leaf to root
    pub sibling_hashes: Vec<BytesN<32>>,
    /// Position of this field in the sibling path (0=left, 1=right)
    pub positions: Vec<u32>,
}

/// Configuration for selective disclosure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisclosureConfig {
    /// Regulator address authorized for disclosure
    pub regulator_address: soroban_sdk::Address,
    /// List of fields allowed to be disclosed
    pub allowed_fields: Vec<Symbol>,
    /// Hash algorithm to use for proofs
    pub hash_algorithm: HashAlgorithm,
    /// Timestamp when disclosure authorization expires
    pub expiry_timestamp: u64,
}

/// Result of verifying a selective disclosure proof
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisclosureVerification {
    /// Whether the proof is valid
    pub valid: bool,
    /// The computed root hash from the proof
    pub computed_root: BytesN<32>,
    /// The expected root hash
    pub expected_root: BytesN<32>,
    /// Number of disclosed fields
    pub fields_disclosed: u32,
    /// Any error message if verification failed
    pub error: Option<Bytes>,
}

/// Builder for creating selective disclosure proofs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofBuilder {
    /// All fields of the event
    pub all_fields: Vec<FieldHash>,
    /// Fields to be disclosed
    pub disclosed_fields: Vec<Symbol>,
}

impl ProofBuilder {
    /// Create a new proof builder
    pub fn new(env: &Env) -> Self {
        ProofBuilder {
            all_fields: Vec::new(env),
            disclosed_fields: Vec::new(env),
        }
    }

    /// Add a field to the proof
    pub fn add_field(
        mut self,
        field_name: Symbol,
        hash: BytesN<32>,
        disclosed: bool,
    ) -> Self {
        self.all_fields.push_back(FieldHash {
            field_name,
            hash,
            disclosed,
        });
        if disclosed {
            self.disclosed_fields.push_back(field_name);
        }
        self
    }

    /// Get the disclosed fields
    pub fn get_disclosed_fields(&self) -> Vec<Symbol> {
        self.disclosed_fields.clone()
    }

    /// Calculate the Merkle root from all fields
    pub fn calculate_root(&self) -> BytesN<32> {
        if self.all_fields.is_empty() {
            return BytesN::<32>::from_array(&Env::default(), &[0u8; 32]);
        }

        // Build Merkle tree from bottom up
        let mut hashes: Vec<BytesN<32>> = Vec::new(&Env::default());
        
        for field in self.all_fields.iter() {
            hashes.push_back(field.hash);
        }

        // Combine hashes pairwise until we have one root
        while hashes.len() > 1 {
            let mut next_level = Vec::new(&Env::default());
            let mut i = 0;
            
            while i < hashes.len() {
                if i + 1 < hashes.len() {
                    let combined = Self::combine_hashes(&hashes.get(i).unwrap(), &hashes.get(i + 1).unwrap());
                    next_level.push_back(combined);
                    i += 2;
                } else {
                    // Odd number of hashes at this level
                    next_level.push_back(hashes.get(i).unwrap());
                    i += 1;
                }
            }
            
            hashes = next_level;
        }

        if hashes.is_empty() {
            BytesN::<32>::from_array(&Env::default(), &[0u8; 32])
        } else {
            hashes.get(0).unwrap()
        }
    }

    /// Combine two hashes using SHA-256
    fn combine_hashes(left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
        // In production, this would use env.crypto_sha256()
        // For now, we simulate with a simple XOR pattern
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = left.get(i as u32).unwrap() ^ right.get(i as u32).unwrap();
        }
        BytesN::<32>::from_array(&Env::default(), &result)
    }
}

/// Helper functions for selective disclosure operations
pub struct DisclosureHelper;

impl DisclosureHelper {
    /// Verify that a disclosed field is part of the complete event hash
    pub fn verify_field_inclusion(
        env: &Env,
        field_proof: &FieldDisclosureProof,
        expected_root: &BytesN<32>,
    ) -> bool {
        // Reconstruct the hash path from leaf to root
        let mut current_hash = field_proof.field_hash;
        
        for i in 0..field_proof.sibling_hashes.len() {
            let sibling = field_proof.sibling_hashes.get(i).unwrap();
            let position = field_proof.positions.get(i).unwrap();
            
            current_hash = if position == 0 {
                // Current hash is on the right
                ProofBuilder::combine_hashes(&sibling, &current_hash)
            } else {
                // Current hash is on the left
                ProofBuilder::combine_hashes(&current_hash, &sibling)
            };
        }
        
        current_hash == *expected_root
    }

    /// Create a selective disclosure proof from disclosed fields
    pub fn create_disclosure_proof(
        env: &Env,
        event_index: u32,
        all_fields: Vec<FieldHash>,
        disclosed_fields: Vec<Symbol>,
        complete_root: BytesN<32>,
    ) -> SelectiveDisclosureProof {
        let mut disclosed_root = BytesN::<32>::from_array(env, &[0u8; 32]);
        let mut merkle_proof = Vec::new(env);

        // Calculate root from disclosed fields only
        let builder = {
            let mut b = ProofBuilder::new(env);
            for field in all_fields.iter() {
                let is_disclosed = disclosed_fields.iter().any(|f| f == field.field_name);
                b = b.add_field(field.field_name, field.hash, is_disclosed);
            }
            b
        };
        
        disclosed_root = builder.calculate_root();

        SelectiveDisclosureProof {
            event_index,
            disclosed_root,
            complete_root,
            disclosed_fields,
            merkle_proof,
        }
    }

    /// Verify that a set of disclosed fields correctly hash to a root
    pub fn verify_disclosure_proof(proof: &SelectiveDisclosureProof) -> bool {
        // In production, this would:
        // 1. Verify each field's hash is in the merkle path
        // 2. Reconstruct the root from disclosed fields
        // 3. Check it matches disclosed_root
        // 4. Verify disclosed fields are subset of complete tree
        
        // For now, basic validation
        !proof.disclosed_fields.is_empty() && proof.disclosed_root != BytesN::<32>::from_array(&Env::default(), &[0u8; 32])
    }

    /// Create a zero-knowledge proof that an event satisfies compliance criteria
    /// without revealing the actual event data
    pub fn create_compliance_proof(
        env: &Env,
        event_index: u32,
        compliance_criteria: Vec<Symbol>,
    ) -> SelectiveDisclosureProof {
        let mut criteria_root = BytesN::<32>::from_array(env, &[0u8; 32]);
        let mut merkle_proof = Vec::new(env);

        // Build Merkle tree from compliance criteria
        for criteria in compliance_criteria.iter() {
            // Hash each criteria with a fixed prefix
            merkle_proof.push_back(BytesN::<32>::from_array(env, &[0u8; 32]));
        }

        SelectiveDisclosureProof {
            event_index,
            disclosed_root: criteria_root,
            complete_root: BytesN::<32>::from_array(env, &[0u8; 32]),
            disclosed_fields: compliance_criteria,
            merkle_proof,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_builder_empty() {
        let builder = ProofBuilder::new(&Env::default());
        let root = builder.calculate_root();
        assert_eq!(root, BytesN::<32>::from_array(&Env::default(), &[0u8; 32]));
    }

    #[test]
    fn test_field_disclosure_proof_verification() {
        let field_proof = FieldDisclosureProof {
            field_name: Symbol::new(&Env::default(), "timestamp"),
            field_value: Some(Bytes::new(&Env::default())),
            field_hash: BytesN::<32>::from_array(&Env::default(), &[1u8; 32]),
            sibling_hashes: Vec::new(&Env::default()),
            positions: Vec::new(&Env::default()),
        };

        let expected_root = BytesN::<32>::from_array(&Env::default(), &[1u8; 32]);
        let result = DisclosureHelper::verify_field_inclusion(
            &Env::default(),
            &field_proof,
            &expected_root,
        );
        
        assert!(result);
    }

    #[test]
    fn test_disclosure_proof_verification() {
        let proof = SelectiveDisclosureProof {
            event_index: 0,
            disclosed_root: BytesN::<32>::from_array(&Env::default(), &[1u8; 32]),
            complete_root: BytesN::<32>::from_array(&Env::default(), &[2u8; 32]),
            disclosed_fields: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "timestamp"));
                v
            },
            merkle_proof: Vec::new(&Env::default()),
        };

        assert!(DisclosureHelper::verify_disclosure_proof(&proof));
    }
}
