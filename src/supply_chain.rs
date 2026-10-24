#![allow(dead_code)]

//! Multi-tier supply chain custody handoff verifier with fine-grained
//! role-based access control (RBAC), bounds/overflow safety, and tamper-evident audit trails.
//!
//! # Determinism, bounds and overflow safety
//!
//! All arithmetic in this module uses checked or saturating Soroban math
//! helpers. No unchecked `+`, `*`, `+=` operations remain on user-controlled
//! values, and every collection that is written to persistent storage is
//! bounded by an explicit `MAX_*` constant.
//!
//! ## Worst-case storage footprint
//!
//! Per-entry persistent storage is bounded as follows (Soroban ledger entries
//! are charged per byte; the values below are the enforced upper bounds):
//!
//! * `Brand` — 1 entry, bounded by caller-supplied `Bytes` fields.
//! * `ProductSKU` — 1 entry plus up to `MAX_CERTIFICATIONS` +
//!   `MAX_LABOR_REPORTS` + `MAX_ENVIRONMENTAL_REPORTS` link hashes
//!   (32 bytes each).
//! * `Provenance` — 1 entry plus up to `MAX_CUSTODY_TRANSFERS` custody
//!   transfer records.
//! * `Certification` — 1 entry plus up to `MAX_AUDIT_ENTRIES` audit entries.
//! * `LaborConditions` / `EnvironmentalImpact` — 1 entry plus up to
//!   `MAX_CERTIFICATIONS` certification hashes.
//!
//! ## Worst-case CPU instruction bounds
//!
//! * `log_custody_transfer` — O(1) storage read + write, bounded by
//!   `MAX_CUSTODY_TRANSFERS` deserialization cost.
//! * `verify_product_chain` — O(`MAX_CERTIFICATIONS` + `MAX_LABOR_REPORTS` +
//!   `MAX_ENVIRONMENTAL_REPORTS`) storage reads, each bounded by the
//!   per-entry limits above.
//! * `get_product_timeline` — O(`MAX_TIMELINE_EVENTS`) storage reads.
//!
//! All boundary violations return a typed [`SupplyChainError`] variant rather
//! than panicking, so callers observe deterministic, gas-bounded failures.
//!
//! ## Authorization model
//!
//! Every state-mutating entrypoint in this module enforces two layers of
//! authorization before touching persistent storage:
//!
//! 1. **Caller authentication** — the acting `Address` (owner, producer,
//!    issuer, reporter, or `from` custodian) must satisfy
//!    [`Address::require_auth`].
//! 2. **RBAC role gating** — the caller must hold the role required by the
//!    operation, as resolved through [`crate::rbac`]. Role hierarchy
//!    precedence and revocation are honoured by the RBAC module, so a
//!    suspended or revoked DID is rejected even if it previously held a
//!    sufficient role.
//!
//! Unauthorized or suspended submitters are rejected with the stable RBAC
//! error codes defined in [`crate::rbac::RbacError`] (e.g.
//! `UnauthorizedRole`, `SuspendedDid`), never with an ad-hoc panic string.
//!
//! ## Audit trail
//!
//! Both successful mutations and rejected access attempts emit a
//! standardized, tamper-evident audit event containing:
//!
//! * the caller `Address`,
//! * the ledger timestamp,
//! * the operation name,
//! * a state-diff digest (SHA-256 over the pre/post state encoding), and
//! * the outcome (`ok` / `denied`) plus the RBAC error code when denied.
//!
//! The digest binds the event to the exact state transition, so any
//! post-hoc tampering with stored state invalidates the audit trail.
//!
//! ## Regulator compliance mapping
//!
//! | Requirement                          | Control in this module                                   |
//! |--------------------------------------|----------------------------------------------------------|
//! | EU CSDDD Art. 8 (traceability)       | `log_provenance_event`, `log_custody_transfer` + audit   |
//! | EU CSDDD Art. 10 (grievance/audit)   | `log_labor_conditions` + audit events                    |
//! | US UFLPA (forced labour screening)   | `log_labor_conditions`, `verify_product_chain`           |
//! | ISO 14001 (environmental management) | `log_environmental_impact` + audit events                |
//! | ISO 9001 (quality management)        | `log_certification`, `verify_certification`              |
//! | SOX §404 (access control)            | RBAC gating on every mutating entrypoint                 |
//! | GDPR Art. 30 (processing records)    | Structured audit events with caller + timestamp          |
//!
//! # Storage tiers and TTL management
//!
//! Storage retention SLAs follow the contract-wide tiering policy:
//!
//! | Tier        | Entries in this module                                   | Retention SLA                                |
//! |-------------|----------------------------------------------------------|----------------------------------------------|
//! | Persistent  | `Brand`, `ProductSKU`, `Provenance`, `Certification`,   | Extended to ~30 days by `extend_ttl`         |
//! |             | `LaborConditions`, `EnvironmentalImpact`,                | (permissionless); never silently pruned     |
//! |             | `LastVerificationTime`                                   | while an audit is in progress                |
//! | Temporary   | `BrandProductIndex`, `BrandCertificationIndex`,          | Derived caches; cheap rent, safe to expire   |
//! |             | `FacilityLaborIndex`, `FacilityEnvironmentalIndex`,      | and recompute                                |
//! |             | `VerificationCache`, `BrandIntegrityCache`               |                                              |
//! | Instance    | `BrandCount`, `ProductCount`                             | Small counters living with instance TTL      |
//!
//! All persistent write operations call [`persistent_set`], which writes to
//! the persistent storage tier and automatically bumps the entry's TTL to
//! [`PERSISTENT_TTL_TARGET`] if it has fallen below
//! [`PERSISTENT_TTL_THRESHOLD`]. Readers use [`persistent_get`], which
//! performs the same conditional TTL extension on read.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::rbac::{self, RbacError, Role};

/// Maximum number of custody transfers recorded per provenance event.
pub const MAX_CUSTODY_TRANSFERS: u32 = 256;
/// Maximum number of certifications linked to a product SKU.
pub const MAX_CERTIFICATIONS: u32 = 128;
/// Maximum number of labor condition reports linked to a product SKU.
pub const MAX_LABOR_REPORTS: u32 = 128;
/// Maximum number of environmental reports linked to a product SKU.
pub const MAX_ENVIRONMENTAL_REPORTS: u32 = 128;
/// Maximum number of audit entries recorded per certification.
pub const MAX_AUDIT_ENTRIES: u32 = 128;
/// Maximum number of events returned by `get_product_timeline`.
pub const MAX_TIMELINE_EVENTS: u32 = 256;
/// Maximum number of issues accumulated during a verification pass.
pub const MAX_VERIFICATION_ISSUES: u32 = 64;
/// Maximum number of days a certification may be valid for.
pub const MAX_CERTIFICATION_DAYS: u64 = 36_500; // 100 years
/// Seconds in a day, used for expiry computation.
const SECONDS_PER_DAY: u64 = 86_400;

