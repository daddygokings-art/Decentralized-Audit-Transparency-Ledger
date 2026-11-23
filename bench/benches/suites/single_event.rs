//! Scenario group: single event ingestion.
//!
//! Covers the write path of the contract from every angle that changes its cost
//! profile — metadata size, dedup behaviour, optional hierarchy, nonce, TTL
//! shadow copies, rate limiting, RBAC, schema validation and emission modes.

use audit_ledger::{DedupPolicy, Role};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Symbol, Vec};

use crate::support::fixture::Fixture;
use crate::support::measure::{Bench, CaseSpec};

/// Arguments for one `log_event` invocation.
type LogArgs = (Address, Symbol, Bytes, Option<Symbol>, Option<Symbol>, bool);

pub fn run(bench: &mut Bench) {
    // ── Metadata size sweep: the dominant cost lever for a single event ──────
    for len in [0u32, 64, 256, 1024] {
        let name = format!("log_event/meta_{len}b");
        bench.measure(
            CaseSpec::new("single_event", &name, "log_event").metadata(len),
            || {
                let fixture = Fixture::new();
                let args = plain_args(&fixture, len, 7);
                (fixture, args)
            },
            log_op,
        );
    }

    // ── Hierarchy: category + sub-type add two more symbols per event ───────
    bench.measure(
        CaseSpec::new("single_event", "log_event/hierarchy", "log_event")
            .metadata(64)
            .notes("category + sub_event_type set"),
        || {
            let fixture = Fixture::new();
            let args = (
                fixture.submitter.clone(),
                Symbol::new(&fixture.env, "transfer"),
                payload_bytes(&fixture, 64, 11),
                Some(Symbol::new(&fixture.env, "finance")),
                Some(Symbol::new(&fixture.env, "transfer")),
                false,
            );
            (fixture, args)
        },
        log_op,
    );

    // ── force = true bypasses the dedup index lookup and write ───────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/force_no_dedup", "log_event")
            .metadata(64)
            .notes("force = true"),
        || {
            let fixture = Fixture::new();
            let (submitter, event_type, metadata, category, sub_event_type, _) = plain_args(&fixture, 64, 13);
            (
                fixture,
                (submitter, event_type, metadata, category, sub_event_type, true),
            )
        },
        log_op,
    );

    // ── Dedup hit: identical payload short-circuits after the content hash ──
    bench.measure(
        CaseSpec::new("single_event", "log_event/dedup_hit", "log_event")
            .metadata(64)
            .notes("second write of an identical payload, force = false"),
        || {
            let fixture = Fixture::new();
            let args = plain_args(&fixture, 64, 17);
            let (submitter, event_type, metadata, category, sub_event_type, force) = &args;
            fixture
                .client
                .log_event(submitter, event_type, metadata, category, sub_event_type, force);
            (fixture, args)
        },
        log_op,
    );

    // ── Emission mode 3 suppresses the contract event write ──────────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/emission_mode_3", "log_event")
            .metadata(64)
            .notes("contract events suppressed"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            fixture.client.set_event_emission_mode(&owner, &3);
            let args = plain_args(&fixture, 64, 19);
            (fixture, args)
        },
        log_op,
    );

    // ── low_cost_mode skips the per-type and per-submitter indexes ───────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/low_cost_mode", "log_event")
            .metadata(64)
            .notes("index maintenance disabled"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            fixture.client.set_low_cost_mode(&owner, &true);
            let args = plain_args(&fixture, 64, 23);
            (fixture, args)
        },
        log_op,
    );

    // ── TTL configured: adds a persistent write plus a rent bump ────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/with_ttl", "log_event")
            .metadata(64)
            .notes("set_event_ttl(1000) active"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            fixture.client.set_event_ttl(&owner, &1_000);
            let args = plain_args(&fixture, 64, 29);
            (fixture, args)
        },
        log_op,
    );

    // ── Rate limiting adds two instance reads/writes per submission ─────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/with_rate_limit", "log_event")
            .metadata(64)
            .notes("submitter rate limit of 10 per timestamp"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            let submitter = fixture.submitter.clone();
            fixture.client.set_submitter_rate_limit(&owner, &submitter, &10);
            let args = plain_args(&fixture, 64, 31);
            (fixture, args)
        },
        log_op,
    );

    // ── RBAC role check on the write path ───────────────────────────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/rbac_enabled", "log_event")
            .metadata(64)
            .notes("RBAC on, submitter holds the Submitter role"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            let submitter = fixture.submitter.clone();
            fixture.client.enable_rbac(&owner, &true);
            fixture.client.set_role(&owner, &submitter, &Some(Role::Submitter));
            let args = plain_args(&fixture, 64, 37);
            (fixture, args)
        },
        log_op,
    );

    // ── Per-type schema validation adds a read on every write ───────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/with_schema", "log_event")
            .metadata(64)
            .notes("metadata schema registered for the event type"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            fixture
                .client
                .set_metadata_schema(&owner, &event_type, &Bytes::from_slice(&fixture.env, &[64, 0, 0, 0]));
            let args = (
                fixture.submitter.clone(),
                event_type,
                payload_bytes(&fixture, 64, 41),
                None,
                None,
                false,
            );
            (fixture, args)
        },
        log_op,
    );

    // ── Per-type event cap adds a count read and a limit check ──────────────
    bench.measure(
        CaseSpec::new("single_event", "log_event/with_type_cap", "log_event")
            .metadata(64)
            .notes("per-type cap of 1000 events"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            fixture.client.set_event_max_logs(&owner, &event_type, &1_000);
            let args = plain_args(&fixture, 64, 47);
            (fixture, args)
        },
        log_op,
    );

    // ── Extended API alias around the same core write path ──────────────────
    bench.measure(
        CaseSpec::new(
            "single_event",
            "log_event/with_hierarchy_alias",
            "log_event_with_hierarchy",
        )
        .metadata(64)
        .notes("extended API alias"),
        || {
            let fixture = Fixture::new();
            let args = plain_args(&fixture, 64, 53);
            (fixture, args)
        },
        |fixture, args, _| {
            let (submitter, event_type, metadata, category, sub_event_type, force) = args;
            let _ = fixture.client.log_event_with_hierarchy(
                submitter,
                event_type,
                metadata,
                category,
                sub_event_type,
                force,
            );
            1
        },
    );

    bench.measure(
        CaseSpec::new("single_event", "log_event/with_nonce", "log_event_with_nonce")
            .metadata(64)
            .notes("replay-protected write path"),
        || {
            let fixture = Fixture::new();
            let submitter = fixture.submitter.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            let metadata = payload_bytes(&fixture, 64, 59);
            (fixture, (submitter, event_type, metadata))
        },
        |fixture, args, iteration| {
            let (submitter, event_type, metadata) = args;
            let _ = fixture
                .client
                .log_event_with_nonce(submitter, event_type, metadata, &(iteration as u32 + 1));
            1
        },
    );

    bench.measure(
        CaseSpec::new("single_event", "log_event/signed", "log_event_signed")
            .metadata(64)
            .notes("96-byte Ed25519 signature payload stored on-chain"),
        || {
            let fixture = Fixture::new();
            let submitter = fixture.submitter.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            let metadata = payload_bytes(&fixture, 64, 61);
            let signature = payload_bytes(&fixture, 96, 67);
            (fixture, (submitter, event_type, metadata, signature))
        },
        |fixture, args, _| {
            let (submitter, event_type, metadata, signature) = args;
            let _ = fixture
                .client
                .log_event_signed(submitter, event_type, metadata, signature);
            1
        },
    );

    bench.measure(
        CaseSpec::new("single_event", "get_event_signature/lookup", "get_event_signature")
            .metadata(64)
            .notes("profile read gas for stored 96-byte event signature"),
        || {
            let fixture = Fixture::new();
            let submitter = fixture.submitter.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            let metadata = payload_bytes(&fixture, 64, 63);
            let signature = payload_bytes(&fixture, 96, 69);
            let event_id = fixture
                .client
                .log_event_signed(&submitter, &event_type, &metadata, &signature);
            (fixture, event_id)
        },
        |fixture, event_id, _| {
            let _ = fixture.client.get_event_signature(event_id);
            1
        },
    );

    bench.measure(
        CaseSpec::new("single_event", "log_event/with_custom_key", "log_event_with_custom_key")
            .metadata(64)
            .notes("caller-supplied dedup key under DedupPolicy::Custom"),
        || {
            let fixture = Fixture::new();
            let owner = fixture.owner.clone();
            fixture.client.set_dedup_policy(&owner, &DedupPolicy::Custom);
            let submitter = fixture.submitter.clone();
            let event_type = Symbol::new(&fixture.env, "transfer");
            let metadata = payload_bytes(&fixture, 64, 71);
            (fixture, (submitter, event_type, metadata))
        },
        |fixture, args, _| {
            let (submitter, event_type, metadata) = args;
            let mut raw = [0u8; 32];
            raw[0..4].copy_from_slice(&(metadata.len() + 1).to_be_bytes());
            raw[8..12].copy_from_slice(&1u32.to_be_bytes());
            let key = BytesN::from_array(&fixture.env, &raw);
            let _ = fixture.client.log_event_with_custom_key(
                submitter,
                event_type,
                metadata,
                &None,
                &None,
                &false,
                &Some(key),
            );
            1
        },
    );

    // ── Deployment cost: initialize writes the whole configuration surface ──
    for owners in [1u32, 5] {
        let name = format!("initialize/{owners}_owner(s)");
        let spec = CaseSpec::new("single_event", &name, "initialize");
        bench.measure(
            spec,
            move || (Fixture::uninitialized(), owners),
            |fixture, owners, _| {
                let mut owner_list = Vec::new(&fixture.env);
                owner_list.push_back(fixture.owner.clone());
                for _ in 1..*owners {
                    owner_list.push_back(Address::generate(&fixture.env));
                }
                fixture
                    .client
                    .initialize(&owner_list, &crate::suites::DEFAULT_MAX_EVENTS, &4096);
                *owners as u64
            },
        );
    }
}

/// Build the argument tuple for a plain `log_event` call.
fn plain_args(fixture: &Fixture, metadata_len: u32, seed: u32) -> LogArgs {
    (
        fixture.submitter.clone(),
        Symbol::new(&fixture.env, "transfer"),
        payload_bytes(fixture, metadata_len as usize, seed),
        None,
        None,
        false,
    )
}

/// Deterministic `Bytes` payload of `len` bytes for seed `seed`.
fn payload_bytes(fixture: &Fixture, len: usize, seed: u32) -> Bytes {
    let data: std::vec::Vec<u8> = (0..len).map(|i| (seed as u8).wrapping_add(i as u8)).collect();
    Bytes::from_slice(&fixture.env, &data)
}

/// Shared `log_event` invocation used by most single-event cases.
fn log_op(fixture: &Fixture, args: &LogArgs, _iteration: usize) -> u64 {
    let (submitter, event_type, metadata, category, sub_event_type, force) = args;
    let _ = fixture
        .client
        .log_event(submitter, event_type, metadata, category, sub_event_type, force);
    1
}
