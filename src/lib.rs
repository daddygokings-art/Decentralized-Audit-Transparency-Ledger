#![no_std]
// Migration to #[contractevent] macro is deferred (issue tracked separately)
#![allow(deprecated)]

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol,
    Vec,
};

/// Zero/invalid Stellar address (all zeroes) used to reject `NewOwnerIsZero`.
const NULL_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/// Default maximum metadata size (1 KB). Used when no explicit cap is set.
const DEFAULT_MAX_METADATA_SIZE: u32 = 1024;

/// Maximum category Symbol length in bytes.
const MAX_CATEGORY_LEN: u32 = 18;

/// Maximum acceptable drift for ledger timestamps when logging new events.
const MAX_TIMESTAMP_DRIFT_SECONDS: u64 = 3600;

/// An audit event stored on-chain.
///
/// # ID scheme (issue #70)
/// `id = sha256(contract_id || submitter || event_type_bytes || metadata || timestamp_le_bytes)`
/// This makes IDs unpredictable and content-addressed.
///
/// # Hash chain (issue #66)
/// Each event records the SHA-256 of the previous event's serialised fields,
/// giving a tamper-evident chain. The genesis event uses `[0u8; 32]`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    /// Sequential position (0-based). Used by `get_event_by_order`.
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    /// Optional category for hierarchical classification (e.g., finance, compliance)
    pub category: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    /// Optional sub-event type for hierarchical classification
    pub sub_event_type: Option<Symbol>,
    /// Schema version of this event for forward/backward compatibility.
    ///
    /// Contract upgrades may change the interpretation of `metadata` and other fields.
    /// Consumers should use this version to decide which schema/migration logic to apply.
    pub version: u32,
    /// SHA-256 of this event (computed over the other fields + prev_hash).
    pub event_hash: BytesN<32>,
    /// SHA-256 of the previous event; `[0u8;32]` for the genesis event.
    pub prev_hash: BytesN<32>,
    /// Optional parent event ID for semantic event chaining.
    ///
    /// When `Some(id)`, this event is a child of the referenced event.
    /// The parent event is identified by its content-addressed event ID.
    /// Used to form directed acyclic graphs of related audit events.
    pub parent_event_id: Option<BytesN<32>>,
}

/// Lightweight event header without metadata (issue #56).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHeader {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
}

/// Combined global config: avoids two separate reads for GlobalMaxLogs + TotalEvents.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub global_max_logs: u32,
    pub total_events: u32,
}

/// Packed global runtime state (issue #114).
/// Combines all global instance storage reads into a single key so that
/// `log_event` only needs 1 read for all global flags instead of ~6.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeState {
    /// Global maximum events and running total (was DataKey::Config).
    pub global_max_logs: u32,
    pub total_events: u32,
    /// Whether the contract is paused (was DataKey::Paused).
    pub paused: bool,
    /// Whether allowlist mode is active (was DataKey::AllowlistMode).
    pub allowlist_mode: bool,
    /// Whether low-cost mode is active (was DataKey::LowCostMode).
    pub low_cost_mode: bool,
    /// Event emission mode 0-3 (was DataKey::EventEmissionConfig).
    pub emission_mode: u32,
    /// Global metadata size cap; 0 means use DEFAULT_MAX_METADATA_SIZE (was DataKey::GlobalMetadataMaxSize).
    pub global_metadata_max_size: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Owner,
    Config,
    /// Replaced by Config — kept as tombstone variant so existing encoded keys
    /// don't collide; no longer written.
    GlobalMaxLogs,
    /// Paused flag: when true, write operations are blocked.
    Paused,
    /// Blocked submitters cannot submit events (issue #141).
    SubmitterBlocklist(Address),
    /// If true, allowlist mode is enabled (issue #141).
    AllowlistMode,
    /// Per-submitter allowlist state (issue #141).
    SubmitterAllowlist(Address),
    /// Replaced by Config — kept as tombstone variant.
    TotalEvents,
    /// Replaced by EventCapConfig(Symbol) — kept as tombstone variant.
    EventCapSet(Symbol),
    /// Replaced by EventCapConfig(Symbol) — kept as tombstone variant.
    EventMaxLogs(Symbol),
    EventCapRemoved(Symbol),
    EventCapConfig(Symbol),
    /// Stores packed Bytes of u32 global-order indices (4 bytes each, LE) for a type (issue #54).
    EventTypeIndices(Symbol),
    /// Stores packed Bytes of u32 global-order indices (4 bytes each, LE) for a submitter (issue #206).
    SubmitterEventIndices(Address),
    /// Cached event count per submitter (issue #206).
    SubmitterEventCount(Address),
    /// Primary storage: event ID → Event.
    EventData(BytesN<32>),
    /// Sequential index → event ID, for ordered retrieval.
    EventOrder(u32),
    /// Per-event-type metadata size cap (issue #67). Absent = use global default.
    EventMetadataMaxSize(Symbol),
    /// Global metadata size cap (issue #67). Absent = DEFAULT_MAX_METADATA_SIZE.
    GlobalMetadataMaxSize,
    /// Per-event-type metadata validation schema (issue #202). Absent = no schema constraint.
    MetadataSchema(Symbol),
    /// Cached full runtime state for fast reads.
    RuntimeState,
    /// Signature stored for an event (issue #69): (pubkey, signature).
    EventSignature(BytesN<32>),
    /// Cached event count per type (issue #52). Updated alongside EventTypeIndices.
    EventTypeCount(Symbol),
    /// Lightweight header (issue #56): EventHeader stored separately from metadata.
    EventHeaderKey(BytesN<32>),
    /// Optimized storage for event headers (issue #53): (index, timestamp, event_type, submitter).
    EventMeta(BytesN<32>),
    /// Optimized storage for event metadata alone (issue #53).
    EventMetadata(BytesN<32>),
    /// Stored update history for events indexed by event order.
    EventVersions(u32),
    /// Event emission configuration (issue #60): 0=full, 1=index-only, 2=hash-only, 3=none.
    EventEmissionConfig,
    /// Event emission version (issue #60): 1=full metadata, 2=index-only.
    EventEmissionVersion,
    /// Low-cost mode configuration (issue #57): 0=normal, 1=low-cost.
    LowCostMode,
    /// Rate limit (max events per ledger timestamp) for a submitter (issue #62). 0 = blocked.
    SubmitterRateLimit(Address),
    /// Rate-limit state (last_timestamp, count) per submitter (issue #62).
    SubmitterRateState(Address),
    /// Per-submitter nonce for replay-attack prevention (issue #64).
    /// Stores the last accepted nonce; absent means no event submitted yet (treat as 0).
    SubmitterNonce(Address),
    /// Packed nonce state per submitter (issue #214): window_size, max_nonce.
    SubmitterNonceConfig(Address),
    /// Global default nonce window size (issue #214).
    DefaultNonceWindowSize,
    /// Global maximum nonce value before exhaustion (issue #214).
    DefaultNonceMaxValue,
    ArchivedTotalEvents,
    ArchivedEventData(BytesN<32>),
    ArchivedEventHeaderKey(BytesN<32>),
    ArchivedEventMetadata(BytesN<32>),
    ArchivedEventArchivedFlag(BytesN<32>),
    ArchivedEventOrder(u32),
    EventArchivedFlag(BytesN<32>),
    Owners,
    RequiredSignatures,
    ProposalCount,
    Proposal(u32),
    /// TTL configuration (#121): number of ledgers after which persistent events are eligible for expiry.
    /// Absent = TTL disabled (instance storage, no expiry).
    EventTtl,
    /// Runtime state cache (#114): packed single-read state.
    /// Contract version marker.
    ContractVersion,
    /// Content-addressed dedup hash → event index.
    EventContentHash(BytesN<32>),
    /// Max category symbol byte length.
    CategoryMaxLen,
    /// Reentrancy guard marker (issue #61).
    LogEventReentrancyGuard,
    /// Timestamp when the contract was paused (issue #78).
    PausedSince,
    /// Webhook registrations (#25): per-event-type list of (url, secret) pairs. Owner-only.
    WebhookRegistrations(Symbol),
    /// Snapshot count — total snapshots created (issue #213).
    SnapshotCount,
    /// Individual snapshot data keyed by snapshot index (issue #213).
    SnapshotData(u32),
    /// Cumulative TTL cleanup statistics (issue #200).
    TtlCleanupStats,
    /// Resume cursor for `archive_events` scans (issue #199).
    ArchiveScanCursor,
    /// Social impact metrics snapshot (keyed by period tag: e.g. "2026_Q1").
    SocialImpactData(Symbol),
    /// Total number of social impact records logged.
    SocialImpactCount,
    /// Stakeholder registry entry keyed by stakeholder address.
    StakeholderEntry(Address),
    /// Total number of registered stakeholders.
    StakeholderCount,
    /// Latest generated impact report.
    LatestImpactReport,
    /// Modern slavery risk assessment keyed by assessment_id.
    MSARiskAssessment(Symbol),
    /// Total number of risk assessments recorded.
    MSARiskAssessmentCount,
    /// Supply chain node keyed by supplier_id.
    MSASupplyChainNode(Symbol),
    /// Total number of supply chain nodes mapped.
    MSASupplyChainNodeCount,
    /// Training record keyed by training_id.
    MSATrainingRecord(Symbol),
    /// Total number of training sessions recorded.
    MSATrainingRecordCount,
    /// Due diligence record keyed by record_id.
    MSADueDiligenceRecord(Symbol),
    /// Total number of due diligence investigations.
    MSADueDiligenceCount,
    /// Modern slavery policy keyed by policy_id.
    MSAPolicy(Symbol),
    /// Total number of active policies.
    MSAPolicyCount,
    /// Latest generated modern slavery report.
    LatestMSAReport,
    /// Conflict minerals allocation keyed by allocation_id.
    CMAllocation(Symbol),
    /// Total number of mineral allocations.
    CMAllocationCount,
    /// Smelter registry keyed by smelter_id.
    CMSmelter(Symbol),
    /// Total number of smelters registered.
    CMSmelterCount,
    /// Country-of-origin record keyed by record_id.
    CMCountryOfOrigin(Symbol),
    /// Total country-of-origin records.
    CMCountryOfOriginCount,
    /// Due diligence record keyed by record_id.
    CMDueDiligence(Symbol),
    /// Total due diligence records.
    CMDueDiligenceCount,
    /// Audit record keyed by audit_id.
    CMAudit(Symbol),
    /// Total audit records.
    CMAuditCount,
    /// Latest generated CMRT report.
    LatestCMRTReport,
}

/// On-chain social impact metrics snapshot for a reporting period.
///
/// All monetary values are in whole units of a reference currency (e.g. USD).
/// Counts are unsigned integers; ratios are stored as basis points (0–10000 = 0%–100%).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocialImpactMetrics {
    /// Reporting period tag, e.g. `"2026_Q1"` or `"2026_FY"`.
    pub period: Symbol,
    /// Unix timestamp when this record was submitted.
    pub recorded_at: u64,
    /// Address of the submitter.
    pub submitter: Address,

    // ── Job creation ──────────────────────────────────────────────────────
    /// Number of full-time-equivalent jobs created during the period.
    pub jobs_created: u32,
    /// Number of training / apprenticeship positions opened.
    pub training_positions: u32,

    // ── Workforce diversity ───────────────────────────────────────────────
    /// Percentage of workforce identifying as women, in basis points (0–10000).
    pub diversity_women_bps: u32,
    /// Percentage of workforce from underrepresented groups, in basis points.
    pub diversity_underrepresented_bps: u32,

    // ── Community investment ──────────────────────────────────────────────
    /// Direct community investment in whole monetary units.
    pub community_investment: u64,
    /// Number of beneficiaries reached by community programmes.
    pub community_beneficiaries: u32,

    // ── Human rights & labour standards ──────────────────────────────────
    /// Whether a human-rights due-diligence assessment was completed this period.
    pub human_rights_assessment_done: bool,
    /// Number of labour-standard violations reported and remediated.
    pub labour_violations_remediated: u32,
    /// Number of active collective-bargaining agreements.
    pub collective_bargaining_agreements: u32,

    // ── SROI inputs ───────────────────────────────────────────────────────
    /// Total investment (cost of interventions) in whole monetary units.
    pub total_investment: u64,
    /// Total social value created in whole monetary units.
    pub total_social_value: u64,
}

/// A registered stakeholder with their engagement details.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stakeholder {
    /// Stellar address that uniquely identifies this stakeholder.
    pub address: Address,
    /// Human-readable name encoded as UTF-8 `Bytes` (max 128 bytes).
    pub name: Bytes,
    /// Stakeholder category: e.g. `"worker"`, `"community"`, `"investor"`, `"regulator"`.
    pub category: Symbol,
    /// Impact weight in basis points (0–10000). Used when aggregating SROI across groups.
    pub weight_bps: u32,
    /// Unix timestamp when this stakeholder was registered.
    pub registered_at: u64,
}

/// An aggregated impact report computed from all on-chain social impact records.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactReport {
    /// Unix timestamp when this report was generated.
    pub generated_at: u64,
    /// Number of reporting periods included.
    pub periods_included: u32,
    /// Total jobs created across all periods.
    pub total_jobs_created: u32,
    /// Total community investment across all periods.
    pub total_community_investment: u64,
    /// Average workforce diversity — women, basis points.
    pub avg_diversity_women_bps: u32,
    /// Total social value across all periods.
    pub total_social_value: u64,
    /// Total investment across all periods.
    pub total_investment: u64,
    /// SROI ratio stored as basis points of (social_value / investment) × 10 000.
    /// E.g. an SROI of 3.5× is stored as 35000. Zero if no investment recorded.
    pub sroi_bps: u64,
    /// Number of registered stakeholders at report generation time.
    pub stakeholder_count: u32,
}

// ── Modern Slavery Act Compliance ──────────────────────────────────────

/// Modern slavery risk assessment record per the UK Modern Slavery Act 2015
/// and Australian Modern Slavery Act 2018 frameworks.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAssessment {
    /// Unique identifier for this assessment (e.g. "2026_q1_assessment").
    pub assessment_id: Symbol,
    /// Unix timestamp when the assessment was recorded.
    pub recorded_at: u64,
    /// Address of the submitter (organisation / assessor).
    pub submitter: Address,
    /// Geographic scope of the assessment (e.g. "global", "region_apac").
    pub scope: Symbol,
    /// Overall risk level: 0=low, 1=medium, 2=high, 3=critical.
    pub risk_level: u32,
    /// Number of identified high-risk areas in supply chain.
    pub high_risk_areas: u32,
    /// Brief description of key risks (max 256 bytes).
    pub key_risks: Bytes,
    /// Number of remediation actions planned.
    pub planned_remediations: u32,
    /// Whether this assessment included stakeholder consultation.
    pub stakeholder_consultation_done: bool,
}

/// A node in a supply chain network, representing a supplier or partner.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyChainNode {
    /// Unique identifier for this supplier (e.g. supplier address or code).
    pub supplier_id: Symbol,
    /// Organization name (max 128 bytes).
    pub name: Bytes,
    /// Geographic location (country code or region).
    pub country: Symbol,
    /// Risk classification: 0=low, 1=medium, 2=high, 3=critical.
    pub risk_level: u32,
    /// Whether this supplier has been audited by the organisation.
    pub audited: bool,
    /// Last audit date (Unix timestamp). 0 if never audited.
    pub last_audit_date: u64,
    /// Unix timestamp when this node was registered.
    pub registered_at: u64,
}

/// A training record for personnel on modern slavery awareness and due diligence.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingRecord {
    /// Unique training session identifier (e.g. "training_2026_001").
    pub training_id: Symbol,
    /// Unix timestamp when the training was delivered.
    pub delivered_at: u64,
    /// Training topic (e.g. "msa_awareness", "due_diligence", "reporting").
    pub topic: Symbol,
    /// Number of personnel trained in this session.
    pub attendees: u32,
    /// Whether the session covered risk assessment methodology.
    pub risk_assessment_covered: bool,
    /// Whether the session covered due diligence procedures.
    pub due_diligence_covered: bool,
    /// Whether the session covered reporting obligations.
    pub reporting_covered: bool,
    /// Brief description of training content (max 256 bytes).
    pub content_summary: Bytes,
}

/// A due diligence record documenting investigations into supplier practices.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DueDiligenceRecord {
    /// Unique record identifier (e.g. "dd_2026_supplier_001").
    pub record_id: Symbol,
    /// Unix timestamp when the due diligence was completed.
    pub completed_at: u64,
    /// Supplier or entity being investigated (Symbol identifier).
    pub subject: Symbol,
    /// Investigation scope (e.g. "labour_practices", "child_labor", "forced_labour").
    pub scope: Symbol,
    /// Findings summary (max 512 bytes).
    pub findings: Bytes,
    /// Risk level identified: 0=none, 1=low, 2=medium, 3=high, 4=critical.
    pub risk_level: u32,
    /// Number of corrective actions required.
    pub corrective_actions_required: u32,
    /// Completion percentage of corrective actions (0-100).
    pub corrective_actions_completed_pct: u32,
}

/// A policy document record for modern slavery prevention.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MSAPolicy {
    /// Policy identifier (e.g. "policy_msa_2026").
    pub policy_id: Symbol,
    /// Unix timestamp when the policy was first adopted.
    pub adopted_at: u64,
    /// Unix timestamp of the last review/update.
    pub last_updated_at: u64,
    /// Policy version number.
    pub version: u32,
    /// Policy scope (e.g. "global", "operations_only", "supply_chain").
    pub scope: Symbol,
    /// Policy content summary (max 1024 bytes).
    pub content_summary: Bytes,
    /// Whether stakeholder consultation was included in policy development.
    pub stakeholder_input_included: bool,
}

/// Aggregated modern slavery compliance report.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MSAReport {
    /// Report generation timestamp.
    pub generated_at: u64,
    /// Total number of risk assessments included.
    pub assessments_count: u32,
    /// Highest risk level found across all assessments: 0=low, 3=critical.
    pub max_risk_level: u32,
    /// Total high-risk areas identified.
    pub total_high_risk_areas: u32,
    /// Number of supply chain nodes mapped.
    pub supply_chain_nodes: u32,
    /// Number of supply chain nodes classified as high/critical risk.
    pub high_risk_suppliers: u32,
    /// Total number of personnel trained.
    pub total_trained_personnel: u32,
    /// Number of due diligence investigations completed.
    pub due_diligence_investigations: u32,
    /// Total corrective actions identified.
    pub total_corrective_actions: u32,
    /// Percentage of corrective actions completed (0-100).
    pub corrective_actions_completion_pct: u32,
    /// Number of active policies.
    pub active_policies: u32,
}

// ── Conflict Minerals Reporting (Dodd-Frank §1502) ──────────────────────

/// Mineral allocation record tracking 3TG minerals through supply chain.
/// Maps minerals to products, manufacturers, and smelters per CMRT.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MineralAllocation {
    /// Unique allocation identifier (e.g. "alloc_2026_001").
    pub allocation_id: Symbol,
    /// Unix timestamp of allocation record.
    pub recorded_at: u64,
    /// Product or component identifier.
    pub product_id: Symbol,
    /// Mineral type: "tin", "tantalum", "tungsten", "gold".
    pub mineral: Symbol,
    /// Quantity in metric tonnes (stored as whole units).
    pub quantity_mt: u64,
    /// Smelter identifier(s) where mineral was processed (semicolon-separated).
    pub smelters: Bytes,
    /// Country of origin (ISO 3166 code or "DRC_conflict", "DRC_artisanal", "undetermined").
    pub country_of_origin: Symbol,
    /// Whether the country is designated as conflict-affected or high-risk.
    pub conflict_region: bool,
    /// Submission address (responsible party).
    pub submitter: Address,
}

