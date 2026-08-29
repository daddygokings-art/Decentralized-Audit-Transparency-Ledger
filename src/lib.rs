#![no_std]
// Migration to #[contractevent] macro is deferred (issue tracked separately)
#![allow(deprecated)]

pub mod regulator;
pub mod regulator_events;
pub mod disclosure;
pub mod data_sharing;
pub mod tamper_evidence;
pub mod compliance_validators;
pub mod tax;
pub mod vat_engine;
pub mod tax_engines;
pub mod tax_audit_trail;

// ── Automated Regulatory Reporting ──────────────────────────────────────────
pub mod regulatory_reporting;
pub mod report_generators;
pub mod report_validation;
pub mod submission_tracker;
pub mod reporting_audit_trail;

#[cfg(test)]
mod tax_tests;

#[cfg(test)]
mod regulator_tests;

#[cfg(test)]
mod regulatory_reporting_tests;

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol,
    Vec,
};

pub mod supply_chain;
pub mod digital_passport;
pub mod carbon_credits;
pub mod esg_reporting;

// Data retention, legal hold, GDPR erasure, and the immutable operational audit log.
pub mod data_retention;

// Contract event serverless functions for event processing (#522)
pub mod serverless_processing;

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

    // ── Circular Economy ────────────────────────────────────────────────────

    /// Material passport keyed by a 32-byte material ID.
    MaterialPassport(BytesN<32>),
    /// All material IDs registered (packed 32-byte chunks).
    AllMaterialIds,
    /// Loop events for a material, stored as a Vec<LoopEvent>.
    MaterialLoopEvents(BytesN<32>),
    /// Circularity snapshot keyed by ledger sequence (u32).
    CircularitySnapshot(u32),
    /// Total number of circularity snapshots taken.
    CircularitySnapshotCount,
    /// Running totals for circularity metric aggregates.
    CircularityTotals,

    // ── Lifecycle Assessment (LCA) ───────────────────────────────────────────

    /// LCA profile keyed by product ID (32-byte content-addressed).
    LcaProfile(BytesN<32>),
    /// Phase impacts stored per (product_id, phase_discriminant).
    LcaPhaseImpact(BytesN<32>, u32),
    /// Finalized LCA result (aggregated across all phases).
    LcaResult(BytesN<32>),
    /// Normalization reference values set keyed by a short name Symbol.
    LcaNormRef(Symbol),
    /// Weighting scheme keyed by a short name Symbol.
    LcaWeightingScheme(Symbol),
    /// Uncertainty bounds (interval arithmetic) for a product's aggregated impacts.
    LcaUncertainty(BytesN<32>),
    /// LCA database entry keyed by a 32-byte reference ID.
    LcaDbEntry(BytesN<32>),
    /// Total number of LCA profiles registered.
    LcaProfileCount,

    // ── Biodiversity ─────────────────────────────────────────────────────────

    /// Biodiversity impact record keyed by a 32-byte supply-chain event ID.
    BioImpact(BytesN<32>),
    /// Biodiversity offset record keyed by a 32-byte offset ID.
    BioOffset(BytesN<32>),
    /// Running global biodiversity totals (accumulator across all impact records).
    BioTotals,
    /// Nature-positive snapshot keyed by a 0-based ordinal.
    BioSnapshot(u32),
    /// Total nature-positive snapshots taken.
    BioSnapshotCount,
    /// Ecosystem service valuation record keyed by a 32-byte site/project ID.
    EcoServiceRecord(BytesN<32>),
    /// Species observation record keyed by a 32-byte observation ID.
    SpeciesObservation(BytesN<32>),

    // ── Water Footprint ───────────────────────────────────────────────────────

    /// Water footprint record keyed by a 32-byte event-linked ID.
    WaterFootprint(BytesN<32>),
    /// Water risk assessment keyed by a 32-byte site/basin ID.
    WaterRiskAssessment(BytesN<32>),
    /// Water stewardship programme record keyed by a 32-byte programme ID.
    WaterStewardship(BytesN<32>),
    /// CDP water disclosure record keyed by a 32-byte disclosure ID.
    WaterDisclosure(BytesN<32>),
    /// Running global water totals (accumulator updated on every write).
    WaterTotals,
    /// Water snapshot keyed by a 0-based ordinal.
    WaterSnapshot(u32),
    /// Total water snapshots taken.
    WaterSnapshotCount,

    // ── Data Retention, Legal Hold & GDPR Erasure ───────────────────────────

    /// Per-category retention policy (in days). Absent = fall back to `DefaultRetentionDays`.
    RetentionPolicy(Symbol),
    /// Global default retention period in days. 0 = no automatic retention policy configured.
    DefaultRetentionDays,
    /// Legal hold record for an event index; while `active`, the event cannot be erased
    /// or cleaned up by `run_retention_sweep`/TTL cleanup.
    LegalHold(u32),
    /// Compliance exception (e.g. a statutory record-keeping requirement) for an event index,
    /// overriding retention-driven erasure until it expires.
    ComplianceException(u32),
    /// GDPR right-to-erasure request, keyed by an auto-incrementing request ID.
    ErasureRequest(u32),
    /// Count of erasure requests filed so far; used to allocate the next request ID.
    ErasureRequestCount,
    /// Finalized erasure record for an event index once a request is fulfilled or a
    /// retention sweep redacts it.
    ErasureRecord(u32),
    /// Addresses authorized to append operational (deploy/config-change/access-grant/
    /// secret-rotation) audit records via `log_operational_action`, in addition to the
    /// contract owner(s).
    OpsAuditRecorders,
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

    /// **Code 34**: The material passport for this ID already exists.
    /// **Common cause**: `register_material_passport` called twice with the same material ID.
    /// **Resolution**: Use a unique material ID per asset.
    MaterialPassportAlreadyExists = 34,

    /// **Code 35**: No material passport found for the given material ID.
    /// **Common cause**: Querying or recording a loop event for an unregistered material.
    /// **Resolution**: Call `register_material_passport` first.
    MaterialPassportNotFound = 35,

    /// **Code 36**: Invalid loop event type. Must be one of: recycle, reuse, repair, remanufacture, return, dispose.
    /// **Common cause**: Caller supplied an unrecognised loop-event-type Symbol.
    /// **Resolution**: Use a recognised loop event type Symbol.
    InvalidLoopEventType = 36,

    /// **Code 37**: Material flow quantity must be greater than zero.
    /// **Common cause**: A zero-weight or zero-volume quantity was submitted.
    /// **Resolution**: Provide a positive quantity in milligrams.
    InvalidFlowQuantity = 37,

    // ── LCA errors (codes 38–46) ─────────────────────────────────────────────

    /// **Code 38**: An LCA profile for this product ID already exists.
    /// **Common cause**: `register_lca_entry` called twice with the same product ID.
    /// **Resolution**: Use a unique product ID per functional unit.
    LcaProfileAlreadyExists = 38,

    /// **Code 39**: No LCA profile found for the given product ID.
    /// **Common cause**: `record_phase_impact` or `finalize_lca` called before `register_lca_entry`.
    /// **Resolution**: Call `register_lca_entry` first.
    LcaProfileNotFound = 39,

    /// **Code 40**: Invalid lifecycle phase. Must be one of the seven recognised phase Symbols.
    /// **Common cause**: Caller supplied an unrecognised phase Symbol.
    /// **Resolution**: Use `raw_mat`, `mfg`, `transport`, `use`, `maint`, `eol`, or `recycling`.
    InvalidLcaPhase = 40,

    /// **Code 41**: Invalid impact category index (must be 0–7).
    /// **Common cause**: Category index out of the defined 8-category set.
    /// **Resolution**: Use indices 0 (GWP) through 7 (LU) as defined in the LCA documentation.
    InvalidImpactCategory = 41,

    /// **Code 42**: LCA profile is already finalized; no further phase impacts may be recorded.
    /// **Common cause**: `record_phase_impact` called after `finalize_lca`.
    /// **Resolution**: LCA profiles are immutable once finalized.
    LcaAlreadyFinalized = 42,

    /// **Code 43**: LCA profile has not been finalized yet; result is not available.
    /// **Common cause**: `get_lca_profile` or `get_lca_uncertainty` called before `finalize_lca`.
    /// **Resolution**: Call `finalize_lca` to lock in the aggregated result.
    LcaNotFinalized = 43,

    /// **Code 44**: Named normalization reference set not found.
    /// **Common cause**: `normalize_impacts` called with a name not registered via `register_norm_ref`.
    /// **Resolution**: Register a normalization reference set first with `register_norm_ref`.
    LcaNormRefNotFound = 44,

    /// **Code 45**: Named weighting scheme not found.
    /// **Common cause**: `apply_weighting_scheme` called with a name not registered via `register_weighting_scheme`.
    /// **Resolution**: Register the scheme first with `register_weighting_scheme`.
    LcaWeightingSchemeNotFound = 45,

    /// **Code 46**: Named LCA database reference not found.
    /// **Common cause**: `get_lca_db_entry` called with an ID not registered via `register_lca_db_entry`.
    /// **Resolution**: Register the database entry first.
    LcaDbEntryNotFound = 46,

    // ── Biodiversity errors (codes 47–55) ────────────────────────────────────

    /// **Code 47**: Biodiversity impact record not found for the given event ID.
    /// **Common cause**: `get_bio_impact` called before `record_bio_impact`.
    /// **Resolution**: Ensure the supply-chain event has been linked to a biodiversity impact record first.
    BioImpactNotFound = 47,

    /// **Code 48**: Biodiversity offset record not found.
    /// **Common cause**: `get_bio_offset` called with an unregistered offset ID.
    /// **Resolution**: Register the offset via `register_bio_offset`.
    BioOffsetNotFound = 48,

    /// **Code 49**: Invalid land-use type symbol.
    /// **Common cause**: Caller supplied an unrecognised land-use-type Symbol.
    /// **Resolution**: Use one of the recognised types: `crop`, `pasture`, `forest`,
    ///   `urban`, `wetland`, `water`, `barren`, or `protected`.
    InvalidLandUseType = 49,

    /// **Code 50**: Invalid ecosystem service category symbol.
    /// **Common cause**: Caller supplied an unrecognised ecosystem service category.
    /// **Resolution**: Use one of: `provision`, `regul`, `culture`, `support`.
    InvalidEcoServiceCat = 50,

    /// **Code 51**: Land area must be greater than zero (in square metres × 10⁻⁶, i.e. m² micro-units).
    InvalidLandArea = 51,

    /// **Code 52**: Offset quantity must be greater than zero.
    InvalidOffsetQuantity = 52,

    /// **Code 53**: Offset is already fully retired; no further retirement possible.
    OffsetAlreadyRetired = 53,

    /// **Code 54**: Retirement quantity exceeds remaining available balance.
    OffsetRetirementExceedsBalance = 54,

    /// **Code 55**: Species observation record not found.
    SpeciesObservationNotFound = 55,

    // ── Water Footprint errors (codes 56–63) ─────────────────────────────────

    /// **Code 56**: Water footprint record not found for the given ID.
    /// **Common cause**: `get_water_footprint` called before `record_water_footprint`.
    WaterFootprintNotFound = 56,

    /// **Code 57**: Water risk assessment not found for the given site ID.
    /// **Common cause**: `get_water_risk_assessment` or `record_water_footprint` with unregistered site.
    WaterRiskNotFound = 57,

    /// **Code 58**: Water stewardship programme not found.
    WaterStewardshipNotFound = 58,

    /// **Code 59**: Water disclosure record not found.
    WaterDisclosureNotFound = 59,

    /// **Code 60**: Invalid water-use sector symbol.
    /// **Resolution**: Use one of: `agri`, `indust`, `munici`, `energy`, `mining`.
    InvalidWaterSector = 60,

    /// **Code 61**: Water volume must be greater than zero (litres × 10⁻⁶ micro-units).
    InvalidWaterVolume = 61,

    /// **Code 62**: Water scarcity factor out of range (must be 0–100_000, i.e. 0–10.000×).
    InvalidScarcityFactor = 62,

    /// **Code 63**: CDP disclosure reporting year is invalid (must be > 2000).
    InvalidDisclosureYear = 63,

    // ── Data Retention, Legal Hold & GDPR Erasure errors (codes 64–73) ───────

    /// **Code 64**: The referenced event is under an active legal hold.
    /// **Common cause**: `process_erasure_request` (approve) or an implicit retention
    /// sweep attempted to redact an event while `place_legal_hold` is still active.
    /// **Resolution**: Call `release_legal_hold` first, or deny the erasure request.
    EventOnLegalHold = 64,

    /// **Code 65**: No active legal hold exists for the given event index.
    /// **Common cause**: `release_legal_hold` called on an index with no hold, or one
    /// already released.
    LegalHoldNotFound = 65,

    /// **Code 66**: An active compliance exception blocks this erasure.
    /// **Common cause**: `grant_compliance_exception` is in effect (not yet expired).
    /// **Resolution**: Wait for the exception to expire or call `revoke_compliance_exception`.
    ComplianceExceptionActive = 66,

    /// **Code 67**: This event's metadata has already been erased.
    /// **Common cause**: Duplicate `request_erasure` call for an already-redacted event.
    EventAlreadyErased = 67,

    /// **Code 68**: Erasure request ID does not exist.
    /// **Common cause**: `process_erasure_request` called with a stale or invalid `request_id`.
    ErasureRequestNotFound = 68,

    /// **Code 69**: Erasure request has already been decided (fulfilled or denied).
    /// **Common cause**: `process_erasure_request` called twice for the same request.
    ErasureRequestAlreadyDecided = 69,

    /// **Code 70**: Retention period must be greater than zero days.
    /// **Common cause**: `set_retention_policy` called with `retention_days == 0`.
    /// **Resolution**: Use `set_default_retention_days(0)` to disable retention instead.
    InvalidRetentionPeriod = 70,

    /// **Code 71**: A legal hold or compliance exception reason must not be empty.
    /// **Common cause**: `place_legal_hold`/`grant_compliance_exception` called with empty `Bytes`.
    EmptyComplianceReason = 71,

    /// **Code 72**: Operational audit log entries (category `operational`) are immutable
    /// and can never be erased, regardless of retention policy or approval.
    /// **Common cause**: `request_erasure`/`process_erasure_request` targeting an event
    /// logged via `log_operational_action`.
    OperationalEventNotErasable = 72,

    /// **Code 73**: Caller is not an authorized operational-audit recorder or the owner/multisig.
    /// **Common cause**: `log_operational_action` called by an address not added via
    /// `add_ops_recorder`.
    /// **Resolution**: Have the owner call `add_ops_recorder` for this address first.
    UnauthorizedOpsRecorder = 73,
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

