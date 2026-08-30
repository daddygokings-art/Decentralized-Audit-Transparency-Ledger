#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Symbol, Vec};

/// Sandbox environment types for regulatory testing.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SandboxEnvironment {
    /// Level 1: Minimal constraints, for proof-of-concept
    Level1PoC = 0,
    /// Level 2: Moderate constraints, for beta testing
    Level2Beta = 1,
    /// Level 3: Production-like constraints, for scale testing
    Level3Production = 2,
}

impl SandboxEnvironment {
    pub fn as_symbol(&self) -> Symbol {
        match self {
            SandboxEnvironment::Level1PoC => Symbol::new(&[b"LEVEL1"]),
            SandboxEnvironment::Level2Beta => Symbol::new(&[b"LEVEL2"]),
            SandboxEnvironment::Level3Production => Symbol::new(&[b"LEVEL3"]),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SandboxEnvironment::Level1PoC => "Proof-of-Concept Environment",
            SandboxEnvironment::Level2Beta => "Beta Testing Environment",
            SandboxEnvironment::Level3Production => "Production-Like Environment",
        }
    }

    pub fn max_transaction_amount(&self) -> u128 {
        match self {
            SandboxEnvironment::Level1PoC => 10_000_00, // 10k
            SandboxEnvironment::Level2Beta => 100_000_00, // 100k
            SandboxEnvironment::Level3Production => 1_000_000_00, // 1M
        }
    }

    pub fn max_daily_volume(&self) -> u128 {
        match self {
            SandboxEnvironment::Level1PoC => 100_000_00, // 100k
            SandboxEnvironment::Level2Beta => 1_000_000_00, // 1M
            SandboxEnvironment::Level3Production => 10_000_000_00, // 10M
        }
    }

    pub fn required_compliance_checks(&self) -> u32 {
        match self {
            SandboxEnvironment::Level1PoC => 3,
            SandboxEnvironment::Level2Beta => 5,
            SandboxEnvironment::Level3Production => 8,
        }
    }

    pub fn monitoring_frequency_seconds(&self) -> u64 {
        match self {
            SandboxEnvironment::Level1PoC => 300, // 5 minutes
            SandboxEnvironment::Level2Beta => 60, // 1 minute
            SandboxEnvironment::Level3Production => 10, // 10 seconds
        }
    }
}

/// Sandbox participant types.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ParticipantType {
    /// Fintech company testing new service
    Fintech = 0,
    /// Bank or financial institution
    Bank = 1,
    /// Payment service provider
    PaymentProvider = 2,
    /// Technology infrastructure provider
    TechProvider = 3,
    /// Crypto/blockchain company
    CryptoCompany = 4,
    /// Cooperative or credit union
    Cooperative = 5,
}

impl ParticipantType {
    pub fn as_symbol(&self) -> Symbol {
        match self {
            ParticipantType::Fintech => Symbol::new(&[b"FINTECH"]),
            ParticipantType::Bank => Symbol::new(&[b"BANK"]),
            ParticipantType::PaymentProvider => Symbol::new(&[b"PAYMENT"]),
            ParticipantType::TechProvider => Symbol::new(&[b"TECH"]),
            ParticipantType::CryptoCompany => Symbol::new(&[b"CRYPTO"]),
            ParticipantType::Cooperative => Symbol::new(&[b"COOP"]),
        }
    }
}

/// Sandbox application status.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ApplicationStatus {
    /// Submitted, awaiting review
    Submitted = 0,
    /// Under review by supervisors
    UnderReview = 1,
    /// Additional information requested
    AdditionalInfoRequested = 2,
    /// Approved to enter sandbox
    Approved = 3,
    /// Rejected
    Rejected = 4,
    /// Withdrawn by applicant
    Withdrawn = 5,
}

impl ApplicationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ApplicationStatus::Approved
                | ApplicationStatus::Rejected
                | ApplicationStatus::Withdrawn
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ApplicationStatus::Approved)
    }
}

/// Sandbox participant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxParticipant {
    /// Unique participant ID
    pub participant_id: BytesN<32>,
    /// Organization name
    pub name: Bytes,
    /// Organization address (contract or account)
    pub address: Address,
    /// Participant type
    pub participant_type: u8, // ParticipantType as u8
    /// Sandbox environment level
    pub environment: u8, // SandboxEnvironment as u8
    /// Entry timestamp
    pub entry_date: u64,
    /// Planned exit date
    pub planned_exit_date: u64,
    /// Is active in sandbox
    pub is_active: bool,
    /// Innovation focus area
    pub innovation_focus: Bytes,
    /// Assigned supervisor
    pub assigned_supervisor: Address,
}

/// Sandbox application.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxApplication {
    /// Application ID
    pub application_id: BytesN<32>,
    /// Applicant address
    pub applicant: Address,
    /// Organization name
    pub organization_name: Bytes,
    /// Participant type
    pub participant_type: u8, // ParticipantType as u8
    /// Requested environment level
    pub requested_environment: u8, // SandboxEnvironment as u8
    /// Application status
    pub status: u8, // ApplicationStatus as u8
    /// Submitted timestamp
    pub submitted_at: u64,
    /// Reviewed timestamp
    pub reviewed_at: Option<u64>,
    /// Application description/purpose
    pub description: Bytes,
    /// Technology/innovation details
    pub technology_details: Bytes,
    /// Expected duration (days)
    pub expected_duration_days: u32,
}

