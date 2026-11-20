//! Rogue-validator simulation for regulatory state-transition consensus.
//!
//! Byzantine validators propose invalid transitions, forge prior states and
//! equivocate. Honest validators drive a subject through its lifecycle. The
//! tests check that rogue nodes are detected and quarantined, that they can
//! never commit a transition on their own, and that every committed state
//! change is in the canonical transition table.

use crate::rbac::Rbac;
use crate::regulatory_consensus::*;
use crate::test_events::{count_topic, events_during};
use crate::AuditLedger;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, Vec};

use crate::regulatory_consensus::RegulatoryState::*;

struct Net {
    env: Env,
    contract: Address,
    admin: Address,
    honest: alloc::vec::Vec<Address>,
    rogue: alloc::vec::Vec<Address>,
}

impl Net {
    fn new(honest: usize, rogue: usize, quorum: u32) -> Net {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract = env.register(AuditLedger, ());
        let admin = Address::generate(&env);
        let honest: alloc::vec::Vec<Address> = (0..honest).map(|_| Address::generate(&env)).collect();
        let rogue: alloc::vec::Vec<Address> = (0..rogue).map(|_| Address::generate(&env)).collect();

        let mut set = Vec::new(&env);
        for v in honest.iter().chain(rogue.iter()) {
            set.push_back(v.clone());
        }
        env.as_contract(&contract, || {
            Rbac::init_admin(&env, &admin);
            RegulatoryConsensus::configure(&env, &admin, set, quorum).unwrap();
        });
        Net {
            env,
            contract,
            admin,
            honest,
            rogue,
        }
    }

    fn subject(&self, tag: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[tag; 32])
    }

    fn propose(
        &self,
        v: &Address,
        s: &BytesN<32>,
        round: u32,
        from: RegulatoryState,
        to: RegulatoryState,
    ) -> Result<VoteOutcome, ConsensusError> {
        self.env.as_contract(&self.contract, || {
            RegulatoryConsensus::propose(&self.env, v, s, round, from, to)
        })
    }

    fn record(&self, s: &BytesN<32>) -> SubjectRecord {
        self.env
            .as_contract(&self.contract, || RegulatoryConsensus::get_subject(&self.env, s))
    }

    fn validator(&self, v: &Address) -> ValidatorRecord {
        self.env
            .as_contract(&self.contract, || RegulatoryConsensus::get_validator(&self.env, v))
    }

    /// All honest validators vote for `to`; returns the last outcome.
    fn honest_round(&self, s: &BytesN<32>, to: RegulatoryState) -> VoteOutcome {
        let rec = self.record(s);
        let mut last = None;
        for v in self.honest.iter() {
            match self.propose(v, s, rec.round, rec.state, to) {
                Ok(o) => {
                    let done = matches!(o, VoteOutcome::Committed(_));
                    last = Some(o);
                    if done {
                        break;
                    }
                }
                Err(e) => panic!("honest validator rejected: {:?}", e),
            }
        }
        last.unwrap()
    }
}

/// Deterministic LCG so the simulation is reproducible without extra deps.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn state(&mut self) -> RegulatoryState {
        RegulatoryState::ALL[(self.next() % 7) as usize]
    }
}

#[test]
fn transition_table_is_closed_and_revoked_is_terminal() {
    for to in RegulatoryState::ALL {
        assert!(!is_valid_transition(Revoked, to));
    }
    for s in RegulatoryState::ALL {
        assert!(!is_valid_transition(s, s), "self-transition {:?} must be invalid", s);
    }
    assert!(is_valid_transition(Draft, Submitted));
    assert!(!is_valid_transition(Draft, Approved));
}

#[test]
fn honest_quorum_commits_valid_transition() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(1);
    let out = net.honest_round(&s, Submitted);
    assert_eq!(out, VoteOutcome::Committed(Submitted));
    assert_eq!(net.record(&s), SubjectRecord { state: Submitted, round: 1 });
}