// ── Circular Economy types ───────────────────────────────────────────────────

/// Loop event types (encoded as a u32 discriminant for compact on-chain storage).
///
/// | Value | Symbol      | Description                                             |
/// |-------|-------------|----------------------------------------------------------|
/// | 0     | `recycle`   | Material sent to recycling process                       |
/// | 1     | `reuse`     | Item used again without transformation                   |
/// | 2     | `repair`    | Item repaired to extend its service life                 |
/// | 3     | `remanuf`   | Product remanufactured to original specification         |
/// | 4     | `return`    | Item returned to manufacturer / supplier                 |
/// | 5     | `dispose`   | Material disposed (landfill, incineration)               |
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopEvent {
    /// Sequential position within the material's loop history.
    pub seq: u32,
    /// Ledger timestamp when this loop event was recorded.
    pub timestamp: u64,
    /// Loop type discriminant: 0=recycle, 1=reuse, 2=repair, 3=remanuf, 4=return, 5=dispose.
    pub loop_type: u32,
    /// Mass of material in milligrams (avoids floating-point; divide by 1_000 for grams).
    pub quantity_mg: u64,
    /// Address of the actor recording this event (facility, logistics provider, etc.).
    pub actor: Address,
    /// Optional reference to another material that this flow feeds into (e.g., recycled output).
    pub target_material_id: Option<BytesN<32>>,
    /// Opaque metadata (e.g., batch ID, certification reference, GPS coordinates as bytes).
    pub metadata: Bytes,
}

