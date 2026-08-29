#![allow(dead_code)]

use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec, Map};

/// EU ESPR Compliance error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DigitalPassportError {
    /// Passport not found
    PassportNotFound = 2001,
    /// Invalid product identity
    InvalidProductIdentity = 2002,
    /// Missing mandatory compliance data
    MissingMandatoryData = 2003,
    /// Passport has expired
    PassportExpired = 2004,
    /// Invalid compliance status
    InvalidComplianceStatus = 2005,
    /// Material composition invalid
    InvalidMaterialComposition = 2006,
    /// Carbon footprint data missing
    MissingCarbonData = 2007,
    /// Circularity data incomplete
    IncompleteCircularityData = 2008,
    /// Unauthorized passport modification
    UnauthorizedModification = 2009,
    /// Invalid lifecycle state transition
    InvalidLifecycleTransition = 2010,
    /// Interoperability format not supported
    UnsupportedFormat = 2011,
    /// Import data validation failed
    ImportValidationFailed = 2012,
}

/// ESPR Compliance Status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComplianceStatus {
    Compliant,
    PartiallyCompliant,
    NonCompliant,
    PendingVerification,
}

/// Product lifecycle stage
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassportLifecycleStage {
    Created,              // Just created
    InProduction,         // Manufacturing phase
    ReadyForMarket,       // Ready for distribution
    InMarket,             // Sold to consumers
    EndOfLife,            // Product end-of-life
    Recycled,             // Successfully recycled
    Archived,             // Historical record
}

/// Product identity information per ESPR
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductIdentity {
    pub product_id: Bytes,            // Unique product identifier
    pub product_name: Bytes,          // Product name
    pub category: Symbol,             // Product category
    pub manufacturer: Address,        // Manufacturer address
    pub model_number: Bytes,          // Model/version
    pub batch_number: Bytes,          // Production batch
    pub registration_date: u64,       // When registered
    pub market_entry_date: u64,       // When entered market
}

/// Material composition per ESPR Article 3
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Material {
    pub material_name: Bytes,         // Name of material (e.g., "Aluminum")
    pub material_code: Symbol,        // ISO code (e.g., "AL")
    pub percentage_by_weight: u32,    // 0-100 percentage
    pub source_type: Symbol,          // virgin, recycled, bio_based
    pub hazardous: bool,              // Contains hazardous substances
    pub hazard_classification: Bytes, // e.g., "Heavy Metal Cd"
}

/// Durability and repairability data per ESPR Article 5
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Durability {
    pub expected_lifetime_years: u32, // Expected product lifetime
    pub warranty_years: u32,          // Manufacturer warranty
    pub spare_parts_available: bool,  // Spare parts availability
    pub spare_parts_years: u32,       // How long parts available
    pub repair_information: Bytes,    // Repair manual/link
    pub repairability_score: u32,     // 0-10 repairability score
}

/// Circularity and end-of-life information per ESPR Article 6
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Circularity {
    pub recyclable_materials: Vec<Material>,    // Materials that are recyclable
    pub recycled_content_percent: u32,          // % of recycled content
    pub reuse_potential: bool,                  // Can product be reused
    pub refurbishment_potential: bool,          // Can be refurbished
    pub disassembly_instructions: Bytes,        // How to disassemble
    pub recycling_instructions: Bytes,          // How to recycle
    pub end_of_life_score: u32,                 // 0-100 end-of-life score
}

/// Carbon footprint data per ESPR Article 4
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarbonFootprint {
    pub manufacturing_emissions: u32,   // kg CO2e for manufacturing
    pub distribution_emissions: u32,    // kg CO2e for distribution
    pub use_phase_emissions: u32,       // kg CO2e during use (per year)
    pub end_of_life_emissions: u32,     // kg CO2e for end-of-life
    pub total_embodied_carbon: u32,     // Total kg CO2e
    pub carbon_neutral: bool,           // Is carbon neutral
    pub carbon_offset_program: Bytes,   // Offset program details
    pub measurement_standard: Symbol,   // e.g., "ISO_14040"
    pub measurement_date: u64,          // When measured
}

