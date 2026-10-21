//! # Contract Event Sourcing with CQRS Pattern (Issues #412, #799, #800)
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
//! ## Authorization and audit trail
//!
//! Every state-mutating engine entrypoint authenticates the acting address and
//! checks the minimum RBAC role. Event submission requires `Submitter`; updates,
//! governance, rebuilding, and snapshot creation require `Admin`. Suspended,
//! unassigned, and under-privileged callers are rejected with stable
//! [`crate::rbac::RbacError`] codes. Successful mutations and role rejections
//! emit `cqrs_audit` events containing caller, ledger timestamp, outcome, error
//! code, and a SHA-256 state-diff digest.
//!
//! ## Deterministic bounds
//!
//! Stored event count is capped at [`MAX_CQRS_EVENTS`], metadata at
//! [`MAX_CQRS_METADATA_BYTES`] bytes per event, per-key indexes at
//! [`MAX_CQRS_INDEX_ENTRIES`], projection keys at
//! [`MAX_CQRS_PROJECTION_KEYS`], snapshots at [`MAX_CQRS_SNAPSHOTS`], and
//! query results at [`MAX_CQRS_QUERY_RESULTS`]. Thus a rebuild examines at
//! most `MAX_CQRS_EVENTS` event records and bounded projection keys; no
//! materialized index or result vector grows without a fixed limit. At the
//! configured maxima, raw event metadata is bounded by 256 KiB; this excludes
//! Soroban key/value encoding overhead. Each event contributes at most 32
//! bytes to its type index and 4 bytes to its submitter index, before key/value
//! encoding. A projection stores at most 32 type-count and 32 submitter-count
//! entries; at most 16 snapshots can each copy those bounded vectors.
//!
//! Worst-case operation counts are bounded as follows: query fallback scans at
//! most 256 event sequences and returns at most 50 views; materialized-index
//! queries inspect at most 256 index entries and return at most 50; projection
//! rebuild replays at most 256 events and scans no more than 32 entries in
//! either aggregate-count vector per logged event. Exact Soroban instruction
//! and encoded storage costs depend on SDK/protocol versions and must be
//! measured with network-target benchmarks before deployment; this module
//! does not claim a measured gas figure.
//!
//! ## Regulator compliance mapping
//!
//! | Requirement | CQRS control |
//! |---|---|
//! | SOX §404 / SOC 2 CC6.1 | Role-gated writes and administrative projections |
//! | SOC 2 CC7.2 / ISO 27001 A.8.15 | Append-only event records and digest audit events |
//! | MiCA Art. 66 | Timestamped, queryable records and projection snapshots |

use soroban_sdk::{
    contracterror, contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Vec,
};

use crate::rbac::{RbacError, RbacManager, Role};

/// Maximum metadata payload retained by a CQRS event.
pub const MAX_CQRS_METADATA_BYTES: u32 = 1024;
/// Maximum number of CQRS events replayed or stored.
pub const MAX_CQRS_EVENTS: u32 = 256;
/// Maximum entries in a per-type or per-submitter materialized index.
pub const MAX_CQRS_INDEX_ENTRIES: u32 = 256;
/// Maximum distinct keys tracked by each projection count vector.
pub const MAX_CQRS_PROJECTION_KEYS: u32 = 32;
/// Maximum number of projection snapshots.
pub const MAX_CQRS_SNAPSHOTS: u32 = 16;
/// Maximum results returned by a CQRS query.
pub const MAX_CQRS_QUERY_RESULTS: u32 = 50;

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
    SequenceOverflow = 609,
    CollectionLimitExceeded = 610,
    SnapshotLimitExceeded = 612,
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

/// Immutable record appended to the CQRS event store.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CqrsStoredEvent {
    pub sequence: u32,
    pub event_id: BytesN<32>,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    pub category: Option<Symbol>,
    pub sub_event_type: Option<Symbol>,
    pub version: u32,
    pub command_type: Symbol,
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

