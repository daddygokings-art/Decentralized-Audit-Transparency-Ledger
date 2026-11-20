//! # Contract Event Sourcing with CQRS Pattern (Issue #412)
//!
//! Implements Command Query Responsibility Segregation (CQRS) with event sourcing
//! for `AuditLedger`.
//!
//! - **Command side**: Accepts and validates write operations (`LogEventCommand`,
//!   `UpdateEventCommand`, `GovernanceCommand`), produces domain events, appends them
//!   to the monotonic append-only event store, and updates projections.
//! - **Query side**: Serves read-optimized representations (`EventView`, `EventProjection`,
//!   `ProjectionSnapshot`) and materialized views without modifying ledger state.
//! - **Event Store**: Full history of sequenced events stored in immutable append-only log.
//! - **Projection Rebuilding**: Rebuilds projection state by replaying events from genesis
//!   or fast-forwarding from the latest snapshot.
//! - **Snapshotting**: Point-in-time state capture for performant projection reconstruction.
//! - **Materialized Views**: Per-type and per-submitter indexes for low-gas query operations.
//!
//! ## Storage tiers & TTL policy
//!
//! Every [`CqrsStorageKey`] is assigned to exactly one Soroban storage tier
//! according to its retention SLA (see [`CqrsStorageKey::tier`]):
//!
//! | Tier         | Keys                                                        | Retention SLA |
//! |--------------|-------------------------------------------------------------|---------------|
//! | `Instance`   | `Seq`, `Proj`, `SnapCnt`                                    | lives as long as the contract; small, read on every command |
//! | `Persistent` | `Event`, `IdSeq`, `Snap`, `TypeIdx`, `TypeCnt`, `SubIdx`, `SubCnt` | archival-critical audit records; bumped on write and via the permissionless `extend_*_ttl` helpers |
//! | `Temporary`  | `Idem`                                                      | idempotency replay window ([`CQRS_IDEMPOTENCY_TTL_LEDGERS`]); safe to expire |
//!
//! Previously every key lived in instance storage, so each command re-wrote a
//! single ledger entry whose size grew with the whole event history. Moving
//! per-event records to dedicated persistent entries keeps the per-command
//! write footprint constant.
//!
//! ## Bounds & failure model
//!
//! All arithmetic is checked; boundary violations return a typed
//! [`CqrsError`] (rolling the whole invocation back) instead of panicking or
//! silently saturating:
//!
//! | Bound | Constant | Error |
//! |-------|----------|-------|
//! | Event sequence | `u32::MAX` | [`CqrsError::SequenceOverflow`] |
//! | Counters (projection totals, index lengths, snapshot ids, versions) | `u32::MAX` | [`CqrsError::CounterOverflow`] |
//! | Metadata payload | [`CQRS_MAX_METADATA_BYTES`] | [`CqrsError::MetadataTooLarge`] |
//! | Events replayed by one rebuild | [`CQRS_MAX_REPLAY_EVENTS`] | [`CqrsError::ReplayWindowTooLarge`] |
//!
//! Collections are capped before they are written:
//! `EventProjection::type_counts` holds at most [`CQRS_MAX_PROJECTION_TYPES`]
//! entries and `submitter_counts` at most [`CQRS_MAX_PROJECTION_SUBMITTERS`];
//! further distinct keys are still counted exactly by the per-key `TypeCnt` /
//! `SubCnt` materialized counters but are not added to the summary vectors.
//! Read paths visit at most [`CQRS_MAX_QUERY_SCAN`] stored events and return
//! at most [`CQRS_MAX_QUERY_LIMIT`] items per call.
//!
//! ### Worst-case footprint per invocation
//!
//! | Entry point | Ledger entries read / written | Notes |
//! |-------------|-------------------------------|-------|
//! | `record_log_event` | ~7 reads / 8 writes | each written record ≤ `CQRS_MAX_METADATA_BYTES` + ~210 B; projection ≤ 64 types × ~24 B + 128 submitters × ~52 B ≈ 8.2 KB |
//! | `record_update_event` / `record_governance_command` | 2 reads / 3-4 writes | |
//! | `query_events` / materialized reads | ≤ `CQRS_MAX_QUERY_SCAN` + `CQRS_MAX_QUERY_LIMIT` reads | read-only |
//! | `rebuild_projection` | ≤ `CQRS_MAX_REPLAY_EVENTS` reads / 1 write | O(events × (types + submitters)) CPU, bounded by the caps above |
//!
//! There are no `i128` amounts in this module; `u64` timestamps are stored
//! verbatim and accepted up to `u64::MAX`.
//!
//! ## Serialized layout packing
//!
//! Stored records use positional tuple structs ([`PackedCqrsEvent`],
//! [`PackedProjection`], [`PackedSnapshot`]) which serialise as `ScVec`
//! instead of an `ScMap` keyed by field-name symbols, the command kind is a
//! `u32` discriminant ([`CqrsCommandKind`]) instead of a `Symbol`, and the
//! sequence number / snapshot id are carried only by the storage key. The
//! public read models (`CqrsStoredEvent`, `EventView`, `EventProjection`,
//! `ProjectionSnapshot`) are unchanged for callers.

