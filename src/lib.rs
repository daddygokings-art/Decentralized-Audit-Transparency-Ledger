#![no_std]

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Default maximum metadata size (1 KB). Used when no explicit cap is set.
const DEFAULT_MAX_METADATA_SIZE: u32 = 1024;

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
    /// SHA-256 of this event (computed over the other fields + prev_hash).
    pub event_hash: BytesN<32>,
    /// SHA-256 of the previous event; `[0u8;32]` for the genesis event.
    pub prev_hash: BytesN<32>,
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

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Owner,
    /// Replaced by Config — kept as tombstone variant so existing encoded keys
    /// don't collide; no longer written.
    GlobalMaxLogs,
    /// Paused flag: when true, write operations are blocked.
    Paused,
    /// Replaced by Config — kept as tombstone variant.
    TotalEvents,
    /// Replaced by EventCapConfig(Symbol) — kept as tombstone variant.
    EventCapSet(Symbol),
    /// Replaced by EventCapConfig(Symbol) — kept as tombstone variant.
    EventMaxLogs(Symbol),
    /// Combined config (global_max_logs + total_events).
    Config,
    /// Replaces EventCapSet + EventMaxLogs: None = no cap, Some(n) = capped at n.
    EventCapConfig(Symbol),
    /// Stores packed Bytes of u32 global-order indices (4 bytes each, LE) for a type (issue #54).
    EventTypeIndices(Symbol),
    /// Primary storage: event ID → Event.
    EventData(BytesN<32>),
    /// Sequential index → event ID, for ordered retrieval.
    EventOrder(u32),
    /// Per-event-type metadata size cap (issue #67). Absent = use global default.
    EventMetadataMaxSize(Symbol),
    /// Global metadata size cap (issue #67). Absent = DEFAULT_MAX_METADATA_SIZE.
    GlobalMetadataMaxSize,
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
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    CallerNotOwner = 1,
    GlobalMaxLogsReached = 2,
    EventTypeMaxLogsReached = 3,
    EventDoesNotExist = 4,
    EventTypeIndexOutOfBounds = 5,
    NewOwnerIsZero = 6,
    CapNotSet = 7,
    MetadataTooLarge = 8,
    InvalidSignature = 9,
    ContractPaused = 10,
    RateLimitExceeded = 11,
    InvalidPaginationParams = 12,
}

const NULL_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

#[contract]
pub struct AuditLedger;

#[contractimpl]
impl AuditLedger {
    pub fn initialize(env: Env, owner: Address, global_max_logs: u32) {
        owner.require_auth();
        // Support both single-owner (legacy) and multi-owner setups.
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage().instance().set(
            &DataKey::Config,
            &Config { global_max_logs, total_events: 0 },
        );
        // start unpaused
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Log a batch of events atomically and return their sequential indices.
    pub fn log_events(env: Env, events: Vec<(Address, Symbol, Bytes)>) -> Vec<u32> {
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }

        let global_max: u32 = env
            .storage()
            .instance()
            .get(&DataKey::GlobalMaxLogs)
            .unwrap();
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap();
        let batch_len: u32 = events.len();

        if total.checked_add(batch_len).is_none() || total + batch_len > global_max {
            panic_with_error!(&env, ContractError::GlobalMaxLogsReached);
        }

        let now = env.ledger().timestamp();
        let mut submitter_batch_counts: Vec<(Address, u32)> = Vec::new(&env);
        let mut type_batch_counts: Vec<(Symbol, u32)> = Vec::new(&env);

        for i in 0..batch_len {
            let (submitter, event_type, metadata) = events.get(i).unwrap().clone();
            submitter.require_auth();

            let max_meta = Self::effective_metadata_max_size(&env, &event_type);
            if metadata.len() > max_meta {
                panic_with_error!(&env, ContractError::MetadataTooLarge);
            }

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
                let batch_count = Self::increment_address_count(
                    &env,
                    &mut submitter_batch_counts,
                    submitter.clone(),
                );
                if now == last_ts {
                    if count + batch_count > limit {
                        panic_with_error!(&env, ContractError::RateLimitExceeded);
                    }
                } else if batch_count > limit {
                    panic_with_error!(&env, ContractError::RateLimitExceeded);
                }
            }

            if env
                .storage()
                .instance()
                .has(&DataKey::EventCapSet(event_type.clone()))
            {
                let cap: u32 = env
                    .storage()
                    .instance()
                    .get(&DataKey::EventMaxLogs(event_type.clone()))
                    .unwrap();
                let current_count = Self::event_type_count(&env, event_type.clone());
                let batch_count = Self::increment_symbol_count(
                    &env,
                    &mut type_batch_counts,
                    event_type.clone(),
                );
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
            let prev_evt: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(prev_id))
                .unwrap();
            prev_evt.event_hash
        };