/// Point-in-time snapshot of the projection state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionSnapshot {
    pub snapshot_id: u32,
    pub sequence: u32,
    pub projection: EventProjection,
    pub timestamp: u64,
}

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CqrsStorageKey {
    /// Monotonic sequence -> CqrsStoredEvent (immutable event log).
    EventStore(u32),
    /// Current highest sequence number (u32).
    EventSequence,
    /// Event ID -> sequence number (u32).
    EventIdToSequence(BytesN<32>),
    /// Active materialized projection state.
    Projection,
    /// Snapshot ID -> ProjectionSnapshot.
    Snapshot(u32),
    /// Total snapshots created.
    SnapshotCount,
    /// Most recently created snapshot ID.
    LatestSnapshotId,
    /// Event type -> list of event IDs (materialized type view).
    MaterializedTypeIndex(Symbol),
    /// Event type -> cached event count.
    MaterializedTypeCount(Symbol),
    /// Submitter address -> list of sequence numbers.
    MaterializedSubmitterIndex(Address),
    /// Submitter address -> cached event count.
    MaterializedSubmitterCount(Address),
    /// Idempotency key -> CommandResult.
    Idempotency(BytesN<32>),
}

// ── CQRS Engine Implementation ──────────────────────────────────────────────

pub struct CqrsEngine;

impl CqrsEngine {
    // ── Command Side (Write Path) ───────────────────────────────────────────

    /// Record a successfully executed event in the append-only event store,
    /// allocate sequence number, update projections and materialized views.
    pub fn record_log_event(
        env: &Env,
        event_id: BytesN<32>,
        cmd: &LogEventCommand,
        timestamp: u64,
        caller: &Address,
    ) -> CommandResult {
        if let Err(error) = Self::check_role(env, caller, Role::Submitter, "log") {
            return Self::denied_result(env, caller, "log", error);
        }
        if !Self::metadata_size_is_valid(cmd.metadata.len()) {
            panic_with_error!(env, CqrsError::MetadataTooLarge);
        }

        let current_seq: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::EventSequence)
            .unwrap_or(0);
        let next_seq = Self::checked_next_sequence(current_seq)
            .unwrap_or_else(|| panic_with_error!(env, CqrsError::SequenceOverflow));
        Self::ensure_index_capacity(env, &cmd.event_type, &cmd.submitter);

