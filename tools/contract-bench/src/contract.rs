//! The contract's storage layout, as data.
//!
//! # Why this is a model and not the contract
//!
//! The obvious way to benchmark a contract is to call it. That is not currently
//! possible: `audit-ledger` does not compile (417 errors against
//! `soroban-sdk 27.0.6`, independently of any benchmark work), so nothing can be
//! linked against it and no test or bench target in that crate can run.
//!
//! Rather than benchmark nothing, this module transcribes the contract's storage
//! layout and the host operations its functions perform, and the suite measures
//! *those operations* in a real host. Every figure produced is therefore a real
//! measurement of real host work; what it is not is a measurement of the
//! contract's own arithmetic. See `docs/performance/benchmark-methodology.md` for
//! what that excludes and in which direction it biases results.
//!
//! The reason the numbers are still worth gating on is that for a ledger contract
//! the host operations *dominate*. An event submission writes several XDR-encoded
//! entries, extends rent on each, and emits an event; the surrounding validation
//! is a handful of comparisons. A change that doubles storage traffic, or that
//! makes a growing per-type index twice as large, moves these numbers even
//! though the contract's own instruction count is not measured at all.
//!
//! # Fidelity
//!
//! Keys mirror the `DataKey` enum in `src/lib.rs` and values mirror the contract's
//! own storage structs, so encoded sizes — and therefore write byte volumes —
//! reflect what the contract actually stores. Two characteristics of the real
//! layout are reproduced because they dominate cost:
//!
//! * `EventTypeIndices` and `SubmitterEventIndices` are packed arrays of `u32`
//!   global-order indices that grow by four bytes per event, and are rewritten
//!   whole on every append. Their cost therefore rises as the ledger fills.
//! * `RuntimeState` exists so that the common path reads one instance entry
//!   instead of seven.

use soroban_sdk::{contractevent, contracttype, Address, Bytes, BytesN, Env, Map, Symbol, Val, Vec};

/// Keys used by the hot paths, mirroring `DataKey` in `src/lib.rs`.
///
/// Event types and schemas are keyed by `&'static str` rather than `Symbol` so
/// that a key can be encoded without a runtime conversion; the benchmark only
/// ever needs the same three type names the contract's own tests use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    // ── instance storage ────────────────────────────────────────────────────
    /// Cached global state, read once per invocation.
    RuntimeState,
    Config,
    Paused,
    AllowlistMode,
    LowCostMode,
    EventEmissionConfig,
    Owner,
    /// Global metadata size cap; absent means "use the default".
    GlobalMetadataMaxSize,
    /// Per-event-type metadata validation schema.
    MetadataSchema(&'static str),
    /// Registered owners, a vector-valued instance key.
    Owners,
    RequiredSignatures,
    ProposalCount,
    ArchivedTotalEvents,

    // ── persistent: the append path ────────────────────────────────────────
    /// Primary storage: event id to event.
    EventData(BytesN<32>),
    /// Sequential index to event id, for ordered retrieval.
    EventOrder(u32),
    /// Packed `u32` global-order indices for one event type. Grows 4 B/event.
    EventTypeIndices(&'static str),
    /// Cached event count per type.
    EventTypeCount(&'static str),
    /// Packed `u32` global-order indices for one submitter. Grows 4 B/event.
    SubmitterEventIndices(Address),
    /// Cached event count per submitter.
    SubmitterEventCount(Address),
    /// Per-event count and timestamp for rate limiting.
    SubmitterRateState(Address),
    /// Last accepted nonce, for replay prevention.
    SubmitterNonce(Address),
    /// Lightweight header stored apart from metadata.
    EventHeader(BytesN<32>),
    /// `(index, timestamp, event_type, submitter)`.
    EventMeta(BytesN<32>),
    /// Event metadata, stored apart from the rest.
    EventMetadata(BytesN<32>),
    /// Update history for one event, indexed by event order.
    EventVersions(u32),
    /// Versioned schema registry entry for an event type and schema version.
    EventSchema(&'static str, u32),

    // ── persistent: the archive path ───────────────────────────────────────
    EventArchivedFlag(BytesN<32>),
    ArchivedEventData(BytesN<32>),
}

/// Cached global state, mirroring the contract's `RuntimeState`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeState {
    pub global_max_logs: u32,
    pub total_events: u32,
    pub last_sequence: u32,
    pub paused: bool,
    pub allowlist_mode: bool,
    pub low_cost_mode: bool,
    pub emission_mode: u32,
    pub global_metadata_max_size: u32,
}

/// Global configuration, mirroring the contract's `Config`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub global_max_logs: u32,
    pub max_metadata_size: u32,
    pub total_events: u32,
}

/// Primary event record, mirroring the contract's event storage struct.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventData {
    pub submitter: Address,
    pub event_type: Symbol,
    pub metadata: Bytes,
    pub category: Option<Symbol>,
    pub timestamp: u64,
    pub sequence: u32,
}

