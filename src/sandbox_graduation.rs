#![no_std]

//! Regulatory sandbox graduation criteria evaluator.
//!
//! #891: adds a Soroban persistent-storage layer for graduation assessments
//! with automated TTL bump policies on write and read, a permissionless TTL
//! extension hook, and archival lease pinning to reduce rent footprint while
//! guaranteeing long-lived assessment records are never silently evicted.

use crate::sandbox_types::*;
use crate::rbac::{RbacManager, Role};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Symbol used to tag audit events emitted by this module.
pub const AUDIT_TOPIC: &str = "grad_audit";

/// Outcome tag for a successful mutation.
pub const AUDIT_OUTCOME_OK: &str = "ok";

/// Outcome tag for a rejected access attempt.
pub const AUDIT_OUTCOME_DENIED: &str = "denied";

/// Maximum number of storage rows scanned per paginated read to bound CPU.
const MAX_PAGE_SCAN: u64 = 200;

/// Graduation-specific error codes.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GraduationError {
    /// Caller is not authorized for the required RBAC role.
    Unauthorized = 1,
    /// Assessment not found.
    AssessmentNotFound = 2,
    /// Assessment already in a terminal decision state.
    AlreadyGraduated = 3,
    /// Participant does not meet graduation criteria.
    Ineligible = 4,
    /// Invalid input supplied to an entrypoint.
    InvalidInput = 5,
}

/// Persistent storage keys for graduation state.
#[contracttype]
pub enum GraduationStorageKey {
    /// Contract admin address.
    Admin,
    /// Stored assessment: assessment_id -> GraduationAssessment.
    Assessment(BytesN<32>),
    /// Ordered id index: seq -> assessment_id.
    AssessmentBySeq(u64),
    /// Reverse lookup: participant_id -> assessment_id.
    ParticipantAssessment(BytesN<32>),
    /// Monotonic sequence counter.
    NextSeq,
    /// Total number of stored assessments.
    TotalAssessments,
}

/// Opaque pagination cursor pointing at the last returned sequence.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentCursor {
    /// Sequence number of the last item on the previous page.
    pub seq: u64,
}

/// Multi-predicate filter for assessment read queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentFilter {
    /// Optional participant id to match exactly.
    pub participant_id: Option<BytesN<32>>,
    /// Optional decision value to match exactly.
    pub decision: Option<u32>,
    /// Optional eligibility flag to match exactly.
    pub is_eligible: Option<bool>,
}

/// Paginated result of a filtered assessment query.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssessmentPage {
    /// Matching assessment ids for this page.
    pub ids: Vec<BytesN<32>>,
    /// Cursor to resume the next page, or None when exhausted.
    pub next_cursor: Option<AssessmentCursor>,
    /// Number of storage rows examined to produce this page.
    pub scanned: u32,
    /// Total number of stored assessments.
    pub total: u32,
}

pub struct GraduationManager;

impl GraduationManager {
    // ========================================================================
    // RBAC + audit helpers
    // ========================================================================

    /// Enforce caller authentication, non-suspension and the minimum RBAC role.
    /// Maps RBAC failures into [`GraduationError::Unauthorized`].
    fn require_caller(
        env: &Env,
        caller: &Address,
        min_role: Role,
        operation: &str,
    ) -> Result<(), GraduationError> {
        RbacManager::require_active_role_min(env, caller, min_role)
            .map_err(|_| GraduationError::Unauthorized)
            .and_then(|_| {
                Self::emit_audit_event(
                    env,
                    caller,
                    operation,
                    &Self::empty_digest(env),
                    AUDIT_OUTCOME_OK,
                    0,
                );
                Ok(())
            })
    }

    /// Emit a denied audit event for a rejected RBAC attempt.
    fn emit_denied(env: &Env, caller: &Address, operation: &str, code: u32) {
        Self::emit_audit_event(
            env,
            caller,
            operation,
            &Self::empty_digest(env),
            AUDIT_OUTCOME_DENIED,
            code,
        );
    }