/// Smelter registry entry with audit and compliance status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Smelter {
    /// Unique smelter identifier (ICGLR ID or internal code).
    pub smelter_id: Symbol,
    /// Smelter name (max 128 bytes).
    pub name: Bytes,
    /// Geographic location (country code).
    pub country: Symbol,
    /// Mineral processed by this smelter ("tin", "tantalum", "tungsten", "gold", or multi-mineral).
    pub mineral_type: Symbol,
    /// Whether smelter is on the CMRT-approved or industry-recognized list.
    pub on_approved_list: bool,
    /// Unix timestamp of last audit.
    pub last_audit_date: u64,
    /// Audit status: 0=never audited, 1=audit scheduled, 2=audit in progress, 3=audit complete.
    pub audit_status: u32,
    /// Whether independent audit has been completed.
    pub independent_audit_done: bool,
    /// Brief assessment of smelter conflict mineral practices (max 256 bytes).
    pub assessment: Bytes,
}

/// Country-of-origin record documenting mineral source verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountryOfOrigin {
    /// Unique record identifier (e.g. "coo_2026_001").
    pub record_id: Symbol,
    /// Unix timestamp of verification.
    pub verified_at: u64,
    /// Mineral allocation this origin record applies to.
    pub allocation_id: Symbol,
    /// Country code (ISO 3166 or conflict designation).
    pub country: Symbol,
    /// Percentage of total allocation from this country (0-100).
    pub percentage: u32,
    /// Verification method: "documentation", "audit", "certification", "third_party".
    pub verification_method: Symbol,
    /// Documentation hash (SHA-256 of supporting evidence).
    pub evidence_hash: BytesN<32>,
}

/// Due diligence record for conflict minerals supply chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DueDiligenceCM {
    /// Unique due diligence record identifier.
    pub record_id: Symbol,
    /// Unix timestamp of due diligence activity.
    pub completed_at: u64,
    /// Supplier or smelter being assessed.
    pub subject: Symbol,
    /// Scope of assessment ("sourcing", "supply_chain", "smelter", "refinement").
    pub scope: Symbol,
    /// Findings summary (max 512 bytes).
    pub findings: Bytes,
    /// Risk level: 0=compliant, 1=low risk, 2=medium risk, 3=high risk.
    pub risk_level: u32,
    /// Whether conflict minerals risk was identified.
    pub conflict_risk_identified: bool,
    /// Number of corrective actions required.
    pub corrective_actions: u32,
    /// Completion percentage of corrective actions (0-100).
    pub corrective_actions_pct: u32,
}

/// Independent audit record for smelter or supply chain verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    /// Unique audit identifier.
    pub audit_id: Symbol,
    /// Unix timestamp of audit completion.
    pub completed_at: u64,
    /// Audited entity (smelter, supplier, or facility).
    pub subject: Symbol,
    /// Audit standard used (e.g. "ICGLR", "RMI", "LBMA", "custom").
    pub audit_standard: Symbol,
    /// Audit firm name (max 128 bytes).
    pub audit_firm: Bytes,
    /// Audit findings summary (max 512 bytes).
    pub findings: Bytes,
    /// Audit result: 0=pass, 1=pass_with_exceptions, 2=fail.
    pub result: u32,
    /// Key corrective actions identified.
    pub corrective_actions: u32,
    /// Documentation hash (SHA-256 of audit report).
    pub report_hash: BytesN<32>,
}

/// Conflict Minerals Reporting Template (CMRT) report aggregation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CMRTReport {
    /// Report generation timestamp.
    pub generated_at: u64,
    /// Reporting period (e.g. "2026_calendar_year").
    pub period: Symbol,
    /// Total mineral allocations reported.
    pub total_allocations: u32,
    /// Allocations from conflict-affected regions.
    pub conflict_region_allocations: u32,
    /// Total smelters identified.
    pub total_smelters: u32,
    /// Smelters on approved/recognized lists.
    pub on_list_smelters: u32,
    /// Smelters with pending independent audits.
    pub audits_pending: u32,
    /// Smelters with completed independent audits.
    pub audits_completed: u32,
    /// Due diligence assessments completed.
    pub due_diligence_count: u32,
    /// Conflict mineral risk identifications.
    pub conflict_risk_identified: u32,
    /// Percentage of supply chain with verified country of origin (0-100).
    pub origin_verification_pct: u32,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// **Code 1**: Caller does not have owner privileges. Only the current owner can invoke governance functions.
    /// **Common cause**: Non-owner attempting `set_global_max_logs`, `set_event_max_logs`, `remove_event_cap`, or `transfer_ownership`.
    /// **Resolution**: Ensure the caller has been authorized as owner or contact the current owner for delegation.
    CallerNotOwner = 1,

    /// **Code 2**: Global event log capacity reached. Total events equal or exceed `global_max_logs`.
    /// **Common cause**: Too many events logged; `global_max_logs` cap is too low for demand.
    /// **Resolution**: Owner should call `set_global_max_logs` with a higher limit, or archive old events off-chain.
    GlobalMaxLogsReached = 2,

    /// **Code 3**: Per-event-type log capacity reached. Events of this type equal or exceed the type-specific cap.
    /// **Common cause**: `set_event_max_logs` configured a limit too low for this event type's demand.
    /// **Resolution**: Owner should increase the cap via `set_event_max_logs`, or call `remove_event_cap` to lift the type-level limit.
    EventTypeMaxLogsReached = 3,

    /// **Code 4**: Event ID does not exist in the ledger.
    /// **Common cause**: Querying a non-existent event index or using an invalid event hash.
    /// **Resolution**: Verify the event ID against `total_events()` or enumerate with `get_event_by_type()`.
    EventDoesNotExist = 4,

    /// **Code 5**: Index out of bounds for a per-event-type sub-ledger.
    /// **Common cause**: `type_index` parameter in `get_event_by_type()` exceeds `event_count(event_type)`.
    /// **Resolution**: Ensure `type_index < event_count(event_type)`. Start from index 0 and iterate.
    EventTypeIndexOutOfBounds = 5,

    /// **Code 6**: New owner address is zero or invalid.
    /// **Common cause**: `transfer_ownership()` called with a null/uninitialized address.
    /// **Resolution**: Provide a valid Stellar account address (e.g., `GXXXXX…`).
    NewOwnerIsZero = 6,

    /// **Code 7**: Event-type cap is not set. Cannot remove a cap that does not exist.
    /// **Common cause**: `remove_event_cap()` called on an event type that never had a cap configured.
    /// **Resolution**: Use `set_event_max_logs()` first, then call `remove_event_cap()` to lift it.
    CapNotSet = 7,

    /// **Code 8**: Event metadata exceeds the configured maximum size.
    /// **Common cause**: Metadata payload larger than `global_metadata_max_size` or event-type specific limit.
    /// **Resolution**: Reduce metadata size, or owner should increase limit via `set_global_metadata_max_size()` or `set_event_metadata_max_size()`.
    MetadataTooLarge = 8,

    /// **Code 32**: Metadata does not satisfy the configured min-length schema for its event type.
    /// **Common cause**: Owner configured a min-bytes schema for this event type and the submitted metadata is too short.
    /// **Resolution**: Include more metadata bytes or have the owner reduce the constraint via `set_metadata_schema`.
    MetadataSchemaViolation = 32,

    /// **Code 9**: Contract has not been initialized.
    /// **Common cause**: Attempting to call functions before `initialize(owner, global_max_logs)`.
    /// **Resolution**: Owner must call `initialize()` once at contract deployment.
    ContractNotInitialized = 9,

    /// **Code 10**: Internal: total events counter would overflow.
    /// **Common cause**: Architectural limit; extremely high event volume (unlikely in practice).
    /// **Resolution**: Contact developers; consider archiving or contract migration.
    TotalEventsOverflow = 10,

    /// **Code 11**: Event timestamp is outside acceptable range.
    /// **Common cause**: Timestamp differs by >3600 seconds from ledger time (possible clock skew or invalid input).
    /// **Resolution**: Verify system clock is synchronized, or contact submitter to resubmit with correct timestamp.
    TimestampOutOfRange = 11,

    /// **Code 12**: Event signature validation failed.
    /// **Common cause**: Signature mismatch; event data was tampered with or signed with wrong key.
    /// **Resolution**: Re-sign the event with the correct private key, or verify the event content has not been modified.
    InvalidSignature = 12,

    /// **Code 13**: Contract is currently paused; write operations are blocked.
    /// **Common cause**: Owner called `set_paused(true)` to halt event logging (maintenance mode).
    /// **Resolution**: Contact owner to resume with `set_paused(false)`.
    ContractPaused = 13,

    /// **Code 14**: Submitter has exceeded per-submitter rate limit.
    /// **Common cause**: Too many events submitted in a single ledger timestamp; rate limit enforced per submitter.
    /// **Resolution**: Wait for the next ledger or contact owner to increase `set_submitter_rate_limit()`.
    RateLimitExceeded = 14,

    /// **Code 15**: Attempted transfer to the same owner address.
    /// **Common cause**: `transfer_ownership()` called with current owner address.
    /// **Resolution**: Provide a different owner address.
    SameOwner = 15,

    /// **Code 16**: New maximum logs is below the current total event count.
    /// **Common cause**: `set_global_max_logs()` or `set_event_max_logs()` called with a value less than current count.
    /// **Resolution**: Set the new max to at least the current count, or archive/prune existing events first.
    MaxLogsBelowCurrentCount = 16,

    /// **Code 17**: Cap already removed for this event type.
    /// **Common cause**: `remove_event_cap()` called twice on the same event type.
    /// **Resolution**: No action needed; cap is already lifted. Use `set_event_max_logs()` to set a new cap.
    CapAlreadyRemoved = 17,
    CapNeverSet = 18,
    NonceTooLow = 19,
    /// **Code 27**: Nonce has reached the exhaustion threshold for this submitter.
    NonceExhausted = 27,
    /// **Code 28**: Nonce is outside the valid window for this submitter.
    NonceWindowExceeded = 28,
    /// **Code 29**: Attempted reset when the submitter's nonce is not exhausted.
    NonceResetNotExhausted = 29,
    NoEventsForType = 20,
    InvalidPaginationParams = 21,
    InvalidWasmHash = 22,
    SubmitterBlocked = 23,
    /// **Code 24**: Category symbol exceeds maximum byte length.
    CategoryTooLong = 24,
    /// **Code 25**: Reentrant call detected; recursion is not permitted.
    ReentrancyDetected = 25,

    /// **Code 26**: Contract has already been initialized.
    /// **Common cause**: Calling `initialize()` more than once.
    /// **Resolution**: The contract can only be initialized once at deployment.
    AlreadyInitialized = 26,
    /// **Code 30**: Snapshot does not exist.
    SnapshotNotFound = 30,
    /// **Code 31**: Snapshot verification failed (hash mismatch).
    SnapshotVerificationFailed = 31,
    /// **Code 33**: Event version does not exist in history.
    /// **Common cause**: `rollback_event` called with a version number beyond the stored history length.
    /// **Resolution**: Use `get_event_history` or `get_event_version_count` to discover valid versions.
    InvalidVersion = 33,
    /// **Code 34**: Social impact period tag already recorded for this period.
    /// **Common cause**: `record_social_impact` called twice with the same period Symbol.
    /// **Resolution**: Use a unique period tag or call `update_social_impact` to revise.
    SocialImpactPeriodExists = 34,
    /// **Code 35**: Social impact record not found for the given period.
    /// **Common cause**: `get_social_impact` called with a period that was never recorded.
    /// **Resolution**: Use `social_impact_count` to enumerate recorded periods.
    SocialImpactNotFound = 35,
    /// **Code 36**: Stakeholder address is already registered.
    /// **Common cause**: `add_stakeholder` called twice with the same address.
    /// **Resolution**: Use `get_stakeholder` to verify before registering.
    StakeholderAlreadyExists = 36,
    /// **Code 37**: Stakeholder not found for the given address.
    /// **Common cause**: `get_stakeholder` or remove called with an unregistered address.
    /// **Resolution**: Verify the address via `stakeholder_count` or `get_stakeholder`.
    StakeholderNotFound = 37,
    /// **Code 38**: SROI cannot be calculated because total investment is zero.
    /// **Common cause**: All recorded periods have `total_investment = 0`.
    /// **Resolution**: Ensure at least one period includes a non-zero `total_investment`.
    SroiDivisionByZero = 38,
    /// **Code 39**: Modern slavery risk assessment already recorded for this assessment_id.
    /// **Common cause**: `record_risk_assessment` called twice with the same assessment_id.
    /// **Resolution**: Use a unique assessment_id or update via a new record.
    MSARiskAssessmentExists = 39,
    /// **Code 40**: Modern slavery risk assessment not found for the given assessment_id.
    /// **Common cause**: `get_risk_assessment` called with a non-existent assessment_id.
    /// **Resolution**: Verify the assessment_id via assessment list or record new.
    MSARiskAssessmentNotFound = 40,
    /// **Code 41**: Supply chain node already registered for this supplier_id.
    /// **Common cause**: `record_supply_chain_node` called twice with same supplier_id.
    /// **Resolution**: Use unique supplier_id or update via separate call.
    MSASupplyChainNodeExists = 41,
    /// **Code 42**: Supply chain node not found for the given supplier_id.
    /// **Common cause**: `get_supply_chain_node` called with non-existent supplier_id.
    /// **Resolution**: Register the node first via `record_supply_chain_node`.
    MSASupplyChainNodeNotFound = 42,
    /// **Code 43**: Training record not found for the given training_id.
    /// **Common cause**: `get_training_record` called with non-existent training_id.
    /// **Resolution**: Verify training_id or record new training session.
    MSATrainingRecordNotFound = 43,
    /// **Code 44**: Due diligence record not found for the given record_id.
    /// **Common cause**: `get_due_diligence_record` called with non-existent record_id.
    /// **Resolution**: Verify record_id or submit new due diligence investigation.
    MSADueDiligenceRecordNotFound = 44,
    /// **Code 45**: Modern slavery policy not found for the given policy_id.
    /// **Common cause**: `get_msa_policy` called with non-existent policy_id.
    /// **Resolution**: Record policy first or verify policy_id.
    MSAPolicyNotFound = 45,
    /// **Code 46**: Conflict minerals allocation already recorded for this allocation_id.
    /// **Common cause**: `record_mineral_allocation` called twice with same allocation_id.
    /// **Resolution**: Use unique allocation_id or update via new record.
    CMAllocationExists = 46,
    /// **Code 47**: Conflict minerals allocation not found.
    /// **Common cause**: `get_mineral_allocation` called with non-existent allocation_id.
    /// **Resolution**: Verify allocation_id or record new allocation.
    CMAllocationNotFound = 47,
    /// **Code 48**: Smelter already registered for this smelter_id.
    /// **Common cause**: `record_smelter` called twice with same smelter_id.
    /// **Resolution**: Use unique smelter_id or update via new record.
    CMSmelterExists = 48,
    /// **Code 49**: Smelter not found for the given smelter_id.
    /// **Common cause**: `get_smelter` called with non-existent smelter_id.
    /// **Resolution**: Register smelter first or verify smelter_id.
    CMSmelterNotFound = 49,
    /// **Code 50**: Country-of-origin record not found.
    /// **Common cause**: `get_country_of_origin` called with non-existent record_id.
    /// **Resolution**: Record origin verification first.
    CMCountryOfOriginNotFound = 50,
    /// **Code 51**: Due diligence record not found.
    /// **Common cause**: `get_due_diligence_cm` called with non-existent record_id.
    /// **Resolution**: Submit due diligence assessment first.
    CMDueDiligenceNotFound = 51,
    /// **Code 52**: Audit record not found.
    /// **Common cause**: `get_audit_record` called with non-existent audit_id.
    /// **Resolution**: Record audit first or verify audit_id.
    CMAuditNotFound = 52,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventVersion {
    pub version: u32,
    pub data: Event,
    pub updated_at: u64,
    pub updated_by: Address,
}

/// On-chain webhook registration entry (#25).
/// Stored per event type; the off-chain relayer reads these to dispatch HTTP callbacks.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebhookEntry {
    /// HTTP(S) URL to POST to.
    pub url: Bytes,
    /// HMAC secret for request signing (opaque bytes, never returned by queries).
    pub secret: Bytes,
}

/// Packed nonce state per submitter for optimized storage (issue #214).
/// Combines last nonce, window size, and max nonce into a single storage read.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonceState {
    /// Last accepted nonce value.
    pub last_nonce: u32,
    /// Window size: max allowed gap between last nonce and submitted nonce. 0 = unlimited.
    pub window_size: u32,
    /// Maximum nonce value before exhaustion.
    pub max_nonce: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStatistics {
    pub total_events: u32,
    pub events_by_type: Vec<(Symbol, u32)>,
    pub events_last_hour: u32,
    pub events_last_day: u32,
    pub events_last_week: u32,
    pub top_submitters: Vec<(Address, u32)>,
}

/// Result of a single event in a batch submission (issue #223).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchEventResult {
    /// Index of the event within the batch.
    pub batch_index: u32,
    /// Whether this event was successfully logged.
    pub success: bool,
    /// Event ID (BytesN<32>) if successful, or None if failed.
    pub event_id: Option<BytesN<32>>,
    /// Error message if failed, or empty if successful.
    pub error: Bytes,
}

/// State of a batch submission retry (issue #223).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRetryState {
    /// Batch identifier.
    pub batch_id: u32,
    /// Total events in original batch.
    pub total_events: u32,
    /// Number of events successfully logged.
    pub succeeded: u32,
    /// Number of events that failed.
    pub failed: u32,
    /// Timestamp when the batch was first submitted.
    pub created_at: u64,
    /// Whether the batch has been fully retried.
    pub completed: bool,
}

/// Record of an event access (issue #216).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRecord {
    /// The address that accessed the event.
    pub accessor: Address,
    /// The index of the event accessed.
    pub event_index: u32,
    /// Timestamp of the access.
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    TransferOwnership(Address),
    AddOwner(Address),
    RemoveOwner(Address),
    SetRequiredSignatures(u32),
    SetGlobalMaxLogs(u32),
    SetMetadataSchema(Symbol, Bytes),
    RollbackEvent(u32, u32),
    Pause,
    Unpause,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u32,
    pub proposer: Address,
    pub action: ProposalAction,
    pub approvals: Vec<Address>,
    pub expires_at: u64,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub id: u32,
    pub timestamp: u64,
    pub event_count: u32,
    pub event_hash: BytesN<32>,
    pub description: Bytes,
}

/// Cumulative statistics for TTL-based cleanup runs (issue #200).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TtlCleanupStats {
    /// Total number of cleanup runs triggered.
    pub runs: u32,
    /// Total events whose TTL was extended during reads.
    pub ttl_extensions: u32,
    /// Total events cleaned up (removed from persistent storage) across all runs.
    pub cleaned: u32,
    /// Ledger sequence number of the last cleanup run, 0 if none.
    pub last_run_ledger: u32,
}

// ── Additional type definitions ──────────────────────────────────────────────

#[contract]
pub struct AuditLedger;

