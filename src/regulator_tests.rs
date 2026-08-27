#![cfg(test)]

//! Comprehensive tests for regulator-specific audit trail features
//! 
//! Test coverage:
//! - Selective disclosure proof generation and verification
//! - Tamper-evidence chain validation
//! - Data sharing agreement lifecycle
//! - Compliance rule validation (ISA 3000, SOC2)
//! - Regulator portal access control

#[cfg(test)]
mod regulator_tests {
    use soroban_sdk::{Env, Address, Symbol, Bytes, BytesN, Vec};

    #[test]
    fn test_regulator_role_hierarchy() {
        use crate::regulator::RegulatorRole;
        
        assert!(RegulatorRole::Auditor < RegulatorRole::RegulatorOfficer);
        assert!(RegulatorRole::RegulatorOfficer < RegulatorRole::RegulatoryAdmin);
    }

    #[test]
    fn test_sensitivity_level_ordering() {
        use crate::regulator::SensitivityLevel;
        
        assert!(SensitivityLevel::Public < SensitivityLevel::Internal);
        assert!(SensitivityLevel::Internal < SensitivityLevel::Confidential);
        assert!(SensitivityLevel::Confidential < SensitivityLevel::Restricted);
    }

    #[test]
    fn test_compliance_standard_comparison() {
        use crate::regulator::ComplianceStandard;
        
        assert_eq!(ComplianceStandard::ISA3000, ComplianceStandard::ISA3000);
        assert_ne!(ComplianceStandard::ISA3000, ComplianceStandard::SOC2);
    }
}

#[cfg(test)]
mod selective_disclosure_tests {
    use soroban_sdk::{Env, Symbol};
    use crate::disclosure::{ProofBuilder, DisclosureHelper};

    #[test]
    fn test_proof_builder_creation() {
        let env = Env::default();
        let builder = ProofBuilder::new(&env);
        assert_eq!(builder.all_fields.len(), 0);
        assert_eq!(builder.disclosed_fields.len(), 0);
    }

    #[test]
    fn test_proof_builder_add_field() {
        let env = Env::default();
        let hash = soroban_sdk::BytesN::from_array([1u8; 32]);
        
        let builder = ProofBuilder::new(&env)
            .add_field(Symbol::new(&env, "timestamp"), hash, true);
        
        assert_eq!(builder.all_fields.len(), 1);
        assert_eq!(builder.disclosed_fields.len(), 1);
    }

    #[test]
    fn test_proof_builder_calculate_root_empty() {
        let env = Env::default();
        let builder = ProofBuilder::new(&env);
        let root = builder.calculate_root();
        assert_eq!(root, soroban_sdk::BytesN::from_array([0u8; 32]));
    }

    #[test]
    fn test_disclosure_helper_verify_field() {
        use crate::disclosure::FieldDisclosureProof;
        
        let env = Env::default();
        let field_proof = FieldDisclosureProof {
            field_name: Symbol::new(&env, "timestamp"),
            field_value: Some(Bytes::new(&env)),
            field_hash: soroban_sdk::BytesN::from_array([1u8; 32]),
            sibling_hashes: Vec::new(&env),
            positions: Vec::new(&env),
        };

        let expected_root = soroban_sdk::BytesN::from_array([1u8; 32]);
        let result = DisclosureHelper::verify_field_inclusion(&env, &field_proof, &expected_root);
        assert!(result);
    }

    #[test]
    fn test_disclosure_proof_verification() {
        use crate::regulator::SelectiveDisclosureProof;
        
        let env = Env::default();
        let proof = SelectiveDisclosureProof {
            event_index: 0,
            disclosed_root: soroban_sdk::BytesN::from_array([1u8; 32]),
            complete_root: soroban_sdk::BytesN::from_array([2u8; 32]),
            disclosed_fields: {
                let mut v = Vec::new(&env);
                v.push_back(Symbol::new(&env, "timestamp"));
                v
            },
            merkle_proof: Vec::new(&env),
        };

        assert!(DisclosureHelper::verify_disclosure_proof(&proof));
    }
}

