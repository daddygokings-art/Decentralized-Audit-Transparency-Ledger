#![allow(dead_code)]

//! EU ESPR Digital Product Passport lifecycle state machine.
//!
//! This module integrates granular RBAC role gating (#885), structured
//! tamper-evident audit trail events (#885), Soroban persistent-storage TTL
//! extension and archival rent policies (#886), and cursor-based pagination
//! with filtered batch queries (#887) into every state-mutating entrypoint.

use crate::rbac::{RbacError, RbacManager, Role, RbacStorageKey};
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, xdr::ToXdr, Address, Bytes, BytesN, Env,
    Symbol, Vec,
};

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
    /// RBAC role gating rejected the caller (insufficient role) (#885)
    RbacInsufficientRole = 2013,
    /// RBAC role gating rejected the caller (suspended submitter DID) (#885)
    RbacSuspended = 2014,
}

/// Deterministic bounds to prevent unbounded storage growth and arithmetic overflow.
const MAX_MATERIALS: u32 = 64;
const MAX_SUBSTANCES: u32 = 64;
const MAX_BYTES_LEN: u32 = 1024;
const PERCENTAGE_MAX: u32 = 100;
const REPAIRABILITY_SCORE_MAX: u32 = 10;
const END_OF_LIFE_SCORE_MAX: u32 = 100;
const TEN_YEARS_SECONDS: u64 = 10 * 365 * 86400;

/// Persistent-storage TTL policy (#886): entries are re-leased only once their
/// remaining TTL drops to or below this threshold.
pub const PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS: u32 = 10_000;

/// Hot-path lease extension applied to active passport records (#886).
pub const PASSPORT_TTL_EXTEND_LEDGERS: u32 = 100_000;

/// Long-tail lease extension applied to append-only history and archival
/// records (#886).
pub const PASSPORT_ARCHIVAL_TTL_EXTEND_LEDGERS: u32 = 300_000;

/// Retention cap on the per-passport audit trail (#886 rent footprint bound).
pub const MAX_AUDIT_TRAIL_LEN: u32 = 128;

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
    pub energy_consumption: EnergyConsumption, // Energy record (zeros = not measured)
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

/// Validate a material's bounded fields; panics deterministically on malformed input.
fn validate_material(_env: &Env, material: &Material) {
    if material.percentage_by_weight > PERCENTAGE_MAX {
        panic!("material percentage exceeds 100%");
    }
    if material.material_name.len() > MAX_BYTES_LEN {
        panic!("material name too long");
    }
    if material.hazard_classification.len() > MAX_BYTES_LEN {
        panic!("hazard classification too long");
    }
}

/// Validate durability bounded fields; panics deterministically on malformed input.
fn validate_durability(_env: &Env, durability: &Durability) {
    if durability.expected_lifetime_years == 0 {
        panic!("expected lifetime must be non-zero");
    }
    if durability.warranty_years > durability.expected_lifetime_years {
        panic!("warranty exceeds expected lifetime");
    }
    if durability.repairability_score > REPAIRABILITY_SCORE_MAX {
        panic!("repairability score exceeds 10");
    }
    if durability.spare_parts_years > durability.expected_lifetime_years {
        panic!("spare parts years exceed expected lifetime");
    }
    if durability.repair_information.len() > MAX_BYTES_LEN {
        panic!("repair information too long");
    }
}

/// Validate circularity bounded fields; panics deterministically on malformed input.
fn validate_circularity(_env: &Env, circularity: &Circularity) {
    if circularity.recycled_content_percent > PERCENTAGE_MAX {
        panic!("recycled content percent exceeds 100%");
    }
    if circularity.end_of_life_score > END_OF_LIFE_SCORE_MAX {
        panic!("end-of-life score exceeds 100");
    }
    if circularity.recyclable_materials.len() > MAX_MATERIALS {
        panic!("too many recyclable materials");
    }
    if circularity.disassembly_instructions.len() > MAX_BYTES_LEN {
        panic!("disassembly instructions too long");
    }
    if circularity.recycling_instructions.len() > MAX_BYTES_LEN {
        panic!("recycling instructions too long");
    }
}