#[test]
fn rogue_invalid_transition_is_rejected_and_struck() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(2);
    let rogue = net.rogue[0].clone();

    // Draft -> Approved skips submission and review.
    let (out, ev) = events_during(&net.env, || net.propose(&rogue, &s, 0, Draft, Approved));
    assert_eq!(out, Ok(VoteOutcome::Rejected(RogueReason::InvalidTransition)));
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "rogue"), 1);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "vote"), 0);

    assert_eq!(net.record(&s).state, Draft, "state must not move");
    assert_eq!(net.validator(&rogue), ValidatorRecord { strikes: 1, quarantined: false });
    assert!(net
        .env
        .as_contract(&net.contract, || RegulatoryConsensus::get_votes(&net.env, &s))
        .is_none());
}

#[test]
fn rogue_forged_prior_state_is_rejected() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(3);
    // Subject is in Draft; rogue claims it is UnderReview to push it to Approved.
    let out = net.propose(&net.rogue[0], &s, 0, UnderReview, Approved);
    assert_eq!(out, Ok(VoteOutcome::Rejected(RogueReason::ForgedPriorState)));
    assert_eq!(net.record(&s).state, Draft);
}

#[test]
fn repeated_rogue_proposals_quarantine_the_validator() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(4);
    let rogue = net.rogue[0].clone();

    net.propose(&rogue, &s, 0, Draft, Revoked).unwrap();
    let (_, ev) = events_during(&net.env, || net.propose(&rogue, &s, 0, Draft, Approved).unwrap());
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "quarantine"), 1);
    assert!(net.validator(&rogue).quarantined);

    // Even a valid proposal is now refused.
    assert_eq!(
        net.propose(&rogue, &s, 0, Draft, Submitted),
        Err(ConsensusError::ValidatorQuarantined)
    );
}

#[test]
fn equivocation_quarantines_immediately_and_vote_stops_counting() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(5);
    net.honest_round(&s, Submitted);
    let rogue = net.rogue[0].clone();

    // Rogue votes for UnderReview and then for Draft in the same round.
    assert_eq!(net.propose(&rogue, &s, 1, Submitted, UnderReview), Ok(VoteOutcome::Pending(1)));
    assert_eq!(
        net.propose(&rogue, &s, 1, Submitted, Draft),
        Ok(VoteOutcome::Rejected(RogueReason::Equivocation))
    );
    assert!(net.validator(&rogue).quarantined);

    // The rogue's earlier UnderReview vote no longer counts: two honest votes
    // stay pending, and the third commits.
    assert_eq!(net.propose(&net.honest[0], &s, 1, Submitted, UnderReview), Ok(VoteOutcome::Pending(1)));
    assert_eq!(net.propose(&net.honest[1], &s, 1, Submitted, UnderReview), Ok(VoteOutcome::Pending(2)));
    assert_eq!(
        net.propose(&net.honest[2], &s, 1, Submitted, UnderReview),
        Ok(VoteOutcome::Committed(UnderReview))
    );
}

#[test]
fn rogue_minority_cannot_commit_a_valid_but_unwanted_transition() {
    // n = 7, f = 2, quorum = 5.
    let net = Net::new(5, 2, 5);
    let s = net.subject(6);
    net.honest_round(&s, Submitted);
    net.honest_round(&s, UnderReview);

    // Both rogues push the (valid) Rejected outcome.
    for r in net.rogue.iter() {
        assert!(matches!(net.propose(r, &s, 2, UnderReview, Rejected), Ok(VoteOutcome::Pending(_))));
    }
    assert_eq!(net.record(&s).state, UnderReview);

    // Honest validators approve; the competing candidate does not block them.
    assert_eq!(net.honest_round(&s, Approved), VoteOutcome::Committed(Approved));
    assert_eq!(net.record(&s), SubjectRecord { state: Approved, round: 3 });
}

#[test]
fn non_member_stale_round_and_duplicate_votes_are_refused() {
    let net = Net::new(3, 1, 3);
    let s = net.subject(7);
    let outsider = Address::generate(&net.env);

    assert_eq!(
        net.propose(&outsider, &s, 0, Draft, Submitted),
        Err(ConsensusError::NotValidator)
    );
    assert_eq!(
        net.propose(&net.honest[0], &s, 5, Draft, Submitted),
        Err(ConsensusError::StaleRound)
    );
    net.propose(&net.honest[0], &s, 0, Draft, Submitted).unwrap();
    assert_eq!(
        net.propose(&net.honest[0], &s, 0, Draft, Submitted),
        Err(ConsensusError::DuplicateVote)
    );
    // None of these are strikes.
    assert_eq!(net.validator(&net.honest[0]).strikes, 0);
}