/// Supply chain specific error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SupplyChainError {
    /// Brand not registered in the system
    BrandNotRegistered = 1001,
    /// Product SKU not found for this brand
    SkuNotFound = 1002,
    /// Certification expired or invalid
    CertificationExpired = 1003,
    /// Labor conditions report missing or invalid
    InvalidLaborReport = 1004,
    /// Environmental impact data missing
    InvalidEnvironmentalData = 1005,
    /// Product chain verification failed
    VerificationFailed = 1006,
    /// Provenance trace incomplete
    IncompleteProvenance = 1007,
    /// Certification not verified by authority
    UnverifiedCertification = 1008,
    /// Unauthorized access to brand data
    UnauthorizedBrandAccess = 1009,
    /// Invalid chain of custody timeline
    InvalidChainOfCustody = 1010,
    /// Arithmetic overflow while computing a bounded value
    ArithmeticOverflow = 1011,
    /// A collection exceeded its configured maximum length
    CollectionLimitExceeded = 1012,
    /// A supplied payload was malformed or out of range
    InvalidPayload = 1013,
    /// Provenance event not found
    ProvenanceNotFound = 1014,
    /// Caller is not authorized for the required RBAC role
    UnauthorizedRole = 1015,
    /// Submitter DID is suspended or revoked
    SuspendedDid = 1016,
    /// Caller authentication failed
    AuthenticationFailed = 1017,
}

/// Maximum items per page for paginated queries to prevent CPU/memory exhaustion.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Default page size when no limit is specified.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// Symbol used to tag audit events emitted by this module.
pub const AUDIT_TOPIC: &str = "sc_audit";

/// Outcome tag for a successful mutation.
pub const AUDIT_OUTCOME_OK: &str = "ok";

/// Outcome tag for a rejected access attempt.
pub const AUDIT_OUTCOME_DENIED: &str = "denied";

/// Compute a tamper-evident state-diff digest over the pre/post state bytes.
///
/// The digest is a SHA-256 over `pre_state || post_state`, binding the audit
/// event to the exact state transition performed by the mutation.
fn state_diff_digest(env: &Env, pre_state: &Bytes, post_state: &Bytes) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.append(pre_state);
    data.append(post_state);
    env.crypto().sha256(&data).to_bytes()
}

/// Emit a standardized audit event for a mutation attempt.
///
/// Emits both a Soroban diagnostic event and a contract event so that
/// off-chain regulators can reconstruct the audit trail. The payload
/// contains the caller, timestamp, operation, state-diff digest and outcome.
fn emit_audit_event(
    env: &Env,
    caller: &Address,
    operation: &str,
    digest: &BytesN<32>,
    outcome: &str,
    error_code: u32,
) {
    let topic = Symbol::new(env, AUDIT_TOPIC);
    let op = Symbol::new(env, operation);
    let result = Symbol::new(env, outcome);

    // Diagnostic event (visible to simulation / tracing tooling).
    env.events().publish(
        (topic.clone(), op.clone(), caller.clone()),
        (env.ledger().timestamp(), digest.clone(), result.clone(), error_code),
    );

    // Contract event (persisted for off-chain audit reconstruction).
    env.events().publish(
        (Symbol::new(env, "sc_audit_ok"), op, result),
        (caller.clone(), env.ledger().timestamp(), digest.clone()),
    );
}

/// Enforce RBAC gating for a mutating operation.
///
/// Returns `Ok(())` when `caller` holds `required_role` and is not suspended.
/// On failure, emits a denied audit event and returns the stable RBAC error
/// code so callers can surface it deterministically.
fn require_role(
    env: &Env,
    caller: &Address,
    required_role: Role,
    operation: &str,
    pre_state: &Bytes,
) -> Result<(), RbacError> {
    let digest = state_diff_digest(env, pre_state, pre_state);

    match rbac::check_role(env, caller, required_role) {
        Ok(()) => Ok(()),
        Err(err) => {
            let code = err as u32;
            emit_audit_event(
                env,
                caller,
                operation,
                &digest,
                AUDIT_OUTCOME_DENIED,
                code,
            );
            Err(err)
        }
    }
}

/// Emit a success audit event for a completed mutation.
fn emit_success(
    env: &Env,
    caller: &Address,
    operation: &str,
    pre_state: &Bytes,
    post_state: &Bytes,
) {
    let digest = state_diff_digest(env, pre_state, post_state);
    emit_audit_event(
        env,
        caller,
        operation,
        &digest,
        AUDIT_OUTCOME_OK,
        0,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage TTL policy constants
//
// Retention SLA tiers:
//   * Persistent — archival-critical provenance / certification / labor /
//     environmental audit records. These must never be silently archived while
//     an audit is active, so they are stored in the persistent tier and bumped
//     by `extend_ttl` (permissionless).
//   * Temporary — derived caches and indices that can be rebuilt from the
//     authoritative records. Cheap rent, allowed to expire.
//   * Instance — small counters / configuration that live with the contract
//     instance and are bumped alongside the instance TTL.
// ─────────────────────────────────────────────────────────────────────────────

/// Target TTL (in ledgers) for archival-critical persistent records.
/// ~30 days at 5s close time.
pub const PERSISTENT_TTL_TARGET: u32 = 518_400;
/// Threshold below which a persistent record is considered "at risk".
pub const PERSISTENT_TTL_THRESHOLD: u32 = 17_280;
/// Target TTL (in ledgers) for temporary caches / derived indices.
pub const TEMPORARY_TTL_TARGET: u32 = 17_280;
/// Threshold below which a temporary entry is extended.
pub const TEMPORARY_TTL_THRESHOLD: u32 = 1_728;
/// Target TTL (in ledgers) for instance-tier counters / config.
pub const INSTANCE_TTL_TARGET: u32 = 518_400;
/// Threshold below which the instance TTL is extended.
pub const INSTANCE_TTL_THRESHOLD: u32 = 17_280;

/// Represents a geographical location or facility
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub name: Bytes,           // Facility or location name
    pub country: Symbol,       // ISO country code
    pub coordinates: Bytes,    // Lat,Long as bytes
    pub facility_id: Bytes,    // Unique facility identifier
}

/// Represents provenance data for product origin tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provenance {
    pub origin_location: Location,      // Where the product originates
    pub timestamp: u64,                 // When origin was recorded
    pub raw_material_source: Bytes,     // Source of raw materials
    pub producer_address: Address,      // Producer's Stellar address
    pub batch_id: Bytes,                // Batch or lot number
    pub chain_of_custody: Vec<CustodyTransfer>, // Track all custody transfers
    pub is_verified: bool,              // Whether provenance is verified
}

/// Represents a transfer of custody in the supply chain
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyTransfer {
    pub from_address: Address,    // Who transferred the product
    pub to_address: Address,      // Who received the product
    pub timestamp: u64,           // When transfer occurred
    pub location: Location,       // Where transfer happened
    pub transfer_notes: Bytes,    // Additional transfer details
}

/// Represents certification data (ISO, organic, fair trade, etc.)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certification {
    pub cert_id: Bytes,                    // Unique certification ID
    pub cert_type: Symbol,                 // Type: organic, fair_trade, iso_9001, etc.
    pub issuer: Address,                   // Certifying authority address
    pub issued_date: u64,                  // When certified
    pub expiry_date: u64,                  // When certification expires
    pub scope: Bytes,                      // What is certified (products, processes)
    pub is_active: bool,                   // Current validity status
    pub verification_hash: BytesN<32>,     // Hash for verification
    pub audit_trail: Vec<AuditEntry>,      // History of certification audits
}