/// Validate carbon footprint bounded fields; panics deterministically on malformed input.
fn validate_carbon_footprint(_env: &Env, carbon: &CarbonFootprint) {
    if carbon.carbon_offset_program.len() > MAX_BYTES_LEN {
        panic!("carbon offset program too long");
    }
}

/// Validate product identity bounded fields; panics deterministically on malformed input.
fn validate_identity(_env: &Env, identity: &ProductIdentity) {
    if identity.product_id.is_empty() {
        panic!("product id must not be empty");
    }
    if identity.product_name.len() > MAX_BYTES_LEN {
        panic!("product name too long");
    }
    if identity.model_number.len() > MAX_BYTES_LEN {
        panic!("model number too long");
    }
    if identity.batch_number.len() > MAX_BYTES_LEN {
        panic!("batch number too long");
    }
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
    /// Passport -> tamper-evident audit trail (#885)
    PassportAuditTrail(BytesN<32>),
    /// Sequence number -> passport ID (ordered index for pagination, #887)
    PassportIndex(u32),
}

// ─────────────────────────────────────────────────────────────────────────────
// RBAC Role Gating (#885)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` when RBAC has been bootstrapped contract-wide (an admin
/// exists). Before bootstrapping, the module stays in legacy mode and relies
/// on the pre-existing per-caller `require_auth()` semantics only, so existing
/// deployments and tests keep working unchanged.
fn rbac_active(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get::<_, u32>(&RbacStorageKey::AdminCount)
        .is_some()
}

/// Enforces the minimum RBAC role for a caller when RBAC is active.
/// Returns the stable [`DigitalPassportError`] codes `RbacInsufficientRole`
/// and `RbacSuspended` so downstream integrators can present them intact.
fn ensure_role(env: &Env, actor: &Address, min_role: Role) {
    if !rbac_active(env) {
        return;
    }
    if let Err(err) = RbacManager::require_active_role_min(env, actor, min_role) {
        panic_with_error!(
            env,
            match err {
                RbacError::SubmitterSuspended => {
                    DigitalPassportError::RbacSuspended
                }
                _ => DigitalPassportError::RbacInsufficientRole,
            }
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tamper-Evident Audit Trail Events (#885)
// ─────────────────────────────────────────────────────────────────────────────

/// Kinds of audit trail events emitted by passport entrypoints.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditEventKind {
    PassportCreated,
    PassportUpdated,
    LifecycleStageChanged,
    RepairRecorded,
    RecyclingRecorded,
    RefurbishmentRecorded,
    ComplianceVerified,
    ExportGenerated,
    MaterialTypeRegistered,
    PassportArchived,
}

impl AuditEventKind {
    /// Stable tag mixed into the event hash chain so stored kinds cannot be
    /// relabeled without detection.
    pub fn tag(&self) -> &'static str {
        match self {
            AuditEventKind::PassportCreated => "passport_created",
            AuditEventKind::PassportUpdated => "passport_updated",
            AuditEventKind::LifecycleStageChanged => "lifecycle_stage_changed",
            AuditEventKind::RepairRecorded => "repair_recorded",
            AuditEventKind::RecyclingRecorded => "recycling_recorded",
            AuditEventKind::RefurbishmentRecorded => "refurbishment_recorded",
            AuditEventKind::ComplianceVerified => "compliance_verified",
            AuditEventKind::ExportGenerated => "export_generated",
            AuditEventKind::MaterialTypeRegistered => "material_type_registered",
            AuditEventKind::PassportArchived => "passport_archived",
        }
    }
}

/// Structured, hash-chained audit trail entry. Each entry commits to its
/// predecessor's `event_hash`, producing a per-passport tamper-evident chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportAuditEvent {
    pub seq: u64,
    pub kind: AuditEventKind,
    pub passport_id: BytesN<32>,
    pub actor: Address,
    pub timestamp: u64,
    /// sha256(tag || passport_id || actor || timestamp || payload)
    pub payload_hash: BytesN<32>,
    /// sha256(payload_hash || previous event hash)
    pub event_hash: BytesN<32>,
}

/// Build (persist, and publish as a structured contract event) a
/// tamper-evident audit trail entry for a passport mutation.
fn record_audit_event(
    env: &Env,
    passport_id: &BytesN<32>,
    kind: AuditEventKind,
    actor: &Address,
    payload: &Bytes,
) {
    let key = PassportDataKey::PassportAuditTrail(passport_id.clone());
    let mut trail: Vec<PassportAuditEvent> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(&env));

    let seq: u64 = trail.len() as u64;
    let prev_hash: BytesN<32> = match trail.last() {
        Some(prev) => prev.event_hash.clone(),
        None => BytesN::from_array(env, &[0u8; 32]),
    };

    let mut payload_input = Bytes::new(env);
    payload_input.append(&Bytes::from_slice(env, kind.tag().as_bytes()));
    payload_input.append(&passport_id.to_bytes());
    payload_input.append(&actor.clone().to_xdr(env));
    payload_input.append(&Bytes::from_slice(
        env,
        &env.ledger().timestamp().to_be_bytes(),
    ));
    payload_input.append(payload);

    let payload_hash = env.crypto().sha256(&payload_input).to_bytes();
    let mut chain_input = Bytes::new(env);
    chain_input.append(&payload_hash.to_bytes());
    chain_input.append(&prev_hash.to_bytes());
    let event_hash = env.crypto().sha256(&chain_input).to_bytes();

    let event = PassportAuditEvent {
        seq,
        kind,
        passport_id: passport_id.clone(),
        actor: actor.clone(),
        timestamp: env.ledger().timestamp(),
        payload_hash,
        event_hash,
    };

    trail.push_back(event.clone());
    // Rent footprint bound (#886): drop the oldest entry once the trail
    // exceeds the retention cap.
    while trail.len() > MAX_AUDIT_TRAIL_LEN {
        trail.pop_front();
    }
    env.storage().persistent().set(&key, &trail);
    bump_history_ttl(env, &key);

    // Structured event emission for off-chain indexers/audit log streams.
    env.events().publish(
        (Symbol::new(env, "passport_audit"), passport_id.clone()),
        (event, payload.clone()),
    );
}

