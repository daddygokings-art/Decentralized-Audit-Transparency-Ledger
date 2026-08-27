/// # Trade Compliance Automation Module
///
/// Comprehensive trade compliance automation framework implementing HS (Harmonized System)
/// classification, origin determination (ROO), FTA qualification, customs valuation,
/// license management, customs broker integration, and AEO (Authorized Economic Operator)
/// certification support for streamlined cross-border trade.
///
/// ## Standards & Frameworks
/// - **HS Code** — Harmonized System commodity classification (6-12 digits)
/// - **ROO** — Rules of Origin (originating vs. non-originating)
/// - **FTA** — Free Trade Agreements (USMCA, CPTPP, RCEP, EU)
/// - **Customs Valuation** — WTO Agreement on Customs Valuation
/// - **AEO** — Authorized Economic Operator (WCO C-TPAT equivalent)

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TradeComplianceError {
    /// HS code not found or invalid
    HSCodeNotFound = 4000,
    /// Origin determination failed
    OriginDeterminationFailed = 4001,
    /// Product does not qualify for FTA
    FTAQualificationFailed = 4002,
    /// Customs valuation method invalid
    CustomsValuationError = 4003,
    /// License required but not present
    LicenseRequired = 4004,
    /// Broker not authorized
    BrokerNotAuthorized = 4005,
    /// AEO certification expired or invalid
    AEOCertificationInvalid = 4006,
    /// Certificate of origin required
    CertificateOfOriginRequired = 4007,
    /// Rules of origin not satisfied
    RulesOfOriginNotSatisfied = 4008,
    /// Preference margin insufficient
    PreferenceMarginInsufficient = 4009,
    /// Tariff classification in dispute
    TariffClassificationDisputed = 4010,
    /// Duty calculation error
    DutyCalculationError = 4011,
    /// Broker license expired
    BrokerLicenseExpired = 4012,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// HS Code classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HSCodeClassification {
    pub id: BytesN<32>,
    pub hs_code: Bytes,           // e.g., "8471.30.00" (computers)
    pub product_description: Bytes,
    pub product_category: Bytes,  // e.g., "Electronics", "Chemicals"
    pub unit_of_measure: Bytes,   // e.g., "KG", "L", "UNIT"
    pub base_duty_rate: u32,      // in basis points (e.g., 500 = 5%)
    pub note: Bytes,              // Additional classification notes
    pub created_at: u64,
    pub code_hash: BytesN<32>,
}

/// Origin determination
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginDetermination {
    pub id: BytesN<32>,
    pub shipment_id: BytesN<32>,
    pub product: Bytes,
    pub hs_code: Bytes,
    pub country_of_origin: Bytes,  // ISO 3166-1 alpha-2
    pub origin_type: u32,           // 0=fully_originating, 1=cumulation, 2=non_originating
    pub value_content: u32,         // Regional value content %
    pub determined_at: u64,
    pub determined_by: Address,
    pub origin_hash: BytesN<32>,
}

/// FTA qualification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FTAQualification {
    pub id: BytesN<32>,
    pub shipment_id: BytesN<32>,
    pub fta_name: Bytes,            // e.g., "USMCA", "CPTPP"
    pub exporter_country: Bytes,
    pub importer_country: Bytes,
    pub product_hs_code: Bytes,
    pub qualifies: bool,
    pub preference_margin: u32,     // Duty savings in basis points
    pub certificate_of_origin_required: bool,
    pub roo_satisfied: bool,        // Rules of Origin met
    pub qualified_at: u64,
    pub qualification_hash: BytesN<32>,
}

/// Customs valuation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomsValuation {
    pub id: BytesN<32>,
    pub shipment_id: BytesN<32>,
    pub invoice_price: u64,        // in cents
    pub currency: Bytes,           // e.g., "USD"
    pub valuation_method: u32,     // 1=transaction, 2=identical, 3=similar, 4=deductive, 5=computed
    pub adjustments: i64,          // freight, insurance, etc. (positive/negative)
    pub dutiable_value: u64,       // Final customs value
    pub valuation_date: u64,
    pub valuation_hash: BytesN<32>,
}

/// Trade license
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeLicense {
    pub id: BytesN<32>,
    pub license_number: Bytes,
    pub holder: Address,
    pub product_categories: Vec<Bytes>,
    pub countries_authorized: Vec<Bytes>,
    pub issued_date: u64,
    pub expiration_date: u64,
    pub status: u32,               // 0=active, 1=suspended, 2=revoked
    pub license_hash: BytesN<32>,
}