use soroban_sdk::{
    contracterror, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec,
};

// ── Error Codes ─────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CqrsError {
    NotAuthorized = 600,
    EventNotFound = 601,
    InvalidSequence = 602,
    SnapshotNotFound = 603,
    MetadataTooLarge = 604,
    OperationPaused = 605,
    SubmitterBlocked = 606,
    InvalidCommand = 607,
    DuplicateEvent = 608,
    /// The event sequence counter would exceed `u32::MAX`.
    SequenceOverflow = 609,
    /// A projection / index / snapshot / version counter would overflow.
    CounterOverflow = 610,
    /// A rebuild would replay more than `CQRS_MAX_REPLAY_EVENTS` events.
    ReplayWindowTooLarge = 611,
}

// ── Bounds ──────────────────────────────────────────────────────────────────

/// Hard ceiling on event metadata accepted by the CQRS store, independent of
/// the (owner-configurable) ledger-wide metadata limit.
pub const CQRS_MAX_METADATA_BYTES: u32 = 8_192;
/// Maximum distinct event types tracked in `EventProjection::type_counts`.
pub const CQRS_MAX_PROJECTION_TYPES: u32 = 64;
/// Maximum distinct submitters tracked in `EventProjection::submitter_counts`.
pub const CQRS_MAX_PROJECTION_SUBMITTERS: u32 = 128;
/// Maximum stored events a single read call may visit.
pub const CQRS_MAX_QUERY_SCAN: u32 = 500;
/// Maximum items returned by a single read call.
pub const CQRS_MAX_QUERY_LIMIT: u32 = 50;
/// Default page size when a caller passes `limit == 0`.
pub const CQRS_DEFAULT_QUERY_LIMIT: u32 = 20;
/// Maximum events replayed by one `rebuild_projection` call. Take snapshots
/// at least this often to keep rebuilds possible.
pub const CQRS_MAX_REPLAY_EVENTS: u32 = 1_000;

// ── Storage tier & TTL policy ───────────────────────────────────────────────

/// Bump instance storage when its remaining TTL falls below ~1 day.
pub const CQRS_INSTANCE_TTL_THRESHOLD: u32 = 17_280;
/// Extend instance storage to ~30 days.
pub const CQRS_INSTANCE_TTL_EXTEND_TO: u32 = 518_400;
/// Bump archival-critical persistent records when below ~30 days.
pub const CQRS_PERSISTENT_TTL_THRESHOLD: u32 = 518_400;
/// Extend archival-critical persistent records to ~120 days.
pub const CQRS_PERSISTENT_TTL_EXTEND_TO: u32 = 2_073_600;
/// Idempotency keys only need to survive the client retry window (~1 day).
pub const CQRS_IDEMPOTENCY_TTL_LEDGERS: u32 = 17_280;

/// Soroban storage tier a [`CqrsStorageKey`] is written to.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CqrsStorageTier {
    Instance,
    Persistent,
    Temporary,
}

/// Kind of command that produced a stored event. Serialised as a `u32`
/// discriminant (4 bytes) rather than a `Symbol`.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CqrsCommandKind {
    Log = 0,
    Update = 1,
    Governance = 2,
}

impl CqrsCommandKind {
    /// Human-readable symbol for the command kind (`log`, `update`, `governance`).
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            CqrsCommandKind::Log => Symbol::new(env, "log"),
            CqrsCommandKind::Update => Symbol::new(env, "update"),
            CqrsCommandKind::Governance => Symbol::new(env, "governance"),
        }
    }
}

// ── Command Side Models (Write Path) ────────────────────────────────────────

/// Command to log a new audit event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEventCommand {
    pub submitter: Address,
    pub event_type: Symbol,
    pub metadata: Bytes,
    pub category: Option<Symbol>,
    pub sub_event_type: Option<Symbol>,
    pub idempotency_key: Option<BytesN<32>>,
    pub force: bool,
}

/// Command to update an existing audit event's metadata.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateEventCommand {
    pub caller: Address,
    pub event_id: BytesN<32>,
    pub new_metadata: Bytes,
    pub version_tag: Option<Symbol>,
}

/// Command to execute a governance action on the ledger.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceCommand {
    pub caller: Address,
    pub action: Symbol,
    pub target_address: Option<Address>,
    pub param_u32: Option<u32>,
    pub param_symbol: Option<Symbol>,
    pub param_bool: Option<bool>,
}

/// Standard response returned by write-side command handlers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    pub success: bool,
    pub event_id: Option<BytesN<32>>,
    pub sequence_number: u32,
    pub error_code: Option<u32>,
    pub message: Option<Symbol>,
}

// ── Event Store (Append-Only Log) ───────────────────────────────────────────

/// Position of an event inside its materialized indexes.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CqrsIndexPos {
    /// Not indexed (`Update` / `Governance` commands).
    Unindexed,
    /// `(type_index_position, submitter_index_position)` of an indexed `Log` event.
    At(u32, u32),
}

