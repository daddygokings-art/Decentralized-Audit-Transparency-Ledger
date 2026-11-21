#[cfg(test)]
mod tests {
    use crate::digital_passport::*;
    use soroban_sdk::{vec, BytesN, Env, Symbol};

    /// Helper to create test material
    fn create_test_material(env: &Env, name: &str, code: &str, percent: u32) -> Material {
        Material {
            material_name: Bytes::from_slice(env, name.as_bytes()),
            material_code: Symbol::new(env, code),
            percentage_by_weight: percent,
            source_type: Symbol::new(env, "virgin"),
            hazardous: false,
            hazard_classification: Bytes::from_slice(env, b""),
        }
    }

    /// Helper to create test durability
    fn create_test_durability(env: &Env) -> Durability {
        Durability {
            expected_lifetime_years: 5,
            warranty_years: 2,
            spare_parts_available: true,
            spare_parts_years: 10,
            repair_information: Bytes::from_slice(env, b"https://repair.example.com"),
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
            disassembly_instructions: Bytes::from_slice(env, b"See manual page 5"),
            recycling_instructions: Bytes::from_slice(env, b"Place in aluminum bin"),
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
            carbon_offset_program: Bytes::from_slice(env, b""),
            measurement_standard: Symbol::new(env, "ISO_14040"),
            measurement_date: 1692345600,
        }
    }

    /// Test 1: Create a basic passport
    #[test]
    fn test_create_passport() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-001");
            let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

