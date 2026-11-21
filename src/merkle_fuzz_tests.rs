#![cfg(test)]

use crate::merkle::{merkle_proof, merkle_root, verify_merkle_proof};
use soroban_sdk::{BytesN, Env, Vec};

fn generate_leaves(env: &Env, n: u32) -> Vec<BytesN<32>> {
    let mut v = Vec::new(env);
    for i in 0..n {
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&i.to_be_bytes());
        v.push_back(BytesN::from_array(env, &b));
    }
    v
}

#[test]
fn test_byzantine_mutated_leaf_fuzzing() {
    let env = Env::default();
    let l = generate_leaves(&env, 8);
    let root = merkle_root(&env, &l);
    let proof = merkle_proof(&env, &l, 3).unwrap();

    // Byzantine validator presents mutated payload
    let forged_leaf = BytesN::from_array(&env, &[0xDE; 32]);
    assert!(!verify_merkle_proof(&env, &forged_leaf, 3, 8, &proof, &root));
}

#[test]
fn test_byzantine_sibling_permutation_fuzzing() {
    let env = Env::default();
    let l = generate_leaves(&env, 8);
    let root = merkle_root(&env, &l);
    let mut proof = merkle_proof(&env, &l, 2).unwrap();

    // Byzantine validator swaps siblings
    let sib0 = proof.get(0).unwrap();
    let sib1 = proof.get(1).unwrap();
    proof.set(0, sib1);
    proof.set(1, sib0);

    assert!(!verify_merkle_proof(&env, &l.get(2).unwrap(), 2, 8, &proof, &root));
}

#[test]
fn test_byzantine_out_of_bounds_index_fuzzing() {
    let env = Env::default();
    let l = generate_leaves(&env, 5);
    let root = merkle_root(&env, &l);
    let proof = merkle_proof(&env, &l, 2).unwrap();

    // Out-of-bounds leaf index
    assert!(!verify_merkle_proof(&env, &l.get(2).unwrap(), 10, 5, &proof, &root));
}

#[test]
fn test_byzantine_truncated_and_padded_proofs() {
    let env = Env::default();
    let l = generate_leaves(&env, 7);
    let root = merkle_root(&env, &l);
    let mut proof = merkle_proof(&env, &l, 1).unwrap();

    // Byzantine truncation
    let mut truncated = proof.clone();
    truncated.pop_back();
    assert!(!verify_merkle_proof(&env, &l.get(1).unwrap(), 1, 7, &truncated, &root));

    // Byzantine padding
    let mut padded = proof.clone();
    padded.push_back(BytesN::from_array(&env, &[0xAA; 32]));
    assert!(!verify_merkle_proof(&env, &l.get(1).unwrap(), 1, 7, &padded, &root));
}
