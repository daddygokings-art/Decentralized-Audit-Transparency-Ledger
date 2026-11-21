#![cfg(test)]

use crate::zk_verifier::{ZkProof, ZkVerifier, ZkVerifierError};
use soroban_sdk::{BytesN, Env, Vec};

fn sample_proof(env: &Env, root: &BytesN<32>) -> ZkProof {
    let mut inputs = Vec::new(env);
    inputs.push_back(root.clone());
    ZkProof {
        a: BytesN::from_array(env, &[0x11; 32]),
        b: BytesN::from_array(env, &[0x22; 64]),
        c: BytesN::from_array(env, &[0x33; 32]),
        public_inputs: inputs,
    }
}

#[test]
fn test_fuzz_byzantine_empty_public_inputs() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0xAA; 32]);
    let mut proof = sample_proof(&env, &root);
    proof.public_inputs = Vec::new(&env);

    let res = ZkVerifier::verify_membership_proof(&env, &proof, &root);
    assert_eq!(res, Err(ZkVerifierError::PublicInputsMismatch));
}

#[test]
fn test_fuzz_byzantine_mismatched_commitment_root() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0xAA; 32]);
    let forged_root = BytesN::from_array(&env, &[0xBB; 32]);
    let proof = sample_proof(&env, &root);

    let res = ZkVerifier::verify_membership_proof(&env, &proof, &forged_root);
    assert_eq!(res, Err(ZkVerifierError::VerificationFailed));
}

#[test]
fn test_fuzz_valid_membership_proof_verification() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0xAA; 32]);
    let proof = sample_proof(&env, &root);

    let res = ZkVerifier::verify_membership_proof(&env, &proof, &root);
    assert_eq!(res, Ok(true));
}
