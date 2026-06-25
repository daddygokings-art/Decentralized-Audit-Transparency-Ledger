#![no_std]

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Default maximum metadata size (1 KB). Used when no explicit cap is set.
const DEFAULT_MAX_METADATA_SIZE: u32 = 1024;

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
    pub submitter: Address,
    pub metadata: Bytes,
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

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Owner,
    GlobalMaxLogs,
    /// Paused flag: when true, write operations are blocked.
    Paused,
    TotalEvents,
    EventCapSet(Symbol),
    EventMaxLogs(Symbol),
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
}

const NULL_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

#[contract]
pub struct AuditLedger;

#[contractimpl]
impl AuditLedger {
    pub fn initialize(env: Env, owner: Address, global_max_logs: u32) {
        owner.require_auth();
        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DataKey::GlobalMaxLogs, &global_max_logs);
        env.storage().instance().set(&DataKey::TotalEvents, &0u32);
        // start unpaused
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Log an event and return its content-addressed `BytesN<32>` ID.
    pub fn log_event(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
    ) -> BytesN<32> {
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

        let global_max: u32 = env
            .storage()
            .instance()
            .get(&DataKey::GlobalMaxLogs)
            .unwrap();
        let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap();

        if total >= global_max {
            panic_with_error!(&env, ContractError::GlobalMaxLogsReached);
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
            let count = Self::event_type_count(&env, event_type.clone());
            if count >= cap {
                panic_with_error!(&env, ContractError::EventTypeMaxLogsReached);
            }
        }

        let index = total;
        let timestamp = env.ledger().timestamp();

        // --- issue #66: retrieve previous hash ---
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
            prev_evt.event_hash
        };

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

        let evt = Event {
            index,
            timestamp,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
            metadata: metadata.clone(),
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
        env.storage()
            .instance()
            .set(&DataKey::EventMeta(event_id.clone()), &evt);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadata(event_id.clone()), &metadata);

        // --- issue #54: packed-Bytes index storage ---
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
        
        if Self::effective_low_cost_mode(&env) {
            // In low-cost mode, emit only index to save gas
            let emission_mode = Self::effective_event_emission_mode(&env);
            if emission_mode == 1 {
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                    (index,),
                );
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::TotalEvents, &(total + 1));

