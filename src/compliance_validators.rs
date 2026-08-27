//! ISA 3000 and SOC2 Compliance Standard Validators
//!
//! Implements validation rules for:
//! - ISA 3000 (International Standard on Assurance Engagements)
//! - SOC 2 (Service Organization Control Framework)
//! Provides audit rule sets, compliance checks, and report generation

use soroban_sdk::{contracttype, Symbol, Bytes, Vec, Env};
use crate::regulator::{ComplianceStandard, RegulatoryEventClass};

/// ISA 3000 validation rule
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ISA3000Rule {
    /// Control objective code (e.g., "CC6.1")
    pub control_code: Symbol,
    /// Description of the control
    pub description: Bytes,
    /// Required evidence event types
    pub required_evidence_types: Vec<Symbol>,
    /// Minimum number of evidence items needed
    pub min_evidence_count: u32,
    /// Is this control required or optional
    pub mandatory: bool,
}

/// SOC2 validation rule
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SOC2Rule {
    /// Criterion code (e.g., "CC6.1")
    pub criterion_code: Symbol,
    /// Trust service principle
    pub principle: Bytes, // "Security", "Availability", "Processing Integrity", "Confidentiality", "Privacy"
    /// Description of the criterion
    pub description: Bytes,
    /// Required evidence event types
    pub required_evidence_types: Vec<Symbol>,
    /// Minimum number of evidence items needed
    pub min_evidence_count: u32,
}

/// Validation result for a compliance control
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlValidationResult {
    /// Control code being validated
    pub control_code: Symbol,
    /// Is the control operating effectively
    pub operating_effectively: bool,
    /// Number of evidence items found
    pub evidence_count: u32,
    /// Whether sufficient evidence was collected
    pub sufficient_evidence: bool,
    /// Issues or gaps identified
    pub issues: Vec<Bytes>,
}

/// Compliance audit report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceAuditReport {
    /// Audit standard
    pub standard: ComplianceStandard,
    /// Date of audit
    pub audit_date: u64,
    /// Total controls tested
    pub total_controls_tested: u32,
    /// Controls operating effectively
    pub controls_operating: u32,
    /// Controls with deficiencies
    pub controls_with_deficiencies: u32,
    /// Overall compliance score (0-100)
    pub compliance_score: u32,
}

/// ISA 3000 validator
pub struct ISA3000Validator;

