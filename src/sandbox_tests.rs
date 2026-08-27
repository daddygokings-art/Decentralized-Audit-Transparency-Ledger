// Regulatory Sandbox Tests
#![cfg(test)]

use soroban_sdk::{Bytes, BytesN, Env};

use crate::sandbox_types::*;
use crate::sandbox_mgmt::*;
use crate::sandbox_env::*;
use crate::sandbox_supervision::*;
use crate::sandbox_innovation::*;
use crate::sandbox_graduation::*;

fn create_test_env() -> Env {
    Env::default()
}

// ============ Sandbox Types Tests ============

#[test]
fn test_sandbox_environment_levels() {
    assert_eq!(SandboxEnvironment::Level1PoC.max_transaction_amount(), 10_000_00);
    assert_eq!(SandboxEnvironment::Level2Beta.max_transaction_amount(), 100_000_00);
    assert_eq!(
        SandboxEnvironment::Level3Production.max_transaction_amount(),
        1_000_000_00
    );
}

#[test]
fn test_environment_daily_volumes() {
    assert_eq!(SandboxEnvironment::Level1PoC.max_daily_volume(), 100_000_00);
    assert_eq!(SandboxEnvironment::Level2Beta.max_daily_volume(), 1_000_000_00);
}

#[test]
fn test_application_status_states() {
    assert!(!ApplicationStatus::Submitted.is_terminal());
    assert!(ApplicationStatus::Approved.is_terminal());
    assert!(ApplicationStatus::Rejected.is_terminal());

    assert!(ApplicationStatus::Approved.is_active());
    assert!(!ApplicationStatus::Rejected.is_active());
}

#[test]
fn test_relaxed_requirements_levels() {
    let level1 = RelaxedRequirements::new_level1();
    assert!(level1.reduced_kyc_enabled);
    assert_eq!(level1.reduction_percentage, 75);

    let level2 = RelaxedRequirements::new_level2();
    assert!(level2.reduced_kyc_enabled);
    assert_eq!(level2.reduction_percentage, 40);

    let level3 = RelaxedRequirements::new_level3();
    assert!(!level3.reduced_kyc_enabled);
    assert_eq!(level3.reduction_percentage, 0);
}

#[test]
fn test_graduation_criteria_presets() {
    let default = GraduationCriteria::default();
    assert_eq!(default.min_transactions, 1000);
    assert_eq!(default.min_duration_days, 90);

    let aggressive = GraduationCriteria::aggressive();
    assert!(aggressive.min_transactions > default.min_transactions);

    let flexible = GraduationCriteria::flexible();
    assert!(flexible.min_transactions < default.min_transactions);
}

// ============ Sandbox Management Tests ============

#[test]
fn test_application_creation() {
    let env = create_test_env();
    let applicant = soroban_sdk::Address::generate(&env);
    let name = Bytes::from_slice(&env, b"TestCorp");
    let desc = Bytes::from_slice(&env, b"Testing new payments");
    let tech = Bytes::from_slice(&env, b"CBDC integration");

    let app = SandboxManager::create_application(
        &env,
        applicant,
        name,
        ParticipantType::Fintech,
        SandboxEnvironment::Level1PoC,
        desc,
        tech,
        90,
    )
    .unwrap();

    assert_eq!(app.status, ApplicationStatus::Submitted as u8);
    assert_eq!(app.expected_duration_days, 90);
}

#[test]
fn test_application_approval() {
    let env = create_test_env();
    let applicant = soroban_sdk::Address::generate(&env);
    let supervisor = soroban_sdk::Address::generate(&env);

    let mut app = SandboxManager::create_application(
        &env,
        applicant,
        Bytes::from_slice(&env, b"TestCorp"),
        ParticipantType::Bank,
        SandboxEnvironment::Level2Beta,
        Bytes::from_slice(&env, b"Testing"),
        Bytes::from_slice(&env, b"Tech"),
        60,
    )
    .unwrap();

    let participant = SandboxManager::approve_application(&env, &mut app, supervisor).unwrap();

    assert!(participant.is_active);
    assert_eq!(app.status, ApplicationStatus::Approved as u8);
}

