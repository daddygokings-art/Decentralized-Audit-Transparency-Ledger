/// Reputation System for Event Submitters
///
/// Implements a reputation system based on:
/// - Event quality
/// - Compliance history
/// - Dispute resolution
/// - Peer reviews
/// - Scoring, tiers, and incentives

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Reputation tiers
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ReputationTier {
    Bronze = 0,
    Silver = 1,
    Gold = 2,
    Platinum = 3,
}

/// A peer review record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReview {
    pub id: BytesN<32>,
    pub reviewer: Address,
    pub subject: Address,
    pub rating: u32,
    pub comment: Bytes,
    pub created_at: u64,
    pub event_ref: BytesN<32>,
}

/// A dispute record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRecord {
    pub id: BytesN<32>,
    pub plaintiff: Address,
    pub defendant: Address,
    pub reason: Bytes,
    pub status: Symbol,
    pub created_at: u64,
    pub resolved_at: u64,
    pub resolution: Bytes,
}

/// A reputation score record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationScore {
    pub submitter: Address,
    pub score: u32,
    pub tier: u32,
    pub event_count: u32,
    pub positive_reviews: u32,
    pub negative_reviews: u32,
    pub disputes_won: u32,
    pub disputes_lost: u32,
    pub compliance_violations: u32,
    pub last_updated: u64,
}

/// An incentive record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncentiveRecord {
    pub id: BytesN<32>,
    pub recipient: Address,
    pub incentive_type: Symbol,
    pub amount: u32,
    pub reason: Bytes,
    pub awarded_at: u64,
}

