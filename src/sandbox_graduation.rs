#![no_std]

use crate::sandbox_types::*;
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Vec};

/// Graduation progress assessment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraduationAssessment {
    /// Assessment ID
    pub assessment_id: BytesN<32>,
    /// Participant ID
    pub participant_id: BytesN<32>,
    /// Assessment timestamp
    pub assessed_at: u64,
    /// Transactions completed (meets criteria if >= min)
    pub transactions_completed: u32,
    /// Days in sandbox (meets criteria if >= min)
    pub days_in_sandbox: u32,
    /// Compliance score (0-100, meets criteria if >= min)
    pub compliance_score: u32,
    /// User satisfaction (0-100, meets criteria if >= min)
    pub user_satisfaction: u32,
    /// Regulatory approval (true = approved)
    pub regulatory_approval: bool,
    /// Financial health assessment (true = passed)
    pub financial_health_passed: bool,
    /// Tech readiness score (0-100)
    pub tech_readiness_score: u32,
    /// Graduation eligible
    pub is_eligible: bool,
    /// Graduation decision (pending/approved/rejected)
    pub decision: u8, // GraduationDecision as u8
    /// Graduation notes
    pub notes: Bytes,
}

/// Graduation decision enumeration.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GraduationDecision {
    Pending = 0,
    Approved = 1,
    Rejected = 2,
    Deferred = 3,
}

impl GraduationDecision {
    pub fn is_terminal(&self) -> bool {
        matches!(self, GraduationDecision::Approved | GraduationDecision::Rejected)
    }
}

/// Graduation manager.
pub struct GraduationManager;

impl GraduationManager {
    /// Create graduation assessment
    pub fn create_assessment(
        env: &Env,
        participant_id: BytesN<32>,
    ) -> GraduationAssessment {
        let assessment_id = Self::compute_assessment_id(env, &participant_id);

        GraduationAssessment {
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
            decision: GraduationDecision::Pending as u8,
            notes: Bytes::new(env),
        }
    }

    /// Compute assessment ID
    pub fn compute_assessment_id(env: &Env, participant_id: &BytesN<32>) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, participant_id.as_ref()));
        input.append(&Bytes::from_slice(env, b"GRADUATION"));

        env.crypto().sha256(&input)
    }

    /// Evaluate graduation eligibility
    pub fn evaluate_eligibility(
        assessment: &mut GraduationAssessment,
        criteria: &GraduationCriteria,
    ) -> bool {
        // Check all criteria
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
        eligible
    }

    /// Approve graduation
    pub fn approve_graduation(
        assessment: &mut GraduationAssessment,
        notes: Bytes,
    ) -> Result<(), &'static str> {
        if !assessment.is_eligible {
            return Err("Participant does not meet graduation criteria");
        }

        assessment.decision = GraduationDecision::Approved as u8;
        assessment.notes = notes;

        Ok(())
    }

    /// Reject graduation
    pub fn reject_graduation(
        assessment: &mut GraduationAssessment,
        reason: Bytes,
    ) -> Result<(), &'static str> {
        assessment.decision = GraduationDecision::Rejected as u8;
        assessment.notes = reason;

        Ok(())
    }

    /// Defer graduation decision
    pub fn defer_graduation(
        assessment: &mut GraduationAssessment,
        reason: Bytes,
    ) -> Result<(), &'static str> {
        assessment.decision = GraduationDecision::Deferred as u8;
        assessment.notes = reason;

        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assessment_creation() {
        let env = soroban_sdk::Env::default();
        let assessment = GraduationManager::create_assessment(&env, BytesN::zero());

        assert_eq!(assessment.transactions_completed, 0);
        assert!(!assessment.is_eligible);
    }

    #[test]
    fn test_eligibility_evaluation() {
        let env = soroban_sdk::Env::default();
        let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());

        assessment.transactions_completed = 1500;
        assessment.days_in_sandbox = 100;
        assessment.compliance_score = 90;
        assessment.user_satisfaction = 80;
        assessment.tech_readiness_score = 85;
        assessment.regulatory_approval = true;

        let criteria = GraduationCriteria::default();
        let eligible = GraduationManager::evaluate_eligibility(&mut assessment, &criteria);

        assert!(eligible);
        assert!(assessment.is_eligible);
    }

    #[test]
    fn test_readiness_percentage() {
        let env = soroban_sdk::Env::default();
        let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());

        assessment.compliance_score = 80;
        assessment.user_satisfaction = 75;
        assessment.tech_readiness_score = 90;

        let readiness = GraduationManager::compute_readiness_percentage(&assessment);
        assert_eq!(readiness, 81); // Average of 80, 75, 90
    }
}