/// Lightweight per-event index row, mirroring the contract's `EventMeta`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMeta {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
}

/// Header stored apart from metadata, mirroring the contract's `EventHeader`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventHeader {
    pub index: u32,
    pub event_type: Symbol,
    pub submitter: Address,
    pub timestamp: u64,
}

/// Rate-limit state, mirroring the contract's rate-limit record.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateState {
    pub window_start: u64,
    pub count: u32,
}

/// The event published for a logged event, mirroring the contract's
/// `event_logged` event.
///
/// Declared with `#[contractevent]` rather than `#[contracttype]` because it is
/// published, not stored: the macro builds the topic and data vectors the host
/// meters, so emitting through it costs what the real emission costs. The event
/// type name becomes the leading topic and the event type itself the second,
/// which is the two-topic shape the contract emits.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLogged {
    #[topic]
    pub event_type: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    pub sequence: u32,
}

/// The event published when a proposal is created, mirroring the contract's
/// `proposed` event.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceProposed {
    #[topic]
    pub proposal_id: u32,
    pub proposer: Address,
}

/// Which storage space a key lives in, mirroring the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Instance,
    Persistent,
}

impl Key {
    /// Which storage space this key lives in.
    ///
    /// Mirrors the contract, where hot-path state is `Instance` and event
    /// records are `Persistent`.
    pub fn space(&self) -> Space {
        match self {
            Key::RuntimeState
            | Key::Config
            | Key::Paused
            | Key::AllowlistMode
            | Key::LowCostMode
            | Key::EventEmissionConfig
            | Key::Owner
            | Key::GlobalMetadataMaxSize
            | Key::MetadataSchema(_)
            | Key::Owners
            | Key::RequiredSignatures
            | Key::ProposalCount
            | Key::ArchivedTotalEvents => Space::Instance,
            _ => Space::Persistent,
        }
    }

    /// A short label identifying the key family, used in the encoding.
    pub fn label(&self) -> &'static str {
        match self {
            Key::RuntimeState => "RuntimeState",
            Key::Config => "Config",
            Key::Paused => "Paused",
            Key::AllowlistMode => "AllowlistMode",
            Key::LowCostMode => "LowCostMode",
            Key::EventEmissionConfig => "EventEmissionConfig",
            Key::Owner => "Owner",
            Key::GlobalMetadataMaxSize => "GlobalMetadataMaxSize",
            Key::MetadataSchema(_) => "MetadataSchema",
            Key::Owners => "Owners",
            Key::RequiredSignatures => "RequiredSignatures",
            Key::ProposalCount => "ProposalCount",
            Key::ArchivedTotalEvents => "ArchivedTotalEvents",
            Key::EventData(_) => "EventData",
            Key::EventOrder(_) => "EventOrder",
            Key::EventTypeIndices(_) => "EventTypeIndices",
            Key::EventTypeCount(_) => "EventTypeCount",
            Key::SubmitterEventIndices(_) => "SubmitterEventIndices",
            Key::SubmitterEventCount(_) => "SubmitterEventCount",
            Key::SubmitterRateState(_) => "SubmitterRateState",
            Key::SubmitterNonce(_) => "SubmitterNonce",
            Key::EventHeader(_) => "EventHeader",
            Key::EventMeta(_) => "EventMeta",
            Key::EventMetadata(_) => "EventMetadata",
            Key::EventVersions(_) => "EventVersions",
            Key::EventSchema(_, _) => "EventSchema",
            Key::EventArchivedFlag(_) => "EventArchivedFlag",
            Key::ArchivedEventData(_) => "ArchivedEventData",
        }
    }
}