/// Logical view of a record in the CQRS event store.
///
/// This is the decoded form of [`PackedCqrsEvent`]; `sequence` comes from the
/// storage key and is not serialised with the record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CqrsStoredEvent {
    pub sequence: u32,
    /// `None` for governance commands, which have no content-addressed ID.
    pub event_id: Option<BytesN<32>>,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    pub category: Option<Symbol>,
    pub sub_event_type: Option<Symbol>,
    pub version: u32,
    pub kind: CqrsCommandKind,
    /// Index positions for `Log` events, so TTL helpers can bump the
    /// matching materialized index entries.
    pub index_pos: CqrsIndexPos,
}

/// Packed on-ledger encoding of [`CqrsStoredEvent`] (positional `ScVec`).
///
/// Field order: `event_id, timestamp, event_type, submitter, metadata,
/// category, sub_event_type, version, kind, index_pos`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedCqrsEvent(
    pub Option<BytesN<32>>,
    pub u64,
    pub Symbol,
    pub Address,
    pub Bytes,
    pub Option<Symbol>,
    pub Option<Symbol>,
    pub u32,
    pub CqrsCommandKind,
    pub CqrsIndexPos,
);

impl CqrsStoredEvent {
    pub fn pack(&self) -> PackedCqrsEvent {
        PackedCqrsEvent(
            self.event_id.clone(),
            self.timestamp,
            self.event_type.clone(),
            self.submitter.clone(),
            self.metadata.clone(),
            self.category.clone(),
            self.sub_event_type.clone(),
            self.version,
            self.kind,
            self.index_pos,
        )
    }

    pub fn unpack(sequence: u32, p: PackedCqrsEvent) -> Self {
        CqrsStoredEvent {
            sequence,
            event_id: p.0,
            timestamp: p.1,
            event_type: p.2,
            submitter: p.3,
            metadata: p.4,
            category: p.5,
            sub_event_type: p.6,
            version: p.7,
            kind: p.8,
            index_pos: p.9,
        }
    }
}

// ── Query Side Models (Read Path) ───────────────────────────────────────────

/// Read-optimized projection of a single audit event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventView {
    pub event_id: BytesN<32>,
    pub sequence: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub category: Option<Symbol>,
    pub submitter: Address,
    pub metadata: Bytes,
    pub version: u32,
}

/// Filter criteria for querying the event store read model.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventQuery {
    pub event_type: Option<Symbol>,
    pub submitter: Option<Address>,
    pub from_sequence: u32,
    pub to_sequence: u32,
    pub limit: u32,
}

/// Materialized projection state aggregated from all sequenced events.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventProjection {
    pub last_sequence: u32,
    pub total_events: u32,
    pub total_commands: u32,
    pub type_counts: Vec<(Symbol, u32)>,
    pub submitter_counts: Vec<(Address, u32)>,
    pub last_updated: u64,
}

/// Packed on-ledger encoding of [`EventProjection`].
///
/// Field order: `last_sequence, total_events, total_commands, type_counts,
/// submitter_counts, last_updated`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedProjection(
    pub u32,
    pub u32,
    pub u32,
    pub Vec<(Symbol, u32)>,
    pub Vec<(Address, u32)>,
    pub u64,
);

impl EventProjection {
    pub fn pack(&self) -> PackedProjection {
        PackedProjection(
            self.last_sequence,
            self.total_events,
            self.total_commands,
            self.type_counts.clone(),
            self.submitter_counts.clone(),
            self.last_updated,
        )
    }

    pub fn unpack(p: PackedProjection) -> Self {
        EventProjection {
            last_sequence: p.0,
            total_events: p.1,
            total_commands: p.2,
            type_counts: p.3,
            submitter_counts: p.4,
            last_updated: p.5,
        }
    }
}

/// Point-in-time snapshot of the projection state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionSnapshot {
    pub snapshot_id: u32,
    pub sequence: u32,
    pub projection: EventProjection,
    pub timestamp: u64,
}

/// Packed on-ledger encoding of [`ProjectionSnapshot`]; the snapshot id is the
/// storage key and the sequence is `projection.last_sequence`.
///
/// Field order: `projection, timestamp`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedSnapshot(pub PackedProjection, pub u64);

// ── Storage Keys ────────────────────────────────────────────────────────────

/// CQRS storage keys. Variant names are kept short because the variant name
/// is serialised as a `Symbol` inside every ledger key.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CqrsStorageKey {
    /// [Persistent] Monotonic sequence -> `PackedCqrsEvent` (immutable event log).
    Event(u32),
    /// [Instance] Current highest sequence number (u32).
    Seq,
    /// [Persistent] Event ID -> sequence number (u32).
    IdSeq(BytesN<32>),
    /// [Instance] Active materialized projection state (`PackedProjection`).
    Proj,
    /// [Persistent] Snapshot ID -> `PackedSnapshot`.
    Snap(u32),
    /// [Instance] Total snapshots created; also the latest snapshot ID.
    SnapCnt,
    /// [Persistent] (event type, position) -> sequence number.
    TypeIdx(Symbol, u32),
    /// [Persistent] Event type -> number of entries in the type index.
    TypeCnt(Symbol),
    /// [Persistent] (submitter, position) -> sequence number.
    SubIdx(Address, u32),
    /// [Persistent] Submitter -> number of entries in the submitter index.
    SubCnt(Address),
    /// [Temporary] Idempotency key -> `CommandResult`.
    Idem(BytesN<32>),
}

