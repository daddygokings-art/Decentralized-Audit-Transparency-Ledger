//! Multi-actor, model-based fuzzing of the tamper-evidence subsystem under
//! Byzantine validator behaviour.
//!
//! Each proptest case spins up a validator set in which any member may act
//! honestly or adversarially, interleaved with outsiders and ledger time
//! passing. Every submission goes through a real contract invocation (so
//! host traps such as a failed ed25519 check roll back state exactly as on
//! chain) and its outcome is compared with a reference model that predicts
//! the precise result. Invariants are re-checked after every step:
//!
//! * a pinned root never changes once accepted (no equivocation succeeds);
//! * a signer's nonce never decreases, and no submission is accepted twice;
//! * outsiders, forged/impersonated signatures, cross-contract replays and
//!   expired witnesses are never accepted;
//! * honest submissions succeed unless a Byzantine validator pinned a
//!   conflicting root first, in which case the conflict is reported.
//!
//! A second property tampers with random links of a hash chain and checks
//! that exactly the predicted links are flagged, and that verification and
//! digests are deterministic across independent hosts.

extern crate std;

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use proptest::prelude::*;
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::{contract, contractimpl, Address};
use std::collections::BTreeMap;
use std::vec::Vec as StdVec;

/// Thin contract wrapper so every submission is a real, rollback-capable invocation.
#[contract]
pub struct WitnessHarness;

#[contractimpl]
impl WitnessHarness {
    pub fn submit(env: Env, signed: SignedWitness, trusted: Vec<BytesN<32>>) -> Result<(), TamperEvidenceError> {
        TamperEvidenceHelper::accept_signed_witness(&env, &signed, &trusted)
    }

    pub fn nonce(env: Env, signer: BytesN<32>) -> Option<u64> {
        TamperEvidenceHelper::last_nonce(&env, &signer)
    }

    pub fn root(env: Env, index: u32) -> Option<BytesN<32>> {
        TamperEvidenceHelper::accepted_root(&env, index)
    }
}

const MAX_VALIDATORS: usize = 5;
const MAX_INDEX: u32 = 6;
const WINDOW: u32 = 10;

#[derive(Clone, Debug)]
enum Action {
    /// Validator signs the canonical root for an index with a fresh nonce.
    Honest { v: usize, index: u32 },
    /// Validator signs a conflicting root for an index with a fresh nonce.
    Equivocate { v: usize, index: u32, salt: u8 },
    /// Resubmit the validator's last accepted witness verbatim.
    ReplayLast { v: usize },
    /// Sign a new witness re-using an already consumed nonce.
    StaleNonce { v: usize, index: u32 },
    /// A key outside the validator set signs a witness.
    Outsider { index: u32, seed: u8 },
    /// Flip one byte of an otherwise valid signature.
    ForgeSignature { v: usize, index: u32, byte: usize },
    /// Validator `v` signs but claims to be validator `as_v`.
    Impersonate { v: usize, as_v: usize, index: u32 },
    /// Witness signed for a different deployment of the contract.
    CrossContract { v: usize, index: u32 },
    /// Witness whose expiry is already behind the ledger.
    Expired { v: usize, index: u32 },
    /// Ledger advances.
    Advance { ledgers: u32 },
}

fn action() -> impl Strategy<Value = Action> {
    let v = 0..MAX_VALIDATORS;
    let i = 0..MAX_INDEX;
    prop_oneof![
        4 => (v.clone(), i.clone()).prop_map(|(v, index)| Action::Honest { v, index }),
        2 => (v.clone(), i.clone(), 1u8..=250).prop_map(|(v, index, salt)| Action::Equivocate { v, index, salt }),
        1 => v.clone().prop_map(|v| Action::ReplayLast { v }),
        1 => (v.clone(), i.clone()).prop_map(|(v, index)| Action::StaleNonce { v, index }),
        1 => (i.clone(), 100u8..=200).prop_map(|(index, seed)| Action::Outsider { index, seed }),
        1 => (v.clone(), i.clone(), 0usize..64).prop_map(|(v, index, byte)| Action::ForgeSignature { v, index, byte }),
        1 => (v.clone(), v.clone(), i.clone()).prop_map(|(v, as_v, index)| Action::Impersonate { v, as_v, index }),
        1 => (v.clone(), i.clone()).prop_map(|(v, index)| Action::CrossContract { v, index }),
        1 => (v.clone(), i.clone()).prop_map(|(v, index)| Action::Expired { v, index }),
        1 => (1u32..=WINDOW * 2).prop_map(|ledgers| Action::Advance { ledgers }),
    ]
}