#[cfg(test)]
mod data_sharing_agreement_tests {
    use soroban_sdk::{Env, Address, Symbol};
    use crate::data_sharing::{DSABuilder, DSAHelper};
    use crate::regulator::{ComplianceStandard, RegulatorRole, SensitivityLevel};

    #[test]
    fn test_dsa_builder_basic_creation() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let builder = DSABuilder::new(&env, provider.clone(), regulator.clone());
        assert_eq!(builder.data_provider, provider);
        assert_eq!(builder.regulator_address, regulator);
    }

    #[test]
    fn test_dsa_builder_with_standards() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let builder = DSABuilder::new(&env, provider, regulator)
            .add_standard(ComplianceStandard::ISA3000)
            .add_standard(ComplianceStandard::SOC2);
        
        assert_eq!(builder.standards.len(), 2);
    }

    #[test]
    fn test_dsa_builder_with_role() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let builder = DSABuilder::new(&env, provider, regulator)
            .with_role(RegulatorRole::RegulatorOfficer);
        
        assert_eq!(builder.role, RegulatorRole::RegulatorOfficer);
    }

    #[test]
    fn test_dsa_is_active_within_window() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_effective_ledger(100)
            .with_expiry_ledger(200)
            .build();

        assert!(DSAHelper::is_dsa_active(&dsa, 150));
    }

    #[test]
    fn test_dsa_is_not_active_before_effective() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_effective_ledger(100)
            .build();

        assert!(!DSAHelper::is_dsa_active(&dsa, 50));
    }

    #[test]
    fn test_dsa_is_not_active_after_expiry() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_effective_ledger(10)
            .with_expiry_ledger(100)
            .build();

        assert!(!DSAHelper::is_dsa_active(&dsa, 150));
    }

    #[test]
    fn test_dsa_event_type_allowed_empty_list() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator).build();
        
        assert!(DSAHelper::is_event_type_allowed(&dsa, &Symbol::new(&env, "payment")));
    }

    #[test]
    fn test_dsa_sensitivity_allowed() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_min_sensitivity(SensitivityLevel::Internal)
            .build();
        
        // Confidential should be allowed (higher than Internal)
        assert!(DSAHelper::is_sensitivity_allowed(&dsa, &SensitivityLevel::Confidential));
        // Public should not be allowed (lower than Internal)
        assert!(!DSAHelper::is_sensitivity_allowed(&dsa, &SensitivityLevel::Public));
    }
}

#[cfg(test)]
mod tamper_evidence_tests {
    use soroban_sdk::{Env, Vec};
    use crate::tamper_evidence::{TamperEvidenceHelper, TamperEvidenceConfig, ImmutabilityProof};

    #[test]
    fn test_verify_event_hash_match() {
        let hash = soroban_sdk::BytesN::from_array([1u8; 32]);
        let expected = soroban_sdk::BytesN::from_array([1u8; 32]);
        assert!(TamperEvidenceHelper::verify_event_hash(&hash, &expected));
    }

    #[test]
    fn test_verify_event_hash_mismatch() {
        let hash = soroban_sdk::BytesN::from_array([1u8; 32]);
        let expected = soroban_sdk::BytesN::from_array([2u8; 32]);
        assert!(!TamperEvidenceHelper::verify_event_hash(&hash, &expected));
    }