        for i in 0..batch_len {
            let (submitter, event_type, metadata) = events.get(i).unwrap().clone();
            let index = current_total;
            let timestamp = env.ledger().timestamp();
            let event_id = Self::compute_event_id(
                &env,
                &submitter,
                &event_type,
                &metadata,
                timestamp,
                index,
            );
            let event_hash = Self::compute_event_hash(&env, &event_id, &prev_hash, index, timestamp);

            let evt = Event {
                index,
                timestamp,
                event_type: event_type.clone(),
                submitter: submitter.clone(),
                metadata: metadata.clone(),
                event_hash: event_hash.clone(),
                prev_hash: prev_hash.clone(),
            };

            env.storage()
                .instance()
                .set(&DataKey::EventData(event_id.clone()), &evt);
            env.storage()
                .instance()
                .set(&DataKey::EventOrder(index), &event_id);

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
                .set(&DataKey::EventMeta(event_id.clone()), &evt);
            env.storage()
                .instance()
                .set(&DataKey::EventMetadata(event_id.clone()), &metadata);

            if !Self::effective_low_cost_mode(&env) {
                Self::push_type_index(&env, event_type.clone(), index);
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

            let emission_mode = Self::effective_event_emission_mode(&env);
            match emission_mode {
                1 => {
                    env.events().publish(
                        (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                        (index,),
                    );
                }
                2 => {
                    let metadata_hash: BytesN<32> = env.crypto().sha256(&metadata).into();
                    env.events().publish(
                        (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                        (index, metadata_hash),
                    );
                }
                3 => {}
                _ => {
                    env.events().publish(
                        (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                        (index, timestamp, metadata.clone()),
                    );
                }
            }

            result_indices.push_back(index);
            prev_hash = event_hash;
            current_total += 1;
        }

        env.storage()
            .instance()
            .set(&DataKey::TotalEvents, &current_total);

        result_indices
    }

    /// Log an event and return its content-addressed `BytesN<32>` ID.
    #[allow(deprecated)]
    pub fn log_event(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
    ) -> BytesN<32> {
        Self::require_initialized(&env);
        submitter.require_auth();

        // Reject writes when contract is paused.
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }

        // --- issue #62: enforce per-submitter rate limit ---
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
                env.storage().instance().set(
                    &DataKey::SubmitterRateState(submitter.clone()),
                    &(now, count + 1),
                );
            } else {
                if limit == 0 {
                    panic_with_error!(&env, ContractError::RateLimitExceeded);
                }
                env.storage().instance().set(
                    &DataKey::SubmitterRateState(submitter.clone()),
                    &(now, 1u32),
                );
            }
        }

        // --- issue #67: enforce metadata size cap ---
        let max_meta = Self::effective_metadata_max_size(&env, &event_type);
        if metadata.len() > max_meta {
            panic_with_error!(&env, ContractError::MetadataTooLarge);
        }

        // Task 1: single read for both global_max and total (was 2 reads).
        let mut cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();

        if cfg.total_events >= cfg.global_max_logs {
            panic_with_error!(&env, ContractError::GlobalMaxLogsReached);
        }

        // Task 2+5: single read for cap + current count; reuse count later.
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

        let index = cfg.total_events;
        let timestamp = env.ledger().timestamp();

        // --- issue #76: validate timestamp monotonicity and drift ---
        let (prev_hash, prev_timestamp): (BytesN<32>, u64) = if index == 0 {
            (BytesN::from_array(&env, &[0u8; 32]), 0u64)
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
            (prev_evt.event_hash, prev_evt.timestamp)
        };

        if index > 0 {
            if timestamp < prev_timestamp
                || timestamp > prev_timestamp + MAX_TIMESTAMP_DRIFT_SECONDS
            {
                panic_with_error!(&env, ContractError::TimestampOutOfRange);
            }
        }

        // --- issue #70: compute content-addressed event ID ---
        let event_id = Self::compute_event_id(
            &env,
            &submitter,
            &event_type,
            &metadata,
            timestamp,
            index,
        );

        // --- issue #66: compute this event's hash (includes prev_hash) ---
        let event_hash =
            Self::compute_event_hash(&env, &event_id, &prev_hash, index, timestamp);

        let cat = category.unwrap_or(Symbol::new(&env, "general"));
        let evt = Event {
            index,
            timestamp,
            event_type: event_type.clone(),
            category: cat.clone(),
            submitter: submitter.clone(),
            metadata: metadata.clone(),
            sub_event_type: sub_event_type.clone(),
            event_hash: event_hash.clone(),
            prev_hash,
        };

        env.storage()
            .instance()
            .set(&DataKey::EventData(event_id.clone()), &evt);
        env.storage()
            .instance()
            .set(&DataKey::EventOrder(index), &event_id);

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
        // Task 3: removed redundant EventMeta write (same data as EventData).
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(event_id.clone()), &metadata);

