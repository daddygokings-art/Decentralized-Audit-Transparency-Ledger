#[cfg(test)]
mod tests {
    use crate::digital_passport::*;
    use soroban_sdk::{vec, BytesN, Env, Symbol};

    /// Helper to create test material
    fn create_test_material(env: &Env, name: &str, code: &str, percent: u32) -> Material {
        Material {
            material_name: soroban_sdk::bytes!(env, name),
            material_code: Symbol::new(env, code),
            percentage_by_weight: percent,
            source_type: Symbol::new(env, "virgin"),
            hazardous: false,
            hazard_classification: soroban_sdk::bytes!(env, ""),
        }
    }

    /// Helper to create test durability
    fn create_test_durability(env: &Env) -> Durability {
        Durability {
            expected_lifetime_years: 5,
            warranty_years: 2,
            spare_parts_available: true,
            spare_parts_years: 10,
            repair_information: soroban_sdk::bytes!(env, "https://repair.example.com"),
            repairability_score: 8,
        }
    }

    /// Helper to create test circularity
    fn create_test_circularity(env: &Env) -> Circularity {
        let mut recyclable = vec![env];
        recyclable.push_back(create_test_material(env, "Aluminum", "AL", 100));

        Circularity {
            recyclable_materials: recyclable,
            recycled_content_percent: 30,
            reuse_potential: true,
            refurbishment_potential: true,
            disassembly_instructions: soroban_sdk::bytes!(env, "See manual page 5"),
            recycling_instructions: soroban_sdk::bytes!(env, "Place in aluminum bin"),
            end_of_life_score: 85,
        }
    }

    /// Helper to create test carbon footprint
    fn create_test_carbon_footprint(env: &Env) -> CarbonFootprint {
        CarbonFootprint {
            manufacturing_emissions: 500,
            distribution_emissions: 100,
            use_phase_emissions: 50,
            end_of_life_emissions: 25,
            total_embodied_carbon: 675,
            carbon_neutral: false,
            carbon_offset_program: soroban_sdk::bytes!(env, ""),
            measurement_standard: Symbol::new(env, "ISO_14040"),
            measurement_date: 1692345600,
        }
    }

    /// Test 1: Create a basic passport
    #[test]
    fn test_create_passport() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-001");
        let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