/// Customs broker profile
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomsBroker {
    pub id: BytesN<32>,
    pub broker_address: Address,
    pub broker_name: Bytes,
    pub license_number: Bytes,
    pub countries_authorized: Vec<Bytes>,
    pub license_issued: u64,
    pub license_expiration: u64,
    pub aeo_certified: bool,
    pub is_active: bool,
    pub broker_hash: BytesN<32>,
}

/// AEO Certification (WCO standard)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AEOCertification {
    pub id: BytesN<32>,
    pub entity: Address,
    pub entity_name: Bytes,
    pub certification_type: Bytes,  // e.g., "C-TPAT", "EORI", "AEO_F"
    pub security_level: u32,        // 1=basic, 2=standard, 3=enhanced
    pub certified_date: u64,
    pub expiration_date: u64,
    pub compliance_record: Bytes,
    pub audit_history: Vec<Bytes>,
    pub status: u32,                // 0=active, 1=suspended, 2=revoked
    pub certification_hash: BytesN<32>,
}

/// Certificate of Origin (CoO)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificateOfOrigin {
    pub id: BytesN<32>,
    pub shipment_id: BytesN<32>,
    pub exporter: Address,
    pub importer: Address,
    pub product_hs_code: Bytes,
    pub country_of_origin: Bytes,
    pub fta_name: Bytes,
    pub issued_date: u64,
    pub issued_by: Address,
    pub certification_number: Bytes,
    pub coo_hash: BytesN<32>,
}

/// Duty calculation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DutyCalculation {
    pub id: BytesN<32>,
    pub shipment_id: BytesN<32>,
    pub dutiable_value: u64,
    pub base_duty_rate: u32,       // basis points
    pub calculated_duty: u64,      // in cents
    pub fta_duty_rate: u32,        // preferential rate if FTA applies
    pub fta_duty: u64,             // duty under FTA
    pub duty_savings: u64,         // difference
    pub calculation_date: u64,
    pub calculation_hash: BytesN<32>,
}

// ── Data Keys ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum TradeComplianceKey {
    Owner,
    HSCodeClassification(BytesN<32>),
    HSCodeByCode(Bytes),
    OriginDetermination(BytesN<32>),
    OriginByShipment(BytesN<32>),
    FTAQualification(BytesN<32>),
    FTAByShipment(BytesN<32>),
    CustomsValuation(BytesN<32>),
    ValuationByShipment(BytesN<32>),
    TradeLicense(BytesN<32>),
    LicenseByHolder(Address),
    CustomsBroker(BytesN<32>),
    BrokerByAddress(Address),
    AEOCertification(BytesN<32>),
    AEOByEntity(Address),
    CertificateOfOrigin(BytesN<32>),
    CoOByShipment(BytesN<32>),
    DutyCalculation(BytesN<32>),
    DutyByShipment(BytesN<32>),
    HSCodeCount,
    LicenseCount,
    BrokerCount,
    AEOCount,
    TradeCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct TradeCompliance;

