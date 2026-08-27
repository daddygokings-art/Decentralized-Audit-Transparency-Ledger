/// DAO Governance System - Platform Decisions via Decentralized Voting
///
/// Architecture:
/// 1. Governance Token: Voting power (can be delegated)
/// 2. Proposals: Parameterized changes, feature priorities, treasury allocation
/// 3. Voting: Vote escrow model with delegation support
/// 4. Timelock: Delay execution for security
/// 5. Execution: Enact approved proposals
/// 6. Treasury: Multi-sig fund management
/// 7. Disputes: Resolve disagreements with voting

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, String, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Proposal types (extensible)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ProposalType {
    /// Parameter change (e.g., tier pricing, fee structure)
    ParameterChange = 0,
    /// Feature priority vote
    FeaturePriority = 1,
    /// Treasury spending allocation
    TreasurySpending = 2,
    /// Emergency protocol pause
    EmergencyPause = 3,
    /// Contract upgrade
    ContractUpgrade = 4,
}

/// Proposal status lifecycle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ProposalStatus {
    /// Voting in progress
    Active = 0,
    /// Voting completed, awaiting execution
    Passed = 1,
    /// Voting completed, proposal rejected
    Defeated = 2,
    /// Executed successfully
    Executed = 3,
    /// Cancelled by proposer or admin
    Cancelled = 4,
    /// Execution failed
    ExecutionFailed = 5,
}

/// Voting choices
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum VoteChoice {
    /// Vote in favor
    For = 0,
    /// Vote against
    Against = 1,
    /// Abstain (counted towards quorum but not voting power)
    Abstain = 2,
}

/// Governance proposal
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    /// Unique proposal ID
    pub proposal_id: u32,
    /// Type of proposal
    pub proposal_type: ProposalType,
    /// Proposer address
    pub proposer: Address,
    /// Human-readable title
    pub title: String,
    /// Detailed description (off-chain reference)
    pub description: String,
    /// JSON-encoded proposal parameters
    pub parameters: Bytes,
    /// Voting start ledger
    pub start_ledger: u32,
    /// Voting end ledger
    pub end_ledger: u32,
    /// Execution earliest ledger (after timelock)
    pub execution_ledger: u32,
    /// Votes in favor (scaled by decimals)
    pub votes_for: u128,
    /// Votes against (scaled by decimals)
    pub votes_against: u128,
    /// Votes abstain (counted for quorum)
    pub votes_abstain: u128,
    /// Current status
    pub status: ProposalStatus,
    /// Quorum requirement in basis points (e.g., 4000 = 40%)
    pub quorum_bps: u32,
    /// Approval threshold in basis points (e.g., 5000 = 50%)
    pub approval_threshold_bps: u32,
    /// Whether proposal can still be cancelled
    pub cancellable: bool,
}

/// User's voting power and delegation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VotingPower {
    /// User address
    pub user: Address,
    /// Base voting power (from governance token holdings)
    pub base_power: u128,
    /// Delegated voting power (received from others)
    pub delegated_power: u128,
    /// Current delegate (who user is delegating to)
    pub delegated_to: Option<Address>,
    /// Ledger height when delegation was set
    pub delegation_ledger: u32,
}

/// User's vote on a proposal
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    /// Voter address
    pub voter: Address,
    /// Proposal voted on
    pub proposal_id: u32,
    /// Choice (For, Against, Abstain)
    pub choice: VoteChoice,
    /// Voting power used (at time of vote)
    pub power: u128,
    /// Ledger where vote was cast
    pub vote_ledger: u32,
}

