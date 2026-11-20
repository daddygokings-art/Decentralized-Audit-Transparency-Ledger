//! Storage tiering, XDR footprint and TTL regression tests for `src/cqrs.rs`.
//!
//! The byte sizes pinned here are mirrored in
//! `scripts/storage-layout/snapshots/cqrs_footprint.json`, which the CI
//! storage-layout check (`scripts/storage-layout/validate_layout.py`) asserts
//! against its byte budgets. Update both together.
#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::testutils::storage::{Persistent as _, Temporary as _};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Test harness contract ────────────────────────────────────────────────────
//
// Drives `CqrsEngine` directly so the storage footprint measured below is the
// CQRS engine's alone (not the primary ledger's).

#[contract]
pub struct CqrsStorageHarness;

#[contractimpl]
impl CqrsStorageHarness {
    pub fn log(
        env: Env,
        id: BytesN<32>,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        idempotency_key: Option<BytesN<32>>,
    ) -> CommandResult {
        let cmd = LogEventCommand {
            submitter,
            event_type,
            metadata,
            category: None,
            sub_event_type: None,
            idempotency_key,
            force: true,
        };
        let ts = env.ledger().timestamp();
        CqrsEngine::record_log_event(&env, id, &cmd, ts)
    }

    pub fn snapshot(env: Env) -> u32 {
        CqrsEngine::create_snapshot(&env)
    }

    pub fn rebuild(env: Env, from_snapshot: bool) -> EventProjection {
        CqrsEngine::rebuild_projection(&env, from_snapshot)
    }

    pub fn extend_event(env: Env, sequence: u32) -> Result<(), CqrsError> {
        CqrsEngine::extend_event_ttl(&env, sequence)
    }

    pub fn extend_snap(env: Env, snapshot_id: u32) -> Result<(), CqrsError> {
        CqrsEngine::extend_snapshot_ttl(&env, snapshot_id)
    }
}

fn setup() -> (Env, Address, CqrsStorageHarnessClient<'static>) {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let id = env.register(CqrsStorageHarness, ());
    let client = CqrsStorageHarnessClient::new(&env, &id);
    (env, id, client)
}

fn event_id(env: &Env, i: u32) -> BytesN<32> {
    let mut b = [0u8; 32];
    b[..4].copy_from_slice(&i.to_be_bytes());
    BytesN::from_array(env, &b)
}

// ── Tier classification ──────────────────────────────────────────────────────

#[test]
fn every_storage_key_is_classified_into_its_retention_tier() {
    let env = Env::default();
    let id = event_id(&env, 1);
    let addr = Address::generate(&env);
    let ty = Symbol::new(&env, "audit");

    use CqrsStorageKey::*;
    use CqrsStorageTier::*;
    let expected = [
        (Seq, Instance),
        (Proj, Instance),
        (SnapCnt, Instance),
        (Event(1), Persistent),
        (IdSeq(id.clone()), Persistent),
        (Snap(1), Persistent),
        (TypeIdx(ty.clone(), 0), Persistent),
        (TypeCnt(ty), Persistent),
        (SubIdx(addr.clone(), 0), Persistent),
        (SubCnt(addr), Persistent),
        (Idem(id), Temporary),
    ];
    for (key, tier) in expected.iter() {
        assert_eq!(key.tier(), *tier, "{:?}", key);
    }
}

#[test]
fn records_are_written_to_their_classified_tier() {
    let (env, contract, client) = setup();
    let sub = Address::generate(&env);
    let ty = Symbol::new(&env, "audit");
    let idem = event_id(&env, 999);
    client.log(&event_id(&env, 1), &sub, &ty, &Bytes::new(&env), &Some(idem.clone()));

    env.as_contract(&contract, || {
        let st = env.storage();
        assert!(st.instance().has(&CqrsStorageKey::Seq));
        assert!(st.instance().has(&CqrsStorageKey::Proj));
        assert!(!st.instance().has(&CqrsStorageKey::Event(1)));
        assert!(st.persistent().has(&CqrsStorageKey::Event(1)));
        assert!(st.persistent().has(&CqrsStorageKey::IdSeq(event_id(&env, 1))));
        assert!(st.persistent().has(&CqrsStorageKey::TypeIdx(ty.clone(), 0)));
        assert!(st.persistent().has(&CqrsStorageKey::TypeCnt(ty.clone())));
        assert!(st.persistent().has(&CqrsStorageKey::SubIdx(sub.clone(), 0)));
        assert!(st.persistent().has(&CqrsStorageKey::SubCnt(sub.clone())));
        assert!(st.temporary().has(&CqrsStorageKey::Idem(idem.clone())));
        assert!(!st.instance().has(&CqrsStorageKey::Idem(idem)));
    });
}