impl ISA3000Validator {
    /// Get all ISA 3000 control objectives
    pub fn get_control_objectives(env: &Env) -> Vec<ISA3000Rule> {
        let mut objectives = Vec::new(env);

        // CC6.1: Segregation of Duties
        objectives.push_back(ISA3000Rule {
            control_code: Symbol::new(env, "CC6.1"),
            description: Bytes::new(env).try_extend_from_slice(
                b"Segregation of duties to prevent unauthorized access and fraud",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "access_grant"));
                v.push_back(Symbol::new(env, "role_assignment"));
                v
            },
            min_evidence_count: 2,
            mandatory: true,
        });

        // CC6.2: Exception handling
        objectives.push_back(ISA3000Rule {
            control_code: Symbol::new(env, "CC6.2"),
            description: Bytes::new(env).try_extend_from_slice(
                b"System-generated exception reports and monitoring",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "exception_report"));
                v.push_back(Symbol::new(env, "approval_log"));
                v
            },
            min_evidence_count: 1,
            mandatory: true,
        });

        // CC7.1: Prevention of unauthorized changes
        objectives.push_back(ISA3000Rule {
            control_code: Symbol::new(env, "CC7.1"),
            description: Bytes::new(env).try_extend_from_slice(
                b"Prevention and detection of unauthorized changes to systems",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "change_log"));
                v.push_back(Symbol::new(env, "audit_trail"));
                v
            },
            min_evidence_count: 2,
            mandatory: true,
        });

        // CC9.1: Monitoring and reconciliation
        objectives.push_back(ISA3000Rule {
            control_code: Symbol::new(env, "CC9.1"),
            description: Bytes::new(env).try_extend_from_slice(
                b"Monitoring and reconciliation of system activity",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "reconciliation"));
                v.push_back(Symbol::new(env, "balance_verification"));
                v
            },
            min_evidence_count: 1,
            mandatory: true,
        });

        // A1.1: Authorization and access control
        objectives.push_back(ISA3000Rule {
            control_code: Symbol::new(env, "A1.1"),
            description: Bytes::new(env).try_extend_from_slice(
                b"Authorization and access control policies are established and enforced",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "policy_document"));
                v.push_back(Symbol::new(env, "access_control_list"));
                v
            },
            min_evidence_count: 1,
            mandatory: true,
        });

        objectives
    }

    /// Validate a control objective
    pub fn validate_control(
        env: &Env,
        control_code: &Symbol,
        evidence_events: &Vec<RegulatoryEventClass>,
        actual_evidence_count: u32,
    ) -> ControlValidationResult {
        let objectives = Self::get_control_objectives(env);
        
        let mut target_rule: Option<ISA3000Rule> = None;
        for obj in objectives.iter() {
            if &obj.control_code == control_code {
                target_rule = Some(obj);
                break;
            }
        }

        if target_rule.is_none() {
            let mut issues = Vec::new(env);
            issues.push_back(
                Bytes::new(env).try_extend_from_slice(b"Control code not found")
                    .unwrap()
            );
            return ControlValidationResult {
                control_code: *control_code,
                operating_effectively: false,
                evidence_count: 0,
                sufficient_evidence: false,
                issues,
            };
        }

        let rule = target_rule.unwrap();
        let sufficient = actual_evidence_count >= rule.min_evidence_count;

        ControlValidationResult {
            control_code: *control_code,
            operating_effectively: sufficient && !evidence_events.is_empty(),
            evidence_count: actual_evidence_count,
            sufficient_evidence: sufficient,
            issues: Vec::new(env),
        }
    }

    /// Calculate ISA 3000 compliance score
    pub fn calculate_compliance_score(
        env: &Env,
        controls_tested: u32,
        controls_operating: u32,
    ) -> u32 {
        if controls_tested == 0 {
            return 0;
        }
        ((controls_operating as u64 * 100) / (controls_tested as u64)) as u32
    }

    /// Generate ISA 3000 compliance report
    pub fn generate_report(
        env: &Env,
        controls_tested: u32,
        controls_operating: u32,
    ) -> ComplianceAuditReport {
        let compliance_score = Self::calculate_compliance_score(env, controls_tested, controls_operating);

        ComplianceAuditReport {
            standard: ComplianceStandard::ISA3000,
            audit_date: env.ledger().timestamp(),
            total_controls_tested: controls_tested,
            controls_operating,
            controls_with_deficiencies: controls_tested.saturating_sub(controls_operating),
            compliance_score,
        }
    }
}

/// SOC2 validator
pub struct SOC2Validator;