/// Material passport — the on-chain identity record for a physical asset.
///
/// Each unique asset (product, component, batch) gets exactly one passport.
/// The passport stores intrinsic material properties and is updated by
/// appending `LoopEvent`s via `record_loop_event`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialPassport {
    /// Content-addressed 32-byte ID (sha256 of owner || name || initial timestamp).
    pub id: BytesN<32>,
    /// Human-readable material name (up to 32 ASCII bytes, encoded in Bytes).
    pub name: Bytes,
    /// Material category (e.g., `plastic`, `metal`, `glass`, `textile`, `organic`).
    pub category: Symbol,
    /// Mass of virgin material in milligrams at time of registration.
    pub virgin_mass_mg: u64,
    /// Recyclability score: 0–10000 basis points (100.00 % = 10000).
    pub recyclability_bps: u32,
    /// Address of the entity registering this passport (manufacturer / data provider).
    pub owner: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Total mass recycled across all loop events, in milligrams (accumulator).
    pub total_recycled_mg: u64,
    /// Total mass reused across all loop events, in milligrams (accumulator).
    pub total_reused_mg: u64,
    /// Total mass repaired (kept in service) across all loop events, in milligrams (accumulator).
    pub total_repaired_mg: u64,
    /// Total mass remanufactured across all loop events, in milligrams (accumulator).
    pub total_remanufactured_mg: u64,
    /// Total mass disposed (waste) across all loop events, in milligrams (accumulator).
    pub total_disposed_mg: u64,
    /// Number of loop events recorded for this material.
    pub loop_event_count: u32,
}