/// Governance configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    /// Governance token address
    pub token: Address,
    /// Voting delay in ledgers (time before voting can start after proposal)
    pub voting_delay: u32,
    /// Voting period in ledgers
    pub voting_period: u32,
    /// Timelock delay in ledgers (before execution)
    pub timelock_delay: u32,
    /// Default quorum in basis points
    pub default_quorum_bps: u32,
    /// Default approval threshold
    pub default_approval_threshold_bps: u32,
    /// Proposal fee (to prevent spam)
    pub proposal_fee: u128,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum GovernanceKey {
    /// DAO owner (for emergency functions)
    Owner,
    /// Governance configuration
    Config,
    /// Governance token address
    GovernanceToken,
    /// Next proposal ID counter
    ProposalCounter,
    /// Proposal details: u32 (proposal_id) → Proposal
    Proposal(u32),
    /// User voting power: Address → VotingPower
    VotingPower(Address),
    /// User votes on proposal: (Address, u32) → Vote
    ProposalVote(Address, u32),
    /// Has user voted on proposal: (Address, u32) → bool
    HasVoted(Address, u32),
    /// Total voting power participating in proposal (for quorum)
    ProposalParticipation(u32),
    /// Active proposals list
    ActiveProposals,
    /// Treasury address
    Treasury,
    /// DAO governance token balance per user
    GovernanceTokenBalance(Address),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GovernanceError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Proposal does not exist
    ProposalNotFound = 2,
    /// Voting period has ended
    VotingClosed = 3,
    /// User has already voted on this proposal
    AlreadyVoted = 4,
    /// User has insufficient voting power
    InsufficientVotingPower = 5,
    /// Proposal cannot be executed yet (timelock not passed)
    TimelockNotPassed = 6,
    /// Proposal status does not allow this action
    InvalidStatus = 7,
    /// Quorum not reached
    QuorumNotReached = 8,
    /// Proposal did not pass (votes for <= votes against)
    ProposalFailed = 9,
    /// Invalid proposal parameters
    InvalidProposal = 10,
    /// Proposal fee not paid
    ProposalFeeNotPaid = 11,
    /// Cannot delegate to self
    CannotDelegateToSelf = 12,
    /// No voting power to delegate
    NoVotingPower = 13,
    /// Treasury insufficient funds
    TreasuryInsufficientFunds = 14,
    /// Treasury transaction failed
    TreasuryExecutionFailed = 15,
    /// Dispute resolution failed
    DisputeResolutionFailed = 16,
    /// Invalid vote choice
    InvalidVoteChoice = 17,
    /// Voting not yet started
    VotingNotStarted = 18,
}

// ============================================================================
// Core Governance Contract
// ============================================================================

#[contract]
pub struct DAOGovernance;

#[contractimpl]
impl DAOGovernance {
    /// Initialize DAO governance (owner-only)
    pub fn initialize(
        env: Env,
        owner: Address,
        governance_token: Address,
        voting_delay: u32,
        voting_period: u32,
        timelock_delay: u32,
        default_quorum_bps: u32,
        default_approval_threshold_bps: u32,
        proposal_fee: u128,
    ) {
        owner.require_auth();

        if env.storage().instance().has(&GovernanceKey::Owner) {
            panic_with_error!(&env, GovernanceError::Unauthorized);
        }

        let config = GovernanceConfig {
            token: governance_token,
            voting_delay,
            voting_period,
            timelock_delay,
            default_quorum_bps,
            default_approval_threshold_bps,
            proposal_fee,
        };

        env.storage().instance().set(&GovernanceKey::Owner, &owner);
        env.storage().instance().set(&GovernanceKey::Config, &config);
        env.storage()
            .instance()
            .set(&GovernanceKey::ProposalCounter, &0u32);

        log!(
            &env,
            "DAOGovernance: initialized - voting_period={}, timelock_delay={}",
            voting_period,
            timelock_delay
        );
    }

    // ========================================================================
    // Proposal Management
    // ========================================================================

    /// Create a new proposal
    pub fn propose(
        env: Env,
        proposal_type: ProposalType,
        title: String,
        description: String,
        parameters: Bytes,
        quorum_bps: u32,
        approval_threshold_bps: u32,
    ) -> u32 {
        let proposer = env.invoker();
        let config = Self::get_config(&env);

        // Check proposal fee
        // TODO: Transfer proposal fee from proposer to treasury

        // Check proposer has voting power
        let voting_power = Self::get_voting_power(&env, &proposer);
        if voting_power.base_power + voting_power.delegated_power == 0 {
            panic_with_error!(&env, GovernanceError::InsufficientVotingPower);
        }

        let proposal_id = Self::get_next_proposal_id(&env);
        let current_ledger = env.ledger().sequence();

        let proposal = Proposal {
            proposal_id,
            proposal_type,
            proposer,
            title,
            description,
            parameters,
            start_ledger: current_ledger + config.voting_delay,
            end_ledger: current_ledger + config.voting_delay + config.voting_period,
            execution_ledger: current_ledger + config.voting_delay + config.voting_period + config.timelock_delay,
            votes_for: 0,
            votes_against: 0,
            votes_abstain: 0,
            status: ProposalStatus::Active,
            quorum_bps: if quorum_bps > 0 { quorum_bps } else { config.default_quorum_bps },
            approval_threshold_bps: if approval_threshold_bps > 0 { approval_threshold_bps } else { config.default_approval_threshold_bps },
            cancellable: true,
        };

        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);

        log!(
            &env,
            "DAOGovernance: proposal created - id={}, type={}, proposer={}",
            proposal_id,
            proposal_type as u32,
            proposer
        );

