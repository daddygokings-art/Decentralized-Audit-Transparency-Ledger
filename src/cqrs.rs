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
}

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
    ) -> CommandResult {
        let next_seq = Self::current_sequence(env).saturating_add(1);

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
        let (type_pos, sub_pos) = Self::add_to_materialized_index(env, &stored, next_seq);
        stored.index_pos = CqrsIndexPos::At(type_pos, sub_pos);

        // Append to immutable log
        Self::store(env, &CqrsStorageKey::Event(next_seq), &stored.pack());
        Self::store(env, &CqrsStorageKey::Seq, &next_seq);
        Self::store(env, &CqrsStorageKey::IdSeq(event_id.clone()), &next_seq);

        // Update live projection
        let mut proj = Self::get_projection(env);
        proj.total_events = proj.total_events.saturating_add(1);
        proj.total_commands = proj.total_commands.saturating_add(1);
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        Self::increment_type_count(env, &mut proj.type_counts, cmd.event_type.clone());
        Self::increment_submitter_count(
            env,
            &mut proj.submitter_counts,
            cmd.submitter.clone(),
        );
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

        result
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
    ) -> CommandResult {
        let next_seq = Self::current_sequence(env).saturating_add(1);

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
        proj.total_commands = proj.total_commands.saturating_add(1);
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

        result
    }

    /// Record a governance command execution in the event store.
    pub fn record_governance_command(
        env: &Env,
        caller: Address,
        action: Symbol,
        timestamp: u64,
    ) -> CommandResult {
        let next_seq = Self::current_sequence(env).saturating_add(1);

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
        proj.total_commands = proj.total_commands.saturating_add(1);
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

        result
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

        let eff_limit = if query.limit == 0 {
            20
        } else if query.limit > 50 {
            50
        } else {
            query.limit
        };

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

        // Sequential range scan fallback
        for seq in start_seq..=end_seq {
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
    pub fn rebuild_projection(env: &Env, from_snapshot: bool) -> EventProjection {
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
            for seq in start_seq..=total_seq {
                if let Some(stored) = Self::get_stored_event(env, seq) {
                    proj.total_commands = proj.total_commands.saturating_add(1);
                    if stored.kind == CqrsCommandKind::Log {
                        proj.total_events = proj.total_events.saturating_add(1);
                        Self::increment_type_count(
                            env,
                            &mut proj.type_counts,
                            stored.event_type.clone(),
                        );
                        Self::increment_submitter_count(
                            env,
                            &mut proj.submitter_counts,
                            stored.submitter.clone(),
                        );
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

        proj
    }

    /// Capture a snapshot of the current projection state.
    pub fn create_snapshot(env: &Env) -> u32 {
        let count: u32 = Self::load(env, &CqrsStorageKey::SnapCnt).unwrap_or(0);
        let next_id = count.saturating_add(1);
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

        next_id
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
    ) -> (u32, u32) {
        let type_pos = Self::get_materialized_type_count(env, event.event_type.clone());
        Self::store(
            env,
            &CqrsStorageKey::TypeIdx(event.event_type.clone(), type_pos),
            &sequence,
        );
        Self::store(
            env,
            &CqrsStorageKey::TypeCnt(event.event_type.clone()),
            &type_pos.saturating_add(1),
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
            &sub_pos.saturating_add(1),
        );

        (type_pos, sub_pos)
    }

    fn increment_type_count(env: &Env, list: &mut Vec<(Symbol, u32)>, key: Symbol) {
        let mut found = false;
        let mut new_list = Vec::new(env);
        for (k, count) in list.iter() {
            if k == key {
                new_list.push_back((k, count.saturating_add(1)));
                found = true;
            } else {
                new_list.push_back((k, count));
            }
        }
        if !found {
            new_list.push_back((key, 1));
        }
        *list = new_list;
    }

    fn increment_submitter_count(env: &Env, list: &mut Vec<(Address, u32)>, key: Address) {
        let mut found = false;
        let mut new_list = Vec::new(env);
        for (k, count) in list.iter() {
            if k == key {
                new_list.push_back((k, count.saturating_add(1)));
                found = true;
            } else {
                new_list.push_back((k, count));
            }
        }
        if !found {
            new_list.push_back((key, 1));
        }
        *list = new_list;
    }
}
