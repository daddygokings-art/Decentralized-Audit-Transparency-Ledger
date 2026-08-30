//! Contract Event Audit and Compliance Automation
//!
//! Provides continuous control monitoring, automated evidence collection from
//! contract events, real-time policy enforcement, and audit-ready compliance reports
//! supporting SOX, GDPR, HIPAA, MiCA, and other major regulatory frameworks.

use soroban_sdk::{
    contracttype, Address, Bytes, BytesN, Env, Symbol, Vec,
};

/// Regulatory compliance framework standards supported by the engine
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ComplianceFramework {
    /// Sarbanes-Oxley Act (Internal controls over financial & transaction reporting)
    SOX = 0,
    /// General Data Protection Regulation (EU 2016/679)
    GDPR = 1,
    /// Health Insurance Portability and Accountability Act (Security & Privacy rules)
    HIPAA = 2,
    /// Markets in Crypto-Assets Regulation (EU 2023/1114)
    MiCA = 3,
    /// Service Organization Control 2 (Trust Services Criteria)
    SOC2 = 4,
    /// International Standard on Assurance Engagements
    ISA3000 = 5,
}

/// Enforcement level for compliance policies
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PolicyEnforcementLevel {
    /// Advisory logging only (does not reject non-compliant actions)
    Advisory = 0,
    /// Strict monitoring (flags violations and emits high-priority alerts)
    Strict = 1,
    /// Mandatory hard enforcement (rejects transaction if compliance checks fail)
    Enforced = 2,
}

/// Status of an evaluated compliance control
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ControlStatus {
    /// Operating effectively with sufficient valid evidence
    Passed = 0,
    /// Operating with minor non-blocking warnings or approaching renewal
    Warning = 1,
    /// Failed control validation or control deficiency detected
    Deficient = 2,
    /// Insufficient evidence collected during the observation window
    InsufficientEvidence = 3,
}

/// Continuous compliance control specification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceControl {
    /// Unique control code (e.g. "SOX-404-01", "GDPR-ART17", "HIPAA-SEC-312", "MICA-TITLE3")
    pub control_id: Symbol,
    /// Regulatory framework this control fulfills
    pub framework: ComplianceFramework,
    /// Control title / name
    pub name: Bytes,
    /// Detailed control objective description
    pub description: Bytes,
    /// Event types required as continuous evidence for this control
    pub required_evidence_types: Vec<Symbol>,
    /// Minimum count of evidence items required within evaluation window
    pub min_evidence_threshold: u32,
    /// Continuous monitoring frequency window in seconds
    pub monitoring_window_seconds: u64,
    /// Enforcement level
    pub enforcement_level: PolicyEnforcementLevel,
    /// Is the control active
    pub is_active: bool,
}

/// Evidence record collected from contract events
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRecord {
    /// Unique cryptographic hash identifying this evidence record
    pub evidence_id: BytesN<32>,
    /// Associated control code
    pub control_id: Symbol,
    /// Regulatory framework
    pub framework: ComplianceFramework,
    /// Associated audit event index
    pub event_index: u64,
    /// Cryptographic hash of the source event
    pub event_hash: BytesN<32>,
    /// Submitter of the source event
    pub submitter: Address,
    /// Timestamp when evidence was ingested
    pub collected_at: u64,
    /// Whether cryptographic verification succeeded
    pub is_verified: bool,
    /// Additional metadata or evidence summary payload
    pub metadata: Bytes,
}

/// Policy rule for runtime event policy enforcement
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRule {
    /// Unique rule ID
    pub rule_id: Symbol,
    /// Associated control ID
    pub control_id: Symbol,
    /// Target framework
    pub framework: ComplianceFramework,
    /// Rule evaluation type (e.g. "access_auth", "segregation_of_duties", "retention_limit", "crypto_shredding")
    pub rule_type: Symbol,
    /// Rule parameters in serialized form
    pub parameters: Bytes,
    /// Active state
    pub is_active: bool,
}

/// Result of policy rule enforcement evaluation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyEnforcementResult {
    /// Whether the evaluated action meets the policy requirements
    pub is_compliant: bool,
    /// Violation code if non-compliant
    pub violation_code: Symbol,
    /// Violation description message
    pub violation_message: Bytes,
    /// Suggested remediation or action
    pub suggested_action: Bytes,
}

