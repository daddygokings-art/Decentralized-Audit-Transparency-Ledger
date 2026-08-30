#![no_std]

use crate::sandbox_types::*;
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Sandbox management system for application processing and participant lifecycle.
pub struct SandboxManager;

impl SandboxManager {
    /// Create new sandbox application
    pub fn create_application(
        env: &Env,
        applicant: Address,
        organization_name: Bytes,
        participant_type: ParticipantType,
        requested_environment: SandboxEnvironment,
        description: Bytes,
        technology_details: Bytes,
        expected_duration_days: u32,
    ) -> Result<SandboxApplication, &'static str> {
        if organization_name.is_empty() {
            return Err("Organization name cannot be empty");
        }

        if expected_duration_days == 0 || expected_duration_days > 730 {
            return Err("Duration must be between 1 and 730 days");
        }

        let application_id = Self::compute_application_id(env, &applicant, &organization_name);

        Ok(SandboxApplication {
            application_id,
            applicant,
            organization_name,
            participant_type: participant_type as u8,
            requested_environment: requested_environment as u8,
            status: ApplicationStatus::Submitted as u8,
            submitted_at: env.ledger().timestamp(),
            reviewed_at: None,
            description,
            technology_details,
            expected_duration_days,
        })
    }

    /// Compute application ID
    pub fn compute_application_id(
        env: &Env,
        applicant: &Address,
        organization_name: &Bytes,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, applicant.to_xdr().as_ref()));
        input.append(organization_name);
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Approve application and create participant
    pub fn approve_application(
        env: &Env,
        application: &mut SandboxApplication,
        assigned_supervisor: Address,
    ) -> Result<SandboxParticipant, &'static str> {
        if application.status != ApplicationStatus::Submitted as u8 {
            return Err("Application must be in submitted state");
        }

        let participant_id = Self::compute_participant_id(env, &application.applicant);
        let now = env.ledger().timestamp();
        let exit_date = now + ((application.expected_duration_days as u64) * 86400);

        application.status = ApplicationStatus::Approved as u8;
        application.reviewed_at = Some(now);

        Ok(SandboxParticipant {
            participant_id,
            name: application.organization_name.clone(),
            address: application.applicant.clone(),
            participant_type: application.participant_type,
            environment: application.requested_environment,
            entry_date: now,
            planned_exit_date: exit_date,
            is_active: true,
            innovation_focus: application.description.clone(),
            assigned_supervisor,
        })
    }

    /// Reject application
    pub fn reject_application(
        env: &Env,
        application: &mut SandboxApplication,
    ) -> Result<(), &'static str> {
        if application.status == ApplicationStatus::Approved as u8 {
            return Err("Cannot reject approved application");
        }

        application.status = ApplicationStatus::Rejected as u8;
        application.reviewed_at = Some(env.ledger().timestamp());

        Ok(())
    }

    /// Request additional information
    pub fn request_additional_info(
        env: &Env,
        application: &mut SandboxApplication,
    ) -> Result<(), &'static str> {
        if application.status != ApplicationStatus::Submitted as u8 {
            return Err("Application must be in submitted state");
        }

        application.status = ApplicationStatus::AdditionalInfoRequested as u8;
        Ok(())
    }

    /// Compute participant ID
    pub fn compute_participant_id(env: &Env, applicant: &Address) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, applicant.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, b"PARTICIPANT"));

        env.crypto().sha256(&input)
    }

    /// Extend participant duration
    pub fn extend_participation(
        participant: &mut SandboxParticipant,
        additional_days: u32,
        max_days: u32,
    ) -> Result<(), &'static str> {
        if additional_days == 0 {
            return Err("Extension must be at least 1 day");
        }

        let current_duration = (participant.planned_exit_date - participant.entry_date) / 86400;
        if current_duration as u32 + additional_days > max_days {
            return Err("Extension exceeds maximum sandbox duration");
        }

        participant.planned_exit_date += (additional_days as u64) * 86400;
        Ok(())
    }

    /// Early exit from sandbox
    pub fn exit_sandbox(
        env: &Env,
        participant: &mut SandboxParticipant,
    ) -> Result<(), &'static str> {
        if !participant.is_active {
            return Err("Participant is already inactive");
        }

        participant.is_active = false;
        participant.planned_exit_date = env.ledger().timestamp();

        Ok(())
    }

    /// Check if participant can continue in sandbox
    pub fn is_participant_active(
        participant: &SandboxParticipant,
        current_time: u64,
    ) -> bool {
        participant.is_active && current_time < participant.planned_exit_date
    }

    /// Get days remaining in sandbox
    pub fn get_days_remaining(
        participant: &SandboxParticipant,
        current_time: u64,
    ) -> u32 {
        if current_time >= participant.planned_exit_date {
            return 0;
        }

        ((participant.planned_exit_date - current_time) / 86400) as u32
    }

    /// Get relaxed requirements for environment
    pub fn get_relaxed_requirements(environment: SandboxEnvironment) -> RelaxedRequirements {
        match environment {
            SandboxEnvironment::Level1PoC => RelaxedRequirements::new_level1(),
            SandboxEnvironment::Level2Beta => RelaxedRequirements::new_level2(),
            SandboxEnvironment::Level3Production => RelaxedRequirements::new_level3(),
        }
    }
}