impl CqrsStorageKey {
    /// Storage tier this key is classified into (see module docs).
    pub fn tier(&self) -> CqrsStorageTier {
        match self {
            CqrsStorageKey::Seq | CqrsStorageKey::Proj | CqrsStorageKey::SnapCnt => {
                CqrsStorageTier::Instance
            }
            CqrsStorageKey::Idem(_) => CqrsStorageTier::Temporary,
            CqrsStorageKey::Event(_)
            | CqrsStorageKey::IdSeq(_)
            | CqrsStorageKey::Snap(_)
            | CqrsStorageKey::TypeIdx(_, _)
            | CqrsStorageKey::TypeCnt(_)
            | CqrsStorageKey::SubIdx(_, _)
            | CqrsStorageKey::SubCnt(_) => CqrsStorageTier::Persistent,
        }
    }
}

// ── CQRS Engine Implementation ──────────────────────────────────────────────

pub struct CqrsEngine;

impl CqrsEngine {
    // ── Tiered storage primitives ───────────────────────────────────────────

    fn load<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(
        env: &Env,
        key: &CqrsStorageKey,
    ) -> Option<V> {
        match key.tier() {
            CqrsStorageTier::Instance => env.storage().instance().get(key),
            CqrsStorageTier::Persistent => env.storage().persistent().get(key),
            CqrsStorageTier::Temporary => env.storage().temporary().get(key),
        }
    }

