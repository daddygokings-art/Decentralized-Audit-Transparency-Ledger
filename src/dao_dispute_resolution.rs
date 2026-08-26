/// Dispute Resolution Contract - Decentralized Conflict Resolution
///
/// Handles:
/// - Dispute filing and evidence submission
/// - Juror selection and voting
/// - Slashing of bad actors
/// - Appeal mechanism

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Dispute status lifecycle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DisputeStatus {
    /// Dispute filed, awaiting evidence
    Filed = 0,
    /// Evidence submission closed, awaiting juror decision
    EvidenceSubmitted = 1,
    /// Voting in progress
    Voting = 2,
    /// Voting complete, decision pending appeal
    Decided = 3,
    /// Appeal filed
    Appealed = 4,
    /// Final decision, appeal rejected
    Final = 5,
    /// Dismissed
    Dismissed = 6,
}

/// Dispute vote outcome
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DisputeOutcome {
    /// Plaintiff wins
    PlaintiffWins = 0,
    /// Defendant wins
    DefendantWins = 1,
    /// Partially upheld
    PartiallyUpheld = 2,
    /// Dismissed (insufficient evidence)
    Dismissed = 3,
}

/// A dispute record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    /// Unique dispute ID
    pub dispute_id: u32,
    /// Plaintiff (filer)
    pub plaintiff: Address,
    /// Defendant
    pub defendant: Address,
    /// Description of dispute
    pub description: String,
    /// Evidence (IPFS hash or URI)
    pub evidence_uri: String,
    /// Current status
    pub status: DisputeStatus,
    /// Assigned jurors
    pub jurors: Vec<Address>,
    /// Voting results (for, against, abstain)
    pub votes_for: u32,
    pub votes_against: u32,
    pub votes_abstain: u32,
    /// Final outcome
    pub outcome: Option<DisputeOutcome>,
    /// Amount at stake (for slashing)
    pub stake_amount: u128,
    /// Filed ledger
    pub filed_ledger: u32,
    /// Deadline for evidence submission
    pub evidence_deadline: u32,
    /// Voting deadline
    pub voting_deadline: u32,
    /// Number of jurors who have voted
    pub votes_cast: u32,
}

/// Juror record for dispute
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurorAssignment {
    /// Dispute ID
    pub dispute_id: u32,
    /// Juror address
    pub juror: Address,
    /// Whether juror has voted
    pub has_voted: bool,
    /// Juror's vote (if cast)
    pub vote: Option<DisputeOutcome>,
    /// Juror's voting power (stake)
    pub stake: u128,
    /// Reward if juror votes with majority
    pub reward: u128,
}

/// Appeal record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Appeal {
    /// Appeal ID
    pub appeal_id: u32,
    /// Original dispute ID
    pub dispute_id: u32,
    /// Appellant address
    pub appellant: Address,
    /// Appeal reason
    pub reason: String,
    /// Higher quorum required
    pub higher_quorum_required: u32,
    /// Status
    pub status: DisputeStatus,
}

/// Dispute configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeConfig {
    /// Number of jurors per dispute
    pub jurors_per_dispute: u32,
    /// Evidence submission period in ledgers
    pub evidence_period: u32,
    /// Voting period in ledgers
    pub voting_period: u32,
    /// Juror reward (in basis points of stake)
    pub juror_reward_bps: u32,
    /// Slashing amount for bad actor (in basis points)
    pub slashing_bps: u32,
    /// Minimum stake to be eligible as juror
    pub min_juror_stake: u128,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum DisputeKey {
    /// Owner of dispute resolution system
    Owner,
    /// Configuration
    Config,
    /// Dispute details: u32 (dispute_id) → Dispute
    Dispute(u32),
    /// Next dispute ID counter
    DisputeCounter,
    /// Juror assignment: (u32, Address) → JurorAssignment
    JurorAssignment(u32, Address),
    /// Juror vote: (u32, Address) → DisputeOutcome
    JurorVote(u32, Address),
    /// Total slashed from address
    TotalSlashed(Address),
    /// Appeal record: u32 (appeal_id) → Appeal
    Appeal(u32),
    /// Next appeal ID counter
    AppealCounter,
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DisputeError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Dispute not found
    DisputeNotFound = 2,
    /// Evidence period not open
    EvEvidencePeriodClosed = 3,
    /// Juror not assigned to this dispute
    NotAssignedAsJuror = 4,
    /// Juror already voted
    AlreadyVoted = 5,
    /// Voting period not open
    VotingPeriodClosed = 6,
    /// Insufficient juror votes
    InsufficientVotes = 7,
    /// Cannot appeal (no grounds or already final)
    CannotAppeal = 8,
    /// Appeal already filed
    AppealAlreadyFiled = 9,
    /// Insufficient juror stake
    InsufficientJurorStake = 10,
    /// Dispute already decided
    DisputeAlreadyDecided = 11,
    /// Invalid dispute status
    InvalidStatus = 12,
    /// Plaintiff cannot be defendant
    InvalidParties = 13,
    /// Invalid outcome
    InvalidOutcome = 14,
}