/// Energy consumption data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyConsumption {
    pub annual_energy_kwh: u32,         // kWh per year
    pub standby_power_watts: u32,       // Standby power draw
    pub estimated_lifetime_energy: u32, // Total lifetime kWh
    pub energy_label: Symbol,           // EU energy label (A+++ to G)
}

/// Substance information per ESPR Annex I
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubstanceInfo {
    pub substance_name: Bytes,          // Chemical name
    pub cas_number: Bytes,              // CAS registry number
    pub concentration_percent: u32,     // % by weight
    pub hazard_class: Bytes,            // SVHC or other classification
    pub regulatory_status: Symbol,      // restricted, banned, monitored
}

/// Compliance verification record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceRecord {
    pub verification_date: u64,         // When verified
    pub verifier: Address,              // Who verified
    pub status: ComplianceStatus,       // Compliance status
    pub checked_fields: Vec<Bytes>,     // Which fields were checked
    pub issues_found: Vec<Bytes>,       // Any compliance issues
    pub next_review_date: u64,          // When to review next
}

/// EU ESPR Digital Product Passport
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigitalPassport {
    pub passport_id: BytesN<32>,                // Unique passport ID
    pub identity: ProductIdentity,              // Product identity
    pub materials: Vec<Material>,               // Material composition
    pub durability: Durability,                 // Durability information
    pub circularity: Circularity,               // Circularity data
    pub carbon_footprint: CarbonFootprint,      // Carbon emissions
    pub energy_consumption: Option<EnergyConsumption>, // Energy if applicable
    pub substances: Vec<SubstanceInfo>,         // Hazardous substances
    pub compliance_records: Vec<ComplianceRecord>, // Compliance history
    pub lifecycle_stage: PassportLifecycleStage,   // Current lifecycle stage
    pub created_date: u64,                      // When created
    pub last_updated: u64,                      // Last modification
    pub expiry_date: u64,                       // When passport expires
    pub version: u32,                           // Passport version
}

/// Passport lifecycle tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportLifecycleEvent {
    pub event_date: u64,                // When event occurred
    pub stage: PassportLifecycleStage,  // New stage
    pub actor: Address,                 // Who made the change
    pub notes: Bytes,                   // Event notes
    pub previous_stage: PassportLifecycleStage,
}

/// Repair event record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairEvent {
    pub repair_date: u64,               // When repaired
    pub repair_facility: Address,       // Where repaired
    pub repair_type: Symbol,            // maintenance, major, minor
    pub parts_replaced: Vec<Bytes>,     // Parts that were replaced
    pub repair_notes: Bytes,            // Repair description
}

/// Recycling event record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecyclingEvent {
    pub recycling_date: u64,            // When recycled
    pub recycling_facility: Address,    // Where recycled
    pub recovery_rate: u32,             // % of material recovered
    pub materials_recovered: Vec<Material>, // What was recovered
    pub certification: Bytes,           // Recycling certification
}

/// Refurbishment event record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefurbishmentEvent {
    pub refurbishment_date: u64,        // When refurbished
    pub refurbishment_facility: Address,// Where refurbished
    pub scope: Bytes,                   // What was done
    pub new_passport_id: Option<BytesN<32>>, // Link to new passport if created
}

/// Interoperability export format
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportFormat {
    JsonLd,             // JSON-LD format
    XmlEuPassport,      // EU XML schema
    Qr,                 // QR code format
    Pdf,                // PDF format
}