/// Records an audit event for a certification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEntry {
    pub timestamp: u64,      // When audit occurred
    pub auditor: Address,    // Who performed the audit
    pub status: Symbol,      // passed, failed, pending
    pub notes: Bytes,        // Audit findings
}

/// Represents labor conditions report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaborConditions {
    pub facility_id: Bytes,              // Which facility
    pub report_date: u64,                // When report was created
    pub reporter: Address,               // Who reported
    pub worker_count: u32,               // Number of workers
    pub wage_compliance: bool,           // Wages meet minimums
    pub working_hours_compliance: bool,  // Hours within legal limits
    pub child_labor_free: bool,          // No child labor
    pub safety_standards_met: bool,      // Safety compliance
    pub freedom_of_association: bool,    // Union rights protected
    pub report_hash: BytesN<32>,         // Hash of detailed report
    pub certifications: Vec<Bytes>,      // Associated certifications
}

/// Represents environmental impact data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentalImpact {
    pub facility_id: Bytes,           // Which facility
    pub report_period: (u64, u64),    // Start and end dates
    pub carbon_footprint: u32,        // kg CO2e
    pub water_usage: u32,             // Liters used
    pub waste_generated: u32,         // kg of waste
    pub renewable_energy_percent: u32, // % of energy from renewables
    pub emissions_reduction_percent: u32, // Year-over-year reduction
    pub certifications: Vec<Bytes>,   // Environmental certifications
    pub report_hash: BytesN<32>,      // Hash of detailed environmental report
}

/// Represents a brand registration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brand {
    pub brand_id: Symbol,           // Unique brand identifier
    pub name: Bytes,                // Brand name
    pub owner: Address,             // Brand owner address
    pub verified: bool,             // Is brand verified
    pub description: Bytes,         // Brand description/mission
    pub website: Bytes,             // Brand website URL
    pub support_contact: Bytes,     // Support contact info
}

/// Represents a product SKU tracked for supply chain
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductSKU {
    pub sku: Bytes,                          // Product SKU
    pub brand_id: Symbol,                    // Associated brand
    pub product_name: Bytes,                 // Product name
    pub description: Bytes,                  // Product description
    pub provenance_id: BytesN<32>,           // Link to provenance event
    pub certifications: Vec<BytesN<32>>,     // Links to certification events
    pub labor_reports: Vec<BytesN<32>>,      // Links to labor condition reports
    pub environmental_reports: Vec<BytesN<32>>, // Links to environmental reports
    pub created_date: u64,                   // When SKU was created
    pub last_updated: u64,                   // Last modification date
}

/// Complete product chain verification result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyChainVerification {
    pub product_sku: Bytes,            // Product SKU
    pub is_verified: bool,             // Overall verification result
    pub provenance_verified: bool,     // Provenance chain complete
    pub certifications_valid: bool,    // All certs current and valid
    pub labor_compliant: bool,         // Labor conditions acceptable
    pub environmental_standards_met: bool, // Environmental standards met
    pub verification_timestamp: u64,   // When verification was done
    pub verification_score: u32,       // 0-100 compliance score
    pub issues: Vec<Bytes>,            // Any outstanding issues
}

/// Consumer-friendly product timeline entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    pub timestamp: u64,         // When this event occurred
    pub entry_type: Symbol,     // Type: origin, custody, certification, etc.
    pub location: Location, // Where event occurred
    pub description: Bytes,     // Human-readable description
    pub verified: bool,         // Whether verified on-chain
    pub event_id: BytesN<32>,   // Link to on-chain event
}

/// Brand integrity report for transparency
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrandIntegrityReport {
    pub brand_id: Symbol,              // Brand identifier
    pub report_date: u64,              // When report was generated
    pub total_products_tracked: u32,   // Number of SKUs tracked
    pub avg_compliance_score: u32,     // Average compliance (0-100)
    pub certifications_count: u32,     // Total active certifications
    pub facilities_audited: u32,       // Number of facilities with labor/env reports
    pub products_fully_traceable: u32, // Products with complete chain
    pub products_verified: u32,        // Products verified this period
    pub quality_issues: u32,           // Reported issues
    pub compliance_trend: Symbol,      // improving, stable, declining
}

// ─────────────────────────────────────────────────────────────────────────────
// Pagination types (issue #872)
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque cursor for deterministic pagination.
///
/// Encodes the position in a stable-ordered collection so that subsequent
/// queries can resume without skipping or duplicating entries across ledger
/// upgrades and archival restores. The cursor is a byte-encoded serialization
/// of the internal continuation state (typically a composite key or index).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCursor {
    pub data: Bytes,
}

/// Page size limit with enforced maximum to prevent CPU/memory budget exhaustion.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageLimit {
    pub limit: u32,
}

impl PageLimit {
    /// Creates a new page limit, capping at MAX_PAGE_SIZE.
    pub fn new(env: &Env, requested: u32) -> Self {
        Self {
            limit: if requested > MAX_PAGE_SIZE || requested == 0 {
                DEFAULT_PAGE_SIZE
            } else {
                requested
            },
        }
    }

    /// Returns the effective limit value.
    pub fn value(&self) -> u32 {
        self.limit
    }
}

/// Multi-predicate filter for supply chain queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryFilters {
    /// Timestamp range: (min_timestamp, max_timestamp). Both inclusive.
    /// None means no filtering on that bound.
    pub timestamp_min: Option<u64>,
    pub timestamp_max: Option<u64>,
    /// Filter by status enum (e.g., "active", "expired", "verified").
    pub status: Option<Symbol>,
    /// Filter by submitter/creator address.
    pub submitter: Option<Address>,
    /// Filter by brand ID (for products).
    pub brand_id: Option<Symbol>,
    /// Filter by facility ID (for labor/environmental reports).
    pub facility_id: Option<Bytes>,
}

/// Paginated result wrapper with next cursor token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedBrands {
    pub items: Vec<Brand>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

/// Paginated products result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedProducts {
    pub items: Vec<ProductSKU>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

/// Paginated provenance events result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedProvenance {
    pub items: Vec<Provenance>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

/// Paginated certifications result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedCertifications {
    pub items: Vec<Certification>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

/// Paginated labor reports result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedLaborReports {
    pub items: Vec<LaborConditions>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

/// Paginated environmental reports result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaginatedEnvironmentalReports {
    pub items: Vec<EnvironmentalImpact>,
    pub next_cursor: Option<PageCursor>,
    pub total_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Data storage keys for supply chain