    /// Emit a success audit event binding the pre/post state transition.
    fn emit_success(
        env: &Env,
        caller: &Address,
        operation: &str,
        pre_state: &Bytes,
        post_state: &Bytes,
    ) {
        let digest = state_diff_digest(env, pre_state, post_state);
        Self::emit_audit_event(env, caller, operation, &digest, AUDIT_OUTCOME_OK, 0);
    }

    /// Emit a standardized audit event (diagnostic + contract event).
    fn emit_audit_event(
        env: &Env,
        caller: &Address,
        operation: &str,
        digest: &BytesN<32>,
        outcome: &str,
        error_code: u32,
    ) {
        let topic = Symbol::new(env, AUDIT_TOPIC);
        let op = Symbol::new(env, operation);
        let result = Symbol::new(env, outcome);

        env.events().publish(
            (topic.clone(), op.clone(), caller.clone()),
            (env.ledger().timestamp(), digest.clone(), result.clone(), error_code),
        );

        env.events().publish(
            (Symbol::new(env, "grad_audit_ok"), op, result),
            (caller.clone(), env.ledger().timestamp(), digest.clone()),
        );
    }

    /// Empty digest used when no meaningful pre/post state exists (e.g. denial).
    fn empty_digest(env: &Env) -> BytesN<32> {
        env.crypto().sha256(&Bytes::new(env)).to_bytes()
    }

    /// Serialize an assessment into bytes for audit state-diff binding.
    fn assessment_to_bytes(env: &Env, a: &GraduationAssessment) -> Bytes {
        let mut b = Bytes::new(env);
        b.append(&a.assessment_id);
        b.append(&a.participant_id);
        b.append(&a.assessed_at.to_le_bytes());
        b.append(&a.transactions_completed.to_le_bytes());
        b.append(&a.days_in_sandbox.to_le_bytes());
        b.append(&a.compliance_score.to_le_bytes());
        b.append(&a.user_satisfaction.to_le_bytes());
        b.append(&a.regulatory_approval.to_le_bytes());
        b.append(&a.financial_health_passed.to_le_bytes());
        b.append(&a.tech_readiness_score.to_le_bytes());
        b.append(&a.is_eligible.to_le_bytes());
        b.append(&a.decision.to_le_bytes());
        b.append(&a.notes);
        b
    }

    // ========================================================================
    // Storage-backed mutating entrypoints (RBAC-gated + audited)
    // ========================================================================

    /// Create and persist a graduation assessment for a participant.
    /// Requires caller to hold at least the Submitter role.
    pub fn create_assessment(
        env: &Env,
        caller: &Address,
        participant_id: BytesN<32>,
    ) -> Result<BytesN<32>, GraduationError> {
        Self::require_caller(env, caller, Role::Submitter, "create_assessment")?;

        if participant_id == BytesN::zero() {
            return Err(GraduationError::InvalidInput);
        }

        let assessment_id = Self::compute_assessment_id(env, &participant_id);

        if env
            .storage()
            .persistent()
            .has(&GraduationStorageKey::Assessment(assessment_id))
        {
            return Err(GraduationError::InvalidInput);
        }

        let mut assessment = GraduationAssessment {
            assessment_id,
            participant_id,
            assessed_at: env.ledger().timestamp(),
            transactions_completed: 0,
            days_in_sandbox: 0,
            compliance_score: 0,
            user_satisfaction: 0,
            regulatory_approval: false,
            financial_health_passed: false,
            tech_readiness_score: 0,
            is_eligible: false,
            decision: GraduationDecision::Pending as u32,
            notes: Bytes::new(env),
        };

        let pre = Bytes::new(env);
        let post = Self::assessment_to_bytes(env, &assessment);

        let mut seq: u64 = env
            .storage()
            .persistent()
            .get(&GraduationStorageKey::NextSeq)
            .unwrap_or(1);
        env.storage()
            .persistent()
            .set(&GraduationStorageKey::NextSeq, &(seq + 1));

        env.storage()
            .persistent()
            .set(&GraduationStorageKey::Assessment(assessment_id.clone()), &assessment);
        env.storage()
            .persistent()
            .set(&GraduationStorageKey::AssessmentBySeq(seq), &assessment_id);
        env.storage()
            .persistent()
            .set(
                &GraduationStorageKey::ParticipantAssessment(participant_id),
                &assessment_id,
            );

        let total: u32 = env
            .storage()
            .persistent()
            .get(&GraduationStorageKey::TotalAssessments)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&GraduationStorageKey::TotalAssessments, &(total + 1));