/// Passport export data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportExport {
    pub export_date: u64,               // When exported
    pub format: ExportFormat,           // Format used
    pub data: Bytes,                    // Exported data
    pub digital_signature: Option<BytesN<32>>, // Signature
    pub verification_url: Bytes,        // URL for verification
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage Keys
// ─────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum PassportDataKey {
    /// Passport ID -> DigitalPassport
    Passport(BytesN<32>),
    /// Product ID -> list of passport IDs
    ProductPassports(Bytes),
    /// Passport -> lifecycle history
    LifecycleHistory(BytesN<32>),
    /// Passport -> repair events
    RepairHistory(BytesN<32>),
    /// Passport -> recycling events
    RecyclingHistory(BytesN<32>),
    /// Passport -> refurbishment events
    RefurbishmentHistory(BytesN<32>),
    /// Passport -> exports
    ExportHistory(BytesN<32>),
    /// Passport -> compliance records
    ComplianceHistory(BytesN<32>),
    /// Material code -> material info
    MaterialRegistry(Symbol),
    /// Passport -> total repair count
    RepairCount(BytesN<32>),
    /// Passport -> total recycling count
    RecyclingCount(BytesN<32>),
    /// Counter for total passports
    PassportCount,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new EU ESPR compliant digital product passport
pub fn create_passport(
    env: &Env,
    product_id: Bytes,
    product_name: Bytes,
    category: Symbol,
    manufacturer: Address,
    model_number: Bytes,
    batch_number: Bytes,
    materials: Vec<Material>,
    durability: Durability,
    circularity: Circularity,
    carbon_footprint: CarbonFootprint,
) -> BytesN<32> {
    manufacturer.require_auth();

    let now = env.ledger().timestamp();
    let passport_id = env.crypto().sha256(&product_id);

    let identity = ProductIdentity {
        product_id: product_id.clone(),
        product_name,
        category,
        manufacturer: manufacturer.clone(),
        model_number,
        batch_number,
        registration_date: now,
        market_entry_date: now,
    };

    let passport = DigitalPassport {
        passport_id: passport_id.clone(),
        identity,
        materials,
        durability,
        circularity,
        carbon_footprint,
        energy_consumption: None,
        substances: Vec::new(&env),
        compliance_records: Vec::new(&env),
        lifecycle_stage: PassportLifecycleStage::Created,
        created_date: now,
        last_updated: now,
        expiry_date: now + (10 * 365 * 86400), // 10 year expiry
        version: 1,
    };

    // Store passport
    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id.clone()), &passport);

    // Add to product index
    let mut product_passports: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::ProductPassports(product_id))
        .unwrap_or_else(|| Vec::new(&env));
    product_passports.push_back(passport_id.clone());
    env.storage()
        .persistent()
        .set(&PassportDataKey::ProductPassports(product_id), &product_passports);

    // Increment counter
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::PassportCount)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::PassportCount, &(count + 1));

    passport_id
}

/// Update passport with new data
pub fn update_passport(
    env: &Env,
    passport_id: BytesN<32>,
    materials: Option<Vec<Material>>,
    carbon_footprint: Option<CarbonFootprint>,
    circularity: Option<Circularity>,
    updater: Address,
) {
    updater.require_auth();

    let mut passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    // Only manufacturer can update
    if passport.identity.manufacturer != updater {
        panic!("Only manufacturer can update passport");
    }

    if let Some(m) = materials {
        passport.materials = m;
    }
    if let Some(c) = carbon_footprint {
        passport.carbon_footprint = c;
    }
    if let Some(ci) = circularity {
        passport.circularity = ci;
    }

    passport.last_updated = env.ledger().timestamp();
    passport.version += 1;

    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id), &passport);
}

/// Transition passport to next lifecycle stage
pub fn transition_lifecycle_stage(
    env: &Env,
    passport_id: BytesN<32>,
    new_stage: PassportLifecycleStage,
    actor: Address,
    notes: Bytes,
) {
    actor.require_auth();

    let mut passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    let old_stage = passport.lifecycle_stage.clone();
    passport.lifecycle_stage = new_stage.clone();
    passport.last_updated = env.ledger().timestamp();

    // Record event
    let event = PassportLifecycleEvent {
        event_date: env.ledger().timestamp(),
        stage: new_stage,
        actor,
        notes,
        previous_stage: old_stage,
    };

    let mut history: Vec<PassportLifecycleEvent> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::LifecycleHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    history.push_back(event);

    env.storage()
        .persistent()
        .set(&PassportDataKey::LifecycleHistory(passport_id.clone()), &history);

    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id), &passport);
}