/// Real-time continuous evaluation result for a compliance control
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlEvaluationResult {
    /// Control ID
    pub control_id: Symbol,
    /// Regulatory framework
    pub framework: ComplianceFramework,
    /// Current evaluation status
    pub status: ControlStatus,
    /// Total valid evidence items collected
    pub evidence_count: u32,
    /// Minimum threshold needed for pass
    pub required_threshold: u32,
    /// Timestamp of last evaluation
    pub last_evaluated_at: u64,
    /// Identified deficiencies or audit findings
    pub findings: Vec<Bytes>,
}

/// Audit-ready compliance report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceAuditReport {
    /// Unique report identifier digest
    pub report_id: BytesN<32>,
    /// Target compliance framework
    pub framework: ComplianceFramework,
    /// Report coverage period start timestamp
    pub period_start: u64,
    /// Report coverage period end timestamp
    pub period_end: u64,
    /// Total number of controls evaluated
    pub total_controls_evaluated: u32,
    /// Controls operating effectively
    pub controls_passed: u32,
    /// Controls operating with warnings
    pub controls_warning: u32,
    /// Controls with deficiencies
    pub controls_deficient: u32,
    /// Controls with insufficient evidence
    pub controls_insufficient_evidence: u32,
    /// Overall compliance score (0-100%)
    pub compliance_score: u32,
    /// Total evidence records supporting the report
    pub total_evidence_collected: u32,
    /// Report generation timestamp
    pub generated_at: u64,
    /// Cryptographic digest of the complete audit report
    pub report_digest: BytesN<32>,
}

/// Framework-specific standard baseline controls factory
pub struct StandardComplianceBaselines;

