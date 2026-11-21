//! Domain-separated binary Merkle tree helpers.
//!
//! Used to commit to append-only, hash-chained logs (RWA oracle price
//! history, CQRS event store) so that a single 32-byte root can be published
//! and any individual entry proven against it.
//!
//! # Construction
//!
//! * Leaf hash: `sha256(0x00 || leaf)`; interior node: `sha256(0x01 || left || right)`.
//!   The prefixes domain-separate leaves from interior nodes, so an interior
//!   node can never be passed off as a leaf (second-preimage resistance).
//! * An odd node at the end of a level is **promoted** unchanged to the next
//!   level instead of being paired with a copy of itself. Duplicating it
//!   would let two different leaf lists share a root (the CVE-2012-2459
//!   mutation); promotion does not.
//! * The root of an empty list is 32 zero bytes.
//!
//! Verification replays the tree shape from `(index, leaf_count)`, so a
//! proof must consume exactly the expected number of siblings. Every helper
//! is pure and deterministic: identical inputs give identical roots on every
//! host.

use soroban_sdk::{contracterror, Bytes, BytesN, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MerkleError {
    LeafIndexOutOfRange = 6001,
    ProofMalformed = 6002,
    RootMismatch = 6003,
    EmptyLeaves = 6004,
}

/// Domain-separation prefix for leaf hashes.
pub const LEAF_PREFIX: u8 = 0x00;
/// Domain-separation prefix for interior node hashes.
pub const NODE_PREFIX: u8 = 0x01;
/// Domain-separation prefix for sparse Merkle tree leaf hashes.
pub const SMT_LEAF_PREFIX: u8 = 0x02;
/// Domain-separation prefix for sparse Merkle tree empty/non-inclusion hashes.
pub const SMT_EMPTY_PREFIX: u8 = 0x03;

/// `sha256(0x00 || leaf)`.
pub fn hash_leaf(env: &Env, leaf: &BytesN<32>) -> BytesN<32> {
    let mut buf = Bytes::from_array(env, &[LEAF_PREFIX]);
    buf.append(&leaf.clone().into());
    env.crypto().sha256(&buf).to_bytes()
}

/// `sha256(0x01 || left || right)`.
pub fn hash_node(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    let mut buf = Bytes::from_array(env, &[NODE_PREFIX]);
    buf.append(&left.clone().into());
    buf.append(&right.clone().into());
    env.crypto().sha256(&buf).to_bytes()
}

fn hashed_leaves(env: &Env, leaves: &Vec<BytesN<32>>) -> Vec<BytesN<32>> {
    let mut level = Vec::new(env);
    for leaf in leaves.iter() {
        level.push_back(hash_leaf(env, &leaf));
    }
    level
}

fn next_level(env: &Env, level: &Vec<BytesN<32>>) -> Vec<BytesN<32>> {
    let mut next = Vec::new(env);
    let len = level.len();
    let mut i = 0u32;
    while i < len {
        if i + 1 < len {
            next.push_back(hash_node(
                env,
                &level.get_unchecked(i),
                &level.get_unchecked(i + 1),
            ));
        } else {
            // Odd node: promote unchanged.
            next.push_back(level.get_unchecked(i));
        }
        i += 2;
    }
    next
}

/// Merkle root over `leaves` (zero bytes for an empty list).
pub fn merkle_root(env: &Env, leaves: &Vec<BytesN<32>>) -> BytesN<32> {
    if leaves.is_empty() {
        return BytesN::from_array(env, &[0u8; 32]);
    }
    let mut level = hashed_leaves(env, leaves);
    while level.len() > 1 {
        level = next_level(env, &level);
    }
    level.get_unchecked(0)
}

/// Sibling path for `leaves[index]`, from the leaf level up to the root.
/// Returns `None` when `index` is out of range.
pub fn merkle_proof(env: &Env, leaves: &Vec<BytesN<32>>, index: u32) -> Option<Vec<BytesN<32>>> {
    if index >= leaves.len() {
        return None;
    }
    let mut level = hashed_leaves(env, leaves);
    let mut idx = index;
    let mut proof = Vec::new(env);
    while level.len() > 1 {
        let sibling = idx ^ 1;
        if sibling < level.len() {
            proof.push_back(level.get_unchecked(sibling));
        }
        level = next_level(env, &level);
        idx /= 2;
    }
    Some(proof)
}

/// Verify that `leaf` sits at `index` in a tree of `leaf_count` leaves with
/// the given `root`. The proof must contain exactly the siblings the tree
/// shape requires; missing or surplus siblings fail verification.
pub fn verify_merkle_proof(
    env: &Env,
    leaf: &BytesN<32>,
    index: u32,
    leaf_count: u32,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    if index >= leaf_count {
        return false;
    }
    let mut hash = hash_leaf(env, leaf);
    let mut idx = index;
    let mut width = leaf_count;
    let mut used = 0u32;
    while width > 1 {
        let sibling = idx ^ 1;
        if sibling < width {
            let Some(sib) = proof.get(used) else {
                return false;
            };
            used += 1;
            hash = if idx & 1 == 0 {
                hash_node(env, &hash, &sib)
            } else {
                hash_node(env, &sib, &hash)
            };
        }
        idx /= 2;
        width = width.div_ceil(2);
    }
    used == proof.len() && hash == *root
}

/// Verify a Merkle proof with structured monitoring events and modern error propagation.
pub fn verify_merkle_proof_with_events(
    env: &Env,
    leaf: &BytesN<32>,
    index: u32,
    leaf_count: u32,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> Result<bool, MerkleError> {
    if index >= leaf_count {
        return Err(MerkleError::LeafIndexOutOfRange);
    }
    let ok = verify_merkle_proof(env, leaf, index, leaf_count, proof, root);
    if ok {
        env.events().publish(
            (Symbol::new(env, "merkle_tree"), Symbol::new(env, "proof_verified")),
            (index, leaf.clone(), root.clone()),
        );
        Ok(true)
    } else {
        env.events().publish(
            (Symbol::new(env, "merkle_tree"), Symbol::new(env, "proof_failed")),
            (index, leaf.clone(), root.clone()),
        );
        Err(MerkleError::RootMismatch)
    }
/// Hash a key-value pair for Sparse Merkle Tree (SMT): `sha256(0x02 || key || value)`.
pub fn hash_smt_leaf(env: &Env, key: &BytesN<32>, value: &BytesN<32>) -> BytesN<32> {
    let mut buf = Bytes::from_array(env, &[SMT_LEAF_PREFIX]);
    buf.append(&key.clone().into());
    buf.append(&value.clone().into());
    env.crypto().sha256(&buf).to_bytes()
}

/// Hash an empty leaf for Sparse Merkle Tree (SMT): `sha256(0x03 || key)`.
pub fn hash_smt_empty_leaf(env: &Env, key: &BytesN<32>) -> BytesN<32> {
    let mut buf = Bytes::from_array(env, &[SMT_EMPTY_PREFIX]);
    buf.append(&key.clone().into());
    env.crypto().sha256(&buf).to_bytes()
}

/// Verify a Sparse Merkle Tree (SMT) membership proof.
/// Reconstructs the root along the bit-path given by `key`.
pub fn verify_smt_proof(
    env: &Env,
    key: &BytesN<32>,
    value: &BytesN<32>,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    let mut current = hash_smt_leaf(env, key, value);
    let key_bytes: [u8; 32] = key.to_array();
    let proof_len = proof.len();

    for i in 0..proof_len {
        let byte_idx = (i / 8) as usize;
        let bit_idx = (i % 8) as u8;
        let is_right_child = (key_bytes[byte_idx] >> (7 - bit_idx)) & 1 == 1;

        let Some(sibling) = proof.get(i) else {
            return false;
        };

        current = if is_right_child {
            hash_node(env, &sibling, &current)
        } else {
            hash_node(env, &current, &sibling)
        };
    }

    current == *root
}

/// Verify a Sparse Merkle Tree (SMT) non-inclusion (absence) proof.
/// Proves that `key` does not exist or has been deleted/archived in the tree.
pub fn verify_smt_non_inclusion_proof(
    env: &Env,
    key: &BytesN<32>,
    proof: &Vec<BytesN<32>>,
    root: &BytesN<32>,
) -> bool {
    let mut current = hash_smt_empty_leaf(env, key);
    let key_bytes: [u8; 32] = key.to_array();
    let proof_len = proof.len();

    for i in 0..proof_len {
        let byte_idx = (i / 8) as usize;
        let bit_idx = (i % 8) as u8;
        let is_right_child = (key_bytes[byte_idx] >> (7 - bit_idx)) & 1 == 1;

        let Some(sibling) = proof.get(i) else {
            return false;
        };

        current = if is_right_child {
            hash_node(env, &sibling, &current)
        } else {
            hash_node(env, &current, &sibling)
        };
    }

    current == *root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(env: &Env, n: u32) -> Vec<BytesN<32>> {
        let mut v = Vec::new(env);
        for i in 0..n {
            let mut b = [0u8; 32];
            b[..4].copy_from_slice(&i.to_be_bytes());
            v.push_back(BytesN::from_array(env, &b));
        }
        v
    }

    #[test]
    fn empty_root_is_zero_and_has_no_proofs() {
        let env = Env::default();
        let empty = Vec::new(&env);
        assert_eq!(merkle_root(&env, &empty), BytesN::from_array(&env, &[0u8; 32]));
        assert!(merkle_proof(&env, &empty, 0).is_none());
    }

    #[test]
    fn single_leaf_root_is_leaf_hash() {
        let env = Env::default();
        let l = leaves(&env, 1);
        let root = merkle_root(&env, &l);
        assert_eq!(root, hash_leaf(&env, &l.get_unchecked(0)));
        let proof = merkle_proof(&env, &l, 0).unwrap();
        assert!(proof.is_empty());
        assert!(verify_merkle_proof(&env, &l.get_unchecked(0), 0, 1, &proof, &root));
    }

    #[test]
    fn every_leaf_proves_for_sizes_1_to_17() {
        let env = Env::default();
        for n in 1..=17u32 {
            let l = leaves(&env, n);
            let root = merkle_root(&env, &l);
            for i in 0..n {
                let proof = merkle_proof(&env, &l, i).unwrap();
                assert!(
                    verify_merkle_proof(&env, &l.get_unchecked(i), i, n, &proof, &root),
                    "leaf {i} of {n}"
                );
                // Wrong position fails.
                let wrong = (i + 1) % n;
                if wrong != i {
                    assert!(!verify_merkle_proof(&env, &l.get_unchecked(i), wrong, n, &proof, &root));
                }
            }
            assert!(merkle_proof(&env, &l, n).is_none());
        }
    }

    #[test]
    fn tampering_is_detected() {
        let env = Env::default();
        let l = leaves(&env, 5);
        let root = merkle_root(&env, &l);
        let proof = merkle_proof(&env, &l, 2).unwrap();
        let forged = BytesN::from_array(&env, &[0xFF; 32]);

        // Tampered leaf.
        assert!(!verify_merkle_proof(&env, &forged, 2, 5, &proof, &root));
        // Tampered sibling.
        let mut bad = proof.clone();
        bad.set(0, forged.clone());
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 2, 5, &bad, &root));
        // Surplus sibling.
        let mut long = proof.clone();
        long.push_back(forged.clone());
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 2, 5, &long, &root));
        // Missing sibling.
        let mut short = proof.clone();
        short.pop_back();
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 2, 5, &short, &root));
        // Wrong leaf count / out-of-range index.
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 2, 4, &proof, &root));
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 5, 5, &proof, &root));
        // Wrong root.
        assert!(!verify_merkle_proof(&env, &l.get_unchecked(2), 2, 5, &proof, &forged));
    }

    #[test]
    fn odd_node_promotion_prevents_duplicate_last_leaf_collision() {
        let env = Env::default();
        let three = leaves(&env, 3);
        let mut four = three.clone();
        four.push_back(three.get_unchecked(2));
        assert_ne!(merkle_root(&env, &three), merkle_root(&env, &four));
    }

    #[test]
    fn leaf_and_node_hashes_are_domain_separated() {
        let env = Env::default();
        let l = leaves(&env, 2);
        let node = hash_node(&env, &l.get_unchecked(0), &l.get_unchecked(1));
        // A root computed over two leaves cannot be reproduced by treating
        // the raw concatenation as a single leaf.
        assert_ne!(hash_leaf(&env, &node), node);
        assert_ne!(merkle_root(&env, &l), hash_node(&env, &l.get_unchecked(0), &l.get_unchecked(1)));
    }

    #[test]
    fn smt_proof_verification_and_non_inclusion() {
        let env = Env::default();
        let key = BytesN::from_array(&env, &[0x42; 32]);
        let val = BytesN::from_array(&env, &[0xAA; 32]);
        let sib1 = BytesN::from_array(&env, &[0x01; 32]);
        let sib2 = BytesN::from_array(&env, &[0x02; 32]);

        let mut proof = Vec::new(&env);
        proof.push_back(sib1.clone());
        proof.push_back(sib2.clone());

        // Reconstruct expected root for membership
        let mut cur = hash_smt_leaf(&env, &key, &val);
        let key_bytes: [u8; 32] = key.to_array();
        for i in 0..2 {
            let byte_idx = (i / 8) as usize;
            let bit_idx = (i % 8) as u8;
            let is_right = (key_bytes[byte_idx] >> (7 - bit_idx)) & 1 == 1;
            let sib = proof.get(i).unwrap();
            cur = if is_right {
                hash_node(&env, &sib, &cur)
            } else {
                hash_node(&env, &cur, &sib)
            };
        }
        let root = cur;

        assert!(verify_smt_proof(&env, &key, &val, &proof, &root));

        // Wrong value fails
        let wrong_val = BytesN::from_array(&env, &[0xBB; 32]);
        assert!(!verify_smt_proof(&env, &key, &wrong_val, &proof, &root));

        // Test non-inclusion
        let mut empty_cur = hash_smt_empty_leaf(&env, &key);
        for i in 0..2 {
            let byte_idx = (i / 8) as usize;
            let bit_idx = (i % 8) as u8;
            let is_right = (key_bytes[byte_idx] >> (7 - bit_idx)) & 1 == 1;
            let sib = proof.get(i).unwrap();
            empty_cur = if is_right {
                hash_node(&env, &sib, &empty_cur)
            } else {
                hash_node(&env, &empty_cur, &sib)
            };
        }
        let non_incl_root = empty_cur;
        assert!(verify_smt_non_inclusion_proof(&env, &key, &proof, &non_incl_root));
        assert!(!verify_smt_non_inclusion_proof(&env, &key, &proof, &root));
    }
}
