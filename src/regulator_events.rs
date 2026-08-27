//! Regulator-specific event types and compliance classification logic.
//!
//! Provides framework for emitting and classifying events according to ISA 3000 and SOC2 standards.

use soroban_sdk::{contracttype, Symbol, Bytes, Vec, Env};
use crate::regulator::{
    ComplianceStandard, SensitivityLevel, RegulatoryEventClass, ISA3000ControlObjective,
    SOC2Criterion,
};

/// Predefined ISA 3000 control objectives
/// Based on International Standard on Assurance Engagements 3000 (Revised)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ISA3000Objectives {
    // Not explicitly indexed - these are reference implementations
}

impl ISA3000Objectives {
    /// CC6.1: Segregation of Duties
    pub fn cc6_1() -> ISA3000ControlObjective {
        ISA3000ControlObjective {
            code: Symbol::new(&Env::default(), "CC6.1"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Segregation of duties to prevent unauthorized access and fraud")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "access_grant"));
                v.push_back(Symbol::new(&Env::default(), "role_assignment"));
                v
            },
            continuous_monitoring: true,
        }
    }

    /// CC6.2: System-generated exceptions
    pub fn cc6_2() -> ISA3000ControlObjective {
        ISA3000ControlObjective {
            code: Symbol::new(&Env::default(), "CC6.2"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"System generates exception reports and monitoring of approval authorities")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "exception_report"));
                v.push_back(Symbol::new(&Env::default(), "approval_log"));
                v
            },
            continuous_monitoring: true,
        }
    }

    /// CC7.1: Prevention and detection of unauthorized changes
    pub fn cc7_1() -> ISA3000ControlObjective {
        ISA3000ControlObjective {
            code: Symbol::new(&Env::default(), "CC7.1"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Prevention and detection of unauthorized changes to systems")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "change_log"));
                v.push_back(Symbol::new(&Env::default(), "audit_trail"));
                v
            },
            continuous_monitoring: true,
        }
    }

    /// CC9.1: Monitoring of system changes
    pub fn cc9_1() -> ISA3000ControlObjective {
        ISA3000ControlObjective {
            code: Symbol::new(&Env::default(), "CC9.1"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Monitoring and reconciliation of system activity")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "reconciliation"));
                v.push_back(Symbol::new(&Env::default(), "balance_verification"));
                v
            },
            continuous_monitoring: true,
        }
    }

    /// A1.1: General Authorization and Access Control
    pub fn a1_1() -> ISA3000ControlObjective {
        ISA3000ControlObjective {
            code: Symbol::new(&Env::default(), "A1.1"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Authorization and access control policies are established and enforced")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "policy_document"));
                v.push_back(Symbol::new(&Env::default(), "access_control_list"));
                v
            },
            continuous_monitoring: true,
        }
    }
}

/// Predefined SOC 2 criteria
/// Based on AICPA Trust Service Criteria
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SOC2Criteria {
    // Reference implementations for SOC 2 criteria
}

impl SOC2Criteria {
    /// CC6.1 - Logical and Physical Access Controls
    pub fn cc6_1() -> SOC2Criterion {
        SOC2Criterion {
            code: Symbol::new(&Env::default(), "CC6.1"),
            principle: Bytes::new(&Env::default()).try_extend_from_slice(b"Security")
                .expect("Principle bytes"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Logical and physical access controls protect system resources")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "access_control"));
                v.push_back(Symbol::new(&Env::default(), "authentication"));
                v
            },
        }
    }

    /// CC6.2 - Protective Measures Against Unauthorized Access
    pub fn cc6_2() -> SOC2Criterion {
        SOC2Criterion {
            code: Symbol::new(&Env::default(), "CC6.2"),
            principle: Bytes::new(&Env::default()).try_extend_from_slice(b"Security")
                .expect("Principle bytes"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Prior to issuing system credentials, identity and access rights are verified")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "identity_verification"));
                v.push_back(Symbol::new(&Env::default(), "credential_issuance"));
                v
            },
        }
    }