impl SOC2Validator {
    /// Get SOC2 criteria for Security principle
    pub fn get_security_criteria(env: &Env) -> Vec<SOC2Rule> {
        let mut criteria = Vec::new(env);

        // CC6.1 - Logical and Physical Access Controls
        criteria.push_back(SOC2Rule {
            criterion_code: Symbol::new(env, "CC6.1"),
            principle: Bytes::new(env).try_extend_from_slice(b"Security").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Logical and physical access controls protect system resources"
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "access_control"));
                v.push_back(Symbol::new(env, "authentication"));
                v
            },
            min_evidence_count: 2,
        });

        // CC6.2 - Identity Verification
        criteria.push_back(SOC2Rule {
            criterion_code: Symbol::new(env, "CC6.2"),
            principle: Bytes::new(env).try_extend_from_slice(b"Security").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Prior to issuing system credentials, identity and access rights are verified"
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "identity_verification"));
                v.push_back(Symbol::new(env, "credential_issuance"));
                v
            },
            min_evidence_count: 1,
        });

        // CC7.1 - Change Management
        criteria.push_back(SOC2Rule {
            criterion_code: Symbol::new(env, "CC7.1"),
            principle: Bytes::new(env).try_extend_from_slice(b"Security").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Changes to the objectives and responsibilities for IT and related processes are managed"
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "change_request"));
                v.push_back(Symbol::new(env, "change_approval"));
                v
            },
            min_evidence_count: 2,
        });

        criteria
    }

    /// Get SOC2 criteria for Availability principle
    pub fn get_availability_criteria(env: &Env) -> Vec<SOC2Rule> {
        let mut criteria = Vec::new(env);

        criteria.push_back(SOC2Rule {
            criterion_code: Symbol::new(env, "A1.1"),
            principle: Bytes::new(env).try_extend_from_slice(b"Availability").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"The entity maintains and monitors commitments and responsibilities for availability"
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "uptime_report"));
                v.push_back(Symbol::new(env, "availability_metric"));
                v
            },
            min_evidence_count: 1,
        });

        criteria
    }

    /// Get SOC2 criteria for Processing Integrity principle
    pub fn get_processing_integrity_criteria(env: &Env) -> Vec<SOC2Rule> {
        let mut criteria = Vec::new(env);

        criteria.push_back(SOC2Rule {
            criterion_code: Symbol::new(env, "PI1.1"),
            principle: Bytes::new(env).try_extend_from_slice(b"Processing Integrity").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"The entity maintains and monitors commitments and responsibilities for processing integrity"
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "transaction_log"));
                v.push_back(Symbol::new(env, "reconciliation"));
                v
            },
            min_evidence_count: 1,
        });

        criteria
    }

    /// Validate a SOC2 criterion
    pub fn validate_criterion(
        env: &Env,
        criterion_code: &Symbol,
        principle: &Bytes,
        actual_evidence_count: u32,
    ) -> ControlValidationResult {
        // Combine all criteria
        let mut all_criteria = Self::get_security_criteria(env);
        all_criteria.try_extend_from_slice(&Self::get_availability_criteria(env)).ok();
        all_criteria.try_extend_from_slice(&Self::get_processing_integrity_criteria(env)).ok();

        let mut target_rule: Option<SOC2Rule> = None;
        for criterion in all_criteria.iter() {
            if &criterion.criterion_code == criterion_code {
                target_rule = Some(criterion);
                break;
            }
        }

        if target_rule.is_none() {
            let mut issues = Vec::new(env);
            issues.push_back(
                Bytes::new(env).try_extend_from_slice(b"Criterion not found")
                    .unwrap()
            );
            return ControlValidationResult {
                control_code: *criterion_code,
                operating_effectively: false,
                evidence_count: 0,
                sufficient_evidence: false,
                issues,
            };
        }

        let rule = target_rule.unwrap();
        let sufficient = actual_evidence_count >= rule.min_evidence_count;

        ControlValidationResult {
            control_code: *criterion_code,
            operating_effectively: sufficient,
            evidence_count: actual_evidence_count,
            sufficient_evidence: sufficient,
            issues: Vec::new(env),
        }
    }

    /// Calculate SOC2 compliance score
    pub fn calculate_compliance_score(
        env: &Env,
        criteria_tested: u32,
        criteria_operating: u32,
    ) -> u32 {
        if criteria_tested == 0 {
            return 0;
        }
        ((criteria_operating as u64 * 100) / (criteria_tested as u64)) as u32
    }

    /// Generate SOC2 compliance report
    pub fn generate_report(
        env: &Env,
        criteria_tested: u32,
        criteria_operating: u32,
    ) -> ComplianceAuditReport {
        let compliance_score = Self::calculate_compliance_score(env, criteria_tested, criteria_operating);

        ComplianceAuditReport {
            standard: ComplianceStandard::SOC2,
            audit_date: env.ledger().timestamp(),
            total_controls_tested: criteria_tested,
            controls_operating: criteria_operating,
            controls_with_deficiencies: criteria_tested.saturating_sub(criteria_operating),
            compliance_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isa3000_get_objectives() {
        let env = Env::default();
        let objectives = ISA3000Validator::get_control_objectives(&env);
        assert!(objectives.len() >= 5);
    }

    #[test]
    fn test_isa3000_compliance_score_perfect() {
        let env = Env::default();
        let score = ISA3000Validator::calculate_compliance_score(&env, 10, 10);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_isa3000_compliance_score_partial() {
        let env = Env::default();
        let score = ISA3000Validator::calculate_compliance_score(&env, 10, 7);
        assert_eq!(score, 70);
    }

    #[test]
    fn test_soc2_security_criteria() {
        let env = Env::default();
        let criteria = SOC2Validator::get_security_criteria(&env);
        assert!(criteria.len() >= 3);
    }

    #[test]
    fn test_soc2_compliance_score() {
        let env = Env::default();
        let score = SOC2Validator::calculate_compliance_score(&env, 15, 12);
        assert_eq!(score, 80);
    }
}
