#![cfg(test)]

use crate::zk_verifier::{SignedZkAttestation, ZkProof, ZkVerifier, ZkVerifierError, MAX_BATCH_SIZE};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};

fn sample_proof(env: &Env, root: &BytesN<32>) -> ZkProof {
    let mut inputs = Vec::new(env);
    inputs.push_back(root.clone());
    inputs.push_back(BytesN::from_array(env, &[0x42; 32]));

    ZkProof {
        a: BytesN::from_array(env, &[0x01; 32]),
        b: BytesN::from_array(env, &[0x02; 64]),
        c: BytesN::from_array(env, &[0x03; 32]),
        public_inputs: inputs,
    }
}

#[test]
fn test_proof_digest_deterministic() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0x11; 32]);
    let proof = sample_proof(&env, &root);

    let d1 = ZkVerifier::compute_digest(&env, &proof);
    let d2 = ZkVerifier::compute_digest(&env, &proof);
    assert_eq!(d1, d2);
}

#[test]
fn test_verify_with_anti_replay_success_and_replay_detection() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0x22; 32]);
    let proof = sample_proof(&env, &root);

    // Initial verification should succeed
    let res = ZkVerifier::verify_with_anti_replay(&env, &proof, &root);
    assert!(res.is_ok());

    // Subsequent verification of the same proof must fail with ProofReplayDetected
    let replay_res = ZkVerifier::verify_with_anti_replay(&env, &proof, &root);
    assert_eq!(replay_res, Err(ZkVerifierError::ProofReplayDetected));
}

#[test]
fn test_verify_with_invalid_commitment_root() {
    let env = Env::default();
    let root = BytesN::from_array(&env, &[0x22; 32]);
    let forged_root = BytesN::from_array(&env, &[0x33; 32]);
    let proof = sample_proof(&env, &root);

    let res = ZkVerifier::verify_with_anti_replay(&env, &proof, &forged_root);
    assert_eq!(res, Err(ZkVerifierError::VerificationFailed));
}

#[test]
fn test_verify_batch_proofs_and_bounds() {
    let env = Env::default();
    let mut proofs = Vec::new(&env);
    let mut roots = Vec::new(&env);

    for i in 0..4u8 {
        let root = BytesN::from_array(&env, &[i; 32]);
        proofs.push_back(sample_proof(&env, &root));
        roots.push_back(root);
    }

    let count = ZkVerifier::verify_batch_proofs(&env, &proofs, &roots).unwrap();
    assert_eq!(count, 4);

    // Empty batch fails with BatchSizeMismatch
    let empty_proofs = Vec::new(&env);
    let empty_roots = Vec::new(&env);
    assert_eq!(
        ZkVerifier::verify_batch_proofs(&env, &empty_proofs, &empty_roots),
        Err(ZkVerifierError::BatchSizeMismatch)
    );
}

#[test]
fn test_record_simultaneous_attestation_race() {
    let env = Env::default();
    let validator = Address::generate(&env);
    let digest = BytesN::from_array(&env, &[0x55; 32]);

    // First sequence expectation matches current (0)
    let s1 = ZkVerifier::record_simultaneous_attestation(&env, &validator, &digest, 0).unwrap();
    assert_eq!(s1, 1);

    // Concurrent/stale validator trying with sequence 0 gets race detected
    let stale = ZkVerifier::record_simultaneous_attestation(&env, &validator, &digest, 0);
    assert_eq!(stale, Err(ZkVerifierError::AttestationRaceDetected));

    // Next sequential validator succeeds with sequence 1
    let s2 = ZkVerifier::record_simultaneous_attestation(&env, &validator, &digest, 1).unwrap();
    assert_eq!(s2, 2);
}

#[test]
fn test_signed_attestation_verification_and_anti_replay() {
    let env = Env::default();
    let validator = Address::generate(&env);
    let proof_digest = BytesN::from_array(&env, &[0x66; 32]);
    let commitment_root = BytesN::from_array(&env, &[0x77; 32]);

    let signing_key = SigningKey::from_bytes(&[0x42; 32]);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes: [u8; 32] = verifying_key.to_bytes();
    let public_key = BytesN::from_array(&env, &pubkey_bytes);

    let mut trusted = Vec::new(&env);
    trusted.push_back(public_key.clone());

    let mut attestation = SignedZkAttestation {
        validator: validator.clone(),
        proof_digest: proof_digest.clone(),
        commitment_root: commitment_root.clone(),
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        valid_until_ledger: env.ledger().sequence() + 100,
        public_key: public_key.clone(),
        signature: BytesN::from_array(&env, &[0u8; 64]),
    };

    let digest = ZkVerifier::compute_signed_attestation_digest(&env, &attestation);
    let mut digest_arr = [0u8; 32];
    digest.copy_into_slice(&mut digest_arr);
    let sig = signing_key.sign(&digest_arr);
    attestation.signature = BytesN::from_array(&env, &sig.to_bytes());

    // Valid signed attestation passes
    let verified = ZkVerifier::verify_signed_attestation(&env, &attestation, &trusted);
    assert!(verified.is_ok());

    // Replay attack with same nonce is rejected
    let replay = ZkVerifier::verify_signed_attestation(&env, &attestation, &trusted);
    assert_eq!(replay, Err(ZkVerifierError::ProofReplayDetected));

    // Stale/lower nonce is rejected
    attestation.nonce = 1;
    let stale = ZkVerifier::verify_signed_attestation(&env, &attestation, &trusted);
    assert_eq!(stale, Err(ZkVerifierError::ProofReplayDetected));

    // Monotonic next nonce succeeds with new signature
    attestation.nonce = 2;
    let next_digest = ZkVerifier::compute_signed_attestation_digest(&env, &attestation);
    let mut next_digest_arr = [0u8; 32];
    next_digest.copy_into_slice(&mut next_digest_arr);
    let next_sig = signing_key.sign(&next_digest_arr);
    attestation.signature = BytesN::from_array(&env, &next_sig.to_bytes());

    let next_verified = ZkVerifier::verify_signed_attestation(&env, &attestation, &trusted);
    assert!(next_verified.is_ok());
}

#[test]
fn test_signed_attestation_unauthorized_validator() {
    let env = Env::default();
    let validator = Address::generate(&env);
    let proof_digest = BytesN::from_array(&env, &[0x88; 32]);
    let commitment_root = BytesN::from_array(&env, &[0x99; 32]);

    let public_key = BytesN::from_array(&env, &[0xAA; 32]);
    let empty_trusted = Vec::new(&env);

    let attestation = SignedZkAttestation {
        validator,
        proof_digest,
        commitment_root,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        valid_until_ledger: env.ledger().sequence() + 10,
        public_key,
        signature: BytesN::from_array(&env, &[0u8; 64]),
    };

    let res = ZkVerifier::verify_signed_attestation(&env, &attestation, &empty_trusted);
    assert_eq!(res, Err(ZkVerifierError::UnauthorizedValidator));
}