/// Application processing statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationStatistics {
    /// Total applications received
    pub total_applications: u32,
    /// Applications approved
    pub approved_applications: u32,
    /// Applications rejected
    pub rejected_applications: u32,
    /// Applications pending review
    pub pending_applications: u32,
    /// Average review time (seconds)
    pub avg_review_time: u64,
    /// Approval rate (0-100)
    pub approval_rate: u32,
}

impl ApplicationStatistics {
    pub fn new() -> Self {
        ApplicationStatistics {
            total_applications: 0,
            approved_applications: 0,
            rejected_applications: 0,
            pending_applications: 0,
            avg_review_time: 0,
            approval_rate: 0,
        }
    }

    pub fn record_application(&mut self) {
        self.total_applications += 1;
        self.pending_applications += 1;
    }

    pub fn record_approval(&mut self) {
        self.approved_applications += 1;
        self.pending_applications = self.pending_applications.saturating_sub(1);
    }

    pub fn compute_approval_rate(&mut self) {
        if self.total_applications == 0 {
            self.approval_rate = 0;
            return;
        }

        self.approval_rate =
            ((self.approved_applications as u64 * 100) / self.total_applications as u64) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_creation() {
        let env = soroban_sdk::Env::default();
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
        let env = soroban_sdk::Env::default();
        let applicant = soroban_sdk::Address::generate(&env);
        let supervisor = soroban_sdk::Address::generate(&env);
        let name = Bytes::from_slice(&env, b"TestCorp");
        let desc = Bytes::from_slice(&env, b"Testing");
        let tech = Bytes::from_slice(&env, b"Tech");

        let mut app = SandboxManager::create_application(
            &env,
            applicant,
            name,
            ParticipantType::Bank,
            SandboxEnvironment::Level2Beta,
            desc,
            tech,
            60,
        )
        .unwrap();

        let participant =
            SandboxManager::approve_application(&env, &mut app, supervisor).unwrap();

        assert!(participant.is_active);
        assert_eq!(app.status, ApplicationStatus::Approved as u8);
    }

    #[test]
    fn test_participant_extension() {
        let env = soroban_sdk::Env::default();
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

        let original_exit = participant.planned_exit_date;
        SandboxManager::extend_participation(&mut participant, 30, 365).unwrap();

        assert!(participant.planned_exit_date > original_exit);
    }

    #[test]
    fn test_days_remaining() {
        let env = soroban_sdk::Env::default();
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

        let days_later = SandboxManager::get_days_remaining(&participant, 1000 + (30 * 86400));
        assert_eq!(days_later, 60);
    }
}