        Self::emit_success(env, caller, "create_assessment", &pre, &post);

        Ok(assessment_id)
    }

    /// Update mutable fields of a stored assessment.
    /// Requires caller to hold at least the Submitter role.
    pub fn update_assessment(
        env: &Env,
        caller: &Address,
        assessment_id: BytesN<32>,
        transactions_completed: Option<u32>,
        days_in_sandbox: Option<u32>,
        compliance_score: Option<u32>,
        user_satisfaction: Option<u32>,
        regulatory_approval: Option<bool>,
        financial_health_passed: Option<bool>,
        tech_readiness_score: Option<u32>,
        notes: Option<Bytes>,
    ) -> Result<(), GraduationError> {
        Self::require_caller(env, caller, Role::Submitter, "update_assessment")?;

        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let mut assessment: GraduationAssessment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(GraduationError::AssessmentNotFound)?;

        if GraduationDecision::from_u32(assessment.decision).is_terminal() {
            return Err(GraduationError::AlreadyGraduated);
        }

        let pre = Self::assessment_to_bytes(env, &assessment);

        if let Some(v) = transactions_completed {
            assessment.transactions_completed = v;
        }
        if let Some(v) = days_in_sandbox {
            assessment.days_in_sandbox = v;
        }
        if let Some(v) = compliance_score {
            assessment.compliance_score = v;
        }
        if let Some(v) = user_satisfaction {
            assessment.user_satisfaction = v;
        }
        if let Some(v) = regulatory_approval {
            assessment.regulatory_approval = v;
        }
        if let Some(v) = financial_health_passed {
            assessment.financial_health_passed = v;
        }
        if let Some(v) = tech_readiness_score {
            assessment.tech_readiness_score = v;
        }
        if let Some(v) = notes {
            assessment.notes = v;
        }

        let post = Self::assessment_to_bytes(env, &assessment);
        env.storage().persistent().set(&key, &assessment);

        Self::emit_success(env, caller, "update_assessment", &pre, &post);

        Ok(())
    }

    /// Approve graduation for an eligible participant.
    /// Requires caller to hold at least the Auditor role.
    pub fn approve_graduation(
        env: &Env,
        caller: &Address,
        assessment_id: BytesN<32>,
        notes: Bytes,
    ) -> Result<(), GraduationError> {
        Self::require_caller(env, caller, Role::Auditor, "approve_graduation")?;

        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let mut assessment: GraduationAssessment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(GraduationError::AssessmentNotFound)?;

        if GraduationDecision::from_u32(assessment.decision).is_terminal() {
            return Err(GraduationError::AlreadyGraduated);
        }
        if !assessment.is_eligible {
            return Err(GraduationError::Ineligible);
        }

        let pre = Self::assessment_to_bytes(env, &assessment);
        assessment.decision = GraduationDecision::Approved as u32;
        assessment.notes = notes;
        let post = Self::assessment_to_bytes(env, &assessment);

        env.storage().persistent().set(&key, &assessment);

        Self::emit_success(env, caller, "approve_graduation", &pre, &post);

        Ok(())
    }

    /// Reject graduation for a participant.
    /// Requires caller to hold at least the Auditor role.
    pub fn reject_graduation(
        env: &Env,
        caller: &Address,
        assessment_id: BytesN<32>,
        reason: Bytes,
    ) -> Result<(), GraduationError> {
        Self::require_caller(env, caller, Role::Auditor, "reject_graduation")?;

        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let mut assessment: GraduationAssessment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(GraduationError::AssessmentNotFound)?;

        if GraduationDecision::from_u32(assessment.decision).is_terminal() {
            return Err(GraduationError::AlreadyGraduated);
        }

        let pre = Self::assessment_to_bytes(env, &assessment);
        assessment.decision = GraduationDecision::Rejected as u32;
        assessment.notes = reason;
        let post = Self::assessment_to_bytes(env, &assessment);

        env.storage().persistent().set(&key, &assessment);

        Self::emit_success(env, caller, "reject_graduation", &pre, &post);

        Ok(())
    }

    /// Defer the graduation decision for a participant.
    /// Requires caller to hold at least the Auditor role.
    pub fn defer_graduation(
        env: &Env,
        caller: &Address,
        assessment_id: BytesN<32>,
        reason: Bytes,
    ) -> Result<(), GraduationError> {
        Self::require_caller(env, caller, Role::Auditor, "defer_graduation")?;

        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let mut assessment: GraduationAssessment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(GraduationError::AssessmentNotFound)?;

        if GraduationDecision::from_u32(assessment.decision).is_terminal() {
            return Err(GraduationError::AlreadyGraduated);
        }

        let pre = Self::assessment_to_bytes(env, &assessment);
        assessment.decision = GraduationDecision::Deferred as u32;
        assessment.notes = reason;
        let post = Self::assessment_to_bytes(env, &assessment);

        env.storage().persistent().set(&key, &assessment);

        Self::emit_success(env, caller, "defer_graduation", &pre, &post);

        Ok(())
    }

    // ========================================================================
    // Read queries (cursor-based pagination + filtered batch queries)
    // ========================================================================

    /// Retrieve a stored assessment by id.
    pub fn get_assessment(
        env: &Env,
        assessment_id: BytesN<32>,
    ) -> Option<GraduationAssessment> {
        env.storage()
            .persistent()
            .get(&GraduationStorageKey::Assessment(assessment_id))
    }

    /// Retrieve the assessment id associated with a participant, if any.
    pub fn get_assessment_by_participant(
        env: &Env,
        participant_id: BytesN<32>,
    ) -> Option<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&GraduationStorageKey::ParticipantAssessment(participant_id))
    }

    /// Cursor-based, multi-predicate filtered batch query of assessments.
    ///
    /// Scans the ordered `AssessmentBySeq` index starting after `cursor.seq`,
    /// applies every non-`None` predicate in `filter` (AND semantics), and
    /// returns up to `limit` matching assessment ids. `next_cursor` allows
    /// resuming the scan; it is `None` when the ordered index is exhausted.
    /// The scan is bounded by [`MAX_PAGE_SCAN`] rows to protect the Soroban
    /// CPU/memory budget on large datasets.
    pub fn list_assessments(
        env: &Env,
        cursor: Option<AssessmentCursor>,
        limit: u32,
        filter: AssessmentFilter,
    ) -> AssessmentPage {
        let mut ids = Vec::new(env);
        let mut scanned: u32 = 0;
        let mut last_seq: u64 = cursor.map(|c| c.seq).unwrap_or(0);
        let total: u32 = env
            .storage()
            .persistent()
            .get(&GraduationStorageKey::TotalAssessments)
            .unwrap_or(0);

        let mut seq = last_seq + 1;
        let mut remaining = limit as u64;
        let mut stop = false;

        while remaining > 0 && scanned < MAX_PAGE_SCAN && !stop {
            let key = GraduationStorageKey::AssessmentBySeq(seq);
            let id: Option<BytesN<32>> = env.storage().persistent().get(&key);
            scanned += 1;
            last_seq = seq;

            if let Some(id) = id {
                let assessment: Option<GraduationAssessment> =
                    env.storage().persistent().get(&GraduationStorageKey::Assessment(id.clone()));

                if let Some(a) = assessment {
                    let matches = filter.participant_id.map_or(true, |p| p == a.participant_id)
                        && filter.decision.map_or(true, |d| d == a.decision)
                        && filter.is_eligible.map_or(true, |e| e == a.is_eligible);

                    if matches {
                        ids.push_back(id);
                        remaining -= 1;
                    }
                }
            }

            if seq >= total as u64 {
                stop = true;
            } else {
                seq += 1;
            }
        }

        let next_cursor = if last_seq < total as u64 {
            Some(AssessmentCursor { seq: last_seq })
        } else {
            None
        };

        AssessmentPage {
            ids,
            next_cursor,
            scanned,
            total,
        }
    }

    // ========================================================================
    // Pure helpers (unchanged logic, no storage)
    // ========================================================================

    /// Compute assessment ID
    pub fn compute_assessment_id(env: &Env, participant_id: &BytesN<32>) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&participant_id.to_bytes());
        input.append(&Bytes::from_slice(env, b"GRADUATION"));

        env.crypto().sha256(&input)
    }

    /// Evaluate graduation eligibility
    pub fn evaluate_eligibility(
        assessment: &mut GraduationAssessment,
        criteria: &GraduationCriteria,
    ) -> bool {
        let tx_ok = assessment.transactions_completed >= criteria.min_transactions;
        let duration_ok = assessment.days_in_sandbox >= criteria.min_duration_days;
        let compliance_ok = assessment.compliance_score >= criteria.min_compliance_score;
        let satisfaction_ok = assessment.user_satisfaction >= criteria.min_user_satisfaction;
        let tech_ok = assessment.tech_readiness_score >= criteria.min_tech_readiness_score;

        let regulatory_ok = if criteria.regulatory_approval_required {
            assessment.regulatory_approval
        } else {
            true
        };

        let financial_ok = if criteria.financial_health_required {
            assessment.financial_health_passed
        } else {
            true
        };

        let eligible = tx_ok
            && duration_ok
            && compliance_ok
            && satisfaction_ok
            && tech_ok
            && regulatory_ok
            && financial_ok;

        assessment.is_eligible = eligible;
        Ok(eligible)
    }

    /// Get graduation readiness percentage
    pub fn compute_readiness_percentage(assessment: &GraduationAssessment) -> u32 {
        let scores = [
            assessment.compliance_score,
            assessment.user_satisfaction,
            assessment.tech_readiness_score,
        ];

        let avg: u64 = scores.iter().map(|s| *s as u64).sum::<u64>() / 3;
        avg.min(100) as u32
    }

    /// Generate graduation recommendation
    pub fn get_recommendation(assessment: &GraduationAssessment) -> &'static str {
        if assessment.is_eligible {
            "Ready for graduation"
        } else if assessment.compliance_score < 75 {
            "Improve compliance score"
        } else if assessment.days_in_sandbox < 60 {
            "Extend sandbox duration"
        } else if assessment.transactions_completed < 500 {
            "Increase transaction volume"
        } else {
            "Defer and reassess"
        }
    }

    // ── Storage persistence with TTL bump policies (#891) ─────────────────

    /// Persists an assessment and applies the write-path TTL bump.
    ///
    /// `extend_ttl` only pays rent when the lease is at or below
    /// [`GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS`], so churn-heavy evaluators
    /// amortize archival costs instead of re-paying on every write.
    pub fn save_assessment(env: &Env, assessment: &GraduationAssessment) {
        let key = GraduationStorageKey::Assessment(assessment.assessment_id.clone());
        env.storage().persistent().set(&key, assessment);
        env.storage().persistent().extend_ttl(
            &key,
            GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS,
            GRADUATION_TTL_EXTEND_LEDGERS,
        );
    }

    /// Loads an assessment and applies the read-path TTL bump so hot records
    /// stay live without unexpectedly entering archival.
    ///
    /// Returns `None` when the assessment has expired out of the live lease
    /// or was never persisted.
    pub fn load_assessment(env: &Env, assessment_id: &BytesN<32>) -> Option<GraduationAssessment> {
        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let result = env.storage().persistent().get::<_, GraduationAssessment>(&key);
        if result.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS,
                GRADUATION_TTL_EXTEND_LEDGERS,
            );
        }
        result
    }

    /// Creates and immediately persists an assessment, appending it to the
    /// ordered index used for cursor-based scans.
    pub fn open_assessment(env: &Env, participant_id: BytesN<32>) -> GraduationAssessment {
        let assessment = Self::create_assessment(env, participant_id);
        Self::save_assessment(env, &assessment);

        let ordinal: u32 = env
            .storage()
            .persistent()
            .get(&GraduationStorageKey::AssessmentCount)
            .unwrap_or(0);
        env.storage().persistent().set(
            &GraduationStorageKey::AssessmentIndex(ordinal),
            &assessment.assessment_id,
        );
        env.storage().persistent().extend_ttl(
            &GraduationStorageKey::AssessmentIndex(ordinal),
            GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS,
            GRADUATION_ARCHIVAL_TTL_EXTEND_LEDGERS,
        );
        env.storage()
            .persistent()
            .set(&GraduationStorageKey::AssessmentCount, &(ordinal + 1));

        assessment
    }

    /// Permissionless TTL extension for archival-critical assessment records.
    ///
    /// Anyone may keep an active assessment's persistent lease alive; safe
    /// because the call only ever *extends* TTL (and pays rent).
    pub fn extend_graduation_ttl(env: &Env, assessment_id: &BytesN<32>) {
        env.storage().persistent().extend_ttl(
            &GraduationStorageKey::Assessment(assessment_id.clone()),
            GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS,
            GRADUATION_TTL_EXTEND_LEDGERS,
        );
    }

    /// Archive an assessment: re-pins the current snapshot under the extended
    /// archival lease. Returns the archived snapshot (#891).
    pub fn archive_assessment(
        env: &Env,
        assessment_id: &BytesN<32>,
    ) -> Option<GraduationAssessment> {
        let key = GraduationStorageKey::Assessment(assessment_id.clone());
        let snapshot: Option<GraduationAssessment> =
            env.storage().persistent().get(&key);
        if let Some(ref assessment) = snapshot {
            env.storage().persistent().set(&key, assessment);
            env.storage().persistent().extend_ttl(
                &key,
                GRADUATION_TTL_BUMP_THRESHOLD_LEDGERS,
                GRADUATION_ARCHIVAL_TTL_EXTEND_LEDGERS,
            );
        }
        snapshot
    }

    /// Total persisted assessments (0 when none).
    pub fn total_assessments(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get(&GraduationStorageKey::AssessmentCount)
            .unwrap_or(0)
    }

    /// Permission helper for archival pinning: an assessment is only
    /// deletable once its decision is terminal (approved or rejected).
    pub fn can_archive(assessment: &GraduationAssessment) -> bool {
        let decision = if assessment.decision == GraduationDecision::Approved as u32 {
            GraduationDecision::Approved
        } else if assessment.decision == GraduationDecision::Rejected as u32 {
            GraduationDecision::Rejected
        } else if assessment.decision == GraduationDecision::Deferred as u32 {
            GraduationDecision::Deferred
        } else {
            GraduationDecision::Pending
        };
        decision.is_terminal()
    }
}