#[test]
fn configuration_enforces_bft_quorum_and_admin_role() {
    let net = Net::new(3, 1, 3);
    let mut four = Vec::new(&net.env);
    for v in net.honest.iter().chain(net.rogue.iter()) {
        four.push_back(v.clone());
    }
    let cfg = |caller: &Address, set: Vec<Address>, q: u32| {
        net.env.as_contract(&net.contract, || {
            RegulatoryConsensus::configure(&net.env, caller, set, q)
        })
    };

    // 2 of 4 is not a BFT quorum (3*2 <= 2*4).
    assert_eq!(cfg(&net.admin, four.clone(), 2), Err(ConsensusError::InvalidConfig));
    assert_eq!(cfg(&net.admin, four.clone(), 5), Err(ConsensusError::InvalidConfig));
    assert_eq!(cfg(&net.admin, Vec::new(&net.env), 0), Err(ConsensusError::InvalidConfig));

    let mut dup = four.clone();
    dup.push_back(net.honest[0].clone());
    assert_eq!(cfg(&net.admin, dup, 4), Err(ConsensusError::InvalidConfig));

    // A validator without the Admin role cannot reconfigure the set.
    assert_eq!(cfg(&net.rogue[0], four.clone(), 4), Err(ConsensusError::Unauthorized));
    assert_eq!(cfg(&net.admin, four, 4), Ok(()));
}

/// Randomised Byzantine simulation: 2 rogue nodes out of 7 send random
/// proposals in between honest rounds that drive the full lifecycle.
#[test]
fn randomized_rogue_simulation_preserves_safety_and_liveness() {
    let lifecycle = [Submitted, UnderReview, Approved, Suspended, Approved, Revoked];

    for seed in 1..=8u64 {
        let net = Net::new(5, 2, 5);
        let s = net.subject(seed as u8);
        let mut rng = Lcg(seed);
        let mut prev = net.record(&s);
        let mut rogue_rejections = 0u32;

        for target in lifecycle {
            // Rogues fire several random proposals at the current round.
            for _ in 0..4 {
                let r = &net.rogue[(rng.next() % 2) as usize];
                let (from, to) = (rng.state(), rng.state());
                let round = if rng.next() % 4 == 0 { prev.round + 1 } else { prev.round };
                match net.propose(r, &s, round, from, to) {
                    Ok(VoteOutcome::Rejected(_)) => rogue_rejections += 1,
                    Ok(VoteOutcome::Committed(_)) => panic!("rogue minority committed (seed {})", seed),
                    Ok(VoteOutcome::Pending(_)) => {}
                    Err(ConsensusError::ValidatorQuarantined)
                    | Err(ConsensusError::StaleRound)
                    | Err(ConsensusError::DuplicateVote) => {}
                    Err(e) => panic!("unexpected error {:?} (seed {})", e, seed),
                }
                // Safety: rogue traffic alone never moves the subject.
                assert_eq!(net.record(&s), prev, "rogue traffic moved state (seed {})", seed);
            }

            // Liveness: the honest quorum always commits its target.
            assert_eq!(net.honest_round(&s, target), VoteOutcome::Committed(target));
            let next = net.record(&s);
            assert!(
                is_valid_transition(prev.state, next.state),
                "committed invalid transition {:?} -> {:?} (seed {})",
                prev.state,
                next.state,
                seed
            );
            assert_eq!(next.round, prev.round + 1);
            prev = next;
        }

        assert_eq!(prev.state, Revoked);
        assert!(rogue_rejections > 0, "simulation never exercised rogue detection (seed {})", seed);
        for v in net.honest.iter() {
            assert_eq!(net.validator(v).strikes, 0, "honest node struck (seed {})", seed);
        }
    }
}