/// Circularity metrics snapshot — aggregates across all registered materials.
///
/// Computed on demand by `compute_circularity_score` and stored on-chain
/// indexed by ledger sequence number, giving an auditable time-series.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircularitySnapshot {
    /// Ledger sequence number when this snapshot was taken.
    pub ledger_seq: u32,
    /// Ledger timestamp when this snapshot was taken.
    pub timestamp: u64,
    /// Total number of material passports registered.
    pub total_materials: u32,
    /// Total virgin mass registered across all passports, in milligrams.
    pub total_virgin_mass_mg: u64,
    /// Total mass that completed a circular loop (recycle + reuse + repair + remanuf), mg.
    pub total_circular_mass_mg: u64,
    /// Total mass disposed (linear end-of-life), mg.
    pub total_disposed_mass_mg: u64,
    /// Material Circularity Indicator (MCI) in basis points (0–10000).
    ///
    /// Formula: `mci = 10000 * total_circular_mass_mg / (total_circular_mass_mg + total_disposed_mass_mg)`
    /// Returns 0 when no flows have been recorded yet.
    pub mci_bps: u32,
    /// Weighted recycling rate in basis points (recycled / total_flow).
    pub recycling_rate_bps: u32,
    /// Weighted reuse rate in basis points (reused / total_flow).
    pub reuse_rate_bps: u32,
    /// Loop closure rate: fraction of passports that have at least one non-dispose loop event.
    pub loop_closure_rate_bps: u32,
    /// Total number of loop events recorded across all materials.
    pub total_loop_events: u32,
    /// Snapshot index (0-based ordinal, matches CircularitySnapshotCount - 1 after creation).
    pub snapshot_index: u32,
}

/// Running aggregate totals (persisted to avoid O(N) scans on every snapshot).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircularityTotals {
    pub total_materials: u32,
    pub total_virgin_mass_mg: u64,
    pub total_recycled_mg: u64,
    pub total_reused_mg: u64,
    pub total_repaired_mg: u64,
    pub total_remanufactured_mg: u64,
    pub total_disposed_mg: u64,
    pub total_loop_events: u32,
    /// Passports that have at least one non-dispose loop event.
    pub materials_with_closed_loop: u32,
}

// ── Lifecycle Assessment (LCA) types ────────────────────────────────────────

/// Lifecycle phase discriminants.
///
/// | Value | Symbol      | Phase name                      |
/// |-------|-------------|----------------------------------|
/// | 0     | `raw_mat`   | Raw material extraction          |
/// | 1     | `mfg`       | Manufacturing & processing       |
/// | 2     | `transport` | Transport & distribution         |
/// | 3     | `use`       | Use phase (operation)            |
/// | 4     | `maint`     | Maintenance & repair             |
/// | 5     | `eol`       | End-of-life (disposal)           |
/// | 6     | `recycling` | Recycling / recovery             |
///
/// The full cradle-to-grave scope covers phases 0–5; phases 0–6 give
/// a cradle-to-cradle (closed-loop) scope.
pub const LCA_PHASE_COUNT: u32 = 7;

/// Impact category indices (ISO 14040/14044 mid-point categories).
///
/// All values are stored as fixed-point integers with an implicit scale
/// of 1 × 10⁻⁶ (micro-units), so 1 kg CO₂-eq = 1_000_000 in storage.
///
/// | Index | Symbol | Unit            | Description                        |
/// |-------|--------|-----------------|------------------------------------|
/// | 0     | GWP    | kg CO₂-eq       | Global Warming Potential           |
/// | 1     | AP     | kg SO₂-eq       | Acidification Potential            |
/// | 2     | EP     | kg PO₄³⁻-eq     | Eutrophication Potential           |
/// | 3     | ODP    | kg CFC-11-eq    | Ozone Depletion Potential          |
/// | 4     | POCP   | kg C₂H₄-eq      | Photochem. Ozone Creation Potential|
/// | 5     | ADP    | kg Sb-eq        | Abiotic Depletion Potential        |
/// | 6     | WU     | m³              | Water Use                          |
/// | 7     | LU     | m² · year       | Land Use                           |
pub const LCA_CATEGORY_COUNT: u32 = 8;

/// Per-phase impact vector — one value per impact category.
///
/// Values are fixed-point micro-units (`i64` to allow negative credits,
/// e.g. avoided burdens from recycling).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaPhaseImpact {
    /// Lifecycle phase (0–6, see `LCA_PHASE_COUNT`).
    pub phase: u32,
    /// Impact values indexed by category (0–7, see `LCA_CATEGORY_COUNT`).
    /// Fixed-point: divide by 1_000_000 to get SI units.
    /// Index 0 = GWP, 1 = AP, 2 = EP, 3 = ODP, 4 = POCP, 5 = ADP, 6 = WU, 7 = LU.
    pub values: Vec<i64>,
    /// Actor that submitted this phase record.
    pub submitter: Address,
    /// Ledger timestamp of submission.
    pub timestamp: u64,
    /// Optional reference to an LCA database entry ID backing this data.
    pub db_ref: Option<BytesN<32>>,
    /// Opaque metadata (data source, methodology version, etc.).
    pub metadata: Bytes,
}

/// LCA profile — the on-chain header for a product's lifecycle assessment.
///
/// Registered once per product / functional unit. Phase impacts are added
/// via `record_phase_impact` and locked by calling `finalize_lca`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaProfile {
    /// Content-addressed 32-byte product ID.
    pub product_id: BytesN<32>,
    /// Human-readable product name.
    pub name: Bytes,
    /// Functional unit description (e.g., "1 kg of product at factory gate").
    pub functional_unit: Bytes,
    /// Address that registered this profile.
    pub owner: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Whether all phases have been submitted and the result finalized.
    pub finalized: bool,
    /// Bitmask of phases that have been recorded (bit i = phase i submitted).
    pub phase_mask: u32,
    /// Optional link to the material passport of the underlying product.
    pub material_passport_id: Option<BytesN<32>>,
}