/// Relaxed requirements for sandbox participants.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelaxedRequirements {
    /// Enable reduced KYC requirements
    pub reduced_kyc_enabled: bool,
    /// Enable reduced AML checks
    pub reduced_aml_enabled: bool,
    /// Enable transaction amount exemptions
    pub transaction_limit_exemptions: bool,
    /// Enable partial compliance checks
    pub partial_compliance_enabled: bool,
    /// Enable testing without full reserves
    pub reserve_requirement_reduced: bool,
    /// Reduction percentage (0-100)
    pub reduction_percentage: u32,
    /// Waivers list (serialized)
    pub waivers: Bytes,
}

impl RelaxedRequirements {
    pub fn new_level1() -> Self {
        RelaxedRequirements {
            reduced_kyc_enabled: true,
            reduced_aml_enabled: true,
            transaction_limit_exemptions: true,
            partial_compliance_enabled: true,
            reserve_requirement_reduced: true,
            reduction_percentage: 75, // 75% relaxation
            waivers: Bytes::new(&soroban_sdk::Env::default()),
        }
    }

    pub fn new_level2() -> Self {
        RelaxedRequirements {
            reduced_kyc_enabled: true,
            reduced_aml_enabled: true,
            transaction_limit_exemptions: false,
            partial_compliance_enabled: true,
            reserve_requirement_reduced: true,
            reduction_percentage: 40, // 40% relaxation
            waivers: Bytes::new(&soroban_sdk::Env::default()),
        }
    }

    pub fn new_level3() -> Self {
        RelaxedRequirements {
            reduced_kyc_enabled: false,
            reduced_aml_enabled: false,
            transaction_limit_exemptions: false,
            partial_compliance_enabled: false,
            reserve_requirement_reduced: false,
            reduction_percentage: 0, // No relaxation
            waivers: Bytes::new(&soroban_sdk::Env::default()),
        }
    }
}

/// Sandbox graduation criteria.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraduationCriteria {
    /// Minimum transactions required
    pub min_transactions: u32,
    /// Minimum duration in sandbox (days)
    pub min_duration_days: u32,
    /// Minimum compliance score (0-100)
    pub min_compliance_score: u32,
    /// Minimum user satisfaction score (0-100)
    pub min_user_satisfaction: u32,
    /// All regulatory requirements met
    pub regulatory_approval_required: bool,
    /// Financial health assessment passed
    pub financial_health_assess_req: bool,
    /// Technology readiness score (0-100)
    pub min_tech_readiness_score: u32,
}

impl GraduationCriteria {
    pub fn default() -> Self {
        GraduationCriteria {
            min_transactions: 1000,
            min_duration_days: 90,
            min_compliance_score: 85,
            min_user_satisfaction: 75,
            regulatory_approval_required: true,
            financial_health_assessment_required: true,
            min_tech_readiness_score: 80,
        }
    }

    pub fn aggressive() -> Self {
        GraduationCriteria {
            min_transactions: 5000,
            min_duration_days: 180,
            min_compliance_score: 95,
            min_user_satisfaction: 85,
            regulatory_approval_required: true,
            financial_health_assessment_required: true,
            min_tech_readiness_score: 90,
        }
    }

    pub fn flexible() -> Self {
        GraduationCriteria {
            min_transactions: 500,
            min_duration_days: 30,
            min_compliance_score: 75,
            min_user_satisfaction: 60,
            regulatory_approval_required: true,
            financial_health_assessment_required: false,
            min_tech_readiness_score: 70,
        }
    }
}

/// Sandbox configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxConfig {
    /// Maximum number of participants
    pub max_participants: u32,
    /// Current participant count
    pub participant_count: u32,
    /// Sandbox is enabled
    pub is_enabled: bool,
    /// Maximum duration per participant (days)
    pub max_duration_days: u32,
    /// Minimum duration per participant (days)
    pub min_duration_days: u32,
    /// Emergency exit allowed (force graduation)
    pub emergency_exit_enabled: bool,
}

impl SandboxConfig {
    pub fn default() -> Self {
        SandboxConfig {
            max_participants: 100,
            participant_count: 0,
            is_enabled: true,
            max_duration_days: 365,
            min_duration_days: 30,
            emergency_exit_enabled: false,
        }
    }

    pub fn can_add_participant(&self) -> bool {
        self.is_enabled && self.participant_count < self.max_participants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_application_status_terminal() {
        assert!(!ApplicationStatus::Submitted.is_terminal());
        assert!(ApplicationStatus::Approved.is_terminal());
        assert!(ApplicationStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_relaxed_requirements_levels() {
        let level1 = RelaxedRequirements::new_level1();
        assert!(level1.reduced_kyc_enabled);
        assert_eq!(level1.reduction_percentage, 75);

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
        assert_eq!(aggressive.min_transactions, 5000);
        assert!(aggressive.min_transactions > default.min_transactions);
    }

    #[test]
    fn test_sandbox_config_capacity() {
        let config = SandboxConfig::default();
        assert!(config.can_add_participant());

        let mut full_config = config.clone();
        full_config.participant_count = full_config.max_participants;
        assert!(!full_config.can_add_participant());
    }
}