    fn store<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
        env: &Env,
        key: &CqrsStorageKey,
        value: &V,
    ) {
        match key.tier() {
            CqrsStorageTier::Instance => env.storage().instance().set(key, value),
            CqrsStorageTier::Persistent => {
                env.storage().persistent().set(key, value);
                env.storage().persistent().extend_ttl(
                    key,
                    CQRS_PERSISTENT_TTL_THRESHOLD,
                    CQRS_PERSISTENT_TTL_EXTEND_TO,
                );
            }
            CqrsStorageTier::Temporary => {
                env.storage().temporary().set(key, value);
                env.storage().temporary().extend_ttl(
                    key,
                    CQRS_IDEMPOTENCY_TTL_LEDGERS,
                    CQRS_IDEMPOTENCY_TTL_LEDGERS,
                );
            }
        }
    }

    /// Extend the TTL of a persistent key if it exists. Returns `false` when
    /// the key is absent.
    fn bump_persistent(env: &Env, key: &CqrsStorageKey) -> bool {
        if env.storage().persistent().has(key) {
            env.storage().persistent().extend_ttl(
                key,
                CQRS_PERSISTENT_TTL_THRESHOLD,
                CQRS_PERSISTENT_TTL_EXTEND_TO,
            );
            true
        } else {
            false
        }
    }

    fn bump_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(CQRS_INSTANCE_TTL_THRESHOLD, CQRS_INSTANCE_TTL_EXTEND_TO);
    }

    fn checked_inc(v: u32) -> Result<u32, CqrsError> {
        v.checked_add(1).ok_or(CqrsError::CounterOverflow)
    }

    /// Allocate the next event sequence number.
    fn next_sequence(env: &Env) -> Result<u32, CqrsError> {
        Self::current_sequence(env)
            .checked_add(1)
            .ok_or(CqrsError::SequenceOverflow)
    }

    /// Validate a log command against the engine's hard bounds. Called by the
    /// contract entry point *before* any state is written.
    pub fn validate_log_command(env: &Env, cmd: &LogEventCommand) -> Result<(), CqrsError> {
        if cmd.metadata.len() > CQRS_MAX_METADATA_BYTES {
            return Err(CqrsError::MetadataTooLarge);
        }
        // Fail fast on sequence exhaustion before the primary ledger is touched.
        Self::next_sequence(env)?;
        Ok(())
    }

    /// Clamp a caller-supplied limit into `[1, CQRS_MAX_QUERY_LIMIT]`.
    pub fn effective_limit(limit: u32) -> u32 {
        if limit == 0 {
            CQRS_DEFAULT_QUERY_LIMIT
        } else {
            core::cmp::min(limit, CQRS_MAX_QUERY_LIMIT)
        }
    }

    /// Current highest sequence number in the event store (0 when empty).
    pub fn current_sequence(env: &Env) -> u32 {
        Self::load(env, &CqrsStorageKey::Seq).unwrap_or(0)
    }

    /// Cached `CommandResult` for an idempotency key, if still live.
    pub fn get_idempotent_result(env: &Env, key: BytesN<32>) -> Option<CommandResult> {
        Self::load(env, &CqrsStorageKey::Idem(key))
    }

    /// Load and decode the stored event at `sequence`.
    pub fn get_stored_event(env: &Env, sequence: u32) -> Option<CqrsStoredEvent> {
        let packed: PackedCqrsEvent = Self::load(env, &CqrsStorageKey::Event(sequence))?;
        Some(CqrsStoredEvent::unpack(sequence, packed))
    }

    // ── Permissionless TTL helpers ──────────────────────────────────────────

    /// Extend the TTL of an archival-critical event record together with its
    /// ID lookup and materialized index entries. Permissionless: anyone may pay
    /// to keep audit history live.
    pub fn extend_event_ttl(env: &Env, sequence: u32) -> Result<(), CqrsError> {
        let stored = Self::get_stored_event(env, sequence).ok_or(CqrsError::EventNotFound)?;
        Self::bump_persistent(env, &CqrsStorageKey::Event(sequence));
        if let Some(ref id) = stored.event_id {
            Self::bump_persistent(env, &CqrsStorageKey::IdSeq(id.clone()));
        }
        if let CqrsIndexPos::At(type_pos, sub_pos) = stored.index_pos {
            Self::bump_persistent(
                env,
                &CqrsStorageKey::TypeIdx(stored.event_type.clone(), type_pos),
            );
            Self::bump_persistent(env, &CqrsStorageKey::TypeCnt(stored.event_type));
            Self::bump_persistent(
                env,
                &CqrsStorageKey::SubIdx(stored.submitter.clone(), sub_pos),
            );
            Self::bump_persistent(env, &CqrsStorageKey::SubCnt(stored.submitter));
        }
        Self::bump_instance(env);
        Ok(())
    }

    /// Extend the TTL of a projection snapshot. Permissionless.
    pub fn extend_snapshot_ttl(env: &Env, snapshot_id: u32) -> Result<(), CqrsError> {
        if !Self::bump_persistent(env, &CqrsStorageKey::Snap(snapshot_id)) {
            return Err(CqrsError::SnapshotNotFound);
        }
        Self::bump_instance(env);
        Ok(())
    }

    // ── Command Side (Write Path) ───────────────────────────────────────────

    /// Record a successfully executed event in the append-only event store,
    /// allocate sequence number, update projections and materialized views.
    pub fn record_log_event(
        env: &Env,
        event_id: BytesN<32>,
        cmd: &LogEventCommand,
        timestamp: u64,
    ) -> Result<CommandResult, CqrsError> {
        Self::validate_log_command(env, cmd)?;
        let next_seq = Self::next_sequence(env)?;

        let mut stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: Some(event_id.clone()),
            timestamp,
            event_type: cmd.event_type.clone(),
            submitter: cmd.submitter.clone(),
            metadata: cmd.metadata.clone(),
            category: cmd.category.clone(),
            sub_event_type: cmd.sub_event_type.clone(),
            version: 1,
            kind: CqrsCommandKind::Log,
            index_pos: CqrsIndexPos::Unindexed,
        };

        // Update materialized index views (records positions on the event)
        let (type_pos, sub_pos) = Self::add_to_materialized_index(env, &stored, next_seq)?;
        stored.index_pos = CqrsIndexPos::At(type_pos, sub_pos);

        // Append to immutable log
        Self::store(env, &CqrsStorageKey::Event(next_seq), &stored.pack());
        Self::store(env, &CqrsStorageKey::Seq, &next_seq);
        Self::store(env, &CqrsStorageKey::IdSeq(event_id.clone()), &next_seq);

        // Update live projection
        let mut proj = Self::get_projection(env);
        proj.total_events = Self::checked_inc(proj.total_events)?;
        proj.total_commands = Self::checked_inc(proj.total_commands)?;
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        Self::increment_type_count(&mut proj.type_counts, cmd.event_type.clone())?;
        Self::increment_submitter_count(&mut proj.submitter_counts, cmd.submitter.clone())?;
        Self::store(env, &CqrsStorageKey::Proj, &proj.pack());

        let result = CommandResult {
            success: true,
            event_id: Some(event_id.clone()),
            sequence_number: next_seq,
            error_code: None,
            message: Some(Symbol::new(env, "logged")),
        };

        // Record idempotency key if supplied (temporary tier)
        if let Some(ref idemp) = cmd.idempotency_key {
            Self::store(env, &CqrsStorageKey::Idem(idemp.clone()), &result);
        }
        Self::bump_instance(env);

        // Publish CQRS event notification
        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "ev_log")),
            (event_id, next_seq),
        );

        Ok(result)
    }

    /// Record an event update in the append-only log and advance projection.
    pub fn record_update_event(
        env: &Env,
        new_event_id: BytesN<32>,
        caller: Address,
        original_event_id: BytesN<32>,
        event_type: Symbol,
        new_metadata: Bytes,
        new_version: u32,
        timestamp: u64,
    ) -> Result<CommandResult, CqrsError> {
        if new_metadata.len() > CQRS_MAX_METADATA_BYTES {
            return Err(CqrsError::MetadataTooLarge);
        }
        let next_seq = Self::next_sequence(env)?;

        let stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: Some(new_event_id.clone()),
            timestamp,
            event_type,
            submitter: caller,
            metadata: new_metadata,
            category: None,
            sub_event_type: None,
            version: new_version,
            kind: CqrsCommandKind::Update,
            index_pos: CqrsIndexPos::Unindexed,
        };

        // Append to immutable log
        Self::store(env, &CqrsStorageKey::Event(next_seq), &stored.pack());
        Self::store(env, &CqrsStorageKey::Seq, &next_seq);
        Self::store(env, &CqrsStorageKey::IdSeq(new_event_id.clone()), &next_seq);
        Self::store(env, &CqrsStorageKey::IdSeq(original_event_id), &next_seq);

        // Update live projection
        let mut proj = Self::get_projection(env);
        proj.total_commands = Self::checked_inc(proj.total_commands)?;
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        Self::store(env, &CqrsStorageKey::Proj, &proj.pack());
        Self::bump_instance(env);

        let result = CommandResult {
            success: true,
            event_id: Some(new_event_id.clone()),
            sequence_number: next_seq,
            error_code: None,
            message: Some(Symbol::new(env, "updated")),
        };

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "ev_upd")),
            (new_event_id, next_seq),
        );

        Ok(result)
    }

    /// Record a governance command execution in the event store.
    pub fn record_governance_command(
        env: &Env,
        caller: Address,
        action: Symbol,
        timestamp: u64,
    ) -> Result<CommandResult, CqrsError> {
        let next_seq = Self::next_sequence(env)?;

        let stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: None,
            timestamp,
            event_type: action.clone(),
            submitter: caller,
            metadata: Bytes::new(env),
            category: None,
            sub_event_type: None,
            version: 1,
            kind: CqrsCommandKind::Governance,
            index_pos: CqrsIndexPos::Unindexed,
        };

        Self::store(env, &CqrsStorageKey::Event(next_seq), &stored.pack());
        Self::store(env, &CqrsStorageKey::Seq, &next_seq);

        let mut proj = Self::get_projection(env);
        proj.total_commands = Self::checked_inc(proj.total_commands)?;
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        Self::store(env, &CqrsStorageKey::Proj, &proj.pack());
        Self::bump_instance(env);

        let result = CommandResult {
            success: true,
            event_id: None,
            sequence_number: next_seq,
            error_code: None,
            message: Some(Symbol::new(env, "gov_applied")),
        };

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "ev_gov")),
            (action, next_seq),
        );

        Ok(result)
    }

    // ── Query Side (Read Path) ──────────────────────────────────────────────

    /// Get read-optimized `EventView` by content-addressed event ID.
    pub fn get_event_view(env: &Env, event_id: BytesN<32>) -> Option<EventView> {
        let seq: u32 = Self::load(env, &CqrsStorageKey::IdSeq(event_id))?;
        Self::get_event_by_sequence(env, seq)
    }

    /// Get read-optimized `EventView` by monotonic sequence number.
    pub fn get_event_by_sequence(env: &Env, sequence: u32) -> Option<EventView> {
        let stored = Self::get_stored_event(env, sequence)?;
        Some(Self::to_view(env, stored))
    }

    fn to_view(env: &Env, stored: CqrsStoredEvent) -> EventView {
        let category = match stored.kind {
            CqrsCommandKind::Log => stored.category,
            other => Some(other.as_symbol(env)),
        };
        EventView {
            event_id: stored
                .event_id
                .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32])),
            sequence: stored.sequence,
            timestamp: stored.timestamp,
            event_type: stored.event_type,
            category,
            submitter: stored.submitter,
            metadata: stored.metadata,
            version: stored.version,
        }
    }

    /// Query the read model with filtering on type, submitter, and sequence range.
    pub fn query_events(env: &Env, query: EventQuery) -> Vec<EventView> {
        let total_seq = Self::current_sequence(env);
        let mut results = Vec::new(env);
        if total_seq == 0 {
            return results;
        }

        let eff_limit = Self::effective_limit(query.limit);
        let mut scanned: u32 = 0;

        let start_seq = if query.from_sequence == 0 {
            1
        } else {
            query.from_sequence
        };
        let end_seq = if query.to_sequence == 0 || query.to_sequence > total_seq {
            total_seq
        } else {
            query.to_sequence
        };

        if start_seq > end_seq {
            return results;
        }

        // Fast path 1: Materialized type index lookup
        if let (Some(q_type), None) = (&query.event_type, &query.submitter) {
            let count = Self::get_materialized_type_count(env, q_type.clone());
            for pos in 0..count {
                if scanned >= CQRS_MAX_QUERY_SCAN {
                    break;
                }
                scanned += 1;
                let seq: Option<u32> =
                    Self::load(env, &CqrsStorageKey::TypeIdx(q_type.clone(), pos));
                if let Some(seq) = seq {
                    if seq > end_seq {
                        break;
                    }
                    if seq >= start_seq {
                        if let Some(view) = Self::get_event_by_sequence(env, seq) {
                            results.push_back(view);
                            if results.len() >= eff_limit {
                                break;
                            }
                        }
                    }
                }
            }
            return results;
        }

        // Fast path 2: Materialized submitter index lookup
        if let (None, Some(q_sub)) = (&query.event_type, &query.submitter) {
            let count = Self::get_materialized_submitter_count(env, q_sub.clone());
            for pos in 0..count {
                if scanned >= CQRS_MAX_QUERY_SCAN {
                    break;
                }
                scanned += 1;
                let seq: Option<u32> =
                    Self::load(env, &CqrsStorageKey::SubIdx(q_sub.clone(), pos));
                if let Some(seq) = seq {
                    if seq > end_seq {
                        break;
                    }
                    if seq >= start_seq {
                        if let Some(view) = Self::get_event_by_sequence(env, seq) {
                            results.push_back(view);
                            if results.len() >= eff_limit {
                                break;
                            }
                        }
                    }
                }
            }
            return results;
        }

        // Sequential range scan fallback (bounded by CQRS_MAX_QUERY_SCAN)
        let scan_end = core::cmp::min(
            end_seq,
            start_seq.saturating_add(CQRS_MAX_QUERY_SCAN - 1),
        );
        for seq in start_seq..=scan_end {
            if let Some(view) = Self::get_event_by_sequence(env, seq) {
                let matches_type = match &query.event_type {
                    Some(t) => &view.event_type == t,
                    None => true,
                };
                let matches_sub = match &query.submitter {
                    Some(s) => &view.submitter == s,
                    None => true,
                };
                if matches_type && matches_sub {
                    results.push_back(view);
                    if results.len() >= eff_limit {
                        break;
                    }
                }
            }
        }

        results
    }

    /// Get current aggregated projection.
    pub fn get_projection(env: &Env) -> EventProjection {
        Self::load::<PackedProjection>(env, &CqrsStorageKey::Proj)
            .map(EventProjection::unpack)
            .unwrap_or_else(|| Self::empty_projection(env))
    }

    // ── Projection Rebuilding & Snapshotting ────────────────────────────────

    /// Rebuild projection state by replaying events from genesis (or from the latest snapshot).
    ///
    /// Materialized indexes are maintained at write time (and their positions
    /// are recorded on each event), so replay only recomputes the projection and
    /// never re-appends index entries.
    ///
    /// Returns [`CqrsError::ReplayWindowTooLarge`] if more than
    /// [`CQRS_MAX_REPLAY_EVENTS`] events would have to be replayed.
    pub fn rebuild_projection(env: &Env, from_snapshot: bool) -> Result<EventProjection, CqrsError> {
        let total_seq = Self::current_sequence(env);

        let latest_snapshot = if from_snapshot {
            Self::latest_snapshot_id(env).and_then(|id| Self::get_snapshot(env, id))
        } else {
            None
        };
        let (mut proj, start_seq) = match latest_snapshot {
            Some(snap) => (snap.projection, snap.sequence.saturating_add(1)),
            None => (Self::empty_projection(env), 1),
        };

        if start_seq <= total_seq {
            if total_seq - start_seq >= CQRS_MAX_REPLAY_EVENTS {
                return Err(CqrsError::ReplayWindowTooLarge);
            }
            for seq in start_seq..=total_seq {
                if let Some(stored) = Self::get_stored_event(env, seq) {
                    proj.total_commands = Self::checked_inc(proj.total_commands)?;
                    if stored.kind == CqrsCommandKind::Log {
                        proj.total_events = Self::checked_inc(proj.total_events)?;
                        Self::increment_type_count(
                            &mut proj.type_counts,
                            stored.event_type.clone(),
                        )?;
                        Self::increment_submitter_count(
                            &mut proj.submitter_counts,
                            stored.submitter.clone(),
                        )?;
                    }
                    proj.last_sequence = seq;
                    proj.last_updated = stored.timestamp;
                }
            }
        }

        Self::store(env, &CqrsStorageKey::Proj, &proj.pack());
        Self::bump_instance(env);

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "rebuilt")),
            (proj.last_sequence, proj.total_events),
        );

        Ok(proj)
    }

    /// Capture a snapshot of the current projection state.
    pub fn create_snapshot(env: &Env) -> Result<u32, CqrsError> {
        let count: u32 = Self::load(env, &CqrsStorageKey::SnapCnt).unwrap_or(0);
        let next_id = Self::checked_inc(count)?;
        let proj = Self::get_projection(env);
        let sequence = proj.last_sequence;

        Self::store(
            env,
            &CqrsStorageKey::Snap(next_id),
            &PackedSnapshot(proj.pack(), env.ledger().timestamp()),
        );
        Self::store(env, &CqrsStorageKey::SnapCnt, &next_id);
        Self::bump_instance(env);

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "snapshot")),
            (next_id, sequence),
        );

        Ok(next_id)
    }

    /// ID of the most recently created snapshot, if any.
    pub fn latest_snapshot_id(env: &Env) -> Option<u32> {
        match Self::load::<u32>(env, &CqrsStorageKey::SnapCnt) {
            Some(0) | None => None,
            Some(id) => Some(id),
        }
    }

    /// Retrieve a saved snapshot by ID.
    pub fn get_snapshot(env: &Env, snapshot_id: u32) -> Option<ProjectionSnapshot> {
        let packed: PackedSnapshot = Self::load(env, &CqrsStorageKey::Snap(snapshot_id))?;
        let projection = EventProjection::unpack(packed.0);
        Some(ProjectionSnapshot {
            snapshot_id,
            sequence: projection.last_sequence,
            projection,
            timestamp: packed.1,
        })
    }

    // ── Materialized Views ──────────────────────────────────────────────────

    /// Get total count of events for an event type from the materialized view.
    pub fn get_materialized_type_count(env: &Env, event_type: Symbol) -> u32 {
        Self::load(env, &CqrsStorageKey::TypeCnt(event_type)).unwrap_or(0)
    }

    /// Get total count of events for a submitter from the materialized view.
    pub fn get_materialized_submitter_count(env: &Env, submitter: Address) -> u32 {
        Self::load(env, &CqrsStorageKey::SubCnt(submitter)).unwrap_or(0)
    }

    /// Get paginated event IDs from the materialized type index.
    pub fn get_materialized_type_events(
        env: &Env,
        event_type: Symbol,
        start: u32,
        limit: u32,
    ) -> Vec<BytesN<32>> {
        let total = Self::get_materialized_type_count(env, event_type.clone());
        let mut out = Vec::new(env);
        if start >= total || limit == 0 {
            return out;
        }
        let limit = core::cmp::min(limit, CQRS_MAX_QUERY_LIMIT);
        let end = core::cmp::min(start.saturating_add(limit), total);
        for pos in start..end {
            let seq: Option<u32> =
                Self::load(env, &CqrsStorageKey::TypeIdx(event_type.clone(), pos));
            if let Some(id) = seq
                .and_then(|s| Self::get_stored_event(env, s))
                .and_then(|e| e.event_id)
            {
                out.push_back(id);
            }
        }
        out
    }

    /// Get paginated sequence numbers from the materialized submitter index.
    pub fn get_materialized_submitter_events(
        env: &Env,
        submitter: Address,
        start: u32,
        limit: u32,
    ) -> Vec<u32> {
        let total = Self::get_materialized_submitter_count(env, submitter.clone());
        let mut out = Vec::new(env);
        if start >= total || limit == 0 {
            return out;
        }
        let limit = core::cmp::min(limit, CQRS_MAX_QUERY_LIMIT);
        let end = core::cmp::min(start.saturating_add(limit), total);
        for pos in start..end {
            if let Some(seq) =
                Self::load::<u32>(env, &CqrsStorageKey::SubIdx(submitter.clone(), pos))
            {
                out.push_back(seq);
            }
        }
        out
    }

    // ── Internal Helpers ────────────────────────────────────────────────────

    fn empty_projection(env: &Env) -> EventProjection {
        EventProjection {
            last_sequence: 0,
            total_events: 0,
            total_commands: 0,
            type_counts: Vec::new(env),
            submitter_counts: Vec::new(env),
            last_updated: 0,
        }
    }

    /// Append `sequence` to the per-type and per-submitter indexes. Each index
    /// entry is its own fixed-size persistent ledger entry, so no growing
    /// vector is ever re-written. Returns `(type_pos, submitter_pos)`.
    fn add_to_materialized_index(
        env: &Env,
        event: &CqrsStoredEvent,
        sequence: u32,
    ) -> Result<(u32, u32), CqrsError> {
        let type_pos = Self::get_materialized_type_count(env, event.event_type.clone());
        Self::store(
            env,
            &CqrsStorageKey::TypeIdx(event.event_type.clone(), type_pos),
            &sequence,
        );
        Self::store(
            env,
            &CqrsStorageKey::TypeCnt(event.event_type.clone()),
            &Self::checked_inc(type_pos)?,
        );

        let sub_pos = Self::get_materialized_submitter_count(env, event.submitter.clone());
        Self::store(
            env,
            &CqrsStorageKey::SubIdx(event.submitter.clone(), sub_pos),
            &sequence,
        );
        Self::store(
            env,
            &CqrsStorageKey::SubCnt(event.submitter.clone()),
            &Self::checked_inc(sub_pos)?,
        );

        Ok((type_pos, sub_pos))
    }

    /// Increment `key` in a bounded summary vector. New keys are only added
    /// while the vector holds fewer than `cap` entries.
    fn increment_bounded<K>(list: &mut Vec<(K, u32)>, key: K, cap: u32) -> Result<(), CqrsError>
    where
        K: Clone
            + PartialEq
            + soroban_sdk::IntoVal<Env, soroban_sdk::Val>
            + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        for i in 0..list.len() {
            if let Some((k, count)) = list.get(i) {
                if k == key {
                    list.set(i, (k, Self::checked_inc(count)?));
                    return Ok(());
                }
            }
        }
        if list.len() < cap {
            list.push_back((key, 1));
        }
        Ok(())
    }

    fn increment_type_count(list: &mut Vec<(Symbol, u32)>, key: Symbol) -> Result<(), CqrsError> {
        Self::increment_bounded(list, key, CQRS_MAX_PROJECTION_TYPES)
    }

    fn increment_submitter_count(
        list: &mut Vec<(Address, u32)>,
        key: Address,
    ) -> Result<(), CqrsError> {
        Self::increment_bounded(list, key, CQRS_MAX_PROJECTION_SUBMITTERS)
    }
}