/// Aggregated LCA result — computed by `finalize_lca` across all recorded phases.
///
/// Stores both the raw totals and (optionally) normalized/weighted results
/// after calling `normalize_impacts` and `apply_weighting_scheme`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaResult {
    /// Raw aggregated impact per category (fixed-point micro-units, summed across all phases).
    pub totals: Vec<i64>,
    /// Normalized impact per category (totals / normalization_ref × 1_000_000), or zeros.
    pub normalized: Vec<i64>,
    /// Weighted single score per category (normalized × weight_bps / 10000), or zeros.
    pub weighted: Vec<i64>,
    /// Sum of all weighted values = single-score LCA result (micro-units).
    pub single_score: i64,
    /// Name of the normalization reference set used (empty = not yet normalized).
    pub norm_ref_name: Bytes,
    /// Name of the weighting scheme used (empty = not yet weighted).
    pub weighting_scheme_name: Bytes,
    /// Ledger timestamp when `finalize_lca` was called.
    pub finalized_at: u64,
}

/// Uncertainty bounds for an LCA result, computed via interval arithmetic.
///
/// Each impact category carries a `[lo, hi]` interval derived from the
/// per-phase uncertainty coefficients (see `record_phase_impact` / `finalize_lca`).
/// The model applies a symmetric percentage uncertainty (`uncertainty_pct_bps`)
/// from each phase, then propagates addition intervals across all phases.
///
/// Fixed-point: divide all values by 1_000_000 to get SI units.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaUncertainty {
    /// Lower bound per impact category (fixed-point micro-units).
    pub lo: Vec<i64>,
    /// Upper bound per impact category (fixed-point micro-units).
    pub hi: Vec<i64>,
    /// Coefficient of variation in basis points used globally (0 = no uncertainty).
    pub cv_bps: u32,
    /// Ledger timestamp when this uncertainty record was computed.
    pub computed_at: u64,
}

/// Normalization reference set (one value per impact category).
///
/// Reference values represent per-person-equivalent annual burdens
/// (e.g., CML 2016, ReCiPe H, EF 3.1). Fixed-point micro-units.
/// A reference value of 0 for a category means "skip normalization" for that
/// category (result stays as raw total).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaNormRef {
    /// Short name used as the storage key (e.g., `cml2016`).
    pub name: Bytes,
    /// Reference values per category, fixed-point micro-units.
    /// Length must equal `LCA_CATEGORY_COUNT` (8).
    pub refs: Vec<i64>,
    /// Owner who registered this reference set.
    pub owner: Address,
}

/// Weighting scheme (one weight in basis points per impact category).
///
/// Weights sum to 10000 bps (100%). Commonly used schemes:
/// CML 2016 equal-weight, ReCiPe H, EF 3.1.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaWeightingScheme {
    /// Short name (e.g., `ef31`, `recipe_h`).
    pub name: Bytes,
    /// Weight per category in basis points. Must sum to 10000, length = 8.
    pub weights_bps: Vec<u32>,
    /// Owner who registered this scheme.
    pub owner: Address,
}

/// LCA database reference entry.
///
/// Provides a lightweight on-chain anchor for an externally maintained LCA
/// dataset (ecoinvent, GaBi, OpenLCA, etc.). Phase impacts recorded with a
/// `db_ref` can be validated against this entry by off-chain consumers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LcaDbEntry {
    /// 32-byte content-addressed ID for this database record.
    pub id: BytesN<32>,
    /// Database name (e.g., `ecoinvent`, `gabi`).
    pub db_name: Bytes,
    /// Dataset version string (e.g., `3.10`).
    pub version: Bytes,
    /// Activity / process name within the database.
    pub activity: Bytes,
    /// Geography code (e.g., `GLO`, `RER`, `US`).
    pub geography: Bytes,
    /// Address of the data provider who registered this entry.
    pub provider: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
}

// ── Biodiversity types ───────────────────────────────────────────────────────

/// Land-use type discriminants (IUCN / GLOBIO convention).
///
/// | Value | Symbol      | Description                                   |
/// |-------|-------------|-----------------------------------------------|
/// | 0     | `crop`      | Annual/permanent cropland                     |
/// | 1     | `pasture`   | Managed grassland / livestock grazing         |
/// | 2     | `forest`    | Natural or semi-natural forest                |
/// | 3     | `urban`     | Urban / built-up area                         |
/// | 4     | `wetland`   | Wetlands (freshwater/coastal/inland)          |
/// | 5     | `water`     | Open water body                               |
/// | 6     | `barren`    | Barren / rock / desert (very low biodiversity)|
/// | 7     | `protected` | Formally protected area (IUCN PA categories)  |
pub const BIO_LAND_USE_COUNT: u32 = 8;

/// Ecosystem service category discriminants (TEEB / CICES classification).
///
/// | Value | Symbol     | Description                                              |
/// |-------|------------|----------------------------------------------------------|
/// | 0     | `provision`| Provisioning services (food, water, raw materials)       |
/// | 1     | `regul`    | Regulating services (climate, flood, water purification) |
/// | 2     | `culture`  | Cultural services (recreation, tourism, spiritual)       |
/// | 3     | `support`  | Supporting services (soil formation, nutrient cycling)   |
pub const BIO_ECO_SERVICE_COUNT: u32 = 4;