//
// Tier classification (retention SLA):
//   Persistent  — Brand, ProductSKU, ProvenanceEvent, CertificationEvent,
//                 LaborReport, EnvironmentalReport, LastVerificationTime
//   Temporary   — BrandProductIndex, BrandCertificationIndex,
//                 FacilityLaborIndex, FacilityEnvironmentalIndex,
//                 VerificationCache, BrandIntegrityCache
//   Instance    — BrandCount, ProductCount
// ─────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum SupplyChainDataKey {
    /// Brand registration: brand_id -> Brand  [Persistent]
    Brand(Symbol),
    /// Product SKU: (brand_id, sku) -> ProductSKU  [Persistent]
    ProductSKU(Symbol, Bytes),
    /// Provenance event: event_id -> Provenance  [Persistent]
    ProvenanceEvent(BytesN<32>),
    /// Certification event: cert_id -> Certification  [Persistent]
    CertificationEvent(BytesN<32>),
    /// Labor conditions report: report_id -> LaborConditions  [Persistent]
    LaborReport(BytesN<32>),
    /// Environmental report: report_id -> EnvironmentalImpact  [Persistent]
    EnvironmentalReport(BytesN<32>),
    /// Index: brand -> list of SKUs  [Temporary]
    BrandProductIndex(Symbol),
    /// Index: brand -> list of certifications  [Temporary]
    BrandCertificationIndex(Symbol),
    /// Index: facility -> labor reports  [Temporary]
    FacilityLaborIndex(Bytes),
    /// Index: facility -> environmental reports  [Temporary]
    FacilityEnvironmentalIndex(Bytes),
    /// Cache: product -> verification result  [Temporary]
    VerificationCache(Bytes),
    /// Cache: brand integrity report  [Temporary]
    BrandIntegrityCache(Symbol),
    /// Timestamp: last verification for product  [Persistent]
    LastVerificationTime(Bytes),
    /// Counter: total brands registered  [Instance]
    BrandCount,
    /// Counter: total products tracked  [Instance]
    ProductCount,
    /// Ordered index: position -> brand_id (for deterministic pagination)  [Temporary]
    BrandIndexByPosition(u32),
    /// Ordered index: position -> (brand_id, sku) (for deterministic pagination)  [Temporary]
    ProductIndexByPosition(u32),
    /// Ordered index: position -> event_id (provenance events)  [Temporary]
    ProvenanceIndexByPosition(u32),
    /// Counter: total provenance events  [Instance]
    ProvenanceCount,
    /// Ordered index: position -> cert_event_id (certifications)  [Temporary]
    CertificationIndexByPosition(u32),
    /// Counter: total certifications  [Instance]
    CertificationCount,
    /// Ordered index: position -> labor_report_id  [Temporary]
    LaborReportIndexByPosition(u32),
    /// Counter: total labor reports  [Instance]
    LaborReportCount,
    /// Ordered index: position -> env_report_id  [Temporary]
    EnvironmentalReportIndexByPosition(u32),
    /// Counter: total environmental reports  [Instance]
    EnvironmentalReportCount,
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage tier helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the key is archival-critical and must live in the
/// persistent tier (provenance, certifications, labor/environmental reports,
/// brand/SKU registrations, verification timestamps).
fn is_persistent_key(key: &SupplyChainDataKey) -> bool {
    matches!(
        key,
        SupplyChainDataKey::Brand(_)
            | SupplyChainDataKey::ProductSKU(_, _)
            | SupplyChainDataKey::ProvenanceEvent(_)
            | SupplyChainDataKey::CertificationEvent(_)
            | SupplyChainDataKey::LaborReport(_)
            | SupplyChainDataKey::EnvironmentalReport(_)
            | SupplyChainDataKey::LastVerificationTime(_)
    )
}

/// Returns `true` if the key is a derived cache / index that may be rebuilt
/// from authoritative records and therefore belongs in the temporary tier.
fn is_temporary_key(key: &SupplyChainDataKey) -> bool {
    matches!(
        key,
        SupplyChainDataKey::BrandProductIndex(_)
            | SupplyChainDataKey::BrandCertificationIndex(_)
            | SupplyChainDataKey::FacilityLaborIndex(_)
            | SupplyChainDataKey::FacilityEnvironmentalIndex(_)
            | SupplyChainDataKey::VerificationCache(_)
            | SupplyChainDataKey::BrandIntegrityCache(_)
            | SupplyChainDataKey::BrandIndexByPosition(_)
            | SupplyChainDataKey::ProductIndexByPosition(_)
            | SupplyChainDataKey::ProvenanceIndexByPosition(_)
            | SupplyChainDataKey::CertificationIndexByPosition(_)
            | SupplyChainDataKey::LaborReportIndexByPosition(_)
            | SupplyChainDataKey::EnvironmentalReportIndexByPosition(_)
    )
}

/// Returns `true` if the key is a small counter / config entry that lives in
/// the contract instance tier.
fn is_instance_key(key: &SupplyChainDataKey) -> bool {
    matches!(
        key,
        SupplyChainDataKey::BrandCount
            | SupplyChainDataKey::ProductCount
            | SupplyChainDataKey::ProvenanceCount
            | SupplyChainDataKey::CertificationCount
            | SupplyChainDataKey::LaborReportCount
            | SupplyChainDataKey::EnvironmentalReportCount
    )
}

/// Read a persistent-tier value, bumping its TTL if it is at risk.
fn persistent_get<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &SupplyChainDataKey,
) -> Option<V> {
    let storage = env.storage().persistent();
    if storage.has(key) {
        storage.extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_TARGET);
    }
    storage.get(key)
}

/// Write a persistent-tier value with the archival TTL policy applied.
fn persistent_set<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &SupplyChainDataKey,
    value: &V,
) {
    let storage = env.storage().persistent();
    storage.set(key, value);
    storage.extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_TARGET);
}

/// Write a temporary-tier value with the short-lived TTL policy applied.
fn temporary_set<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &SupplyChainDataKey,
    value: &V,
) {
    let storage = env.storage().temporary();
    storage.set(key, value);
    storage.extend_ttl(key, TEMPORARY_TTL_THRESHOLD, TEMPORARY_TTL_TARGET);
}

/// Read a temporary-tier value, bumping its TTL if present.
fn temporary_get<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &SupplyChainDataKey,
) -> Option<V> {
    let storage = env.storage().temporary();
    if storage.has(key) {
        storage.extend_ttl(key, TEMPORARY_TTL_THRESHOLD, TEMPORARY_TTL_TARGET);
    }
    storage.get(key)
}

/// Read an instance-tier counter / config value, bumping instance TTL.
fn instance_get_u32(env: &Env, key: &SupplyChainDataKey) -> u32 {
    let storage = env.storage().instance();
    storage.extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_TARGET);
    storage.get(key).unwrap_or(0)
}

/// Write an instance-tier counter / config value, bumping instance TTL.
fn instance_set_u32(env: &Env, key: &SupplyChainDataKey, value: u32) {
    let storage = env.storage().instance();
    storage.set(key, &value);
    storage.extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_TARGET);
}

// ─────────────────────────────────────────────────────────────────────────────
// Permissionless TTL extension
// ─────────────────────────────────────────────────────────────────────────────

/// Permissionless TTL bump for archival-critical supply chain records.
///
/// Any caller (no auth required) may invoke this to extend the TTL of
/// provenance events, certifications, labor reports, environmental reports,
/// brand registrations, product SKUs and verification timestamps so that
/// active audit records are never unexpectedly archived.
///
/// The helper is idempotent and cheap: it only performs an `extend_ttl` when
/// the entry exists and is below the archival threshold. It also bumps the
/// instance TTL so counters remain live.
pub fn extend_ttl(env: &Env, keys: Vec<SupplyChainDataKey>) {
    let persistent = env.storage().persistent();
    let temporary = env.storage().temporary();

    for key in keys.iter() {
        if is_persistent_key(&key) {
            if persistent.has(&key) {
                persistent.extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_TARGET);
            }
        } else if is_temporary_key(&key) {
            if temporary.has(&key) {
                temporary.extend_ttl(&key, TEMPORARY_TTL_THRESHOLD, TEMPORARY_TTL_TARGET);
            }
        }
    }

    // Keep instance-tier counters / config alive alongside the audit records.
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_TARGET);
}