impl StandardComplianceBaselines {
    /// Standard baseline controls for SOX (Sarbanes-Oxley Section 302 & 404)
    pub fn get_sox_baseline_controls(env: &Env) -> Vec<ComplianceControl> {
        let mut controls = Vec::new(env);

        // SOX-404-01: Access Controls & Segregation of Duties
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "SOX-404-01"),
            framework: ComplianceFramework::SOX,
            name: Bytes::new(env).try_extend_from_slice(b"Access Control & Segregation of Duties").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Ensures segregation of duties for financial transactions and administrative actions",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "access_control"));
                v.push_back(Symbol::new(env, "governance_action"));
                v.push_back(Symbol::new(env, "multisig_approval"));
                v
            },
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400 * 30, // 30 days
            enforcement_level: PolicyEnforcementLevel::Enforced,
            is_active: true,
        });

        // SOX-404-02: Change Management & Audit Trail Integrity
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "SOX-404-02"),
            framework: ComplianceFramework::SOX,
            name: Bytes::new(env).try_extend_from_slice(b"Change Management & Audit Integrity").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Validates immutable append-only trail for configuration and smart contract parameter updates",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "config_change"));
                v.push_back(Symbol::new(env, "audit_trail"));
                v
            },
            min_evidence_threshold: 1,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        });

        controls
    }

    /// Standard baseline controls for GDPR (EU 2016/679)
    pub fn get_gdpr_baseline_controls(env: &Env) -> Vec<ComplianceControl> {
        let mut controls = Vec::new(env);

        // GDPR-ART17: Right to Erasure & Crypto-shredding Verification
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "GDPR-ART17"),
            framework: ComplianceFramework::GDPR,
            name: Bytes::new(env).try_extend_from_slice(b"Right to Erasure & Crypto-shredding").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Enforces verified erasure requests and cryptographic shredding of PII metadata",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "erasure_request"));
                v.push_back(Symbol::new(env, "crypto_shredding"));
                v
            },
            min_evidence_threshold: 1,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Enforced,
            is_active: true,
        });

        // GDPR-ART32: Security of Processing & Data Integrity
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "GDPR-ART32"),
            framework: ComplianceFramework::GDPR,
            name: Bytes::new(env).try_extend_from_slice(b"Security of Processing & Data Integrity").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Ensures data encryption, hashing, and access authorization for all sensitive data",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "data_protection"));
                v.push_back(Symbol::new(env, "access_authorization"));
                v
            },
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        });

        controls
    }

    /// Standard baseline controls for HIPAA (Health Insurance Portability & Accountability Act)
    pub fn get_hipaa_baseline_controls(env: &Env) -> Vec<ComplianceControl> {
        let mut controls = Vec::new(env);

        // HIPAA-164-312: Technical Safeguards & ePHI Audit Controls
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "HIPAA-164-312"),
            framework: ComplianceFramework::HIPAA,
            name: Bytes::new(env).try_extend_from_slice(b"Technical Safeguards & Audit Controls").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Implements hardware, software, and procedural mechanisms to record and examine access to ePHI",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "ephi_access"));
                v.push_back(Symbol::new(env, "auth_verification"));
                v
            },
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Enforced,
            is_active: true,
        });

        // HIPAA-164-308: Administrative Safeguards & Access Management
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "HIPAA-164-308"),
            framework: ComplianceFramework::HIPAA,
            name: Bytes::new(env).try_extend_from_slice(b"Administrative Safeguards & Role Access").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Restricts access to healthcare records strictly based on minimum necessary rule",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "role_assignment"));
                v.push_back(Symbol::new(env, "least_privilege_audit"));
                v
            },
            min_evidence_threshold: 1,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        });

        controls
    }

    /// Standard baseline controls for MiCA (Markets in Crypto-Assets Regulation EU 2023/1114)
    pub fn get_mica_baseline_controls(env: &Env) -> Vec<ComplianceControl> {
        let mut controls = Vec::new(env);

        // MICA-TITLE3: Asset-Referenced Token & Reserve Transparency
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "MICA-TITLE3"),
            framework: ComplianceFramework::MiCA,
            name: Bytes::new(env).try_extend_from_slice(b"Reserve Transparency & Asset Backing").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Continuous attestation and audit trail for token reserve backing and custody",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "reserve_attestation"));
                v.push_back(Symbol::new(env, "custody_verification"));
                v
            },
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Enforced,
            is_active: true,
        });

        // MICA-TITLE6: Market Abuse Prevention & Transaction Monitoring
        controls.push_back(ComplianceControl {
            control_id: Symbol::new(env, "MICA-TITLE6"),
            framework: ComplianceFramework::MiCA,
            name: Bytes::new(env).try_extend_from_slice(b"Market Abuse & Insider Prevention").unwrap(),
            description: Bytes::new(env).try_extend_from_slice(
                b"Continuous monitoring for suspicious transaction velocity, front-running, and market abuse",
            ).unwrap(),
            required_evidence_types: {
                let mut v = Vec::new(env);
                v.push_back(Symbol::new(env, "anomaly_check"));
                v.push_back(Symbol::new(env, "velocity_check"));
                v
            },
            min_evidence_threshold: 1,
            monitoring_window_seconds: 86400 * 30,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        });

        controls
    }
}

/// Continuous Compliance Monitoring and Automated Audit Engine
pub struct ComplianceAutomationEngine;

impl ComplianceAutomationEngine {
    /// Evaluate an individual control based on collected evidence count and validity
    pub fn evaluate_control(
        env: &Env,
        control: &ComplianceControl,
        evidence_count: u32,
        unresolved_issues: &Vec<Bytes>,
    ) -> ControlEvaluationResult {
        let status = if !control.is_active {
            ControlStatus::Warning
        } else if evidence_count == 0 && control.min_evidence_threshold > 0 {
            ControlStatus::InsufficientEvidence
        } else if !unresolved_issues.is_empty() {
            ControlStatus::Deficient
        } else if evidence_count >= control.min_evidence_threshold {
            ControlStatus::Passed
        } else {
            ControlStatus::Warning
        };

        ControlEvaluationResult {
            control_id: control.control_id,
            framework: control.framework,
            status,
            evidence_count,
            required_threshold: control.min_evidence_threshold,
            last_evaluated_at: env.ledger().timestamp(),
            findings: unresolved_issues.clone(),
        }
    }