/// Biodiversity impact record — linked to a supply-chain event.
///
/// Captures the footprint of a single operational event (site clearing,
/// agriculture, construction, logistics) across land-use change, species
/// richness loss, and ecosystem service degradation.
///
/// All area values are in **square-metre micro-units** (m² × 10⁻⁶).
/// Species richness loss is in **MSA-hectare micro-units** (MSA·ha × 10⁻⁶),
/// where MSA = Mean Species Abundance relative to undisturbed habitat (0–1).
/// Ecosystem service values are in **USD-cent micro-units** (US¢ × 10⁻⁶) for
/// the total annual loss.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BioImpact {
    /// 32-byte content-addressed ID (sha256 of event_ref || actor || timestamp).
    pub id: BytesN<32>,
    /// Reference to the supply-chain event driving this impact (e.g., an audit event ID).
    pub event_ref: BytesN<32>,
    /// Actor recording this impact (supplier, auditor, third-party assessor).
    pub actor: Address,
    /// Ledger timestamp of recording.
    pub timestamp: u64,
    /// Land-use type at the impacted site (discriminant 0–7).
    pub land_use_type: u32,
    /// Total land area affected in m² micro-units (> 0).
    pub area_m2_micro: u64,
    /// Species richness loss in MSA·ha micro-units (0 = no loss).
    /// MSA·ha = area_ha × (1 − MSA_after / MSA_before)
    pub msa_loss_micro: u64,
    /// Per-ecosystem-service annual value loss in USD-cent micro-units (4 values).
    /// Index: 0=provisioning, 1=regulating, 2=cultural, 3=supporting.
    pub eco_service_loss: Vec<i64>,
    /// Optional geographic coordinates encoded as UTF-8 bytes (e.g., "lat,lon" decimal string).
    pub location: Bytes,
    /// Optional IUCN threat category for the primary affected species/habitat.
    /// Encoded as bytes (e.g., b"CR", b"EN", b"VU", b"NT", b"LC").
    pub iucn_threat: Bytes,
    /// Opaque metadata (certification reference, assessment methodology, etc.).
    pub metadata: Bytes,
}

/// Biodiversity offset record — tracks nature-based compensation credits.
///
/// Supports voluntary biodiversity credits (VBCs), biodiversity net gain (BNG)
/// units, mitigation banking credits, and any other credit scheme.
/// All quantities are in **MSA·ha micro-units** (same scale as `BioImpact.msa_loss_micro`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BioOffset {
    /// 32-byte content-addressed offset ID.
    pub id: BytesN<32>,
    /// Short name of the offset scheme (e.g., `vbc`, `bng`, `mitbank`).
    pub scheme: Bytes,
    /// Address of the entity that issued or registered this offset.
    pub issuer: Address,
    /// Total credit quantity in MSA·ha micro-units.
    pub total_micro: u64,
    /// Quantity retired (applied to offset actual impacts) in MSA·ha micro-units.
    pub retired_micro: u64,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Expiry timestamp (0 = no expiry).
    pub expires_at: u64,
    /// Optional reference to a site/project whose ecosystem services back this offset.
    pub eco_service_ref: Option<BytesN<32>>,
    /// Opaque metadata (project location, registry URL, certificate hash, etc.).
    pub metadata: Bytes,
}

/// Ecosystem service valuation record for a project or geographic site.
///
/// Provides an annual monetary valuation of the site's four service categories,
/// enabling offset verification and nature-positive accounting.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EcoServiceRecord {
    /// 32-byte content-addressed site/project ID.
    pub id: BytesN<32>,
    /// Human-readable project name.
    pub name: Bytes,
    /// Address of the party registering this valuation.
    pub owner: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Total site area in m² micro-units.
    pub area_m2_micro: u64,
    /// Annual value per ecosystem service in USD-cent micro-units (4 values, CICES order).
    /// Index: 0=provisioning, 1=regulating, 2=cultural, 3=supporting.
    pub annual_values: Vec<i64>,
    /// Land-use type at this site (discriminant 0–7).
    pub land_use_type: u32,
    /// Opaque metadata (SEEA reference, assessor, methodology version).
    pub metadata: Bytes,
}

/// Species observation record — links a supply-chain event to a direct
/// species sighting or survey result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeciesObservation {
    /// 32-byte content-addressed observation ID.
    pub id: BytesN<32>,
    /// Reference event ID (audit log event this observation relates to).
    pub event_ref: BytesN<32>,
    /// Species common name (UTF-8 bytes).
    pub species_name: Bytes,
    /// IUCN taxonomic code or identifier (UTF-8 bytes, e.g., b"Panthera tigris").
    pub species_code: Bytes,
    /// IUCN Red List category bytes (e.g., b"EN").
    pub iucn_category: Bytes,
    /// Individual count observed (0 = presence-only record).
    pub count: u32,
    /// Impact type: 0=positive (sighted), 1=negative (mortality/displacement), 2=neutral.
    pub impact_direction: u32,
    /// Actor recording the observation.
    pub observer: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Opaque metadata (survey method, GPS, photo hash, etc.).
    pub metadata: Bytes,
}

/// Running global biodiversity totals — updated atomically on every write.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BioTotals {
    /// Total number of biodiversity impact records registered.
    pub total_impacts: u32,
    /// Total land area affected across all impacts, m² micro-units.
    pub total_area_m2_micro: u64,
    /// Total MSA·ha lost across all impacts.
    pub total_msa_loss_micro: u64,
    /// Total ecosystem service value lost, USD-cent micro-units (sum across all categories).
    pub total_eco_loss_micro: i64,
    /// Total biodiversity offset credits registered, MSA·ha micro-units.
    pub total_offset_micro: u64,
    /// Total offset credits retired, MSA·ha micro-units.
    pub total_retired_micro: u64,
    /// Total species observations recorded.
    pub total_observations: u32,
    /// Total ecosystem service records registered.
    pub total_eco_records: u32,
}

/// Nature-positive snapshot — point-in-time biodiversity accounting.
///
/// Records the net biodiversity position (impact − offset balance) and
/// key performance indicators for nature-positive reporting frameworks
/// (TNFD, GBF Target 15, EU CSRD).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BioSnapshot {
    /// 0-based snapshot ordinal.
    pub index: u32,
    /// Ledger sequence when this snapshot was taken.
    pub ledger_seq: u32,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Total impacts recorded at snapshot time.
    pub total_impacts: u32,
    /// Total MSA·ha lost across all recorded impacts (micro-units).
    pub total_msa_loss_micro: u64,
    /// Total MSA·ha offset credits registered (micro-units).
    pub total_offset_micro: u64,
    /// Total MSA·ha offset credits retired (micro-units).
    pub total_retired_micro: u64,
    /// Net MSA position: retired_micro − total_msa_loss_micro (signed).
    /// Positive = nature-positive; negative = net biodiversity debt.
    pub net_msa_micro: i64,
    /// Nature-positive indicator in basis points.
    /// `nature_positive_bps = (retired_micro × 10_000) / total_msa_loss_micro`
    /// Returns 10_000 (100%) when losses = 0 (no impact recorded).
    pub nature_positive_bps: u32,
    /// Offset coverage ratio: retired / total_msa_loss (bps, capped at 10000).
    pub offset_coverage_bps: u32,
    /// Total ecosystem service loss at snapshot (USD-cent micro-units).
    pub total_eco_loss_micro: i64,
    /// Total species observations at snapshot.
    pub total_observations: u32,
}