/// What the reference model predicts for a submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    Accepted,
    Rejected(TamperEvidenceError),
    /// The host traps in signature verification.
    Trap,
}

struct World {
    env: Env,
    id: Address,
    other_id: Address,
    keys: StdVec<SigningKey>,
    trusted: Vec<BytesN<32>>,
    // Reference model.
    nonces: BTreeMap<usize, u64>,
    roots: BTreeMap<u32, BytesN<32>>,
    last_accepted: BTreeMap<usize, SignedWitness>,
}

impl World {
    fn new(validators: usize) -> Self {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.sequence_number = 10_000;
            l.timestamp = 1_700_000_000;
        });
        let id = env.register(WitnessHarness, ());
        let other_id = env.register(WitnessHarness, ());
        let keys: StdVec<SigningKey> = (0..validators)
            .map(|i| SigningKey::from_bytes(&[i as u8 + 1; 32]))
            .collect();
        let mut trusted = Vec::new(&env);
        for k in &keys {
            trusted.push_back(BytesN::from_array(&env, &k.verifying_key().to_bytes()));
        }
        Self {
            env,
            id,
            other_id,
            keys,
            trusted,
            nonces: BTreeMap::new(),
            roots: BTreeMap::new(),
            last_accepted: BTreeMap::new(),
        }
    }

    fn pk(&self, key: &SigningKey) -> BytesN<32> {
        BytesN::from_array(&self.env, &key.verifying_key().to_bytes())
    }

    fn canonical_root(&self, index: u32) -> BytesN<32> {
        let mut a = [0xC0u8; 32];
        a[..4].copy_from_slice(&index.to_be_bytes());
        BytesN::from_array(&self.env, &a)
    }

    fn byzantine_root(&self, index: u32, salt: u8) -> BytesN<32> {
        let mut a = [salt; 32];
        a[..4].copy_from_slice(&index.to_be_bytes());
        a[4] = 0xBA; // never equals a canonical root
        BytesN::from_array(&self.env, &a)
    }

    fn fresh_nonce(&self, v: usize) -> u64 {
        self.nonces.get(&v).map_or(0, |n| n + 1)
    }

    fn valid_until(&self) -> u32 {
        self.env.ledger().sequence() + WINDOW
    }

    /// Sign `root` for `index` with `key`, claiming to be `claimed`, bound to `contract`.
    #[allow(clippy::too_many_arguments)]
    fn sign(
        &self,
        contract: &Address,
        key: &SigningKey,
        claimed: &BytesN<32>,
        index: u32,
        root: BytesN<32>,
        nonce: u64,
        valid_until: u32,
    ) -> SignedWitness {
        self.env.as_contract(contract, || {
            let w = TamperEvidenceHelper::create_chain_witness(&self.env, index, root);
            let d = TamperEvidenceHelper::witness_digest(&self.env, &w, claimed, nonce, valid_until);
            let sig = key.sign(&d.to_array());
            SignedWitness {
                witness: w,
                signer: claimed.clone(),
                nonce,
                valid_until_ledger: valid_until,
                signature: BytesN::from_array(&self.env, &sig.to_bytes()),
            }
        })
    }

    fn honest_sign(&self, v: usize, index: u32, root: BytesN<32>, nonce: u64) -> SignedWitness {
        let pk = self.pk(&self.keys[v]);
        self.sign(&self.id, &self.keys[v], &pk, index, root, nonce, self.valid_until())
    }

    /// Model outcome for a well-formed, correctly signed submission by `v`.
    fn expect_signed(&self, v: usize, sw: &SignedWitness) -> Expect {
        if self.nonces.get(&v).is_some_and(|&n| sw.nonce <= n) {
            return Expect::Rejected(TamperEvidenceError::ReplayedNonce);
        }
        match self.roots.get(&sw.witness.event_index) {
            Some(r) if *r != sw.witness.root_at_index => Expect::Rejected(TamperEvidenceError::Equivocation),
            _ => Expect::Accepted,
        }
    }

    fn submit(&self, sw: &SignedWitness) -> Expect {
        let client = WitnessHarnessClient::new(&self.env, &self.id);
        match client.try_submit(sw, &self.trusted) {
            Ok(Ok(())) => Expect::Accepted,
            Err(Ok(e)) => Expect::Rejected(e),
            Err(Err(_)) => Expect::Trap,
            Ok(Err(e)) => panic!("conversion error: {e:?}"),
        }
    }

    fn record(&mut self, v: usize, sw: SignedWitness) {
        self.nonces.insert(v, sw.nonce);
        self.roots
            .entry(sw.witness.event_index)
            .or_insert(sw.witness.root_at_index.clone());
        self.last_accepted.insert(v, sw);
    }

    /// Execute one action, asserting the contract agrees with the model.
    fn step(&mut self, a: &Action) {
        let n = self.keys.len();
        let (expected, signed, accepted_by) = match *a {
            Action::Advance { ledgers } => {
                self.env.ledger().with_mut(|l| {
                    l.sequence_number += ledgers;
                    l.timestamp += 5 * ledgers as u64;
                });
                return;
            }
            Action::Honest { v, index } => {
                let v = v % n;
                let sw = self.honest_sign(v, index, self.canonical_root(index), self.fresh_nonce(v));
                (self.expect_signed(v, &sw), sw, Some(v))
            }
            Action::Equivocate { v, index, salt } => {
                let v = v % n;
                let sw = self.honest_sign(v, index, self.byzantine_root(index, salt), self.fresh_nonce(v));
                (self.expect_signed(v, &sw), sw, Some(v))
            }
            Action::ReplayLast { v } => {
                let Some(sw) = self.last_accepted.get(&(v % n)).cloned() else {
                    return;
                };
                // The expiry check runs before the nonce check.
                let e = if self.env.ledger().sequence() > sw.valid_until_ledger {
                    TamperEvidenceError::WitnessExpired
                } else {
                    TamperEvidenceError::ReplayedNonce
                };
                (Expect::Rejected(e), sw, None)
            }
            Action::StaleNonce { v, index } => {
                let v = v % n;
                let Some(&last) = self.nonces.get(&v) else { return };
                let sw = self.honest_sign(v, index, self.canonical_root(index), last);
                (Expect::Rejected(TamperEvidenceError::ReplayedNonce), sw, None)
            }
            Action::Outsider { index, seed } => {
                let key = SigningKey::from_bytes(&[seed; 32]);
                let pk = self.pk(&key);
                let sw = self.sign(
                    &self.id,
                    &key,
                    &pk,
                    index,
                    self.canonical_root(index),
                    0,
                    self.valid_until(),
                );
                (Expect::Rejected(TamperEvidenceError::UnauthorizedSigner), sw, None)
            }
            Action::ForgeSignature { v, index, byte } => {
                let v = v % n;
                let mut sw = self.honest_sign(v, index, self.canonical_root(index), self.fresh_nonce(v));
                let mut sig = sw.signature.to_array();
                sig[byte] ^= 0x01;
                sw.signature = BytesN::from_array(&self.env, &sig);
                (Expect::Trap, sw, None)
            }
            Action::Impersonate { v, as_v, index } => {
                let (v, as_v) = (v % n, as_v % n);
                if v == as_v {
                    return;
                }
                let claimed = self.pk(&self.keys[as_v]);
                let sw = self.sign(
                    &self.id,
                    &self.keys[v],
                    &claimed,
                    index,
                    self.canonical_root(index),
                    self.fresh_nonce(as_v),
                    self.valid_until(),
                );
                (Expect::Trap, sw, None)
            }
            Action::CrossContract { v, index } => {
                let v = v % n;
                let pk = self.pk(&self.keys[v]);
                let sw = self.sign(
                    &self.other_id,
                    &self.keys[v],
                    &pk,
                    index,
                    self.canonical_root(index),
                    self.fresh_nonce(v),
                    self.valid_until(),
                );
                (Expect::Trap, sw, None)
            }
            Action::Expired { v, index } => {
                let v = v % n;
                let pk = self.pk(&self.keys[v]);
                let until = self.env.ledger().sequence() - 1;
                let sw = self.sign(
                    &self.id,
                    &self.keys[v],
                    &pk,
                    index,
                    self.canonical_root(index),
                    self.fresh_nonce(v),
                    until,
                );
                (Expect::Rejected(TamperEvidenceError::WitnessExpired), sw, None)
            }
        };

        let actual = self.submit(&signed);
        assert_eq!(actual, expected, "action {a:?}");
        if actual == Expect::Accepted {
            self.record(
                accepted_by.expect("only signer-attributed actions may be accepted"),
                signed,
            );
        }
        self.check_invariants();
    }

    fn check_invariants(&self) {
        let client = WitnessHarnessClient::new(&self.env, &self.id);
        for index in 0..MAX_INDEX {
            assert_eq!(client.root(&index), self.roots.get(&index).cloned(), "root @ {index}");
        }
        for (v, key) in self.keys.iter().enumerate() {
            assert_eq!(
                client.nonce(&self.pk(key)),
                self.nonces.get(&v).copied(),
                "nonce of {v}"
            );
        }
        // Nothing ever lands in the second deployment.
        let other = WitnessHarnessClient::new(&self.env, &self.other_id);
        for index in 0..MAX_INDEX {
            assert_eq!(other.root(&index), None);
        }
    }
}

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config(48))]

    #[test]
    fn byzantine_validators_cannot_break_witness_invariants(
        validators in 2..=MAX_VALIDATORS,
        actions in prop::collection::vec(action(), 1..40),
    ) {
        let mut world = World::new(validators);
        for a in &actions {
            world.step(a);
        }
    }
}