/// Convenience conversion for the raw u32 decision field.
impl GraduationDecision {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => GraduationDecision::Approved,
            2 => GraduationDecision::Rejected,
            3 => GraduationDecision::Deferred,
            _ => GraduationDecision::Pending,
        }
    }
}

/// Compute a tamper-evident state-diff digest over the pre/post state bytes.
fn state_diff_digest(env: &Env, pre_state: &Bytes, post_state: &Bytes) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.append(pre_state);
    data.append(post_state);
    env.crypto().sha256(&data).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup_env() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let auditor = Address::generate(&env);
        let submitter = Address::generate(&env);

        RbacManager::initialize_admin(&env, &admin);
        RbacManager::set_role(&env, &admin, &auditor, Role::Auditor).unwrap();
        RbacManager::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();

        (env, admin, auditor, submitter)
    }

    #[test]
    fn test_create_and_approve_assessment() {
        let (env, _admin, auditor, submitter) = setup_env();
        let participant_id = BytesN::from_array(&env, &[1u8; 32]);

        let id = GraduationManager::create_assessment(&env, &submitter, participant_id).unwrap();

        // Viewer/caller without sufficient role is rejected.
        let stranger = Address::generate(&env);
        let err = GraduationManager::create_assessment(&env, &stranger, BytesN::zero());
        assert_eq!(err, Err(GraduationError::Unauthorized));

        // Update fields.
        GraduationManager::update_assessment(
            &env,
            &submitter,
            id.clone(),
            Some(1500),
            Some(100),
            Some(90),
            Some(80),
            Some(true),
            Some(true),
            Some(85),
            None,
        )
        .unwrap();

        // Eligibility eval on the stored copy.
        let mut stored = GraduationManager::get_assessment(&env, id.clone()).unwrap();
        assert!(GraduationManager::evaluate_eligibility(
            &mut stored,
            &GraduationCriteria::flexible()
        ));

        // Auditor approves.
        GraduationManager::approve_graduation(
            &env,
            &auditor,
            id.clone(),
            Bytes::from_slice(&env, b"Ready"),
        )
        .unwrap();

        let approved = GraduationManager::get_assessment(&env, id).unwrap();
        assert_eq!(
            approved.decision,
            GraduationDecision::Approved as u32
        );

        // Re-approving a terminal decision fails.
        let err = GraduationManager::approve_graduation(
            &env,
            &auditor,
            id,
            Bytes::from_slice(&env, b"again"),
        );
        assert_eq!(err, Err(GraduationError::AlreadyGraduated));
    }

    #[test]
    fn test_reject_and_defer() {
        let (env, _admin, auditor, submitter) = setup_env();
        let participant_id = BytesN::from_array(&env, &[2u8; 32]);
        let id = GraduationManager::create_assessment(&env, &submitter, participant_id).unwrap();

        GraduationManager::reject_graduation(
            &env,
            &auditor,
            id.clone(),
            Bytes::from_slice(&env, b"not ready"),
        )
        .unwrap();

        let rejected = GraduationManager::get_assessment(&env, id).unwrap();
        assert_eq!(
            rejected.decision,
            GraduationDecision::Rejected as u32
        );

        // Defer on terminal decision fails.
        let err = GraduationManager::defer_graduation(
            &env,
            &auditor,
            id,
            Bytes::from_slice(&env, b"later"),
        );
        assert_eq!(err, Err(GraduationError::AlreadyGraduated));
    }

    #[test]
    fn test_audit_events_emitted() {
        let (env, _admin, auditor, submitter) = setup_env();
        let participant_id = BytesN::from_array(&env, &[3u8; 32]);

        GraduationManager::create_assessment(&env, &submitter, participant_id).unwrap();
        let before = env.events().all().len();

        // Denied attempt also emits an audit event.
        let stranger = Address::generate(&env);
        let _ = GraduationManager::create_assessment(&env, &stranger, BytesN::zero());
        let after = env.events().all().len();
        assert!(after > before);
    }

    #[test]
    fn test_cursor_pagination_and_filtering() {
        let (env, _admin, auditor, submitter) = setup_env();

        let mut ids = Vec::new(&env);
        for i in 0..5 {
            let pid = BytesN::from_array(&env, &[i as u8 + 10; 32]);
            let id = GraduationManager::create_assessment(&env, &submitter, pid).unwrap();
            ids.push_back(id);

            // Mark last two as eligible + approved.
            if i >= 3 {
                let mut a = GraduationManager::get_assessment(&env, id.clone()).unwrap();
                a.transactions_completed = 5000;
                a.is_eligible = true;
                a.decision = GraduationDecision::Approved as u32;
                let key = GraduationStorageKey::Assessment(id.clone());
                env.storage().persistent().set(&key, &a);
            }
        }

        // Page 1: limit 2, no filter.
        let page1 = GraduationManager::list_assessments(
            &env,
            None,
            2,
            AssessmentFilter {
                participant_id: None,
                decision: None,
                is_eligible: None,
            },
        );
        assert_eq!(page1.ids.len(), 2);
        assert!(page1.next_cursor.is_some());
        assert_eq!(page1.total, 5);

        // Page 2: resume from cursor.
        let cursor = page1.next_cursor.unwrap();
        let page2 = GraduationManager::list_assessments(&env, Some(cursor), 2, AssessmentFilter::default());
        assert_eq!(page2.ids.len(), 2);
        assert!(page2.next_cursor.is_some());

        // Filter: only eligible assessments.
        let filtered = GraduationManager::list_assessments(
            &env,
            None,
            10,
            AssessmentFilter {
                participant_id: None,
                decision: None,
                is_eligible: Some(true),
            },
        );
        assert_eq!(filtered.ids.len(), 2);
    }

    #[test]
    fn test_readiness_and_recommendation() {
        let env = Env::default();
        let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());
        assessment.compliance_score = 80;
        assessment.user_satisfaction = 75;
        assessment.tech_readiness_score = 90;

        assert_eq!(GraduationManager::compute_readiness_percentage(&assessment), 81);
    }

    #[test]
    fn test_not_found_errors() {
        let (env, _admin, auditor, _submitter) = setup_env();
        let missing = BytesN::from_array(&env, &[9u8; 32]);

        assert_eq!(
            GraduationManager::approve_graduation(
                &env,
                &auditor,
                missing,
                Bytes::from_slice(&env, b"x")
            ),
            Err(GraduationError::AssessmentNotFound)
        );
    }
}

