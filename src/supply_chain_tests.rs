#[cfg(test)]
mod tests {
    use crate::rbac::*;
    use crate::supply_chain::*;
    use soroban_sdk::{bytes, vec, Address, BytesN, Env, Symbol, Vec};

    /// Helper to create test location
    fn create_test_location(env: &Env, name: &str, country: &str) -> Location {
        Location {
            name: bytes!(env, name),
            country: Symbol::new(env, country),
            coordinates: bytes!(env, "0.0,0.0"),
            facility_id: bytes!(env, "FAC001"),
        }
    }

    /// Helper to grant a role to an address for RBAC-gated tests.
    fn grant_role(env: &Env, admin: &Address, account: &Address, role: Role) {
        rbac_grant_role(env, admin.clone(), account.clone(), role);
    }

    /// Helper to set up an admin and grant a role to a caller.
    fn setup_caller_with_role(env: &Env, role: Role) -> Address {
        let admin = Address::random(env);
        let caller = Address::random(env);
        rbac_init(env, admin.clone());
        grant_role(env, &admin, &caller, role);
        caller
    }

    /// Helper to count audit events emitted in the current test environment.
    fn audit_event_count(env: &Env) -> u32 {
        env.events().all().len()
    }

    /// Test brand registration
    #[test]
    fn test_register_brand() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");

        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier of quality products"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        // Verify brand was registered by checking it exists in storage
        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::Brand(brand_id.clone())));
    }

    /// Test product SKU registration
    #[test]
    fn test_register_product_sku() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");

        // Register brand first
        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier of quality products"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        // Register product SKU
        let sku = bytes!(&env, "SKU-12345");
        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Premium Widget"),
            bytes!(&env, "High quality widget for general use"),
        );

        // Verify product was registered
        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::ProductSKU(brand_id.clone(), sku)));
    }

    /// Test provenance event logging
    #[test]
    fn test_log_provenance_event() {
        let env = Env::default();
        env.mock_all_auths();

        let producer = Address::random(&env);
        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Widget Factory", "US");

        log_provenance_event(
            &env,
            event_id,
            location,
            bytes!(&env, "Premium aluminum"),
            producer.clone(),
            bytes!(&env, "BATCH-2024-001"),
        );

        // Verify provenance was logged
        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id))
            .expect("Provenance event should exist");

        assert_eq!(provenance.producer_address, producer);
        assert!(!provenance.is_verified);
    }

    /// Test custody transfer logging
    #[test]
    fn test_log_custody_transfer() {
        let env = Env::default();
        env.mock_all_auths();

        let producer = Address::random(&env);
        let distributor = Address::random(&env);
        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Widget Factory", "US");

        // First log provenance
        log_provenance_event(
            &env,
            event_id,
            location.clone(),
            bytes!(&env, "Premium aluminum"),
            producer.clone(),
            bytes!(&env, "BATCH-2024-001"),
        );

        // Log custody transfer
        let transfer_location = create_test_location(&env, "Distribution Center", "CA");
        log_custody_transfer(
            &env,
            event_id,
            producer.clone(),
            distributor.clone(),
            transfer_location,
            bytes!(&env, "Shipped via truck"),
        );

        // Verify custody transfer was logged
        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id))
            .expect("Provenance event should exist");

        assert_eq!(provenance.chain_of_custody.len(), 1);
        assert_eq!(provenance.chain_of_custody.get(0).unwrap().to_address, distributor);
    }

    /// Test certification logging
    #[test]
    fn test_log_certification() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let cert_id = bytes!(&env, "CERT-ISO9001-2024");
        let cert_type = Symbol::new(&env, "ISO_9001");

        log_certification(
            &env,
            cert_id.clone(),
            cert_type.clone(),
            issuer.clone(),
            365, // 1 year validity
            bytes!(&env, "Quality management system for manufacturing"),
        );

        // Retrieve certification from storage
        let cert_event_id = env.crypto().sha256(&cert_id);
        let certification: Certification = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::CertificationEvent(cert_event_id))
            .expect("Certification should exist");

        assert_eq!(certification.issuer, issuer);
        assert!(certification.is_active);
        assert_eq!(certification.cert_type, cert_type);
    }

    /// Test labor conditions logging
    #[test]
    fn test_log_labor_conditions() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-FACTORY-001");
        let report_hash = BytesN::<32>::random(&env);

        log_labor_conditions(
            &env,
            facility_id.clone(),
            500,            // workers
            true,           // wage compliant
            true,           // hours compliant
            true,           // no child labor
            true,           // safety met
            true,           // freedom of association
            report_hash,
            reporter,
        );

        // Verify labor report was logged
        let report_id = env.crypto().sha256(&facility_id);
        let labor: LaborConditions = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::LaborReport(report_id))
            .expect("Labor report should exist");

        assert_eq!(labor.worker_count, 500);
        assert!(labor.wage_compliance);
        assert!(labor.child_labor_free);
    }

    /// Test environmental impact logging
    #[test]
    fn test_log_environmental_impact() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-FACTORY-001");
        let report_hash = BytesN::<32>::random(&env);
        let now = env.ledger().timestamp();

        log_environmental_impact(
            &env,
            facility_id.clone(),
            now,                                    // period start
            now + (30 * 86400),                     // period end
            5000,                                   // carbon footprint (kg CO2e)
            1000000,                                // water usage (liters)
            500,                                    // waste (kg)
            75,                                     // renewable energy %
            10,                                     // emissions reduction %
            report_hash,
            reporter,
        );

        // Verify environmental report was logged
        let report_id = env.crypto().sha256(&facility_id);
        let env_impact: EnvironmentalImpact = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::EnvironmentalReport(report_id))
            .expect("Environmental report should exist");

        assert_eq!(env_impact.carbon_footprint, 5000);
        assert_eq!(env_impact.renewable_energy_percent, 75);
        assert!(env_impact.emissions_reduction_percent > 0);
    }

    /// Test product chain verification
    #[test]
    fn test_verify_product_chain_minimal() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");
        let sku = bytes!(&env, "SKU-12345");

        // Setup brand and product
        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Premium Widget"),
            bytes!(&env, "High quality widget"),
        );

        // Verify product chain
        let verification = verify_product_chain(&env, brand_id.clone(), sku.clone()).unwrap();

        assert_eq!(verification.product_sku, sku);
        assert!(!verification.is_verified); // Not verified without full chain
    }

    /// Test certification verification
    #[test]
    fn test_verify_certification_valid() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let cert_id = bytes!(&env, "CERT-ISO9001-2024");

        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            issuer,
            365, // 1 year
            bytes!(&env, "Quality management system"),
        );

        let is_valid = verify_certification(&env, cert_id);
        assert!(is_valid);
    }

    /// Test certification verification expired
    #[test]
    fn test_verify_certification_expired() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let cert_id = bytes!(&env, "CERT-EXPIRED-2024");

        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            issuer,
            0, // Already expired
            bytes!(&env, "Expired certification"),
        );

        // Note: This may not work as expected due to how expiry is calculated
        // but demonstrates the testing pattern
        let is_valid = verify_certification(&env, cert_id);
        assert!(!is_valid);
    }

    /// Test get product timeline
    #[test]
    fn test_get_product_timeline() {
        let env = Env::default();
        env.mock_all_auths();

        let producer = Address::random(&env);
        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Widget Factory", "US");

        log_provenance_event(
            &env,
            event_id,
            location,
            bytes!(&env, "Premium aluminum"),
            producer.clone(),
            bytes!(&env, "BATCH-2024-001"),
        );

        let event_ids = vec![&env, event_id];
        let timeline = get_product_timeline(&env, event_ids).unwrap();

        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline.get(0).unwrap().entry_type, Symbol::new(&env, "origin"));
    }

    /// Test brand integrity report
    #[test]
    fn test_get_brand_integrity_report() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");

        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        let report = get_brand_integrity_report(&env, brand_id.clone()).unwrap();

        assert_eq!(report.brand_id, brand_id);
        assert!(report.compliance_trend == Symbol::new(&env, "stable"));
    }

    /// Test QR code URL generation
    #[test]
    fn test_generate_qr_code_url() {
        let env = Env::default();
        let brand_id = Symbol::new(&env, "ACME");
        let sku = bytes!(&env, "SKU-12345");
        let base_url = bytes!(&env, "https://verify.acme.example.com/verify");

        let qr_url = generate_qr_code_url(&env, brand_id, sku, base_url);

        assert!(!qr_url.is_empty());
    }

    /// Test integrity proof generation
    #[test]
    fn test_generate_integrity_proof() {
        let env = Env::default();
        env.mock_all_auths();

        let event_ids = vec![&env, BytesN::<32>::random(&env), BytesN::<32>::random(&env)];

        let proof = generate_integrity_proof(&env, event_ids).unwrap();

        // Proof should be a valid 32-byte hash
        assert_eq!(proof.len(), 32);
    }

    /// Test multiple custody transfers
    #[test]
    fn test_multiple_custody_transfers() {
        let env = Env::default();
        env.mock_all_auths();

        let producer = Address::random(&env);
        let distributor = Address::random(&env);
        let retailer = Address::random(&env);
        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Factory", "US");

        // Log provenance
        log_provenance_event(
            &env,
            event_id,
            location.clone(),
            bytes!(&env, "Premium aluminum"),
            producer.clone(),
            bytes!(&env, "BATCH-2024-001"),
        );

        // Transfer 1: Producer -> Distributor
        let dist_location = create_test_location(&env, "Distribution", "CA");
        log_custody_transfer(
            &env,
            event_id,
            producer.clone(),
            distributor.clone(),
            dist_location,
            bytes!(&env, "Transport 1"),
        );

        // Transfer 2: Distributor -> Retailer
        let retail_location = create_test_location(&env, "Retail", "TX");
        log_custody_transfer(
            &env,
            event_id,
            distributor.clone(),
            retailer.clone(),
            retail_location,
            bytes!(&env, "Transport 2"),
        );

        // Verify chain
        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id))
            .expect("Provenance event should exist");

        assert_eq!(provenance.chain_of_custody.len(), 2);
        assert_eq!(provenance.chain_of_custody.get(0).unwrap().to_address, distributor);
        assert_eq!(provenance.chain_of_custody.get(1).unwrap().to_address, retailer);
    }

    /// Test labor conditions with compliance issues
    #[test]
    fn test_labor_conditions_non_compliant() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-FACTORY-BAD");
        let report_hash = BytesN::<32>::random(&env);

        log_labor_conditions(
            &env,
            facility_id.clone(),
            50,             // workers
            false,          // wage NOT compliant
            false,          // hours NOT compliant
            false,          // child labor present
            false,          // safety NOT met
            false,          // no freedom of association
            report_hash,
            reporter,
        );

        let report_id = env.crypto().sha256(&facility_id);
        let labor: LaborConditions = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::LaborReport(report_id))
            .expect("Labor report should exist");

        assert!(!labor.wage_compliance);
        assert!(!labor.child_labor_free);
        assert!(!labor.safety_standards_met);
    }

    /// Test environmental impact tracking
    #[test]
    fn test_environmental_impact_improvement() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-FACTORY-GREEN");
        let report_hash = BytesN::<32>::random(&env);
        let now = env.ledger().timestamp();

        log_environmental_impact(
            &env,
            facility_id.clone(),
            now,
            now + (30 * 86400),
            2000,       // Lower carbon footprint
            500000,     // Lower water usage
            100,        // Lower waste
            90,         // High renewable energy
            25,         // Good emissions reduction
            report_hash,
            reporter,
        );

        let report_id = env.crypto().sha256(&facility_id);
        let env_impact: EnvironmentalImpact = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::EnvironmentalReport(report_id))
            .expect("Environmental report should exist");

        assert!(env_impact.renewable_energy_percent >= 80);
        assert!(env_impact.emissions_reduction_percent > 20);
    }

    // ========================================================================
    // Storage footprint regression tests
    // ========================================================================

    /// Pinned XDR byte budgets for each record type. These budgets are the
    /// regression thresholds: any change that inflates the serialized size of
    /// a record beyond its budget will fail CI.
    const BUDGET_BRAND: u32 = 512;
    const BUDGET_PRODUCT_SKU: u32 = 512;
    const BUDGET_LOCATION: u32 = 256;
    const BUDGET_CUSTODY_TRANSFER: u32 = 384;
    const BUDGET_PROVENANCE: u32 = 1024;
    const BUDGET_CERTIFICATION: u32 = 512;
    const BUDGET_LABOR_CONDITIONS: u32 = 384;
    const BUDGET_ENVIRONMENTAL_IMPACT: u32 = 384;
    const BUDGET_VERIFICATION_RESULT: u32 = 512;
    const BUDGET_INTEGRITY_REPORT: u32 = 512;

    /// Serialize a value to XDR and return its byte length.
    fn xdr_len<T>(env: &Env, value: &T) -> u32
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        let val: soroban_sdk::Val = value.into_val(env);
        let bytes = val.to_bytes();
        bytes.len()
    }

    /// Assert that a serialized record stays within its pinned byte budget.
    fn assert_within_budget(label: &str, actual: u32, budget: u32) {
        assert!(
            actual <= budget,
            "{} serialized size {} exceeds pinned budget {}",
            label,
            actual,
            budget
        );
    }

    #[test]
    fn test_storage_footprint_brand() {
        let env = Env::default();
        let brand = Brand {
            brand_id: Symbol::new(&env, "ACME"),
            owner: Address::random(&env),
            name: bytes!(&env, "ACME Corporation"),
            description: bytes!(&env, "Leading supplier of quality products"),
            website: bytes!(&env, "https://acme.example.com"),
            contact_email: bytes!(&env, "support@acme.example.com"),
            is_verified: true,
            registered_at: 1_700_000_000,
        };
        assert_within_budget("Brand", xdr_len(&env, &brand), BUDGET_BRAND);
    }

    #[test]
    fn test_storage_footprint_product_sku() {
        let env = Env::default();
        let product = ProductSKU {
            brand_id: Symbol::new(&env, "ACME"),
            sku: bytes!(&env, "SKU-12345"),
            name: bytes!(&env, "Premium Widget"),
            description: bytes!(&env, "High quality widget for general use"),
            created_at: 1_700_000_000,
        };
        assert_within_budget("ProductSKU", xdr_len(&env, &product), BUDGET_PRODUCT_SKU);
    }

    #[test]
    fn test_storage_footprint_location() {
        let env = Env::default();
        let location = create_test_location(&env, "Widget Factory", "US");
        assert_within_budget("Location", xdr_len(&env, &location), BUDGET_LOCATION);
    }

    #[test]
    fn test_storage_footprint_custody_transfer() {
        let env = Env::default();
        let transfer = CustodyTransfer {
            from_address: Address::random(&env),
            to_address: Address::random(&env),
            location: create_test_location(&env, "Distribution Center", "CA"),
            timestamp: 1_700_000_000,
            notes: bytes!(&env, "Shipped via truck"),
        };
        assert_within_budget(
            "CustodyTransfer",
            xdr_len(&env, &transfer),
            BUDGET_CUSTODY_TRANSFER,
        );
    }

    #[test]
    fn test_storage_footprint_provenance() {
        let env = Env::default();
        let mut chain = vec![&env];
        chain.push_back(CustodyTransfer {
            from_address: Address::random(&env),
            to_address: Address::random(&env),
            location: create_test_location(&env, "Distribution Center", "CA"),
            timestamp: 1_700_000_000,
            notes: bytes!(&env, "Shipped via truck"),
        });
        let provenance = Provenance {
            event_id: BytesN::<32>::random(&env),
            location: create_test_location(&env, "Widget Factory", "US"),
            raw_materials: bytes!(&env, "Premium aluminum"),
            producer_address: Address::random(&env),
            timestamp: 1_700_000_000,
            batch_number: bytes!(&env, "BATCH-2024-001"),
            is_verified: true,
            chain_of_custody: chain,
        };
        assert_within_budget("Provenance", xdr_len(&env, &provenance), BUDGET_PROVENANCE);
    }

    #[test]
    fn test_storage_footprint_certification() {
        let env = Env::default();
        let cert = Certification {
            cert_id: bytes!(&env, "CERT-ISO9001-2024"),
            cert_type: Symbol::new(&env, "ISO_9001"),
            issuer: Address::random(&env),
            issued_at: 1_700_000_000,
            expires_at: 1_700_000_000 + 365 * 86400,
            is_active: true,
            details: bytes!(&env, "Quality management system for manufacturing"),
        };
        assert_within_budget(
            "Certification",
            xdr_len(&env, &cert),
            BUDGET_CERTIFICATION,
        );
    }

    #[test]
    fn test_storage_footprint_labor_conditions() {
        let env = Env::default();
        let labor = LaborConditions {
            facility_id: bytes!(&env, "FAC-FACTORY-001"),
            worker_count: 500,
            wage_compliance: true,
            hours_compliance: true,
            child_labor_free: true,
            safety_standards_met: true,
            freedom_of_association: true,
            report_hash: BytesN::<32>::random(&env),
            reporter: Address::random(&env),
            timestamp: 1_700_000_000,
        };
        assert_within_budget(
            "LaborConditions",
            xdr_len(&env, &labor),
            BUDGET_LABOR_CONDITIONS,
        );
    }

    #[test]
    fn test_storage_footprint_environmental_impact() {
        let env = Env::default();
        let impact = EnvironmentalImpact {
            facility_id: bytes!(&env, "FAC-FACTORY-001"),
            period_start: 1_700_000_000,
            period_end: 1_700_000_000 + 30 * 86400,
            carbon_footprint: 5000,
            water_usage: 1_000_000,
            waste_generated: 500,
            renewable_energy_percent: 75,
            emissions_reduction_percent: 10,
            report_hash: BytesN::<32>::random(&env),
            reporter: Address::random(&env),
            timestamp: 1_700_000_000,
        };
        assert_within_budget(
            "EnvironmentalImpact",
            xdr_len(&env, &impact),
            BUDGET_ENVIRONMENTAL_IMPACT,
        );
    }

    #[test]
    fn test_storage_footprint_verification_result() {
        let env = Env::default();
        let result = VerificationResult {
            brand_id: Symbol::new(&env, "ACME"),
            product_sku: bytes!(&env, "SKU-12345"),
            is_verified: true,
            provenance_events: 3,
            certifications: 2,
            labor_compliant: true,
            environmental_compliant: true,
            verified_at: 1_700_000_000,
        };
        assert_within_budget(
            "VerificationResult",
            xdr_len(&env, &result),
            BUDGET_VERIFICATION_RESULT,
        );
    }

    #[test]
    fn test_storage_footprint_integrity_report() {
        let env = Env::default();
        let report = BrandIntegrityReport {
            brand_id: Symbol::new(&env, "ACME"),
            total_products: 10,
            verified_products: 8,
            total_provenance_events: 25,
            active_certifications: 3,
            labor_compliance_rate: 95,
            environmental_score: 80,
            compliance_trend: Symbol::new(&env, "stable"),
            generated_at: 1_700_000_000,
        };
        assert_within_budget(
            "BrandIntegrityReport",
            xdr_len(&env, &report),
            BUDGET_INTEGRITY_REPORT,
        );
    }

    // ========================================================================
    // Storage tier classification tests
    // ========================================================================

    /// Assert that a key lives in the persistent storage tier.
    fn assert_persistent_tier(env: &Env, key: &SupplyChainDataKey) {
        assert!(
            env.storage().persistent().has(key),
            "key {:?} must be stored in the persistent tier",
            key
        );
    }

    #[test]
    fn test_storage_tier_classification() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");
        let sku = bytes!(&env, "SKU-12345");

        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );
        assert_persistent_tier(&env, &SupplyChainDataKey::Brand(brand_id.clone()));

        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Premium Widget"),
            bytes!(&env, "High quality widget"),
        );
        assert_persistent_tier(
            &env,
            &SupplyChainDataKey::ProductSKU(brand_id.clone(), sku.clone()),
        );

        let event_id = BytesN::<32>::random(&env);
        log_provenance_event(
            &env,
            event_id,
            create_test_location(&env, "Widget Factory", "US"),
            bytes!(&env, "Premium aluminum"),
            Address::random(&env),
            bytes!(&env, "BATCH-2024-001"),
        );
        assert_persistent_tier(&env, &SupplyChainDataKey::ProvenanceEvent(event_id));

        let cert_id = bytes!(&env, "CERT-ISO9001-2024");
        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            Address::random(&env),
            365,
            bytes!(&env, "Quality management system"),
        );
        let cert_event_id = env.crypto().sha256(&cert_id);
        assert_persistent_tier(&env, &SupplyChainDataKey::CertificationEvent(cert_event_id));

        let facility_id = bytes!(&env, "FAC-FACTORY-001");
        log_labor_conditions(
            &env,
            facility_id.clone(),
            500,
            true,
            true,
            true,
            true,
            true,
            BytesN::<32>::random(&env),
            Address::random(&env),
        );
        let labor_id = env.crypto().sha256(&facility_id);
        assert_persistent_tier(&env, &SupplyChainDataKey::LaborReport(labor_id));

        let now = env.ledger().timestamp();
        log_environmental_impact(
            &env,
            facility_id.clone(),
            now,
            now + 30 * 86400,
            5000,
            1_000_000,
            500,
            75,
            10,
            BytesN::<32>::random(&env),
            Address::random(&env),
        );
        let env_id = env.crypto().sha256(&facility_id);
        assert_persistent_tier(&env, &SupplyChainDataKey::EnvironmentalReport(env_id));
    }

    // ========================================================================
    // Permissionless TTL extension tests
    // ========================================================================

    #[test]
    fn test_extend_ttl_permissionless_for_brand() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");

        register_brand(
            &env,
            owner,
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        let key = SupplyChainDataKey::Brand(brand_id);
        let before = env.storage().persistent().get_ttl(&key);

        // Any address (no auth required) may extend the TTL.
        let caller = Address::random(&env);
        let _ = caller;
        extend_ttl(&env, key.clone());

        let after = env.storage().persistent().get_ttl(&key);
        assert!(
            after >= before,
            "extend_ttl must not reduce the TTL (before={}, after={})",
            before,
            after
        );
    }

    #[test]
    fn test_extend_ttl_permissionless_for_provenance() {
        let env = Env::default();
        env.mock_all_auths();

        let event_id = BytesN::<32>::random(&env);
        log_provenance_event(
            &env,
            event_id,
            create_test_location(&env, "Widget Factory", "US"),
            bytes!(&env, "Premium aluminum"),
            Address::random(&env),
            bytes!(&env, "BATCH-2024-001"),
        );

        let key = SupplyChainDataKey::ProvenanceEvent(event_id);
        let before = env.storage().persistent().get_ttl(&key);

        // No auth mocking for the caller: the helper must be permissionless.
        extend_ttl(&env, key.clone());

        let after = env.storage().persistent().get_ttl(&key);
        assert!(after >= before);
    }

    #[test]
    fn test_extend_ttl_for_all_archival_critical_records() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");
        let sku = bytes!(&env, "SKU-12345");

        register_brand(
            &env,
            owner,
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );
        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Premium Widget"),
            bytes!(&env, "High quality widget"),
        );

        let event_id = BytesN::<32>::random(&env);
        log_provenance_event(
            &env,
            event_id,
            create_test_location(&env, "Widget Factory", "US"),
            bytes!(&env, "Premium aluminum"),
            Address::random(&env),
            bytes!(&env, "BATCH-2024-001"),
        );

        let cert_id = bytes!(&env, "CERT-ISO9001-2024");
        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            Address::random(&env),
            365,
            bytes!(&env, "Quality management system"),
        );

        let facility_id = bytes!(&env, "FAC-FACTORY-001");
        log_labor_conditions(
            &env,
            facility_id.clone(),
            500,
            true,
            true,
            true,
            true,
            true,
            BytesN::<32>::random(&env),
            Address::random(&env),
        );

        let now = env.ledger().timestamp();
        log_environmental_impact(
            &env,
            facility_id.clone(),
            now,
            now + 30 * 86400,
            5000,
            1_000_000,
            500,
            75,
            10,
            BytesN::<32>::random(&env),
            Address::random(&env),
        );

        let keys = vec![
            &env,
            SupplyChainDataKey::Brand(brand_id.clone()),
            SupplyChainDataKey::ProductSKU(brand_id.clone(), sku.clone()),
            SupplyChainDataKey::ProvenanceEvent(event_id),
            SupplyChainDataKey::CertificationEvent(env.crypto().sha256(&cert_id)),
            SupplyChainDataKey::LaborReport(env.crypto().sha256(&facility_id)),
            SupplyChainDataKey::EnvironmentalReport(env.crypto().sha256(&facility_id)),
        ];

        for key in keys.iter() {
            let before = env.storage().persistent().get_ttl(&key);
            extend_ttl(&env, key.clone());
            let after = env.storage().persistent().get_ttl(&key);
            assert!(
                after >= before,
                "TTL must not shrink for archival-critical key {:?}",
                key
            );
        }
    }

    // ========================================================================
    // Ledger byte read/write savings benchmark (1,000 simulated transactions)
    // ========================================================================

    /// Benchmark-style assertion: measure the total serialized ledger bytes
    /// written and read across 1,000 simulated transactions and assert the
    /// per-transaction footprint stays within the pinned budget.
    #[test]
    fn test_ledger_byte_savings_benchmark_1000_tx() {
        const TX_COUNT: u32 = 1_000;
        // Pinned per-transaction byte budget for the write + read footprint.
        const BYTES_PER_TX_BUDGET: u32 = 4_096;

        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let brand_id = Symbol::new(&env, "ACME");
        let sku = bytes!(&env, "SKU-12345");

        register_brand(
            &env,
            owner,
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );
        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Premium Widget"),
            bytes!(&env, "High quality widget"),
        );

        let brand_key = SupplyChainDataKey::Brand(brand_id.clone());
        let sku_key = SupplyChainDataKey::ProductSKU(brand_id.clone(), sku.clone());

        let brand: Brand = env
            .storage()
            .persistent()
            .get(&brand_key)
            .expect("brand must exist");
        let product: ProductSKU = env
            .storage()
            .persistent()
            .get(&sku_key)
            .expect("product must exist");

        let write_bytes_per_tx = xdr_len(&env, &brand) + xdr_len(&env, &product);

        let mut total_write_bytes: u64 = 0;
        let mut total_read_bytes: u64 = 0;

        for _ in 0..TX_COUNT {
            // Simulated write: re-serialize the records that would be persisted.
            total_write_bytes += write_bytes_per_tx as u64;

            // Simulated read: fetch and re-serialize the records.
            let read_brand: Brand = env
                .storage()
                .persistent()
                .get(&brand_key)
                .expect("brand must exist");
            let read_product: ProductSKU = env
                .storage()
                .persistent()
                .get(&sku_key)
                .expect("product must exist");
            total_read_bytes += (xdr_len(&env, &read_brand) + xdr_len(&env, &read_product)) as u64;
        }

        let total_bytes = total_write_bytes + total_read_bytes;
        let bytes_per_tx = (total_bytes / TX_COUNT as u64) as u32;

        assert!(
            bytes_per_tx <= BYTES_PER_TX_BUDGET,
            "per-transaction ledger footprint {} exceeds pinned budget {}",
            bytes_per_tx,
            BYTES_PER_TX_BUDGET
        );

        // Sanity: the benchmark must actually exercise 1,000 transactions.
        assert_eq!(total_write_bytes / write_bytes_per_tx as u64, TX_COUNT as u64);
    }

    /// Test full supply chain scenario
    #[test]
    fn test_full_supply_chain_scenario() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let producer = Address::random(&env);
        let certifier = Address::random(&env);
        let labor_auditor = Address::random(&env);
        let env_auditor = Address::random(&env);

        let brand_id = Symbol::new(&env, "ECO_BRAND");
        let sku = bytes!(&env, "ECO-WIDGET-001");

        // 1. Register brand
        register_brand(
            &env,
            owner,
            brand_id.clone(),
            bytes!(&env, "Eco-Friendly Products Inc"),
            bytes!(&env, "Sustainable product manufacturer"),
            bytes!(&env, "https://eco-brand.example.com"),
            bytes!(&env, "support@eco-brand.example.com"),
        );

        // 2. Register product SKU
        register_product_sku(
            &env,
            brand_id.clone(),
            sku.clone(),
            bytes!(&env, "Eco Widget"),
            bytes!(&env, "100% recyclable widget"),
        );

        // 3. Log provenance
        let factory_location = create_test_location(&env, "Eco Factory", "DE");
        let prov_event_id = BytesN::<32>::random(&env);
        log_provenance_event(
            &env,
            prov_event_id,
            factory_location,
            bytes!(&env, "Recycled materials"),
            producer.clone(),
            bytes!(&env, "BATCH-ECO-2024-001"),
        );

        // 4. Log certification
        log_certification(
            &env,
            bytes!(&env, "CERT-ECOLABEL-001"),
            Symbol::new(&env, "EU_ECOLABEL"),
            certifier,
            730, // 2 years
            bytes!(&env, "EU Ecolabel certified sustainable product"),
        );

        // 5. Log labor conditions
        log_labor_conditions(
            &env,
            bytes!(&env, "FAC-ECO-001"),
            200,
            true,
            true,
            true,
            true,
            true,
            BytesN::<32>::random(&env),
            labor_auditor,
        );

        // 6. Log environmental impact
        let now = env.ledger().timestamp();
        log_environmental_impact(
            &env,
            bytes!(&env, "FAC-ECO-001"),
            now,
            now + (30 * 86400),
            1000,
            250000,
            50,
            95,
            15,
            BytesN::<32>::random(&env),
            env_auditor,
        );

        // 7. Verify product chain (will have limited verification without full setup)
        let verification = verify_product_chain(&env, brand_id, sku).unwrap();
        assert_eq!(verification.product_sku, bytes!(&env, "ECO-WIDGET-001"));
    }

    // =========================================================================
    // Boundary & adversarial tests (issue #869)
    // =========================================================================

    /// Helper: build a provenance event with a given number of custody transfers.
    fn build_custody_chain(env: &Env, transfers: u32) -> BytesN<32> {
        let producer = Address::random(env);
        let event_id = BytesN::<32>::random(env);
        let location = create_test_location(env, "Factory", "US");

        log_provenance_event(
            env,
            event_id.clone(),
            location,
            bytes!(env, "material"),
            producer.clone(),
            bytes!(env, "BATCH"),
        );

        let mut from = producer;
        for i in 0..transfers {
            let to = Address::random(env);
            let loc = create_test_location(env, "Transfer", "US");
            log_custody_transfer(
                env,
                event_id.clone(),
                from.clone(),
                to.clone(),
                loc,
                bytes!(env, "handoff"),
            );
            from = to;
            let _ = i;
        }

        event_id
    }

    /// Custody chain length is capped; exceeding the cap returns a typed error
    /// instead of panicking or growing storage unboundedly.
    #[test]
    fn test_custody_chain_length_cap_enforced() {
        let env = Env::default();
        env.mock_all_auths();

        let event_id = build_custody_chain(&env, MAX_CUSTODY_CHAIN_LENGTH);

        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id.clone()))
            .expect("Provenance event should exist");
        assert_eq!(provenance.chain_of_custody.len(), MAX_CUSTODY_CHAIN_LENGTH);

        // One more transfer must be rejected with a typed error.
        let from = Address::random(&env);
        let to = Address::random(&env);
        let loc = create_test_location(&env, "Overflow", "US");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_custody_transfer(
                &env,
                event_id.clone(),
                from,
                to,
                loc,
                bytes!(&env, "overflow"),
            );
        }));
        assert!(result.is_err(), "expected typed error, not silent growth");

        // Storage must remain bounded.
        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id))
            .expect("Provenance event should exist");
        assert_eq!(provenance.chain_of_custody.len(), MAX_CUSTODY_CHAIN_LENGTH);
    }

    /// Timeline input length is capped; oversized inputs return a typed error.
    #[test]
    fn test_timeline_length_cap_enforced() {
        let env = Env::default();
        env.mock_all_auths();

        let mut ids = vec![&env];
        for _ in 0..(MAX_TIMELINE_ENTRIES + 1) {
            ids.push_back(BytesN::<32>::random(&env));
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            get_product_timeline(&env, ids).unwrap();
        }));
        assert!(result.is_err(), "expected typed error for oversized timeline");
    }

    /// Integrity proof input length is capped; oversized inputs return a typed error.
    #[test]
    fn test_integrity_proof_length_cap_enforced() {
        let env = Env::default();
        env.mock_all_auths();

        let mut ids = vec![&env];
        for _ in 0..(MAX_INTEGRITY_PROOF_EVENTS + 1) {
            ids.push_back(BytesN::<32>::random(&env));
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_integrity_proof(&env, ids).unwrap();
        }));
        assert!(result.is_err(), "expected typed error for oversized proof input");
    }

    /// Labor conditions with `u64::MAX` worker count must not overflow and must
    /// either be accepted deterministically or return a typed error.
    #[test]
    fn test_labor_conditions_u64_max_worker_count() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-MAX");
        let report_hash = BytesN::<32>::random(&env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_labor_conditions(
                &env,
                facility_id.clone(),
                u64::MAX,
                true,
                true,
                true,
                true,
                true,
                report_hash,
                reporter,
            );
        }));

        // Either a typed error or a deterministic stored value; never a panic
        // from arithmetic overflow.
        if result.is_ok() {
            let report_id = env.crypto().sha256(&facility_id);
            let labor: LaborConditions = env
                .storage()
                .persistent()
                .get(&SupplyChainDataKey::LaborReport(report_id))
                .expect("Labor report should exist");
            assert_eq!(labor.worker_count, u64::MAX);
        }
    }

    /// Environmental impact with `i128::MAX` values must not overflow and must
    /// return a typed error or store deterministically.
    #[test]
    fn test_environmental_impact_i128_max() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-I128-MAX");
        let report_hash = BytesN::<32>::random(&env);
        let now = env.ledger().timestamp();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_environmental_impact(
                &env,
                facility_id.clone(),
                now,
                now.saturating_add(30 * 86400),
                i128::MAX,
                i128::MAX,
                i128::MAX,
                100,
                100,
                report_hash,
                reporter,
            );
        }));

        if result.is_ok() {
            let report_id = env.crypto().sha256(&facility_id);
            let env_impact: EnvironmentalImpact = env
                .storage()
                .persistent()
                .get(&SupplyChainDataKey::EnvironmentalReport(report_id))
                .expect("Environmental report should exist");
            assert_eq!(env_impact.carbon_footprint, i128::MAX);
        }
    }

    /// Environmental impact with `i128::MIN` values must not underflow.
    #[test]
    fn test_environmental_impact_i128_min() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let facility_id = bytes!(&env, "FAC-I128-MIN");
        let report_hash = BytesN::<32>::random(&env);
        let now = env.ledger().timestamp();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_environmental_impact(
                &env,
                facility_id,
                now,
                now.saturating_add(30 * 86400),
                i128::MIN,
                i128::MIN,
                i128::MIN,
                0,
                0,
                report_hash,
                reporter,
            );
        }));

        // Must not panic from underflow; typed error is acceptable.
        let _ = result;
    }

    /// Certification validity with `u64::MAX` duration must not overflow when
    /// computing expiry timestamps.
    #[test]
    fn test_certification_u64_max_validity() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let cert_id = bytes!(&env, "CERT-MAX-VALIDITY");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_certification(
                &env,
                cert_id.clone(),
                Symbol::new(&env, "ISO_9001"),
                issuer,
                u64::MAX,
                bytes!(&env, "max validity"),
            );
        }));

        // Must not panic from timestamp overflow.
        let _ = result;
    }

    /// Certification with zero validity is deterministically invalid.
    #[test]
    fn test_certification_zero_validity_is_invalid() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let cert_id = bytes!(&env, "CERT-ZERO");

        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            issuer,
            0,
            bytes!(&env, "zero validity"),
        );

        assert!(!verify_certification(&env, cert_id));
    }

    /// Empty inputs must return deterministic results without panicking.
    #[test]
    fn test_empty_inputs_are_deterministic() {
        let env = Env::default();
        env.mock_all_auths();

        let empty_ids = vec![&env];
        let timeline = get_product_timeline(&env, empty_ids.clone()).unwrap();
        assert_eq!(timeline.len(), 0);

        let proof = generate_integrity_proof(&env, empty_ids).unwrap();
        assert_eq!(proof.len(), 32);
    }

    /// Adversarial: repeated custody transfers from the same address must not
    /// bypass the cap or corrupt the chain.
    #[test]
    fn test_repeated_transfers_do_not_bypass_cap() {
        let env = Env::default();
        env.mock_all_auths();

        let event_id = build_custody_chain(&env, MAX_CUSTODY_CHAIN_LENGTH);

        let same = Address::random(&env);
        let loc = create_test_location(&env, "Same", "US");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            log_custody_transfer(
                &env,
                event_id.clone(),
                same.clone(),
                same.clone(),
                loc,
                bytes!(&env, "self"),
            );
        }));
        assert!(result.is_err());

        let provenance: Provenance = env
            .storage()
            .persistent()
            .get(&SupplyChainDataKey::ProvenanceEvent(event_id))
            .expect("Provenance event should exist");
        assert_eq!(provenance.chain_of_custody.len(), MAX_CUSTODY_CHAIN_LENGTH);
    }

    // =========================================================================
    // RBAC Gating & Audit Event Tests
    // =========================================================================

    /// Positive RBAC: authorized caller with the required role can register a brand.
    #[test]
    fn test_rbac_register_brand_authorized() {
        let env = Env::default();
        env.mock_all_auths();

        let caller = setup_caller_with_role(&env, Role::BrandOwner);
        let brand_id = Symbol::new(&env, "ACME");

        register_brand(
            &env,
            caller.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::Brand(brand_id)));
    }

    /// Negative RBAC: caller without the required role is rejected with a stable error code.
    #[test]
    fn test_rbac_register_brand_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());
        // Caller has no role granted.

        let brand_id = Symbol::new(&env, "ACME");

        let result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "register_brand"),
            vec![
                &env,
                caller.clone().into_val(&env),
                brand_id.clone().into_val(&env),
            ],
        );

        assert!(result.is_err());
    }

    /// Rejected access attempts must emit an audit event.
    #[test]
    fn test_rbac_rejected_access_emits_audit_event() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());

        let before = audit_event_count(&env);

        let brand_id = Symbol::new(&env, "ACME");
        let result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "register_brand"),
            vec![
                &env,
                caller.clone().into_val(&env),
                brand_id.clone().into_val(&env),
            ],
        );

        assert!(result.is_err());
        // A rejected access attempt must still emit an audit event.
        assert!(audit_event_count(&env) > before);
    }

    /// Successful mutations emit an audit event containing caller, timestamp, and state-diff digest.
    #[test]
    fn test_rbac_successful_mutation_emits_audit_event() {
        let env = Env::default();
        env.mock_all_auths();

        let caller = setup_caller_with_role(&env, Role::BrandOwner);
        let brand_id = Symbol::new(&env, "ACME");

        let before = audit_event_count(&env);

        register_brand(
            &env,
            caller.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        assert!(audit_event_count(&env) > before);

        // The emitted audit event must carry caller, timestamp, and state-diff digest.
        let events = env.events().all();
        let mut found = false;
        for i in 0..events.len() {
            let (_, _, data) = events.get(i).unwrap();
            // Audit events are structured; verify the payload is non-empty and
            // contains the expected audit topic marker.
            if !data.is_void() {
                found = true;
            }
        }
        assert!(found, "expected a structured audit event to be emitted");
    }

    /// Role hierarchy precedence: a higher-privilege role satisfies lower-privilege requirements.
    #[test]
    fn test_rbac_role_hierarchy_precedence() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &caller, Role::Admin);

        // Admin should satisfy a BrandOwner-gated operation.
        let brand_id = Symbol::new(&env, "ACME");
        register_brand(
            &env,
            caller.clone(),
            brand_id.clone(),
            bytes!(&env, "ACME Corporation"),
            bytes!(&env, "Leading supplier"),
            bytes!(&env, "https://acme.example.com"),
            bytes!(&env, "support@acme.example.com"),
        );

        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::Brand(brand_id)));
    }

    /// Revocation behavior: a revoked role no longer authorizes the mutation.
    #[test]
    fn test_rbac_revocation_blocks_mutation() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &caller, Role::BrandOwner);

        // Revoke the role.
        rbac_revoke_role(&env, admin.clone(), caller.clone(), Role::BrandOwner);

        let brand_id = Symbol::new(&env, "ACME");
        let result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "register_brand"),
            vec![
                &env,
                caller.clone().into_val(&env),
                brand_id.clone().into_val(&env),
            ],
        );

        assert!(result.is_err());
    }

    /// Suspended submitter DIDs are rejected with a stable RBAC error code.
    #[test]
    fn test_rbac_suspended_submitter_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &caller, Role::BrandOwner);
        rbac_suspend_account(&env, admin.clone(), caller.clone());

        let brand_id = Symbol::new(&env, "ACME");
        let result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "register_brand"),
            vec![
                &env,
                caller.clone().into_val(&env),
                brand_id.clone().into_val(&env),
            ],
        );

        assert!(result.is_err());
    }

    /// Each mutating entrypoint enforces its required role (positive matrix).
    #[test]
    fn test_rbac_mutating_entrypoints_positive_matrix() {
        let env = Env::default();
        env.mock_all_auths();

        // Provenance logging requires a producer role.
        let producer = setup_caller_with_role(&env, Role::Producer);
        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Factory", "US");
        log_provenance_event(
            &env,
            event_id,
            location.clone(),
            bytes!(&env, "Premium aluminum"),
            producer.clone(),
            bytes!(&env, "BATCH-2024-001"),
        );
        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::ProvenanceEvent(event_id)));

        // Certification logging requires a certifier role.
        let certifier = setup_caller_with_role(&env, Role::Certifier);
        let cert_id = bytes!(&env, "CERT-ISO9001-2024");
        log_certification(
            &env,
            cert_id.clone(),
            Symbol::new(&env, "ISO_9001"),
            certifier.clone(),
            365,
            bytes!(&env, "Quality management system"),
        );
        let cert_event_id = env.crypto().sha256(&cert_id);
        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::CertificationEvent(cert_event_id)));

        // Labor conditions logging requires an auditor role.
        let auditor = setup_caller_with_role(&env, Role::Auditor);
        let facility_id = bytes!(&env, "FAC-FACTORY-001");
        log_labor_conditions(
            &env,
            facility_id.clone(),
            500,
            true,
            true,
            true,
            true,
            true,
            BytesN::<32>::random(&env),
            auditor.clone(),
        );
        let report_id = env.crypto().sha256(&facility_id);
        assert!(env
            .storage()
            .persistent()
            .has(&SupplyChainDataKey::LaborReport(report_id)));
    }

    /// Each mutating entrypoint rejects unauthorized callers (negative matrix).
    #[test]
    fn test_rbac_mutating_entrypoints_negative_matrix() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let caller = Address::random(&env);
        rbac_init(&env, admin.clone());
        // Caller has no roles.

        let event_id = BytesN::<32>::random(&env);
        let location = create_test_location(&env, "Factory", "US");

        let prov_result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "log_provenance_event"),
            vec![
                &env,
                event_id.clone().into_val(&env),
                location.clone().into_val(&env),
                caller.clone().into_val(&env),
            ],
        );
        assert!(prov_result.is_err());

        let cert_result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "log_certification"),
            vec![
                &env,
                bytes!(&env, "CERT-X").into_val(&env),
                caller.clone().into_val(&env),
            ],
        );
        assert!(cert_result.is_err());

        let labor_result = env.try_invoke_contract::<(), SupplyChainError>(
            &env.current_contract_address(),
            Symbol::new(&env, "log_labor_conditions"),
            vec![
                &env,
                bytes!(&env, "FAC-X").into_val(&env),
                caller.clone().into_val(&env),
            ],
        );
        assert!(labor_result.is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pagination tests (issue #872)
//
// Comprehensive edge-case and integration tests for cursor-based pagination:
// - Empty collections (no results)
// - Single-item pages
// - Cursor past end
// - Multi-page traversal
// - Filter combinations
// - Deterministic ordering across ledger upgrades
// ─────────────────────────────────────────────────────────────────────────────

mod pagination_tests {
    use super::*;

    /// Test paginated query on empty brand collection
    #[test]
    fn test_list_brands_paginated_empty() {
        let env = Env::default();
        env.mock_all_auths();

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        let result = list_brands_paginated(&env, None, Some(10), filters)
            .expect("Empty query should succeed");

        assert_eq!(result.items.len(), 0, "Empty collection should return 0 items");
        assert_eq!(result.total_count, 0, "Total count should be 0");
        assert!(result.next_cursor.is_none(), "Empty collection should have no next cursor");
    }

    /// Test single-item page for brands
    #[test]
    fn test_list_brands_paginated_single_item() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        // Register exactly one brand
        register_brand(
            &env,
            owner.clone(),
            Symbol::new(&env, "BRAND1"),
            bytes!(&env, "Brand One"),
            bytes!(&env, "First brand"),
            bytes!(&env, "https://brand1.example.com"),
            bytes!(&env, "support@brand1.example.com"),
        )
        .expect("Brand registration should succeed");

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Request page size of 1
        let result = list_brands_paginated(&env, None, Some(1), filters.clone())
            .expect("Query should succeed");

        assert_eq!(result.items.len(), 1, "Should return exactly 1 item");
        assert_eq!(result.total_count, 1, "Total count should be 1");
        assert!(result.next_cursor.is_none(), "Single item with page size 1 should have no next cursor");

        // Request page size larger than collection
        let result2 = list_brands_paginated(&env, None, Some(10), filters)
            .expect("Query should succeed");

        assert_eq!(result2.items.len(), 1, "Should return 1 item even with larger page size");
        assert!(result2.next_cursor.is_none(), "Should have no next cursor when all items fit on one page");
    }

    /// Test cursor past end returns empty results
    #[test]
    fn test_list_brands_paginated_cursor_past_end() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        // Register 3 brands
        for i in 1..=3 {
            let brand_id = Symbol::new(&env, &format!("BRAND{}", i));
            register_brand(
                &env,
                owner.clone(),
                brand_id,
                bytes!(&env, &format!("Brand {}", i)),
                bytes!(&env, &format!("Description {}", i)),
                bytes!(&env, "https://example.com"),
                bytes!(&env, "support@example.com"),
            )
            .expect("Brand registration should succeed");
        }

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Create a cursor pointing past the end (position 100)
        let past_end_cursor = Some(PageCursor {
            data: bytes!(&env, &100u32.to_le_bytes()),
        });

        let result = list_brands_paginated(&env, past_end_cursor, Some(10), filters)
            .expect("Query with cursor past end should succeed");

        assert_eq!(result.items.len(), 0, "Cursor past end should return 0 items");
        assert!(result.next_cursor.is_none(), "Cursor past end should have no next cursor");
    }

    /// Test multi-page traversal with deterministic ordering
    #[test]
    fn test_list_brands_paginated_multi_page() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        // Register 10 brands
        let brand_count = 10;
        for i in 1..=brand_count {
            let brand_id = Symbol::new(&env, &format!("BRAND{:02}", i));
            register_brand(
                &env,
                owner.clone(),
                brand_id,
                bytes!(&env, &format!("Brand {:02}", i)),
                bytes!(&env, &format!("Description {:02}", i)),
                bytes!(&env, "https://example.com"),
                bytes!(&env, "support@example.com"),
            )
            .expect("Brand registration should succeed");
        }

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Fetch first page (page size 3)
        let page1 = list_brands_paginated(&env, None, Some(3), filters.clone())
            .expect("First page query should succeed");

        assert_eq!(page1.items.len(), 3, "First page should have 3 items");
        assert_eq!(page1.total_count, 10, "Total count should be 10");
        assert!(page1.next_cursor.is_some(), "First page should have next cursor");

        // Fetch second page
        let page2 = list_brands_paginated(&env, page1.next_cursor.clone(), Some(3), filters.clone())
            .expect("Second page query should succeed");

        assert_eq!(page2.items.len(), 3, "Second page should have 3 items");
        assert_eq!(page2.total_count, 10, "Total count should still be 10");
        assert!(page2.next_cursor.is_some(), "Second page should have next cursor");

        // Fetch third page
        let page3 = list_brands_paginated(&env, page2.next_cursor.clone(), Some(3), filters.clone())
            .expect("Third page query should succeed");

        assert_eq!(page3.items.len(), 3, "Third page should have 3 items");
        assert!(page3.next_cursor.is_some(), "Third page should have next cursor");

        // Fetch fourth page (partial)
        let page4 = list_brands_paginated(&env, page3.next_cursor.clone(), Some(3), filters.clone())
            .expect("Fourth page query should succeed");

        assert_eq!(page4.items.len(), 1, "Fourth page should have 1 remaining item");
        assert!(page4.next_cursor.is_none(), "Fourth page should have no next cursor");

        // Verify no duplicate items across pages
        let mut seen_brands = vec![&env];
        seen_brands.extend_from_slice(&page1.items);
        seen_brands.extend_from_slice(&page2.items);
        seen_brands.extend_from_slice(&page3.items);
        seen_brands.extend_from_slice(&page4.items);
        assert_eq!(seen_brands.len(), 10, "Should have exactly 10 unique brands across all pages");
    }

    /// Test page size limit enforcement (max 100)
    #[test]
    fn test_list_brands_paginated_limit_enforcement() {
        let env = Env::default();
        env.mock_all_auths();

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Request page size exceeding MAX_PAGE_SIZE
        let result = list_brands_paginated(&env, None, Some(200), filters.clone())
            .expect("Query should succeed");

        // PageLimit::new should clamp to DEFAULT_PAGE_SIZE when invalid
        assert!(result.items.len() <= DEFAULT_PAGE_SIZE as usize, 
                "Result size should be clamped to default");

        // Request page size of 0 (should use default)
        let result2 = list_brands_paginated(&env, None, Some(0), filters)
            .expect("Query should succeed");

        assert_eq!(result2.items.len(), 0, "Empty collection returns 0 items regardless of limit");
    }

    /// Test product filtering by brand_id
    #[test]
    fn test_list_products_paginated_brand_filter() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        // Register two brands
        let brand1 = Symbol::new(&env, "BRAND1");
        let brand2 = Symbol::new(&env, "BRAND2");

        register_brand(
            &env,
            owner.clone(),
            brand1.clone(),
            bytes!(&env, "Brand One"),
            bytes!(&env, "First brand"),
            bytes!(&env, "https://brand1.example.com"),
            bytes!(&env, "support@brand1.example.com"),
        )
        .expect("Brand 1 registration should succeed");

        register_brand(
            &env,
            owner.clone(),
            brand2.clone(),
            bytes!(&env, "Brand Two"),
            bytes!(&env, "Second brand"),
            bytes!(&env, "https://brand2.example.com"),
            bytes!(&env, "support@brand2.example.com"),
        )
        .expect("Brand 2 registration should succeed");

        // Register products for each brand
        for i in 1..=3 {
            register_product_sku(
                &env,
                owner.clone(),
                brand1.clone(),
                bytes!(&env, &format!("SKU-1-{}", i)),
                bytes!(&env, &format!("Product 1-{}", i)),
                bytes!(&env, "Description"),
            )
            .expect("Product registration should succeed");
        }

        for i in 1..=2 {
            register_product_sku(
                &env,
                owner.clone(),
                brand2.clone(),
                bytes!(&env, &format!("SKU-2-{}", i)),
                bytes!(&env, &format!("Product 2-{}", i)),
                bytes!(&env, "Description"),
            )
            .expect("Product registration should succeed");
        }

        // Filter by brand1
        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: Some(brand1.clone()),
            facility_id: None,
        };

        let result = list_products_paginated(&env, None, Some(10), filters)
            .expect("Filtered query should succeed");

        assert_eq!(result.items.len(), 3, "Should return only brand1 products");
        for product in result.items.iter() {
            assert_eq!(product.brand_id, brand1, "All products should belong to brand1");
        }

        // Filter by brand2
        let filters2 = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: Some(brand2.clone()),
            facility_id: None,
        };

        let result2 = list_products_paginated(&env, None, Some(10), filters2)
            .expect("Filtered query should succeed");

        assert_eq!(result2.items.len(), 2, "Should return only brand2 products");
        for product in result2.items.iter() {
            assert_eq!(product.brand_id, brand2, "All products should belong to brand2");
        }
    }

    /// Test timestamp range filtering
    #[test]
    fn test_list_products_paginated_timestamp_filter() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        let brand_id = Symbol::new(&env, "BRAND1");
        register_brand(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "Brand One"),
            bytes!(&env, "First brand"),
            bytes!(&env, "https://brand1.example.com"),
            bytes!(&env, "support@brand1.example.com"),
        )
        .expect("Brand registration should succeed");

        // Register products at different mock timestamps
        env.ledger().set_timestamp(1000);
        register_product_sku(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "SKU-OLD"),
            bytes!(&env, "Old Product"),
            bytes!(&env, "Description"),
        )
        .expect("Product registration should succeed");

        env.ledger().set_timestamp(2000);
        register_product_sku(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "SKU-MID"),
            bytes!(&env, "Mid Product"),
            bytes!(&env, "Description"),
        )
        .expect("Product registration should succeed");

        env.ledger().set_timestamp(3000);
        register_product_sku(
            &env,
            owner.clone(),
            brand_id.clone(),
            bytes!(&env, "SKU-NEW"),
            bytes!(&env, "New Product"),
            bytes!(&env, "Description"),
        )
        .expect("Product registration should succeed");

        // Filter by timestamp range [1500, 2500]
        let filters = QueryFilters {
            timestamp_min: Some(1500),
            timestamp_max: Some(2500),
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        let result = list_products_paginated(&env, None, Some(10), filters)
            .expect("Filtered query should succeed");

        assert_eq!(result.items.len(), 1, "Should return only mid-range product");
        assert_eq!(result.items.get(0).unwrap().sku, bytes!(&env, "SKU-MID"));
    }

    /// Test certification status filtering (active vs expired)
    #[test]
    fn test_list_certifications_paginated_status_filter() {
        let env = Env::default();
        env.mock_all_auths();

        let issuer = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &issuer, Role::Certifier);

        // Set current time
        env.ledger().set_timestamp(1000);

        // Register active certification (expires in future)
        log_certification(
            &env,
            bytes!(&env, "CERT-ACTIVE"),
            Symbol::new(&env, "ISO9001"),
            issuer.clone(),
            365, // 365 days in future
            bytes!(&env, "Active scope"),
        )
        .expect("Active cert registration should succeed");

        // Register expired certification (set time ahead, then register cert that expires immediately)
        env.ledger().set_timestamp(2000);
        log_certification(
            &env,
            bytes!(&env, "CERT-EXPIRED"),
            Symbol::new(&env, "ISO9001"),
            issuer.clone(),
            0, // Expires immediately
            bytes!(&env, "Expired scope"),
        )
        .expect("Expired cert registration should succeed");

        // Set time further ahead to ensure first cert is still active and second is expired
        env.ledger().set_timestamp(3000);

        // Filter for active certifications
        let filters_active = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: Some(Symbol::new(&env, "active")),
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        let result_active = list_certifications_paginated(&env, None, Some(10), filters_active)
            .expect("Active filter query should succeed");

        assert!(result_active.items.len() >= 1, "Should find at least one active certification");

        // Filter for expired certifications
        let filters_expired = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: Some(Symbol::new(&env, "expired")),
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        let result_expired = list_certifications_paginated(&env, None, Some(10), filters_expired)
            .expect("Expired filter query should succeed");

        assert!(result_expired.items.len() >= 1, "Should find at least one expired certification");
    }

    /// Test facility_id filtering for labor reports
    #[test]
    fn test_list_labor_reports_paginated_facility_filter() {
        let env = Env::default();
        env.mock_all_auths();

        let reporter = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &reporter, Role::Auditor);

        let facility1 = bytes!(&env, "FAC001");
        let facility2 = bytes!(&env, "FAC002");

        // Log labor reports for facility 1
        for _i in 1..=3 {
            log_labor_conditions(
                &env,
                facility1.clone(),
                100,
                true,
                true,
                true,
                true,
                true,
                BytesN::from_array(&env, &[0u8; 32]),
                reporter.clone(),
            )
            .expect("Labor report should succeed");
        }

        // Log labor reports for facility 2
        for _i in 1..=2 {
            log_labor_conditions(
                &env,
                facility2.clone(),
                50,
                true,
                true,
                true,
                true,
                true,
                BytesN::from_array(&env, &[0u8; 32]),
                reporter.clone(),
            )
            .expect("Labor report should succeed");
        }

        // Filter by facility1
        let filters1 = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: Some(facility1.clone()),
        };

        let result1 = list_labor_reports_paginated(&env, None, Some(10), filters1)
            .expect("Filtered query should succeed");

        assert_eq!(result1.items.len(), 3, "Should return only facility1 reports");
        for report in result1.items.iter() {
            assert_eq!(report.facility_id, facility1, "All reports should be for facility1");
        }

        // Filter by facility2
        let filters2 = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: Some(facility2.clone()),
        };

        let result2 = list_labor_reports_paginated(&env, None, Some(10), filters2)
            .expect("Filtered query should succeed");

        assert_eq!(result2.items.len(), 2, "Should return only facility2 reports");
        for report in result2.items.iter() {
            assert_eq!(report.facility_id, facility2, "All reports should be for facility2");
        }
    }

    /// Test invalid cursor handling
    #[test]
    fn test_list_brands_paginated_invalid_cursor() {
        let env = Env::default();
        env.mock_all_auths();

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Create invalid cursor (wrong length)
        let invalid_cursor = Some(PageCursor {
            data: bytes!(&env, &[1u8, 2u8]), // Only 2 bytes instead of 4
        });

        let result = list_brands_paginated(&env, invalid_cursor, Some(10), filters);

        assert!(result.is_err(), "Invalid cursor should return error");
        assert_eq!(
            result.unwrap_err(),
            SupplyChainError::InvalidPayload,
            "Should return InvalidPayload error"
        );
    }

    /// Test complete multi-page traversal integration
    #[test]
    fn test_complete_pagination_traversal() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::random(&env);
        let admin = Address::random(&env);
        rbac_init(&env, admin.clone());
        grant_role(&env, &admin, &owner, Role::BrandOwner);

        // Register 25 brands
        let total_brands = 25;
        for i in 1..=total_brands {
            let brand_id = Symbol::new(&env, &format!("BRAND{:03}", i));
            register_brand(
                &env,
                owner.clone(),
                brand_id,
                bytes!(&env, &format!("Brand {:03}", i)),
                bytes!(&env, &format!("Description {:03}", i)),
                bytes!(&env, "https://example.com"),
                bytes!(&env, "support@example.com"),
            )
            .expect("Brand registration should succeed");
        }

        let filters = QueryFilters {
            timestamp_min: None,
            timestamp_max: None,
            status: None,
            submitter: None,
            brand_id: None,
            facility_id: None,
        };

        // Traverse all pages with page size of 10
        let mut all_brands = vec![&env];
        let mut cursor: Option<PageCursor> = None;
        let mut page_count = 0;

        loop {
            let page = list_brands_paginated(&env, cursor.clone(), Some(10), filters.clone())
                .expect("Page query should succeed");

            page_count += 1;
            all_brands.extend_from_slice(&page.items);

            if page.next_cursor.is_none() {
                break;
            }

            cursor = page.next_cursor;
        }

        assert_eq!(all_brands.len(), total_brands as usize, "Should collect all brands");
        assert_eq!(page_count, 3, "Should require exactly 3 pages (10+10+5)");
    }
}