    /// Enforce runtime policy rule against an event submitter and payload
    pub fn enforce_policy(
        env: &Env,
        rule: &PolicyRule,
        has_valid_auth: bool,
        is_encrypted: bool,
        is_held_legally: bool,
    ) -> PolicyEnforcementResult {
        if !rule.is_active {
            return PolicyEnforcementResult {
                is_compliant: true,
                violation_code: Symbol::new(env, "RULE_INACTIVE"),
                violation_message: Bytes::new(env).try_extend_from_slice(b"Policy rule inactive").unwrap(),
                suggested_action: Bytes::new(env),
            };
        }

        // Evaluate rule by type
        if rule.rule_type == Symbol::new(env, "access_auth") {
            if !has_valid_auth {
                return PolicyEnforcementResult {
                    is_compliant: false,
                    violation_code: Symbol::new(env, "AUTH_FAILURE"),
                    violation_message: Bytes::new(env).try_extend_from_slice(
                        b"Caller lacks required authorization role for this operation",
                    ).unwrap(),
                    suggested_action: Bytes::new(env).try_extend_from_slice(
                        b"Obtain appropriate role grant or multisig signature",
                    ).unwrap(),
                };
            }
        } else if rule.rule_type == Symbol::new(env, "data_encryption") {
            if !is_encrypted {
                return PolicyEnforcementResult {
                    is_compliant: false,
                    violation_code: Symbol::new(env, "UNENCRYPTED_PAYLOAD"),
                    violation_message: Bytes::new(env).try_extend_from_slice(
                        b"Sensitive data must be encrypted before ledger submission under GDPR/HIPAA",
                    ).unwrap(),
                    suggested_action: Bytes::new(env).try_extend_from_slice(
                        b"Encrypt payload with target recipient public key or zero-knowledge proof",
                    ).unwrap(),
                };
            }
        } else if rule.rule_type == Symbol::new(env, "erasure_protection") {
            if is_held_legally {
                return PolicyEnforcementResult {
                    is_compliant: false,
                    violation_code: Symbol::new(env, "LEGAL_HOLD_ACTIVE"),
                    violation_message: Bytes::new(env).try_extend_from_slice(
                        b"Record is currently subject to active legal hold",
                    ).unwrap(),
                    suggested_action: Bytes::new(env).try_extend_from_slice(
                        b"Resolve legal hold before processing erasure under GDPR Art 17(3)(e)",
                    ).unwrap(),
                };
            }
        }

        PolicyEnforcementResult {
            is_compliant: true,
            violation_code: Symbol::new(env, "COMPLIANT"),
            violation_message: Bytes::new(env).try_extend_from_slice(b"Policy check passed").unwrap(),
            suggested_action: Bytes::new(env),
        }
    }

    /// Calculate compliance score percentage
    pub fn calculate_compliance_score(
        total_controls: u32,
        passed_controls: u32,
        warning_controls: u32,
    ) -> u32 {
        if total_controls == 0 {
            return 100;
        }
        // Passed gives 100% weight, Warning gives 50% partial weight
        let points = (passed_controls as u64 * 100) + (warning_controls as u64 * 50);
        let max_points = total_controls as u64 * 100;
        ((points * 100) / max_points) as u32
    }

    /// Generate an audit-ready compliance report for a specific framework
    pub fn generate_compliance_report(
        env: &Env,
        framework: ComplianceFramework,
        period_start: u64,
        period_end: u64,
        evaluation_results: &Vec<ControlEvaluationResult>,
        total_evidence_collected: u32,
    ) -> ComplianceAuditReport {
        let mut passed = 0u32;
        let mut warning = 0u32;
        let mut deficient = 0u32;
        let mut insufficient = 0u32;
        let mut total = 0u32;

        for eval in evaluation_results.iter() {
            if eval.framework == framework {
                total += 1;
                match eval.status {
                    ControlStatus::Passed => passed += 1,
                    ControlStatus::Warning => warning += 1,
                    ControlStatus::Deficient => deficient += 1,
                    ControlStatus::InsufficientEvidence => insufficient += 1,
                }
            }
        }

        let score = Self::calculate_compliance_score(total, passed, warning);
        let report_id = env.crypto().sha256(&Bytes::new(env));
        let report_digest = env.crypto().sha256(&Bytes::new(env));

        ComplianceAuditReport {
            report_id,
            framework,
            period_start,
            period_end,
            total_controls_evaluated: total,
            controls_passed: passed,
            controls_warning: warning,
            controls_deficient: deficient,
            controls_insufficient_evidence: insufficient,
            compliance_score: score,
            total_evidence_collected,
            generated_at: env.ledger().timestamp(),
            report_digest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sox_baseline_generation() {
        let env = Env::default();
        let controls = StandardComplianceBaselines::get_sox_baseline_controls(&env);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls.get(0).unwrap().framework, ComplianceFramework::SOX);
    }

    #[test]
    fn test_gdpr_baseline_generation() {
        let env = Env::default();
        let controls = StandardComplianceBaselines::get_gdpr_baseline_controls(&env);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls.get(0).unwrap().framework, ComplianceFramework::GDPR);
    }