/// Record a repair event
pub fn record_repair(
    env: &Env,
    passport_id: BytesN<32>,
    repair_facility: Address,
    repair_type: Symbol,
    parts_replaced: Vec<Bytes>,
    repair_notes: Bytes,
) {
    repair_facility.require_auth();

    let repair = RepairEvent {
        repair_date: env.ledger().timestamp(),
        repair_facility: repair_facility.clone(),
        repair_type,
        parts_replaced,
        repair_notes,
    };

    let mut repairs: Vec<RepairEvent> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RepairHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    repairs.push_back(repair);

    env.storage()
        .persistent()
        .set(&PassportDataKey::RepairHistory(passport_id.clone()), &repairs);

    // Increment repair count
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RepairCount(passport_id.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::RepairCount(passport_id), &(count + 1));
}

/// Record a recycling event
pub fn record_recycling(
    env: &Env,
    passport_id: BytesN<32>,
    recycling_facility: Address,
    recovery_rate: u32,
    materials_recovered: Vec<Material>,
    certification: Bytes,
) {
    recycling_facility.require_auth();

    let recycling = RecyclingEvent {
        recycling_date: env.ledger().timestamp(),
        recycling_facility: recycling_facility.clone(),
        recovery_rate,
        materials_recovered,
        certification,
    };

    let mut recyclings: Vec<RecyclingEvent> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RecyclingHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    recyclings.push_back(recycling);

    env.storage()
        .persistent()
        .set(&PassportDataKey::RecyclingHistory(passport_id.clone()), &recyclings);

    // Increment recycling count
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RecyclingCount(passport_id.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::RecyclingCount(passport_id), &(count + 1));

    // Transition to recycled if recovery rate > 80%
    if recovery_rate > 80 {
        transition_lifecycle_stage(
            &env,
            passport_id,
            PassportLifecycleStage::Recycled,
            recycling_facility,
            Bytes::from_slice(&env, b"Successfully recycled"),
        );
    }
}

