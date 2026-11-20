//! # Validator Consensus for Regulatory State Transitions
//!
//! A quorum of validator nodes must agree before a regulated subject (a
//! licence, filing, or sandbox participant) moves between regulatory states.
//! Every vote is checked against the canonical transition table. Validators
//! that propose invalid transitions are recorded as rogue and quarantined.
//!
//! ## Transition table
//!
//! ```text
//! Draft ──► Submitted ──► UnderReview ──► Approved ◄──► Suspended
//!   ▲           │               │             │             │
//!   └───────────┘               ▼             ▼             ▼
//!   ▲                        Rejected       Revoked ◄───────┘
//!   └────────────────────────────┘          (terminal)
//! ```
//!
//! ## Rogue behaviour
//!
//! | Behaviour | Detection | Consequence |
//! |-----------|-----------|-------------|
//! | Transition not in the table | [`RogueReason::InvalidTransition`] | strike |
//! | Claims a prior state that is not the committed one | [`RogueReason::ForgedPriorState`] | strike |
//! | Votes for two different targets in one round | [`RogueReason::Equivocation`] | immediate quarantine |
//!
//! A validator reaching [`QUARANTINE_STRIKES`] strikes is quarantined. Its
//! later proposals are refused, and its existing votes stop counting toward
//! quorum. Rogue votes are reported through `Ok(VoteOutcome::Rejected(_))`
//! rather than `Err`, so the strike persists even when this runs inside a
//! contract invocation (an `Err` would roll it back).
//!
//! ## Safety
//!
//! The quorum must satisfy `3 * quorum > 2 * n`, the classic BFT bound, so up
//! to `f < n / 3` rogue validators can never commit a transition on their own.
//! With the minimal quorum `⌊2n/3⌋ + 1`, they also cannot block a transition
//! that every honest validator supports. Since only table-valid transitions
//! can gather votes, a committed state change is always a valid one.
//!
//! ## Events (`reg_cons` topic)
//!
//! | Topics | Data |
//! |--------|------|
//! | `("reg_cons", "vote")`       | `(subject, validator, round, from, to, approvals)` |
//! | `("reg_cons", "commit")`     | `(subject, round, from, to)` |
//! | `("reg_cons", "rogue")`      | `(subject, validator, reason, strikes)` |
//! | `("reg_cons", "quarantine")` | `(validator, strikes)` |
//! | `("reg_cons", "config")`     | `(caller, validators, quorum)` |

use crate::rbac::{RbacManager, Role};
use soroban_sdk::{contracterror, contracttype, Address, BytesN, Env, Symbol, Vec};

/// Maximum validator-set size (bounds every per-vote loop).
pub const MAX_VALIDATORS: u32 = 32;
/// Strikes after which a validator is quarantined.
pub const QUARANTINE_STRIKES: u32 = 2;
/// Event topic for consensus events.
pub const EVENT_TOPIC: &str = "reg_cons";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConsensusError {
    NotConfigured = 7301,
    InvalidConfig = 7302,
    NotValidator = 7303,
    ValidatorQuarantined = 7304,
    StaleRound = 7305,
    DuplicateVote = 7306,
    Unauthorized = 7307,
}

/// Lifecycle states of a regulated subject.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegulatoryState {
    Draft = 0,
    Submitted = 1,
    UnderReview = 2,
    Approved = 3,
    Rejected = 4,
    Suspended = 5,
    Revoked = 6,
}

impl RegulatoryState {
    /// All states, in discriminant order.
    pub const ALL: [RegulatoryState; 7] = [
        RegulatoryState::Draft,
        RegulatoryState::Submitted,
        RegulatoryState::UnderReview,
        RegulatoryState::Approved,
        RegulatoryState::Rejected,
        RegulatoryState::Suspended,
        RegulatoryState::Revoked,
    ];
}

/// Why a vote was classified as rogue.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RogueReason {
    InvalidTransition = 1,
    ForgedPriorState = 2,
    Equivocation = 3,
}