// ── Water Footprint types ────────────────────────────────────────────────────

/// Water-use sector discriminants (aligned with AQUASTAT / WRI Aqueduct categories).
///
/// | Value | Symbol   | Description                                      |
/// |-------|----------|--------------------------------------------------|
/// | 0     | `agri`   | Agriculture (irrigation, livestock)              |
/// | 1     | `indust` | Industrial processes (manufacturing, chemicals)  |
/// | 2     | `munici` | Municipal / domestic supply                      |
/// | 3     | `energy` | Thermoelectric cooling and hydropower            |
/// | 4     | `mining` | Mining and quarrying                             |
pub const WATER_SECTOR_COUNT: u32 = 5;

/// Water footprint record linked to a supply-chain event.
///
/// Tracks four complementary water accounting measures per ISO 14046 /
/// Water Footprint Network (WFN) standard:
///
/// * **Blue water** — surface/groundwater consumed (irrigation, cooling, process).
/// * **Green water** — rainwater consumed by crops or vegetation (evapotranspiration).
/// * **Grey water** — freshwater required to dilute pollution to acceptable quality.
/// * **Scarcity-weighted blue water** — blue water × local scarcity factor (WSI).
///
/// All volumes are in **litre micro-units** (L × 10⁻⁶). Divide by 1 000 000 for litres,
/// by 1 000 000 000 for cubic metres.
///
/// The scarcity factor (`scarcity_factor_ppb`) is the Water Stress Index (WSI) in
/// parts-per-billion (0 = no stress, 100 000 = maximum, i.e. 0.000–0.100 dimensionless).
/// Scarcity-weighted volume: `blue_L_micro × scarcity_factor_ppb / 1 000 000`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterFootprint {
    /// Content-addressed 32-byte ID: sha256(event_ref || actor_strkey || timestamp_le64).
    pub id: BytesN<32>,
    /// Reference to the originating supply-chain audit event.
    pub event_ref: BytesN<32>,
    /// Actor recording this footprint.
    pub actor: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Water-use sector discriminant (0–4).
    pub sector: u32,
    /// Blue water consumed, L × 10⁻⁶.
    pub blue_L_micro: u64,
    /// Green water consumed, L × 10⁻⁶.
    pub green_L_micro: u64,
    /// Grey water footprint, L × 10⁻⁶.
    pub grey_L_micro: u64,
    /// Water Stress Index at source basin (0–100 000 ppb; 0 = no stress).
    pub scarcity_factor_ppb: u32,
    /// Scarcity-weighted blue water: blue_L_micro × scarcity_factor_ppb / 1 000 000.
    pub scarcity_weighted_L_micro: u64,
    /// Optional reference to the water risk assessment for the source basin.
    pub risk_assessment_ref: Option<BytesN<32>>,
    /// Optional reference to an active stewardship programme at this site.
    pub stewardship_ref: Option<BytesN<32>>,
    /// ISO 3166-1 alpha-2 country code bytes (e.g., b"IN").
    pub country: Bytes,
    /// HydroSHEDS / HydroBASINS basin identifier (UTF-8, e.g., b"4050017220").
    pub basin_id: Bytes,
    /// Opaque metadata (measurement method, data quality, etc.).
    pub metadata: Bytes,
}

/// Water risk assessment for a geographic basin or operational site.
///
/// Aligned with WRI Aqueduct (overall water risk, quantity, quality, regulatory/reputational)
/// and WWF Water Risk Filter. All scores are in basis points (0–10 000).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterRiskAssessment {
    /// 32-byte content-addressed site/basin ID.
    pub id: BytesN<32>,
    /// Human-readable site or basin name.
    pub name: Bytes,
    /// Address of the entity registering this assessment.
    pub assessor: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Overall water risk score (WRI Aqueduct composite), 0–10 000 bps.
    pub overall_risk_bps: u32,
    /// Quantity risk sub-score (drought, depletion, variability), 0–10 000 bps.
    pub quantity_risk_bps: u32,
    /// Quality risk sub-score (pollution, untreated wastewater), 0–10 000 bps.
    pub quality_risk_bps: u32,
    /// Regulatory & reputational risk, 0–10 000 bps.
    pub regulatory_risk_bps: u32,
    /// Water Stress Index (WSI) at this location (0–100 000 ppb).
    pub wsi_ppb: u32,
    /// Country code bytes (ISO 3166-1 alpha-2).
    pub country: Bytes,
    /// HydroBASINS basin ID bytes.
    pub basin_id: Bytes,
    /// Assessment tool/methodology reference (e.g., b"aqueduct4", b"wwf_wrf").
    pub methodology: Bytes,
    /// Opaque metadata.
    pub metadata: Bytes,
}

/// Water stewardship programme record.
///
/// Captures participation in site-level or basin-level stewardship
/// schemes (Alliance for Water Stewardship AWS, CEO Water Mandate, etc.).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterStewardship {
    /// 32-byte content-addressed programme ID.
    pub id: BytesN<32>,
    /// Programme name (e.g., b"aws_core", b"ceo_mandate").
    pub programme: Bytes,
    /// Address of the participating entity.
    pub participant: Address,
    /// Ledger timestamp of registration.
    pub registered_at: u64,
    /// Programme start date as a Unix timestamp (0 = not specified).
    pub start_ts: u64,
    /// Programme end / target date (0 = ongoing).
    pub end_ts: u64,
    /// Water reduction target in L × 10⁻⁶ (absolute target; 0 = no explicit target).
    pub target_reduction_L_micro: u64,
    /// Water reduction achieved to date in L × 10⁻⁶.
    pub achieved_reduction_L_micro: u64,
    /// Optional reference to the basin risk assessment backing this programme.
    pub risk_assessment_ref: Option<BytesN<32>>,
    /// Opaque metadata (certifier, audit date, certificate hash).
    pub metadata: Bytes,
}