        let stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: event_id.clone(),
            timestamp,
            event_type: cmd.event_type.clone(),
            submitter: cmd.submitter.clone(),
            metadata: cmd.metadata.clone(),
            category: cmd.category.clone(),
            sub_event_type: cmd.sub_event_type.clone(),
            version: 1,
            command_type: Symbol::new(env, "log"),
        };

        // Append to immutable log
        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventStore(next_seq), &stored);
        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventSequence, &next_seq);
        env.storage().instance().set(
            &CqrsStorageKey::EventIdToSequence(event_id.clone()),
            &next_seq,
        );

        // Update materialized index views
        Self::add_to_materialized_index(env, &stored, next_seq);

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
        env.storage()
            .instance()
            .set(&CqrsStorageKey::Projection, &proj);

        Self::emit_audit_event(env, caller, "log", current_seq as i128, next_seq as i128, "ok", 0);

        let result = CommandResult {
            success: true,
            event_id: Some(event_id.clone()),
            sequence_number: next_seq,
            error_code: None,
            message: Some(Symbol::new(env, "logged")),
        };

        // Record idempotency key if supplied
        if let Some(ref idemp) = cmd.idempotency_key {
            env.storage()
                .instance()
                .set(&CqrsStorageKey::Idempotency(idemp.clone()), &result);
        }

        // Publish CQRS event notification
        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "ev_log")),
            (event_id, next_seq),
        );

        result
    }

    pub(crate) fn checked_next_sequence(current: u32) -> Option<u32> {
        current
            .checked_add(1)
            .filter(|sequence| *sequence <= MAX_CQRS_EVENTS)
    }

    pub(crate) fn metadata_size_is_valid(length: u32) -> bool {
        length <= MAX_CQRS_METADATA_BYTES
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
        if let Err(error) = Self::check_role(env, &caller, Role::Admin, "update") {
            return Self::denied_result(env, &caller, "update", error);
        }
        if !Self::metadata_size_is_valid(new_metadata.len()) {
            panic_with_error!(env, CqrsError::MetadataTooLarge);
        }
        let current_seq: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::EventSequence)
            .unwrap_or(0);
        let next_seq = Self::checked_next_sequence(current_seq)
            .unwrap_or_else(|| panic_with_error!(env, CqrsError::SequenceOverflow));

        let stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: new_event_id.clone(),
            timestamp,
            event_type: event_type.clone(),
            submitter: caller,
            metadata: new_metadata,
            category: Some(Symbol::new(env, "update")),
            sub_event_type: None,
            version: new_version,
            command_type: Symbol::new(env, "update"),
        };

        // Append to immutable log
        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventStore(next_seq), &stored);
        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventSequence, &next_seq);
        env.storage().instance().set(
            &CqrsStorageKey::EventIdToSequence(new_event_id.clone()),
            &next_seq,
        );
        env.storage().instance().set(
            &CqrsStorageKey::EventIdToSequence(original_event_id),
            &next_seq,
        );

        // Update live projection
        let mut proj = Self::get_projection(env);
        proj.total_commands = proj.total_commands.saturating_add(1);
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        env.storage()
            .instance()
            .set(&CqrsStorageKey::Projection, &proj);

        Self::emit_audit_event(env, &stored.submitter, "update", current_seq as i128, next_seq as i128, "ok", 0);

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
        if let Err(error) = Self::check_role(env, &caller, Role::Admin, "governance") {
            return Self::denied_result(env, &caller, "governance", error);
        }
        let current_seq: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::EventSequence)
            .unwrap_or(0);
        let next_seq = Self::checked_next_sequence(current_seq)
            .unwrap_or_else(|| panic_with_error!(env, CqrsError::SequenceOverflow));

        let stored = CqrsStoredEvent {
            sequence: next_seq,
            event_id: BytesN::from_array(env, &[0u8; 32]),
            timestamp,
            event_type: action.clone(),
            submitter: caller,
            metadata: Bytes::new(env),
            category: Some(Symbol::new(env, "governance")),
            sub_event_type: None,
            version: 1,
            command_type: Symbol::new(env, "governance"),
        };

        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventStore(next_seq), &stored);
        env.storage()
            .instance()
            .set(&CqrsStorageKey::EventSequence, &next_seq);

        let mut proj = Self::get_projection(env);
        proj.total_commands = proj.total_commands.saturating_add(1);
        proj.last_sequence = next_seq;
        proj.last_updated = timestamp;
        env.storage()
            .instance()
            .set(&CqrsStorageKey::Projection, &proj);

        Self::emit_audit_event(env, &stored.submitter, "governance", current_seq as i128, next_seq as i128, "ok", 0);

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
        let seq = env
            .storage()
            .instance()
            .get::<_, u32>(&CqrsStorageKey::EventIdToSequence(event_id))?;
        Self::get_event_by_sequence(env, seq)
    }

    /// Get read-optimized `EventView` by monotonic sequence number.
    pub fn get_event_by_sequence(env: &Env, sequence: u32) -> Option<EventView> {
        let stored = env
            .storage()
            .instance()
            .get::<_, CqrsStoredEvent>(&CqrsStorageKey::EventStore(sequence))?;
        Some(EventView {
            event_id: stored.event_id,
            sequence: stored.sequence,
            timestamp: stored.timestamp,
            event_type: stored.event_type,
            category: stored.category,
            submitter: stored.submitter,
            metadata: stored.metadata,
            version: stored.version,
        })
    }

    /// Query the read model with filtering on type, submitter, and sequence range.
    pub fn query_events(env: &Env, query: EventQuery) -> Vec<EventView> {
        let total_seq: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::EventSequence)
            .unwrap_or(0);
        if total_seq > MAX_CQRS_EVENTS {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
        let mut results = Vec::new(env);
        if total_seq == 0 {
            return results;
        }

        let eff_limit = if query.limit == 0 {
            20
        } else if query.limit > MAX_CQRS_QUERY_RESULTS {
            MAX_CQRS_QUERY_RESULTS
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
        if let Some(ref q_type) = query.event_type {
            if query.submitter.is_none() {
                if let Some(type_event_ids) = env
                    .storage()
                    .instance()
                    .get::<_, Vec<BytesN<32>>>(&CqrsStorageKey::MaterializedTypeIndex(q_type.clone()))
                {
                    if type_event_ids.len() > MAX_CQRS_INDEX_ENTRIES {
                        panic_with_error!(env, CqrsError::CollectionLimitExceeded);
                    }
                    for i in 0..type_event_ids.len() {
                        let id = type_event_ids.get(i).unwrap();
                        if let Some(seq) = env
                            .storage()
                            .instance()
                            .get::<_, u32>(&CqrsStorageKey::EventIdToSequence(id))
                        {
                            if seq >= start_seq && seq <= end_seq {
                                if let Some(view) = Self::get_event_by_sequence(env, seq) {
                                    results.push_back(view);
                                    if results.len() >= eff_limit {
                                        return results;
                                    }
                                }
                            }
                        }
                    }
                    return results;
                }
            }
        }

        // Fast path 2: Materialized submitter index lookup
        if let Some(ref q_sub) = query.submitter {
            if query.event_type.is_none() {
                if let Some(sub_seqs) = env
                    .storage()
                    .instance()
                    .get::<_, Vec<u32>>(&CqrsStorageKey::MaterializedSubmitterIndex(q_sub.clone()))
                {
                    if sub_seqs.len() > MAX_CQRS_INDEX_ENTRIES {
                        panic_with_error!(env, CqrsError::CollectionLimitExceeded);
                    }
                    for i in 0..sub_seqs.len() {
                        let seq = sub_seqs.get(i).unwrap();
                        if seq >= start_seq && seq <= end_seq {
                            if let Some(view) = Self::get_event_by_sequence(env, seq) {
                                results.push_back(view);
                                if results.len() >= eff_limit {
                                    return results;
                                }
                            }
                        }
                    }
                    return results;
                }
            }
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
        env.storage()
            .instance()
            .get(&CqrsStorageKey::Projection)
            .unwrap_or_else(|| Self::empty_projection(env))
    }

    // ── Projection Rebuilding & Snapshotting ────────────────────────────────

    /// Rebuild projection state by replaying events from genesis (or from the latest snapshot).
    pub fn rebuild_projection(env: &Env, from_snapshot: bool, caller: &Address) -> EventProjection {
        Self::require_role_or_panic(env, caller, Role::Admin, "rebuild");
        let total_seq: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::EventSequence)
            .unwrap_or(0);
        if total_seq > MAX_CQRS_EVENTS {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }

        let (mut proj, start_seq) = if from_snapshot {
            if let Some(latest_id) = env
                .storage()
                .instance()
                .get::<_, u32>(&CqrsStorageKey::LatestSnapshotId)
            {
                if let Some(snap) = env
                    .storage()
                    .instance()
                    .get::<_, ProjectionSnapshot>(&CqrsStorageKey::Snapshot(latest_id))
                {
                    if snap.projection.type_counts.len() > MAX_CQRS_PROJECTION_KEYS
                        || snap.projection.submitter_counts.len() > MAX_CQRS_PROJECTION_KEYS
                        || snap.sequence > MAX_CQRS_EVENTS
                    {
                        panic_with_error!(env, CqrsError::CollectionLimitExceeded);
                    }
                    (snap.projection, snap.sequence.saturating_add(1))
                } else {
                    (Self::empty_projection(env), 1)
                }
            } else {
                (Self::empty_projection(env), 1)
            }
        } else {
            (Self::empty_projection(env), 1)
        };

        if start_seq <= total_seq {
            for seq in start_seq..=total_seq {
                if let Some(stored) = env
                    .storage()
                    .instance()
                    .get::<_, CqrsStoredEvent>(&CqrsStorageKey::EventStore(seq))
                {
                    proj.total_commands = proj.total_commands.saturating_add(1);
                    if stored.command_type == Symbol::new(env, "log") {
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
                        if !from_snapshot {
                            Self::add_to_materialized_index(env, &stored, seq);
                        }
                    }
                    proj.last_sequence = seq;
                    proj.last_updated = stored.timestamp;
                }
            }
        }

        env.storage()
            .instance()
            .set(&CqrsStorageKey::Projection, &proj);

        Self::emit_audit_event(
            env,
            caller,
            "rebuild",
            proj.last_sequence as i128,
            total_seq as i128,
            "ok",
            0,
        );

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "rebuilt")),
            (proj.last_sequence, proj.total_events),
        );

        proj
    }

    /// Capture a snapshot of the current projection state.
    pub fn create_snapshot(env: &Env, caller: &Address) -> u32 {
        Self::require_role_or_panic(env, caller, Role::Admin, "snapshot");
        let count: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::SnapshotCount)
            .unwrap_or(0);
        if count >= MAX_CQRS_SNAPSHOTS {
            panic_with_error!(env, CqrsError::SnapshotLimitExceeded);
        }
        let next_id = count
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(env, CqrsError::SequenceOverflow));
        let proj = Self::get_projection(env);

        let snapshot = ProjectionSnapshot {
            snapshot_id: next_id,
            sequence: proj.last_sequence,
            projection: proj,
            timestamp: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&CqrsStorageKey::Snapshot(next_id), &snapshot);
        env.storage()
            .instance()
            .set(&CqrsStorageKey::SnapshotCount, &next_id);
        env.storage()
            .instance()
            .set(&CqrsStorageKey::LatestSnapshotId, &next_id);

        env.events().publish(
            (Symbol::new(env, "cqrs"), Symbol::new(env, "snapshot")),
            (next_id, snapshot.sequence),
        );

        Self::emit_audit_event(
            env,
            caller,
            "snapshot",
            count as i128,
            next_id as i128,
            "ok",
            0,
        );

        next_id
    }

    /// Retrieve a saved snapshot by ID.
    pub fn get_snapshot(env: &Env, snapshot_id: u32) -> Option<ProjectionSnapshot> {
        env.storage()
            .instance()
            .get(&CqrsStorageKey::Snapshot(snapshot_id))
    }

    // ── Materialized Views ──────────────────────────────────────────────────

    /// Get total count of events for an event type from the materialized view.
    pub fn get_materialized_type_count(env: &Env, event_type: Symbol) -> u32 {
        env.storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedTypeCount(event_type))
            .unwrap_or(0)
    }

    /// Get paginated event IDs from the materialized type index.
    pub fn get_materialized_type_events(
        env: &Env,
        event_type: Symbol,
        start: u32,
        limit: u32,
    ) -> Vec<BytesN<32>> {
        let all: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedTypeIndex(event_type))
            .unwrap_or_else(|| Vec::new(env));
        if all.len() > MAX_CQRS_INDEX_ENTRIES {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
        let total = all.len();
        let mut out = Vec::new(env);
        if start >= total || limit == 0 {
            return out;
        }
        let bounded_limit = core::cmp::min(limit, MAX_CQRS_QUERY_RESULTS);
        let end = core::cmp::min(start.saturating_add(bounded_limit), total);
        for i in start..end {
            out.push_back(all.get(i).unwrap());
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
        let all: Vec<u32> = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedSubmitterIndex(submitter))
            .unwrap_or_else(|| Vec::new(env));
        if all.len() > MAX_CQRS_INDEX_ENTRIES {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
        let total = all.len();
        let mut out = Vec::new(env);
        if start >= total || limit == 0 {
            return out;
        }
        let bounded_limit = core::cmp::min(limit, MAX_CQRS_QUERY_RESULTS);
        let end = core::cmp::min(start.saturating_add(bounded_limit), total);
        for i in start..end {
            out.push_back(all.get(i).unwrap());
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

    fn add_to_materialized_index(env: &Env, event: &CqrsStoredEvent, sequence: u32) {
        // Materialized type index
        let mut type_indices: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedTypeIndex(event.event_type.clone()))
            .unwrap_or_else(|| Vec::new(env));
        if type_indices.len() >= MAX_CQRS_INDEX_ENTRIES {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
        type_indices.push_back(event.event_id.clone());
        env.storage().instance().set(
            &CqrsStorageKey::MaterializedTypeIndex(event.event_type.clone()),
            &type_indices,
        );

        // Materialized type count
        let count: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedTypeCount(event.event_type.clone()))
            .unwrap_or(0);
        env.storage().instance().set(
            &CqrsStorageKey::MaterializedTypeCount(event.event_type.clone()),
            &count.saturating_add(1),
        );

        // Materialized submitter index
        let mut sub_indices: Vec<u32> = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedSubmitterIndex(event.submitter.clone()))
            .unwrap_or_else(|| Vec::new(env));
        if sub_indices.len() >= MAX_CQRS_INDEX_ENTRIES {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
        sub_indices.push_back(sequence);
        env.storage().instance().set(
            &CqrsStorageKey::MaterializedSubmitterIndex(event.submitter.clone()),
            &sub_indices,
        );

        // Materialized submitter count
        let sub_count: u32 = env
            .storage()
            .instance()
            .get(&CqrsStorageKey::MaterializedSubmitterCount(event.submitter.clone()))
            .unwrap_or(0);
        env.storage().instance().set(
            &CqrsStorageKey::MaterializedSubmitterCount(event.submitter.clone()),
            &sub_count.saturating_add(1),
        );
    }

    fn increment_type_count(env: &Env, list: &mut Vec<(Symbol, u32)>, key: Symbol) {
        let mut found = false;
        let mut new_list = Vec::new(env);
        for i in 0..list.len() {
            let (k, count) = list.get(i).unwrap();
            if k == key {
                new_list.push_back((k, count.saturating_add(1)));
                found = true;
            } else {
                new_list.push_back((k, count));
            }
        }
        if !found {
            if list.len() >= MAX_CQRS_PROJECTION_KEYS {
                panic_with_error!(env, CqrsError::CollectionLimitExceeded);
            }
            new_list.push_back((key, 1));
        }
        *list = new_list;
    }

    fn increment_submitter_count(env: &Env, list: &mut Vec<(Address, u32)>, key: Address) {
        let mut found = false;
        let mut new_list = Vec::new(env);
        for i in 0..list.len() {
            let (k, count) = list.get(i).unwrap();
            if k == key {
                new_list.push_back((k, count.saturating_add(1)));
                found = true;
            } else {
                new_list.push_back((k, count));
            }
        }
        if !found {
            if list.len() >= MAX_CQRS_PROJECTION_KEYS {
                panic_with_error!(env, CqrsError::CollectionLimitExceeded);
            }
            new_list.push_back((key, 1));
        }
        *list = new_list;
    }

    fn ensure_index_capacity(env: &Env, event_type: &Symbol, submitter: &Address) {
        let type_len = env
            .storage()
            .instance()
            .get::<_, Vec<BytesN<32>>>(&CqrsStorageKey::MaterializedTypeIndex(event_type.clone()))
            .map(|index| index.len())
            .unwrap_or(0);
        let submitter_len = env
            .storage()
            .instance()
            .get::<_, Vec<u32>>(&CqrsStorageKey::MaterializedSubmitterIndex(submitter.clone()))
            .map(|index| index.len())
            .unwrap_or(0);
        if type_len >= MAX_CQRS_INDEX_ENTRIES || submitter_len >= MAX_CQRS_INDEX_ENTRIES {
            panic_with_error!(env, CqrsError::CollectionLimitExceeded);
        }
    }

    fn check_role(
        env: &Env,
        caller: &Address,
        role: Role,
        operation: &str,
    ) -> Result<(), RbacError> {
        caller.require_auth();
        let result = if RbacManager::is_suspended(env, caller) {
            Err(RbacError::SubmitterSuspended)
        } else if !RbacManager::has_role_min(env, caller, role) {
            if RbacManager::get_role(env, caller).is_none() {
                Err(RbacError::UnauthorizedSubmitter)
            } else {
                Err(RbacError::InsufficientRole)
            }
        } else {
            Ok(())
        };
        if let Err(error) = result {
            Self::emit_audit_event(env, caller, operation, 0, 0, "denied", error as u32);
        }
        result
    }

    /// Check role and authentication before a contract wrapper performs any
    /// primary-ledger mutation associated with a CQRS command.
    pub fn authorize_mutation(
        env: &Env,
        caller: &Address,
        role: Role,
        operation: &str,
    ) -> Result<(), RbacError> {
        Self::check_role(env, caller, role, operation)
    }

    /// Build a stable failed command result after an authorization rejection.
    pub fn authorization_denied_result(
        env: &Env,
        caller: &Address,
        operation: &str,
        error: RbacError,
    ) -> CommandResult {
        Self::denied_result(env, caller, operation, error)
    }

    fn require_role_or_panic(env: &Env, caller: &Address, role: Role, operation: &str) {
        if let Err(error) = Self::check_role(env, caller, role, operation) {
            panic_with_error!(env, error);
        }
    }

    fn denied_result(
        env: &Env,
        _caller: &Address,
        _operation: &str,
        error: RbacError,
    ) -> CommandResult {
        CommandResult {
            success: false,
            event_id: None,
            sequence_number: env
                .storage()
                .instance()
                .get(&CqrsStorageKey::EventSequence)
                .unwrap_or(0),
            error_code: Some(error as u32),
            message: Some(Symbol::new(env, "rbac_denied")),
        }
    }

    fn emit_audit_event(
        env: &Env,
        caller: &Address,
        operation: &str,
        previous: i128,
        current: i128,
        outcome: &str,
        error_code: u32,
    ) {
        let operation = Symbol::new(env, operation);
        let outcome = Symbol::new(env, outcome);
        let digest = RbacManager::state_diff_digest(env, caller, &operation, previous, current);
        let timestamp = env.ledger().timestamp();
        env.events().publish(
            (Symbol::new(env, "cqrs_audit"), operation.clone()),
            (caller.clone(), timestamp, digest.clone(), outcome.clone(), error_code),
        );
        soroban_sdk::log!(
            env,
            "cqrs_audit",
            caller,
            timestamp,
            operation,
            digest,
            outcome,
            error_code
        );
    }
}