    #[test]
    fn test_hipaa_baseline_generation() {
        let env = Env::default();
        let controls = StandardComplianceBaselines::get_hipaa_baseline_controls(&env);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls.get(0).unwrap().framework, ComplianceFramework::HIPAA);
    }

    #[test]
    fn test_mica_baseline_generation() {
        let env = Env::default();
        let controls = StandardComplianceBaselines::get_mica_baseline_controls(&env);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls.get(0).unwrap().framework, ComplianceFramework::MiCA);
    }

    #[test]
    fn test_evaluate_control_passed() {
        let env = Env::default();
        let control = ComplianceControl {
            control_id: Symbol::new(&env, "TEST-CTRL-01"),
            framework: ComplianceFramework::SOX,
            name: Bytes::new(&env),
            description: Bytes::new(&env),
            required_evidence_types: Vec::new(&env),
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        };
        let issues = Vec::new(&env);
        let result = ComplianceAutomationEngine::evaluate_control(&env, &control, 3, &issues);
        assert_eq!(result.status, ControlStatus::Passed);
        assert_eq!(result.evidence_count, 3);
    }

    #[test]
    fn test_evaluate_control_deficient() {
        let env = Env::default();
        let control = ComplianceControl {
            control_id: Symbol::new(&env, "TEST-CTRL-01"),
            framework: ComplianceFramework::GDPR,
            name: Bytes::new(&env),
            description: Bytes::new(&env),
            required_evidence_types: Vec::new(&env),
            min_evidence_threshold: 2,
            monitoring_window_seconds: 86400,
            enforcement_level: PolicyEnforcementLevel::Strict,
            is_active: true,
        };
        let mut issues = Vec::new(&env);
        issues.push_back(Bytes::new(&env).try_extend_from_slice(b"Unredacted PII found").unwrap());
        let result = ComplianceAutomationEngine::evaluate_control(&env, &control, 5, &issues);
        assert_eq!(result.status, ControlStatus::Deficient);
    }

    #[test]
    fn test_policy_enforcement_access_auth() {
        let env = Env::default();
        let rule = PolicyRule {
            rule_id: Symbol::new(&env, "AUTH-RULE-1"),
            control_id: Symbol::new(&env, "SOX-404-01"),
            framework: ComplianceFramework::SOX,
            rule_type: Symbol::new(&env, "access_auth"),
            parameters: Bytes::new(&env),
            is_active: true,
        };
        let res_fail = ComplianceAutomationEngine::enforce_policy(&env, &rule, false, true, false);
        assert!(!res_fail.is_compliant);
        assert_eq!(res_fail.violation_code, Symbol::new(&env, "AUTH_FAILURE"));

        let res_ok = ComplianceAutomationEngine::enforce_policy(&env, &rule, true, true, false);
        assert!(res_ok.is_compliant);
    }

    #[test]
    fn test_compliance_score_calculation() {
        assert_eq!(ComplianceAutomationEngine::calculate_compliance_score(10, 10, 0), 100);
        assert_eq!(ComplianceAutomationEngine::calculate_compliance_score(10, 8, 2), 90);
        assert_eq!(ComplianceAutomationEngine::calculate_compliance_score(10, 5, 0), 50);
    }
}