        let passport_id = create_passport(
            &env,
            product_id.clone(),
            soroban_sdk::bytes!(&env, b"Premium Widget"),
            Symbol::new(&env, "electronics"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-001"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        assert!(!passport_id.to_vec().is_empty());

        // Verify passport was created
        let passport = get_passport(&env, passport_id);
        assert_eq!(passport.version, 1);
    }

    /// Test 2: Update passport
    #[test]
    fn test_update_passport() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-002");
        let materials = vec![&env, create_test_material(&env, "Steel", "FE", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Steel Product"),
            Symbol::new(&env, "industrial"),
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-002"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        // Update with new carbon data
        let new_carbon = CarbonFootprint {
            manufacturing_emissions: 600,
            distribution_emissions: 120,
            use_phase_emissions: 60,
            end_of_life_emissions: 30,
            total_embodied_carbon: 810,
            carbon_neutral: false,
            carbon_offset_program: soroban_sdk::bytes!(&env, ""),
            measurement_standard: Symbol::new(&env, "ISO_14040"),
            measurement_date: env.ledger().timestamp(),
        };

        update_passport(
            &env,
            passport_id.clone(),
            None,
            Some(new_carbon),
            None,
            manufacturer,
        );

        let updated_passport = get_passport(&env, passport_id);
        assert_eq!(updated_passport.version, 2);
        assert_eq!(updated_passport.carbon_footprint.total_embodied_carbon, 810);
    }

    /// Test 3: Lifecycle stage transition
    #[test]
    fn test_lifecycle_transition() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-003");
        let materials = vec![&env, create_test_material(&env, "Plastic", "PL", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Plastic Item"),
            Symbol::new(&env, "consumer"),
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-003"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        // Transition to in production
        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::InProduction,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"Started manufacturing"),
        );

        let passport = get_passport(&env, passport_id.clone());
        assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::InProduction);

        // Transition to ready for market
        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::ReadyForMarket,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"Manufacturing complete"),
        );

        let passport = get_passport(&env, passport_id);
        assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::ReadyForMarket);
    }

    /// Test 4: Record repair event
    #[test]
    fn test_record_repair() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let repair_facility = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-004");
        let materials = vec![&env, create_test_material(&env, "Metal", "ME", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Repairable Product"),
            Symbol::new(&env, "durable"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-004"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let mut parts = vec![&env];
        parts.push_back(soroban_sdk::bytes!(&env, b"Motor"));
        parts.push_back(soroban_sdk::bytes!(&env, b"Bearings"));

        record_repair(
            &env,
            passport_id.clone(),
            repair_facility,
            Symbol::new(&env, "major"),
            parts,
            soroban_sdk::bytes!(&env, b"Replaced motor and bearings"),
        );

        let repairs = get_repair_history(&env, passport_id.clone());
        assert_eq!(repairs.len(), 1);
        assert_eq!(get_repair_count(&env, passport_id), 1);
    }

    /// Test 5: Record recycling event
    #[test]
    fn test_record_recycling() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let recycling_facility = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-005");
        let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Recyclable Item"),
            Symbol::new(&env, "materials"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-005"),
            materials.clone(),
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        record_recycling(
            &env,
            passport_id.clone(),
            recycling_facility,
            90,  // 90% recovery rate
            materials,
            soroban_sdk::bytes!(&env, b"CERT-RECYCLE-001"),
        );

        let recyclings = get_recycling_history(&env, passport_id.clone());
        assert_eq!(recyclings.len(), 1);
        assert_eq!(get_recycling_count(&env, passport_id.clone()), 1);

        // Should transition to recycled with >80% recovery
        let passport = get_passport(&env, passport_id);
        assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::Recycled);
    }

    /// Test 6: Record refurbishment event
    #[test]
    fn test_record_refurbishment() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let refurb_facility = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-006");
        let materials = vec![&env, create_test_material(&env, "Electronics", "EL", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Refurbish-able Product"),
            Symbol::new(&env, "electronics"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-006"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        record_refurbishment(
            &env,
            passport_id.clone(),
            refurb_facility,
            soroban_sdk::bytes!(&env, b"Complete hardware refresh and testing"),
        );

        let refurbs = get_refurbishment_history(&env, passport_id.clone());
        assert_eq!(refurbs.len(), 1);

        // Should transition to in-market
        let passport = get_passport(&env, passport_id);
        assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::InMarket);
    }

    /// Test 7: Verify ESPR compliance
    #[test]
    fn test_verify_espr_compliance() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let verifier = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-007");
        let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Compliant Product"),
            Symbol::new(&env, "industrial"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-007"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let status = verify_espr_compliance(&env, passport_id.clone(), verifier);
        assert_eq!(status, ComplianceStatus::Compliant);

        let records = env
            .storage()
            .persistent()
            .get::<_, Vec<ComplianceRecord>>(&PassportDataKey::ComplianceHistory(passport_id))
            .unwrap_or_else(|| vec![&env]);
        assert_eq!(records.len(), 1);
    }

    /// Test 8: Check passport validity
    #[test]
    fn test_check_passport_validity() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-008");
        let materials = vec![&env, create_test_material(&env, "Paper", "PA", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Valid Product"),
            Symbol::new(&env, "packaging"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-008"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let is_valid = check_passport_validity(&env, passport_id);
        assert!(is_valid);
    }

    /// Test 9: Get material breakdown
    #[test]
    fn test_get_material_breakdown() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-009");
        let mut materials = vec![&env];
        materials.push_back(create_test_material(&env, "Plastic", "PL", 60));
        materials.push_back(create_test_material(&env, "Metal", "ME", 40));

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Mixed Material Product"),
            Symbol::new(&env, "composite"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-009"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let breakdown = get_material_breakdown(&env, passport_id);
        assert_eq!(breakdown.len(), 2);
    }

    /// Test 10: Get carbon footprint
    #[test]
    fn test_get_carbon_footprint() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-010");
        let materials = vec![&env, create_test_material(&env, "Carbon", "C", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Carbon Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-010"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let carbon = get_carbon_footprint(&env, passport_id);
        assert_eq!(carbon.total_embodied_carbon, 675);
    }

    /// Test 11: Get circularity info
    #[test]
    fn test_get_circularity_info() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-011");
        let materials = vec![&env, create_test_material(&env, "Recycled", "RC", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Circular Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-011"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let circularity = get_circularity_info(&env, passport_id);
        assert_eq!(circularity.recycled_content_percent, 30);
        assert_eq!(circularity.end_of_life_score, 85);
    }

    /// Test 12: Export passport
    #[test]
    fn test_generate_passport_export() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-012");
        let materials = vec![&env, create_test_material(&env, "Export", "EX", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Export Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-012"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let export = generate_passport_export(&env, passport_id, ExportFormat::JsonLd);
        assert!(!export.data.is_empty());
        assert!(export.digital_signature.is_some());
    }

    /// Test 13: Import passport data
    #[test]
    fn test_import_passport_data() {
        let env = Env::default();
        env.mock_all_auths();

        let importer = soroban_sdk::Address::random(&env);
        let import_data = soroban_sdk::bytes!(&env, b"IMPORT_DATA_123");

        let imported_id = import_passport_data(&env, import_data, importer);
        assert!(!imported_id.to_vec().is_empty());
    }

    /// Test 14: Export to standard format
    #[test]
    fn test_export_to_standard_format() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-014");
        let materials = vec![&env, create_test_material(&env, "Standard", "ST", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Standard Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-014"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let exported = export_to_standard_format(&env, passport_id, Symbol::new(&env, "EU_XML"));
        assert!(!exported.is_empty());
    }

    /// Test 15: Validate interoperability
    #[test]
    fn test_validate_interoperability() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-015");
        let materials = vec![&env, create_test_material(&env, "Interop", "IO", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Interop Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-015"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let is_interop = validate_interoperability(&env, passport_id);
        assert!(is_interop);
    }

    /// Test 16: Calculate environmental score
    #[test]
    fn test_calculate_environmental_score() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-016");
        let materials = vec![&env, create_test_material(&env, "Green", "GR", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Green Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-016"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let score = calculate_environmental_score(&env, passport_id);
        assert!(score > 0);
        assert!(score <= 100);
    }

    /// Test 17: Check hazardous substances
    #[test]
    fn test_check_hazardous_substances() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-017");
        let materials = vec![&env, create_test_material(&env, "Lead", "PB", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Substance Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-017"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let hazards = check_hazardous_substances(&env, passport_id);
        // Should be empty for our test data
        assert_eq!(hazards.len(), 0);
    }

    /// Test 18: Get product passports
    #[test]
    fn test_get_product_passports() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-018");
        let materials = vec![&env, create_test_material(&env, "Multi", "MU", 100)];

        create_passport(
            &env,
            product_id.clone(),
            soroban_sdk::bytes!(&env, b"Multi Passport Product"),
            Symbol::new(&env, "test"),
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-018"),
            materials.clone(),
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let passports = get_product_passports(&env, product_id);
        assert_eq!(passports.len(), 1);
    }

    /// Test 19: Register material type
    #[test]
    fn test_register_material_type() {
        let env = Env::default();
        register_material_type(
            &env,
            Symbol::new(&env, "TITANIUM"),
            soroban_sdk::bytes!(&env, b"Titanium"),
        );
        // Test successful registration
    }

    /// Test 20: Get total passports
    #[test]
    fn test_get_total_passports() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);

        for i in 0..3 {
            let product_id = soroban_sdk::bytes!(&env, format!("PRODUCT-{:03}", i).as_bytes());
            let materials = vec![&env, create_test_material(&env, "Test", "TS", 100)];

            create_passport(
                &env,
                product_id,
                soroban_sdk::bytes!(&env, b"Test Product"),
                Symbol::new(&env, "test"),
                manufacturer.clone(),
                soroban_sdk::bytes!(&env, b"V1.0"),
                soroban_sdk::bytes!(&env, format!("BATCH-2024-{:03}", i).as_bytes()),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );
        }

        let total = get_total_passports(&env);
        assert_eq!(total, 3);
    }

    /// Test 21: Get lifecycle history
    #[test]
    fn test_get_lifecycle_history() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-021");
        let materials = vec![&env, create_test_material(&env, "History", "HI", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"History Product"),
            Symbol::new(&env, "test"),
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-021"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::InProduction,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"Started"),
        );

        let history = get_lifecycle_history(&env, passport_id);
        assert!(history.len() > 0);
    }

    /// Test 22: Multiple material composition
    #[test]
    fn test_multiple_material_composition() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-022");
        let mut materials = vec![&env];
        materials.push_back(create_test_material(&env, "Aluminum", "AL", 40));
        materials.push_back(create_test_material(&env, "Steel", "FE", 35));
        materials.push_back(create_test_material(&env, "Plastic", "PL", 25));

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Composite Product"),
            Symbol::new(&env, "composite"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-022"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let breakdown = get_material_breakdown(&env, passport_id);
        assert_eq!(breakdown.len(), 3);
    }

    /// Test 23: Compliance record verification
    #[test]
    fn test_compliance_record_verification() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let verifier = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-023");
        let materials = vec![&env, create_test_material(&env, "Compliant", "CM", 100)];

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Compliance Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-023"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        verify_espr_compliance(&env, passport_id.clone(), verifier.clone());
        verify_espr_compliance(&env, passport_id.clone(), verifier);

        // Check that compliance records were created
        let records = env
            .storage()
            .persistent()
            .get::<_, Vec<ComplianceRecord>>(&PassportDataKey::ComplianceHistory(passport_id))
            .unwrap_or_else(|| vec![&env]);
        assert_eq!(records.len(), 2);
    }

    /// Test 24: Full product lifecycle
    #[test]
    fn test_full_product_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let repair_facility = soroban_sdk::Address::random(&env);
        let recycling_facility = soroban_sdk::Address::random(&env);

        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-024");
        let materials = vec![&env, create_test_material(&env, "Lifecycle", "LC", 100)];

        // Create passport
        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"Lifecycle Product"),
            Symbol::new(&env, "lifecycle"),
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-024"),
            materials.clone(),
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        // Transition through lifecycle
        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::InProduction,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"In production"),
        );

        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::ReadyForMarket,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"Ready for market"),
        );

        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::InMarket,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"In market"),
        );

        // Record repair
        record_repair(
            &env,
            passport_id.clone(),
            repair_facility,
            Symbol::new(&env, "maintenance"),
            vec![&env],
            soroban_sdk::bytes!(&env, b"Routine maintenance"),
        );

        // Record end of life
        transition_lifecycle_stage(
            &env,
            passport_id.clone(),
            PassportLifecycleStage::EndOfLife,
            manufacturer.clone(),
            soroban_sdk::bytes!(&env, b"End of life reached"),
        );

        // Record recycling
        record_recycling(
            &env,
            passport_id.clone(),
            recycling_facility,
            95,
            materials,
            soroban_sdk::bytes!(&env, b"CERT-RECYCLE-024"),
        );

        let final_passport = get_passport(&env, passport_id);
        assert_eq!(final_passport.lifecycle_stage, PassportLifecycleStage::Recycled);
    }

    /// Test 25: Non-compliant material composition
    #[test]
    fn test_noncompliant_material_composition() {
        let env = Env::default();
        env.mock_all_auths();

        let manufacturer = soroban_sdk::Address::random(&env);
        let verifier = soroban_sdk::Address::random(&env);
        let product_id = soroban_sdk::bytes!(&env, b"PRODUCT-025");
        let mut materials = vec![&env];
        materials.push_back(create_test_material(&env, "Test", "TS", 60)); // Only 60% instead of 100%

        let passport_id = create_passport(
            &env,
            product_id,
            soroban_sdk::bytes!(&env, b"NonCompliant Product"),
            Symbol::new(&env, "test"),
            manufacturer,
            soroban_sdk::bytes!(&env, b"V1.0"),
            soroban_sdk::bytes!(&env, b"BATCH-2024-025"),
            materials,
            create_test_durability(&env),
            create_test_circularity(&env),
            create_test_carbon_footprint(&env),
        );

        let status = verify_espr_compliance(&env, passport_id, verifier);
        // Should be non-compliant due to material percentage issue
        assert_eq!(status, ComplianceStatus::NonCompliant);
    }
}
