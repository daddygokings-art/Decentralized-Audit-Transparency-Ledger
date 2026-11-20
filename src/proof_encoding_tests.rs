//! Tests for canonical Groth16 / BN254 proof serialization.

use crate::proof_encoding::*;
use soroban_sdk::{Bytes, BytesN, Env, Vec};

const P: [u8; 32] = BN254_FQ_MODULUS;
const R: [u8; 32] = BN254_FR_MODULUS;

/// Big-endian 32-byte encoding of a small integer.
fn small(v: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[31] = v;
    out
}

/// `p + 1` (still non-canonical; aliases the field element 1).
fn p_plus_one() -> [u8; 32] {
    let mut out = P;
    out[31] += 1;
    out
}

/// `p - 1` — the largest canonical base-field element.
fn p_minus_one() -> [u8; 32] {
    let mut out = P;
    out[31] -= 1;
    out
}

/// Builds a 256-byte proof buffer from eight 32-byte coordinates.
fn raw_proof(coords: [[u8; 32]; 8]) -> [u8; 256] {
    let mut buf = [0u8; 256];
    for (i, c) in coords.iter().enumerate() {
        buf[i * 32..(i + 1) * 32].copy_from_slice(c);
    }
    buf
}

/// A canonically-encoded proof (G1 generator for A and C, small G2 coords).
fn valid_coords() -> [[u8; 32]; 8] {
    [
        small(1),
        small(2),
        small(3),
        small(4),
        small(5),
        p_minus_one(),
        small(1),
        small(2),
    ]
}

#[test]
fn decodes_and_round_trips_canonical_proof() {
    let env = Env::default();
    let raw = raw_proof(valid_coords());
    let bytes = Bytes::from_array(&env, &raw);

    let proof = decode_proof(&env, &bytes).expect("canonical proof must decode");
    assert_eq!(proof.a.x, BytesN::from_array(&env, &small(1)));
    assert_eq!(proof.b.y_c0, BytesN::from_array(&env, &p_minus_one()));

    let encoded = encode_proof(&env, &proof).expect("canonical proof must encode");
    assert_eq!(encoded, bytes, "encode(decode(b)) must equal b");
}

#[test]
fn rejects_wrong_length() {
    let env = Env::default();
    let raw = raw_proof(valid_coords());

    let short = Bytes::from_slice(&env, &raw[..255]);
    assert_eq!(decode_proof(&env, &short), Err(ProofEncodingError::InvalidLength));

    let mut long = Bytes::from_array(&env, &raw);
    long.push_back(0);
    assert_eq!(decode_proof(&env, &long), Err(ProofEncodingError::InvalidLength));

    assert_eq!(
        decode_proof(&env, &Bytes::new(&env)),
        Err(ProofEncodingError::InvalidLength)
    );
}

#[test]
fn rejects_coordinate_equal_to_modulus_in_every_slot() {
    let env = Env::default();
    for slot in 0..8 {
        let mut coords = valid_coords();
        coords[slot] = P;
        let bytes = Bytes::from_array(&env, &raw_proof(coords));
        assert_eq!(
            decode_proof(&env, &bytes),
            Err(ProofEncodingError::NonCanonicalFieldElement),
            "slot {slot} = p must be rejected"
        );
    }
}

#[test]
fn rejects_aliased_coordinates_above_modulus() {
    let env = Env::default();
    // `p + 1` aliases 1: accepting it would give two encodings of the same proof.
    let mut coords = valid_coords();
    coords[0] = p_plus_one();
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert_eq!(
        decode_proof(&env, &bytes),
        Err(ProofEncodingError::NonCanonicalFieldElement)
    );

    let mut coords = valid_coords();
    coords[3] = [0xff; 32];
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert_eq!(
        decode_proof(&env, &bytes),
        Err(ProofEncodingError::NonCanonicalFieldElement)
    );
}

