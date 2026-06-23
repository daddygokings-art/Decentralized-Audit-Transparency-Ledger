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

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Owner,
    GlobalMaxLogs,
    TotalEvents,
    EventCapSet(Symbol),
    EventMaxLogs(Symbol),
    /// Stores `Vec<BytesN<32>>` – ordered list of event IDs for a type.
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
    }

    /// Log an event and return its content-addressed `BytesN<32>` ID.
    pub fn log_event(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
    ) -> BytesN<32> {
        submitter.require_auth();

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

        let mut indices: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type.clone()))
            .unwrap_or(Vec::new(&env));
        indices.push_back(event_id.clone());
        env.storage()
            .instance()
            .set(&DataKey::EventTypeIndices(event_type.clone()), &indices);

        env.storage()
            .instance()
            .set(&DataKey::TotalEvents, &(total + 1));

        env.events().publish(
            (Symbol::new(&env, "log_event"), event_type, submitter),
            (index, timestamp, metadata),
        );

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
        Self::event_type_count(&env, event_type)
    }

    pub fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event {
        let indices: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type.clone()))
            .unwrap_or_else(|| {
                panic_with_error!(&env, ContractError::EventTypeIndexOutOfBounds);
            });

        let event_id = indices.get(type_index).unwrap_or_else(|| {
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
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::GlobalMaxLogs, &new_max);
    }

    pub fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventCapSet(event_type.clone()), &true);
        env.storage()
            .instance()
            .set(&DataKey::EventMaxLogs(event_type), &new_max);
    }

    pub fn remove_event_cap(env: Env, caller: Address, event_type: Symbol) {
        caller.require_auth();
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
            .remove(&DataKey::EventMaxLogs(event_type));
    }

    pub fn transfer_ownership(env: Env, caller: Address, new_owner: Address) {
        caller.require_auth();
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
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::EventMetadataMaxSize(event_type), &max_size);
    }

    /// Get the effective metadata size limit for the given event type.
    /// Returns the per-type cap if set, otherwise the global cap, otherwise the default.
    pub fn get_metadata_max_size(env: Env, event_type: Symbol) -> u32 {
        Self::effective_metadata_max_size(&env, &event_type)
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

    fn require_owner(env: &Env, addr: &Address) {
        let owner: Address = env.storage().instance().get(&DataKey::Owner).unwrap();
        if addr != &owner {
            panic_with_error!(env, ContractError::CallerNotOwner);
        }
    }

    fn event_type_count(env: &Env, event_type: Symbol) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::EventTypeIndices(event_type))
            .map(|v: Vec<BytesN<32>>| v.len())
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