// ── Serialized XDR footprint snapshots ───────────────────────────────────────

/// Pre-optimisation shape of `CqrsStoredEvent` (named-field `ScMap`, redundant
/// `sequence`, `Symbol` command type, zeroed governance ID), kept here only to
/// measure the reduction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyCqrsStoredEvent {
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

/// Pre-optimisation storage key shape (long variant names).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyCqrsStorageKey {
    EventStore(u32),
    EventIdToSequence(BytesN<32>),
    MaterializedSubmitterIndex(Address),
}

fn xdr_len<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, v: T) -> u32 {
    v.to_xdr(env).len()
}

fn sample_log_event(env: &Env) -> CqrsStoredEvent {
    CqrsStoredEvent {
        sequence: 42,
        event_id: Some(event_id(env, 42)),
        timestamp: 1_700_000_000,
        event_type: Symbol::new(env, "transfer"),
        submitter: Address::from_str(
            env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ),
        metadata: Bytes::from_slice(env, &[7u8; 64]),
        category: Some(Symbol::new(env, "finance")),
        sub_event_type: None,
        version: 1,
        kind: CqrsCommandKind::Log,
        index_pos: CqrsIndexPos::At(3, 9),
    }
}

fn legacy_of(env: &Env, e: &CqrsStoredEvent) -> LegacyCqrsStoredEvent {
    LegacyCqrsStoredEvent {
        sequence: e.sequence,
        event_id: e
            .event_id
            .clone()
            .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32])),
        timestamp: e.timestamp,
        event_type: e.event_type.clone(),
        submitter: e.submitter.clone(),
        metadata: e.metadata.clone(),
        category: e.category.clone(),
        sub_event_type: e.sub_event_type.clone(),
        version: e.version,
        command_type: e.kind.as_symbol(env),
    }
}

/// Pinned serialized sizes (bytes). Mirrored in
/// `scripts/storage-layout/snapshots/cqrs_footprint.json`.
const PINNED_LOG_EVENT_BYTES: u32 = 272;
const PINNED_LEGACY_LOG_EVENT_BYTES: u32 = 428;
const PINNED_GOV_EVENT_BYTES: u32 = 152;
const PINNED_LEGACY_GOV_EVENT_BYTES: u32 = 376;
const PINNED_EVENT_KEY_BYTES: u32 = 36;
const PINNED_LEGACY_EVENT_KEY_BYTES: u32 = 40;
const PINNED_SUB_IDX_KEY_BYTES: u32 = 80;
const PINNED_LEGACY_SUB_IDX_KEY_BYTES: u32 = 92;
const PINNED_EMPTY_PROJECTION_BYTES: u32 = 72;
const PINNED_LEGACY_EMPTY_PROJECTION_BYTES: u32 = 204;

#[test]
fn packed_event_record_xdr_size_is_pinned_and_smaller_than_legacy() {
    let env = Env::default();
    let ev = sample_log_event(&env);
    let packed = xdr_len(&env, ev.pack());
    let legacy = xdr_len(&env, legacy_of(&env, &ev));
    std::println!("log event: packed={packed} legacy={legacy}");
    assert_eq!(packed, PINNED_LOG_EVENT_BYTES);
    assert_eq!(legacy, PINNED_LEGACY_LOG_EVENT_BYTES);
    assert!(packed < legacy);

    let mut gov = ev.clone();
    gov.event_id = None;
    gov.metadata = Bytes::new(&env);
    gov.category = None;
    gov.kind = CqrsCommandKind::Governance;
    gov.index_pos = CqrsIndexPos::Unindexed;
    let mut legacy_gov = legacy_of(&env, &gov);
    legacy_gov.category = Some(Symbol::new(&env, "governance"));
    let packed_gov = xdr_len(&env, gov.pack());
    let legacy_gov = xdr_len(&env, legacy_gov);
    std::println!("gov event: packed={packed_gov} legacy={legacy_gov}");
    assert_eq!(packed_gov, PINNED_GOV_EVENT_BYTES);
    assert_eq!(legacy_gov, PINNED_LEGACY_GOV_EVENT_BYTES);
}

