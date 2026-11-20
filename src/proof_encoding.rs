//! # Canonical Proof Serialization (Groth16 / BN254)
//!
//! Strict (de)serialization for the Groth16 proofs used by the ZK bridge
//! (see `docs/ZK_BRIDGE.md`). Proof points use the EIP-197 uncompressed,
//! big-endian encoding that the BN254 pairing precompile expects:
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0      | 64   | `A` ∈ G1 — `x ‖ y` |
//! | 64     | 128  | `B` ∈ G2 — `x.c1 ‖ x.c0 ‖ y.c1 ‖ y.c0` |
//! | 192    | 64   | `C` ∈ G1 — `x ‖ y` |
//!
//! ## Why canonical encodings matter
//!
//! A base-field coordinate `v` and `v + p` reduce to the same field element, so
//! a lenient decoder accepts several byte strings for one mathematical proof.
//! That breaks every property that keys on the proof bytes (replay protection,
//! proof-hash commitments, deduplication, nullifiers) and is a classic
//! malleability vector. Likewise a public input `s` and `s + r` are the same
//! scalar. This module therefore rejects:
//!
//! * any coordinate `≥ p` (BN254 base-field modulus) — [`ProofEncodingError::NonCanonicalFieldElement`];
//! * any public input `≥ r` (BN254 scalar-field modulus) — [`ProofEncodingError::NonCanonicalScalar`];
//! * the point at infinity (all-zero encoding) for `A`, `B` or `C` — [`ProofEncodingError::IdentityPoint`];
//! * truncated or trailing bytes — [`ProofEncodingError::InvalidLength`].
//!
//! Encoding is held to the same rules: [`encode_proof`] refuses to serialize a
//! proof that would not round-trip through [`decode_proof`], so
//! `encode(decode(b)) == b` for every accepted `b`.
//!
//! Curve-membership and subgroup checks are intentionally *not* performed
//! here — they require field arithmetic and are enforced by the pairing
//! verifier. This module is the cheap, deterministic gate in front of it.
//!
//! ## Metering
//!
//! Decoding performs one host call to copy the proof into a fixed stack buffer
//! and then only constant-size byte comparisons (no allocation, no field
//! arithmetic, no storage access), so its cost is constant and independent of
//! the proof contents.

use soroban_sdk::{contracterror, contracttype, Bytes, BytesN, Env, Vec};

/// Size of one serialized base-field / scalar-field element.
pub const FIELD_ELEMENT_BYTES: u32 = 32;
/// Size of an uncompressed G1 point (`x ‖ y`).
pub const G1_POINT_BYTES: u32 = 64;
/// Size of an uncompressed G2 point (`x.c1 ‖ x.c0 ‖ y.c1 ‖ y.c0`).
pub const G2_POINT_BYTES: u32 = 128;
/// Size of a serialized Groth16 proof (`A ‖ B ‖ C`).
pub const GROTH16_PROOF_BYTES: u32 = 256;
/// Upper bound on the number of public inputs accepted per proof.
pub const MAX_PUBLIC_INPUTS: u32 = 16;

/// BN254 base-field modulus `p`, big-endian.
pub const BN254_FQ_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

/// BN254 scalar-field modulus `r`, big-endian.
pub const BN254_FR_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];

/// Stable error codes for proof (de)serialization.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ProofEncodingError {
    /// Input is not exactly the expected number of bytes.
    InvalidLength = 7101,
    /// A point coordinate is `≥ p` and therefore not canonically encoded.
    NonCanonicalFieldElement = 7102,
    /// A proof point is the point at infinity.
    IdentityPoint = 7103,
    /// A public input is `≥ r` and therefore not canonically encoded.
    NonCanonicalScalar = 7104,
    /// More than [`MAX_PUBLIC_INPUTS`] public inputs were supplied.
    TooManyPublicInputs = 7105,
}

/// Uncompressed G1 affine point.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G1Point {
    pub x: BytesN<32>,
    pub y: BytesN<32>,
}

/// Uncompressed G2 affine point over `Fq2` (EIP-197 ordering: imaginary first).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G2Point {
    pub x_c1: BytesN<32>,
    pub x_c0: BytesN<32>,
    pub y_c1: BytesN<32>,
    pub y_c0: BytesN<32>,
}

/// Groth16 proof `(A, B, C)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Groth16Proof {
    pub a: G1Point,
    pub b: G2Point,
    pub c: G1Point,
}

/// Returns `true` iff `v < modulus` when both are read as big-endian integers.
#[inline]
fn lt_modulus(v: &[u8], modulus: &[u8; 32]) -> bool {
    // Equal-length big-endian byte strings compare like the integers they encode.
    v < &modulus[..]
}

/// Returns `true` iff `v` is the canonical encoding of a BN254 base-field element.
#[inline]
pub fn is_canonical_fq(v: &[u8; 32]) -> bool {
    lt_modulus(v, &BN254_FQ_MODULUS)
}

/// Returns `true` iff `v` is the canonical encoding of a BN254 scalar-field element.
#[inline]
pub fn is_canonical_fr(v: &[u8; 32]) -> bool {
    lt_modulus(v, &BN254_FR_MODULUS)
}