#[test]
fn test_participant_extension() {
    let env = create_test_env();
    let mut participant = SandboxParticipant {
        participant_id: BytesN::zero(),
        name: Bytes::from_slice(&env, b"Corp"),
        address: soroban_sdk::Address::generate(&env),
        participant_type: ParticipantType::Fintech as u8,
        environment: SandboxEnvironment::Level1PoC as u8,
        entry_date: 1000,
        planned_exit_date: 10000,
        is_active: true,
        innovation_focus: Bytes::from_slice(&env, b"focus"),
        assigned_supervisor: soroban_sdk::Address::generate(&env),
    };

    let original = participant.planned_exit_date;
    SandboxManager::extend_participation(&mut participant, 30, 365).unwrap();
    assert!(participant.planned_exit_date > original);
}

#[test]
fn test_days_remaining() {
    let env = create_test_env();
    let participant = SandboxParticipant {
        participant_id: BytesN::zero(),
        name: Bytes::from_slice(&env, b"Corp"),
        address: soroban_sdk::Address::generate(&env),
        participant_type: ParticipantType::Fintech as u8,
        environment: SandboxEnvironment::Level1PoC as u8,
        entry_date: 1000,
        planned_exit_date: 1000 + (90 * 86400),
        is_active: true,
        innovation_focus: Bytes::new(&env),
        assigned_supervisor: soroban_sdk::Address::generate(&env),
    };

    let days = SandboxManager::get_days_remaining(&participant, 1000);
    assert_eq!(days, 90);
}

// ============ Sandbox Environment Tests ============

#[test]
fn test_sandbox_instance_creation() {
    let env = create_test_env();
    let sandbox = EnvironmentManager::create_sandbox_instance(
        &env,
        BytesN::zero(),
        SandboxEnvironment::Level1PoC,
    )
    .unwrap();

    assert!(sandbox.is_fully_isolated);
    assert_eq!(sandbox.transaction_count, 0);
    assert_eq!(sandbox.daily_volume_used, 0);
}

#[test]
fn test_transaction_limits() {
    let env = create_test_env();
    let mut sandbox =
        EnvironmentManager::create_sandbox_instance(&env, BytesN::zero(), SandboxEnvironment::Level1PoC)
            .unwrap();

    let result = EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 1_000_00).unwrap();
    assert_eq!(result, TransactionApprovalStatus::Approved);
    assert_eq!(sandbox.transaction_count, 1);

    let result =
        EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 200_000_00).unwrap();
    assert_eq!(result, TransactionApprovalStatus::LimitExceeded);
}

#[test]
fn test_daily_reset() {
    let env = create_test_env();
    let mut sandbox =
        EnvironmentManager::create_sandbox_instance(&env, BytesN::zero(), SandboxEnvironment::Level2Beta)
            .unwrap();

    EnvironmentManager::execute_sandbox_transaction(&mut sandbox, 50_000_00).unwrap();
    assert!(sandbox.daily_volume_used > 0);

    EnvironmentManager::reset_daily_limits(&mut sandbox);
    assert_eq!(sandbox.daily_volume_used, 0);
}

#[test]
fn test_abuse_detection() {
    let env = create_test_env();
    let sandbox = EnvironmentManager::create_sandbox_instance(
        &env,
        BytesN::zero(),
        SandboxEnvironment::Level1PoC,
    )
    .unwrap();

    assert!(!EnvironmentManager::detect_abuse(&sandbox, 100, 10));
    assert!(EnvironmentManager::detect_abuse(&sandbox, 100, 40));
}

// ============ Supervision Tests ============

#[test]
fn test_supervision_record() {
    let env = create_test_env();
    let supervisor = soroban_sdk::Address::generate(&env);

    let record = SupervisionManager::create_supervision_record(
        &env,
        BytesN::zero(),
        supervisor,
        Bytes::from_slice(&env, b"findings"),
        85,
        RiskLevel::Low,
    )
    .unwrap();

    assert_eq!(record.assessment_score, 85);
    assert!(!SupervisionManager::requires_regular_monitoring(&record));
}