        proposal_id
    }

    /// Cast a vote on a proposal
    pub fn vote(env: Env, proposal_id: u32, choice: VoteChoice) {
        let voter = env.invoker();

        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        let config = Self::get_config(&env);
        let current_ledger = env.ledger().sequence();

        // Check voting is active
        if current_ledger < proposal.start_ledger {
            panic_with_error!(&env, GovernanceError::VotingNotStarted);
        }
        if current_ledger > proposal.end_ledger {
            panic_with_error!(&env, GovernanceError::VotingClosed);
        }

        // Check user hasn't voted yet
        if env
            .storage()
            .instance()
            .get::<_, bool>(&GovernanceKey::HasVoted(voter.clone(), proposal_id))
            .unwrap_or(false)
        {
            panic_with_error!(&env, GovernanceError::AlreadyVoted);
        }

        // Get voter's voting power
        let voting_power = Self::get_voting_power(&env, &voter);
        let total_power = voting_power.base_power + voting_power.delegated_power;

        if total_power == 0 {
            panic_with_error!(&env, GovernanceError::InsufficientVotingPower);
        }

        // Record the vote
        let vote = Vote {
            voter: voter.clone(),
            proposal_id,
            choice,
            power: total_power,
            vote_ledger: current_ledger,
        };

        env.storage()
            .instance()
            .set(&GovernanceKey::ProposalVote(voter.clone(), proposal_id), &vote);
        env.storage()
            .instance()
            .set(&GovernanceKey::HasVoted(voter.clone(), proposal_id), &true);

        // Update proposal vote counts
        match choice {
            VoteChoice::For => proposal.votes_for += total_power,
            VoteChoice::Against => proposal.votes_against += total_power,
            VoteChoice::Abstain => proposal.votes_abstain += total_power,
        }

        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);

        log!(
            &env,
            "DAOGovernance: vote cast - proposal={}, voter={}, power={}, choice={}",
            proposal_id,
            voter,
            total_power,
            choice as u32
        );
    }

    /// Delegate voting power to another address
    pub fn delegate(env: Env, delegate_to: Address) {
        let delegator = env.invoker();

        if delegator == delegate_to {
            panic_with_error!(&env, GovernanceError::CannotDelegateToSelf);
        }

        let mut voting_power = Self::get_voting_power(&env, &delegator);

        if voting_power.base_power == 0 {
            panic_with_error!(&env, GovernanceError::NoVotingPower);
        }

        // Update delegator's delegation
        voting_power.delegated_to = Some(delegate_to.clone());
        voting_power.delegation_ledger = env.ledger().sequence();

        // Update delegate's received power
        let mut delegate_power = Self::get_voting_power(&env, &delegate_to);
        delegate_power.delegated_power += voting_power.base_power;

        env.storage()
            .instance()
            .set(&GovernanceKey::VotingPower(delegator.clone()), &voting_power);
        env.storage()
            .instance()
            .set(&GovernanceKey::VotingPower(delegate_to.clone()), &delegate_power);

        log!(
            &env,
            "DAOGovernance: delegation - from={}, to={}, power={}",
            delegator,
            delegate_to,
            voting_power.base_power
        );
    }

    /// Revoke delegation
    pub fn undelegate(env: Env) {
        let delegator = env.invoker();

        let mut voting_power = Self::get_voting_power(&env, &delegator);

        if let Some(delegate) = voting_power.delegated_to.clone() {
            // Reduce delegate's received power
            let mut delegate_power = Self::get_voting_power(&env, &delegate);
            delegate_power.delegated_power -= voting_power.base_power;

            env.storage()
                .instance()
                .set(&GovernanceKey::VotingPower(delegate), &delegate_power);
        }

        // Clear delegation
        voting_power.delegated_to = None;

        env.storage()
            .instance()
            .set(&GovernanceKey::VotingPower(delegator.clone()), &voting_power);

        log!(
            &env,
            "DAOGovernance: delegation revoked - user={}",
            delegator
        );
    }

    // ========================================================================
    // Proposal Execution
    // ========================================================================

    /// Check if proposal has passed
    pub fn proposal_passed(env: Env, proposal_id: u32) -> bool {
        let proposal = Self::get_proposal_or_panic(&env, proposal_id);
        let current_ledger = env.ledger().sequence();

        // Voting must be complete
        if current_ledger <= proposal.end_ledger {
            return false;
        }

        // Check quorum
        let total_participation = proposal.votes_for + proposal.votes_against + proposal.votes_abstain;
        // TODO: Get total governance token supply to calculate quorum percentage
        // For now, use vote counts as proxy

        // Check approval
        proposal.votes_for > proposal.votes_against
    }

    /// Execute a passed proposal
    pub fn execute_proposal(env: Env, proposal_id: u32) {
        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        let current_ledger = env.ledger().sequence();

        // Check timelock has passed
        if current_ledger < proposal.execution_ledger {
            panic_with_error!(&env, GovernanceError::TimelockNotPassed);
        }

        // Check proposal passed
        if !Self::proposal_passed(&env, proposal_id) {
            panic_with_error!(&env, GovernanceError::ProposalFailed);
        }

        // Update status and execute
        proposal.status = ProposalStatus::Executed;

        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);

        // TODO: Execute proposal based on type
        // - ParameterChange: update relevant parameter
        // - FeaturePriority: record decision for off-chain processing
        // - TreasurySpending: transfer funds from treasury
        // - EmergencyPause: pause contract operations
        // - ContractUpgrade: upgrade contract code

        log!(
            &env,
            "DAOGovernance: proposal executed - id={}, type={}",
            proposal_id,
            proposal.proposal_type as u32
        );
    }

    /// Cancel a proposal (proposer or owner)
    pub fn cancel_proposal(env: Env, proposal_id: u32) {
        let caller = env.invoker();
        let mut proposal = Self::get_proposal_or_panic(&env, proposal_id);
        let owner = Self::get_owner(&env);

        if caller != proposal.proposer && caller != owner {
            panic_with_error!(&env, GovernanceError::Unauthorized);
        }

        if !proposal.cancellable {
            panic_with_error!(&env, GovernanceError::InvalidStatus);
        }

        proposal.status = ProposalStatus::Cancelled;

        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);

        log!(
            &env,
            "DAOGovernance: proposal cancelled - id={}",
            proposal_id
        );
    }

    // ========================================================================
    // Governance Configuration
    // ========================================================================

    /// Update governance configuration (owner-only)
    pub fn update_config(
        env: Env,
        voting_delay: Option<u32>,
        voting_period: Option<u32>,
        timelock_delay: Option<u32>,
        default_quorum_bps: Option<u32>,
        default_approval_threshold_bps: Option<u32>,
        proposal_fee: Option<u128>,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let mut config = Self::get_config(&env);

        if let Some(val) = voting_delay { config.voting_delay = val; }
        if let Some(val) = voting_period { config.voting_period = val; }
        if let Some(val) = timelock_delay { config.timelock_delay = val; }
        if let Some(val) = default_quorum_bps { config.default_quorum_bps = val; }
        if let Some(val) = default_approval_threshold_bps { config.default_approval_threshold_bps = val; }
        if let Some(val) = proposal_fee { config.proposal_fee = val; }

        env.storage().instance().set(&GovernanceKey::Config, &config);

        log!(
            &env,
            "DAOGovernance: config updated"
        );
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get a proposal
    pub fn get_proposal(env: Env, proposal_id: u32) -> Option<Proposal> {
        env.storage()
            .instance()
            .get(&GovernanceKey::Proposal(proposal_id))
    }

    /// Get user's voting power
    pub fn get_user_voting_power(env: Env, user: Address) -> VotingPower {
        Self::get_voting_power(&env, &user)
    }

    /// Get governance configuration
    pub fn get_governance_config(env: Env) -> GovernanceConfig {
        Self::get_config(&env)
    }

    /// Get user's vote on a proposal
    pub fn get_user_vote(env: Env, user: Address, proposal_id: u32) -> Option<Vote> {
        env.storage()
            .instance()
            .get(&GovernanceKey::ProposalVote(user, proposal_id))
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&GovernanceKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, GovernanceError::Unauthorized))
    }

    fn get_config(env: &Env) -> GovernanceConfig {
        env.storage()
            .instance()
            .get(&GovernanceKey::Config)
            .unwrap_or_else(|| panic_with_error!(env, GovernanceError::Unauthorized))
    }

    fn get_voting_power(env: &Env, user: &Address) -> VotingPower {
        env.storage()
            .instance()
            .get(&GovernanceKey::VotingPower(user.clone()))
            .unwrap_or(VotingPower {
                user: user.clone(),
                base_power: 0,
                delegated_power: 0,
                delegated_to: None,
                delegation_ledger: 0,
            })
    }

    fn get_proposal_or_panic(env: &Env, proposal_id: u32) -> Proposal {
        env.storage()
            .instance()
            .get(&GovernanceKey::Proposal(proposal_id))
            .unwrap_or_else(|| panic_with_error!(env, GovernanceError::ProposalNotFound))
    }

    fn get_next_proposal_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&GovernanceKey::ProposalCounter)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&GovernanceKey::ProposalCounter, &(current + 1));

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_creation() {
        // Tests for proposal creation
    }

    #[test]
    fn test_voting() {
        // Tests for voting mechanism
    }

    #[test]
    fn test_delegation() {
        // Tests for voting delegation
    }

    #[test]
    fn test_execution() {
        // Tests for proposal execution
    }
}