/// Result of a validator vote.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteOutcome {
    /// Vote recorded; carries the current approval count for that target.
    Pending(u32),
    /// Quorum reached; the subject moved to this state.
    Committed(RegulatoryState),
    /// Vote rejected as rogue behaviour (strike recorded).
    Rejected(RogueReason),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsensusConfig {
    pub validators: Vec<Address>,
    pub quorum: u32,
}

/// Committed state of a subject and the round currently open for voting.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectRecord {
    pub state: RegulatoryState,
    pub round: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatorRecord {
    pub strikes: u32,
    pub quarantined: bool,
}

/// Votes for one candidate target state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub to: RegulatoryState,
    pub approvals: Vec<Address>,
}

/// Open votes for a subject's current round.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoundVotes {
    pub round: u32,
    pub candidates: Vec<Candidate>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsensusKey {
    Config,
    Subject(BytesN<32>),
    Votes(BytesN<32>),
    Validator(Address),
}

/// Canonical regulatory transition table.
pub fn is_valid_transition(from: RegulatoryState, to: RegulatoryState) -> bool {
    use RegulatoryState::*;
    matches!(
        (from, to),
        (Draft, Submitted)
            | (Submitted, UnderReview)
            | (Submitted, Draft)
            | (UnderReview, Approved)
            | (UnderReview, Rejected)
            | (Approved, Suspended)
            | (Approved, Revoked)
            | (Suspended, Approved)
            | (Suspended, Revoked)
            | (Rejected, Draft)
    )
}

pub struct RegulatoryConsensus;

impl RegulatoryConsensus {
    /// Installs the validator set and quorum. Requires `Admin`.
    pub fn configure(
        env: &Env,
        caller: &Address,
        validators: Vec<Address>,
        quorum: u32,
    ) -> Result<(), ConsensusError> {
        RbacManager::require_role_precedence(env, caller, Role::Admin)
            .map_err(|_| ConsensusError::Unauthorized)?;

        let n = validators.len();
        if n == 0 || n > MAX_VALIDATORS || quorum > n || quorum.saturating_mul(3) <= n.saturating_mul(2) {
            return Err(ConsensusError::InvalidConfig);
        }
        for i in 0..n {
            let v = validators.get_unchecked(i);
            for j in (i + 1)..n {
                if validators.get_unchecked(j) == v {
                    return Err(ConsensusError::InvalidConfig);
                }
            }
        }

        let config = ConsensusConfig {
            validators: validators.clone(),
            quorum,
        };
        env.storage().persistent().set(&ConsensusKey::Config, &config);
        env.events().publish(
            (Symbol::new(env, EVENT_TOPIC), Symbol::new(env, "config")),
            (caller.clone(), validators, quorum),
        );
        Ok(())
    }

    pub fn get_config(env: &Env) -> Option<ConsensusConfig> {
        env.storage().persistent().get(&ConsensusKey::Config)
    }

    pub fn get_subject(env: &Env, subject: &BytesN<32>) -> SubjectRecord {
        env.storage()
            .persistent()
            .get(&ConsensusKey::Subject(subject.clone()))
            .unwrap_or(SubjectRecord {
                state: RegulatoryState::Draft,
                round: 0,
            })
    }

    pub fn get_validator(env: &Env, validator: &Address) -> ValidatorRecord {
        env.storage()
            .persistent()
            .get(&ConsensusKey::Validator(validator.clone()))
            .unwrap_or(ValidatorRecord {
                strikes: 0,
                quarantined: false,
            })
    }

    pub fn get_votes(env: &Env, subject: &BytesN<32>) -> Option<RoundVotes> {
        env.storage()
            .persistent()
            .get(&ConsensusKey::Votes(subject.clone()))
    }