// ── Hash-chain tampering ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Tamper {
    /// Rewrite the content (hence the hash) of an event.
    RewriteEvent(usize),
    /// Rewrite the back-pointer stored in an event.
    RewritePrevPointer(usize),
}

fn tamper() -> impl Strategy<Value = Tamper> {
    prop_oneof![
        (0usize..256).prop_map(Tamper::RewriteEvent),
        (0usize..256).prop_map(Tamper::RewritePrevPointer),
    ]
}

fn honest_hash(env: &Env, i: usize) -> BytesN<32> {
    let mut a = [0x11u8; 32];
    a[..8].copy_from_slice(&(i as u64).to_be_bytes());
    BytesN::from_array(env, &a)
}

/// Build an `n`-link chain, apply `tampers`, and return it with the set of
/// link indices the model expects to be flagged.
fn tampered_chain(env: &Env, n: usize, tampers: &[Tamper]) -> (Vec<ChainLink>, StdVec<u32>) {
    let mut hashes: StdVec<BytesN<32>> = (0..=n).map(|i| honest_hash(env, i)).collect();
    // hashes[0] is the genesis anchor; link k has hash hashes[k+1], prev hashes[k].
    let mut prevs: StdVec<BytesN<32>> = hashes[..n].to_vec();
    let mut flagged = std::collections::BTreeSet::new();
    for (counter, t) in tampers.iter().enumerate() {
        let mut forged = [0xFFu8; 32];
        forged[..8].copy_from_slice(&(counter as u64).to_be_bytes());
        let forged = BytesN::from_array(env, &forged);
        match *t {
            Tamper::RewriteEvent(k) => {
                let k = k % n;
                hashes[k + 1] = forged;
                // The successor's back-pointer no longer matches.
                if k + 1 < n {
                    flagged.insert(k as u32 + 1);
                }
            }
            Tamper::RewritePrevPointer(k) => {
                let k = k % n;
                prevs[k] = forged;
                flagged.insert(k as u32);
            }
        }
    }
    let honest: StdVec<BytesN<32>> = (0..n).map(|i| honest_hash(env, i)).collect();
    let mut links = Vec::new(env);
    for k in 0..n {
        links.push_back(ChainLink {
            index: k as u32,
            hash: hashes[k + 1].clone(),
            prev_hash: prevs[k].clone(),
            expected_prev_hash: honest[k].clone(), // independent anchor (e.g. a witness)
            valid: true,                           // adversary claims everything is fine
        });
    }
    (links, flagged.into_iter().collect())
}