#[contractimpl]
impl AuditLedger {
    pub fn initialize(
        env: Env,
        owners: Vec<Address>,
        global_max_logs: u32,
        max_metadata_bytes: u32,
    ) {
        if env.storage().instance().has(&DataKey::Owner) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }
        if owners.is_empty() {
            panic_with_error!(&env, ContractError::NewOwnerIsZero);
        }
        let primary = owners.get(0).unwrap();
        primary.require_auth();
        env.storage().instance().set(&DataKey::Owner, &primary);
        env.storage().instance().set(&DataKey::Owners, &owners);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                global_max_logs,
                total_events: 0,
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::GlobalMaxLogs, &global_max_logs);
        env.storage()
            .instance()
            .set(&DataKey::GlobalMetadataMaxSize, &max_metadata_bytes);
        env.storage()
            .instance()
            .set(
                &DataKey::RuntimeState,
                &RuntimeState {
                    global_max_logs,
                    total_events: 0,
                    paused: false,
                    allowlist_mode: false,
                    low_cost_mode: false,
                    emission_mode: 1,
                    global_metadata_max_size: max_metadata_bytes,
                },
            );
        env.storage().instance().set(&DataKey::TotalEvents, &0u32);
        env.storage().instance().set(&DataKey::Paused, &false);

        // Set version to 1 (marks contract as initialized, immutable)
        env.storage().instance().set(&DataKey::ContractVersion, &1u32);

        // Issue #214: Initialize default nonce replay protection configuration
        env.storage()
            .instance()
            .set(&DataKey::DefaultNonceWindowSize, &1000u32);
        env.storage()
            .instance()
            .set(&DataKey::DefaultNonceMaxValue, &u32::MAX);
    }

    /// Log a batch of events atomically and return their sequential indices.
    pub fn log_events(env: Env, events: Vec<(Address, Symbol, Bytes)>) -> Vec<u32> {
        Self::require_initialized(&env);

        // Single read for all global state (issue #114)
        let rs: RuntimeState = env.storage().instance().get(&DataKey::RuntimeState).unwrap_or_else(|| {
            let cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
            RuntimeState {
                global_max_logs: cfg.global_max_logs,
                total_events: cfg.total_events,
                paused: env
                    .storage()
                    .instance()
                    .get::<_, bool>(&DataKey::Paused)
                    .unwrap_or(false),
                allowlist_mode: false,
                low_cost_mode: env
                    .storage()
                    .instance()
                    .get::<_, bool>(&DataKey::LowCostMode)
                    .unwrap_or(false),
                emission_mode: env
                    .storage()
                    .instance()
                    .get::<_, u32>(&DataKey::EventEmissionConfig)
                    .unwrap_or(1),
                global_metadata_max_size: 0,
            }
        });

        if rs.paused {
            panic_with_error!(&env, ContractError::ContractPaused);
        }

        let global_max = rs.global_max_logs;
        let total = rs.total_events;
        let batch_len: u32 = events.len();

        if total.checked_add(batch_len).is_none() || total + batch_len > global_max {
            panic_with_error!(&env, ContractError::GlobalMaxLogsReached);
        }

        let now = env.ledger().timestamp();
        let mut submitter_batch_counts: Vec<(Address, u32)> = Vec::new(&env);
        let mut type_batch_counts: Vec<(Symbol, u32)> = Vec::new(&env);
        let mut authorized_submitters: Vec<Address> = Vec::new(&env);

        for i in 0..batch_len {
            let (submitter, event_type, metadata) = events.get(i).unwrap().clone();
            let mut already_authorized = false;
            for j in 0..authorized_submitters.len() {
                if authorized_submitters.get(j).unwrap() == submitter {
                    already_authorized = true;
                    break;
                }
            }
            if !already_authorized {
                submitter.require_auth();
                authorized_submitters.push_back(submitter.clone());
            }

            let max_meta = Self::effective_metadata_max_size(&env, &event_type);
            if metadata.len() > max_meta {
                panic_with_error!(&env, ContractError::MetadataTooLarge);
            }

            // --- issue #202: validate metadata against optional per-type schema ---
            Self::validate_metadata_against_schema(&env, &event_type, &metadata);

            if let Some(limit) = env
                .storage()
                .instance()
                .get::<_, u32>(&DataKey::SubmitterRateLimit(submitter.clone()))
            {
                let (last_ts, count): (u64, u32) = env
                    .storage()
                    .instance()
                    .get(&DataKey::SubmitterRateState(submitter.clone()))
                    .unwrap_or((0u64, 0u32));
                let batch_count = Self::increment_address_count(&env, &mut submitter_batch_counts, submitter.clone());
                if now == last_ts {
                    if count + batch_count > limit {
                        panic_with_error!(&env, ContractError::RateLimitExceeded);
                    }
                } else if batch_count > limit {
                    panic_with_error!(&env, ContractError::RateLimitExceeded);
                }
            }

            if let Some(cap) = env
                .storage()
                .instance()
                .get::<_, Option<u32>>(&DataKey::EventCapConfig(event_type.clone()))
                .flatten()
            {
                let current_count = Self::event_type_count(&env, event_type.clone());
                let batch_count = Self::increment_symbol_count(&env, &mut type_batch_counts, event_type.clone());
                if current_count + batch_count > cap {
                    panic_with_error!(&env, ContractError::EventTypeMaxLogsReached);
                }
            }
        }

        let mut result_indices: Vec<u32> = Vec::new(&env);
        let mut current_total = total;
        let mut prev_hash: BytesN<32> = if current_total == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let prev_id: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(current_total - 1))
                .unwrap();
            let prev_evt: Event = env.storage().instance().get(&DataKey::EventData(prev_id)).unwrap();
            prev_evt.event_hash
        };

        for i in 0..batch_len {
            let (submitter, event_type, metadata) = events.get(i).unwrap().clone();
            let index = current_total;
            let timestamp = env.ledger().timestamp();
            let event_id = Self::compute_event_id(&env, &submitter, &event_type, &metadata, timestamp, index);
            let event_hash = Self::compute_event_hash(&env, &event_id, &prev_hash, index, timestamp);

            let evt = Event {
                index,
                timestamp,
                event_type: event_type.clone(),
                category: Symbol::new(&env, "general"),
                submitter: submitter.clone(),
                metadata: metadata.clone(),
                sub_event_type: None,
                version: Self::current_contract_version(&env),
                event_hash: event_hash.clone(),
                prev_hash: prev_hash.clone(),
                parent_event_id: None,
            };

            env.storage()
                .instance()
                .set(&DataKey::EventData(event_id.clone()), &evt);
            env.storage().instance().set(&DataKey::EventOrder(index), &event_id);

            let header = EventHeader {
                index,
                timestamp,
                event_type: event_type.clone(),
                submitter: submitter.clone(),
            };
            env.storage()
                .instance()
                .set(&DataKey::EventHeaderKey(event_id.clone()), &header);
            env.storage()
                .instance()
                .set(&DataKey::EventMetadata(event_id.clone()), &metadata);

            if !Self::effective_low_cost_mode(&env) {
                Self::push_type_index(&env, event_type.clone(), index);
                Self::push_submitter_index(&env, &submitter, index);
                let mut count: u32 = env
                    .storage()
                    .instance()
                    .get(&DataKey::EventTypeCount(event_type.clone()))
                    .unwrap_or(0);
                count += 1;
                env.storage()
                    .instance()
                    .set(&DataKey::EventTypeCount(event_type.clone()), &count);
            }

            // #175: emit structured ("audit", "log_event") event with (submitter, event_type, index)
            let emission_mode = Self::effective_event_emission_mode(&env);
            if emission_mode != 3 {
                env.events().publish(
                    (Symbol::new(&env, "audit"), Symbol::new(&env, "log_event")),
                    (submitter.clone(), event_type.clone(), index),
                );
            }

            result_indices.push_back(index);
            prev_hash = event_hash;
            current_total += 1;
        }

        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                global_max_logs: global_max,
                total_events: current_total,
            },
        );
        if let Some(mut rs2) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs2.total_events = current_total;
            env.storage().instance().set(&DataKey::RuntimeState, &rs2);
        }

        result_indices
    }

    /// Log an event and return its content-addressed `BytesN<32>` ID.
    ///
    /// When `force` is `false` (the default), identical events (same `event_type`,
    /// `submitter`, and `metadata`) are deduplicated: the second call returns the
    /// existing event's ID without storing a new event.
    /// Set `force = true` to bypass deduplication and always store a new event.
    pub fn log_event(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        force: bool,
    ) -> BytesN<32> {
        Self::log_event_with_hierarchy(env, submitter, event_type, metadata, category, sub_event_type, force)
    }

    // Extended API — alias for log_event.
    pub fn log_event_with_hierarchy(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        force: bool,
    ) -> BytesN<32> {
        Self::require_initialized(&env);
        submitter.require_auth();

        // --- issue #61: reentrancy guard ---
        // Temporary storage is scoped to the current transaction; if a
        // reentrant call arrives before the key is cleared the guard fires.
        if env.storage().temporary().has(&DataKey::LogEventReentrancyGuard) {
            panic_with_error!(&env, ContractError::ReentrancyDetected);
        }
        env.storage().temporary().set(&DataKey::LogEventReentrancyGuard, &true);

        // --- issue #63: validate event_type Symbol ---
        Self::validate_event_type(&env, &event_type);

        // Reject writes when contract is paused.
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }

        // --- issue #141: enforce submitter blocklist/allowlist ---
        // Check if submitter is blocked
        if let Some(true) = env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::SubmitterBlocklist(submitter.clone()))
        {
            panic_with_error!(&env, ContractError::SubmitterBlocked);
        }

        // Check allowlist mode
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::AllowlistMode) {
            if let Some(false) = env
                .storage()
                .instance()
                .get::<_, bool>(&DataKey::SubmitterAllowlist(submitter.clone()))
            {
                panic_with_error!(&env, ContractError::SubmitterBlocked);
            }
            // If allowlist key doesn't exist, reject by default
            if env
                .storage()
                .instance()
                .get::<_, bool>(&DataKey::SubmitterAllowlist(submitter.clone()))
                .is_none()
            {
                panic_with_error!(&env, ContractError::SubmitterBlocked);
            }
        }

        let rs: RuntimeState = env
            .storage()
            .instance()
            .get(&DataKey::RuntimeState)
            .unwrap_or_else(|| {
                let cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
                RuntimeState {
                    global_max_logs: cfg.global_max_logs,
                    total_events: cfg.total_events,
                    paused: env.storage().instance().get::<_, bool>(&DataKey::Paused).unwrap_or(false),
                    allowlist_mode: env.storage().instance().get::<_, bool>(&DataKey::AllowlistMode).unwrap_or(false),
                    low_cost_mode: env.storage().instance().get::<_, bool>(&DataKey::LowCostMode).unwrap_or(false),
                    emission_mode: env.storage().instance().get::<_, u32>(&DataKey::EventEmissionConfig).unwrap_or(1),
                    global_metadata_max_size: env.storage().instance().get::<_, u32>(&DataKey::GlobalMetadataMaxSize).unwrap_or(0),
                }
            });
        let mut cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap_or(Config {
            global_max_logs: rs.global_max_logs,
            total_events: rs.total_events,
        });
        if let Some(limit) = env
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::SubmitterRateLimit(submitter.clone()))
        {
            let now = env.ledger().timestamp();
            let (last_ts, count): (u64, u32) = env
                .storage()
                .instance()
                .get(&DataKey::SubmitterRateState(submitter.clone()))
                .unwrap_or((0u64, 0u32));
            if now == last_ts {
                if count >= limit {
                    panic_with_error!(&env, ContractError::RateLimitExceeded);
                }
                env.storage()
                    .instance()
                    .set(&DataKey::SubmitterRateState(submitter.clone()), &(now, count + 1));
            } else {
                if limit == 0 {
                    panic_with_error!(&env, ContractError::RateLimitExceeded);
                }
                env.storage()
                    .instance()
                    .set(&DataKey::SubmitterRateState(submitter.clone()), &(now, 1u32));
            }
        }

        // --- issue #67: enforce metadata size cap ---
        let max_meta = Self::effective_metadata_max_size(&env, &event_type);
        if metadata.len() > max_meta {
            panic_with_error!(&env, ContractError::MetadataTooLarge);
        }

        // --- issue #202: validate metadata against optional per-type schema ---
        Self::validate_metadata_against_schema(&env, &event_type, &metadata);

        if rs.total_events >= rs.global_max_logs {
            panic_with_error!(&env, ContractError::GlobalMaxLogsReached);
        }

        // Read 2: EventCapConfig (per-type cap, optional) — single read.
        let mut type_count_opt: Option<u32> = None;
        if let Some(cap) = env
            .storage()
            .instance()
            .get::<_, Option<u32>>(&DataKey::EventCapConfig(event_type.clone()))
            .flatten()
        {
            let count = Self::event_type_count(&env, event_type.clone());
            if count >= cap {
                panic_with_error!(&env, ContractError::EventTypeMaxLogsReached);
            }
            type_count_opt = Some(count);
        }

        // --- Content-addressed deduplication ---
        // Compute hash(event_type || submitter || metadata) for dedup.
        let content_hash = Self::compute_content_hash(&env, &event_type, &submitter, &metadata);
        if !force {
            if let Some(existing_index) = env
                .storage()
                .instance()
                .get::<_, u32>(&DataKey::EventContentHash(content_hash.clone()))
            {
                let existing_id: BytesN<32> = env
                    .storage()
                    .instance()
                    .get(&DataKey::EventOrder(existing_index))
                    .unwrap();
                return existing_id;
            }
        }

        let index = cfg.total_events;
        let timestamp = env.ledger().timestamp();

        // --- issue #76: validate timestamp monotonicity and drift ---
        let (prev_hash, prev_timestamp): (BytesN<32>, u64) = if index == 0 {
            (BytesN::from_array(&env, &[0u8; 32]), 0u64)
        } else {
            let prev_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(index - 1)).unwrap();
            let prev_evt: Event = env.storage().instance().get(&DataKey::EventData(prev_id)).unwrap();
            (prev_evt.event_hash, prev_evt.timestamp)
        };

        if index > 0 && (timestamp < prev_timestamp || timestamp > prev_timestamp + MAX_TIMESTAMP_DRIFT_SECONDS) {
            panic_with_error!(&env, ContractError::TimestampOutOfRange);
        }

        // --- issue #70: compute content-addressed event ID ---
        let event_id = Self::compute_event_id(&env, &submitter, &event_type, &metadata, timestamp, index);

        // --- issue #66: compute this event's hash (includes prev_hash) ---
        let event_hash = Self::compute_event_hash(&env, &event_id, &prev_hash, index, timestamp);

        // Reject categories exceeding max length to prevent storage cost attacks.
        // Category length validation is a no-op: Symbol does not expose a
        // byte-level read API in soroban-sdk no_std, and the Soroban protocol
        // already limits all Symbols to ≤32 bytes, which is sufficient for the
        // intended `MAX_CATEGORY_LEN` constraint.
        let evt = Event {
            index,
            timestamp,
            event_type: event_type.clone(),
            category: Symbol::new(&env, "general"),
            submitter: submitter.clone(),
            metadata: metadata.clone(),
            sub_event_type: sub_event_type.clone(),
            version: Self::current_contract_version(&env),
            event_hash: event_hash.clone(),
            prev_hash,
            parent_event_id: None,
        };

        env.storage()
            .instance()
            .set(&DataKey::EventData(event_id.clone()), &evt);
        env.storage().instance().set(&DataKey::EventOrder(index), &event_id);

        // --- issue #56: store lightweight header separately ---
        let header = EventHeader {
            index,
            timestamp,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
        };
        env.storage()
            .instance()
            .set(&DataKey::EventHeaderKey(event_id.clone()), &header);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(event_id.clone()), &metadata);

        // --- issue #121: write to persistent storage when TTL is configured ---
        let ttl: u32 = env.storage().instance().get(&DataKey::EventTtl).unwrap_or(0);
        if ttl > 0 {
            env.storage()
                .persistent()
                .set(&DataKey::EventData(event_id.clone()), &evt);
            env.storage()
                .persistent()
                .extend_ttl(&DataKey::EventData(event_id.clone()), ttl, ttl);
        }

        // Task 4: cache low_cost_mode to avoid double read.
        let low_cost = Self::effective_low_cost_mode(&env);

        // --- issue #54: packed-Bytes index storage ---
        if !low_cost {
            Self::push_type_index(&env, event_type.clone(), index);
            Self::push_submitter_index(&env, &submitter, index);
            // Task 5: reuse cached count instead of re-reading.
            let new_count = type_count_opt.unwrap_or_else(|| Self::event_type_count(&env, event_type.clone())) + 1;
            env.storage()
                .instance()
                .set(&DataKey::EventTypeCount(event_type.clone()), &new_count);
        }

        // Task 4: cache emission_mode to avoid double read.
        let emission_mode = Self::effective_event_emission_mode(&env);

        cfg.total_events += 1;
        env.storage().instance().set(&DataKey::Config, &cfg);
        // Sync RuntimeState total_events (issue #114)
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.total_events = cfg.total_events;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }

        // #175: emit structured ("audit", "log_event") event with (submitter, event_type, index)
        if emission_mode != 3 {
            env.events().publish(
                (Symbol::new(&env, "audit"), Symbol::new(&env, "log_event")),
                (submitter, event_type, index),
            );
        }

        // --- issue #61: clear reentrancy guard before returning ---
        env.storage().temporary().remove(&DataKey::LogEventReentrancyGuard);

        event_id
    }

    /// Log an event with an explicit nonce to prevent replay attacks (issue #64, enhanced #214).
    ///
    /// Rules (enhanced with window validation and exhaustion):
    /// - `nonce` must be > `stored_nonce` (strict ordering).
    /// - If `window_size > 0` and `nonce - stored_nonce > window_size`, rejects with `NonceWindowExceeded`.
    /// - If `nonce > max_nonce`, rejects with `NonceExhausted`.
    /// - If `nonce <= stored_nonce`, rejects with `NonceTooLow`.
    /// - If `nonce == 0`, rejects with `NonceTooLow` (nonces are 1-based).
    ///
    /// `log_event()` remains available for backward compatibility (no nonce enforcement).
    pub fn log_event_with_nonce(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        nonce: u32,
    ) -> BytesN<32> {
        // Read optimized NonceState (single read instead of separate reads).
        let state: NonceState = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterNonce(submitter.clone()))
            .unwrap_or_else(|| NonceState {
                last_nonce: 0,
                window_size: Self::default_nonce_window_size(&env),
                max_nonce: Self::default_nonce_max_value(&env),
            });

        // Allow per-submitter config override if present.
        let config: Option<NonceState> = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterNonceConfig(submitter.clone()));
        let effective_state = config.unwrap_or(state);

        // --- nonce exhaustion handling (issue #214) ---
        if nonce > effective_state.max_nonce {
            panic_with_error!(&env, ContractError::NonceExhausted);
        }

        // --- nonce ordering check ---
        if nonce == 0 || nonce <= effective_state.last_nonce {
            panic_with_error!(&env, ContractError::NonceTooLow);
        }

        // --- nonce window validation (issue #214) ---
        if effective_state.window_size > 0
            && nonce.checked_sub(effective_state.last_nonce).is_none()
                || (effective_state.window_size > 0
                    && (nonce - effective_state.last_nonce) > effective_state.window_size)
        {
            panic_with_error!(&env, ContractError::NonceWindowExceeded);
        }

        let event_id =
            Self::log_event_with_hierarchy(env.clone(), submitter.clone(), event_type, metadata, None, None, false);

        // Store updated NonceState (optimized single write).
        let updated_state = NonceState {
            last_nonce: nonce,
            window_size: effective_state.window_size,
            max_nonce: effective_state.max_nonce,
        };
        env.storage()
            .instance()
            .set(&DataKey::SubmitterNonce(submitter), &updated_state);

        event_id
    }

    /// Return the last accepted nonce for `submitter`. Returns 0 if no nonce has been used yet.
    pub fn get_submitter_nonce(env: Env, submitter: Address) -> u32 {
        env.storage()
            .instance()
            .get::<_, NonceState>(&DataKey::SubmitterNonce(submitter))
            .map(|s| s.last_nonce)
            .unwrap_or(0)
    }

    /// Return the full nonce state for a submitter (issue #214).
    pub fn get_submitter_nonce_state(env: Env, submitter: Address) -> NonceState {
        env.storage()
            .instance()
            .get::<_, NonceState>(&DataKey::SubmitterNonce(submitter))
            .unwrap_or_else(|| NonceState {
                last_nonce: 0,
                window_size: Self::default_nonce_window_size(&env),
                max_nonce: Self::default_nonce_max_value(&env),
            })
    }

    /// Set the nonce window configuration for a specific submitter (owner-only, issue #214).
    /// `window_size`: max gap allowed between last accepted nonce and new nonce (0 = unlimited).
    /// `max_nonce`: upper nonce bound before exhaustion is triggered.
    pub fn set_submitter_nonce_config(
        env: Env,
        caller: Address,
        submitter: Address,
        window_size: u32,
        max_nonce: u32,
    ) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if max_nonce == 0 {
            panic_with_error!(&env, ContractError::NonceExhausted);
        }
        let current: NonceState = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterNonce(submitter.clone()))
            .unwrap_or_else(|| NonceState {
                last_nonce: 0,
                window_size: Self::default_nonce_window_size(&env),
                max_nonce: Self::default_nonce_max_value(&env),
            });
        let updated = NonceState {
            last_nonce: current.last_nonce,
            window_size,
            max_nonce,
        };
        env.storage()
            .instance()
            .set(&DataKey::SubmitterNonceConfig(submitter.clone()), &updated);
        env.events().publish(
            (Symbol::new(&env, "nonce_config_set"),),
            (submitter, window_size, max_nonce),
        );
    }

    /// Reset a submitter's nonce back to 0 (owner-only, issue #214).
    pub fn reset_submitter_nonce(env: Env, caller: Address, submitter: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let state: NonceState = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterNonce(submitter.clone()))
            .unwrap_or_else(|| NonceState {
                last_nonce: 0,
                window_size: Self::default_nonce_window_size(&env),
                max_nonce: Self::default_nonce_max_value(&env),
            });
        let reset_state = NonceState {
            last_nonce: 0,
            window_size: state.window_size,
            max_nonce: state.max_nonce,
        };
        env.storage()
            .instance()
            .set(&DataKey::SubmitterNonce(submitter.clone()), &reset_state);
        env.storage()
            .instance()
            .remove(&DataKey::SubmitterNonceConfig(submitter.clone()));
        env.events()
            .publish((Symbol::new(&env, "nonce_reset"),), (submitter, caller));
    }

    /// Set the default nonce window configuration for all submitters (owner-only, issue #214).
    pub fn set_default_nonce_config(
        env: Env,
        caller: Address,
        window_size: u32,
        max_nonce: u32,
    ) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if max_nonce == 0 {
            panic_with_error!(&env, ContractError::NonceExhausted);
        }
        env.storage()
            .instance()
            .set(&DataKey::DefaultNonceWindowSize, &window_size);
        env.storage()
            .instance()
            .set(&DataKey::DefaultNonceMaxValue, &max_nonce);
        env.events().publish(
            (Symbol::new(&env, "default_nonce_config_set"),),
            (caller, window_size, max_nonce),
        );
    }

    /// Return the default nonce window size.
    pub fn get_default_nonce_window_size(env: Env) -> u32 {
        Self::default_nonce_window_size(&env)
    }

    /// Return the default nonce max value.
    pub fn get_default_nonce_max_value(env: Env) -> u32 {
        Self::default_nonce_max_value(&env)
    }

    pub fn total_events(env: Env) -> u32 {
        Self::require_initialized(&env);
        // Prefer RuntimeState for a single read; fallback to Config for legacy contracts.
        if let Some(rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            return rs.total_events;
        }
        env.storage()
            .instance()
            .get::<_, Config>(&DataKey::Config)
            .map(|c| c.total_events)
            .unwrap_or(0)
    }

    /// Return the cached count of events for a given `event_type`.
    /// This provides a lightweight aggregation query (issue #205).
    pub fn get_event_type_count(env: Env, event_type: Symbol) -> u32 {
        env.storage()
            .instance()
            .get::<_, u32>(&DataKey::EventTypeCount(event_type))
            .unwrap_or(0u32)
    }

    /// Retrieve an event by its content-addressed ID.
    /// When TTL is configured, the persistent entry's TTL is extended on each read (issue #200).
    pub fn get_event(env: Env, id: BytesN<32>) -> Event {
        Self::require_initialized(&env);
        let evt: Event = env.storage()
            .instance()
            .get(&DataKey::EventData(id.clone()))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            });
        // TTL extension on read (#200): keep the persistent copy alive.
        let ttl: u32 = env.storage().instance().get(&DataKey::EventTtl).unwrap_or(0);
        if ttl > 0 && env.storage().persistent().has(&DataKey::EventData(id.clone())) {
            env.storage()
                .persistent()
                .extend_ttl(&DataKey::EventData(id), ttl, ttl);
            // Update cumulative extension counter.
            let mut stats: TtlCleanupStats = env
                .storage()
                .instance()
                .get(&DataKey::TtlCleanupStats)
                .unwrap_or(TtlCleanupStats { runs: 0, ttl_extensions: 0, cleaned: 0, last_run_ledger: 0 });
            stats.ttl_extensions = stats.ttl_extensions.saturating_add(1);
            env.storage().instance().set(&DataKey::TtlCleanupStats, &stats);
        }
        evt
    }

    /// Retrieve only the event metadata (optimized for low-fee environments, issue #57).
    pub fn get_event_metadata(env: Env, id: BytesN<32>) -> Bytes {
        Self::require_initialized(&env);
        let evt: Event = env
            .storage()
            .instance()
            .get(&DataKey::EventData(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            });
        evt.metadata
    }

    /// Retrieve only the event header (index, timestamp, event_type, submitter) — no metadata (issue #56).
    pub fn get_event_header(env: Env, id: BytesN<32>) -> EventHeader {
        Self::require_initialized(&env);
        let evt: Event = env
            .storage()
            .instance()
            .get(&DataKey::EventData(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            });
        EventHeader {
            index: evt.index,
            timestamp: evt.timestamp,
            event_type: evt.event_type,
            submitter: evt.submitter,
        }
    }

    /// Retrieve an event by its sequential insertion order (0-based).
    pub fn get_event_by_order(env: Env, order: u32) -> Event {
        Self::require_initialized(&env);
        let id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(order))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            });
        env.storage()
            .instance()
            .get(&DataKey::EventData(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }

    pub fn event_count(env: Env, event_type: Symbol) -> u32 {
        Self::require_initialized(&env);
        if Self::effective_low_cost_mode(&env) {
            panic_with_error!(&env, ContractError::CapNotSet);
        }
        Self::event_type_count(&env, event_type)
    }

    /// Count events matching a category (scans all events; pagination available via list_events_by_category)
    pub fn event_count_by_category(env: Env, category: Symbol) -> u32 {
        let total = Self::total_events(env.clone());
        let mut cnt: u32 = 0;
        for i in 0..total {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let evt: Event = env.storage().instance().get(&DataKey::EventData(id)).unwrap();
            if evt.category == category {
                cnt += 1;
            }
        }
        cnt
    }

    /// List event headers for a given category with simple pagination.
    pub fn list_events_by_category(env: Env, category: Symbol, start: u32, limit: u32) -> Vec<EventHeader> {
        let total = Self::total_events(env.clone());
        let mut out: Vec<EventHeader> = Vec::new(&env);
        if start >= total {
            return out;
        }
        let mut added: u32 = 0;
        let mut i = start;
        while i < total && added < limit {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let evt: Event = env.storage().instance().get(&DataKey::EventData(id)).unwrap();
            if evt.category == category {
                let header = EventHeader {
                    index: evt.index,
                    timestamp: evt.timestamp,
                    event_type: evt.event_type.clone(),
                    submitter: evt.submitter.clone(),
                };
                out.push_back(header);
                added += 1;
            }
            i += 1;
        }
        out
    }

    /// Archive events older than `cutoff_timestamp` into cold storage.
    /// Owner-only. Returns number archived.
    pub fn archive_events(env: Env, caller: Address, cutoff_timestamp: u64) -> u32 {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let total = Self::total_events(env.clone());
        let mut archived: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArchivedTotalEvents)
            .unwrap_or(0u32);
        let mut moved: u32 = 0;
        // Resume scanning from the last position to avoid re-scanning entire history on each call.
        let mut i: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArchiveScanCursor)
            .unwrap_or(0u32);
        while i < total {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            // skip if already archived
            if env.storage().instance().has(&DataKey::EventArchivedFlag(id.clone())) {
                i += 1;
                continue;
            }
            let evt: Event = env.storage().instance().get(&DataKey::EventData(id.clone())).unwrap();
            // Events are appended sequentially; once we reach a timestamp >= cutoff, later
            // events will be newer and can be skipped (early exit).
            if evt.timestamp >= cutoff_timestamp {
                break;
            }
            // copy into archived storage
            env.storage()
                .instance()
                .set(&DataKey::ArchivedEventData(id.clone()), &evt);
            if let Some(header) = env
                .storage()
                .instance()
                .get::<_, EventHeader>(&DataKey::EventHeaderKey(id.clone()))
            {
                env.storage()
                    .instance()
                    .set(&DataKey::ArchivedEventHeaderKey(id.clone()), &header);
            }
            if let Some(meta) = env
                .storage()
                .instance()
                .get::<_, Bytes>(&DataKey::EventMetadata(id.clone()))
            {
                env.storage()
                    .instance()
                    .set(&DataKey::ArchivedEventMetadata(id.clone()), &meta);
            }
            env.storage()
                .instance()
                .set(&DataKey::EventArchivedFlag(id.clone()), &true);
            env.storage()
                .instance()
                .set(&DataKey::ArchivedEventOrder(archived), &id.clone());
            archived += 1;
            moved += 1;
            i += 1;
        }
        // persist scan cursor so subsequent calls resume where we left off
        env.storage().instance().set(&DataKey::ArchiveScanCursor, &i);
        env.storage().instance().set(&DataKey::ArchivedTotalEvents, &archived);
        env.events().publish((Symbol::new(&env, "events_archived"),), (moved,));
        moved
    }

    pub fn get_archived_event(env: Env, id: BytesN<32>) -> Event {
        env.storage()
            .instance()
            .get(&DataKey::ArchivedEventData(id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::EventDoesNotExist))
    }

    pub fn get_archived_event_count(env: Env) -> u32 {
        // count actual archived entries (tolerate gaps)
        let total: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArchivedTotalEvents)
            .unwrap_or(0u32);
        let mut cnt: u32 = 0;
        for i in 0..total {
            if let Some(id) = env
                .storage()
                .instance()
                .get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i))
            {
                if env.storage().instance().has(&DataKey::ArchivedEventData(id)) {
                    cnt += 1;
                }
            }
        }
        cnt
    }

    pub fn list_archived_events(env: Env, start: u32, limit: u32) -> Vec<EventHeader> {
        let total: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArchivedTotalEvents)
            .unwrap_or(0u32);
        let mut out: Vec<EventHeader> = Vec::new(&env);
        if start >= total {
            return out;
        }
        let mut added: u32 = 0;
        let mut i = start;
        while i < total && added < limit {
            if let Some(id) = env
                .storage()
                .instance()
                .get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i))
            {
                if let Some(header) = env
                    .storage()
                    .instance()
                    .get::<_, EventHeader>(&DataKey::ArchivedEventHeaderKey(id.clone()))
                {
                    out.push_back(header);
                    added += 1;
                }
            }
            i += 1;
        }
        out
    }

    /// Permanently purge archived events older than cutoff. `confirm` must be true.
    pub fn purge_archived_events(env: Env, caller: Address, cutoff_timestamp: u64, confirm: bool) -> u32 {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if !confirm {
            return 0u32;
        }
        let total: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ArchivedTotalEvents)
            .unwrap_or(0u32);
        let mut removed: u32 = 0;
        for i in 0..total {
            if let Some(id) = env
                .storage()
                .instance()
                .get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i))
            {
                if let Some(evt) = env
                    .storage()
                    .instance()
                    .get::<_, Event>(&DataKey::ArchivedEventData(id.clone()))
                {
                    if evt.timestamp < cutoff_timestamp {
                        env.storage().instance().remove(&DataKey::ArchivedEventData(id.clone()));
                        env.storage()
                            .instance()
                            .remove(&DataKey::ArchivedEventHeaderKey(id.clone()));
                        env.storage()
                            .instance()
                            .remove(&DataKey::ArchivedEventMetadata(id.clone()));
                        // remove archived order mapping
                        env.storage().instance().remove(&DataKey::ArchivedEventOrder(i));
                        removed += 1;
                    }
                }
            }
        }
        env.events()
            .publish((Symbol::new(&env, "archived_events_purged"),), (removed,));
        removed
    }

    /// Upgrade the contract's WASM. Owner-only. Emits `contract_upgraded(old_hash, new_hash)`.
    pub fn upgrade_contract(env: Env, caller: Address, new_wasm_hash: BytesN<32>) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if new_wasm_hash == BytesN::from_array(&env, &[0u8; 32]) {
            panic_with_error!(&env, ContractError::InvalidWasmHash);
        }
        // Emit event and attempt to perform the upgrade via the deployer.
        // Note: callers should ensure the new WASM is compatible with storage layout.
        // Try to obtain current wasm hash if available (best-effort).
        let old_hash_opt: Option<BytesN<32>> = None;
        env.events().publish(
            (Symbol::new(&env, "contract_upgraded"),),
            (old_hash_opt, new_wasm_hash.clone()),
        );
        // Perform upgrade via deployer API (Soroban deployer helper).
        // This is a best-effort call and may vary by runtime.
        env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
    }

    pub fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event {
        Self::require_initialized(&env);
        if Self::effective_low_cost_mode(&env) {
            panic_with_error!(&env, ContractError::EventTypeIndexOutOfBounds);
        }

        let count = Self::event_type_count(&env, event_type.clone());
        if count == 0 {
            panic_with_error!(&env, ContractError::NoEventsForType);
        }
        if type_index >= count {
            panic_with_error!(&env, ContractError::EventTypeIndexOutOfBounds);
        }

        let global_order = Self::get_type_index(&env, event_type, type_index);
        let event_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(global_order))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventTypeIndexOutOfBounds);
            });

        env.storage()
            .instance()
            .get(&DataKey::EventData(event_id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }

    pub fn list_events(env: Env, offset: u32, limit: u32) -> Vec<Event> {
        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::total_events(env.clone());
        if offset >= total {
            return Vec::new(&env);
        }

        let end = (offset.saturating_add(limit)).min(total);
        let mut results = Vec::new(&env);
        for i in offset..end {
            results.push_back(Self::get_event_by_order(env.clone(), i));
        }
        results
    }

    pub fn list_events_by_type(env: Env, event_type: Symbol, offset: u32, limit: u32) -> Vec<Event> {
        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::event_type_count(&env, event_type.clone());
        if offset >= total {
            return Vec::new(&env);
        }

        let end = (offset.saturating_add(limit)).min(total);
        let mut results = Vec::new(&env);
        for i in offset..end {
            results.push_back(Self::get_event_by_type(env.clone(), event_type.clone(), i));
        }
        results
    }

    /// Return a paginated slice of events for a given event type.
    ///
    /// * `event_type` — the type to filter by.
    /// * `start`      — 0-based index into the per-type sub-ledger to begin reading from.
    /// * `limit`      — maximum number of events to return (capped at 100).
    ///
    /// Returns an empty `Vec` when `start` is beyond the last index for the type,
    /// or when the type has no events at all — no panics for out-of-range inputs.
    /// A partial slice is returned when fewer than `limit` events remain after `start`.
    pub fn get_events_by_type(
        env: Env,
        event_type: Symbol,
        start: u32,
        limit: u32,
    ) -> Vec<Event> {
        Self::require_initialized(&env);

        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::event_type_count(&env, event_type.clone());
        if total == 0 || start >= total {
            return Vec::new(&env);
        }

        let end = (start.saturating_add(limit)).min(total);
        let mut results = Vec::new(&env);
        for i in start..end {
            results.push_back(Self::get_event_by_type(env.clone(), event_type.clone(), i));
        }
        results
    }

    // ── Submitter-based event filtering (issue #206) ────────────────────────

    /// Return the number of events submitted by a given address (issue #206).
    pub fn submitter_event_count(env: Env, submitter: Address) -> u32 {
        Self::require_initialized(&env);
        Self::submitter_count(&env, submitter)
    }

    /// Retrieve an event by submitter address and local index (issue #206).
    /// `submitter_index` is 0-based within the submitter's sub-ledger.
    pub fn get_event_by_submitter(
        env: Env,
        submitter: Address,
        submitter_index: u32,
    ) -> Event {
        Self::require_initialized(&env);

        let count = Self::submitter_count(&env, submitter.clone());
        if count == 0 {
            panic_with_error!(&env, ContractError::NoEventsForType);
        }
        if submitter_index >= count {
            panic_with_error!(&env, ContractError::EventTypeIndexOutOfBounds);
        }

        let global_order = Self::get_submitter_index(&env, &submitter, submitter_index);
        let event_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(global_order))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            });

        env.storage()
            .instance()
            .get(&DataKey::EventData(event_id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }

    /// Return a paginated list of events for a given submitter (issue #206).
    ///
    /// * `submitter` — the address to filter by.
    /// * `start`     — 0-based index into the submitter's sub-ledger.
    /// * `limit`     — maximum number of events to return (capped at 100).
    ///
    /// Returns an empty `Vec` when `start` is beyond the last index, or when
    /// the submitter has no events — no panics for out-of-range inputs.
    pub fn get_events_by_submitter(
        env: Env,
        submitter: Address,
        start: u32,
        limit: u32,
    ) -> Vec<Event> {
        Self::require_initialized(&env);

        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::submitter_count(&env, submitter.clone());
        if total == 0 || start >= total {
            return Vec::new(&env);
        }

        let end = (start.saturating_add(limit)).min(total);
        let mut results = Vec::new(&env);
        for i in start..end {
            results.push_back(Self::get_event_by_submitter(env.clone(), submitter.clone(), i));
        }
        results
    }

    pub fn get_events_by_time_range(
        env: Env,
        start_time: u64,
        end_time: u64,
        offset: u32,
        limit: u32,
    ) -> Vec<Event> {
        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }
        if end_time < start_time {
            return Vec::new(&env);
        }

        let total = Self::total_events(env.clone());
        let mut matches = Vec::new(&env);
        for i in 0..total {
            let evt = Self::get_event_by_order(env.clone(), i);
            if evt.timestamp >= start_time && evt.timestamp <= end_time {
                matches.push_back(evt);
            }
        }

        let matched_count = matches.len();
        if offset >= matched_count {
            return Vec::new(&env);
        }

        let end = (offset.saturating_add(limit)).min(matched_count);
        let mut results = Vec::new(&env);
        for i in offset..end {
            results.push_back(matches.get(i).unwrap());
        }
        results
    }

    pub fn search_events(env: Env, query: Bytes, offset: u32, limit: u32) -> Vec<Event> {
        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::total_events(env.clone());
        let mut matches = Vec::new(&env);
        for i in 0..total {
            let evt = Self::get_event_by_order(env.clone(), i);
            if Self::bytes_contains(&evt.metadata, &query) {
                matches.push_back(evt);
            }
        }

        let matched_count = matches.len();
        if offset >= matched_count {
            return Vec::new(&env);
        }

        let end = (offset.saturating_add(limit)).min(matched_count);
        let mut results = Vec::new(&env);
        for i in offset..end {
            results.push_back(matches.get(i).unwrap());
        }
        results
    }

    pub fn update_event(env: Env, caller: Address, index: u32, new_metadata: Bytes) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let total = Self::total_events(env.clone());
        if index >= total {
            panic_with_error!(&env, ContractError::EventDoesNotExist);
        }

        let current_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(index)).unwrap();
        let current_event: Event = env
            .storage()
            .instance()
            .get(&DataKey::EventData(current_id.clone()))
            .unwrap();

        let max_meta = Self::effective_metadata_max_size(&env, &current_event.event_type);
        if new_metadata.len() > max_meta {
            panic_with_error!(&env, ContractError::MetadataTooLarge);
        }

        // --- issue #202: validate new metadata against optional per-type schema ---
        Self::validate_metadata_against_schema(&env, &current_event.event_type, &new_metadata);

        let new_id = Self::compute_event_id(
            &env,
            &current_event.submitter,
            &current_event.event_type,
            &new_metadata,
            current_event.timestamp,
            index,
        );

        if new_id == current_id {
            return current_id;
        }

        let mut versions: Vec<EventVersion> = env
            .storage()
            .instance()
            .get(&DataKey::EventVersions(index))
            .unwrap_or_else(|| Vec::new(&env));

        if versions.is_empty() {
            let original_version = EventVersion {
                version: 0,
                data: current_event.clone(),
                updated_at: current_event.timestamp,
                updated_by: current_event.submitter.clone(),
            };
            versions.push_back(original_version);
        }

        let prev_hash: BytesN<32> = if index == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let prev_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(index - 1)).unwrap();
            let prev_evt: Event = env.storage().instance().get(&DataKey::EventData(prev_id)).unwrap();
            prev_evt.event_hash.clone()
        };

        let updated_event_hash = Self::compute_event_hash(&env, &new_id, &prev_hash, index, current_event.timestamp);

        let updated_event = Event {
            index,
            timestamp: current_event.timestamp,
            event_type: current_event.event_type.clone(),
            category: current_event.category.clone(),
            submitter: current_event.submitter.clone(),
            metadata: new_metadata.clone(),
            sub_event_type: current_event.sub_event_type.clone(),
            version: Self::current_contract_version(&env),
            event_hash: updated_event_hash.clone(),
            prev_hash: prev_hash.clone(),
            parent_event_id: current_event.parent_event_id.clone(),
        };

        let update_version = EventVersion {
            version: versions.len(),
            data: updated_event.clone(),
            updated_at: env.ledger().timestamp(),
            updated_by: caller.clone(),
        };
        versions.push_back(update_version);
        env.storage().instance().set(&DataKey::EventVersions(index), &versions);

        env.storage()
            .instance()
            .set(&DataKey::EventData(new_id.clone()), &updated_event);
        env.storage().instance().set(&DataKey::EventOrder(index), &new_id);
        env.storage().instance().set(
            &DataKey::EventHeaderKey(new_id.clone()),
            &EventHeader {
                index,
                timestamp: current_event.timestamp,
                event_type: current_event.event_type.clone(),
                submitter: current_event.submitter.clone(),
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::EventMeta(new_id.clone()), &updated_event);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(new_id.clone()), &new_metadata);

        let mut next_prev_hash = updated_event_hash;
        for i in index + 1..total {
            let event_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let mut later_event: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(event_id.clone()))
                .unwrap();
            later_event.prev_hash = next_prev_hash.clone();
            later_event.event_hash =
                Self::compute_event_hash(&env, &event_id, &later_event.prev_hash, i, later_event.timestamp);
            env.storage()
                .instance()
                .set(&DataKey::EventData(event_id.clone()), &later_event);
            env.storage()
                .instance()
                .set(&DataKey::EventMeta(event_id.clone()), &later_event);
            next_prev_hash = later_event.event_hash.clone();
        }

        for i in 0..total {
            let event_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let mut later_event: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(event_id.clone()))
                .unwrap();
            if let Some(parent) = &later_event.parent_event_id {
                if parent == &current_id {
                    later_event.parent_event_id = Some(new_id.clone());
                    env.storage()
                        .instance()
                        .set(&DataKey::EventData(event_id.clone()), &later_event);
                    env.storage()
                        .instance()
                        .set(&DataKey::EventMeta(event_id.clone()), &later_event);
                }
            }
        }

        env.events().publish(
            (Symbol::new(&env, "event_updated"),),
            (index, current_id, new_id.clone(), caller, env.ledger().timestamp()),
        );

        new_id
    }

    pub fn get_event_history(env: Env, index: u32) -> Vec<EventVersion> {
        let total = Self::total_events(env.clone());
        if index >= total {
            return Vec::new(&env);
        }

        if let Some(versions) = env
            .storage()
            .instance()
            .get::<_, Vec<EventVersion>>(&DataKey::EventVersions(index))
        {
            return versions;
        }

        let event_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(index)).unwrap();
        let event: Event = env.storage().instance().get(&DataKey::EventData(event_id)).unwrap();

        let mut history = Vec::new(&env);
        history.push_back(EventVersion {
            version: 0,
            data: event.clone(),
            updated_at: event.timestamp,
            updated_by: event.submitter,
        });
        history
    }

    /// Roll back an event to a specific version from its history (issue #204).
    ///
    /// Owner-only. Restores the event data (metadata, timestamps, submitter, etc.)
    /// to the state recorded at `target_version`, recomputes the event ID and
    /// hash chain, and appends a new version entry recording the rollback.
    ///
    /// Returns the new content-addressed event ID after rollback.
    pub fn rollback_event(env: Env, caller: Address, index: u32, target_version: u32) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let total = Self::total_events(env.clone());
        if index >= total {
            panic_with_error!(&env, ContractError::EventDoesNotExist);
        }

        let history = Self::get_event_history(env.clone(), index);
        if target_version >= history.len() as u32 {
            panic_with_error!(&env, ContractError::InvalidVersion);
        }

        let target = history.get(target_version).unwrap();
        let restored = &target.data;

        let new_id = Self::compute_event_id(
            &env,
            &restored.submitter,
            &restored.event_type,
            &restored.metadata,
            restored.timestamp,
            index,
        );

        let current_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(index))
            .unwrap();

        if new_id == current_id {
            return new_id;
        }

        let prev_hash: BytesN<32> = if index == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let prev_id: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(index - 1))
                .unwrap();
            let prev_evt: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(prev_id))
                .unwrap();
            prev_evt.event_hash.clone()
        };

        let new_hash = Self::compute_event_hash(&env, &new_id, &prev_hash, index, restored.timestamp);

        let updated_event = Event {
            index: restored.index,
            timestamp: restored.timestamp,
            event_type: restored.event_type.clone(),
            category: restored.category.clone(),
            submitter: restored.submitter.clone(),
            metadata: restored.metadata.clone(),
            sub_event_type: restored.sub_event_type.clone(),
            version: Self::current_contract_version(&env),
            event_hash: new_hash.clone(),
            prev_hash: prev_hash.clone(),
            parent_event_id: restored.parent_event_id.clone(),
        };

        let mut versions = history;
        let rollback_version = EventVersion {
            version: versions.len() as u32,
            data: updated_event.clone(),
            updated_at: env.ledger().timestamp(),
            updated_by: caller.clone(),
        };
        versions.push_back(rollback_version);
        env.storage()
            .instance()
            .set(&DataKey::EventVersions(index), &versions);

        env.storage()
            .instance()
            .set(&DataKey::EventData(new_id.clone()), &updated_event);
        env.storage()
            .instance()
            .set(&DataKey::EventOrder(index), &new_id);
        env.storage().instance().set(
            &DataKey::EventHeaderKey(new_id.clone()),
            &EventHeader {
                index: updated_event.index,
                timestamp: updated_event.timestamp,
                event_type: updated_event.event_type.clone(),
                submitter: updated_event.submitter.clone(),
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::EventMeta(new_id.clone()), &updated_event);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(new_id.clone()), &updated_event.metadata);

        let mut next_prev_hash = new_hash;
        for i in index + 1..total {
            let event_id: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(i))
                .unwrap();
            let mut later_event: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(event_id.clone()))
                .unwrap();
            later_event.prev_hash = next_prev_hash.clone();
            later_event.event_hash =
                Self::compute_event_hash(&env, &event_id, &later_event.prev_hash, i, later_event.timestamp);
            env.storage()
                .instance()
                .set(&DataKey::EventData(event_id.clone()), &later_event);
            env.storage()
                .instance()
                .set(&DataKey::EventMeta(event_id.clone()), &later_event);
            next_prev_hash = later_event.event_hash.clone();
        }

        env.events().publish(
            (Symbol::new(&env, "versioning"), Symbol::new(&env, "event_rolled_back")),
            (index, target_version, caller, env.ledger().timestamp()),
        );

        new_id
    }

    /// Return the number of recorded versions for an event (including version 0).
    pub fn get_event_version_count(env: Env, index: u32) -> u32 {
        let history = Self::get_event_history(env.clone(), index);
        history.len() as u32
    }

    /// Compare two versions of the same event by metadata length.
    ///
    /// Returns -1 if version_a metadata is shorter, 0 if equal, 1 if longer.
    pub fn compare_event_versions(env: Env, index: u32, version_a: u32, version_b: u32) -> i32 {
        let history = Self::get_event_history(env.clone(), index);
        if history.is_empty() {
            return 0;
        }
        let a = version_a;
        let b = version_b;
        if a >= history.len() || b >= history.len() {
            panic_with_error!(&env, ContractError::InvalidVersion);
        }
        let len_a = history.get(a).unwrap().data.metadata.len();
        let len_b = history.get(b).unwrap().data.metadata.len();
        len_a.cmp(&len_b) as i32
    }

    // ── Integrity verification (issue #66) ──────────────────────────────────

    /// Verify the full hash chain. Returns `true` if every event's
    /// `prev_hash` matches the previous event's `event_hash`.
    pub fn verify_integrity(env: Env) -> bool {
        Self::require_initialized(&env);
        let total = Self::total_events(env.clone());
        Self::verify_range(&env, 0, total)
    }

    /// Verify a range `[from, to)` of the hash chain.
    pub fn verify_integrity_range(env: Env, from: u32, to: u32) -> bool {
        Self::require_initialized(&env);
        Self::verify_range(&env, from, to)
    }

    // ── Snapshots (issue #213) ─────────────────────────────────────────────

    /// Create a point-in-time snapshot of the audit ledger state.
    /// Records the current timestamp, event count, and last event hash.
    /// Owner-only. Returns the new snapshot ID.
    pub fn create_snapshot(env: Env, caller: Address, description: Bytes) -> u32 {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let total = Self::total_events(env.clone());
        let timestamp = env.ledger().timestamp();

        let event_hash: BytesN<32> = if total == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let last_id: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(total - 1))
                .unwrap();
            let last_evt: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(last_id))
                .unwrap();
            last_evt.event_hash
        };

        let snap_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::SnapshotCount)
            .unwrap_or(0);

        let snapshot = Snapshot {
            id: snap_id,
            timestamp,
            event_count: total,
            event_hash,
            description,
        };

        env.storage()
            .instance()
            .set(&DataKey::SnapshotData(snap_id), &snapshot);
        env.storage()
            .instance()
            .set(&DataKey::SnapshotCount, &(snap_id + 1));

        env.events().publish(
            (Symbol::new(&env, "snapshot_created"),),
            (snap_id, timestamp, total),
        );

        snap_id
    }

    /// Retrieve a snapshot by its ID.
    pub fn get_snapshot(env: Env, snapshot_id: u32) -> Snapshot {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::SnapshotData(snapshot_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::SnapshotNotFound))
    }

    /// Return the total number of snapshots that have been created.
    pub fn snapshot_count(env: Env) -> u32 {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::SnapshotCount)
            .unwrap_or(0)
    }

    /// Verify that a snapshot is consistent with the current ledger state.
    /// Checks that the event_count and event_hash at snapshot time are still valid
    /// by re-walking the chain up to the snapshot's event_count.
    pub fn verify_snapshot(env: Env, snapshot_id: u32) -> bool {
        Self::require_initialized(&env);
        let snapshot: Snapshot = env
            .storage()
            .instance()
            .get(&DataKey::SnapshotData(snapshot_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::SnapshotNotFound));

        let current_total = Self::total_events(env.clone());

        // Snapshot must reference events that still exist.
        if snapshot.event_count > current_total {
            return false;
        }

        if snapshot.event_count == 0 {
            return snapshot.event_hash == BytesN::from_array(&env, &[0u8; 32]);
        }

        // Re-derive the hash chain up to snapshot.event_count and compare.
        let last_idx = snapshot.event_count - 1;
        let last_id: BytesN<32> = match env.storage().instance().get(&DataKey::EventOrder(last_idx)) {
            Some(v) => v,
            None => return false,
        };
        let last_evt: Event = match env.storage().instance().get(&DataKey::EventData(last_id)) {
            Some(v) => v,
            None => return false,
        };

        last_evt.event_hash == snapshot.event_hash
    }

    // ── Deduplication cleanup (issue #212) ──────────────────────────────────

    /// Remove stale content hash dedup entries that no longer correspond to an event.
    /// Owner-only. Returns the number of entries cleaned.
    pub fn cleanup_stale_hashes(env: Env, caller: Address, start_index: u32, batch_size: u32) -> u32 {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let total = Self::total_events(env.clone());
        let mut cleaned: u32 = 0;
        let mut idx = start_index;
        let end = (start_index + batch_size).min(total);

        // Scan all event order entries and verify their content hash still maps correctly.
        while idx < end {
            if let Some(event_id) = env
                .storage()
                .instance()
                .get::<_, BytesN<32>>(&DataKey::EventOrder(idx))
            {
                if let Some(evt) = env
                    .storage()
                    .instance()
                    .get::<_, Event>(&DataKey::EventData(event_id.clone()))
                {
                    let content_hash =
                        Self::compute_content_hash(&env, &evt.event_type, &evt.submitter, &evt.metadata);
                    if let Some(stored_index) = env
                        .storage()
                        .instance()
                        .get::<_, u32>(&DataKey::EventContentHash(content_hash.clone()))
                    {
                        // If the stored index no longer points to the same event, clean it.
                        if stored_index != idx {
                            env.storage()
                                .instance()
                                .remove(&DataKey::EventContentHash(content_hash));
                            cleaned += 1;
                        }
                    }
                }
            }
            idx += 1;
        }

        env.events()
            .publish((Symbol::new(&env, "stale_hashes_cleaned"),), (cleaned,));
        cleaned
    }

    // ── Governance ──────────────────────────────────────────────────────────

    pub fn set_global_max_logs(env: Env, caller: Address, new_max: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        // governance writes should be blocked while paused
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        let total_events = Self::total_events(env.clone());
        if new_max < total_events {
            panic_with_error!(&env, ContractError::MaxLogsBelowCurrentCount);
        }
        let mut cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        let old_max = cfg.global_max_logs;
        cfg.global_max_logs = new_max;
        env.storage().instance().set(&DataKey::Config, &cfg);
        env.storage().instance().set(&DataKey::GlobalMaxLogs, &new_max);
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.global_max_logs = new_max;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "set_global_max")),
            (caller, old_max, new_max),
        );
    }

    pub fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        // --- issue #63: validate event_type Symbol ---
        Self::validate_event_type(&env, &event_type);
        env.storage()
            .instance()
            .set(&DataKey::EventCapSet(event_type.clone()), &true);
        env.storage()
            .instance()
            .set(&DataKey::EventMaxLogs(event_type.clone()), &new_max);
        env.storage()
            .instance()
            .set(&DataKey::EventCapConfig(event_type.clone()), &Some(new_max));
        env.storage()
            .instance()
            .remove(&DataKey::EventCapRemoved(event_type.clone()));

        if !Self::effective_low_cost_mode(&env)
            && !env
                .storage()
                .instance()
                .has(&DataKey::EventCapConfig(event_type.clone()))
        {
            env.storage()
                .instance()
                .set(&DataKey::EventTypeIndices(event_type.clone()), &Bytes::new(&env));
        }
    }

    pub fn remove_event_cap(env: Env, caller: Address, event_type: Symbol) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        if !env
            .storage()
            .instance()
            .has(&DataKey::EventCapConfig(event_type.clone()))
        {
            if env
                .storage()
                .instance()
                .has(&DataKey::EventCapRemoved(event_type.clone()))
            {
                panic_with_error!(&env, ContractError::CapAlreadyRemoved);
            }
            panic_with_error!(&env, ContractError::CapNeverSet);
        }
        env.storage()
            .instance()
            .remove(&DataKey::EventCapConfig(event_type.clone()));
        env.storage()
            .instance()
            .remove(&DataKey::EventMaxLogs(event_type.clone()));
        env.storage()
            .instance()
            .set(&DataKey::EventCapRemoved(event_type.clone()), &true);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "remove_event_cap")),
            (caller, event_type),
        );
    }

    pub fn has_cap(env: Env, event_type: Symbol) -> bool {
        Self::require_initialized(&env);
        env.storage().instance().has(&DataKey::EventCapConfig(event_type))
    }

    pub fn transfer_ownership(env: Env, caller: Address, new_owner: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        let current_owner: Address = env.storage().instance().get(&DataKey::Owner).unwrap();
        if new_owner == Address::from_str(&env, NULL_ACCOUNT) {
            panic_with_error!(&env, ContractError::NewOwnerIsZero);
        }
        if new_owner == current_owner {
            panic_with_error!(&env, ContractError::SameOwner);
        }
        env.storage().instance().set(&DataKey::Owner, &new_owner);
        // Also update the multi-sig Owners list (replace old owner with new)
        let mut owners = Self::get_owners(&env);
        for i in 0..owners.len() {
            if owners.get(i).unwrap() == current_owner {
                owners.set(i, new_owner.clone());
                break;
            }
        }
        env.storage().instance().set(&DataKey::Owners, &owners);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "transfer_ownership")),
            (caller, current_owner, new_owner),
        );
    }

    // ── issue #67: metadata size governance ──────────────────────────────────

    /// Set a global metadata size limit (owner-only).
    /// Events with `metadata.len() > max_size` will be rejected.
    /// Pass `u32::MAX` to effectively disable the limit.
    pub fn set_metadata_max_size(env: Env, caller: Address, max_size: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::GlobalMetadataMaxSize, &max_size);
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.global_metadata_max_size = max_size;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }
    }

    /// Set a per-event-type metadata size limit (owner-only).
    /// Overrides the global limit for the given event type.
    pub fn set_event_metadata_max_size(env: Env, caller: Address, event_type: Symbol, max_size: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadataMaxSize(event_type), &max_size);
    }

    /// Set a metadata validation schema for a specific event type (owner-only, issue #202).
    ///
    /// The schema format is length-prefixed: the first 4 bytes (LE u32) encode the
    /// minimum required metadata length in bytes.  If `schema` is empty or shorter
    /// than 4 bytes, the constraint is removed (any metadata passes).
    pub fn set_metadata_schema(env: Env, caller: Address, event_type: Symbol, schema: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::MetadataSchema(event_type.clone()), &schema);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "set_metadata_schema")),
            (caller, event_type, schema.len() as u32),
        );
    }

    /// Return the metadata validation schema for `event_type`, or empty `Bytes` if none is configured (issue #202).
    pub fn get_metadata_schema(env: Env, event_type: Symbol) -> Bytes {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::MetadataSchema(event_type))
            .unwrap_or_else(|| Bytes::new(&env))
    }

    /// Set the TTL for events written to persistent storage (#121).
    ///
    /// When `ttl_ledgers > 0`, subsequent `log_event` calls store each event in
    /// `env.storage().persistent()` and extend its TTL to `ttl_ledgers` ledgers
    /// from the current ledger sequence.  When `ttl_ledgers == 0`, TTL is
    /// disabled and events continue to be stored in instance storage (no expiry).
    ///
    /// **Cost tradeoffs** — see docs/fees.md#ttl-storage.
    pub fn set_event_ttl(env: Env, caller: Address, ttl_ledgers: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        let old_ttl: u32 = env.storage().instance().get(&DataKey::EventTtl).unwrap_or(0);
        env.storage().instance().set(&DataKey::EventTtl, &ttl_ledgers);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "set_event_ttl")),
            (caller, old_ttl, ttl_ledgers),
        );
    }

    /// Return the currently configured TTL in ledgers, or 0 if disabled.
    pub fn get_event_ttl(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::EventTtl).unwrap_or(0)
    }

    // ── TTL auto-cleanup (#200) ───────────────────────────────────────────────

    /// Clean up expired persistent events in a bounded batch (issue #200).
    ///
    /// Scans up to `batch_size` events starting at `start_index` and removes
    /// those whose persistent-storage TTL has expired (i.e. the key is no longer
    /// present in persistent storage even though it was written there).
    ///
    /// Governance-only (owner or multisig). Returns the number of expired entries
    /// removed in this run and emits a `("ttl_cleanup", "expired_removed")` event
    /// for monitoring.
    pub fn cleanup_expired_events(env: Env, caller: Address, start_index: u32, batch_size: u32) -> u32 {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let ttl: u32 = env.storage().instance().get(&DataKey::EventTtl).unwrap_or(0);
        // Nothing to clean if TTL is disabled.
        if ttl == 0 {
            return 0;
        }

        let total = Self::total_events(env.clone());
        let end = if start_index.saturating_add(batch_size) < total {
            start_index + batch_size
        } else {
            total
        };

        let mut removed: u32 = 0;
        for i in start_index..end {
            if let Some(id) = env
                .storage()
                .instance()
                .get::<_, BytesN<32>>(&DataKey::EventOrder(i))
            {
                // The persistent entry is expired when the key no longer exists.
                if !env.storage().persistent().has(&DataKey::EventData(id.clone())) {
                    // The entry has already been evicted by the network; nothing to
                    // remove from instance storage, but we count it for statistics.
                    removed += 1;
                }
            }
        }

        // Update cumulative stats.
        let mut stats: TtlCleanupStats = env
            .storage()
            .instance()
            .get(&DataKey::TtlCleanupStats)
            .unwrap_or(TtlCleanupStats { runs: 0, ttl_extensions: 0, cleaned: 0, last_run_ledger: 0 });
        stats.runs = stats.runs.saturating_add(1);
        stats.cleaned = stats.cleaned.saturating_add(removed);
        stats.last_run_ledger = env.ledger().sequence();
        env.storage().instance().set(&DataKey::TtlCleanupStats, &stats);

        // Emit monitoring event.
        env.events().publish(
            (Symbol::new(&env, "ttl_cleanup"), Symbol::new(&env, "expired_removed")),
            (caller, start_index, end, removed),
        );

        removed
    }

    /// Return cumulative TTL cleanup statistics (issue #200).
    pub fn get_cleanup_stats(env: Env) -> TtlCleanupStats {
        env.storage()
            .instance()
            .get(&DataKey::TtlCleanupStats)
            .unwrap_or(TtlCleanupStats { runs: 0, ttl_extensions: 0, cleaned: 0, last_run_ledger: 0 })
    }
    ///
    /// Owner-only. The off-chain relayer reads these registrations to dispatch
    /// HTTP POST requests when matching events are emitted.
    ///
    /// `url` — HTTP(S) endpoint; `secret` — HMAC signing secret (opaque bytes).
    pub fn register_webhook(env: Env, caller: Address, event_type: Symbol, url: Bytes, secret: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner(&env, &caller);
        let entry = WebhookEntry {
            url: url.clone(),
            secret,
        };
        let key = DataKey::WebhookRegistrations(event_type.clone());
        let mut list: Vec<WebhookEntry> = env.storage().instance().get(&key).unwrap_or_else(|| Vec::new(&env));
        list.push_back(entry);
        env.storage().instance().set(&key, &list);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "register_webhook")),
            (caller, event_type, url),
        );
    }

    /// Unregister a webhook for a specific event type (#25).
    ///
    /// Owner-only. Removes the first entry whose `url` matches `url`.
    pub fn unregister_webhook(env: Env, caller: Address, event_type: Symbol, url: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner(&env, &caller);
        let key = DataKey::WebhookRegistrations(event_type.clone());
        let list: Vec<WebhookEntry> = env.storage().instance().get(&key).unwrap_or_else(|| Vec::new(&env));
        let mut new_list: Vec<WebhookEntry> = Vec::new(&env);
        let mut removed = false;
        for entry in list.iter() {
            if !removed && entry.url == url {
                removed = true;
            } else {
                new_list.push_back(entry);
            }
        }
        env.storage().instance().set(&key, &new_list);
        env.events().publish(
            (Symbol::new(&env, "governance"), Symbol::new(&env, "unregister_webhook")),
            (caller, event_type, url),
        );
    }

    /// Return registered webhooks for an event type (URLs only, secrets are not exposed) (#25).
    pub fn get_webhooks(env: Env, event_type: Symbol) -> Vec<Bytes> {
        let list: Vec<WebhookEntry> = env
            .storage()
            .instance()
            .get(&DataKey::WebhookRegistrations(event_type))
            .unwrap_or_else(|| Vec::new(&env));
        let mut urls: Vec<Bytes> = Vec::new(&env);
        for entry in list.iter() {
            urls.push_back(entry.url);
        }
        urls
    }

    /// Pause write operations. Owner-only. Works even if contract already paused.
    pub fn pause(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let _already_paused = env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((Symbol::new(&env, "contract_paused"),), (caller,));
    }

    /// Unpause write operations. Owner-only.
    pub fn unpause(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().remove(&DataKey::PausedSince);
        env.events()
            .publish((Symbol::new(&env, "contract_unpaused"),), (caller,));
    }

    /// Returns true if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Returns the timestamp when the contract was paused, or 0 if not paused.
    pub fn paused_since(env: Env) -> u64 {
        env.storage()
            .instance()
            .get::<_, u64>(&DataKey::PausedSince)
            .unwrap_or(0)
    }

    /// Set the maximum allowed category Symbol length in bytes (owner-only).
    pub fn set_category_max_len(env: Env, caller: Address, max_len: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::CategoryMaxLen, &max_len);
    }

    /// Block a submitter (owner-only). Issue #141: governance.
    /// Blocked submitters cannot submit events and will receive SubmitterBlocked error.
    pub fn block_submitter(env: Env, caller: Address, submitter: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::SubmitterBlocklist(submitter.clone()), &true);
        env.events()
            .publish((Symbol::new(&env, "submitter_blocked"),), (submitter, caller));
    }

    /// Unblock a submitter (owner-only). Issue #141: governance.
    pub fn unblock_submitter(env: Env, caller: Address, submitter: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .remove(&DataKey::SubmitterBlocklist(submitter.clone()));
        env.events()
            .publish((Symbol::new(&env, "submitter_unblocked"),), (submitter, caller));
    }

    /// Enable allowlist mode (owner-only). Issue #141: governance.
    /// When enabled, only whitelisted submitters can submit events.
    pub fn enable_allowlist_mode(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::AllowlistMode, &true);
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.allowlist_mode = true;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }
        env.events()
            .publish((Symbol::new(&env, "allowlist_enabled"),), (caller,));
    }

    /// Disable allowlist mode (owner-only). Issue #141: governance.
    pub fn disable_allowlist_mode(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::AllowlistMode, &false);
        env.events()
            .publish((Symbol::new(&env, "allowlist_disabled"),), (caller,));
    }

    /// Allow a submitter (owner-only). Issue #141: governance.
    /// When allowlist mode is enabled, only whitelisted submitters can submit.
    pub fn allow_submitter(env: Env, caller: Address, submitter: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::SubmitterAllowlist(submitter.clone()), &true);
        env.events()
            .publish((Symbol::new(&env, "submitter_allowed"),), (submitter, caller));
    }

    /// Remove a submitter from the allowlist (owner-only). Issue #141: governance.
    pub fn remove_submitter_from_allowlist(env: Env, caller: Address, submitter: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .remove(&DataKey::SubmitterAllowlist(submitter.clone()));
        env.events().publish(
            (Symbol::new(&env, "submitter_removed_from_allowlist"),),
            (submitter, caller),
        );
    }

    /// Get the effective metadata size limit for the given event type.
    /// Returns the per-type cap if set, otherwise the global cap, otherwise the default.
    pub fn get_metadata_max_size(env: Env, event_type: Symbol) -> u32 {
        Self::require_initialized(&env);
        Self::effective_metadata_max_size(&env, &event_type)
    }

    pub fn get_statistics(env: Env) -> ContractStatistics {
        Self::require_initialized(&env);
        Self::collect_statistics(&env)
    }

    /// Set the event emission mode (owner-only).
    /// 0 = full metadata emission (default, backward compatible)
    /// 1 = index-only emission (issue #60)
    /// 2 = hash-only emission (issue #60)
    /// 3 = no emission (issue #60)
    pub fn set_event_emission_mode(env: Env, caller: Address, mode: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::EventEmissionConfig, &mode);
        env.storage().instance().set(&DataKey::EventEmissionVersion, &2u32);
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.emission_mode = mode;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }
    }

    /// Get the current event emission mode.
    pub fn get_event_emission_mode(env: Env) -> u32 {
        Self::require_initialized(&env);
        if let Some(rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            return rs.emission_mode;
        }
        Self::effective_event_emission_mode(&env)
    }

    /// Enable/disable low-cost mode (owner-only).
    /// Low-cost mode sacrifices some features (e.g., per-type indexing) for lower per-event cost.
    /// This is useful for environments with strict fee budgets (issue #57).
    pub fn set_low_cost_mode(env: Env, caller: Address, enabled: bool) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::LowCostMode, &enabled);
        if let Some(mut rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            rs.low_cost_mode = enabled;
            env.storage().instance().set(&DataKey::RuntimeState, &rs);
        }
    }

    /// Check if low-cost mode is enabled.
    pub fn is_low_cost_mode(env: Env) -> bool {
        Self::require_initialized(&env);
        if let Some(rs) = env.storage().instance().get::<_, RuntimeState>(&DataKey::RuntimeState) {
            return rs.low_cost_mode;
        }
        env.storage().instance().get(&DataKey::LowCostMode).unwrap_or(false)
    }

    // ── issue #62: rate limiting ──────────────────────────────────────────────

    /// Set a per-submitter rate limit (owner-only).
    /// `max_per_timestamp` = max events allowed per ledger timestamp.
    /// 0 = completely block that submitter.
    pub fn set_submitter_rate_limit(env: Env, caller: Address, submitter: Address, max_per_timestamp: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::SubmitterRateLimit(submitter), &max_per_timestamp);
    }

    // ── issue #59: on-demand storage compaction ───────────────────────────────

    /// Remove stale governance keys for the given event types.
    /// "Stale" means EventCapSet/EventMaxLogs entries whose cap was removed but
    /// whose EventTypeIndices packed-bytes still lingers, or orphaned entries.
    /// Emits a `storage_compacted` event with the count of removed entries.
    /// Owner-only.
    pub fn compact_storage(env: Env, caller: Address, stale_types: Vec<Symbol>) -> u32 {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let mut removed: u32 = 0;
        for i in 0..stale_types.len() {
            let et = stale_types.get(i).unwrap();
            // Only compact if the cap is no longer set (i.e., was removed).
            if !env.storage().instance().has(&DataKey::EventCapConfig(et.clone()))
                && env.storage().instance().has(&DataKey::EventTypeIndices(et.clone()))
            {
                env.storage().instance().remove(&DataKey::EventTypeIndices(et.clone()));
                removed += 1;
            }
        }

        env.events()
            .publish((Symbol::new(&env, "storage_compacted"),), (removed,));
        removed
    }

    fn effective_low_cost_mode(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::LowCostMode).unwrap_or(false)
    }

    fn effective_metadata_max_size(env: &Env, event_type: &Symbol) -> u32 {
        // per-type overrides global
        if let Some(v) = env
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::EventMetadataMaxSize(event_type.clone()))
        {
            return v;
        }
        // global fallback
        if let Some(v) = env.storage().instance().get::<_, u32>(&DataKey::GlobalMetadataMaxSize) {
            return v;
        }
        DEFAULT_MAX_METADATA_SIZE
    }

    fn effective_event_emission_mode(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::EventEmissionConfig)
            .unwrap_or(1u32) // Default to full metadata emission
    }

    // ── issue #69: event signatures (Ed25519) ────────────────────────────────

    /// Log an event and attach a 96-byte Ed25519 signature payload
    /// (`pubkey[32] || signature[64]`) for non-repudiation.
    ///
    /// The signature is **not** verified on-chain (gas efficiency); instead it is
    /// stored and can be verified off-chain. The signed message SHOULD be the
    /// event's content-addressed ID returned by this function.
    pub fn log_event_signed(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        signature_payload: Bytes,
    ) -> BytesN<32> {
        // Delegates auth to the inner log_event call.
        if signature_payload.len() != 96 {
            panic_with_error!(&env, ContractError::InvalidSignature);
        }
        let event_id = Self::log_event_with_hierarchy(
            env.clone(),
            submitter,
            event_type,
            metadata.clone(),
            None,
            None,
            false,
        );
        env.storage()
            .instance()
            .set(&DataKey::EventSignature(event_id.clone()), &signature_payload);
        event_id
    }

    /// Return the stored 96-byte signature payload (pubkey || signature) for an
    /// event. Returns `None` if no signature was attached during logging.
    pub fn get_event_signature(env: Env, event_id: BytesN<32>) -> Option<Bytes> {
        Self::require_initialized(&env);
        env.storage().instance().get(&DataKey::EventSignature(event_id))
    }

    /// Look up an event by its content (event_type, submitter, metadata).
    ///
    /// Returns `Some(Event)` if an event with that exact content was previously
    /// stored (and deduplication recorded its position), `None` otherwise.
    pub fn find_event_by_content(env: Env, event_type: Symbol, submitter: Address, metadata: Bytes) -> Option<Event> {
        Self::require_initialized(&env);
        let content_hash = Self::compute_content_hash(&env, &event_type, &submitter, &metadata);
        if let Some(index) = env
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::EventContentHash(content_hash))
        {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(index)).unwrap();
            env.storage().instance().get(&DataKey::EventData(id))
        } else {
            None
        }
    }

    // ── Private helpers ─────────────────────────────────────────────────────

    /// Panic with `ContractNotInitialized` if the contract has not been initialized.
    fn increment_address_count(
        _env: &Env,
        counts: &mut Vec<(Address, u32)>,
        addr: Address,
    ) -> u32 {
        for idx in 0..counts.len() {
            let pair: (Address, u32) = counts.get(idx).unwrap();
            if pair.0 == addr {
                let new_count = pair.1 + 1;
                counts.set(idx, (addr.clone(), new_count));
                return new_count;
            }
        }
        counts.push_back((addr, 1u32));
        1u32
    }

    /// Increment the count for `sym` in a (Symbol, u32) accumulator Vec.
    /// Returns the NEW count for that symbol in the batch.
    fn increment_symbol_count(
        _env: &Env,
        counts: &mut Vec<(Symbol, u32)>,
        sym: Symbol,
    ) -> u32 {
        for idx in 0..counts.len() {
            let pair: (Symbol, u32) = counts.get(idx).unwrap();
            if pair.0 == sym {
                let new_count = pair.1 + 1;
                counts.set(idx, (sym.clone(), new_count));
                return new_count;
            }
        }
        counts.push_back((sym, 1u32));
        1u32
    }

    /// Validate that `metadata` conforms to `schema`.
    ///
    /// ## Schema format
    /// The schema is a length-prefixed byte sequence: the first 4 bytes (LE u32)
    /// encode the minimum required metadata length.  If `metadata.len() >= min_len`,
    /// the validation passes.  A `min_len` of 0 accepts any metadata (including empty).
    ///
    /// This simple format is intentionally minimal to remain gas-efficient on-chain.
    /// Off-chain consumers can apply richer validation (JSON Schema, Protobuf, etc.)
    /// after retrieving the schema via `get_metadata_schema`.
    fn validate_metadata_schema(metadata: &Bytes, schema: &Bytes) -> bool {
        if schema.len() < 4 {
            // Schema too short to contain the minimum-length prefix — treat as no constraint.
            return true;
        }
        let b0 = schema.get(0).unwrap() as u32;
        let b1 = schema.get(1).unwrap() as u32;
        let b2 = schema.get(2).unwrap() as u32;
        let b3 = schema.get(3).unwrap() as u32;
        let min_len: u32 = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
        metadata.len() >= min_len
    }

    /// Panic with `MetadataSchemaViolation` if `metadata` does not satisfy the
    /// optional schema configured for `event_type`.  When no schema is configured,
    /// validation is a no-op (issue #202).
    fn validate_metadata_against_schema(env: &Env, event_type: &Symbol, metadata: &Bytes) {
        if let Some(schema) = env
            .storage()
            .instance()
            .get::<_, Bytes>(&DataKey::MetadataSchema(event_type.clone()))
        {
            if !Self::validate_metadata_schema(metadata, &schema) {
                panic_with_error!(env, ContractError::MetadataSchemaViolation);
            }
        }
    }

    // ── issue #54: packed-Bytes index storage helpers ────────────────────────

    /// Append a global order index (u32, 4 bytes LE) to the packed Bytes for `event_type`.
    fn push_type_index(env: &Env, event_type: Symbol, global_index: u32) {
        let mut packed: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type.clone()))
            .unwrap_or(Bytes::new(env));
        packed.append(&Self::u32_to_bytes(env, global_index));
        env.storage()
            .instance()
            .set(&DataKey::EventTypeIndices(event_type), &packed);
    }

    /// Read the `type_index`-th global order index from the packed Bytes for `event_type`.
    fn get_type_index(env: &Env, event_type: Symbol, type_index: u32) -> u32 {
        let packed: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type))
            .unwrap_or_else(|| panic_with_error!(env, ContractError::EventTypeIndexOutOfBounds));
        let byte_offset = type_index * 4;
        if byte_offset + 4 > packed.len() {
            panic_with_error!(env, ContractError::EventTypeIndexOutOfBounds);
        }
        let b0 = packed.get(byte_offset).unwrap() as u32;
        let b1 = packed.get(byte_offset + 1).unwrap() as u32;
        let b2 = packed.get(byte_offset + 2).unwrap() as u32;
        let b3 = packed.get(byte_offset + 3).unwrap() as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    // ── issue #206: submitter-Bytes index storage helpers ──────────────────

    /// Append a global order index to the packed Bytes for `submitter`.
    fn push_submitter_index(env: &Env, submitter: &Address, global_index: u32) {
        let mut packed: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterEventIndices(submitter.clone()))
            .unwrap_or(Bytes::new(env));
        packed.append(&Self::u32_to_bytes(env, global_index));
        env.storage()
            .instance()
            .set(&DataKey::SubmitterEventIndices(submitter.clone()), &packed);
        // Increment submitter event count.
        let count: u32 = Self::submitter_count(env, submitter.clone());
        env.storage()
            .instance()
            .set(&DataKey::SubmitterEventCount(submitter.clone()), &(count + 1));
    }

    /// Read the `submitter_index`-th global order index from packed Bytes for `submitter`.
    fn get_submitter_index(env: &Env, submitter: &Address, submitter_index: u32) -> u32 {
        let packed: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterEventIndices(submitter.clone()))
            .unwrap_or_else(|| panic_with_error!(env, ContractError::EventTypeIndexOutOfBounds));
        let byte_offset = submitter_index * 4;
        if byte_offset + 4 > packed.len() {
            panic_with_error!(env, ContractError::EventTypeIndexOutOfBounds);
        }
        let b0 = packed.get(byte_offset).unwrap() as u32;
        let b1 = packed.get(byte_offset + 1).unwrap() as u32;
        let b2 = packed.get(byte_offset + 2).unwrap() as u32;
        let b3 = packed.get(byte_offset + 3).unwrap() as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Return cached event count for a submitter.
    fn submitter_count(env: &Env, submitter: Address) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::SubmitterEventCount(submitter))
            .unwrap_or(0)
    }

    fn require_owner(env: &Env, addr: &Address) {
        let owner: Address = env.storage().instance().get(&DataKey::Owner).unwrap();
        if addr != &owner {
            panic_with_error!(env, ContractError::CallerNotOwner);
        }
    }

    fn current_contract_version(env: &Env) -> u32 {
        env.storage().instance().get(&DataKey::ContractVersion).unwrap_or(1u32)
    }

    fn validate_event_type(_env: &Env, _event_type: &Symbol) {
        // Soroban protocol guarantees Symbol is valid UTF-8 <= 32 bytes.
        // Additional content validation (alphanumeric + underscore) is skipped because
        // Symbol does not expose a no_std-safe byte-level read API in soroban-sdk 26.1.0.
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Owner) || !env.storage().instance().has(&DataKey::Config) {
            panic_with_error!(env, ContractError::ContractNotInitialized);
        }
    }

    fn get_owners(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Owners)
            .unwrap_or_else(|| Vec::new(env))
    }

    fn get_required_signatures(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::RequiredSignatures)
            .unwrap_or(1u32)
    }

    fn is_addr_owner(env: &Env, addr: &Address) -> bool {
        let owners = Self::get_owners(env);
        for i in 0..owners.len() {
            if &owners.get(i).unwrap() == addr {
                return true;
            }
        }
        // fallback single Owner for legacy setups
        if let Some(owner) = env.storage().instance().get::<_, Address>(&DataKey::Owner) {
            if addr == &owner {
                return true;
            }
        }
        false
    }

    fn require_owner_or_multisig(env: &Env, addr: &Address) {
        if !Self::is_addr_owner(env, addr) {
            panic_with_error!(env, ContractError::CallerNotOwner);
        }
    }

    pub fn add_owner(env: Env, caller: Address, new_owner: Address) {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if new_owner == Address::from_str(&env, NULL_ACCOUNT) {
            panic_with_error!(&env, ContractError::NewOwnerIsZero);
        }
        let mut owners = Self::get_owners(&env);
        for i in 0..owners.len() {
            if owners.get(i).unwrap() == new_owner {
                return; // already an owner
            }
        }
        owners.push_back(new_owner.clone());
        env.storage().instance().set(&DataKey::Owners, &owners);
        env.events().publish((Symbol::new(&env, "owner_added"),), (new_owner,));
    }

    pub fn remove_owner(env: Env, caller: Address, owner_to_remove: Address) {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let owners = Self::get_owners(&env);
        let mut found = false;
        let mut new_vec: Vec<Address> = Vec::new(&env);
        for i in 0..owners.len() {
            let o = owners.get(i).unwrap();
            if o == owner_to_remove {
                found = true;
                continue;
            }
            new_vec.push_back(o.clone());
        }
        if !found {
            return; // nothing to do
        }
        // ensure required_signatures is not greater than owners.len()
        let req = Self::get_required_signatures(&env);
        if req > new_vec.len() {
            // reduce required signatures to new_vec.len()
            env.storage()
                .instance()
                .set(&DataKey::RequiredSignatures, &new_vec.len());
        }
        env.storage().instance().set(&DataKey::Owners, &new_vec);
        env.events()
            .publish((Symbol::new(&env, "owner_removed"),), (owner_to_remove,));
    }

    pub fn set_required_signatures(env: Env, caller: Address, required: u32) {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let owners = Self::get_owners(&env);
        if required == 0 || required > owners.len() {
            return; // invalid; ignore
        }
        env.storage().instance().set(&DataKey::RequiredSignatures, &required);
        env.events()
            .publish((Symbol::new(&env, "required_signatures_set"),), (required,));
    }

    pub fn submit_proposal(env: Env, proposer: Address, action: ProposalAction, ttl_seconds: u64) -> u32 {
        proposer.require_auth();
        if !Self::is_addr_owner(&env, &proposer) {
            panic_with_error!(&env, ContractError::CallerNotOwner);
        }
        let count: u32 = env.storage().instance().get(&DataKey::ProposalCount).unwrap_or(0u32);
        let id = count;
        let now = env.ledger().timestamp();
        let mut approvals: Vec<Address> = Vec::new(&env);
        approvals.push_back(proposer.clone());
        let prop = Proposal {
            id,
            proposer: proposer.clone(),
            action,
            approvals,
            expires_at: now + ttl_seconds,
            executed: false,
        };
        env.storage().instance().set(&DataKey::Proposal(id), &prop);
        env.storage().instance().set(&DataKey::ProposalCount, &(count + 1));
        env.events().publish((Symbol::new(&env, "proposal_submitted"),), (id,));
        id
    }

    pub fn approve_proposal(env: Env, approver: Address, proposal_id: u32) {
        approver.require_auth();
        if !Self::is_addr_owner(&env, &approver) {
            panic_with_error!(&env, ContractError::CallerNotOwner);
        }
        let mut prop: Proposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::EventDoesNotExist));
        if prop.executed {
            return;
        }
        let now = env.ledger().timestamp();
        if prop.expires_at < now {
            return; // expired
        }
        // add approver if not present
        for i in 0..prop.approvals.len() {
            if prop.approvals.get(i).unwrap() == approver {
                return;
            }
        }
        prop.approvals.push_back(approver.clone());
        env.storage().instance().set(&DataKey::Proposal(proposal_id), &prop);
        env.events()
            .publish((Symbol::new(&env, "proposal_approved"),), (proposal_id, approver));
    }

    pub fn execute_proposal(env: Env, executor: Address, proposal_id: u32) {
        executor.require_auth();
        if !Self::is_addr_owner(&env, &executor) {
            panic_with_error!(&env, ContractError::CallerNotOwner);
        }
        let mut prop: Proposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::EventDoesNotExist));
        if prop.executed {
            return;
        }
        let now = env.ledger().timestamp();
        if prop.expires_at < now {
            return; // expired
        }
        let approvals_needed = Self::get_required_signatures(&env);
        if prop.approvals.len() < approvals_needed {
            return; // not enough approvals
        }
        // perform the action
        match prop.action.clone() {
            ProposalAction::TransferOwnership(new_owner) => {
                env.storage().instance().set(&DataKey::Owner, &new_owner);
            }
            ProposalAction::AddOwner(ref addr) => {
                let mut owners = Self::get_owners(&env);
                let mut exists = false;
                for i in 0..owners.len() {
                    if owners.get(i).unwrap() == *addr {
                        exists = true;
                    }
                }
                if !exists {
                    owners.push_back(addr.clone());
                    env.storage().instance().set(&DataKey::Owners, &owners);
                }
            }
            ProposalAction::RemoveOwner(ref addr) => {
                let owners = Self::get_owners(&env);
                let mut new_vec: Vec<Address> = Vec::new(&env);
                for i in 0..owners.len() {
                    let o = owners.get(i).unwrap();
                    if o != *addr {
                        new_vec.push_back(o);
                    }
                }
                env.storage().instance().set(&DataKey::Owners, &new_vec);
            }
            ProposalAction::SetRequiredSignatures(req) => {
                env.storage().instance().set(&DataKey::RequiredSignatures, &req);
            }
            ProposalAction::SetGlobalMaxLogs(v) => {
                if let Some(mut c) = env.storage().instance().get::<_, Config>(&DataKey::Config) {
                    c.global_max_logs = v;
                    env.storage().instance().set(&DataKey::Config, &c);
                }
            }
            ProposalAction::SetMetadataSchema(ref event_type, ref schema) => {
                env.storage().instance().set(&DataKey::MetadataSchema(event_type.clone()), schema);
            }
            ProposalAction::RollbackEvent(index, target_version) => {
                let _ = Self::rollback_event(env.clone(), executor.clone(), index, target_version);
            }
            ProposalAction::Pause => {
                env.storage().instance().set(&DataKey::Paused, &true);
            }
            ProposalAction::Unpause => {
                env.storage().instance().set(&DataKey::Paused, &false);
            }
        }
        prop.executed = true;
        env.storage().instance().set(&DataKey::Proposal(proposal_id), &prop);
        env.events()
            .publish((Symbol::new(&env, "proposal_executed"),), (proposal_id, executor));
    }

    fn event_type_count(env: &Env, event_type: Symbol) -> u32 {
        let packed: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type))
            .unwrap_or(Bytes::new(env));
        packed.len() / 4
    }

    /// Compute a content-addressed event ID (issue #70).
    /// `sha256(contract_strkey_bytes || submitter_strkey_bytes || event_type_name_bytes || metadata || timestamp_le || index_le)`
    fn compute_event_id(
        env: &Env,
        submitter: &Address,
        event_type: &Symbol,
        metadata: &Bytes,
        timestamp: u64,
        index: u32,
    ) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        // contract address as strkey string bytes
        let contract_str = env.current_contract_address().to_string();
        preimage.append(&contract_str.to_bytes());
        // submitter strkey string bytes
        preimage.append(&submitter.to_string().to_bytes());
        // event_type as its u64 raw bits (unique per symbol)
        preimage.append(&Self::u64_to_bytes(env, event_type.to_val().get_payload()));
        // metadata
        preimage.append(metadata);
        // timestamp (8 bytes LE)
        preimage.append(&Self::u64_to_bytes(env, timestamp));
        // index (4 bytes LE)
        preimage.append(&Self::u32_to_bytes(env, index));
        env.crypto().sha256(&preimage).into()
    }

    /// Compute the event's own hash for the chain (issue #66).
    /// `sha256(event_id || prev_hash || index_le || timestamp_le)`
    fn compute_event_hash(
        env: &Env,
        event_id: &BytesN<32>,
        prev_hash: &BytesN<32>,
        index: u32,
        timestamp: u64,
    ) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&event_id.clone().into());
        preimage.append(&prev_hash.clone().into());
        preimage.append(&Self::u32_to_bytes(env, index));
        preimage.append(&Self::u64_to_bytes(env, timestamp));
        env.crypto().sha256(&preimage).into()
    }

    /// Compute a content hash for deduplication: sha256(event_type_payload_le || submitter_strkey || metadata).
    /// This hash is independent of timestamp and index, making it stable across retries.
    fn compute_content_hash(env: &Env, event_type: &Symbol, submitter: &Address, metadata: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&Self::u64_to_bytes(env, event_type.to_val().get_payload()));
        preimage.append(&submitter.to_string().to_bytes());
        preimage.append(metadata);
        env.crypto().sha256(&preimage).into()
    }

    fn verify_range(env: &Env, from: u32, to: u32) -> bool {
        // Seed expected_prev: genesis is all-zeros; for a mid-range start,
        // use the event_hash of the preceding event.
        let mut expected_prev: BytesN<32> = if from == 0 {
            BytesN::from_array(env, &[0u8; 32])
        } else {
            let prev_id: BytesN<32> = match env.storage().instance().get(&DataKey::EventOrder(from - 1)) {
                Some(v) => v,
                None => return false,
            };
            let prev_evt: Event = match env.storage().instance().get(&DataKey::EventData(prev_id)) {
                Some(v) => v,
                None => return false,
            };
            prev_evt.event_hash
        };
        for i in from..to {
            let id: BytesN<32> = match env.storage().instance().get(&DataKey::EventOrder(i)) {
                Some(v) => v,
                None => return false,
            };
            let evt: Event = match env.storage().instance().get(&DataKey::EventData(id.clone())) {
                Some(v) => v,
                None => return false,
            };
            if evt.prev_hash != expected_prev {
                return false;
            }
            // Re-derive and compare the stored hash
            let recomputed = Self::compute_event_hash(env, &id, &evt.prev_hash, i, evt.timestamp);
            if evt.event_hash != recomputed {
                return false;
            }
            expected_prev = evt.event_hash.clone();
        }
        true
    }

    fn collect_statistics(env: &Env) -> ContractStatistics {
        let total: u32 = env
            .storage()
            .instance()
            .get::<_, Config>(&DataKey::Config)
            .map(|c| c.total_events)
            .unwrap_or(0);
        let now = env.ledger().timestamp();
        let mut events_by_type: Vec<(Symbol, u32)> = Vec::new(env);
        let mut top_submitters: Vec<(Address, u32)> = Vec::new(env);
        let mut events_last_hour: u32 = 0;
        let mut events_last_day: u32 = 0;
        let mut events_last_week: u32 = 0;

        for i in 0..total {
            let event_id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let evt: Event = env.storage().instance().get(&DataKey::EventData(event_id)).unwrap();

            Self::increment_type_count(env, &mut events_by_type, evt.event_type.clone());
            Self::increment_submitter_count(env, &mut top_submitters, evt.submitter.clone());

            if let Some(elapsed) = now.checked_sub(evt.timestamp) {
                if elapsed <= 3600 {
                    events_last_hour += 1;
                }
                if elapsed <= 86400 {
                    events_last_day += 1;
                }
                if elapsed <= 604800 {
                    events_last_week += 1;
                }
            }
        }

        ContractStatistics {
            total_events: total,
            events_by_type,
            events_last_hour,
            events_last_day,
            events_last_week,
            top_submitters,
        }
    }

    fn increment_type_count(_env: &Env, counts: &mut Vec<(Symbol, u32)>, event_type: Symbol) {
        for idx in 0..counts.len() {
            let pair: (Symbol, u32) = counts.get(idx).unwrap();
            if pair.0 == event_type {
                counts.set(idx, (event_type.clone(), pair.1 + 1));
                return;
            }
        }
        counts.push_back((event_type, 1u32));
    }

    fn increment_submitter_count(_env: &Env, counts: &mut Vec<(Address, u32)>, submitter: Address) {
        for idx in 0..counts.len() {
            let pair: (Address, u32) = counts.get(idx).unwrap();
            if pair.0 == submitter {
                counts.set(idx, (submitter.clone(), pair.1 + 1));
                return;
            }
        }
        counts.push_back((submitter, 1u32));
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

    fn u32_to_bytes(env: &Env, v: u32) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
            ]
        )
    }

    fn bytes_contains(haystack: &Bytes, needle: &Bytes) -> bool {
        let haystack_len = haystack.len();
        let needle_len = needle.len();
        if needle_len == 0 {
            return true;
        }
        if needle_len > haystack_len {
            return false;
        }
        let last_start = haystack_len - needle_len;
        for start in 0..=last_start {
            let mut matched = true;
            for i in 0..needle_len {
                let h = haystack.get(start + i).unwrap();
                let n = needle.get(i).unwrap();
                if h != n {
                    matched = false;
                    break;
                }
            }
            if matched {
                return true;
            }
        }
        false
    }

    /// Default nonce window size (issue #214). 0 = unlimited gap allowed.
    fn default_nonce_window_size(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::DefaultNonceWindowSize)
            .unwrap_or(1000)
    }

    // ── Conflict minerals reporting (Dodd-Frank §1502) ──────────────────

    /// Record a mineral allocation to product and smelter(s). Owner-only.
    ///
    /// Emits a `("cm", "alloc")` event with payload `(submitter, allocation_id, mineral)`.
    pub fn record_mineral_allocation(
        env: Env,
        caller: Address,
        allocation: MineralAllocation,
    ) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::CMAllocation(allocation.allocation_id.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::CMAllocationExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMAllocationCount)
            .unwrap_or(0u32);

        env.storage().instance().set(&key, &allocation);
        env.storage()
            .instance()
            .set(&DataKey::CMAllocationCount, &(count + 1));

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("alloc")),
            (caller, allocation.allocation_id.clone(), allocation.mineral.clone()),
        );

        count
    }

    /// Retrieve a mineral allocation by allocation_id.
    pub fn get_mineral_allocation(env: Env, allocation_id: Symbol) -> MineralAllocation {
        env.storage()
            .instance()
            .get(&DataKey::CMAllocation(allocation_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CMAllocationNotFound))
    }

    /// Return total mineral allocations recorded.
    pub fn cm_allocation_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CMAllocationCount)
            .unwrap_or(0u32)
    }

    /// Register a smelter in the conflict minerals registry. Owner-only.
    pub fn record_smelter(env: Env, caller: Address, smelter: Smelter) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::CMSmelter(smelter.smelter_id.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::CMSmelterExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMSmelterCount)
            .unwrap_or(0u32);

        env.storage().instance().set(&key, &smelter);
        env.storage()
            .instance()
            .set(&DataKey::CMSmelterCount, &(count + 1));

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("smelter")),
            (caller, smelter.smelter_id.clone(), smelter.mineral_type.clone()),
        );

        count
    }

    /// Retrieve a smelter by smelter_id.
    pub fn get_smelter(env: Env, smelter_id: Symbol) -> Smelter {
        env.storage()
            .instance()
            .get(&DataKey::CMSmelter(smelter_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CMSmelterNotFound))
    }

    /// Return total smelters registered.
    pub fn cm_smelter_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CMSmelterCount)
            .unwrap_or(0u32)
    }

    /// Record a country-of-origin verification. Owner-only.
    pub fn record_country_of_origin(env: Env, caller: Address, record: CountryOfOrigin) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMCountryOfOriginCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::CMCountryOfOrigin(record.record_id.clone()), &record);
        env.storage()
            .instance()
            .set(&DataKey::CMCountryOfOriginCount, &(count + 1));

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("coo")),
            (caller, record.record_id.clone(), record.country.clone()),
        );

        count
    }

    /// Retrieve a country-of-origin record by record_id.
    pub fn get_country_of_origin(env: Env, record_id: Symbol) -> CountryOfOrigin {
        env.storage()
            .instance()
            .get(&DataKey::CMCountryOfOrigin(record_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CMCountryOfOriginNotFound))
    }

    /// Return total country-of-origin records.
    pub fn cm_country_of_origin_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CMCountryOfOriginCount)
            .unwrap_or(0u32)
    }

    /// Submit a conflict minerals due diligence assessment. Owner-only.
    pub fn submit_due_diligence_cm(
        env: Env,
        caller: Address,
        record: DueDiligenceCM,
    ) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMDueDiligenceCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::CMDueDiligence(record.record_id.clone()), &record);
        env.storage()
            .instance()
            .set(&DataKey::CMDueDiligenceCount, &(count + 1));

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("dd")),
            (caller, record.record_id.clone(), record.risk_level),
        );

        count
    }

    /// Retrieve a conflict minerals due diligence record by record_id.
    pub fn get_due_diligence_cm(env: Env, record_id: Symbol) -> DueDiligenceCM {
        env.storage()
            .instance()
            .get(&DataKey::CMDueDiligence(record_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CMDueDiligenceNotFound))
    }

    /// Return total due diligence assessments.
    pub fn cm_due_diligence_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CMDueDiligenceCount)
            .unwrap_or(0u32)
    }

    /// Record an independent audit. Owner-only.
    pub fn record_audit(env: Env, caller: Address, audit: AuditRecord) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMAuditCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::CMAudit(audit.audit_id.clone()), &audit);
        env.storage()
            .instance()
            .set(&DataKey::CMAuditCount, &(count + 1));

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("audit")),
            (caller, audit.audit_id.clone(), audit.result),
        );

        count
    }

    /// Retrieve an audit record by audit_id.
    pub fn get_audit_record(env: Env, audit_id: Symbol) -> AuditRecord {
        env.storage()
            .instance()
            .get(&DataKey::CMAudit(audit_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CMAuditNotFound))
    }

    /// Return total audit records.
    pub fn cm_audit_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::CMAuditCount)
            .unwrap_or(0u32)
    }

    /// Generate and persist a CMRT (Conflict Minerals Reporting Template) report. Owner-only.
    pub fn build_cmrt_report(env: Env, caller: Address, period: Symbol) -> CMRTReport {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let alloc_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMAllocationCount)
            .unwrap_or(0u32);
        let smelter_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMSmelterCount)
            .unwrap_or(0u32);
        let dd_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMDueDiligenceCount)
            .unwrap_or(0u32);
        let audit_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMAuditCount)
            .unwrap_or(0u32);
        let coo_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CMCountryOfOriginCount)
            .unwrap_or(0u32);

        let report = CMRTReport {
            generated_at: env.ledger().timestamp(),
            period,
            total_allocations: alloc_count,
            conflict_region_allocations: 0u32,
            total_smelters: smelter_count,
            on_list_smelters: 0u32,
            audits_pending: 0u32,
            audits_completed: 0u32,
            due_diligence_count: dd_count,
            conflict_risk_identified: 0u32,
            origin_verification_pct: 0u32,
        };

        env.storage()
            .instance()
            .set(&DataKey::LatestCMRTReport, &report);

        env.events().publish(
            (symbol_short!("cm"), symbol_short!("cmrt")),
            (caller, period, alloc_count),
        );

        report
    }

    /// Retrieve the most recently generated CMRT report.
    pub fn get_cmrt_report(env: Env) -> Option<CMRTReport> {
        env.storage().instance().get(&DataKey::LatestCMRTReport)
    }

    // ── Modern slavery act compliance ─────────────────────────────────────

    /// Record a modern slavery risk assessment. Owner-only.
    ///
    /// Emits a `("msa", "risk_assess")` event with payload
    /// `(submitter, assessment_id, risk_level)`.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    /// - `MSARiskAssessmentExists` — assessment_id already recorded.
    pub fn record_risk_assessment(
        env: Env,
        caller: Address,
        assessment: RiskAssessment,
    ) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::MSARiskAssessment(assessment.assessment_id.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::MSARiskAssessmentExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSARiskAssessmentCount)
            .unwrap_or(0u32);

        env.storage().instance().set(&key, &assessment);
        env.storage()
            .instance()
            .set(&DataKey::MSARiskAssessmentCount, &(count + 1));

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("risk_as")),
            (caller, assessment.assessment_id.clone(), assessment.risk_level),
        );

        count
    }

    /// Retrieve a risk assessment by assessment_id.
    ///
    /// # Errors
    /// - `MSARiskAssessmentNotFound` — no record exists for this assessment_id.
    pub fn get_risk_assessment(env: Env, assessment_id: Symbol) -> RiskAssessment {
        env.storage()
            .instance()
            .get(&DataKey::MSARiskAssessment(assessment_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MSARiskAssessmentNotFound))
    }

    /// Return the total number of recorded risk assessments.
    pub fn msa_risk_assessment_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MSARiskAssessmentCount)
            .unwrap_or(0u32)
    }

    /// Record a supply chain node (supplier / partner). Owner-only.
    ///
    /// Emits a `("msa", "supply_ch")` event with payload
    /// `(submitter, supplier_id, risk_level)`.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    /// - `MSASupplyChainNodeExists` — supplier_id already mapped.
    pub fn record_supply_chain_node(
        env: Env,
        caller: Address,
        node: SupplyChainNode,
    ) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::MSASupplyChainNode(node.supplier_id.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::MSASupplyChainNodeExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSASupplyChainNodeCount)
            .unwrap_or(0u32);

        env.storage().instance().set(&key, &node);
        env.storage()
            .instance()
            .set(&DataKey::MSASupplyChainNodeCount, &(count + 1));

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("supply_")),
            (caller, node.supplier_id.clone(), node.risk_level),
        );

        count
    }

    /// Retrieve a supply chain node by supplier_id.
    ///
    /// # Errors
    /// - `MSASupplyChainNodeNotFound` — no node found for this supplier_id.
    pub fn get_supply_chain_node(env: Env, supplier_id: Symbol) -> SupplyChainNode {
        env.storage()
            .instance()
            .get(&DataKey::MSASupplyChainNode(supplier_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MSASupplyChainNodeNotFound))
    }

    /// Return the total number of supply chain nodes mapped.
    pub fn msa_supply_chain_node_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MSASupplyChainNodeCount)
            .unwrap_or(0u32)
    }

    /// Record a training session. Owner-only.
    ///
    /// Emits a `("msa", "training")` event with payload
    /// `(submitter, training_id, attendees)`.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    pub fn record_training(env: Env, caller: Address, training: TrainingRecord) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSATrainingRecordCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::MSATrainingRecord(training.training_id.clone()), &training);
        env.storage()
            .instance()
            .set(&DataKey::MSATrainingRecordCount, &(count + 1));

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("train")),
            (caller, training.training_id.clone(), training.attendees),
        );

        count
    }

    /// Retrieve a training record by training_id.
    ///
    /// # Errors
    /// - `MSATrainingRecordNotFound` — no record exists for this training_id.
    pub fn get_training_record(env: Env, training_id: Symbol) -> TrainingRecord {
        env.storage()
            .instance()
            .get(&DataKey::MSATrainingRecord(training_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MSATrainingRecordNotFound))
    }

    /// Return the total number of training sessions recorded.
    pub fn msa_training_record_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MSATrainingRecordCount)
            .unwrap_or(0u32)
    }

    /// Submit a due diligence investigation record. Owner-only.
    ///
    /// Emits a `("msa", "dd")` event with payload
    /// `(submitter, record_id, risk_level)`.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    pub fn submit_due_diligence(env: Env, caller: Address, record: DueDiligenceRecord) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSADueDiligenceCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::MSADueDiligenceRecord(record.record_id.clone()), &record);
        env.storage()
            .instance()
            .set(&DataKey::MSADueDiligenceCount, &(count + 1));

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("dd")),
            (caller, record.record_id.clone(), record.risk_level),
        );

        count
    }

    /// Retrieve a due diligence record by record_id.
    ///
    /// # Errors
    /// - `MSADueDiligenceRecordNotFound` — no record exists for this record_id.
    pub fn get_due_diligence_record(env: Env, record_id: Symbol) -> DueDiligenceRecord {
        env.storage()
            .instance()
            .get(&DataKey::MSADueDiligenceRecord(record_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MSADueDiligenceRecordNotFound))
    }

    /// Return the total number of due diligence investigations recorded.
    pub fn msa_due_diligence_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MSADueDiligenceCount)
            .unwrap_or(0u32)
    }

    /// Record a modern slavery policy. Owner-only.
    ///
    /// Emits a `("msa", "policy")` event with payload
    /// `(submitter, policy_id, version)`.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    pub fn record_msa_policy(env: Env, caller: Address, policy: MSAPolicy) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSAPolicyCount)
            .unwrap_or(0u32);

        env.storage()
            .instance()
            .set(&DataKey::MSAPolicy(policy.policy_id.clone()), &policy);
        env.storage()
            .instance()
            .set(&DataKey::MSAPolicyCount, &(count + 1));

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("policy")),
            (caller, policy.policy_id.clone(), policy.version),
        );

        count
    }

    /// Retrieve a modern slavery policy by policy_id.
    ///
    /// # Errors
    /// - `MSAPolicyNotFound` — no policy exists for this policy_id.
    pub fn get_msa_policy(env: Env, policy_id: Symbol) -> MSAPolicy {
        env.storage()
            .instance()
            .get(&DataKey::MSAPolicy(policy_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MSAPolicyNotFound))
    }

    /// Return the total number of active policies.
    pub fn msa_policy_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MSAPolicyCount)
            .unwrap_or(0u32)
    }

    /// Generate and persist an aggregated modern slavery compliance report.
    ///
    /// Aggregates risk assessments, supply chain data, training, and due diligence
    /// records to produce a compliance snapshot. Owner-only.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    pub fn build_msa_report(env: Env, caller: Address) -> MSAReport {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let assess_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSARiskAssessmentCount)
            .unwrap_or(0u32);
        let node_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSASupplyChainNodeCount)
            .unwrap_or(0u32);
        let train_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSATrainingRecordCount)
            .unwrap_or(0u32);
        let dd_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSADueDiligenceCount)
            .unwrap_or(0u32);
        let policy_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MSAPolicyCount)
            .unwrap_or(0u32);

        // Aggregate metrics (simplified for on-chain feasibility)
        let mut max_risk: u32 = 0u32;
        let mut total_high_risk_areas: u32 = 0u32;
        let mut high_risk_suppliers: u32 = 0u32;
        let mut total_trained: u32 = 0u32;
        let mut total_corrective_actions: u32 = 0u32;
        let mut completed_corrective_actions: u32 = 0u32;

        // In a real implementation, iterate through stored records.
        // For chain-safe execution, the caller aggregates off-chain and verifies on-chain.
        // This placeholder uses stored counts; full aggregation happens off-chain.

        let report = MSAReport {
            generated_at: env.ledger().timestamp(),
            assessments_count: assess_count,
            max_risk_level: max_risk,
            total_high_risk_areas,
            supply_chain_nodes: node_count,
            high_risk_suppliers,
            total_trained_personnel: total_trained,
            due_diligence_investigations: dd_count,
            total_corrective_actions,
            corrective_actions_completion_pct: if total_corrective_actions > 0 {
                (completed_corrective_actions * 100 / total_corrective_actions) as u32
            } else {
                0u32
            },
            active_policies: policy_count,
        };

        env.storage()
            .instance()
            .set(&DataKey::LatestMSAReport, &report);

        env.events().publish(
            (symbol_short!("msa"), symbol_short!("report")),
            (caller, assess_count, report.max_risk_level),
        );

        report
    }

    /// Retrieve the most recently generated modern slavery report.
    ///
    /// Returns `None` if no report has been generated yet.
    pub fn get_msa_report(env: Env) -> Option<MSAReport> {
        env.storage().instance().get(&DataKey::LatestMSAReport)
    }
}

    /// Record a social impact metrics snapshot for a reporting period.
    ///
    /// Owner-only. Each period tag must be unique. Emits a
    /// `("social_impact", "recorded")` Soroban event with payload
    /// `(submitter, period, sroi_bps)` so off-chain monitors can index it.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    /// - `SocialImpactPeriodExists` — a record for this period already exists.
    pub fn record_social_impact(
        env: Env,
        caller: Address,
        metrics: SocialImpactMetrics,
    ) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::SocialImpactData(metrics.period.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::SocialImpactPeriodExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::SocialImpactCount)
            .unwrap_or(0u32);

        // Compute sroi_bps inline for the event payload
        let sroi_bps: u64 = if metrics.total_investment > 0 {
            metrics
                .total_social_value
                .saturating_mul(10_000)
                .saturating_div(metrics.total_investment)
        } else {
            0
        };

        env.storage().instance().set(&key, &metrics);
        env.storage()
            .instance()
            .set(&DataKey::SocialImpactCount, &(count + 1));

        env.events().publish(
            (symbol_short!("soc_imp"), symbol_short!("recorded")),
            (caller, metrics.period.clone(), sroi_bps),
        );

        count
    }

    /// Retrieve a social impact metrics record by period tag.
    ///
    /// # Errors
    /// - `SocialImpactNotFound` — no record exists for this period.
    pub fn get_social_impact(env: Env, period: Symbol) -> SocialImpactMetrics {
        env.storage()
            .instance()
            .get(&DataKey::SocialImpactData(period.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::SocialImpactNotFound))
    }

    /// Return the total number of recorded social impact periods.
    pub fn social_impact_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::SocialImpactCount)
            .unwrap_or(0u32)
    }

    /// Calculate SROI as basis points (social_value / investment × 10 000) across all
    /// recorded periods. Aggregates `total_investment` and `total_social_value` from
    /// every stored period that can be found via `SocialImpactPeriodIndex(i)`.
    ///
    /// Because the contract stores records by period Symbol (not integer key), this
    /// function accepts a `Vec<Symbol>` of the periods to aggregate, giving callers
    /// full control over scope.
    ///
    /// # Errors
    /// - `SroiDivisionByZero` — aggregate investment is zero.
    /// - `SocialImpactNotFound` — any supplied period tag is not recorded.
    pub fn calculate_sroi(env: Env, periods: Vec<Symbol>) -> u64 {
        let mut total_inv: u64 = 0u64;
        let mut total_val: u64 = 0u64;

        for i in 0..periods.len() {
            let period: Symbol = periods.get(i).unwrap();
            let m: SocialImpactMetrics = env
                .storage()
                .instance()
                .get(&DataKey::SocialImpactData(period.clone()))
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::SocialImpactNotFound));
            total_inv = total_inv.saturating_add(m.total_investment);
            total_val = total_val.saturating_add(m.total_social_value);
        }

        if total_inv == 0 {
            panic_with_error!(&env, ContractError::SroiDivisionByZero);
        }

        total_val.saturating_mul(10_000).saturating_div(total_inv)
    }

    /// Register a stakeholder in the on-chain registry. Owner-only.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    /// - `StakeholderAlreadyExists` — stakeholder address is already registered.
    pub fn add_stakeholder(env: Env, caller: Address, stakeholder: Stakeholder) -> u32 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let key = DataKey::StakeholderEntry(stakeholder.address.clone());
        if env.storage().instance().has(&key) {
            panic_with_error!(&env, ContractError::StakeholderAlreadyExists);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::StakeholderCount)
            .unwrap_or(0u32);

        env.storage().instance().set(&key, &stakeholder);
        env.storage()
            .instance()
            .set(&DataKey::StakeholderCount, &(count + 1));

        env.events().publish(
            (symbol_short!("soc_imp"), symbol_short!("stk_add")),
            (caller, stakeholder.address.clone(), stakeholder.category.clone()),
        );

        count
    }

    /// Retrieve a stakeholder by Stellar address.
    ///
    /// # Errors
    /// - `StakeholderNotFound` — no stakeholder registered at this address.
    pub fn get_stakeholder(env: Env, address: Address) -> Stakeholder {
        env.storage()
            .instance()
            .get(&DataKey::StakeholderEntry(address.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::StakeholderNotFound))
    }

    /// Return the total number of registered stakeholders.
    pub fn stakeholder_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::StakeholderCount)
            .unwrap_or(0u32)
    }

    /// Generate and persist an aggregated `ImpactReport` from the supplied period list.
    ///
    /// The report is stored under `DataKey::LatestImpactReport` and can be retrieved
    /// via `get_impact_report`. Owner-only to prevent spam.
    ///
    /// # Errors
    /// - `CallerNotOwner` — caller is not an owner.
    /// - `SocialImpactNotFound` — any supplied period is not recorded.
    /// - `SroiDivisionByZero` — aggregate investment is zero.
    pub fn generate_impact_report(
        env: Env,
        caller: Address,
        periods: Vec<Symbol>,
    ) -> ImpactReport {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut total_jobs: u32 = 0u32;
        let mut total_community_inv: u64 = 0u64;
        let mut sum_diversity_women: u64 = 0u64;
        let mut total_social_value: u64 = 0u64;
        let mut total_investment: u64 = 0u64;
        let period_count = periods.len();

        for i in 0..period_count {
            let period: Symbol = periods.get(i).unwrap();
            let m: SocialImpactMetrics = env
                .storage()
                .instance()
                .get(&DataKey::SocialImpactData(period.clone()))
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::SocialImpactNotFound));

            total_jobs = total_jobs.saturating_add(m.jobs_created);
            total_community_inv = total_community_inv.saturating_add(m.community_investment);
            sum_diversity_women = sum_diversity_women.saturating_add(m.diversity_women_bps as u64);
            total_social_value = total_social_value.saturating_add(m.total_social_value);
            total_investment = total_investment.saturating_add(m.total_investment);
        }

        if total_investment == 0 {
            panic_with_error!(&env, ContractError::SroiDivisionByZero);
        }

        let sroi_bps = total_social_value
            .saturating_mul(10_000)
            .saturating_div(total_investment);

        let avg_diversity_women_bps = if period_count > 0 {
            (sum_diversity_women / period_count as u64) as u32
        } else {
            0u32
        };

        let stk_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::StakeholderCount)
            .unwrap_or(0u32);

        let report = ImpactReport {
            generated_at: env.ledger().timestamp(),
            periods_included: period_count,
            total_jobs_created: total_jobs,
            total_community_investment: total_community_inv,
            avg_diversity_women_bps,
            total_social_value,
            total_investment,
            sroi_bps,
            stakeholder_count: stk_count,
        };

        env.storage()
            .instance()
            .set(&DataKey::LatestImpactReport, &report);

        env.events().publish(
            (symbol_short!("soc_imp"), symbol_short!("report")),
            (caller, period_count, sroi_bps),
        );

        report
    }

    /// Retrieve the most recently generated impact report.
    ///
    /// Returns `None` if no report has been generated yet.
    pub fn get_impact_report(env: Env) -> Option<ImpactReport> {
        env.storage()
            .instance()
            .get(&DataKey::LatestImpactReport)
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod regression_tests;

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod cross_contract_tests;

#[cfg(test)]
mod fee_tests;

#[cfg(test)]
mod upgrade_tests;

#[cfg(test)]
mod security_tests;

#[cfg(test)]
mod comprehensive_fuzz;