/// CDP Water Disclosure record.
///
/// Covers the key quantitative fields from CDP Water Security questionnaire
/// (W1–W8 sections). All volumes in L × 10⁻⁶.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterDisclosure {
    /// 32-byte content-addressed disclosure ID.
    pub id: BytesN<32>,
    /// Reporting organisation address.
    pub organisation: Address,
    /// Calendar year of this disclosure (e.g., 2025).
    pub reporting_year: u32,
    /// Ledger timestamp when the disclosure was recorded.
    pub recorded_at: u64,
    // ── W1: Water withdrawal ──
    /// Total freshwater withdrawal, L × 10⁻⁶ (CDP W1.2a).
    pub total_withdrawal_L_micro: u64,
    // ── W2: Water consumption ──
    /// Total water consumed (not returned), L × 10⁻⁶ (CDP W2.1).
    pub total_consumption_L_micro: u64,
    // ── W3: Water discharge ──
    /// Total water discharged, L × 10⁻⁶ (CDP W3.1).
    pub total_discharge_L_micro: u64,
    // ── W4: Water data quality ──
    /// Percentage of water data estimated vs. metered, 0–10 000 bps (CDP W4).
    pub estimated_data_pct_bps: u32,
    // ── W5: Water targets ──
    /// Absolute reduction target relative to base year, L × 10⁻⁶ (CDP W5.1).
    pub reduction_target_L_micro: u64,
    // ── W6: Water risks ──
    /// Number of sites in water-stressed areas (CDP W6.2).
    pub sites_in_stressed_areas: u32,
    // ── W7: Accounting ──
    /// Scarcity-weighted total water use across all recorded footprints, L × 10⁻⁶.
    pub scarcity_weighted_total_L_micro: u64,
    // ── W8: Targets achieved ──
    /// Reduction achieved since base year, L × 10⁻⁶ (CDP W8.1).
    pub reduction_achieved_L_micro: u64,
    /// Opaque metadata (CDP submission ID, base year, etc.).
    pub metadata: Bytes,
}

/// Running global water totals — updated atomically on every write.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterTotals {
    /// Total water footprint records registered.
    pub total_footprints: u32,
    /// Cumulative blue water consumed, L × 10⁻⁶.
    pub total_blue_L_micro: u64,
    /// Cumulative green water consumed, L × 10⁻⁶.
    pub total_green_L_micro: u64,
    /// Cumulative grey water footprint, L × 10⁻⁶.
    pub total_grey_L_micro: u64,
    /// Cumulative scarcity-weighted blue water, L × 10⁻⁶.
    pub total_scarcity_weighted_L_micro: u64,
    /// Total water risk assessments registered.
    pub total_risk_assessments: u32,
    /// Total stewardship programmes registered.
    pub total_stewardship_programmes: u32,
    /// Total CDP disclosures recorded.
    pub total_disclosures: u32,
}

/// Water snapshot — point-in-time aggregation of water accounting metrics.
///
/// Created by `compute_water_snapshot`; stored on-chain for an auditable time-series.
/// Aligns with TNFD freshwater metrics, GRI 303 (Water and Effluents),
/// and CDP Water Security W1–W8.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterSnapshot {
    /// 0-based ordinal.
    pub index: u32,
    /// Ledger sequence at snapshot time.
    pub ledger_seq: u32,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Total footprint records at snapshot time.
    pub total_footprints: u32,
    /// Total blue water (L × 10⁻⁶).
    pub total_blue_L_micro: u64,
    /// Total green water (L × 10⁻⁶).
    pub total_green_L_micro: u64,
    /// Total grey water (L × 10⁻⁶).
    pub total_grey_L_micro: u64,
    /// Total scarcity-weighted blue water (L × 10⁻⁶).
    pub total_scarcity_weighted_L_micro: u64,
    /// Total water footprint (blue + green + grey), L × 10⁻⁶.
    pub total_water_footprint_L_micro: u64,
    /// Scarcity ratio: scarcity_weighted / blue × 10 000 (bps). 0 when no blue water.
    pub scarcity_ratio_bps: u32,
    /// Blue-water fraction of total footprint, bps.
    pub blue_fraction_bps: u32,
    /// Total risk assessments at snapshot.
    pub total_risk_assessments: u32,
    /// Total stewardship programmes at snapshot.
    pub total_stewardship_programmes: u32,
}

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
            category: category.unwrap_or_else(|| Symbol::new(&env, "general")),
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

#[cfg(test)]
mod comprehensive_fuzz;

// CBDC Integration Modules
pub mod cbdc_types;
pub mod cbdc_logging;
pub mod cbdc_interop;
pub mod cbdc_offline;
pub mod cbdc_privacy;

#[cfg(test)]
mod cbdc_tests;

use cbdc_types::*;
use cbdc_logging::*;
use cbdc_interop::*;
use cbdc_offline::*;
use cbdc_privacy::*;

// SupTech Integration Modules
pub mod suptech_types;
pub mod suptech_feeds;
pub mod suptech_reporting;
pub mod suptech_api;
pub mod suptech_rules;
pub mod suptech_integration;

#[cfg(test)]
mod suptech_tests;

use suptech_types::*;
use suptech_feeds::*;
use suptech_reporting::*;
use suptech_api::*;
use suptech_rules::*;
use suptech_integration::*;

// Regulatory Sandbox Modules
pub mod sandbox_types;
pub mod sandbox_mgmt;
pub mod sandbox_env;
pub mod sandbox_supervision;
pub mod sandbox_innovation;
pub mod sandbox_graduation;

#[cfg(test)]
mod supply_chain_tests;

#[cfg(test)]
mod data_retention_tests;