fn verify_in_fresh_host(n: usize, tampers: &[Tamper]) -> (ChainVerification, StdVec<u32>) {
    let env = Env::default();
    let id = env.register(WitnessHarness, ());
    let (links, expected) = tampered_chain(&env, n, tampers);
    let (v, compact) = env.as_contract(&id, || {
        let v = TamperEvidenceHelper::verify_chain_continuity(&env, &links).unwrap();
        // The compact large-batch path must reach the same verdict.
        let (mut hashes, mut prevs) = (Vec::new(&env), Vec::new(&env));
        for l in links.iter() {
            hashes.push_back(l.hash);
            prevs.push_back(l.prev_hash);
        }
        let anchor = links.get(0).unwrap().expected_prev_hash;
        let c = TamperEvidenceHelper::verify_hash_chain(&env, 0, &anchor, &hashes, &prevs).unwrap();
        (v, c)
    });
    assert_eq!(v, compact, "compact path disagrees");
    (v, expected)
}

proptest! {
    #![proptest_config(config(64))]

    #[test]
    fn every_tampered_link_is_detected(
        n in 1usize..48,
        tampers in prop::collection::vec(tamper(), 0..8),
    ) {
        let (v, expected) = verify_in_fresh_host(n, &tampers);
        let got: StdVec<u32> = v.compromised_indices.iter().collect();
        prop_assert_eq!(&got, &expected);
        prop_assert_eq!(v.valid, expected.is_empty());
        prop_assert_eq!(v.mismatches as usize, expected.len());
        prop_assert_eq!(v.events_verified as usize, n);
    }

    #[test]
    fn verification_is_deterministic_across_hosts(
        n in 1usize..32,
        tampers in prop::collection::vec(tamper(), 0..4),
    ) {
        let (a, _) = verify_in_fresh_host(n, &tampers);
        let (b, _) = verify_in_fresh_host(n, &tampers);
        prop_assert_eq!(a.valid, b.valid);
        prop_assert_eq!(a.mismatches, b.mismatches);
        prop_assert_eq!(a.integrity_score, b.integrity_score);
        let ai: StdVec<u32> = a.compromised_indices.iter().collect();
        let bi: StdVec<u32> = b.compromised_indices.iter().collect();
        prop_assert_eq!(ai, bi);
    }

    #[test]
    fn witness_digest_and_merkle_root_are_deterministic(
        index in any::<u32>(),
        root in any::<[u8; 32]>(),
        nonce in any::<u64>(),
        leaf_index in 0u32..16,
        path in prop::collection::vec(any::<[u8; 32]>(), 4),
    ) {
        let run = || {
            let env = Env::default();
            let id = env.register(WitnessHarness, ());
            env.as_contract(&id, || {
                let w = TamperEvidenceHelper::create_chain_witness(&env, index, BytesN::from_array(&env, &root));
                let signer = BytesN::from_array(&env, &[7u8; 32]);
                let d = TamperEvidenceHelper::witness_digest(&env, &w, &signer, nonce, 1);
                let mut p = Vec::new(&env);
                for s in &path {
                    p.push_back(BytesN::from_array(&env, s));
                }
                let r = TamperEvidenceHelper::compute_merkle_root(&env, &w.root_at_index, leaf_index, &p)
                    .unwrap();
                (w.commitment_hash.to_array(), d.to_array(), r.to_array())
            })
        };
        prop_assert_eq!(run(), run());
    }
}