    #[test]
    fn test_integrity_score_perfect() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 0);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_integrity_score_50_percent() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 50);
        assert_eq!(score, 50);
    }

    #[test]
    fn test_integrity_score_all_compromised() {
        let score = TamperEvidenceHelper::calculate_integrity_score(100, 100);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_immutability_threshold_met() {
        let env = Env::default();
        let config = TamperEvidenceConfig {
            immutability_threshold: 10,
            verify_all: true,
            hash_algorithm: 0,
            verify_archives: true,
        };

        let proof = TamperEvidenceHelper::verify_immutability(0, 100, &config);
        assert!(proof.immutable);
        assert_eq!(proof.chain_length, 100);
    }

    #[test]
    fn test_immutability_threshold_not_met() {
        let env = Env::default();
        let config = TamperEvidenceConfig {
            immutability_threshold: 50,
            verify_all: true,
            hash_algorithm: 0,
            verify_archives: true,
        };

        let proof = TamperEvidenceHelper::verify_immutability(95, 100, &config);
        assert!(!proof.immutable);
        assert_eq!(proof.chain_length, 5);
    }

    #[test]
    fn test_no_retroactive_modification() {
        let env = Env::default();
        let hash = soroban_sdk::BytesN::from_array([1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(soroban_sdk::BytesN::from_array([1u8; 32]));
        references.push_back(soroban_sdk::BytesN::from_array([1u8; 32]));

        assert!(TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }

    #[test]
    fn test_retroactive_modification_detected() {
        let env = Env::default();
        let hash = soroban_sdk::BytesN::from_array([1u8; 32]);
        let mut references = Vec::new(&env);
        references.push_back(soroban_sdk::BytesN::from_array([1u8; 32]));
        references.push_back(soroban_sdk::BytesN::from_array([2u8; 32])); // Mismatch!

        assert!(!TamperEvidenceHelper::verify_no_retroactive_modification(&hash, &references));
    }
}

#[cfg(test)]
mod compliance_validator_tests {
    use soroban_sdk::Env;
    use crate::compliance_validators::{ISA3000Validator, SOC2Validator};

    #[test]
    fn test_isa3000_get_control_objectives() {
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
    fn test_isa3000_compliance_score_none() {
        let env = Env::default();
        let score = ISA3000Validator::calculate_compliance_score(&env, 10, 0);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_soc2_get_security_criteria() {
        let env = Env::default();
        let criteria = SOC2Validator::get_security_criteria(&env);
        assert!(criteria.len() >= 3);
    }

    #[test]
    fn test_soc2_get_availability_criteria() {
        let env = Env::default();
        let criteria = SOC2Validator::get_availability_criteria(&env);
        assert!(criteria.len() >= 1);
    }

    #[test]
    fn test_soc2_get_processing_integrity_criteria() {
        let env = Env::default();
        let criteria = SOC2Validator::get_processing_integrity_criteria(&env);
        assert!(criteria.len() >= 1);
    }

    #[test]
    fn test_soc2_compliance_score() {
        let env = Env::default();
        let score = SOC2Validator::calculate_compliance_score(&env, 15, 12);
        assert_eq!(score, 80);
    }

    #[test]
    fn test_isa3000_generate_report() {
        let env = Env::default();
        let report = ISA3000Validator::generate_report(&env, 10, 9);
        assert_eq!(report.total_controls_tested, 10);
        assert_eq!(report.controls_operating, 9);
        assert_eq!(report.controls_with_deficiencies, 1);
        assert_eq!(report.compliance_score, 90);
    }

    #[test]
    fn test_soc2_generate_report() {
        let env = Env::default();
        let report = SOC2Validator::generate_report(&env, 10, 8);
        assert_eq!(report.total_controls_tested, 10);
        assert_eq!(report.controls_operating, 8);
        assert_eq!(report.controls_with_deficiencies, 2);
        assert_eq!(report.compliance_score, 80);
    }
}

#[cfg(test)]
mod regulator_event_tests {
    use soroban_sdk::{Env, Symbol};
    use crate::regulator_events::{ComplianceEventType, ISA3000Objectives, SOC2Criteria};

    #[test]
    fn test_compliance_event_type_to_symbol() {
        let env = Env::default();
        let symbol = ComplianceEventType::AccessControl.to_symbol(&env);
        assert_eq!(symbol.to_string(), "access_control");
    }

    #[test]
    fn test_all_compliance_event_types() {
        let env = Env::default();
        
        assert_eq!(ComplianceEventType::AccessControl.to_symbol(&env).to_string(), "access_control");
        assert_eq!(ComplianceEventType::Authentication.to_symbol(&env).to_string(), "authentication");
        assert_eq!(ComplianceEventType::AuthorizationChange.to_symbol(&env).to_string(), "auth_change");
        assert_eq!(ComplianceEventType::DataModification.to_symbol(&env).to_string(), "data_mod");
        assert_eq!(ComplianceEventType::ConfigurationChange.to_symbol(&env).to_string(), "config_change");
    }

    #[test]
    fn test_isa3000_cc6_1_objective() {
        let objective = ISA3000Objectives::cc6_1();
        assert_eq!(objective.code.to_string(), "CC6.1");
        assert!(objective.continuous_monitoring);
        assert!(objective.evidence_types.len() >= 2);
    }

    #[test]
    fn test_soc2_cc6_1_criterion() {
        let criterion = SOC2Criteria::cc6_1();
        assert_eq!(criterion.code.to_string(), "CC6.1");
        assert_eq!(criterion.principle.to_string(), "Security");
        assert!(criterion.evidence_types.len() >= 2);
    }
}

#[cfg(test)]
mod access_control_tests {
    use soroban_sdk::{Env, Address};
    use crate::data_sharing::{DSAHelper, AccessDecision};
    use crate::regulator::{RegulatorRole, ComplianceStandard, SensitivityLevel, AccessRequest, DataSharingAgreement};

    #[test]
    fn test_evaluate_access_denied_inactive_dsa() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DataSharingAgreement {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            data_provider: provider.clone(),
            regulator_address: regulator.clone(),
            effective_ledger: 100,
            expiry_ledger: 50, // Already expired
            standards: Vec::new(&env),
            allowed_event_types: Vec::new(&env),
            role: RegulatorRole::Auditor,
            min_sensitivity: SensitivityLevel::Public,
            active: false,
            signature_provider: soroban_sdk::BytesN::from_array([0u8; 64]),
            signature_regulator: soroban_sdk::BytesN::from_array([0u8; 64]),
        };

        let request = AccessRequest {
            id: soroban_sdk::BytesN::from_array([1u8; 32]),
            requester: regulator.clone(),
            data_owner: provider.clone(),
            standard: ComplianceStandard::ISA3000,
            event_types: Vec::new(&env),
            legal_basis: soroban_sdk::Bytes::new(&env),
            proposed_terms: dsa.clone(),
            status: 0,
            created_at: 0,
            resolved_at: 0,
        };

        let decision = DSAHelper::evaluate_access(&dsa, &request, 150);
        assert_eq!(decision, AccessDecision::Rejected);
    }

    #[test]
    fn test_evaluate_access_approved() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DataSharingAgreement {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            data_provider: provider.clone(),
            regulator_address: regulator.clone(),
            effective_ledger: 10,
            expiry_ledger: 0, // No expiry
            standards: {
                let mut v = Vec::new(&env);
                v.push_back(ComplianceStandard::ISA3000);
                v
            },
            allowed_event_types: Vec::new(&env),
            role: RegulatorRole::RegulatorOfficer,
            min_sensitivity: SensitivityLevel::Public,
            active: true,
            signature_provider: soroban_sdk::BytesN::from_array([1u8; 64]),
            signature_regulator: soroban_sdk::BytesN::from_array([1u8; 64]),
        };

        let request = AccessRequest {
            id: soroban_sdk::BytesN::from_array([1u8; 32]),
            requester: regulator.clone(),
            data_owner: provider.clone(),
            standard: ComplianceStandard::ISA3000,
            event_types: Vec::new(&env),
            legal_basis: soroban_sdk::Bytes::new(&env),
            proposed_terms: dsa.clone(),
            status: 0,
            created_at: 0,
            resolved_at: 0,
        };

        let decision = DSAHelper::evaluate_access(&dsa, &request, 150);
        assert_eq!(decision, AccessDecision::Approved);
    }
}