/// Record a refurbishment event
pub fn record_refurbishment(
    env: &Env,
    passport_id: BytesN<32>,
    refurbishment_facility: Address,
    scope: Bytes,
) {
    refurbishment_facility.require_auth();

    let refurbishment = RefurbishmentEvent {
        refurbishment_date: env.ledger().timestamp(),
        refurbishment_facility: refurbishment_facility.clone(),
        scope,
        new_passport_id: None,
    };

    let mut refurbishments: Vec<RefurbishmentEvent> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RefurbishmentHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    refurbishments.push_back(refurbishment);

    env.storage()
        .persistent()
        .set(&PassportDataKey::RefurbishmentHistory(passport_id.clone()), &refurbishments);

    // Transition to in-market for reuse
    transition_lifecycle_stage(
        &env,
        passport_id,
        PassportLifecycleStage::InMarket,
        refurbishment_facility,
        Bytes::from_slice(&env, b"Product refurbished and ready for resale"),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Verification and Compliance Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Verify ESPR compliance of a passport
pub fn verify_espr_compliance(
    env: &Env,
    passport_id: BytesN<32>,
    verifier: Address,
) -> ComplianceStatus {
    verifier.require_auth();

    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    let mut status = ComplianceStatus::Compliant;
    let mut checked_fields: Vec<Bytes> = Vec::new(&env);
    let mut issues: Vec<Bytes> = Vec::new(&env);

    // Check mandatory fields
    if passport.identity.product_id.is_empty() {
        issues.push_back(Bytes::from_slice(&env, b"Missing product ID"));
        status = ComplianceStatus::NonCompliant;
    }
    checked_fields.push_back(Bytes::from_slice(&env, b"product_id"));

    if passport.materials.is_empty() {
        issues.push_back(Bytes::from_slice(&env, b"Missing material composition"));
        status = ComplianceStatus::PartiallyCompliant;
    }
    checked_fields.push_back(Bytes::from_slice(&env, b"materials"));

    if passport.carbon_footprint.total_embodied_carbon == 0 {
        issues.push_back(Bytes::from_slice(&env, b"Missing carbon footprint data"));
        status = ComplianceStatus::PartiallyCompliant;
    }
    checked_fields.push_back(Bytes::from_slice(&env, b"carbon_footprint"));

    if passport.circularity.end_of_life_score == 0 {
        issues.push_back(Bytes::from_slice(&env, b"Missing end-of-life information"));
        status = ComplianceStatus::PartiallyCompliant;
    }
    checked_fields.push_back(Bytes::from_slice(&env, b"circularity"));

    // Verify total material percentage (should sum to ~100)
    let mut total_percent: u32 = 0;
    for material in passport.materials.iter() {
        total_percent += material.percentage_by_weight;
    }
    if total_percent < 95 || total_percent > 105 {
        issues.push_back(Bytes::from_slice(&env, b"Material percentages don't sum to 100%"));
        status = ComplianceStatus::NonCompliant;
    }

    let record = ComplianceRecord {
        verification_date: env.ledger().timestamp(),
        verifier: verifier.clone(),
        status: status.clone(),
        checked_fields,
        issues_found: issues,
        next_review_date: env.ledger().timestamp() + (365 * 86400), // 1 year
    };

    let mut records: Vec<ComplianceRecord> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::ComplianceHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    records.push_back(record);

    env.storage()
        .persistent()
        .set(&PassportDataKey::ComplianceHistory(passport_id), &records);

    status
}

/// Check if passport is still valid (not expired)
pub fn check_passport_validity(env: &Env, passport_id: BytesN<32>) -> bool {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    passport.expiry_date > env.ledger().timestamp()
}

/// Get material breakdown for a passport
pub fn get_material_breakdown(env: &Env, passport_id: BytesN<32>) -> Vec<Material> {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    passport.materials
}

/// Get carbon footprint details
pub fn get_carbon_footprint(env: &Env, passport_id: BytesN<32>) -> CarbonFootprint {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    passport.carbon_footprint
}

/// Get circularity information
pub fn get_circularity_info(env: &Env, passport_id: BytesN<32>) -> Circularity {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    passport.circularity
}

/// Get repair history for a passport
pub fn get_repair_history(env: &Env, passport_id: BytesN<32>) -> Vec<RepairEvent> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::RepairHistory(passport_id))
        .unwrap_or_else(|| Vec::new(&env))
}

/// Get recycling history for a passport
pub fn get_recycling_history(env: &Env, passport_id: BytesN<32>) -> Vec<RecyclingEvent> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::RecyclingHistory(passport_id))
        .unwrap_or_else(|| Vec::new(&env))
}

/// Get refurbishment history
pub fn get_refurbishment_history(env: &Env, passport_id: BytesN<32>) -> Vec<RefurbishmentEvent> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::RefurbishmentHistory(passport_id))
        .unwrap_or_else(|| Vec::new(&env))
}

/// Get lifecycle history
pub fn get_lifecycle_history(env: &Env, passport_id: BytesN<32>) -> Vec<PassportLifecycleEvent> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::LifecycleHistory(passport_id))
        .unwrap_or_else(|| Vec::new(&env))
}