// Default filter (match everything).
impl Default for AssessmentFilter {
    fn default() -> Self {
        AssessmentFilter {
            participant_id: None,
            decision: None,
            is_eligible: None,
        }
    }

    #[test]
    fn test_notes_length_rejected_when_too_long() {
        let env = soroban_sdk::Env::default();
        let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());
        assessment.is_eligible = true;

        let mut long_notes = soroban_sdk::Bytes::new(&env);
        for _ in 0..2048u32 {
            long_notes.push_back(b'a');
        }
        let result = GraduationManager::approve_graduation(&mut assessment, long_notes);
        assert_eq!(result, Err("notes exceed maximum permitted length"));
    }

    // ── #891: storage TTL policy coverage ───────────────────────────────────

    #[test]
    fn test_save_and_load_assessment_roundtrip() {
        let env = soroban_sdk::Env::default();
        let assessment = GraduationManager::open_assessment(&env, BytesN::zero());
        let id = assessment.assessment_id.clone();

        let loaded = GraduationManager::load_assessment(&env, &id);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().participant_id, BytesN::zero());
        assert_eq!(GraduationManager::total_assessments(&env), 1);
    }

    #[test]
    fn test_load_missing_assessment_is_none() {
        let env = soroban_sdk::Env::default();
        assert!(GraduationManager::load_assessment(&env, &BytesN::zero()).is_none());
        assert_eq!(GraduationManager::total_assessments(&env), 0);
    }

    #[test]
    fn test_ttl_hooks_do_not_panic() {
        let env = soroban_sdk::Env::default();
        let assessment = GraduationManager::open_assessment(&env, BytesN::zero());
        GraduationManager::extend_graduation_ttl(&env, &assessment.assessment_id);
        let archived = GraduationManager::archive_assessment(&env, &assessment.assessment_id);
        assert!(archived.is_some());
    }

    #[test]
    fn test_can_archive_uses_terminal_decision() {
        let env = soroban_sdk::Env::default();
        let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());
        assert!(!GraduationManager::can_archive(&assessment));
        assessment.decision = GraduationDecision::Approved as u32;
        assert!(GraduationManager::can_archive(&assessment));
    }
}