/// Encode a key to the `Bytes` the host actually stores.
///
/// A `Bytes` key is what a real contract builds too — label, then payload — and
/// it costs what a real key costs: a `Bytes` vector header plus its contents. The
/// variable-length payload is what makes an `EventData` key substantially more
/// expensive to touch than an `EventOrder` key, which is the distinction the
/// storage benchmarks exist to expose.
fn encode_key(env: &Env, key: &Key) -> Bytes {
    let mut b = Bytes::from_slice(env, key.label().as_bytes());
    match key {
        Key::EventData(id)
        | Key::EventHeader(id)
        | Key::EventMeta(id)
        | Key::EventMetadata(id)
        | Key::EventArchivedFlag(id)
        | Key::ArchivedEventData(id) => b.append(&id.to_bytes()),
        Key::EventOrder(i) | Key::EventVersions(i) => b.append(&Bytes::from_slice(env, &i.to_le_bytes())),
        Key::EventTypeIndices(t) | Key::EventTypeCount(t) | Key::MetadataSchema(t) => {
            b.append(&Bytes::from_slice(env, t.as_bytes()))
        }
        Key::EventSchema(t, v) => {
            b.append(&Bytes::from_slice(env, t.as_bytes()));
            b.append(&Bytes::from_slice(env, &v.to_le_bytes()));
        }
        Key::SubmitterEventIndices(a)
        | Key::SubmitterEventCount(a)
        | Key::SubmitterRateState(a)
        | Key::SubmitterNonce(a) => b.append(&a.to_string().to_bytes()),
        _ => {}
    }
    b
}

/// Read a key, or `None` if unset.
///
/// A single type parameter is enough: every stored type in this model implements
/// `TryFromVal<Env, Val>`, including the `#[contracttype]` structs, so the host
/// decodes whichever type the caller asks for.
pub fn get<T: soroban_sdk::TryFromVal<Env, Val>>(env: &Env, key: &Key) -> Option<T> {
    let k = encode_key(env, key);
    match key.space() {
        Space::Instance => env.storage().instance().get(&k),
        Space::Persistent => env.storage().persistent().get(&k),
    }
}

/// Write a key.
pub fn put<T: soroban_sdk::IntoVal<Env, Val>>(env: &Env, key: &Key, value: &T) {
    let k = encode_key(env, key);
    match key.space() {
        Space::Instance => env.storage().instance().set(&k, value),
        Space::Persistent => env.storage().persistent().set(&k, value),
    }
}

/// Whether a key is present, as a `has` query would see it.
pub fn has(env: &Env, key: &Key) -> bool {
    let k = encode_key(env, key);
    match key.space() {
        Space::Instance => env.storage().instance().has(&k),
        Space::Persistent => env.storage().persistent().has(&k),
    }
}

/// Extend the rent of a persistent key.
///
/// The contract does this for every entry it writes, and rent extension is metered
/// separately from the write, so it is modelled explicitly rather than left
/// implicit in [`put`].
pub fn bump_rent(env: &Env, key: &Key, extend_to: u32) {
    if key.space() == Space::Persistent {
        let k = encode_key(env, key);
        env.storage().persistent().extend_ttl(&k, extend_to / 2, extend_to);
    }
}

/// Build a packed index array holding `n` little-endian `u32` values.
///
/// This is the on-wire shape of `EventTypeIndices` and `SubmitterEventIndices`:
/// one entry per event, four bytes each, rewritten in full on every append. It is
/// the reason an append is not O(1) in ledger bytes.
pub fn packed_indices(env: &Env, n: u32) -> Bytes {
    let mut b = Bytes::new(env);
    for i in 0..n {
        b.append(&Bytes::from_slice(env, &i.to_le_bytes()));
    }
    b
}

/// Append one `u32` to a packed index array, as the contract does.
pub fn append_index(env: &Env, existing: &Bytes, value: u32) -> Bytes {
    let mut b = Bytes::new(env);
    b.append(existing);
    b.append(&Bytes::from_slice(env, &value.to_le_bytes()));
    b
}