    /// CC7.1 - System Changes Are Managed
    pub fn cc7_1() -> SOC2Criterion {
        SOC2Criterion {
            code: Symbol::new(&Env::default(), "CC7.1"),
            principle: Bytes::new(&Env::default()).try_extend_from_slice(b"Security")
                .expect("Principle bytes"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"Changes to the objectives and responsibilities for IT and related processes are managed")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "change_request"));
                v.push_back(Symbol::new(&Env::default(), "change_approval"));
                v
            },
        }
    }

    /// A1.1 - Objectives for Availability
    pub fn a1_1() -> SOC2Criterion {
        SOC2Criterion {
            code: Symbol::new(&Env::default(), "A1.1"),
            principle: Bytes::new(&Env::default()).try_extend_from_slice(b"Availability")
                .expect("Principle bytes"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"The entity maintains and monitors commitments and responsibilities for availability")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "uptime_report"));
                v.push_back(Symbol::new(&Env::default(), "availability_metric"));
                v
            },
        }
    }

    /// PI1.1 - Objectives for Processing Integrity
    pub fn pi1_1() -> SOC2Criterion {
        SOC2Criterion {
            code: Symbol::new(&Env::default(), "PI1.1"),
            principle: Bytes::new(&Env::default()).try_extend_from_slice(b"Processing Integrity")
                .expect("Principle bytes"),
            description: Bytes::new(&Env::default()).try_extend_from_slice(b"The entity maintains and monitors commitments and responsibilities for processing integrity")
                .expect("Description bytes"),
            evidence_types: {
                let mut v = Vec::new(&Env::default());
                v.push_back(Symbol::new(&Env::default(), "transaction_log"));
                v.push_back(Symbol::new(&Env::default(), "reconciliation"));
                v
            },
        }
    }
}

/// Helper to create regulatory classification for an event
pub fn classify_event_isa3000(
    control_code: Symbol,
    demonstrates_control: bool,
    retention_ledgers: u32,
    sensitivity: SensitivityLevel,
) -> RegulatoryEventClass {
    RegulatoryEventClass {
        standard: ComplianceStandard::ISA3000,
        control_code,
        demonstrates_control,
        retention_ledgers,
        sensitivity,
    }
}

/// Helper to create regulatory classification for an event according to SOC2
pub fn classify_event_soc2(
    control_code: Symbol,
    demonstrates_control: bool,
    retention_ledgers: u32,
    sensitivity: SensitivityLevel,
) -> RegulatoryEventClass {
    RegulatoryEventClass {
        standard: ComplianceStandard::SOC2,
        control_code,
        demonstrates_control,
        retention_ledgers,
        sensitivity,
    }
}

/// Standard event types for compliance logging
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComplianceEventType {
    /// Access control decisions
    AccessControl,
    /// Authentication events
    Authentication,
    /// Authorization changes
    AuthorizationChange,
    /// Data modification events
    DataModification,
    /// Configuration changes
    ConfigurationChange,
    /// System exception
    SystemException,
    /// Approval workflow event
    ApprovalWorkflow,
    /// Compliance check
    ComplianceCheck,
    /// Audit trail event
    AuditTrail,
    /// User activity
    UserActivity,
}

impl ComplianceEventType {
    /// Convert to Symbol for use in contracts
    pub fn to_symbol(&self, env: &Env) -> Symbol {
        match self {
            ComplianceEventType::AccessControl => Symbol::new(env, "access_control"),
            ComplianceEventType::Authentication => Symbol::new(env, "authentication"),
            ComplianceEventType::AuthorizationChange => Symbol::new(env, "auth_change"),
            ComplianceEventType::DataModification => Symbol::new(env, "data_mod"),
            ComplianceEventType::ConfigurationChange => Symbol::new(env, "config_change"),
            ComplianceEventType::SystemException => Symbol::new(env, "exception"),
            ComplianceEventType::ApprovalWorkflow => Symbol::new(env, "approval"),
            ComplianceEventType::ComplianceCheck => Symbol::new(env, "compliance"),
            ComplianceEventType::AuditTrail => Symbol::new(env, "audit_trail"),
            ComplianceEventType::UserActivity => Symbol::new(env, "user_activity"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isa3000_cc6_1_has_evidence_types() {
        let objective = ISA3000Objectives::cc6_1();
        assert_eq!(objective.code.to_string(), "CC6.1");
        assert!(objective.continuous_monitoring);
        assert!(!objective.evidence_types.is_empty());
    }

    #[test]
    fn test_soc2_cc6_1_has_security_principle() {
        let criterion = SOC2Criteria::cc6_1();
        assert_eq!(criterion.code.to_string(), "CC6.1");
        assert_eq!(criterion.principle.to_string(), "Security");
    }

    #[test]
    fn test_classify_event_isa3000_creates_correct_standard() {
        let classification = classify_event_isa3000(
            Symbol::new(&Env::default(), "CC6.1"),
            true,
            52560, // 1 year in ledgers
            SensitivityLevel::Confidential,
        );
        assert_eq!(classification.standard, ComplianceStandard::ISA3000);
        assert!(classification.demonstrates_control);
    }
}