        let emission_mode = Self::effective_event_emission_mode(&env);
        match emission_mode {
            1 => {
                // Index-only emission (issue #60)
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                    (index,),
                );
            }
            2 => {
                // Hash-only emission (issue #60)
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
                // Default: full metadata emission (backward compatible)
                env.events().publish(
                    (Symbol::new(&env, "log_event"), event_type.clone(), submitter.clone()),
                    (index, timestamp, metadata),
                );
            }
        }

        event_id
    }

    pub fn total_events(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TotalEvents)
            .unwrap_or(0)
    }

    /// Retrieve an event by its content-addressed ID.
    pub fn get_event(env: Env, id: BytesN<32>) -> Event {
        env.storage()
            .instance()
            .get(&DataKey::EventData(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }
    
    /// Retrieve only the event metadata (optimized for low-fee environments, issue #57).
    pub fn get_event_metadata(env: Env, id: BytesN<32>) -> Bytes {
        env.storage()
            .instance()
            .get(&DataKey::EventMetadata(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }
    
    /// Retrieve only the event header (index, timestamp, event_type, submitter) — no metadata (issue #56).
    pub fn get_event_header(env: Env, id: BytesN<32>) -> EventHeader {
        env.storage()
            .instance()
            .get(&DataKey::EventHeaderKey(id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventDoesNotExist);
            })
    }

    /// Retrieve an event by its sequential insertion order (0-based).
    pub fn get_event_by_order(env: Env, order: u32) -> Event {
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
        if Self::effective_low_cost_mode(&env) {
            panic_with_error!(&env, ContractError::CapNotSet);
        }
        Self::event_type_count(&env, event_type)
    }

    pub fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event {
        if Self::effective_low_cost_mode(&env) {
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

    // ── Integrity verification (issue #66) ──────────────────────────────────

    /// Verify the full hash chain. Returns `true` if every event's
    /// `prev_hash` matches the previous event's `event_hash`.
    pub fn verify_integrity(env: Env) -> bool {
        let total: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalEvents)
            .unwrap_or(0);
        Self::verify_range(&env, 0, total)
    }

    /// Verify a range `[from, to)` of the hash chain.
    pub fn verify_integrity_range(env: Env, from: u32, to: u32) -> bool {
        Self::verify_range(&env, from, to)
    }

    // ── Governance ──────────────────────────────────────────────────────────

    pub fn set_global_max_logs(env: Env, caller: Address, new_max: u32) {
        caller.require_auth();
        // governance writes should be blocked while paused
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::GlobalMaxLogs, &new_max);
    }

    pub fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32) {
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventCapSet(event_type.clone()), &true);
        env.storage()
            .instance()
            .set(&DataKey::EventMaxLogs(event_type.clone()), &new_max);
        
        if !Self::effective_low_cost_mode(&env) {
            if !env
                .storage()
                .instance()
                .has(&DataKey::EventTypeCount(event_type.clone()))
            {
                env.storage()
                    .instance()
                    .set(&DataKey::EventTypeCount(event_type.clone()), &0u32);
            }
        }
    }

    pub fn remove_event_cap(env: Env, caller: Address, event_type: Symbol) {
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        if !env
            .storage()
            .instance()
            .has(&DataKey::EventCapSet(event_type.clone()))
        {
            panic_with_error!(&env, ContractError::CapNotSet);
        }
        env.storage()
            .instance()
            .remove(&DataKey::EventCapSet(event_type.clone()));
        env.storage()
            .instance()
            .remove(&DataKey::EventMaxLogs(event_type.clone()));
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
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
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
        caller.require_auth();
        if let Some(true) = env.storage().instance().get::<_, bool>(&DataKey::Paused) {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadataMaxSize(event_type), &max_size);
    }

    /// Pause write operations. Owner-only. Works even if contract already paused.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((Symbol::new(&env, "contract_paused"),), (caller,));
    }

    /// Unpause write operations. Owner-only.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((Symbol::new(&env, "contract_unpaused"),), (caller,));
    }

    /// Get the effective metadata size limit for the given event type.
    /// Returns the per-type cap if set, otherwise the global cap, otherwise the default.
    pub fn get_metadata_max_size(env: Env, event_type: Symbol) -> u32 {
        Self::effective_metadata_max_size(&env, &event_type)
    }
    
    /// Set the event emission mode (owner-only).
    /// 0 = full metadata emission (default, backward compatible)
    /// 1 = index-only emission (issue #60)
    /// 2 = hash-only emission (issue #60)
    /// 3 = no emission (issue #60)
    pub fn set_event_emission_mode(env: Env, caller: Address, mode: u32) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventEmissionConfig, &mode);
        env.storage()
            .instance()
            .set(&DataKey::EventEmissionVersion, &2u32);
    }
    
    /// Get the current event emission mode.
    pub fn get_event_emission_mode(env: Env) -> u32 {
        Self::effective_event_emission_mode(&env)
    }
    
    /// Enable/disable low-cost mode (owner-only).
    /// Low-cost mode sacrifices some features (e.g., per-type indexing) for lower per-event cost.
    /// This is useful for environments with strict fee budgets (issue #57).
    pub fn set_low_cost_mode(env: Env, caller: Address, enabled: bool) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::LowCostMode, &enabled);
    }
    
    /// Check if low-cost mode is enabled.
    pub fn is_low_cost_mode(env: Env) -> bool {
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
        caller.require_auth();
        Self::require_owner(&env, &caller);
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
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut removed: u32 = 0;
        for i in 0..stale_types.len() {
            let et = stale_types.get(i).unwrap();
            // Only compact if the cap is no longer set (i.e., was removed).
            if !env
                .storage()
                .instance()
                .has(&DataKey::EventCapSet(et.clone()))
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
                if env
                    .storage()
                    .instance()
                    .has(&DataKey::EventTypeCount(et.clone()))
                {
                    env.storage()
                        .instance()
                        .remove(&DataKey::EventTypeCount(et.clone()));
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
        signature_payload: Bytes,
    ) -> BytesN<32> {
        // Delegates auth to the inner log_event call.
        if signature_payload.len() != 96 {
            panic_with_error!(&env, ContractError::InvalidSignature);
        }
        let event_id = Self::log_event(env.clone(), submitter, event_type, metadata.clone());
        env.storage()
            .instance()
            .set(&DataKey::EventSignature(event_id.clone()), &signature_payload);
        event_id
    }

    /// Return the stored 96-byte signature payload (pubkey || signature) for an
    /// event. Returns `None` if no signature was attached during logging.
    pub fn get_event_signature(env: Env, event_id: BytesN<32>) -> Option<Bytes> {
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

    fn event_type_count(env: &Env, event_type: Symbol) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::EventTypeCount(event_type))
            .unwrap_or(0)
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
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod regression_tests;

#[cfg(test)]
mod boundary_tests;

#[cfg(test)]
mod cross_contract_tests;