            let passport_id = create_passport(
                &env,
                product_id.clone(),
                Bytes::from_slice(&env, b"Premium Widget"),
                Symbol::new(&env, "electronics"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-001"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            assert!(!passport_id.to_vec().is_empty());

            // Verify passport was created
            let passport = get_passport(&env, passport_id);
            assert_eq!(passport.version, 1);
        });
    }

    /// Test 2: Update passport
    #[test]
    fn test_update_passport() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-002");
            let materials = vec![&env, create_test_material(&env, "Steel", "FE", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Steel Product"),
                Symbol::new(&env, "industrial"),
                manufacturer.clone(),
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-002"),
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
                carbon_offset_program: Bytes::from_slice(&env, b""),
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
        });
    }

    /// Test 3: Lifecycle stage transition
    #[test]
    fn test_lifecycle_transition() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-003");
            let materials = vec![&env, create_test_material(&env, "Plastic", "PL", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Plastic Item"),
                Symbol::new(&env, "consumer"),
                manufacturer.clone(),
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-003"),
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
                Bytes::from_slice(&env, b"Started manufacturing"),
            );

            let passport = get_passport(&env, passport_id.clone());
            assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::InProduction);

            // Transition to ready for market
            transition_lifecycle_stage(
                &env,
                passport_id.clone(),
                PassportLifecycleStage::ReadyForMarket,
                manufacturer.clone(),
                Bytes::from_slice(&env, b"Manufacturing complete"),
            );

            let passport = get_passport(&env, passport_id);
            assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::ReadyForMarket);
        });
    }

    /// Test 4: Record repair event
    #[test]
    fn test_record_repair() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let repair_facility = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-004");
            let materials = vec![&env, create_test_material(&env, "Metal", "ME", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Repairable Product"),
                Symbol::new(&env, "durable"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-004"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let mut parts = vec![&env];
            parts.push_back(Bytes::from_slice(&env, b"Motor"));
            parts.push_back(Bytes::from_slice(&env, b"Bearings"));

            record_repair(
                &env,
                passport_id.clone(),
                repair_facility,
                Symbol::new(&env, "major"),
                parts,
                Bytes::from_slice(&env, b"Replaced motor and bearings"),
            );

            let repairs = get_repair_history(&env, passport_id.clone());
            assert_eq!(repairs.len(), 1);
            assert_eq!(get_repair_count(&env, passport_id), 1);
        });
    }

    /// Test 5: Record recycling event
    #[test]
    fn test_record_recycling() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let recycling_facility = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-005");
            let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Recyclable Item"),
                Symbol::new(&env, "materials"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-005"),
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
                Bytes::from_slice(&env, b"CERT-RECYCLE-001"),
            );

            let recyclings = get_recycling_history(&env, passport_id.clone());
            assert_eq!(recyclings.len(), 1);
            assert_eq!(get_recycling_count(&env, passport_id.clone()), 1);

            // Should transition to recycled with >80% recovery
            let passport = get_passport(&env, passport_id);
            assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::Recycled);
        });
    }

    /// Test 6: Record refurbishment event
    #[test]
    fn test_record_refurbishment() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let refurb_facility = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-006");
            let materials = vec![&env, create_test_material(&env, "Electronics", "EL", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Refurbish-able Product"),
                Symbol::new(&env, "electronics"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-006"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            record_refurbishment(
                &env,
                passport_id.clone(),
                refurb_facility,
                Bytes::from_slice(&env, b"Complete hardware refresh and testing"),
            );

            let refurbs = get_refurbishment_history(&env, passport_id.clone());
            assert_eq!(refurbs.len(), 1);

            // Should transition to in-market
            let passport = get_passport(&env, passport_id);
            assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::InMarket);
        });
    }

    /// Test 7: Verify ESPR compliance
    #[test]
    fn test_verify_espr_compliance() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let verifier = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-007");
            let materials = vec![&env, create_test_material(&env, "Aluminum", "AL", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Compliant Product"),
                Symbol::new(&env, "industrial"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-007"),
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
        });
    }

    /// Test 8: Check passport validity
    #[test]
    fn test_check_passport_validity() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-008");
            let materials = vec![&env, create_test_material(&env, "Paper", "PA", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Valid Product"),
                Symbol::new(&env, "packaging"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-008"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let is_valid = check_passport_validity(&env, passport_id);
            assert!(is_valid);
        });
    }

    /// Test 9: Get material breakdown
    #[test]
    fn test_get_material_breakdown() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-009");
            let mut materials = vec![&env];
            materials.push_back(create_test_material(&env, "Plastic", "PL", 60));
            materials.push_back(create_test_material(&env, "Metal", "ME", 40));

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Mixed Material Product"),
                Symbol::new(&env, "composite"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-009"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let breakdown = get_material_breakdown(&env, passport_id);
            assert_eq!(breakdown.len(), 2);
        });
    }

    /// Test 10: Get carbon footprint
    #[test]
    fn test_get_carbon_footprint() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-010");
            let materials = vec![&env, create_test_material(&env, "Carbon", "C", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Carbon Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-010"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let carbon = get_carbon_footprint(&env, passport_id);
            assert_eq!(carbon.total_embodied_carbon, 675);
        });
    }

    /// Test 11: Get circularity info
    #[test]
    fn test_get_circularity_info() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-011");
            let materials = vec![&env, create_test_material(&env, "Recycled", "RC", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Circular Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-011"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let circularity = get_circularity_info(&env, passport_id);
            assert_eq!(circularity.recycled_content_percent, 30);
            assert_eq!(circularity.end_of_life_score, 85);
        });
    }

    /// Test 12: Export passport
    #[test]
    fn test_generate_passport_export() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-012");
            let materials = vec![&env, create_test_material(&env, "Export", "EX", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Export Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-012"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let export = generate_passport_export(&env, passport_id, ExportFormat::JsonLd);
            assert!(!export.data.is_empty());
            assert!(export.digital_signature.is_some());
        });
    }

    /// Test 13: Import passport data
    #[test]
    fn test_import_passport_data() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let importer = Address::generate(&env);
            let import_data = Bytes::from_slice(&env, b"IMPORT_DATA_123");

            let imported_id = import_passport_data(&env, import_data, importer);
            assert!(!imported_id.to_vec().is_empty());
        });
    }

    /// Test 14: Export to standard format
    #[test]
    fn test_export_to_standard_format() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-014");
            let materials = vec![&env, create_test_material(&env, "Standard", "ST", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Standard Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-014"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let exported = export_to_standard_format(&env, passport_id, Symbol::new(&env, "EU_XML"));
            assert!(!exported.is_empty());
        });
    }

    /// Test 15: Validate interoperability
    #[test]
    fn test_validate_interoperability() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-015");
            let materials = vec![&env, create_test_material(&env, "Interop", "IO", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Interop Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-015"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let is_interop = validate_interoperability(&env, passport_id);
            assert!(is_interop);
        });
    }

    /// Test 16: Calculate environmental score
    #[test]
    fn test_calculate_environmental_score() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-016");
            let materials = vec![&env, create_test_material(&env, "Green", "GR", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Green Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-016"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let score = calculate_environmental_score(&env, passport_id);
            assert!(score > 0);
            assert!(score <= 100);
        });
    }

    /// Test 17: Check hazardous substances
    #[test]
    fn test_check_hazardous_substances() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-017");
            let materials = vec![&env, create_test_material(&env, "Lead", "PB", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Substance Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-017"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let hazards = check_hazardous_substances(&env, passport_id);
            // Should be empty for our test data
            assert_eq!(hazards.len(), 0);
        });
    }

    /// Test 18: Get product passports
    #[test]
    fn test_get_product_passports() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-018");
            let materials = vec![&env, create_test_material(&env, "Multi", "MU", 100)];

            create_passport(
                &env,
                product_id.clone(),
                Bytes::from_slice(&env, b"Multi Passport Product"),
                Symbol::new(&env, "test"),
                manufacturer.clone(),
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-018"),
                materials.clone(),
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let passports = get_product_passports(&env, product_id);
            assert_eq!(passports.len(), 1);
        });
    }

    /// Test 19: Register material type
    #[test]
    fn test_register_material_type() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            register_material_type(
                &env,
                Symbol::new(&env, "TITANIUM"),
                Bytes::from_slice(&env, b"Titanium"),
            );
            // Test successful registration
        });
    }

    /// Test 20: Get total passports
    #[test]
    fn test_get_total_passports() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);

            for i in 0..3 {
                let product_id = soroban_sdk::bytes!(&env, format!("PRODUCT-{:03}", i).as_bytes());
                let materials = vec![&env, create_test_material(&env, "Test", "TS", 100)];

                create_passport(
                    &env,
                    product_id,
                    Bytes::from_slice(&env, b"Test Product"),
                    Symbol::new(&env, "test"),
                    manufacturer.clone(),
                    Bytes::from_slice(&env, b"V1.0"),
                    soroban_sdk::bytes!(&env, format!("BATCH-2024-{:03}", i).as_bytes()),
                    materials,
                    create_test_durability(&env),
                    create_test_circularity(&env),
                    create_test_carbon_footprint(&env),
                );
            }

            let total = get_total_passports(&env);
            assert_eq!(total, 3);
        });
    }

    /// Test 21: Get lifecycle history
    #[test]
    fn test_get_lifecycle_history() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-021");
            let materials = vec![&env, create_test_material(&env, "History", "HI", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"History Product"),
                Symbol::new(&env, "test"),
                manufacturer.clone(),
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-021"),
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
                Bytes::from_slice(&env, b"Started"),
            );

            let history = get_lifecycle_history(&env, passport_id);
            assert!(history.len() > 0);
        });
    }

    /// Test 22: Multiple material composition
    #[test]
    fn test_multiple_material_composition() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-022");
            let mut materials = vec![&env];
            materials.push_back(create_test_material(&env, "Aluminum", "AL", 40));
            materials.push_back(create_test_material(&env, "Steel", "FE", 35));
            materials.push_back(create_test_material(&env, "Plastic", "PL", 25));

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Composite Product"),
                Symbol::new(&env, "composite"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-022"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let breakdown = get_material_breakdown(&env, passport_id);
            assert_eq!(breakdown.len(), 3);
        });
    }

    /// Test 23: Compliance record verification
    #[test]
    fn test_compliance_record_verification() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let verifier = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-023");
            let materials = vec![&env, create_test_material(&env, "Compliant", "CM", 100)];

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Compliance Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-023"),
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
        });
    }

    /// Test 24: Full product lifecycle
    #[test]
    fn test_full_product_lifecycle() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let repair_facility = Address::generate(&env);
            let recycling_facility = Address::generate(&env);

            let product_id = Bytes::from_slice(&env, b"PRODUCT-024");
            let materials = vec![&env, create_test_material(&env, "Lifecycle", "LC", 100)];

            // Create passport
            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"Lifecycle Product"),
                Symbol::new(&env, "lifecycle"),
                manufacturer.clone(),
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-024"),
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
                Bytes::from_slice(&env, b"In production"),
            );

            transition_lifecycle_stage(
                &env,
                passport_id.clone(),
                PassportLifecycleStage::ReadyForMarket,
                manufacturer.clone(),
                Bytes::from_slice(&env, b"Ready for market"),
            );

            transition_lifecycle_stage(
                &env,
                passport_id.clone(),
                PassportLifecycleStage::InMarket,
                manufacturer.clone(),
                Bytes::from_slice(&env, b"In market"),
            );

            // Record repair
            record_repair(
                &env,
                passport_id.clone(),
                repair_facility,
                Symbol::new(&env, "maintenance"),
                vec![&env],
                Bytes::from_slice(&env, b"Routine maintenance"),
            );

            // Record end of life
            transition_lifecycle_stage(
                &env,
                passport_id.clone(),
                PassportLifecycleStage::EndOfLife,
                manufacturer.clone(),
                Bytes::from_slice(&env, b"End of life reached"),
            );

            // Record recycling
            record_recycling(
                &env,
                passport_id.clone(),
                recycling_facility,
                95,
                materials,
                Bytes::from_slice(&env, b"CERT-RECYCLE-024"),
            );

            let final_passport = get_passport(&env, passport_id);
            assert_eq!(final_passport.lifecycle_stage, PassportLifecycleStage::Recycled);
        });
    }

    /// Test 25: Non-compliant material composition
    #[test]
    fn test_noncompliant_material_composition() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();

            let manufacturer = Address::generate(&env);
            let verifier = Address::generate(&env);
            let product_id = Bytes::from_slice(&env, b"PRODUCT-025");
            let mut materials = vec![&env];
            materials.push_back(create_test_material(&env, "Test", "TS", 60)); // Only 60% instead of 100%

            let passport_id = create_passport(
                &env,
                product_id,
                Bytes::from_slice(&env, b"NonCompliant Product"),
                Symbol::new(&env, "test"),
                manufacturer,
                Bytes::from_slice(&env, b"V1.0"),
                Bytes::from_slice(&env, b"BATCH-2024-025"),
                materials,
                create_test_durability(&env),
                create_test_circularity(&env),
                create_test_carbon_footprint(&env),
            );

            let status = verify_espr_compliance(&env, passport_id, verifier);
            // Should be non-compliant due to material percentage issue
            assert_eq!(status, ComplianceStatus::NonCompliant);
        });
    }
}