#[test]
fn storage_key_xdr_sizes_are_pinned_and_smaller_than_legacy() {
    let env = Env::default();
    let sub = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let ev_key = xdr_len(&env, CqrsStorageKey::Event(42));
    let legacy_ev_key = xdr_len(&env, LegacyCqrsStorageKey::EventStore(42));
    let sub_key = xdr_len(&env, CqrsStorageKey::SubIdx(sub.clone(), 0));
    let legacy_sub_key = xdr_len(&env, LegacyCqrsStorageKey::MaterializedSubmitterIndex(sub));
    std::println!("keys: event={ev_key}/{legacy_ev_key} sub_idx={sub_key}/{legacy_sub_key}");
    assert_eq!(ev_key, PINNED_EVENT_KEY_BYTES);
    assert_eq!(legacy_ev_key, PINNED_LEGACY_EVENT_KEY_BYTES);
    assert_eq!(sub_key, PINNED_SUB_IDX_KEY_BYTES);
    assert_eq!(legacy_sub_key, PINNED_LEGACY_SUB_IDX_KEY_BYTES);
}

#[test]
fn packed_projection_xdr_size_is_pinned_and_smaller_than_legacy() {
    let env = Env::default();
    let proj = EventProjection {
        last_sequence: 0,
        total_events: 0,
        total_commands: 0,
        type_counts: Vec::new(&env),
        submitter_counts: Vec::new(&env),
        last_updated: 0,
    };
    let packed = xdr_len(&env, proj.pack());
    let legacy = xdr_len(&env, proj);
    std::println!("projection: packed={packed} legacy={legacy}");
    assert_eq!(packed, PINNED_EMPTY_PROJECTION_BYTES);
    assert_eq!(legacy, PINNED_LEGACY_EMPTY_PROJECTION_BYTES);
}

#[test]
fn packed_records_round_trip() {
    let env = Env::default();
    let ev = sample_log_event(&env);
    assert_eq!(CqrsStoredEvent::unpack(ev.sequence, ev.pack()), ev);

    let mut tc = Vec::new(&env);
    tc.push_back((Symbol::new(&env, "a"), 3u32));
    let proj = EventProjection {
        last_sequence: 9,
        total_events: 3,
        total_commands: 4,
        type_counts: tc,
        submitter_counts: Vec::new(&env),
        last_updated: u64::MAX,
    };
    assert_eq!(EventProjection::unpack(proj.pack()), proj);
}

// ── TTL policy ───────────────────────────────────────────────────────────────

#[test]
fn writes_bump_persistent_records_to_policy_ttl() {
    let (env, contract, client) = setup();
    let sub = Address::generate(&env);
    client.log(&event_id(&env, 1), &sub, &Symbol::new(&env, "a"), &Bytes::new(&env), &None);
    env.as_contract(&contract, || {
        let ttl = env.storage().persistent().get_ttl(&CqrsStorageKey::Event(1));
        assert_eq!(ttl, CQRS_PERSISTENT_TTL_EXTEND_TO);
    });
}

#[test]
fn permissionless_extend_event_ttl_restores_ttl_for_event_and_indexes() {
    let (env, contract, client) = setup();
    let sub = Address::generate(&env);
    let ty = Symbol::new(&env, "a");
    client.log(&event_id(&env, 1), &sub, &ty, &Bytes::new(&env), &None);

    // Age the ledger until the record is below the bump threshold.
    let aged = CQRS_PERSISTENT_TTL_EXTEND_TO - CQRS_PERSISTENT_TTL_THRESHOLD + 10;
    env.ledger().with_mut(|l| l.sequence_number += aged);
    env.as_contract(&contract, || {
        assert!(
            env.storage().persistent().get_ttl(&CqrsStorageKey::Event(1))
                < CQRS_PERSISTENT_TTL_THRESHOLD
        );
    });

    // No auths are mocked: the helper must be callable by anyone.
    client.extend_event(&1);

    env.as_contract(&contract, || {
        let p = env.storage().persistent();
        for key in [
            CqrsStorageKey::Event(1),
            CqrsStorageKey::IdSeq(event_id(&env, 1)),
            CqrsStorageKey::TypeIdx(ty.clone(), 0),
            CqrsStorageKey::TypeCnt(ty.clone()),
            CqrsStorageKey::SubIdx(sub.clone(), 0),
            CqrsStorageKey::SubCnt(sub.clone()),
        ] {
            assert_eq!(p.get_ttl(&key), CQRS_PERSISTENT_TTL_EXTEND_TO, "{:?}", key);
        }
    });
}