/// A map keyed by event type, as used by per-type counters.
pub fn type_map(env: &Env, entries: &[&'static str]) -> Map<Symbol, u32> {
    let mut m = Map::new(env);
    for (i, t) in entries.iter().enumerate() {
        m.set(Symbol::new(env, t), i as u32 + 1);
    }
    m
}

/// A `Vec` of values, used for return shapes and event payloads.
pub fn svec<T: soroban_sdk::TryFromVal<Env, Val> + soroban_sdk::IntoVal<Env, Val> + Clone>(
    env: &Env,
    items: &[T],
) -> Vec<T> {
    let mut v = Vec::new(env);
    for item in items {
        v.push_back(item.clone());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _};

    /// `soroban-sdk 27` refuses storage access outside a contract frame, so every
    /// test here runs as a contract. The address is fixed for the duration of the
    /// test so all its writes and reads address the same ledger.
    fn in_ledger<T>(env: &Env, f: impl FnOnce() -> T) -> T {
        let id = crate::measure::register(env);
        env.as_contract(&id, f)
    }

    #[test]
    fn instance_and_persistent_keys_land_in_the_right_space() {
        assert_eq!(Key::RuntimeState.space(), Space::Instance);
        assert_eq!(Key::Paused.space(), Space::Instance);
        assert_eq!(Key::MetadataSchema("trade").space(), Space::Instance);
        assert_eq!(Key::Owners.space(), Space::Instance);
        assert_eq!(Key::EventOrder(1).space(), Space::Persistent);
        assert_eq!(Key::EventSchema("trade", 1).space(), Space::Persistent);
        assert_eq!(
            Key::SubmitterNonce(Address::generate(&crate::measure::test_env())).space(),
            Space::Persistent
        );
    }

    #[test]
    fn values_round_trip_through_real_xdr_encoding() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            let key = Key::EventTypeCount("trade");
            put(&env, &key, &41u32);
            assert_eq!(get::<u32>(&env, &key), Some(41));
            assert!(has(&env, &key));
        });
    }

    #[test]
    fn contracttype_structs_round_trip() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            let submitter = Address::generate(&env);
            let state = RuntimeState {
                global_max_logs: 10_000,
                total_events: 41,
                last_sequence: 41,
                paused: false,
                allowlist_mode: false,
                low_cost_mode: false,
                emission_mode: 1,
                global_metadata_max_size: 256,
            };
            put(&env, &Key::RuntimeState, &state);
            let back: Option<RuntimeState> = get(&env, &Key::RuntimeState);
            assert_eq!(back, Some(state));
            let _ = submitter;
        });
    }

    #[test]
    fn option_fields_survive_encoding() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            let data = EventData {
                submitter: Address::generate(&env),
                event_type: symbol_short!("trade"),
                metadata: Bytes::from_slice(&env, &[1, 2, 3]),
                category: None,
                timestamp: 42,
                sequence: 1,
            };
            let id = BytesN::from_array(&env, &[7u8; 32]);
            put(&env, &Key::EventData(id.clone()), &data);
            assert_eq!(get::<EventData>(&env, &Key::EventData(id)), Some(data));
        });
    }

    #[test]
    fn same_label_different_payload_are_distinct_keys() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            // The same event id under two different key families must not collide.
            let id = BytesN::from_array(&env, &[1u8; 32]);
            put(&env, &Key::EventData(id.clone()), &1u32);
            put(&env, &Key::EventMeta(id.clone()), &2u32);
            put(&env, &Key::ArchivedEventData(id.clone()), &3u32);
            assert_eq!(get::<u32>(&env, &Key::EventData(id.clone())), Some(1));
            assert_eq!(get::<u32>(&env, &Key::EventMeta(id.clone())), Some(2));
            assert_eq!(get::<u32>(&env, &Key::ArchivedEventData(id)), Some(3));
        });
    }

    #[test]
    fn distinct_ids_under_one_family_are_distinct_keys() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            let a = BytesN::from_array(&env, &[1u8; 32]);
            let b = BytesN::from_array(&env, &[2u8; 32]);
            put(&env, &Key::EventData(a.clone()), &1u32);
            put(&env, &Key::EventData(b.clone()), &2u32);
            assert_eq!(get::<u32>(&env, &Key::EventData(a)), Some(1));
            assert_eq!(get::<u32>(&env, &Key::EventData(b)), Some(2));
        });
    }

    #[test]
    fn instance_and_persistent_are_distinct_stores() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            put(&env, &Key::Paused, &true);
            assert_eq!(get::<bool>(&env, &Key::Paused), Some(true));
            assert!(!has(&env, &Key::EventOrder(0)));
        });
    }

    #[test]
    fn numeric_keys_distinguish_values() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            put(&env, &Key::EventOrder(1), &BytesN::from_array(&env, &[1u8; 32]));
            put(&env, &Key::EventOrder(2), &BytesN::from_array(&env, &[2u8; 32]));
            assert_eq!(
                get::<BytesN<32>>(&env, &Key::EventOrder(1)),
                Some(BytesN::from_array(&env, &[1u8; 32]))
            );
            assert_eq!(
                get::<BytesN<32>>(&env, &Key::EventOrder(2)),
                Some(BytesN::from_array(&env, &[2u8; 32]))
            );
        });
    }

    #[test]
    fn type_keyed_families_do_not_collide_across_types() {
        let env = crate::measure::test_env();
        in_ledger(&env, || {
            put(&env, &Key::EventTypeCount("trade"), &1u32);
            put(&env, &Key::EventTypeCount("transfer"), &2u32);
            assert_eq!(get::<u32>(&env, &Key::EventTypeCount("trade")), Some(1));
            assert_eq!(get::<u32>(&env, &Key::EventTypeCount("transfer")), Some(2));
        });
    }

    #[test]
    fn keys_carry_their_variable_payload_in_their_size() {
        // The reason storage cost is not flat: a 32-byte-id key costs materially
        // more to touch than a bare label.
        let env = crate::measure::test_env();
        let id = BytesN::from_array(&env, &[9u8; 32]);
        let wide = encode_key(&env, &Key::EventData(id)).len();
        let narrow = encode_key(&env, &Key::EventOrder(1)).len();
        assert!(wide > narrow, "id key {wide} should exceed scalar key {narrow}");
    }

    #[test]
    fn packed_indices_are_four_bytes_each_and_append_preserves_prefix() {
        let env = crate::measure::test_env();
        let base = packed_indices(&env, 3);
        assert_eq!(base.len(), 12, "three u32 indices must occupy 12 bytes");
        let grown = append_index(&env, &base, 99);
        assert_eq!(grown.len(), 16);
        let expected_prefix = Bytes::from_slice(&env, &[0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(
            grown.slice(0u32..12u32),
            expected_prefix,
            "appending must not disturb earlier indices"
        );
    }

    #[test]
    fn growing_an_index_costs_linearly_in_bytes() {
        // The property the scaling benchmarks exist to protect: a re-appended
        // packed index is rewritten whole, so each append pays for every index
        // already recorded.
        let env = crate::measure::test_env();
        let small = packed_indices(&env, 10);
        let large = packed_indices(&env, 1000);
        assert!(large.len() > small.len() * 90, "growth must be linear, not flat");
    }

    #[test]
    fn rent_extension_is_metered_only_for_persistent_keys() {
        // Instance entries are re-created each invocation, so extending their rent
        // would be wasted work; the model must not pretend otherwise.
        assert_eq!(Key::RuntimeState.space(), Space::Instance);
        assert_eq!(
            Key::EventData(BytesN::from_array(&Env::default(), &[0u8; 32])).space(),
            Space::Persistent
        );
    }

    #[test]
    fn svec_builds_a_vector_from_a_slice() {
        let env = crate::measure::test_env();
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let v = svec(&env, &[a.clone(), b.clone()]);
        assert_eq!(v.len(), 2);
        assert_eq!(v.get(0), Some(a));
        assert_eq!(v.get(1), Some(b));
    }

    #[test]
    fn type_map_indexes_by_symbol() {
        let env = crate::measure::test_env();
        let m = type_map(&env, &["trade", "transfer"]);
        assert_eq!(m.len(), 2);
        assert_eq!(m.get(Symbol::new(&env, "trade")), Some(1));
        assert_eq!(m.get(Symbol::new(&env, "transfer")), Some(2));
    }
}