/// #885/#886/#887 regression coverage: RBAC role gating, tamper-evident audit
/// trail chaining, TTL helpers, and cursor-based pagination.
#[cfg(test)]
mod gating_audit_ttl_pagination_tests {
    use crate::digital_passport::*;
    use crate::rbac::{RbacManager, Role};
    use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Symbol};

    fn test_material(env: &Env) -> Material {
        Material {
            material_name: Bytes::from_slice(env, b"Aluminum"),
            material_code: Symbol::new(env, "AL"),
            percentage_by_weight: 100,
            source_type: Symbol::new(env, "virgin"),
            hazardous: false,
            hazard_classification: Bytes::from_slice(env, b""),
        }
    }

    fn make_passport(env: &Env, manufacturer: &Address, tag: u8) -> BytesN<32> {
        let mut product_id = Bytes::from_slice(env, b"PRODUCT-");
        product_id.append(&Bytes::from_slice(env, &[tag]));
        let materials = vec![env, test_material(env)];
        create_passport(
            env,
            product_id,
            Bytes::from_slice(env, b"Widget"),
            Symbol::new(env, "electronics"),
            manufacturer.clone(),
            Bytes::from_slice(env, b"V1.0"),
            Bytes::from_slice(env, b"BATCH-001"),
            materials,
            Durability {
                expected_lifetime_years: 5,
                warranty_years: 2,
                spare_parts_available: true,
                spare_parts_years: 10,
                repair_information: Bytes::from_slice(env, b"https://repair.example.com"),
                repairability_score: 8,
            },
            Circularity {
                recyclable_materials: vec![env, test_material(env)],
                recycled_content_percent: 30,
                reuse_potential: true,
                refurbishment_potential: true,
                disassembly_instructions: Bytes::from_slice(env, b"See manual"),
                recycling_instructions: Bytes::from_slice(env, b"Recycle"),
                end_of_life_score: 85,
            },
            CarbonFootprint {
                manufacturing_emissions: 500,
                distribution_emissions: 100,
                use_phase_emissions: 50,
                end_of_life_emissions: 25,
                total_embodied_carbon: 675,
                carbon_neutral: false,
                carbon_offset_program: Bytes::from_slice(env, b""),
                measurement_standard: Symbol::new(env, "ISO_14040"),
                measurement_date: 1692345600,
            },
        )
    }

    #[test]
    fn test_legacy_mode_allows_unroled_callers() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            // No RBAC bootstrap -> legacy behavior, plain require_auth only.
            let manufacturer = Address::generate(&env);
            let passport_id = make_passport(&env, &manufacturer, 1);
            assert_eq!(get_total_passports(&env), 1);
            assert_eq!(get_audit_trail(&env, passport_id).len(), 1);
        });
    }

    #[test]
    #[should_panic(expected = "#2013")]
    fn test_rbac_blocks_unroled_manufacturer() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            let admin = Address::generate(&env);
            RbacManager::initialize_admin(&env, &admin);
            let manufacturer = Address::generate(&env);
            make_passport(&env, &manufacturer, 2);
        });
    }

    #[test]
    fn test_rbac_allows_roled_manufacturer_and_records_audit() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            let admin = Address::generate(&env);
            let manufacturer = Address::generate(&env);
            let verifier = Address::generate(&env);
            RbacManager::initialize_admin(&env, &admin);
            RbacManager::set_role(&env, &admin, &manufacturer, Role::Submitter).unwrap();
            RbacManager::set_role(&env, &admin, &verifier, Role::Auditor).unwrap();

            let passport_id = make_passport(&env, &manufacturer, 3);

            // Lifecycle transition gated to Submitter+ -> manufacturer qualifies.
            transition_lifecycle_stage(
                &env,
                passport_id,
                PassportLifecycleStage::InProduction,
                manufacturer.clone(),
                Bytes::from_slice(&env, b"production started"),
            );

            // Compliance verification gated to Auditor+ -> verifier qualifies.
            let status = verify_espr_compliance(&env, passport_id, verifier.clone());
            assert_eq!(status, ComplianceStatus::Compliant);

            let trail = get_audit_trail(&env, passport_id);
            assert_eq!(trail.len(), 3);
            let a = trail.get(0).unwrap();
            let b = trail.get(1).unwrap();
            let c = trail.get(2).unwrap();
            assert_eq!(a.seq, 0);
            assert_eq!(a.kind, AuditEventKind::PassportCreated);
            assert_eq!(b.kind, AuditEventKind::LifecycleStageChanged);
            assert_eq!(c.kind, AuditEventKind::ComplianceVerified);
            // Hash chain is tamper-evident: c commits to (c.payload_hash, b.event_hash).
            let mut chain_input = Bytes::new(&env);
            chain_input.append(&c.payload_hash.to_bytes());
            chain_input.append(&b.event_hash.to_bytes());
            assert_eq!(env.crypto().sha256(&chain_input).to_bytes(), c.event_hash);
        });
    }

    #[test]
    #[should_panic(expected = "#2014")]
    fn test_rbac_blocks_suspended_submitter() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            let admin = Address::generate(&env);
            let manufacturer = Address::generate(&env);
            RbacManager::initialize_admin(&env, &admin);
            RbacManager::set_role(&env, &admin, &manufacturer, Role::Submitter).unwrap();
            let passport_id = make_passport(&env, &manufacturer, 4);
            RbacManager::suspend_submitter(&env, &admin, &manufacturer).unwrap();
            update_passport(
                &env,
                passport_id,
                None,
                None,
                None,
                manufacturer.clone(),
            );
        });
    }

    #[test]
    fn test_cursor_pagination_and_filters() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            let manufacturer = Address::generate(&env);
            for tag in 5..8u8 {
                let passport_id = make_passport(&env, &manufacturer, tag);
                transition_lifecycle_stage(
                    &env,
                    passport_id,
                    PassportLifecycleStage::InProduction,
                    manufacturer.clone(),
                    Bytes::from_slice(&env, b"prod"),
                );
            }
            assert_eq!(get_total_passports(&env), 3);

            // Page 1: deterministic order and an explorable cursor.
            let page1 = get_all_passports_paged(&env, None, 2, None);
            assert_eq!(page1.items.len(), 2);
            assert!(page1.has_more);
            assert_eq!(page1.scanned, 2);
            // Page 2 resumes past the cursor without duplication.
            let page2 = get_all_passports_paged(&env, page1.next_cursor.clone(), 2, None);
            assert_eq!(page2.items.len(), 1);
            assert!(!page2.has_more);
            assert_ne!(page1.items.get(0).unwrap(), page2.items.get(0).unwrap());

            // Filters: only stage-matching passports are returned.
            let mut filter = PassportFilter::new(&env);
            filter.stage = Some(PassportLifecycleStage::Created);
            let created_page = get_all_passports_paged(&env, None, 10, Some(filter.clone()));
            assert_eq!(created_page.items.len(), 0);

            let mut filter = PassportFilter::new(&env);
            filter.stage = Some(PassportLifecycleStage::InProduction);
            let prod_page = get_all_passports_paged(&env, None, 10, Some(filter.clone()));
            assert_eq!(prod_page.items.len(), 3);
            // get_passports_by_stage now runs the same deterministic scan.
            assert_eq!(
                get_passports_by_stage(&env, PassportLifecycleStage::InProduction).len(),
                3
            );
        });
    }

    #[test]
    fn test_ttl_and_archival_helpers() {
        let env = Env::default();
        let contract_id = env.register(PassportTestContract, ());
        env.as_contract(&contract_id, || {
            env.mock_all_auths();
            let manufacturer = Address::generate(&env);
            let passport_id = make_passport(&env, &manufacturer, 9);

            // Permissionless TTL hook and archival pinning must not panic.
            extend_passport_ttl(&env, &passport_id);
            let archived = archive_passport(&env, passport_id, Address::generate(&env));
            assert_eq!(archived.passport_id, passport_id);
            // Audit trail records the archival action.
            let trail = get_audit_trail(&env, passport_id);
            assert_eq!(
                trail.last().unwrap().kind,
                AuditEventKind::PassportArchived
            );
        });
    }
}