/// Retrieve the persisted audit trail for a passport (#885).
pub fn get_audit_trail(env: &Env, passport_id: BytesN<32>) -> Vec<PassportAuditEvent> {
    env.storage()
        .persistent()
        .get(&PassportDataKey::PassportAuditTrail(passport_id))
        .unwrap_or_else(|| Vec::new(&env))
}

// ─────────────────────────────────────────────────────────────────────────────
// TTL / Archival Rent Policies (#886)
// ─────────────────────────────────────────────────────────────────────────────

/// Bump the TTL of a hot (archival-critical) storage entry if it is at or
/// below the bump threshold. `extend_ttl` is a no-op when the entry's TTL is
/// already above `threshold`, which keeps archival rent payments amortized
/// instead of paid eagerly on every access. It must use the exact same key
/// representation as the `set` call that wrote the lease.
fn bump_hot_ttl(env: &Env, passport_id: &BytesN<32>) {
    let key = PassportDataKey::Passport(passport_id.clone());
    env.storage().persistent().extend_ttl(
        &key,
        PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
        PASSPORT_TTL_EXTEND_LEDGERS,
    );
}

/// Bump the TTL of a long-tail history entry with the archival window.
fn bump_history_ttl(env: &Env, key: &PassportDataKey) {
    env.storage().persistent().extend_ttl(
        key,
        PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
        PASSPORT_ARCHIVAL_TTL_EXTEND_LEDGERS,
    );
}

/// Permissionless TTL extension hook for archival-critical passport records.
///
/// Anyone can call this to keep an active passport's persistent lease alive
/// (never unexpectedly archived); safe because it only ever *extends* TTL and
/// pays rent, mirroring [`crate::cbdc_interop::InteropManager::extend_ttl`].
pub fn extend_passport_ttl(env: &Env, passport_id: &BytesN<32>) {
    bump_hot_ttl(env, passport_id);
}