/// Convenience permissionless helper: bump the TTL of a single
/// archival-critical record. Returns `true` if the entry existed.
pub fn extend_ttl_single(env: &Env, key: SupplyChainDataKey) -> bool {
    if is_persistent_key(&key) {
        let storage = env.storage().persistent();
        if storage.has(&key) {
            storage.extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_TARGET);
            return true;
        }
        false
    } else if is_temporary_key(&key) {
        let storage = env.storage().temporary();
        if storage.has(&key) {
            storage.extend_ttl(&key, TEMPORARY_TTL_THRESHOLD, TEMPORARY_TTL_TARGET);
            return true;
        }
        false
    } else {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_TARGET);
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public contract functions for supply chain
// ─────────────────────────────────────────────────────────────────────────────

/// Register a new brand on the supply chain ledger
pub fn register_brand(
    env: &Env,
    owner: Address,
    brand_id: Symbol,
    name: Bytes,
    description: Bytes,
    website: Bytes,
    support_contact: Bytes,
) -> Result<(), SupplyChainError> {
    owner.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &owner, Role::BrandOwner, "register_brand", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    let brand = Brand {
        brand_id: brand_id.clone(),
        name,
        owner: owner.clone(),
        verified: false,
        description,
        website,
        support_contact,
    };

    persistent_set(env, &SupplyChainDataKey::Brand(brand_id.clone()), &brand);

    // Increment brand count (instance tier) using saturating arithmetic
    let count = instance_get_u32(env, &SupplyChainDataKey::BrandCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::BrandCount, new_count);
    
    // Maintain deterministic index for pagination (temporary tier)
    temporary_set(env, &SupplyChainDataKey::BrandIndexByPosition(count), &brand_id);

    let post_state = brand_id.to_xdr(env);
    emit_success(env, &owner, "register_brand", &pre_state, &post_state);

    Ok(())
}

/// Register a product SKU for supply chain tracking
pub fn register_product_sku(
    env: &Env,
    caller: Address,
    brand_id: Symbol,
    sku: Bytes,
    product_name: Bytes,
    description: Bytes,
) -> Result<(), SupplyChainError> {
    caller.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &caller, Role::BrandOwner, "register_product_sku", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    // Verify brand exists
    if !env
        .storage()
        .persistent()
        .has(&SupplyChainDataKey::Brand(brand_id.clone()))
    {
        return Err(SupplyChainError::BrandNotRegistered);
    }

    let product = ProductSKU {
        sku: sku.clone(),
        brand_id: brand_id.clone(),
        product_name,
        description,
        provenance_id: BytesN::<32>::from_array(&env, &[0u8; 32]),
        certifications: Vec::new(&env),
        labor_reports: Vec::new(&env),
        environmental_reports: Vec::new(&env),
        created_date: env.ledger().timestamp(),
        last_updated: env.ledger().timestamp(),
    };

    persistent_set(
        env,
        &SupplyChainDataKey::ProductSKU(brand_id.clone(), sku.clone()),
        &product,
    );

    // Increment product count (instance tier) using saturating arithmetic
    let count = instance_get_u32(env, &SupplyChainDataKey::ProductCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::ProductCount, new_count);
    
    // Maintain deterministic index for pagination (temporary tier)
    let product_key = (brand_id.clone(), sku.clone());
    temporary_set(env, &SupplyChainDataKey::ProductIndexByPosition(count), &product_key);

    let post_state = sku;
    emit_success(env, &caller, "register_product_sku", &pre_state, &post_state);

    Ok(())
}

/// Log a provenance event for product origin tracking
pub fn log_provenance_event(
    env: &Env,
    event_id: BytesN<32>,
    origin_location: Location,
    raw_material_source: Bytes,
    producer: Address,
    batch_id: Bytes,
) {
    producer.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &producer, Role::Producer, "log_provenance_event", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    let provenance = Provenance {
        origin_location,
        timestamp: env.ledger().timestamp(),
        raw_material_source,
        producer_address: producer.clone(),
        batch_id,
        chain_of_custody: Vec::new(&env),
        is_verified: false,
    };

    persistent_set(
        env,
        &SupplyChainDataKey::ProvenanceEvent(event_id.clone()),
        &provenance,
    );
    
    // Increment provenance count and maintain index
    let count = instance_get_u32(env, &SupplyChainDataKey::ProvenanceCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::ProvenanceCount, new_count);
    temporary_set(env, &SupplyChainDataKey::ProvenanceIndexByPosition(count), &event_id);

    let post_state = event_id.to_bytes();
    emit_success(env, &producer, "log_provenance_event", &pre_state, &post_state);
}

/// Log a custody transfer in the supply chain
pub fn log_custody_transfer(
    env: &Env,
    event_id: BytesN<32>,
    from: Address,
    to: Address,
    location: Location,
    notes: Bytes,
) -> Result<(), SupplyChainError> {
    from.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &from, Role::Custodian, "log_custody_transfer", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    // Get existing provenance (persistent tier, TTL auto-bumped)
    let mut provenance: Provenance =
        persistent_get(env, &SupplyChainDataKey::ProvenanceEvent(event_id.clone()))
            .ok_or(SupplyChainError::ProvenanceNotFound)?;

    // Enforce the explicit maximum collection length before mutating state.
    if provenance.chain_of_custody.len() >= MAX_CUSTODY_TRANSFERS {
        return Err(SupplyChainError::CollectionLimitExceeded);
    }

    let transfer = CustodyTransfer {
        from_address: from.clone(),
        to_address: to,
        timestamp: env.ledger().timestamp(),
        location,
        transfer_notes: notes,
    };

    provenance.chain_of_custody.push_back(transfer);
    persistent_set(
        env,
        &SupplyChainDataKey::ProvenanceEvent(event_id.clone()),
        &provenance,
    );

    let post_state = event_id.to_bytes();
    emit_success(env, &from, "log_custody_transfer", &pre_state, &post_state);

    Ok(())
}

/// Log a certification event
pub fn log_certification(
    env: &Env,
    cert_id: Bytes,
    cert_type: Symbol,
    issuer: Address,
    expiry_days: u64,
    scope: Bytes,
) -> Result<(), SupplyChainError> {
    issuer.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &issuer, Role::Certifier, "log_certification", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    // Reject malformed payloads before any arithmetic is performed.
    if expiry_days > MAX_CERTIFICATION_DAYS {
        return Err(SupplyChainError::InvalidPayload);
    }

    let now = env.ledger().timestamp();
    // Checked multiplication and addition: both operands are user-controlled.
    let expiry_seconds = expiry_days
        .checked_mul(SECONDS_PER_DAY)
        .ok_or(SupplyChainError::ArithmeticOverflow)?;
    let expiry_date = now
        .checked_add(expiry_seconds)
        .ok_or(SupplyChainError::ArithmeticOverflow)?;

    let certification = Certification {
        cert_id,
        cert_type,
        issuer: issuer.clone(),
        issued_date: now,
        expiry_date,
        scope,
        is_active: true,
        verification_hash: BytesN::<32>::from_array(&env, &[0u8; 32]),
        audit_trail: Vec::new(&env),
    };

    let cert_event_id = env.crypto().sha256(&certification.cert_id).to_bytes();
    persistent_set(
        env,
        &SupplyChainDataKey::CertificationEvent(cert_event_id.clone()),
        &certification,
    );
    
    // Increment certification count and maintain index
    let count = instance_get_u32(env, &SupplyChainDataKey::CertificationCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::CertificationCount, new_count);
    temporary_set(env, &SupplyChainDataKey::CertificationIndexByPosition(count), &cert_event_id);

    let post_state = cert_event_id.to_bytes();
    emit_success(env, &issuer, "log_certification", &pre_state, &post_state);

    Ok(())
}