        // Task 4: cache low_cost_mode to avoid double read.
        let low_cost = Self::effective_low_cost_mode(&env);

        // --- issue #54: packed-Bytes index storage ---
        if !low_cost {
            Self::push_type_index(&env, event_type.clone(), index);
            // Task 5: reuse cached count instead of re-reading.
            let new_count = type_count_opt.unwrap_or_else(|| Self::event_type_count(&env, event_type.clone())) + 1;
            env.storage()
                .instance()
                .set(&DataKey::EventTypeCount(event_type.clone()), &new_count);
        }

        // Task 4: cache emission_mode to avoid double read.
        let emission_mode = Self::effective_event_emission_mode(&env);

        if low_cost && emission_mode == 1 {
            env.events().publish(
                (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                (index,),
            );
        }

        // Task 1: single write back the updated Config (was separate TotalEvents write).
        cfg.total_events += 1;
        env.storage().instance().set(&DataKey::Config, &cfg);

        match emission_mode {
            1 => {
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                    (index,),
                );
            }
            2 => {
                let metadata_hash: BytesN<32> = env.crypto().sha256(&metadata).into();
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                    (index, metadata_hash.to_val()),
                );
            }
            3 => {
                // No emission (issue #60)
            }
            _ => {
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type, submitter),
                    (index, timestamp, metadata, cat, sub_event_type),
                );
            }
        }

        event_id
    }

    /// Log an event with an explicit nonce to prevent replay attacks (issue #64).
    ///
    /// Rules:
    /// - `nonce` must equal `stored_nonce + 1` (strict sequential) or be any value
    ///   greater than `stored_nonce` (gaps accepted, stored nonce jumps to `nonce`).
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
        let stored: u32 = env
            .storage()
            .instance()
            .get(&DataKey::SubmitterNonce(submitter.clone()))
            .unwrap_or(0);

        if nonce == 0 || nonce <= stored {
            panic_with_error!(&env, ContractError::NonceTooLow);
        }

        let event_id = Self::log_event(env.clone(), submitter.clone(), event_type, metadata);

        env.storage()
            .instance()
            .set(&DataKey::SubmitterNonce(submitter), &nonce);

        event_id
    }

    /// Return the last accepted nonce for `submitter`. Returns 0 if no nonce has been used yet.
    pub fn get_submitter_nonce(env: Env, submitter: Address) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::SubmitterNonce(submitter))
            .unwrap_or(0)
    }

    pub fn total_events(env: Env) -> u32 {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get::<_, Config>(&DataKey::Config)
            .map(|c| c.total_events)
            .unwrap_or(0)
    }

    /// Retrieve an event by its content-addressed ID.
    pub fn get_event(env: Env, id: BytesN<32>) -> Event {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::EventData(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }
    
    /// Retrieve only the event metadata (optimized for low-fee environments, issue #57).
    pub fn get_event_metadata(env: Env, id: BytesN<32>) -> Bytes {
        Self::require_initialized(&env);
        env.storage()
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
        env.storage()
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
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap_or(0u32);
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
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap_or(0u32);
        let mut out: Vec<EventHeader> = Vec::new(&env);
        if start >= total { return out; }
        let mut added: u32 = 0;
        let mut i = start;
        while i < total && added < limit {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            let evt: Event = env.storage().instance().get(&DataKey::EventData(id)).unwrap();
            if evt.category == category {
                let header = EventHeader { index: evt.index, timestamp: evt.timestamp, event_type: evt.event_type.clone(), submitter: evt.submitter.clone() };
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
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap_or(0u32);
        let mut archived: u32 = env.storage().instance().get(&DataKey::ArchivedTotalEvents).unwrap_or(0u32);
        let mut moved: u32 = 0;
        for i in 0..total {
            let id: BytesN<32> = env.storage().instance().get(&DataKey::EventOrder(i)).unwrap();
            // skip if already archived
            if env.storage().instance().has(&DataKey::EventArchivedFlag(id.clone())) {
                continue;
            }
            let evt: Event = env.storage().instance().get(&DataKey::EventData(id.clone())).unwrap();
            if evt.timestamp < cutoff_timestamp {
                // copy into archived storage
                env.storage().instance().set(&DataKey::ArchivedEventData(id.clone()), &evt);
                if let Some(header) = env.storage().instance().get::<_, EventHeader>(&DataKey::EventHeaderKey(id.clone())) {
                    env.storage().instance().set(&DataKey::ArchivedEventHeaderKey(id.clone()), &header);
                }
                if let Some(meta) = env.storage().instance().get::<_, Bytes>(&DataKey::EventMetadata(id.clone())) {
                    env.storage().instance().set(&DataKey::ArchivedEventMetadata(id.clone()), &meta);
                }
                env.storage().instance().set(&DataKey::EventArchivedFlag(id.clone()), &true);
                env.storage().instance().set(&DataKey::ArchivedEventOrder(archived), &id.clone());
                archived += 1;
                moved += 1;
            }
        }
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
        let total: u32 = env.storage().instance().get(&DataKey::ArchivedTotalEvents).unwrap_or(0u32);
        let mut cnt: u32 = 0;
        for i in 0..total {
            if let Some(id) = env.storage().instance().get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i)) {
                if env.storage().instance().has(&DataKey::ArchivedEventData(id)) {
                    cnt += 1;
                }
            }
        }
        cnt
    }

    pub fn list_archived_events(env: Env, start: u32, limit: u32) -> Vec<EventHeader> {
        let total: u32 = env.storage().instance().get(&DataKey::ArchivedTotalEvents).unwrap_or(0u32);
        let mut out: Vec<EventHeader> = Vec::new(&env);
        if start >= total { return out; }
        let mut added: u32 = 0;
        let mut i = start;
        while i < total && added < limit {
            if let Some(id) = env.storage().instance().get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i)) {
                if let Some(header) = env.storage().instance().get::<_, EventHeader>(&DataKey::ArchivedEventHeaderKey(id.clone())) {
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
        if !confirm { return 0u32; }
        let total: u32 = env.storage().instance().get(&DataKey::ArchivedTotalEvents).unwrap_or(0u32);
        let mut removed: u32 = 0;
        for i in 0..total {
            if let Some(id) = env.storage().instance().get::<_, BytesN<32>>(&DataKey::ArchivedEventOrder(i)) {
                if let Some(evt) = env.storage().instance().get::<_, Event>(&DataKey::ArchivedEventData(id.clone())) {
                    if evt.timestamp < cutoff_timestamp {
                        env.storage().instance().remove(&DataKey::ArchivedEventData(id.clone()));
                        env.storage().instance().remove(&DataKey::ArchivedEventHeaderKey(id.clone()));
                        env.storage().instance().remove(&DataKey::ArchivedEventMetadata(id.clone()));
                        // remove archived order mapping
                        env.storage().instance().remove(&DataKey::ArchivedEventOrder(i));
                        removed += 1;
                    }
                }
            }
        }
        env.events().publish((Symbol::new(&env, "archived_events_purged"),), (removed,));
        removed
    }

    /// Upgrade the contract's WASM. Owner-only. Emits `contract_upgraded(old_hash, new_hash)`.
    pub fn upgrade_contract(env: Env, caller: Address, new_wasm_hash: BytesN<32>) {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        // Emit event and attempt to perform the upgrade via the deployer.
        // Note: callers should ensure the new WASM is compatible with storage layout.
        // Try to obtain current wasm hash if available (best-effort).
        let old_hash_opt: Option<BytesN<32>> = None;
        env.events().publish((Symbol::new(&env, "contract_upgraded"),), (old_hash_opt, new_wasm_hash.clone()));
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

        let total = Self::total_events(&env);
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

    pub fn list_events_by_type(
        env: Env,
        event_type: Symbol,
        offset: u32,
        limit: u32,
    ) -> Vec<Event> {
        if limit == 0 {
            return Vec::new(&env);
        }
        if limit > 100 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }

        let total = Self::event_count(&env, event_type.clone());
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

        let total = Self::total_events(&env);
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

        let total = Self::total_events(&env);
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

    pub fn update_event(
        env: Env,
        caller: Address,
        index: u32,
        new_metadata: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let total = Self::total_events(&env);
        if index >= total {
            panic_with_error!(&env, ContractError::EventDoesNotExist);
        }

        let current_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(index))
            .unwrap();
        let current_event: Event = env
            .storage()
            .instance()
            .get(&DataKey::EventData(current_id.clone()))
            .unwrap();

        let max_meta = Self::effective_metadata_max_size(&env, &current_event.event_type);
        if new_metadata.len() > max_meta {
            panic_with_error!(&env, ContractError::MetadataTooLarge);
        }

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

        if versions.len() == 0 {
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

        let updated_event_hash = Self::compute_event_hash(
            &env,
            &new_id,
            &prev_hash,
            index,
            current_event.timestamp,
        );

        let updated_event = Event {
            index,
            timestamp: current_event.timestamp,
            event_type: current_event.event_type.clone(),
            submitter: current_event.submitter.clone(),
            metadata: new_metadata.clone(),
            event_hash: updated_event_hash.clone(),
            prev_hash: prev_hash.clone(),
        };

        let update_version = EventVersion {
            version: versions.len(),
            data: updated_event.clone(),
            updated_at: env.ledger().timestamp(),
            updated_by: caller.clone(),
        };
        versions.push_back(update_version);
        env.storage()
            .instance()
            .set(&DataKey::EventVersions(index), &versions);

        env.storage()
            .instance()
            .set(&DataKey::EventData(new_id.clone()), &updated_event);
        env.storage()
            .instance()
            .set(&DataKey::EventOrder(index), &new_id);
        env.storage()
            .instance()
            .set(&DataKey::EventHeaderKey(new_id.clone()), &EventHeader {
                index,
                timestamp: current_event.timestamp,
                event_type: current_event.event_type.clone(),
                submitter: current_event.submitter.clone(),
            });
        env.storage()
            .instance()
            .set(&DataKey::EventMeta(new_id.clone()), &updated_event);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(new_id.clone()), &new_metadata);

        let mut next_prev_hash = updated_event_hash;
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
            later_event.event_hash = Self::compute_event_hash(
                &env,
                &event_id,
                &later_event.prev_hash,
                i,
                later_event.timestamp,
            );
            env.storage()
                .instance()
                .set(&DataKey::EventData(event_id.clone()), &later_event);
            env.storage()
                .instance()
                .set(&DataKey::EventMeta(event_id.clone()), &later_event);
            next_prev_hash = later_event.event_hash.clone();
        }

        env.events().publish(
            (Symbol::new(&env, "event_updated"),),
            (index, current_id, new_id.clone(), caller, env.ledger().timestamp()),
        );

        new_id
    }

    pub fn get_event_history(env: Env, index: u32) -> Vec<EventVersion> {
        let total = Self::total_events(&env);
        if index >= total {
            return Vec::new(&env);
        }

        if let Some(versions) = env.storage().instance().get::<_, Vec<EventVersion>>(
            &DataKey::EventVersions(index),
        ) {
            return versions;
        }

        let event_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::EventOrder(index))
            .unwrap();
        let event: Event = env
            .storage()
            .instance()
            .get(&DataKey::EventData(event_id))
            .unwrap();

        let mut history = Vec::new(&env);
        history.push_back(EventVersion {
            version: 0,
            data: event,
            updated_at: event.timestamp,
            updated_by: event.submitter.clone(),
        });
        history
    }

    // ── Integrity verification (issue #66) ──────────────────────────────────

    /// Verify the full hash chain. Returns `true` if every event's
    /// `prev_hash` matches the previous event's `event_hash`.
    pub fn verify_integrity(env: Env) -> bool {
        Self::require_initialized(&env);
        let total: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalEvents)
            .unwrap_or(0);
        Self::verify_range(&env, 0, total)
    }

    /// Verify a range `[from, to)` of the hash chain.
    pub fn verify_integrity_range(env: Env, from: u32, to: u32) -> bool {
        Self::require_initialized(&env);
        Self::verify_range(&env, from, to)
    }

    // ── Governance ──────────────────────────────────────────────────────────

    pub fn set_global_max_logs(env: Env, caller: Address, new_max: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        // governance writes should be blocked while paused
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        let mut cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        cfg.global_max_logs = new_max;
        env.storage().instance().set(&DataKey::Config, &cfg);
    }

    pub fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventCapConfig(event_type.clone()), &Some(new_max));

        if !Self::effective_low_cost_mode(&env) {
            if !env
                .storage()
                .instance()
                .has(&DataKey::EventTypeIndices(event_type.clone()))
            {
                env.storage()
                    .instance()
                    .set(&DataKey::EventTypeIndices(event_type.clone()), &Bytes::new(&env));
            }
        }
    }

    pub fn remove_event_cap(env: Env, caller: Address, event_type: Symbol) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        if !env
            .storage()
            .instance()
            .has(&DataKey::EventCapConfig(event_type.clone()))
        {
            panic_with_error!(&env, ContractError::CapNotSet);
        }
        env.storage()
            .instance()
            .remove(&DataKey::EventCapConfig(event_type.clone()));
        env.storage()
            .instance()
            .remove(&DataKey::EventTypeCount(event_type.clone()));

        if Self::effective_low_cost_mode(&env) {
            env.storage()
                .instance()
                .remove(&DataKey::EventTypeIndices(event_type.clone()));
        }
    }

    pub fn transfer_ownership(env: Env, caller: Address, new_owner: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        if new_owner == Address::from_str(&env, NULL_ACCOUNT) {
            panic_with_error!(&env, ContractError::NewOwnerIsZero);
        }
        env.storage().instance().set(&DataKey::Owner, &new_owner);
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
        env.storage()
            .instance()
            .set(&DataKey::GlobalMetadataMaxSize, &max_size);
    }

    /// Set a per-event-type metadata size limit (owner-only).
    /// Overrides the global limit for the given event type.
    pub fn set_event_metadata_max_size(
        env: Env,
        caller: Address,
        event_type: Symbol,
        max_size: u32,
    ) {
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

    /// Pause write operations. Owner-only. Works even if contract already paused.
    pub fn pause(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((Symbol::new(&env, "contract_paused"),), (caller,));
    }

    /// Unpause write operations. Owner-only.
    pub fn unpause(env: Env, caller: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((Symbol::new(&env, "contract_unpaused"),), (caller,));
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
        env.storage()
            .instance()
            .set(&DataKey::EventEmissionConfig, &mode);
        env.storage()
            .instance()
            .set(&DataKey::EventEmissionVersion, &2u32);
    }
    
    /// Get the current event emission mode.
    pub fn get_event_emission_mode(env: Env) -> u32 {
        Self::require_initialized(&env);
        Self::effective_event_emission_mode(&env)
    }
    
    /// Enable/disable low-cost mode (owner-only).
    /// Low-cost mode sacrifices some features (e.g., per-type indexing) for lower per-event cost.
    /// This is useful for environments with strict fee budgets (issue #57).
    pub fn set_low_cost_mode(env: Env, caller: Address, enabled: bool) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::LowCostMode, &enabled);
    }
    
    /// Check if low-cost mode is enabled.
    pub fn is_low_cost_mode(env: Env) -> bool {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::LowCostMode)
            .unwrap_or(false)
    }

    // ── issue #62: rate limiting ──────────────────────────────────────────────

    /// Set a per-submitter rate limit (owner-only).
    /// `max_per_timestamp` = max events allowed per ledger timestamp.
    /// 0 = completely block that submitter.
    pub fn set_submitter_rate_limit(
        env: Env,
        caller: Address,
        submitter: Address,
        max_per_timestamp: u32,
    ) {
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
        Self::require_owner(&env, &caller);

        let mut removed: u32 = 0;
        for i in 0..stale_types.len() {
            let et = stale_types.get(i).unwrap();
            // Only compact if the cap is no longer set (i.e., was removed).
            if !env
                .storage()
                .instance()
                .has(&DataKey::EventCapConfig(et.clone()))
            {
                if env
                    .storage()
                    .instance()
                    .has(&DataKey::EventTypeIndices(et.clone()))
                {
                    env.storage()
                        .instance()
                        .remove(&DataKey::EventTypeIndices(et.clone()));
                    removed += 1;
                }
            }
        }

        env.events().publish(
            (Symbol::new(&env, "storage_compacted"),),
            (removed,),
        );
        removed
    }
    
    fn effective_low_cost_mode(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::LowCostMode)
            .unwrap_or(false)
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
        if let Some(v) = env
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::GlobalMetadataMaxSize)
        {
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
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        signature_payload: Bytes,
    ) -> BytesN<32> {
        // Delegates auth to the inner log_event call.
        if signature_payload.len() != 96 {
            panic_with_error!(&env, ContractError::InvalidSignature);
        }
        let event_id = Self::log_event(env.clone(), submitter, event_type, metadata.clone(), category, sub_event_type);
        env.storage()
            .instance()
            .set(&DataKey::EventSignature(event_id.clone()), &signature_payload);
        event_id
    }

    /// Return the stored 96-byte signature payload (pubkey || signature) for an
    /// event. Returns `None` if no signature was attached during logging.
    pub fn get_event_signature(env: Env, event_id: BytesN<32>) -> Option<Bytes> {
        Self::require_initialized(&env);
        env.storage()
            .instance()
            .get(&DataKey::EventSignature(event_id))
    }

    // ── Private helpers ─────────────────────────────────────────────────────

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

    fn require_owner(env: &Env, addr: &Address) {
        let owner: Address = env.storage().instance().get(&DataKey::Owner).unwrap();
        if addr != &owner {
            panic_with_error!(env, ContractError::CallerNotOwner);
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
        let mut owners = Self::get_owners(&env);
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
        if req as u32 > new_vec.len() {
            // reduce required signatures to new_vec.len()
            env.storage()
                .instance()
                .set(&DataKey::RequiredSignatures, &new_vec.len());
        }
        env.storage().instance().set(&DataKey::Owners, &new_vec);
        env.events().publish((Symbol::new(&env, "owner_removed"),), (owner_to_remove,));
    }

    pub fn set_required_signatures(env: Env, caller: Address, required: u32) {
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let owners = Self::get_owners(&env);
        if required == 0 || required > owners.len() {
            return; // invalid; ignore
        }
        env.storage()
            .instance()
            .set(&DataKey::RequiredSignatures, &required);
        env.events().publish((Symbol::new(&env, "required_signatures_set"),), (required,));
    }

    pub fn submit_proposal(env: Env, proposer: Address, action: ProposalAction, ttl_seconds: u64) -> u32 {
        proposer.require_auth();
        if !Self::is_addr_owner(&env, &proposer) {
            panic_with_error!(&env, ContractError::CallerNotOwner);
        }
        let mut count: u32 = env.storage().instance().get(&DataKey::ProposalCount).unwrap_or(0u32);
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
        env.events().publish((Symbol::new(&env, "proposal_approved"),), (proposal_id, approver));
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
        match prop.action {
            ProposalAction::TransferOwnership(new_owner) => {
                env.storage().instance().set(&DataKey::Owner, &new_owner);
            }
            ProposalAction::AddOwner(addr) => {
                let mut owners = Self::get_owners(&env);
                let mut exists = false;
                for i in 0..owners.len() { if owners.get(i).unwrap() == addr { exists = true; } }
                if !exists { owners.push_back(addr); env.storage().instance().set(&DataKey::Owners, &owners); }
            }
            ProposalAction::RemoveOwner(addr) => {
                let mut owners = Self::get_owners(&env);
                let mut new_vec: Vec<Address> = Vec::new(&env);
                for i in 0..owners.len() { let o = owners.get(i).unwrap(); if o != addr { new_vec.push_back(o.clone()); } }
                env.storage().instance().set(&DataKey::Owners, &new_vec);
            }
            ProposalAction::SetRequiredSignatures(req) => {
                env.storage().instance().set(&DataKey::RequiredSignatures, &req);
            }
            ProposalAction::SetGlobalMaxLogs(v) => {
                env.storage().instance().set(&DataKey::GlobalMaxLogs, &v);
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
        env.events().publish((Symbol::new(&env, "proposal_executed"),), (proposal_id, executor));
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
            let id: BytesN<32> = match env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(i))
            {
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
            let recomputed =
                Self::compute_event_hash(env, &id, &evt.prev_hash, i, evt.timestamp);
            if evt.event_hash != recomputed {
                return false;
            }
            expected_prev = evt.event_hash.clone();
        }
        true
    }

    fn collect_statistics(env: &Env) -> ContractStatistics {
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap_or(0);
        let now = env.ledger().timestamp();
        let mut events_by_type: Vec<(Symbol, u32)> = Vec::new(&env);
        let mut top_submitters: Vec<(Address, u32)> = Vec::new(&env);
        let mut events_last_hour: u32 = 0;
        let mut events_last_day: u32 = 0;
        let mut events_last_week: u32 = 0;

        for i in 0..total {
            let event_id: BytesN<32> = env
                .storage()
                .instance()
                .get(&DataKey::EventOrder(i))
                .unwrap();
            let evt: Event = env
                .storage()
                .instance()
                .get(&DataKey::EventData(event_id))
                .unwrap();

            Self::increment_type_count(&env, &mut events_by_type, evt.event_type.clone());
            Self::increment_submitter_count(&env, &mut top_submitters, evt.submitter.clone());

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

    fn increment_type_count(env: &Env, counts: &mut Vec<(Symbol, u32)>, event_type: Symbol) {
        for idx in 0..counts.len() {
            let pair: (Symbol, u32) = counts.get(idx).unwrap();
            if pair.0 == event_type {
                counts.set(idx, &(event_type.clone(), pair.1 + 1));
                return;
            }
        }
        counts.push_back(&(event_type, 1u32));
    }

    fn increment_submitter_count(env: &Env, counts: &mut Vec<(Address, u32)>, submitter: Address) {
        for idx in 0..counts.len() {
            let pair: (Address, u32) = counts.get(idx).unwrap();
            if pair.0 == submitter {
                counts.set(idx, &(submitter.clone(), pair.1 + 1));
                return;
            }
        }
        counts.push_back(&(submitter, 1u32));
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
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod regression_tests;

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod cross_contract_tests;