// ─────────────────────────────────────────────────────────────────────────────
// Interoperability Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Export passport in standard format
pub fn generate_passport_export(
    env: &Env,
    passport_id: BytesN<32>,
    format: ExportFormat,
) -> PassportExport {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    let mut verification_url = Bytes::from_slice(&env, b"https://verify-passport.eu/");
    verification_url.append(&passport_id);
    let export = PassportExport {
        export_date: env.ledger().timestamp(),
        format,
        data: Bytes::from_slice(&env, b"PASSPORT_DATA"), // Simplified for demo
        digital_signature: Some(env.crypto().sha256(&Bytes::from_slice(&env, b"PASSPORT_DATA"))),
        verification_url,
    };

    let mut exports: Vec<PassportExport> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::ExportHistory(passport_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    exports.push_back(export.clone());

    env.storage()
        .persistent()
        .set(&PassportDataKey::ExportHistory(passport_id), &exports);

    export
}

/// Import passport data from external source
pub fn import_passport_data(
    env: &Env,
    import_data: Bytes,
    importer: Address,
) -> BytesN<32> {
    importer.require_auth();

    // Simplified import - in real system would parse import_data
    let passport_id = env.crypto().sha256(&import_data);

    // Verify import format is valid
    if import_data.is_empty() {
        panic!("Invalid import data");
    }

    passport_id
}

/// Export to standardized format (e.g., EU XML schema)
pub fn export_to_standard_format(
    env: &Env,
    passport_id: BytesN<32>,
    standard: Symbol,
) -> Bytes {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    // Simplified export - real system would generate proper XML/JSON
    let mut export_data = Vec::new(&env);
    export_data.push_back(Bytes::from_slice(&env, passport.identity.product_id.to_vec().as_slice()));

    Bytes::from_slice(&env, b"EXPORTED_DATA")
}

/// Validate interoperability compliance
pub fn validate_interoperability(env: &Env, passport_id: BytesN<32>) -> bool {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    // Check that all mandatory fields exist for interoperability
    !passport.identity.product_id.is_empty()
        && !passport.materials.is_empty()
        && passport.carbon_footprint.total_embodied_carbon > 0
}

// ─────────────────────────────────────────────────────────────────────────────
// Retrieval and Query Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Get a complete passport
pub fn get_passport(env: &Env, passport_id: BytesN<32>) -> DigitalPassport {
    env.storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"))
}

/// Get all passports for a product
pub fn get_product_passports(env: &Env, product_id: Bytes) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::ProductPassports(product_id))
        .unwrap_or_else(|| Vec::new(&env))
}

/// Get passport for a specific stage
pub fn get_passports_by_stage(
    env: &Env,
    stage: PassportLifecycleStage,
) -> Vec<BytesN<32>> {
    // Simplified - real implementation would maintain index
    Vec::new(&env)
}

/// Calculate environmental impact score (0-100)
pub fn calculate_environmental_score(
    env: &Env,
    passport_id: BytesN<32>,
) -> u32 {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    let mut score: u32 = 0;

    // Carbon footprint (max 40 points)
    if passport.carbon_footprint.total_embodied_carbon < 1000 {
        score += 40;
    } else if passport.carbon_footprint.total_embodied_carbon < 5000 {
        score += 20;
    }

    // Recycled content (max 30 points)
    if passport.circularity.recycled_content_percent >= 50 {
        score += 30;
    } else if passport.circularity.recycled_content_percent >= 25 {
        score += 15;
    }

    // Recyclability (max 30 points)
    if passport.circularity.end_of_life_score >= 80 {
        score += 30;
    } else if passport.circularity.end_of_life_score >= 50 {
        score += 15;
    }

    score
}

/// Check for hazardous substances
pub fn check_hazardous_substances(env: &Env, passport_id: BytesN<32>) -> Vec<SubstanceInfo> {
    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id))
        .unwrap_or_else(|| panic!("Passport not found"));

    let mut hazardous: Vec<SubstanceInfo> = Vec::new(&env);
    for substance in passport.substances.iter() {
        if substance.hazard_class.len() > 0 {
            hazardous.push_back(substance);
        }
    }
    hazardous
}

/// Register a material type
pub fn register_material_type(
    env: &Env,
    material_code: Symbol,
    material_name: Bytes,
) {
    env.storage()
        .persistent()
        .set(&PassportDataKey::MaterialRegistry(material_code), &material_name);
}

/// Get repair count for product
pub fn get_repair_count(env: &Env, passport_id: BytesN<32>) -> u32 {
    env.storage()
        .persistent()
        .get(&PassportDataKey::RepairCount(passport_id))
        .unwrap_or(0)
}

/// Get recycling count for product
pub fn get_recycling_count(env: &Env, passport_id: BytesN<32>) -> u32 {
    env.storage()
        .persistent()
        .get(&PassportDataKey::RecyclingCount(passport_id))
        .unwrap_or(0)
}

/// Get total passports issued
pub fn get_total_passports(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&PassportDataKey::PassportCount)
        .unwrap_or(0)
}