/// Archive a passport: pins the current snapshot under an archival lease.
///
/// The archival copy is written with the extended archival TTL so historical
/// records survive without being repeatedly re-bumped and re-paid on the hot
/// path. Returns the archived snapshot for client-side verification (#886).
pub fn archive_passport(
    env: &Env,
    passport_id: BytesN<32>,
    archiver: Address,
) -> DigitalPassport {
    ensure_role(env, &archiver, Role::Auditor);
    archiver.require_auth();

    let passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    env.storage().persistent().set(
        &PassportDataKey::Passport(passport_id.clone()),
        &passport,
    );
    bump_hot_ttl(env, &passport_id);
    bump_history_ttl(env, &PassportDataKey::LifecycleHistory(passport_id.clone()));
    bump_history_ttl(env, &PassportDataKey::PassportAuditTrail(passport_id.clone()));

    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::PassportArchived,
        &archiver,
        &Bytes::from_slice(env, b"archival lease extended"),
    );

    passport
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
    // #885: submitters (or higher) may mint passports once RBAC is bootstrapped.
    ensure_role(env, &manufacturer, Role::Submitter);

    let now = env.ledger().timestamp();

    // Bound and validate inputs before any state mutation.
    if materials.len() > MAX_MATERIALS {
        panic!("materials exceed max count");
    }
    for material in materials.iter() {
        validate_material(&env, &material);
    }
    validate_durability(&env, &durability);
    validate_circularity(&env, &circularity);
    validate_carbon_footprint(&env, &carbon_footprint);

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
    validate_identity(&env, &identity);

    let passport_id = env.crypto().sha256(&product_id).to_bytes();

    // Reject duplicate passports to keep counters and index consistent.
    if env.storage()
        .persistent()
        .has(&PassportDataKey::Passport(passport_id.clone()))
    {
        panic!("passport already exists");
    }

    let passport = DigitalPassport {
        passport_id: passport_id.clone(),
        identity,
        materials,
        durability,
        circularity,
        carbon_footprint,
        energy_consumption: EnergyConsumption {
            annual_energy_kwh: 0,
            standby_power_watts: 0,
            estimated_lifetime_energy: 0,
            energy_label: Symbol::new(&env, "N/A"),
        },
        substances: Vec::new(&env),
        compliance_records: Vec::new(&env),
        lifecycle_stage: PassportLifecycleStage::Created,
        created_date: now,
        last_updated: now,
        expiry_date: now.saturating_add(TEN_YEARS_SECONDS),
        version: 1,
    };

    // Store passport
    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id.clone()), &passport);
    bump_hot_ttl(env, &passport_id);

    // Add to product index
    let mut product_passports: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::ProductPassports(product_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));
    product_passports.push_back(passport_id.clone());
    env.storage()
        .persistent()
        .set(&PassportDataKey::ProductPassports(product_id.clone()), &product_passports);
    bump_history_ttl(
        env,
        &PassportDataKey::ProductPassports(product_id.clone()),
    );

    // Increment counter with saturating arithmetic to prevent overflow.
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::PassportCount)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::PassportIndex(count), &passport_id.clone());
    bump_history_ttl(env, &PassportDataKey::PassportIndex(count));

    // Increment counter
    env.storage()
        .persistent()
        .set(&PassportDataKey::PassportCount, &(count.saturating_add(1)));

    // #885: genesis entry of the per-passport tamper-evident audit chain.
    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::PassportCreated,
        &manufacturer,
        &product_id,
    );

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
    // #885: submitters (or higher) may update passports once RBAC is bootstrapped.
    ensure_role(env, &updater, Role::Submitter);

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
        if m.len() > MAX_MATERIALS {
            panic!("materials exceed max count");
        }
        for material in m.iter() {
            validate_material(&env, &material);
        }
        passport.materials = m;
    }
    if let Some(c) = carbon_footprint {
        validate_carbon_footprint(&env, &c);
        passport.carbon_footprint = c;
    }
    if let Some(ci) = circularity {
        validate_circularity(&env, &ci);
        passport.circularity = ci;
    }

    passport.last_updated = env.ledger().timestamp();
    passport.version = passport.version.saturating_add(1);

    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id.clone()), &passport);
    bump_hot_ttl(env, &passport_id);

    // #885: audit trail entry with a versioned payload fingerprint.
    let mut audit_payload = Bytes::from_slice(env, b"update_passport:");
    audit_payload.append(&Bytes::from_slice(env, &passport.version.to_be_bytes()));
    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::PassportUpdated,
        &updater,
        &audit_payload,
    );
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
    // #885: submitters (or higher) may advance the lifecycle state machine.
    ensure_role(env, &actor, Role::Submitter);

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
        stage: new_stage.clone(),
        actor: actor.clone(),
        notes: notes.clone(),
        previous_stage: old_stage.clone(),
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
    bump_history_ttl(env, &PassportDataKey::LifecycleHistory(passport_id.clone()));

    env.storage()
        .persistent()
        .set(&PassportDataKey::Passport(passport_id.clone()), &passport);
    bump_hot_ttl(env, &passport_id);

    // #885: tamper-evident audit entry keyed to the lifecycle transition.
    let mut audit_payload = Bytes::from_slice(env, b"transition:");
    audit_payload.append(&passport_id.to_bytes());
    audit_payload.append(&notes);
    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::LifecycleStageChanged,
        &actor,
        &audit_payload,
    );
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
    // #885: submitters (or higher) may record repair events.
    ensure_role(env, &repair_facility, Role::Submitter);

    if parts_replaced.len() > MAX_SUBSTANCES {
        panic!("too many replaced parts");
    }
    for part in parts_replaced.iter() {
        if part.len() > MAX_BYTES_LEN {
            panic!("part name too long");
        }
    }
    if repair_notes.len() > MAX_BYTES_LEN {
        panic!("repair notes too long");
    }

    let repair = RepairEvent {
        repair_date: env.ledger().timestamp(),
        repair_facility: repair_facility.clone(),
        repair_type,
        parts_replaced: parts_replaced.clone(),
        repair_notes: repair_notes.clone(),
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
    bump_history_ttl(env, &PassportDataKey::RepairHistory(passport_id.clone()));

    // Increment repair count with saturating arithmetic.
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RepairCount(passport_id.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::RepairCount(passport_id), &(count.saturating_add(1)));
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
    // #885: submitters (or higher) may record recycling events.
    ensure_role(env, &recycling_facility, Role::Submitter);

    if recovery_rate > PERCENTAGE_MAX {
        panic!("recovery rate exceeds 100%");
    }
    if materials_recovered.len() > MAX_MATERIALS {
        panic!("too many recovered materials");
    }
    for material in materials_recovered.iter() {
        validate_material(&env, &material);
    }
    if certification.len() > MAX_BYTES_LEN {
        panic!("certification too long");
    }

    let recycling = RecyclingEvent {
        recycling_date: env.ledger().timestamp(),
        recycling_facility: recycling_facility.clone(),
        recovery_rate,
        materials_recovered,
        certification: certification.clone(),
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
    bump_history_ttl(env, &PassportDataKey::RecyclingHistory(passport_id.clone()));

    // Increment recycling count with saturating arithmetic.
    let count: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::RecyclingCount(passport_id.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&PassportDataKey::RecyclingCount(passport_id.clone()), &(count.saturating_add(1)));

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
    // #885: submitters (or higher) may record refurbishment events.
    ensure_role(env, &refurbishment_facility, Role::Submitter);

    if scope.len() > MAX_BYTES_LEN {
        panic!("refurbishment scope too long");
    }

    let refurbishment = RefurbishmentEvent {
        refurbishment_date: env.ledger().timestamp(),
        refurbishment_facility: refurbishment_facility.clone(),
        scope: scope.clone(),
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
    bump_history_ttl(env, &PassportDataKey::RefurbishmentHistory(passport_id.clone()));

    // #885: tamper-evident audit entry for the refurbishment.
    let mut audit_payload = Bytes::from_slice(env, b"record_refurbishment:");
    audit_payload.append(&scope);
    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::RefurbishmentRecorded,
        &refurbishment_facility,
        &audit_payload,
    );

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
    // #885: auditors (or higher) may record compliance verifications.
    ensure_role(env, &verifier, Role::Auditor);

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

    // Verify total material percentage (should sum to ~100) with saturating arithmetic.
    let mut total_percent: u32 = 0;
    for material in passport.materials.iter() {
        total_percent = total_percent.saturating_add(material.percentage_by_weight);
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
        issues_found: issues.clone(),
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
        .set(&PassportDataKey::ComplianceHistory(passport_id.clone()), &records);
    bump_history_ttl(env, &PassportDataKey::ComplianceHistory(passport_id.clone()));

    // #885: tamper-evident audit entry for the compliance verdict.
    let mut audit_payload = Bytes::from_slice(env, b"verify_espr_compliance:");
    for issue in issues.iter() {
        audit_payload.append(&issue);
    }
    record_audit_event(
        env,
        &passport_id,
        AuditEventKind::ComplianceVerified,
        &verifier,
        &audit_payload,
    );

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
    let _passport: DigitalPassport = env
        .storage()
        .persistent()
        .get(&PassportDataKey::Passport(passport_id.clone()))
        .unwrap_or_else(|| panic!("Passport not found"));

    let mut verification_url = Bytes::from_slice(&env, b"https://verify-passport.eu/");
    verification_url.append(&passport_id.to_bytes());
    let export = PassportExport {
        export_date: env.ledger().timestamp(),
        format,
        data: Bytes::from_slice(&env, b"PASSPORT_DATA"), // Simplified for demo
        digital_signature: Some(env.crypto().sha256(&Bytes::from_slice(&env, b"PASSPORT_DATA")).to_bytes()),
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
        .set(&PassportDataKey::ExportHistory(passport_id.clone()), &exports);
    bump_history_ttl(env, &PassportDataKey::ExportHistory(passport_id.clone()));
    // Tip the hot passport lease so the export never outlives its source (#886).
    bump_hot_ttl(env, &passport_id);

    export
}

/// Import passport data from external source
pub fn import_passport_data(
    env: &Env,
    import_data: Bytes,
    importer: Address,
) -> BytesN<32> {
    importer.require_auth();
    // #885: submitters (or higher) may import passport data.
    ensure_role(env, &importer, Role::Submitter);

    if import_data.is_empty() || import_data.len() > MAX_BYTES_LEN {
        panic!("Invalid import data");
    }

    // Simplified import - in real system would parse import_data
    env.crypto().sha256(&import_data).to_bytes()
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
    let _ = (passport, standard);
    Bytes::from_slice(env, b"EXPORTED_DATA")
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
    // #887: stage filtering now runs through the budget-bounded, deterministic
    // scan used by the cursor-based pagination primitives.
    let mut filter = PassportFilter::new(env);
    filter.stage = Some(stage);
    get_all_passports_paged(env, None, PASSPORT_PAGE_MAX_LIMIT, Some(filter)).items
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
        .set(&PassportDataKey::MaterialRegistry(material_code.clone()), &material_name.clone());
    bump_history_ttl(env, &PassportDataKey::MaterialRegistry(material_code));
}

/// Admin-gated material registration (#885).
///
/// Legacy [`register_material_type`] predates RBAC and carries no caller
/// identity; this variant requires an authenticated
/// [`Role::Submitter`]-or-higher actor and emits an audit trail entry.
pub fn register_material_type_gated(
    env: &Env,
    caller: Address,
    material_code: Symbol,
    material_name: Bytes,
) {
    caller.require_auth();
    ensure_role(env, &caller, Role::Submitter);

    env.storage()
        .persistent()
        .set(&PassportDataKey::MaterialRegistry(material_code.clone()), &material_name.clone());
    bump_history_ttl(env, &PassportDataKey::MaterialRegistry(material_code.clone()));

    record_audit_event(
        env,
        &BytesN::from_array(env, &[0u8; 32]),
        AuditEventKind::MaterialTypeRegistered,
        &caller,
        &material_name,
    );
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

// ─────────────────────────────────────────────────────────────────────────────
// Cursor-Based Pagination and Filtered Batch Queries (#887)
// ─────────────────────────────────────────────────────────────────────────────

/// Hard per-call page size cap: bounds the number of IDs returned to the
/// caller so no single read can exhaust the Soroban CPU/memory budget.
pub const PASSPORT_PAGE_MAX_LIMIT: u32 = 100;

/// Hard per-call scan cap: bounds how many candidate entries a filtered query
/// may load and discard while searching for matches.
pub const PASSPORT_PAGE_MAX_SCAN: u32 = 500;

/// Multi-predicate filter applied during batch queries. `None` fields are
/// non-restricting pass-throughs.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportFilter {
    /// Restrict to a single lifecycle stage.
    pub stage: Option<PassportLifecycleStage>,
    /// Restrict to a single manufacturer.
    pub manufacturer: Option<Address>,
    /// Restrict to a product category symbol.
    pub category: Option<Symbol>,
    /// Require a minimum recycled-content percentage.
    pub min_recycled_content_percent: Option<u32>,
    /// Require a minimum end-of-life (circularity) score.
    pub min_end_of_life_score: Option<u32>,
}

impl PassportFilter {
    /// A permissive filter that matches every passport.
    pub fn new(_env: &Env) -> Self {
        PassportFilter {
            stage: None,
            manufacturer: None,
            category: None,
            min_recycled_content_percent: None,
            min_end_of_life_score: None,
        }
    }

    /// Applies all configured predicates to a passport.
    fn matches(&self, passport: &DigitalPassport) -> bool {
        if let Some(stage) = &self.stage {
            if &passport.lifecycle_stage != stage {
                return false;
            }
        }
        if let Some(manufacturer) = &self.manufacturer {
            if &passport.identity.manufacturer != manufacturer {
                return false;
            }
        }
        if let Some(category) = &self.category {
            if &passport.identity.category != category {
                return false;
            }
        }
        if let Some(min_recycled) = self.min_recycled_content_percent {
            if passport.circularity.recycled_content_percent < min_recycled {
                return false;
            }
        }
        if let Some(min_eol) = self.min_end_of_life_score {
            if passport.circularity.end_of_life_score < min_eol {
                return false;
            }
        }
        true
    }
}

/// Deterministic pagination cursor: the next scan offset into the ordered
/// candidate set (passport insertion index or per-product position).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportPageCursor {
    pub offset: u64,
}

/// One deterministic page of query results.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassportPage {
    /// Passport IDs matching the filter on this page.
    pub items: Vec<BytesN<32>>,
    /// Cursor to resume the next page, `None` when the scan is exhausted.
    pub next_cursor: Option<PassportPageCursor>,
    /// Number of candidates inspected while producing this page.
    pub scanned: u32,
    /// Whether additional pages remain.
    pub has_more: bool,
}

/// Deterministic, cursor-based batch query over all passports with
/// multi-predicate filtering (#887).
pub fn get_all_passports_paged(
    env: &Env,
    cursor: Option<PassportPageCursor>,
    limit: u32,
    filter: Option<PassportFilter>,
) -> PassportPage {
    let limit = limit.min(PASSPORT_PAGE_MAX_LIMIT);
    let filter = filter.unwrap_or_else(|| PassportFilter::new(env));
    let total: u32 = env
        .storage()
        .persistent()
        .get(&PassportDataKey::PassportCount)
        .unwrap_or(0);

    let start: u64 = cursor.map(|c| c.offset).unwrap_or(0);
    let mut matched: Vec<BytesN<32>> = Vec::new(env);
    let mut scanned: u32 = 0;
    let mut i: u64 = start;

    // Scan deterministically from `start`, stopping as soon as one page is
    // filled plus one lookahead match (which signals `has_more`) so no match
    // is ever discarded between cursor positions, and never exceeding the
    // per-call scan budget.
    while i < total as u64 && scanned < PASSPORT_PAGE_MAX_SCAN && matched.len() < limit + 1 {
        let id: BytesN<32> = env
            .storage()
            .persistent()
            .get(&PassportDataKey::PassportIndex(i as u32))
            .unwrap_or_else(|| panic!("Passport index missing"));
        let passport: DigitalPassport = env
            .storage()
            .persistent()
            .get(&PassportDataKey::Passport(id.clone()))
            .unwrap_or_else(|| panic!("Passport not found"));
        if filter.matches(&passport) {
            matched.push_back(id);
        }
        scanned += 1;
        i += 1;
    }
    let bump_for_read = |pid: &BytesN<32>| {
        env.storage().persistent().extend_ttl(
            &PassportDataKey::Passport(pid.clone()),
            PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
            PASSPORT_TTL_EXTEND_LEDGERS,
        );
    };
    for pid in matched.iter() {
        bump_for_read(&pid);
    }
    if scanned > 0 {
        env.storage().persistent().extend_ttl(
            &PassportDataKey::PassportIndex(start as u32),
            PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
            PASSPORT_ARCHIVAL_TTL_EXTEND_LEDGERS,
        );
    }

    let mut items = Vec::new(env);
    for j in 0..(limit.min(matched.len())) {
        items.push_back(matched.get(j).unwrap());
    }
    // When a lookahead match was consumed to signal `has_more`, resume the
    // next page AT that last scanned ordinal so no match is ever skipped.
    let more_matches = matched.len() > limit;
    let final_offset = if more_matches && i > start { i - 1 } else { i };
    let has_more = more_matches || final_offset < total as u64;
    let next_cursor = if has_more {
        Some(PassportPageCursor { offset: final_offset })
    } else {
        None
    };
    PassportPage {
        items,
        next_cursor,
        scanned,
        has_more,
    }
}

/// Deterministic, cursor-based batch query over the passports of a single
/// product with multi-predicate filtering (#887).
pub fn get_product_passports_paged(
    env: &Env,
    product_id: Bytes,
    cursor: Option<PassportPageCursor>,
    limit: u32,
    filter: Option<PassportFilter>,
) -> PassportPage {
    let limit = limit.min(PASSPORT_PAGE_MAX_LIMIT);
    let filter = filter.unwrap_or_else(|| PassportFilter::new(env));
    let candidates: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&PassportDataKey::ProductPassports(product_id.clone()))
        .unwrap_or_else(|| Vec::new(&env));

    let start: u64 = cursor.map(|c| c.offset).unwrap_or(0);
    let mut matched: Vec<BytesN<32>> = Vec::new(env);
    let mut scanned: u32 = 0;
    let mut i: u64 = start;

    // Same page-fill + scan-cap policy as [`get_all_passports_paged`].
    while i < candidates.len() as u64
        && scanned < PASSPORT_PAGE_MAX_SCAN
        && matched.len() < limit + 1
    {
        let id = candidates.get(i as u32).unwrap();
        let passport: DigitalPassport = env
            .storage()
            .persistent()
            .get(&PassportDataKey::Passport(id.clone()))
            .unwrap_or_else(|| panic!("Passport not found"));
        if filter.matches(&passport) {
            matched.push_back(id);
        }
        scanned += 1;
        i += 1;
    }
    for pid in matched.iter() {
        env.storage().persistent().extend_ttl(
            &PassportDataKey::Passport(pid.clone()),
            PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
            PASSPORT_TTL_EXTEND_LEDGERS,
        );
    }
    if scanned > 0 {
        env.storage().persistent().extend_ttl(
            &PassportDataKey::ProductPassports(product_id),
            PASSPORT_TTL_BUMP_THRESHOLD_LEDGERS,
            PASSPORT_ARCHIVAL_TTL_EXTEND_LEDGERS,
        );
    }

    let mut items = Vec::new(env);
    for j in 0..(limit.min(matched.len())) {
        items.push_back(matched.get(j).unwrap());
    }
    // Same lookahead-resume policy as [`get_all_passports_paged`].
    let more_matches = matched.len() > limit;
    let final_offset = if more_matches && i > start { i - 1 } else { i };
    let has_more = more_matches || final_offset < candidates.len() as u64;
    let next_cursor = if has_more {
        Some(PassportPageCursor { offset: final_offset })
    } else {
        None
    };
    PassportPage {
        items,
        next_cursor,
        scanned,
        has_more,
    }
}