#[test]
fn test_high_risk_monitoring() {
    let env = create_test_env();
    let supervisor = soroban_sdk::Address::generate(&env);

    let record = SupervisionManager::create_supervision_record(
        &env,
        BytesN::zero(),
        supervisor,
        Bytes::from_slice(&env, b"findings"),
        50,
        RiskLevel::High,
    )
    .unwrap();

    assert!(SupervisionManager::requires_regular_monitoring(&record));
}

// ============ Innovation Tests ============

#[test]
fn test_innovation_metrics() {
    let metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
    assert_eq!(metrics.impact_score, 0);
    assert!(!metrics.deployment_ready);
}

#[test]
fn test_overall_innovation_score() {
    let mut metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
    InnovationTracker::update_scores(&mut metrics, 80, 75, 85, 70).unwrap();

    let overall = metrics.overall_innovation_score();
    assert_eq!(overall, 77);
}

#[test]
fn test_mainnet_readiness() {
    let mut metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
    InnovationTracker::update_scores(&mut metrics, 85, 80, 90, 75).unwrap();
    InnovationTracker::set_deployment_ready(&mut metrics, true);

    assert!(metrics.is_ready_for_mainnet());
}

// ============ Graduation Tests ============

#[test]
fn test_graduation_assessment() {
    let env = create_test_env();
    let assessment = GraduationManager::create_assessment(&env, BytesN::zero());

    assert_eq!(assessment.transactions_completed, 0);
    assert!(!assessment.is_eligible);
}

#[test]
fn test_eligibility_evaluation() {
    let env = create_test_env();
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
fn test_graduation_approval() {
    let env = create_test_env();
    let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());
    assessment.is_eligible = true;

    let result = GraduationManager::approve_graduation(
        &mut assessment,
        Bytes::from_slice(&env, b"Ready"),
    );

    assert!(result.is_ok());
    assert_eq!(assessment.decision, GraduationDecision::Approved as u8);
}

#[test]
fn test_readiness_percentage() {
    let env = create_test_env();
    let mut assessment = GraduationManager::create_assessment(&env, BytesN::zero());

    assessment.compliance_score = 80;
    assessment.user_satisfaction = 75;
    assessment.tech_readiness_score = 90;

    let readiness = GraduationManager::compute_readiness_percentage(&assessment);
    assert_eq!(readiness, 81);
}

// ============ Integration Workflow Tests ============

#[test]
fn test_complete_sandbox_lifecycle() {
    let env = create_test_env();

    // 1. Submit application
    let applicant = soroban_sdk::Address::generate(&env);
    let supervisor = soroban_sdk::Address::generate(&env);

    let mut app = SandboxManager::create_application(
        &env,
        applicant.clone(),
        Bytes::from_slice(&env, b"InnovateCorp"),
        ParticipantType::Fintech,
        SandboxEnvironment::Level2Beta,
        Bytes::from_slice(&env, b"CBDC testing"),
        Bytes::from_slice(&env, b"Tech stack"),
        90,
    )
    .unwrap();

    // 2. Approve application
    let participant = SandboxManager::approve_application(&env, &mut app, supervisor.clone()).unwrap();
    assert!(participant.is_active);

    // 3. Create sandbox instance
    let sandbox = EnvironmentManager::create_sandbox_instance(
        &env,
        participant.participant_id.clone(),
        SandboxEnvironment::Level2Beta,
    )
    .unwrap();

    assert_eq!(sandbox.daily_volume_limit, 1_000_000_00);

    // 4. Execute transactions
    let mut sandbox_mut = sandbox;
    for _ in 0..100 {
        EnvironmentManager::execute_sandbox_transaction(&mut sandbox_mut, 5_000_00).ok();
    }

    assert_eq!(sandbox_mut.transaction_count, 100);

    // 5. Create graduation assessment
    let mut grad_assessment = GraduationManager::create_assessment(&env, participant.participant_id);

    grad_assessment.transactions_completed = sandbox_mut.transaction_count;
    grad_assessment.days_in_sandbox = 90;
    grad_assessment.compliance_score = 90;
    grad_assessment.user_satisfaction = 80;
    grad_assessment.tech_readiness_score = 85;
    grad_assessment.regulatory_approval = true;

    let criteria = GraduationCriteria::flexible();
    assert!(GraduationManager::evaluate_eligibility(&mut grad_assessment, &criteria));
}