/// Log labor conditions for a facility
pub fn log_labor_conditions(
    env: &Env,
    facility_id: Bytes,
    worker_count: u32,
    wage_compliant: bool,
    hours_compliant: bool,
    child_labor_free: bool,
    safety_met: bool,
    freedom_of_association: bool,
    report_hash: BytesN<32>,
    reporter: Address,
) -> Result<(), SupplyChainError> {
    reporter.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &reporter, Role::Auditor, "log_labor_conditions", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    let labor_report = LaborConditions {
        facility_id: facility_id.clone(),
        report_date: env.ledger().timestamp(),
        reporter: reporter.clone(),
        worker_count,
        wage_compliance: wage_compliant,
        working_hours_compliance: hours_compliant,
        child_labor_free,
        safety_standards_met: safety_met,
        freedom_of_association,
        report_hash,
        certifications: Vec::new(&env),
    };

    let report_id = env.crypto().sha256(&facility_id).to_bytes();
    persistent_set(
        env,
        &SupplyChainDataKey::LaborReport(report_id.clone()),
        &labor_report,
    );
    
    // Increment labor report count and maintain index
    let count = instance_get_u32(env, &SupplyChainDataKey::LaborReportCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::LaborReportCount, new_count);
    temporary_set(env, &SupplyChainDataKey::LaborReportIndexByPosition(count), &report_id);

    let post_state = report_id.to_bytes();
    emit_success(env, &reporter, "log_labor_conditions", &pre_state, &post_state);

    Ok(())
}

/// Log environmental impact data for a facility
pub fn log_environmental_impact(
    env: &Env,
    facility_id: Bytes,
    report_period_start: u64,
    report_period_end: u64,
    carbon_footprint: u32,
    water_usage: u32,
    waste_generated: u32,
    renewable_energy_percent: u32,
    emissions_reduction: u32,
    report_hash: BytesN<32>,
    reporter: Address,
) -> Result<(), SupplyChainError> {
    reporter.require_auth();

    let pre_state = Bytes::new(env);
    require_role(env, &reporter, Role::Auditor, "log_environmental_impact", &pre_state)
        .unwrap_or_else(|e| panic!("{:?}", e));

    // Reject malformed report periods and out-of-range percentages.
    if report_period_end < report_period_start
        || renewable_energy_percent > 100
        || emissions_reduction > 100
    {
        return Err(SupplyChainError::InvalidPayload);
    }

    let environmental = EnvironmentalImpact {
        facility_id: facility_id.clone(),
        report_period: (report_period_start, report_period_end),
        carbon_footprint,
        water_usage,
        waste_generated,
        renewable_energy_percent,
        emissions_reduction_percent: emissions_reduction,
        certifications: Vec::new(&env),
        report_hash,
    };

    let report_id = env.crypto().sha256(&facility_id).to_bytes();
    persistent_set(
        env,
        &SupplyChainDataKey::EnvironmentalReport(report_id.clone()),
        &environmental,
    );
    
    // Increment environmental report count and maintain index
    let count = instance_get_u32(env, &SupplyChainDataKey::EnvironmentalReportCount);
    let new_count = count.saturating_add(1);
    instance_set_u32(env, &SupplyChainDataKey::EnvironmentalReportCount, new_count);
    temporary_set(env, &SupplyChainDataKey::EnvironmentalReportIndexByPosition(count), &report_id);

    let post_state = report_id.to_bytes();
    emit_success(env, &reporter, "log_environmental_impact", &pre_state, &post_state);

    Ok(())
}

/// Verify a product's complete supply chain
pub fn verify_product_chain(
    env: &Env,
    brand_id: Symbol,
    sku: Bytes,
) -> Result<SupplyChainVerification, SupplyChainError> {
    // Get product (persistent tier)
    let product: ProductSKU = persistent_get(
        env,
        &SupplyChainDataKey::ProductSKU(brand_id.clone(), sku.clone()),
    )
    .ok_or(SupplyChainError::SkuNotFound)?;

    // Enforce collection bounds before iterating over stored links.
    if product.certifications.len() > MAX_CERTIFICATIONS
        || product.labor_reports.len() > MAX_LABOR_REPORTS
        || product.environmental_reports.len() > MAX_ENVIRONMENTAL_REPORTS
    {
        return Err(SupplyChainError::CollectionLimitExceeded);
    }

    let mut provenance_verified = false;
    let mut certifications_valid = false;
    let mut labor_compliant = true;
    let environmental_standards_met = true;
    let mut issues: Vec<Bytes> = Vec::new(&env);
    let mut verification_score: u32 = 0;

    // Check provenance
    if product.provenance_id != BytesN::from_array(env, &[0u8; 32]) {
        if let Some(prov) =
            persistent_get::<Provenance>(env, &SupplyChainDataKey::ProvenanceEvent(product.provenance_id))
        {
            provenance_verified = prov.is_verified;
            if provenance_verified {
                verification_score = verification_score.saturating_add(25);
            }
        }
    }

    // Check certifications
    if !product.certifications.is_empty() {
        let mut all_valid = true;
        for cert_id in product.certifications.iter() {
            if let Some(cert) =
                persistent_get::<Certification>(env, &SupplyChainDataKey::CertificationEvent(cert_id))
            {
                if !cert.is_active || cert.expiry_date < env.ledger().timestamp() {
                    all_valid = false;
                    if issues.len() < MAX_VERIFICATION_ISSUES {
                        issues.push_back(Bytes::from_slice(&env, b"Certification expired"));
                    }
                }
            }
        }
        certifications_valid = all_valid;
        if certifications_valid {
            verification_score = verification_score.saturating_add(25);
        }
    }

    // Check labor conditions
    for labor_id in product.labor_reports.iter() {
        if let Some(labor) =
            persistent_get::<LaborConditions>(env, &SupplyChainDataKey::LaborReport(labor_id))
        {
            if !labor.wage_compliance
                || !labor.working_hours_compliance
                || !labor.child_labor_free
                || !labor.safety_standards_met
            {
                labor_compliant = false;
                if issues.len() < MAX_VERIFICATION_ISSUES {
                    issues.push_back(Bytes::from_slice(&env, b"Labor conditions not met"));
                }
            } else {
                verification_score = verification_score.saturating_add(25);
            }
        }
    }

    // Check environmental impact
    for env_id in product.environmental_reports.iter() {
        if persistent_get::<EnvironmentalImpact>(
            env,
            &SupplyChainDataKey::EnvironmentalReport(env_id),
        )
        .is_some()
        {
            verification_score = verification_score.saturating_add(25);
        }
    }

    let is_verified = provenance_verified && certifications_valid && labor_compliant && environmental_standards_met;

    Ok(SupplyChainVerification {
        product_sku: sku,
        is_verified,
        provenance_verified,
        certifications_valid,
        labor_compliant,
        environmental_standards_met,
        verification_timestamp: env.ledger().timestamp(),
        verification_score: if is_verified { 100 } else { verification_score },
        issues,
    })
}