#[test]
fn extend_ttl_helpers_return_typed_errors_for_missing_records() {
    let (_env, _contract, client) = setup();
    assert_eq!(client.try_extend_event(&7), Err(Ok(CqrsError::EventNotFound)));
    assert_eq!(client.try_extend_snap(&7), Err(Ok(CqrsError::SnapshotNotFound)));
}

#[test]
fn extend_snapshot_ttl_bumps_snapshot() {
    let (env, contract, client) = setup();
    let id = client.snapshot();
    let aged = CQRS_PERSISTENT_TTL_EXTEND_TO - CQRS_PERSISTENT_TTL_THRESHOLD + 10;
    env.ledger().with_mut(|l| l.sequence_number += aged);
    client.extend_snap(&id);
    env.as_contract(&contract, || {
        assert_eq!(
            env.storage().persistent().get_ttl(&CqrsStorageKey::Snap(id)),
            CQRS_PERSISTENT_TTL_EXTEND_TO
        );
    });
}

#[test]
fn idempotency_keys_live_in_temporary_storage_with_short_ttl() {
    let (env, contract, client) = setup();
    let idem = event_id(&env, 77);
    client.log(
        &event_id(&env, 1),
        &Address::generate(&env),
        &Symbol::new(&env, "a"),
        &Bytes::new(&env),
        &Some(idem.clone()),
    );
    env.as_contract(&contract, || {
        assert_eq!(
            env.storage().temporary().get_ttl(&CqrsStorageKey::Idem(idem.clone())),
            CQRS_IDEMPOTENCY_TTL_LEDGERS
        );
        assert!(CqrsEngine::get_idempotent_result(&env, idem.clone()).is_some());
    });
}

#[test]
fn rebuild_from_genesis_does_not_duplicate_materialized_indexes() {
    let (env, contract, client) = setup();
    let sub = Address::generate(&env);
    let ty = Symbol::new(&env, "a");
    for i in 1..=3 {
        client.log(&event_id(&env, i), &sub, &ty, &Bytes::new(&env), &None);
    }
    let proj = client.rebuild(&false);
    assert_eq!(proj.total_events, 3);
    env.as_contract(&contract, || {
        assert_eq!(CqrsEngine::get_materialized_type_count(&env, ty.clone()), 3);
        assert_eq!(CqrsEngine::get_materialized_submitter_count(&env, sub.clone()), 3);
    });
}

// ── 1,000 simulated transaction benchmark ────────────────────────────────────

/// Upper bound on ledger bytes written by a single `record_log_event` with a
/// 64-byte metadata payload. Before tiering, every event re-wrote the whole
/// instance entry (1.4 KB at tx 1, 62 KB at tx 100) and the 64 KiB instance
/// entry limit was exceeded at ~105 events.
const MAX_WRITE_BYTES_PER_LOG_TX: u32 = 2_048;

#[test]
fn bench_1000_simulated_transactions_have_constant_write_footprint() {
    let (env, _contract, client) = setup();
    let subs: std::vec::Vec<Address> = (0..10).map(|_| Address::generate(&env)).collect();
    let types = ["alpha", "beta", "gamma", "delta"];
    let meta = Bytes::from_slice(&env, &[7u8; 64]);

    let mut total_write: u64 = 0;
    let mut max_write: u32 = 0;
    let mut write_at_10: u32 = 0;
    for i in 0..1_000u32 {
        env.ledger().set_timestamp(1_000 + i as u64);
        client.log(
            &event_id(&env, i),
            &subs[(i % 10) as usize],
            &Symbol::new(&env, types[(i % 4) as usize]),
            &meta,
            &None,
        );
        let r = env.cost_estimate().resources();
        total_write += r.write_bytes as u64;
        max_write = max_write.max(r.write_bytes);
        if i == 9 {
            write_at_10 = r.write_bytes;
        }
        if i == 999 {
            // Steady state: the 1,000th write costs (almost) the same as the
            // 10th; only varint-free fixed-width counters change.
            assert!(
                r.write_bytes <= write_at_10 + 32,
                "write footprint must not grow with history: tx10={write_at_10} tx1000={}",
                r.write_bytes
            );
        }
    }
    std::println!(
        "cqrs bench: 1000 txs, total_write_bytes={total_write}, max_write_bytes={max_write}, avg={}",
        total_write / 1_000
    );
    assert!(max_write <= MAX_WRITE_BYTES_PER_LOG_TX, "max_write={max_write}");
    assert!(total_write <= 1_000 * MAX_WRITE_BYTES_PER_LOG_TX as u64);
}