// ============================================================================
// Dispute Resolution Contract
// ============================================================================

#[contract]
pub struct DAODisputeResolution;

#[contractimpl]
impl DAODisputeResolution {
    /// Initialize dispute resolution (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        jurors_per_dispute: u32,
        evidence_period: u32,
        voting_period: u32,
        juror_reward_bps: u32,
        slashing_bps: u32,
        min_juror_stake: u128,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&DisputeKey::Owner) {
            panic_with_error!(&env, DisputeError::Unauthorized);
        }

        let config = DisputeConfig {
            jurors_per_dispute,
            evidence_period,
            voting_period,
            juror_reward_bps,
            slashing_bps,
            min_juror_stake,
        };

        env.storage().instance().set(&DisputeKey::Owner, &owner);
        env.storage().instance().set(&DisputeKey::Config, &config);
        env.storage()
            .instance()
            .set(&DisputeKey::DisputeCounter, &0u32);
        env.storage()
            .instance()
            .set(&DisputeKey::AppealCounter, &0u32);

        log!(
            &env,
            "DAODisputeResolution: initialized - jurors_per_dispute={}",
            jurors_per_dispute
        );
    }

    // ========================================================================
    // Dispute Filing
    // ========================================================================

    /// File a new dispute
    pub fn file_dispute(
        env: Env,
        defendant: Address,
        description: String,
        evidence_uri: String,
        stake_amount: u128,
    ) -> u32 {
        let plaintiff = env.invoker();

        if plaintiff == defendant {
            panic_with_error!(&env, DisputeError::InvalidParties);
        }

        let config = Self::get_config(&env);
        let dispute_id = Self::get_next_dispute_id(&env);
        let current_ledger = env.ledger().sequence();

        let dispute = Dispute {
            dispute_id,
            plaintiff,
            defendant,
            description,
            evidence_uri,
            status: DisputeStatus::Filed,
            jurors: Vec::new(&env),
            votes_for: 0,
            votes_against: 0,
            votes_abstain: 0,
            outcome: None,
            stake_amount,
            filed_ledger: current_ledger,
            evidence_deadline: current_ledger + config.evidence_period,
            voting_deadline: 0, // Set after evidence period
            votes_cast: 0,
        };

        env.storage()
            .instance()
            .set(&DisputeKey::Dispute(dispute_id), &dispute);

        // TODO: Collect stake from plaintiff

        log!(
            &env,
            "DAODisputeResolution: dispute filed - id={}, plaintiff={}, defendant={}",
            dispute_id,
            plaintiff,
            defendant
        );

        dispute_id
    }

    /// Submit evidence for a dispute
    pub fn submit_evidence(
        env: Env,
        dispute_id: u32,
        evidence_uri: String,
        is_plaintiff: bool,
    ) {
        let submitter = env.invoker();
        let mut dispute = Self::get_dispute_or_panic(&env, dispute_id);
        let config = Self::get_config(&env);
        let current_ledger = env.ledger().sequence();

        if dispute.status != DisputeStatus::Filed {
            panic_with_error!(&env, DisputeError::EvEvidencePeriodClosed);
        }

        if current_ledger > dispute.evidence_deadline {
            panic_with_error!(&env, DisputeError::EvEvidencePeriodClosed);
        }

        // Verify submitter is plaintiff or defendant
        if is_plaintiff && submitter != dispute.plaintiff {
            panic_with_error!(&env, DisputeError::Unauthorized);
        }
        if !is_plaintiff && submitter != dispute.defendant {
            panic_with_error!(&env, DisputeError::Unauthorized);
        }

        // Update evidence (append to URI or create new reference)
        dispute.evidence_uri = evidence_uri;
        
        // If both parties have submitted, close evidence period
        dispute.status = DisputeStatus::EvidenceSubmitted;
        dispute.voting_deadline = current_ledger + config.voting_period;

        env.storage()
            .instance()
            .set(&DisputeKey::Dispute(dispute_id), &dispute);

        log!(
            &env,
            "DAODisputeResolution: evidence submitted - dispute={}, submitter={}",
            dispute_id,
            submitter
        );
    }

    // ========================================================================
    // Juror Management & Voting
    // ========================================================================

    /// Assign jurors to dispute (owner-only, off-chain random selection)
    pub fn assign_jurors(env: Env, dispute_id: u32, jurors: Vec<Address>) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let config = Self::get_config(&env);
        let mut dispute = Self::get_dispute_or_panic(&env, dispute_id);

        if jurors.len() as u32 != config.jurors_per_dispute {
            panic_with_error!(&env, DisputeError::InvalidStatus);
        }

        for juror in jurors.iter() {
            // TODO: Verify juror has sufficient stake
            
            let assignment = JurorAssignment {
                dispute_id,
                juror: juror.clone(),
                has_voted: false,
                vote: None,
                stake: 0, // TODO: Get from governance token
                reward: 0,
            };

            env.storage()
                .instance()
                .set(&DisputeKey::JurorAssignment(dispute_id, juror.clone()), &assignment);
        }

        dispute.jurors = jurors.clone();
        dispute.status = DisputeStatus::Voting;

        env.storage()
            .instance()
            .set(&DisputeKey::Dispute(dispute_id), &dispute);

        log!(
            &env,
            "DAODisputeResolution: jurors assigned - dispute={}, count={}",
            dispute_id,
            config.jurors_per_dispute
        );
    }

    /// Cast vote as juror
    pub fn cast_juror_vote(env: Env, dispute_id: u32, outcome: DisputeOutcome) {
        let juror = env.invoker();
        let mut dispute = Self::get_dispute_or_panic(&env, dispute_id);
        let current_ledger = env.ledger().sequence();

        if dispute.status != DisputeStatus::Voting {
            panic_with_error!(&env, DisputeError::VotingPeriodClosed);
        }

        if current_ledger > dispute.voting_deadline {
            panic_with_error!(&env, DisputeError::VotingPeriodClosed);
        }

        let mut assignment = env
            .storage()
            .instance()
            .get::<_, JurorAssignment>(&DisputeKey::JurorAssignment(dispute_id, juror.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, DisputeError::NotAssignedAsJuror));

        if assignment.has_voted {
            panic_with_error!(&env, DisputeError::AlreadyVoted);
        }

        // Record vote
        assignment.has_voted = true;
        assignment.vote = Some(outcome);

        env.storage()
            .instance()
            .set(&DisputeKey::JurorAssignment(dispute_id, juror.clone()), &assignment);
        env.storage()
            .instance()
            .set(&DisputeKey::JurorVote(dispute_id, juror.clone()), &outcome);

        // Update dispute vote tallies
        match outcome {
            DisputeOutcome::PlaintiffWins => dispute.votes_for += 1,
            DisputeOutcome::DefendantWins => dispute.votes_against += 1,
            _ => dispute.votes_abstain += 1,
        }
        dispute.votes_cast += 1;

        env.storage()
            .instance()
            .set(&DisputeKey::Dispute(dispute_id), &dispute);

        log!(
            &env,
            "DAODisputeResolution: vote cast - dispute={}, juror={}, votes_cast={}",
            dispute_id,
            juror,
            dispute.votes_cast
        );
    }

    /// Tally votes and finalize dispute
    pub fn finalize_dispute(env: Env, dispute_id: u32) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let mut dispute = Self::get_dispute_or_panic(&env, dispute_id);
        let config = Self::get_config(&env);
        let current_ledger = env.ledger().sequence();

        if dispute.status != DisputeStatus::Voting {
            panic_with_error!(&env, DisputeError::InvalidStatus);
        }

        if current_ledger <= dispute.voting_deadline {
            panic_with_error!(&env, DisputeError::VotingPeriodClosed);
        }

        // Determine outcome based on votes
        let outcome = if dispute.votes_for > dispute.votes_against {
            DisputeOutcome::PlaintiffWins
        } else if dispute.votes_against > dispute.votes_for {
            DisputeOutcome::DefendantWins
        } else {
            DisputeOutcome::Dismissed // Tie = dismissed
        };

        dispute.outcome = Some(outcome);
        dispute.status = DisputeStatus::Decided;

        // Apply slashing if defendant loses
        if outcome == DisputeOutcome::PlaintiffWins {
            // TODO: Slash defendant
            let slashed_amount = (dispute.stake_amount * config.slashing_bps as u128) / 10_000u128;
            let current_total = env
                .storage()
                .instance()
                .get::<_, u128>(&DisputeKey::TotalSlashed(dispute.defendant.clone()))
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DisputeKey::TotalSlashed(dispute.defendant.clone()), &(current_total + slashed_amount));
        }

        // Distribute rewards to majority-voting jurors
        for juror in dispute.jurors.iter() {
            if let Some(juror_assignment) = env
                .storage()
                .instance()
                .get::<_, JurorAssignment>(&DisputeKey::JurorAssignment(dispute_id, juror.clone()))
            {
                if let Some(juror_vote) = juror_assignment.vote {
                    if (outcome == DisputeOutcome::PlaintiffWins && juror_vote == DisputeOutcome::PlaintiffWins) ||
                       (outcome == DisputeOutcome::DefendantWins && juror_vote == DisputeOutcome::DefendantWins) {
                        // Juror voted with majority - reward them
                        // TODO: Transfer reward
                    }
                }
            }
        }

        env.storage()
            .instance()
            .set(&DisputeKey::Dispute(dispute_id), &dispute);

        log!(
            &env,
            "DAODisputeResolution: dispute finalized - id={}, outcome={}",
            dispute_id,
            outcome as u32
        );
    }

    // ========================================================================
    // Appeals
    // ========================================================================

    /// File an appeal
    pub fn file_appeal(env: Env, dispute_id: u32, reason: String) -> u32 {
        let appellant = env.invoker();
        let dispute = Self::get_dispute_or_panic(&env, dispute_id);

        if dispute.status != DisputeStatus::Decided {
            panic_with_error!(&env, DisputeError::CannotAppeal);
        }

        // Only plaintiff or defendant can appeal
        if appellant != dispute.plaintiff && appellant != dispute.defendant {
            panic_with_error!(&env, DisputeError::Unauthorized);
        }

        let appeal_id = Self::get_next_appeal_id(&env);

        let appeal = Appeal {
            appeal_id,
            dispute_id,
            appellant,
            reason,
            higher_quorum_required: 2, // TODO: Calculate higher quorum
            status: DisputeStatus::Appealed,
        };

        env.storage()
            .instance()
            .set(&DisputeKey::Appeal(appeal_id), &appeal);

        log!(
            &env,
            "DAODisputeResolution: appeal filed - id={}, dispute={}, appellant={}",
            appeal_id,
            dispute_id,
            appellant
        );

        appeal_id
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get dispute details
    pub fn get_dispute(env: Env, dispute_id: u32) -> Option<Dispute> {
        env.storage()
            .instance()
            .get(&DisputeKey::Dispute(dispute_id))
    }

    /// Get configuration
    pub fn get_dispute_config(env: Env) -> DisputeConfig {
        Self::get_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DisputeKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, DisputeError::Unauthorized))
    }

    fn get_config(env: &Env) -> DisputeConfig {
        env.storage()
            .instance()
            .get(&DisputeKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, DisputeError::Unauthorized))
    }

    fn get_dispute_or_panic(env: &Env, dispute_id: u32) -> Dispute {
        env.storage()
            .instance()
            .get(&DisputeKey::Dispute(dispute_id))
            .unwrap_or_else(|| panic_with_error!(env, DisputeError::DisputeNotFound))
    }

    fn get_next_dispute_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DisputeKey::DisputeCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&DisputeKey::DisputeCounter, &(current + 1));

        current
    }

    fn get_next_appeal_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DisputeKey::AppealCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&DisputeKey::AppealCounter, &(current + 1));

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispute_filing() {
        // Tests for dispute creation
    }

    #[test]
    fn test_juror_voting() {
        // Tests for voting mechanism
    }

    #[test]
    fn test_dispute_resolution() {
        // Tests for finalization
    }

    #[test]
    fn test_appeals() {
        // Tests for appeal mechanism
    }
}