/// Validates a raw point encoding of `N` bytes: every 32-byte coordinate must
/// be `< p`, and the all-zero (infinity) encoding is rejected.
fn check_point(raw: &[u8]) -> Result<(), ProofEncodingError> {
    let mut all_zero = true;
    for coord in raw.chunks_exact(FIELD_ELEMENT_BYTES as usize) {
        if !lt_modulus(coord, &BN254_FQ_MODULUS) {
            return Err(ProofEncodingError::NonCanonicalFieldElement);
        }
        if all_zero && coord.iter().any(|b| *b != 0) {
            all_zero = false;
        }
    }
    if all_zero {
        return Err(ProofEncodingError::IdentityPoint);
    }
    Ok(())
}

/// Validates the canonical layout of a raw 256-byte proof buffer.
fn check_proof_buffer(buf: &[u8; GROTH16_PROOF_BYTES as usize]) -> Result<(), ProofEncodingError> {
    let g1 = G1_POINT_BYTES as usize;
    let g2 = G2_POINT_BYTES as usize;
    check_point(&buf[0..g1])?;
    check_point(&buf[g1..g1 + g2])?;
    check_point(&buf[g1 + g2..])?;
    Ok(())
}

fn field(env: &Env, buf: &[u8], offset: usize) -> BytesN<32> {
    let mut out = [0u8; 32];
    out.copy_from_slice(&buf[offset..offset + 32]);
    BytesN::from_array(env, &out)
}

/// Strictly decodes a serialized Groth16 proof.
///
/// Rejects any input that is not exactly [`GROTH16_PROOF_BYTES`] long, any
/// coordinate `≥ p`, and any identity point.
pub fn decode_proof(env: &Env, bytes: &Bytes) -> Result<Groth16Proof, ProofEncodingError> {
    if bytes.len() != GROTH16_PROOF_BYTES {
        return Err(ProofEncodingError::InvalidLength);
    }
    let mut buf = [0u8; GROTH16_PROOF_BYTES as usize];
    bytes.copy_into_slice(&mut buf);
    check_proof_buffer(&buf)?;

    Ok(Groth16Proof {
        a: G1Point {
            x: field(env, &buf, 0),
            y: field(env, &buf, 32),
        },
        b: G2Point {
            x_c1: field(env, &buf, 64),
            x_c0: field(env, &buf, 96),
            y_c1: field(env, &buf, 128),
            y_c0: field(env, &buf, 160),
        },
        c: G1Point {
            x: field(env, &buf, 192),
            y: field(env, &buf, 224),
        },
    })
}

/// Serializes a Groth16 proof, refusing non-canonical points so that the
/// output always round-trips through [`decode_proof`].
pub fn encode_proof(env: &Env, proof: &Groth16Proof) -> Result<Bytes, ProofEncodingError> {
    let mut buf = [0u8; GROTH16_PROOF_BYTES as usize];
    let parts: [&BytesN<32>; 8] = [
        &proof.a.x,
        &proof.a.y,
        &proof.b.x_c1,
        &proof.b.x_c0,
        &proof.b.y_c1,
        &proof.b.y_c0,
        &proof.c.x,
        &proof.c.y,
    ];
    for (i, part) in parts.iter().enumerate() {
        buf[i * 32..(i + 1) * 32].copy_from_slice(&part.to_array());
    }
    check_proof_buffer(&buf)?;
    Ok(Bytes::from_array(env, &buf))
}

/// Strictly decodes a concatenation of 32-byte big-endian public inputs.
///
/// Rejects lengths that are not a multiple of 32, more than
/// [`MAX_PUBLIC_INPUTS`] inputs, and any input `≥ r`.
pub fn decode_public_inputs(env: &Env, bytes: &Bytes) -> Result<Vec<BytesN<32>>, ProofEncodingError> {
    let len = bytes.len();
    if len % FIELD_ELEMENT_BYTES != 0 {
        return Err(ProofEncodingError::InvalidLength);
    }
    let count = len / FIELD_ELEMENT_BYTES;
    if count > MAX_PUBLIC_INPUTS {
        return Err(ProofEncodingError::TooManyPublicInputs);
    }

    let mut out = Vec::new(env);
    let mut elem = [0u8; 32];
    for i in 0..count {
        let start = i * FIELD_ELEMENT_BYTES;
        bytes
            .slice(start..start + FIELD_ELEMENT_BYTES)
            .copy_into_slice(&mut elem);
        if !is_canonical_fr(&elem) {
            return Err(ProofEncodingError::NonCanonicalScalar);
        }
        out.push_back(BytesN::from_array(env, &elem));
    }
    Ok(out)
}

/// Serializes public inputs, refusing any scalar `≥ r`.
pub fn encode_public_inputs(env: &Env, inputs: &Vec<BytesN<32>>) -> Result<Bytes, ProofEncodingError> {
    if inputs.len() > MAX_PUBLIC_INPUTS {
        return Err(ProofEncodingError::TooManyPublicInputs);
    }
    let mut out = Bytes::new(env);
    for input in inputs.iter() {
        let elem = input.to_array();
        if !is_canonical_fr(&elem) {
            return Err(ProofEncodingError::NonCanonicalScalar);
        }
        out.extend_from_array(&elem);
    }
    Ok(out)
}