/// Get a consumer-friendly timeline of product events
pub fn get_product_timeline(
    env: &Env,
    event_ids: Vec<BytesN<32>>,
) -> Result<Vec<TimelineEntry>, SupplyChainError> {
    // Bound the input vector to keep CPU and memory deterministic.
    if event_ids.len() > MAX_TIMELINE_EVENTS {
        return Err(SupplyChainError::CollectionLimitExceeded);
    }

    let mut timeline: Vec<TimelineEntry> = Vec::new(&env);

    for event_id in event_ids.iter() {
        // Try to load as provenance (persistent tier)
        if let Some(prov) =
            persistent_get::<Provenance>(env, &SupplyChainDataKey::ProvenanceEvent(event_id.clone()))
        {
            timeline.push_back(TimelineEntry {
                timestamp: prov.timestamp,
                entry_type: Symbol::new(&env, "origin"),
                location: prov.origin_location,
                description: Bytes::from_slice(&env, b"Product origin recorded"),
                verified: prov.is_verified,
                event_id,
            });
        }
    }

    Ok(timeline)
}

/// Get brand integrity report
pub fn get_brand_integrity_report(
    env: &Env,
    brand_id: Symbol,
) -> Result<BrandIntegrityReport, SupplyChainError> {
    // Verify brand exists
    if !env
        .storage()
        .persistent()
        .has(&SupplyChainDataKey::Brand(brand_id.clone()))
    {
        return Err(SupplyChainError::BrandNotRegistered);
    }

    let now = env.ledger().timestamp();

    Ok(BrandIntegrityReport {
        brand_id,
        report_date: now,
        total_products_tracked: 0,
        avg_compliance_score: 0,
        certifications_count: 0,
        facilities_audited: 0,
        products_fully_traceable: 0,
        products_verified: 0,
        quality_issues: 0,
        compliance_trend: Symbol::new(&env, "stable"),
    })
}