#[contractimpl]
impl TradeCompliance {
    /// Initialize trade compliance module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&TradeComplianceKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::HSCodeCount, &0u32);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::LicenseCount, &0u32);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::BrokerCount, &0u32);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::AEOCount, &0u32);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::TradeCount, &0u32);
    }

    // ── HS Code Management ───────────────────────────────────────────────

    pub fn register_hs_code(
        env: Env,
        caller: Address,
        hs_code: Bytes,
        description: Bytes,
        category: Bytes,
        unit: Bytes,
        duty_rate: u32,
        note: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let code_id = env.crypto().sha256(&hs_code).into();
        let now = env.ledger().timestamp();

        let classification = HSCodeClassification {
            id: code_id.clone(),
            hs_code: hs_code.clone(),
            product_description: description,
            product_category: category,
            unit_of_measure: unit,
            base_duty_rate: duty_rate,
            note,
            created_at: now,
            code_hash: env.crypto().sha256(&hs_code).into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::HSCodeClassification(code_id.clone()), &classification);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::HSCodeByCode(hs_code.clone()), &code_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::HSCodeCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::HSCodeCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "hs_code_registered"),),
            (code_id.clone(), hs_code, duty_rate),
        );

        code_id
    }

    pub fn get_hs_code(env: Env, code_id: BytesN<32>) -> HSCodeClassification {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::HSCodeClassification(code_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::HSCodeNotFound))
    }

    // ── Origin Determination ─────────────────────────────────────────────

    pub fn determine_origin(
        env: Env,
        caller: Address,
        shipment_id: BytesN<32>,
        product: Bytes,
        hs_code: Bytes,
        country_of_origin: Bytes,
        origin_type: u32,
        value_content: u32,
    ) -> BytesN<32> {
        caller.require_auth();

        let origin_id = Self::compute_origin_id(&env, &shipment_id, &country_of_origin);
        let now = env.ledger().timestamp();

        let determination = OriginDetermination {
            id: origin_id.clone(),
            shipment_id,
            product,
            hs_code,
            country_of_origin: country_of_origin.clone(),
            origin_type,
            value_content,
            determined_at: now,
            determined_by: caller.clone(),
            origin_hash: env
                .crypto()
                .sha256(&Self::pack_origin_data(&env, &country_of_origin, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::OriginDetermination(origin_id.clone()), &determination);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::OriginByShipment(shipment_id), &origin_id);

        env.events().publish(
            (Symbol::new(&env, "origin_determined"),),
            (origin_id.clone(), shipment_id, country_of_origin),
        );

        origin_id
    }

    pub fn get_origin(env: Env, origin_id: BytesN<32>) -> OriginDetermination {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::OriginDetermination(origin_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::OriginDeterminationFailed))
    }

    // ── FTA Qualification ────────────────────────────────────────────────

    pub fn qualify_for_fta(
        env: Env,
        caller: Address,
        shipment_id: BytesN<32>,
        fta_name: Bytes,
        exporter_country: Bytes,
        importer_country: Bytes,
        hs_code: Bytes,
        qualifies: bool,
        roo_satisfied: bool,
    ) -> BytesN<32> {
        caller.require_auth();

        let fta_id = Self::compute_fta_id(&env, &shipment_id, &fta_name);
        let now = env.ledger().timestamp();

        // Calculate preference margin (duty savings)
        let preference_margin = if qualifies { 200u32 } else { 0u32 }; // 2% savings example

        let qualification = FTAQualification {
            id: fta_id.clone(),
            shipment_id,
            fta_name: fta_name.clone(),
            exporter_country,
            importer_country,
            product_hs_code: hs_code,
            qualifies,
            preference_margin,
            certificate_of_origin_required: qualifies,
            roo_satisfied,
            qualified_at: now,
            qualification_hash: env
                .crypto()
                .sha256(&Self::pack_fta_data(&env, &fta_name, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::FTAQualification(fta_id.clone()), &qualification);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::FTAByShipment(shipment_id), &fta_id);

        if !qualifies {
            panic_with_error!(&env, TradeComplianceError::FTAQualificationFailed);
        }

        env.events().publish(
            (Symbol::new(&env, "fta_qualified"),),
            (fta_id.clone(), shipment_id, preference_margin),
        );

        fta_id
    }

    pub fn get_fta_qualification(env: Env, fta_id: BytesN<32>) -> FTAQualification {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::FTAQualification(fta_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::FTAQualificationFailed))
    }

    // ── Customs Valuation ────────────────────────────────────────────────

    pub fn valuate_for_customs(
        env: Env,
        caller: Address,
        shipment_id: BytesN<32>,
        invoice_price: u64,
        currency: Bytes,
        valuation_method: u32,
        adjustments: i64,
    ) -> BytesN<32> {
        caller.require_auth();

        let valuation_id = Self::compute_valuation_id(&env, &shipment_id);
        let now = env.ledger().timestamp();

        let dutiable_value = if adjustments >= 0 {
            invoice_price + (adjustments as u64)
        } else {
            invoice_price - ((-adjustments) as u64)
        };

        let valuation = CustomsValuation {
            id: valuation_id.clone(),
            shipment_id,
            invoice_price,
            currency,
            valuation_method,
            adjustments,
            dutiable_value,
            valuation_date: now,
            valuation_hash: env
                .crypto()
                .sha256(&Self::pack_valuation_data(&env, invoice_price, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::CustomsValuation(valuation_id.clone()), &valuation);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::ValuationByShipment(shipment_id), &valuation_id);

        env.events().publish(
            (Symbol::new(&env, "customs_valuation_completed"),),
            (valuation_id.clone(), shipment_id, dutiable_value),
        );

        valuation_id
    }

    pub fn get_valuation(env: Env, valuation_id: BytesN<32>) -> CustomsValuation {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::CustomsValuation(valuation_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::CustomsValuationError))
    }

    // ── Trade License Management ─────────────────────────────────────────

    pub fn issue_trade_license(
        env: Env,
        caller: Address,
        license_number: Bytes,
        holder: Address,
        product_categories: Vec<Bytes>,
        countries: Vec<Bytes>,
        validity_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let license_id = env.crypto().sha256(&license_number).into();
        let now = env.ledger().timestamp();

        let license = TradeLicense {
            id: license_id.clone(),
            license_number,
            holder: holder.clone(),
            product_categories,
            countries_authorized: countries,
            issued_date: now,
            expiration_date: now + (validity_days as u64 * 86400),
            status: 0, // active
            license_hash: env.crypto().sha256(&license_number).into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::TradeLicense(license_id.clone()), &license);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::LicenseByHolder(holder.clone()), &license_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::LicenseCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::LicenseCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "trade_license_issued"),),
            (license_id.clone(), holder),
        );

        license_id
    }

    pub fn get_license(env: Env, license_id: BytesN<32>) -> TradeLicense {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::TradeLicense(license_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::LicenseRequired))
    }

    // ── Customs Broker Management ────────────────────────────────────────

    pub fn register_customs_broker(
        env: Env,
        caller: Address,
        broker_address: Address,
        broker_name: Bytes,
        license_number: Bytes,
        countries: Vec<Bytes>,
        validity_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let broker_id = env.crypto().sha256(&license_number).into();
        let now = env.ledger().timestamp();

        let broker = CustomsBroker {
            id: broker_id.clone(),
            broker_address: broker_address.clone(),
            broker_name,
            license_number,
            countries_authorized: countries,
            license_issued: now,
            license_expiration: now + (validity_days as u64 * 86400),
            aeo_certified: false,
            is_active: true,
            broker_hash: env.crypto().sha256(&broker_address.to_string().to_bytes()).into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::CustomsBroker(broker_id.clone()), &broker);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::BrokerByAddress(broker_address.clone()), &broker_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::BrokerCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::BrokerCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "broker_registered"),),
            (broker_id.clone(), broker_address),
        );

        broker_id
    }

    pub fn get_broker(env: Env, broker_id: BytesN<32>) -> CustomsBroker {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::CustomsBroker(broker_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::BrokerNotAuthorized))
    }

    // ── AEO Certification ────────────────────────────────────────────────

    pub fn certify_aeo(
        env: Env,
        caller: Address,
        entity: Address,
        entity_name: Bytes,
        cert_type: Bytes,
        security_level: u32,
        validity_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let aeo_id = Self::compute_aeo_id(&env, &entity);
        let now = env.ledger().timestamp();

        let certification = AEOCertification {
            id: aeo_id.clone(),
            entity: entity.clone(),
            entity_name,
            certification_type: cert_type,
            security_level,
            certified_date: now,
            expiration_date: now + (validity_days as u64 * 86400),
            compliance_record: Bytes::new(&env),
            audit_history: Vec::new(&env),
            status: 0, // active
            certification_hash: env.crypto().sha256(&entity.to_string().to_bytes()).into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::AEOCertification(aeo_id.clone()), &certification);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::AEOByEntity(entity.clone()), &aeo_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::AEOCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::AEOCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "aeo_certified"),),
            (aeo_id.clone(), entity, security_level),
        );

        aeo_id
    }

    pub fn get_aeo_certification(env: Env, aeo_id: BytesN<32>) -> AEOCertification {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::AEOCertification(aeo_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::AEOCertificationInvalid))
    }

    // ── Certificate of Origin ────────────────────────────────────────────

    pub fn issue_certificate_of_origin(
        env: Env,
        caller: Address,
        shipment_id: BytesN<32>,
        importer: Address,
        hs_code: Bytes,
        country_of_origin: Bytes,
        fta_name: Bytes,
        cert_number: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();

        let coo_id = Self::compute_coo_id(&env, &shipment_id, &cert_number);
        let now = env.ledger().timestamp();

        let coo = CertificateOfOrigin {
            id: coo_id.clone(),
            shipment_id,
            exporter: caller.clone(),
            importer,
            product_hs_code: hs_code,
            country_of_origin,
            fta_name,
            issued_date: now,
            issued_by: caller.clone(),
            certification_number: cert_number,
            coo_hash: env
                .crypto()
                .sha256(&Self::pack_coo_data(&env, &shipment_id, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::CertificateOfOrigin(coo_id.clone()), &coo);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::CoOByShipment(shipment_id), &coo_id);

        env.events().publish(
            (Symbol::new(&env, "certificate_of_origin_issued"),),
            (coo_id.clone(), shipment_id),
        );

        coo_id
    }

    pub fn get_certificate_of_origin(env: Env, coo_id: BytesN<32>) -> CertificateOfOrigin {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::CertificateOfOrigin(coo_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::CertificateOfOriginRequired))
    }

    // ── Duty Calculation ─────────────────────────────────────────────────

    pub fn calculate_duty(
        env: Env,
        caller: Address,
        shipment_id: BytesN<32>,
        dutiable_value: u64,
        base_duty_rate: u32,
        fta_duty_rate: u32,
    ) -> BytesN<32> {
        caller.require_auth();

        let duty_id = Self::compute_duty_id(&env, &shipment_id);
        let now = env.ledger().timestamp();

        let calculated_duty = (dutiable_value * (base_duty_rate as u64)) / 10000u64;
        let fta_duty = (dutiable_value * (fta_duty_rate as u64)) / 10000u64;
        let duty_savings = if calculated_duty > fta_duty {
            calculated_duty - fta_duty
        } else {
            0u64
        };

        let calculation = DutyCalculation {
            id: duty_id.clone(),
            shipment_id,
            dutiable_value,
            base_duty_rate,
            calculated_duty,
            fta_duty_rate,
            fta_duty,
            duty_savings,
            calculation_date: now,
            calculation_hash: env
                .crypto()
                .sha256(&Self::pack_duty_data(&env, dutiable_value, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TradeComplianceKey::DutyCalculation(duty_id.clone()), &calculation);
        env.storage()
            .instance()
            .set(&TradeComplianceKey::DutyByShipment(shipment_id), &duty_id);

        env.events().publish(
            (Symbol::new(&env, "duty_calculated"),),
            (duty_id.clone(), shipment_id, calculated_duty),
        );

        duty_id
    }

    pub fn get_duty_calculation(env: Env, duty_id: BytesN<32>) -> DutyCalculation {
        env.storage()
            .instance()
            .get(&TradeComplianceKey::DutyCalculation(duty_id))
            .unwrap_or_else(|| panic_with_error!(&env, TradeComplianceError::DutyCalculationError))
    }

    // ── Statistics ───────────────────────────────────────────────────────

    pub fn get_trade_compliance_stats(env: Env) -> (u32, u32, u32, u32, u32) {
        let hs_codes: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::HSCodeCount)
            .unwrap_or(0);
        let licenses: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::LicenseCount)
            .unwrap_or(0);
        let brokers: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::BrokerCount)
            .unwrap_or(0);
        let aeos: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::AEOCount)
            .unwrap_or(0);
        let trades: u32 = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::TradeCount)
            .unwrap_or(0);

        (hs_codes, licenses, brokers, aeos, trades)
    }

    // ── Private Helpers ──────────────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&TradeComplianceKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, TradeComplianceError::BrokerNotAuthorized);
        }
    }

    fn compute_origin_id(env: &Env, shipment_id: &BytesN<32>, country: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(country);
        env.crypto().sha256(&preimage).into()
    }

    fn compute_fta_id(env: &Env, shipment_id: &BytesN<32>, fta_name: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(fta_name);
        env.crypto().sha256(&preimage).into()
    }

    fn compute_valuation_id(env: &Env, shipment_id: &BytesN<32>) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_aeo_id(env: &Env, entity: &Address) -> BytesN<32> {
        env.crypto().sha256(&entity.to_string().to_bytes()).into()
    }

    fn compute_coo_id(env: &Env, shipment_id: &BytesN<32>, cert_number: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(cert_number);
        env.crypto().sha256(&preimage).into()
    }

    fn compute_duty_id(env: &Env, shipment_id: &BytesN<32>) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn pack_origin_data(env: &Env, country: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(country);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_fta_data(env: &Env, fta_name: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(fta_name);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_valuation_data(env: &Env, value: u64, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&Self::u64_to_bytes(env, value));
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_coo_data(env: &Env, shipment_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&shipment_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_duty_data(env: &Env, value: u64, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&Self::u64_to_bytes(env, value));
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn u64_to_bytes(env: &Env, v: u64) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
                ((v >> 32) & 0xff) as u8,
                ((v >> 40) & 0xff) as u8,
                ((v >> 48) & 0xff) as u8,
                ((v >> 56) & 0xff) as u8,
            ]
        )
    }
}

#[cfg(test)]
mod tests;