/// Reputation configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationConfig {
    pub base_score: u32,
    pub bronze_threshold: u32,
    pub silver_threshold: u32,
    pub gold_threshold: u32,
    pub platinum_threshold: u32,
    pub review_weight: u32,
    pub dispute_weight: u32,
    pub compliance_penalty: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum ReputationKey {
    Owner,
    ReputationScore(Address),
    AllSubmitterAddresses,
    PeerReview(BytesN<32>),
    AllReviewIds,
    DisputeRecord(BytesN<32>),
    AllDisputeIds,
    IncentiveRecord(BytesN<32>),
    AllIncentiveIds,
    ReputationConfig,
    NextReviewId,
    NextDisputeId,
    NextIncentiveId,
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReputationError {
    Unauthorized = 1,
    SubmitternotFound = 2,
    ReviewNotFound = 3,
    DisputeNotFound = 4,
    IncentiveNotFound = 5,
    InvalidRating = 6,
    InvalidTier = 7,
    SelfReview = 8,
    DuplicateReview = 9,
    DisputeAlreadyResolved = 10,
    InsufficientReputation = 11,
    ConfigNotFound = 12,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct ReputationSystem;

#[contractimpl]
impl ReputationSystem {
    /// Initialize reputation system (owner-only)
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&ReputationKey::Owner) {
            panic_with_error!(&env, ReputationError::Unauthorized);
        }

        env.storage().instance().set(&ReputationKey::Owner, &owner);
        env.storage().instance().set(&ReputationKey::NextReviewId, &1u32);
        env.storage().instance().set(&ReputationKey::NextDisputeId, &1u32);
        env.storage().instance().set(&ReputationKey::NextIncentiveId, &1u32);

        let config = ReputationConfig {
            base_score: 100,
            bronze_threshold: 100,
            silver_threshold: 500,
            gold_threshold: 1000,
            platinum_threshold: 5000,
            review_weight: 10,
            dispute_weight: 50,
            compliance_penalty: 100,
        };

        env.storage()
            .instance()
            .set(&ReputationKey::ReputationConfig, &config);
    }

    // ========================================================================
    // Reputation Scoring
    // ========================================================================

    /// Initialize or get reputation score for a submitter
    pub fn get_reputation(env: Env, submitter: Address) -> ReputationScore {
        env.storage()
            .instance()
            .get(&ReputationKey::ReputationScore(submitter.clone()))
            .unwrap_or_else(|| {
                let config = Self::get_config(&env);
                ReputationScore {
                    submitter: submitter.clone(),
                    score: config.base_score,
                    tier: 0,
                    event_count: 0,
                    positive_reviews: 0,
                    negative_reviews: 0,
                    disputes_won: 0,
                    disputes_lost: 0,
                    compliance_violations: 0,
                    last_updated: env.ledger().timestamp(),
                }
            })
    }

    /// Update event count for a submitter (called when event is logged)
    pub fn update_event_count(env: Env, caller: Address, submitter: Address) {
        Self::require_owner(&env, &caller);

        let mut score = Self::get_reputation(env.clone(), submitter.clone());
        score.event_count += 1;
        score.last_updated = env.ledger().timestamp();
        score.tier = Self::compute_tier(&env, &score);

        env.storage()
            .instance()
            .set(&ReputationKey::ReputationScore(submitter), &score);
    }

    /// Record a compliance violation for a submitter
    pub fn record_violation(env: Env, caller: Address, submitter: Address) {
        Self::require_owner(&env, &caller);

        let mut score = Self::get_reputation(env.clone(), submitter.clone());
        let config = Self::get_config(&env);

        score.compliance_violations += 1;
        score.score = score.score.saturating_sub(config.compliance_penalty);
        score.last_updated = env.ledger().timestamp();
        score.tier = Self::compute_tier(&env, &score);

        env.storage()
            .instance()
            .set(&ReputationKey::ReputationScore(submitter), &score);
    }

    /// Get reputation configuration
    pub fn get_config(env: Env) -> ReputationConfig {
        env.storage()
            .instance()
            .get(&ReputationKey::ReputationConfig)
            .unwrap_or_else(|| ReputationConfig {
                base_score: 100,
                bronze_threshold: 100,
                silver_threshold: 500,
                gold_threshold: 1000,
                platinum_threshold: 5000,
                review_weight: 10,
                dispute_weight: 50,
                compliance_penalty: 100,
            })
    }

    // ========================================================================
    // Peer Reviews
    // ========================================================================

    /// Submit a peer review for an event
    pub fn submit_review(
        env: Env,
        reviewer: Address,
        subject: Address,
        rating: u32,
        comment: Bytes,
        event_ref: BytesN<32>,
    ) -> PeerReview {
        reviewer.require_auth();

        if reviewer == subject {
            panic_with_error!(&env, ReputationError::SelfReview);
        }

        if rating > 5 {
            panic_with_error!(&env, ReputationError::InvalidRating);
        }

        let id = Self::get_next_review_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let review = PeerReview {
            id: id_bytes.clone(),
            reviewer: reviewer.clone(),
            subject: subject.clone(),
            rating,
            comment,
            created_at: env.ledger().timestamp(),
            event_ref,
        };

        env.storage()
            .instance()
            .set(&ReputationKey::PeerReview(id_bytes.clone()), &review);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&ReputationKey::AllReviewIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&ReputationKey::AllReviewIds, &all_ids);

        let mut score = Self::get_reputation(env.clone(), subject.clone());
        let config = Self::get_config(&env);

        if rating >= 3 {
            score.positive_reviews += 1;
            score.score += config.review_weight;
        } else {
            score.negative_reviews += 1;
            score.score = score.score.saturating_sub(config.review_weight);
        }

        score.last_updated = env.ledger().timestamp();
        score.tier = Self::compute_tier(&env, &score);

        env.storage()
            .instance()
            .set(&ReputationKey::ReputationScore(subject), &score);

        log!(
            &env,
            "ReputationSystem: review submitted - reviewer={}, subject={}, rating={}",
            reviewer,
            subject,
            rating
        );

        review
    }

    /// Get a peer review by ID
    pub fn get_review(env: Env, review_id: BytesN<32>) -> PeerReview {
        Self::get_review_or_panic(&env, review_id)
    }

    /// List all review IDs
    pub fn list_review_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&ReputationKey::AllReviewIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Dispute Resolution
    // ========================================================================

    /// File a dispute against a submitter
    pub fn file_dispute(
        env: Env,
        plaintiff: Address,
        defendant: Address,
        reason: Bytes,
    ) -> DisputeRecord {
        plaintiff.require_auth();

        let id = Self::get_next_dispute_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let dispute = DisputeRecord {
            id: id_bytes.clone(),
            plaintiff: plaintiff.clone(),
            defendant: defendant.clone(),
            reason,
            status: Symbol::new(&env, "open"),
            created_at: env.ledger().timestamp(),
            resolved_at: 0,
            resolution: Bytes::new(&env),
        };

        env.storage()
            .instance()
            .set(&ReputationKey::DisputeRecord(id_bytes.clone()), &dispute);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&ReputationKey::AllDisputeIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&ReputationKey::AllDisputeIds, &all_ids);

        log!(
            &env,
            "ReputationSystem: dispute filed - plaintiff={}, defendant={}",
            plaintiff,
            defendant
        );

        dispute
    }

    /// Resolve a dispute (owner-only)
    pub fn resolve_dispute(
        env: Env,
        caller: Address,
        dispute_id: BytesN<32>,
        defendant_won: bool,
        resolution: Bytes,
    ) {
        Self::require_owner(&env, &caller);

        let mut dispute = Self::get_dispute_or_panic(&env, dispute_id.clone());

        if dispute.status == Symbol::new(&env, "resolved") {
            panic_with_error!(&env, ReputationError::DisputeAlreadyResolved);
        }

        dispute.status = Symbol::new(&env, "resolved");
        dispute.resolved_at = env.ledger().timestamp();
        dispute.resolution = resolution;

        env.storage()
            .instance()
            .set(&ReputationKey::DisputeRecord(dispute_id.clone()), &dispute);

        let mut defendant_score = Self::get_reputation(env.clone(), dispute.defendant.clone());
        let config = Self::get_config(&env);

        if defendant_won {
            defendant_score.disputes_won += 1;
            defendant_score.score += config.dispute_weight;
        } else {
            defendant_score.disputes_lost += 1;
            defendant_score.score = defendant_score.score.saturating_sub(config.dispute_weight);
        }

        defendant_score.last_updated = env.ledger().timestamp();
        defendant_score.tier = Self::compute_tier(&env, &defendant_score);

        env.storage()
            .instance()
            .set(&ReputationKey::ReputationScore(dispute.defendant), &defendant_score);
    }

    /// Get a dispute by ID
    pub fn get_dispute(env: Env, dispute_id: BytesN<32>) -> DisputeRecord {
        Self::get_dispute_or_panic(&env, dispute_id)
    }

    /// List all dispute IDs
    pub fn list_dispute_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&ReputationKey::AllDisputeIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Incentives
    // ========================================================================

    /// Award an incentive to a submitter
    pub fn award_incentive(
        env: Env,
        caller: Address,
        recipient: Address,
        incentive_type: Symbol,
        amount: u32,
        reason: Bytes,
    ) -> IncentiveRecord {
        Self::require_owner(&env, &caller);

        let id = Self::get_next_incentive_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let incentive = IncentiveRecord {
            id: id_bytes.clone(),
            recipient: recipient.clone(),
            incentive_type,
            amount,
            reason,
            awarded_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&ReputationKey::IncentiveRecord(id_bytes.clone()), &incentive);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&ReputationKey::AllIncentiveIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&ReputationKey::AllIncentiveIds, &all_ids);

        log!(
            &env,
            "ReputationSystem: incentive awarded - recipient={}, amount={}",
            recipient,
            amount
        );

        incentive
    }

    /// Get an incentive by ID
    pub fn get_incentive(env: Env, incentive_id: BytesN<32>) -> IncentiveRecord {
        Self::get_incentive_or_panic(&env, incentive_id)
    }

    /// List all incentive IDs
    pub fn list_incentive_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&ReputationKey::AllIncentiveIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Tier Computation
    // ========================================================================

    /// Compute reputation tier from score
    pub fn compute_tier(env: Env, score: &ReputationScore) -> u32 {
        let config = Self::get_config(env);

        if score.score >= config.platinum_threshold {
            3
        } else if score.score >= config.gold_threshold {
            2
        } else if score.score >= config.silver_threshold {
            1
        } else {
            0
        }
    }

    /// Get tier name as string
    pub fn get_tier_name(tier: u32) -> &'static str {
        match tier {
            0 => "Bronze",
            1 => "Silver",
            2 => "Gold",
            3 => "Platinum",
            _ => "Unknown",
        }
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&ReputationKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, ReputationError::Unauthorized));
        if &owner != caller {
            panic_with_error!(env, ReputationError::Unauthorized);
        }
    }

    fn get_review_or_panic(env: &Env, review_id: BytesN<32>) -> PeerReview {
        env.storage()
            .instance()
            .get(&ReputationKey::PeerReview(review_id))
            .unwrap_or_else(|| panic_with_error!(env, ReputationError::ReviewNotFound))
    }

    fn get_dispute_or_panic(env: &Env, dispute_id: BytesN<32>) -> DisputeRecord {
        env.storage()
            .instance()
            .get(&ReputationKey::DisputeRecord(dispute_id))
            .unwrap_or_else(|| panic_with_error!(env, ReputationError::DisputeNotFound))
    }

    fn get_incentive_or_panic(env: &Env, incentive_id: BytesN<32>) -> IncentiveRecord {
        env.storage()
            .instance()
            .get(&ReputationKey::IncentiveRecord(incentive_id))
            .unwrap_or_else(|| panic_with_error!(env, ReputationError::IncentiveNotFound))
    }

    fn get_next_review_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&ReputationKey::NextReviewId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&ReputationKey::NextReviewId, &(current + 1));
        current
    }

    fn get_next_dispute_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&ReputationKey::NextDisputeId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&ReputationKey::NextDisputeId, &(current + 1));
        current
    }

    fn get_next_incentive_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&ReputationKey::NextIncentiveId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&ReputationKey::NextIncentiveId, &(current + 1));
        current
    }

    fn sha2_digest(env: &Env, data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_initialization() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let submitter = Address::from_array(&env, &[2; 32]);

        ReputationSystem::initialize(env.clone(), owner.clone());

        let score = ReputationSystem::get_reputation(env.clone(), submitter);
        assert_eq!(score.score, 100);
        assert_eq!(score.tier, 0);
        assert_eq!(score.event_count, 0);
    }

    #[test]
    fn test_peer_review() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let reviewer = Address::from_array(&env, &[2; 32]);
        let subject = Address::from_array(&env, &[3; 32]);

        ReputationSystem::initialize(env.clone(), owner.clone());

        let review = ReputationSystem::submit_review(
            env.clone(),
            reviewer,
            subject.clone(),
            5,
            Bytes::new(&env),
            BytesN::from_array(&env, &[4; 32]),
        );
        assert_eq!(review.rating, 5);

        let score = ReputationSystem::get_reputation(env, subject);
        assert_eq!(score.positive_reviews, 1);
    }

    #[test]
    fn test_dispute_resolution() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let plaintiff = Address::from_array(&env, &[2; 32]);
        let defendant = Address::from_array(&env, &[3; 32]);

        ReputationSystem::initialize(env.clone(), owner.clone());

        let dispute = ReputationSystem::file_dispute(
            env.clone(),
            plaintiff,
            defendant.clone(),
            Bytes::new(&env),
        );
        assert_eq!(dispute.status, Symbol::new(&env, "open"));

        ReputationSystem::resolve_dispute(env.clone(), owner, dispute.id, false, Bytes::new(&env));

        let score = ReputationSystem::get_reputation(env, defendant);
        assert_eq!(score.disputes_lost, 1);
    }
}