    /// Casts `validator`'s vote to move `subject` from `from` to `to` in `round`.
    pub fn propose(
        env: &Env,
        validator: &Address,
        subject: &BytesN<32>,
        round: u32,
        from: RegulatoryState,
        to: RegulatoryState,
    ) -> Result<VoteOutcome, ConsensusError> {
        validator.require_auth();

        let config = Self::get_config(env).ok_or(ConsensusError::NotConfigured)?;
        if !config.validators.contains(validator) {
            return Err(ConsensusError::NotValidator);
        }
        if Self::get_validator(env, validator).quarantined {
            return Err(ConsensusError::ValidatorQuarantined);
        }

        let mut record = Self::get_subject(env, subject);
        if round != record.round {
            // Lagging honest nodes legitimately hit this, so it is not a strike.
            return Err(ConsensusError::StaleRound);
        }
        if from != record.state {
            return Ok(Self::strike(env, validator, subject, RogueReason::ForgedPriorState));
        }
        if !is_valid_transition(from, to) {
            return Ok(Self::strike(env, validator, subject, RogueReason::InvalidTransition));
        }

        let mut votes = match Self::get_votes(env, subject) {
            Some(v) if v.round == round => v,
            _ => RoundVotes {
                round,
                candidates: Vec::new(env),
            },
        };

        // One vote per validator per round; a second, different target is equivocation.
        let mut target_idx: Option<u32> = None;
        for i in 0..votes.candidates.len() {
            let c = votes.candidates.get_unchecked(i);
            if c.approvals.contains(validator) {
                if c.to == to {
                    return Err(ConsensusError::DuplicateVote);
                }
                return Ok(Self::strike(env, validator, subject, RogueReason::Equivocation));
            }
            if c.to == to {
                target_idx = Some(i);
            }
        }

        let mut candidate = match target_idx {
            Some(i) => votes.candidates.get_unchecked(i),
            None => Candidate {
                to,
                approvals: Vec::new(env),
            },
        };
        candidate.approvals.push_back(validator.clone());
        let approvals = Self::effective_approvals(env, &candidate.approvals);

        if approvals >= config.quorum {
            record.state = to;
            record.round = round.saturating_add(1);
            env.storage()
                .persistent()
                .set(&ConsensusKey::Subject(subject.clone()), &record);
            env.storage()
                .persistent()
                .remove(&ConsensusKey::Votes(subject.clone()));
            env.events().publish(
                (Symbol::new(env, EVENT_TOPIC), Symbol::new(env, "commit")),
                (subject.clone(), round, from, to),
            );
            return Ok(VoteOutcome::Committed(to));
        }

        match target_idx {
            Some(i) => votes.candidates.set(i, candidate),
            None => votes.candidates.push_back(candidate),
        }
        env.storage()
            .persistent()
            .set(&ConsensusKey::Votes(subject.clone()), &votes);
        env.events().publish(
            (Symbol::new(env, EVENT_TOPIC), Symbol::new(env, "vote")),
            (subject.clone(), validator.clone(), round, from, to, approvals),
        );
        Ok(VoteOutcome::Pending(approvals))
    }

    /// Counts approvals from validators that are not quarantined.
    fn effective_approvals(env: &Env, approvals: &Vec<Address>) -> u32 {
        let mut n = 0u32;
        for a in approvals.iter() {
            if !Self::get_validator(env, &a).quarantined {
                n += 1;
            }
        }
        n
    }

    /// Records a strike (quarantining on equivocation or at the threshold)
    /// and emits rogue-detection events.
    fn strike(env: &Env, validator: &Address, subject: &BytesN<32>, reason: RogueReason) -> VoteOutcome {
        let mut rec = Self::get_validator(env, validator);
        rec.strikes = rec.strikes.saturating_add(1);
        let quarantine = reason == RogueReason::Equivocation || rec.strikes >= QUARANTINE_STRIKES;
        rec.quarantined = rec.quarantined || quarantine;
        env.storage()
            .persistent()
            .set(&ConsensusKey::Validator(validator.clone()), &rec);

        let topic = Symbol::new(env, EVENT_TOPIC);
        env.events().publish(
            (topic.clone(), Symbol::new(env, "rogue")),
            (subject.clone(), validator.clone(), reason as u32, rec.strikes),
        );
        if quarantine {
            env.events().publish(
                (topic, Symbol::new(env, "quarantine")),
                (validator.clone(), rec.strikes),
            );
        }
        VoteOutcome::Rejected(reason)
    }
}