#[test]
fn rejects_identity_points() {
    let env = Env::default();

    // A = infinity
    let mut coords = valid_coords();
    coords[0] = [0; 32];
    coords[1] = [0; 32];
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert_eq!(decode_proof(&env, &bytes), Err(ProofEncodingError::IdentityPoint));

    // B = infinity
    let mut coords = valid_coords();
    for c in coords.iter_mut().skip(2).take(4) {
        *c = [0; 32];
    }
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert_eq!(decode_proof(&env, &bytes), Err(ProofEncodingError::IdentityPoint));

    // C = infinity
    let mut coords = valid_coords();
    coords[6] = [0; 32];
    coords[7] = [0; 32];
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert_eq!(decode_proof(&env, &bytes), Err(ProofEncodingError::IdentityPoint));
}

#[test]
fn zero_coordinate_alone_is_not_identity() {
    let env = Env::default();
    let mut coords = valid_coords();
    coords[0] = [0; 32]; // x = 0, y = 2: canonical encoding, not infinity
    let bytes = Bytes::from_array(&env, &raw_proof(coords));
    assert!(decode_proof(&env, &bytes).is_ok());
}

#[test]
fn encode_rejects_non_canonical_proof() {
    let env = Env::default();
    let raw = raw_proof(valid_coords());
    let mut proof = decode_proof(&env, &Bytes::from_array(&env, &raw)).unwrap();
    proof.c.y = BytesN::from_array(&env, &P);
    assert_eq!(
        encode_proof(&env, &proof),
        Err(ProofEncodingError::NonCanonicalFieldElement)
    );

    let mut proof = decode_proof(&env, &Bytes::from_array(&env, &raw)).unwrap();
    proof.a.x = BytesN::from_array(&env, &[0; 32]);
    proof.a.y = BytesN::from_array(&env, &[0; 32]);
    assert_eq!(encode_proof(&env, &proof), Err(ProofEncodingError::IdentityPoint));
}

#[test]
fn modulus_boundaries() {
    assert!(is_canonical_fq(&[0; 32]));
    assert!(is_canonical_fq(&p_minus_one()));
    assert!(!is_canonical_fq(&P));
    assert!(!is_canonical_fq(&p_plus_one()));

    let mut r_minus_one = R;
    r_minus_one[31] -= 1;
    assert!(is_canonical_fr(&r_minus_one));
    assert!(!is_canonical_fr(&R));
    // r < p, so values in [r, p) are canonical base-field but not scalar elements.
    assert!(is_canonical_fq(&R));
}

#[test]
fn public_inputs_round_trip_and_reject_non_canonical_scalars() {
    let env = Env::default();
    let mut r_minus_one = R;
    r_minus_one[31] -= 1;

    let mut raw = Bytes::new(&env);
    raw.extend_from_array(&small(7));
    raw.extend_from_array(&r_minus_one);
    let inputs = decode_public_inputs(&env, &raw).unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(encode_public_inputs(&env, &inputs).unwrap(), raw);

    // s == r aliases 0.
    let mut bad = Bytes::new(&env);
    bad.extend_from_array(&R);
    assert_eq!(
        decode_public_inputs(&env, &bad),
        Err(ProofEncodingError::NonCanonicalScalar)
    );

    let mut bad_vec: Vec<BytesN<32>> = Vec::new(&env);
    bad_vec.push_back(BytesN::from_array(&env, &R));
    assert_eq!(
        encode_public_inputs(&env, &bad_vec),
        Err(ProofEncodingError::NonCanonicalScalar)
    );
}

#[test]
fn public_inputs_reject_bad_length_and_excess_count() {
    let env = Env::default();
    let ragged = Bytes::from_slice(&env, &[1u8; 33]);
    assert_eq!(
        decode_public_inputs(&env, &ragged),
        Err(ProofEncodingError::InvalidLength)
    );

    let mut too_many = Bytes::new(&env);
    for _ in 0..(MAX_PUBLIC_INPUTS + 1) {
        too_many.extend_from_array(&small(1));
    }
    assert_eq!(
        decode_public_inputs(&env, &too_many),
        Err(ProofEncodingError::TooManyPublicInputs)
    );

    assert_eq!(decode_public_inputs(&env, &Bytes::new(&env)).unwrap().len(), 0);
}