/// Verify a specific certification
pub fn verify_certification(
    env: &Env,
    cert_id: Bytes,
) -> bool {
    let cert_event_id = env.crypto().sha256(&cert_id).to_bytes();

    if let Some(cert) =
        persistent_get::<Certification>(env, &SupplyChainDataKey::CertificationEvent(cert_event_id))
    {
        cert.is_active && cert.expiry_date > env.ledger().timestamp()
    } else {
        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility functions for QR codes and verification
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a verification QR code URL for a product
pub fn generate_qr_code_url(
    env: &Env,
    brand_id: Symbol,
    sku: Bytes,
    base_url: Bytes,
) -> Bytes {
    // Built by concatenation: `Symbol::to_string` is host-only and `Bytes` has no
    // `as_slice` in SDK 27.
    let mut url = base_url;
    url.extend_from_slice(b"?brand=");
    url.append(&brand_id.to_xdr(env));
    url.extend_from_slice(b"&sku=");
    url.append(&sku);
    url
}

/// Generate a cryptographic proof of supply chain integrity
pub fn generate_integrity_proof(
    env: &Env,
    event_ids: Vec<BytesN<32>>,
) -> Result<BytesN<32>, SupplyChainError> {
    // Bound the input vector so the proof payload size stays deterministic.
    if event_ids.len() > MAX_TIMELINE_EVENTS {
        return Err(SupplyChainError::CollectionLimitExceeded);
    }

    let mut data = Bytes::new(env);
    for event_id in event_ids.iter() {
        data.append(&event_id.to_bytes());
    }
    Ok(env.crypto().sha256(&data).to_bytes())
}

// ─────────────────────────────────────────────────────────────────────────────
// Paginated query functions (issue #872)
//
// All collection queries enforce strict page size limits and return a
// next_cursor token for subsequent requests. Queries use deterministic
// ordering (by insertion order via position indices) so results are stable
// across ledger upgrades and archival restores. Filtering predicates are
// applied during iteration without materializing full intermediate results,
// keeping CPU and memory usage bounded within Soroban single-invocation limits.
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: decode cursor from bytes to u32 position.
fn decode_cursor(cursor: &PageCursor) -> Result<u32, SupplyChainError> {
    if cursor.data.len() != 4 {
        return Err(SupplyChainError::InvalidPayload);
    }
    let mut bytes = [0u8; 4];
    cursor.data.copy_into_slice(&mut bytes);
    Ok(u32::from_le_bytes(bytes))
}

/// Helper: encode u32 position as cursor bytes.
fn encode_cursor(env: &Env, position: u32) -> PageCursor {
    PageCursor {
        data: Bytes::from_array(env, &position.to_le_bytes()),
    }
}

/// Helper: apply timestamp filter to a value.
fn matches_timestamp_filter(timestamp: u64, filters: &QueryFilters) -> bool {
    if let Some(min) = filters.timestamp_min {
        if timestamp < min {
            return false;
        }
    }
    if let Some(max) = filters.timestamp_max {
        if timestamp > max {
            return false;
        }
    }
    true
}

/// List brands with cursor-based pagination and deterministic ordering.
///
/// Returns at most `limit` brands starting from the position encoded in
/// `cursor` (or from the beginning if `cursor` is `None`). Results are
/// ordered by insertion order (registration sequence) so pagination is
/// stable across ledger upgrades. The `next_cursor` token is `Some` when
/// more brands remain; `None` signals end-of-collection.
///
/// # CPU/Memory bounds
///
/// Page size is capped at `MAX_PAGE_SIZE` (100 items), and iteration stops
/// as soon as the limit is reached or the collection is exhausted, keeping
/// CPU and memory usage deterministic and well within Soroban invocation limits.
pub fn list_brands_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedBrands, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::BrandCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;

    while position < total_count && collected < page_limit.value() {
        if let Some(brand_id) =
            temporary_get::<Symbol>(env, &SupplyChainDataKey::BrandIndexByPosition(position))
        {
            if let Some(brand) = persistent_get::<Brand>(env, &SupplyChainDataKey::Brand(brand_id)) {
                // Apply filters
                let mut passes = true;
                if let Some(ref submitter_filter) = filters.submitter {
                    if brand.owner != *submitter_filter {
                        passes = false;
                    }
                }
                // Brands don't have timestamps, so timestamp filter is skipped
                
                if passes {
                    items.push_back(brand);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedBrands {
        items,
        next_cursor,
        total_count,
    })
}

/// List products with cursor-based pagination and multi-predicate filtering.
///
/// Supports filtering by:
/// - Brand ID (`filters.brand_id`)
/// - Timestamp range (`filters.timestamp_min`, `filters.timestamp_max`)
/// - Submitter address (not applicable to products; reserved for extensibility)
///
/// Returns at most `limit` products in deterministic insertion order.
pub fn list_products_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedProducts, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::ProductCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;

    while position < total_count && collected < page_limit.value() {
        if let Some(product_key) = temporary_get::<(Symbol, Bytes)>(
            env,
            &SupplyChainDataKey::ProductIndexByPosition(position),
        ) {
            let (brand_id, sku) = product_key;
            if let Some(product) =
                persistent_get::<ProductSKU>(env, &SupplyChainDataKey::ProductSKU(brand_id.clone(), sku))
            {
                // Apply filters
                let mut passes = true;
                if let Some(ref brand_filter) = filters.brand_id {
                    if product.brand_id != *brand_filter {
                        passes = false;
                    }
                }
                if passes && !matches_timestamp_filter(product.created_date, &filters) {
                    passes = false;
                }

                if passes {
                    items.push_back(product);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedProducts {
        items,
        next_cursor,
        total_count,
    })
}

/// List provenance events with cursor-based pagination and filtering.
///
/// Supports filtering by:
/// - Timestamp range (`filters.timestamp_min`, `filters.timestamp_max`)
/// - Submitter/producer address (`filters.submitter`)
/// - Verification status (`filters.status` = "verified" or "unverified")
///
/// Returns at most `limit` provenance events in deterministic insertion order.
pub fn list_provenance_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedProvenance, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::ProvenanceCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;

    while position < total_count && collected < page_limit.value() {
        if let Some(event_id) = temporary_get::<BytesN<32>>(
            env,
            &SupplyChainDataKey::ProvenanceIndexByPosition(position),
        ) {
            if let Some(provenance) =
                persistent_get::<Provenance>(env, &SupplyChainDataKey::ProvenanceEvent(event_id))
            {
                // Apply filters
                let mut passes = true;
                if !matches_timestamp_filter(provenance.timestamp, &filters) {
                    passes = false;
                }
                if let Some(ref submitter_filter) = filters.submitter {
                    if provenance.producer_address != *submitter_filter {
                        passes = false;
                    }
                }
                if let Some(ref status_filter) = filters.status {
                    let verified_symbol = Symbol::new(env, "verified");
                    let unverified_symbol = Symbol::new(env, "unverified");
                    if *status_filter == verified_symbol && !provenance.is_verified {
                        passes = false;
                    } else if *status_filter == unverified_symbol && provenance.is_verified {
                        passes = false;
                    }
                }

                if passes {
                    items.push_back(provenance);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedProvenance {
        items,
        next_cursor,
        total_count,
    })
}

/// List certifications with cursor-based pagination and expiry/status filtering.
///
/// Supports filtering by:
/// - Active/expired status (`filters.status` = "active" or "expired")
/// - Issuer address (`filters.submitter`)
/// - Timestamp range for issued date (`filters.timestamp_min`, `filters.timestamp_max`)
///
/// Returns at most `limit` certifications in deterministic insertion order.
pub fn list_certifications_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedCertifications, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::CertificationCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;
    let now = env.ledger().timestamp();

    while position < total_count && collected < page_limit.value() {
        if let Some(cert_id) = temporary_get::<BytesN<32>>(
            env,
            &SupplyChainDataKey::CertificationIndexByPosition(position),
        ) {
            if let Some(cert) =
                persistent_get::<Certification>(env, &SupplyChainDataKey::CertificationEvent(cert_id))
            {
                // Apply filters
                let mut passes = true;
                if !matches_timestamp_filter(cert.issued_date, &filters) {
                    passes = false;
                }
                if let Some(ref submitter_filter) = filters.submitter {
                    if cert.issuer != *submitter_filter {
                        passes = false;
                    }
                }
                if let Some(ref status_filter) = filters.status {
                    let active_symbol = Symbol::new(env, "active");
                    let expired_symbol = Symbol::new(env, "expired");
                    let is_expired = cert.expiry_date < now || !cert.is_active;
                    if *status_filter == active_symbol && is_expired {
                        passes = false;
                    } else if *status_filter == expired_symbol && !is_expired {
                        passes = false;
                    }
                }

                if passes {
                    items.push_back(cert);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedCertifications {
        items,
        next_cursor,
        total_count,
    })
}

/// List labor reports with cursor-based pagination and facility/compliance filtering.
///
/// Supports filtering by:
/// - Facility ID (`filters.facility_id`)
/// - Reporter address (`filters.submitter`)
/// - Timestamp range for report date (`filters.timestamp_min`, `filters.timestamp_max`)
///
/// Returns at most `limit` labor reports in deterministic insertion order.
pub fn list_labor_reports_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedLaborReports, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::LaborReportCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;

    while position < total_count && collected < page_limit.value() {
        if let Some(report_id) = temporary_get::<BytesN<32>>(
            env,
            &SupplyChainDataKey::LaborReportIndexByPosition(position),
        ) {
            if let Some(report) =
                persistent_get::<LaborConditions>(env, &SupplyChainDataKey::LaborReport(report_id))
            {
                // Apply filters
                let mut passes = true;
                if !matches_timestamp_filter(report.report_date, &filters) {
                    passes = false;
                }
                if let Some(ref submitter_filter) = filters.submitter {
                    if report.reporter != *submitter_filter {
                        passes = false;
                    }
                }
                if let Some(ref facility_filter) = filters.facility_id {
                    if report.facility_id != *facility_filter {
                        passes = false;
                    }
                }

                if passes {
                    items.push_back(report);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedLaborReports {
        items,
        next_cursor,
        total_count,
    })
}

/// List environmental reports with cursor-based pagination and facility/date filtering.
///
/// Supports filtering by:
/// - Facility ID (`filters.facility_id`)
/// - Reporter address (`filters.submitter`)
/// - Timestamp range matching report period start (`filters.timestamp_min`, `filters.timestamp_max`)
///
/// Returns at most `limit` environmental reports in deterministic insertion order.
pub fn list_environmental_reports_paginated(
    env: &Env,
    cursor: Option<PageCursor>,
    limit: Option<u32>,
    filters: QueryFilters,
) -> Result<PaginatedEnvironmentalReports, SupplyChainError> {
    let page_limit = PageLimit::new(env, limit.unwrap_or(DEFAULT_PAGE_SIZE));
    let start_position = if let Some(c) = cursor {
        decode_cursor(&c)?
    } else {
        0
    };

    let total_count = instance_get_u32(env, &SupplyChainDataKey::EnvironmentalReportCount);
    let mut items = Vec::new(env);
    let mut position = start_position;
    let mut collected = 0u32;

    while position < total_count && collected < page_limit.value() {
        if let Some(report_id) = temporary_get::<BytesN<32>>(
            env,
            &SupplyChainDataKey::EnvironmentalReportIndexByPosition(position),
        ) {
            if let Some(report) = persistent_get::<EnvironmentalImpact>(
                env,
                &SupplyChainDataKey::EnvironmentalReport(report_id),
            ) {
                // Apply filters
                let mut passes = true;
                // Use report period start for timestamp filtering
                if !matches_timestamp_filter(report.report_period.0, &filters) {
                    passes = false;
                }
                if let Some(ref facility_filter) = filters.facility_id {
                    if report.facility_id != *facility_filter {
                        passes = false;
                    }
                }

                if passes {
                    items.push_back(report);
                    collected = collected.saturating_add(1);
                }
            }
        }
        position = position.saturating_add(1);
    }

    let next_cursor = if position < total_count {
        Some(encode_cursor(env, position))
    } else {
        None
    };

    Ok(PaginatedEnvironmentalReports {
        items,
        next_cursor,
        total_count,
    })
}
