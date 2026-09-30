//! The five benchmark scenarios the suite is required to cover.
//!
//! Each scenario is the host work of one real contract operation, measured in a
//! real `Env` against a pre-seeded ledger. The scenarios are deliberately named
//! after the contract's own workflows so a regression can be traced back to a
//! feature rather than to a number in a table.
//!
//! Ledger size is a parameter rather than a constant, because the contract's cost
//! is not constant in it: the packed per-type and per-submitter index arrays are
//! rewritten in full on every append, so an append gets more expensive as the
//! ledger fills. Every write-path scenario is therefore measured at several
//! ledger sizes, and the growth is reported as its own metric.

use crate::contract::{
    self as model, EventData, EventHeader, EventLogged, EventMeta, GovernanceProposed, Key, RateState, RuntimeState,
};
use crate::measure::{fixture, Builder, Measurement};
use soroban_sdk::{testutils::Address as _, vec, Address, Bytes, BytesN, Env, Symbol};
use std::collections::BTreeMap;

/// The largest fixture the harness can build.
///
/// A test `Env` meters against the mainnet limits and its 40 MiB memory budget
/// covers the *whole* environment, not one invocation — there is no public way
/// to widen it. Seeded events cost roughly 50 KB of metered memory each, so a
/// fixture runs out of budget somewhere around eight hundred events and the
/// failure surfaces as an opaque limit abort part-way through seeding.
///
/// 400 is the largest size that leaves real headroom — a single append against
/// a 700-event fixture still fits, but a batched append against a 500-event one
/// does not — and
/// `the_declared_fixture_cap_fits_in_the_host_budget` measures it rather than
/// trusting the arithmetic.
pub const MAX_FIXTURE_EVENTS: u32 = 400;

/// Ledger sizes every read-path scenario is measured at.
///
/// Reads are the case where the size of the ledger behind the call is the thing
/// being measured, so these go as large as the harness can build.
pub const LEDGER_SIZES: [u32; 3] = [0, 100, MAX_FIXTURE_EVENTS];

/// Ledger sizes every write-path scenario is measured at.
///
/// Smaller than [`LEDGER_SIZES`], and bounded by measurement rather than by
/// taste: the largest write workload is a sixteen-event batch, and a batch of
/// that length on a 400-event fixture exceeds the host's 40 MiB budget for the
/// whole environment. 300 fits at about 34 MiB.
///
/// The ceiling is worth stating plainly, because it is the more useful finding:
/// at 300 events a sixteen-event batch already holds roughly 85% of the
/// network's per-invocation memory. Batch length, not ledger size, is what
/// bounds a write on this contract.
pub const WRITE_LEDGER_SIZES: [u32; 2] = [100, 300];

/// Metadata payload size, in bytes, for a typical submission.
///
/// Mirrors the contract's default metadata cap; a realistic payload rather than
/// an empty one, because metadata is stored inside `EventData` and so directly
/// scales write bytes.
pub const METADATA_BYTES: u32 = 256;

/// Rent window the contract extends entries to.
pub const RENT_EXTEND_TO: u32 = 2_592_000;

/// The event types the seeded ledger uses.
pub const EVENT_TYPES: [&str; 3] = ["trade", "transfer", "compliance"];

/// Batch sizes the batched-logging scenario is measured at.
///
/// Bounded by the network's per-invocation write limit rather than chosen for
/// convenience: an append touches ten ledger entries and a Soroban invocation
/// may write at most 200, so a batch larger than about sixteen could never
/// succeed on a real network. Measuring 100 would have produced a number for a
/// transaction that cannot be submitted, so the suite measures the largest
/// batches that can actually be sent.
pub const BATCH_SIZES: [u32; 3] = [4, 8, 16];

/// One named, measured workload in the suite.
pub struct Case {
    pub id: String,
    pub group: String,
    pub name: String,
    /// Parameters this case was built with, carried into the result file so a
    /// figure is never separated from the size it was measured at.
    params: BTreeMap<String, String>,
    builder: Builder,
}

/// Build a case's parameter map from `key=value` pairs.
fn params(pairs: &[(&str, u32)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

impl Case {
    /// Assemble a case. Used by this module and by the storage primitives.
    pub fn new(
        id: impl Into<String>,
        group: &str,
        name: impl Into<String>,
        params: BTreeMap<String, String>,
        builder: Builder,
    ) -> Self {
        Self {
            id: id.into(),
            group: group.into(),
            name: name.into(),
            params,
            builder,
        }
    }

    pub fn params(&self) -> BTreeMap<String, String> {
        self.params.clone()
    }

    /// Measure this case on a freshly seeded ledger.
    pub fn measure(&self) -> Measurement {
        self.builder.scenario().measure()
    }

    /// Measure twice and confirm the host reports identical numbers.
    ///
    /// Cheap enough to do on every case, and it is what makes a 5% threshold
    /// trustworthy: if a scenario's own result moved by more than the threshold,
    /// every run would be a coin flip and the gate would be noise.
    pub fn assert_repeatable(&self) -> Measurement {
        let first = self.measure();
        assert_eq!(self.measure(), first, "{} is not repeatable", self.id);
        first
    }
}

/// Deterministic event id for a sequence number.
///
/// The contract derives ids by hashing, which is real work this suite does not
/// measure. A distinct id per sequence is all the storage layer needs, and using a
/// fixed pattern keeps repeated runs identical.
fn event_id(env: &Env, seq: u32) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[..4].copy_from_slice(&seq.to_le_bytes());
    BytesN::from_array(env, &raw)
}

fn metadata(env: &Env, seed: u32, len: u32) -> Bytes {
    let mut b = Bytes::new(env);
    let mut written = 0u32;
    let seed = seed as u8;
    let mut i = 0u8;
    while written < len {
        let chunk = (len - written).min(32);
        let mut block = [0u8; 32];
        for (n, slot) in block.iter_mut().enumerate().take(chunk as usize) {
            *slot = seed.wrapping_add(i).wrapping_add(n as u8);
        }
        b.append(&Bytes::from_slice(env, &block[..chunk as usize]));
        written += chunk;
        i = i.wrapping_add(1);
    }
    b
}

/// Events written per frame while seeding.
///
/// `soroban-sdk 27`'s host enforces the network's per-invocation write limits on
/// work done inside a contract frame, and an append touches ten entries, so a
/// frame that writes more than about sixteen events would fail on
/// `ExceededLimit` rather than on anything about the contract. Seeding is
/// therefore split into slices that each fit inside one invocation's budget.
const SEED_CHUNK: u32 = 16;

/// Seed a ledger with `size` events across [`EVENT_TYPES`] and `submitters`.
///
/// The shape is what matters for cost, not the values: one primary record, one
/// order entry, one header, one index row, one metadata blob, and two packed
/// index arrays that are rewritten on every subsequent append.
///
/// `contract` is the contract the scenario runs as. Seeding happens in
/// network-legal frames (see [`SEED_CHUNK`]) and is excluded from every
/// measurement; the caller wraps only the workload itself.
pub fn seed(env: &Env, contract: &Address, size: u32) -> SeedInfo {
    assert!(
        size <= MAX_FIXTURE_EVENTS,
        "a fixture of {size} events exceeds the harness ceiling of {MAX_FIXTURE_EVENTS}; \
         the test host's 40 MiB metered memory budget cannot hold it"
    );
    let submitters: std::vec::Vec<Address> = (0..8).map(|_| Address::generate(env)).collect();
    let timestamp = 1_700_000_000u64;

    fixture(env, contract, || {
        model::put(
            env,
            &Key::RuntimeState,
            &RuntimeState {
                global_max_logs: 10_000_000,
                total_events: 0,
                last_sequence: 0,
                paused: false,
                allowlist_mode: false,
                low_cost_mode: false,
                emission_mode: 1,
                global_metadata_max_size: METADATA_BYTES,
            },
        );
        model::put(env, &Key::Paused, &false);
        model::put(env, &Key::AllowlistMode, &false);
        model::put(env, &Key::LowCostMode, &false);
        model::put(env, &Key::EventEmissionConfig, &1u32);
    });

    // Each event is a full append, so the fixture grows the same way the real
    // ledger does: the per-type and per-submitter index arrays are rewritten
    // whole every time, which is what makes them expensive at larger sizes.
    let mut seq = 0u32;
    while seq < size {
        let end = (seq + SEED_CHUNK).min(size);
        let (lo, hi) = (seq, end);
        fixture(env, contract, || {
            for s in lo..hi {
                seed_event(env, &submitters, timestamp, s);
            }
        });
        seq = end;
    }

    fixture(env, contract, || {
        // The packed indices must already hold one entry per seeded event, not
        // one per type, so that a later append rewrites a realistically large
        // array.
        for (t_index, event_type) in EVENT_TYPES.iter().enumerate() {
            let count = count_of_type(size, t_index);
            model::put(
                env,
                &Key::EventTypeIndices(event_type),
                &model::packed_indices(env, count),
            );
            model::put(env, &Key::EventTypeCount(event_type), &count);
        }
        for (s_index, submitter) in submitters.iter().enumerate() {
            let count = count_of_submitter(size, s_index, submitters.len() as u32);
            model::put(
                env,
                &Key::SubmitterEventIndices(submitter.clone()),
                &model::packed_indices(env, count),
            );
            model::put(env, &Key::SubmitterEventCount(submitter.clone()), &count);
            model::put(
                env,
                &Key::SubmitterRateState(submitter.clone()),
                &RateState {
                    window_start: timestamp,
                    count: 0,
                },
            );
            model::put(env, &Key::SubmitterNonce(submitter.clone()), &(size.saturating_sub(1)));
        }
        model::put(env, &Key::ArchivedTotalEvents, &0u32);
    });

    SeedInfo {
        contract: contract.clone(),
        submitters,
        types: &EVENT_TYPES,
        size,
        timestamp,
    }
}

/// One seeded event's worth of entries.
fn seed_event(env: &Env, submitters: &[Address], timestamp: u64, seq: u32) {
    let submitter = submitters[(seq as usize) % submitters.len()].clone();
    let name = EVENT_TYPES[(seq as usize) % EVENT_TYPES.len()];
    let event_type = Symbol::new(env, name);
    let id = event_id(env, seq);

    model::put(
        env,
        &Key::EventData(id.clone()),
        &EventData {
            submitter: submitter.clone(),
            event_type: event_type.clone(),
            metadata: metadata(env, seq, METADATA_BYTES),
            category: None,
            timestamp: timestamp + u64::from(seq),
            sequence: seq,
        },
    );
    model::put(env, &Key::EventOrder(seq), &id);
    model::put(
        env,
        &Key::EventHeader(id.clone()),
        &EventHeader {
            index: seq,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
            timestamp: timestamp + u64::from(seq),
        },
    );
    model::put(
        env,
        &Key::EventMeta(id.clone()),
        &EventMeta {
            index: seq,
            timestamp: timestamp + u64::from(seq),
            event_type,
            submitter,
        },
    );
    model::put(
        env,
        &Key::EventMetadata(id),
        &Bytes::from_slice(env, &seq.to_le_bytes()),
    );
}

/// Number of seeded events carrying the type at `t_index`.
fn count_of_type(size: u32, t_index: usize) -> u32 {
    let types = EVENT_TYPES.len() as u32;
    let base = size / types;
    let remainder = size % types;
    base + u32::from((t_index as u32) < remainder)
}

/// Number of seeded events submitted by the submitter at `s_index`.
fn count_of_submitter(size: u32, s_index: usize, submitters: u32) -> u32 {
    let base = size / submitters;
    let remainder = size % submitters;
    base + u32::from((s_index as u32) < remainder)
}

/// The seeded state a scenario runs against.
pub struct SeedInfo {
    pub contract: Address,
    pub submitters: std::vec::Vec<Address>,
    /// Event type names, used for per-type keys and converted to `Symbol`
    /// only where a stored value needs one.
    pub types: &'static [&'static str],
    pub size: u32,
    pub timestamp: u64,
}

/// One append, as the contract's append path performs it.
///
/// This is the shared core of `single_event` and `batch_events`, kept as one
/// function so the two scenarios cannot drift apart and so a per-event figure
/// from the batch case is comparable with the single case.
///
/// `name` is the event type's name, which is what keys on it; the `Symbol` the
/// contract stores is derived from it, so the two can never disagree.
pub fn append_event(env: &Env, seed: &SeedInfo, seq: u32, submitter: &Address, name: &'static str) {
    let event_type = Symbol::new(env, name);
    let id = event_id(env, seq);
    let timestamp = seed.timestamp + u64::from(seq);

    // The contract caches global state in one instance entry precisely so the
    // common path does not read seven.
    let mut state: RuntimeState = model::get(env, &Key::RuntimeState).unwrap_or(RuntimeState {
        global_max_logs: 10_000_000,
        total_events: seq,
        last_sequence: seq,
        paused: false,
        allowlist_mode: false,
        low_cost_mode: false,
        emission_mode: 1,
        global_metadata_max_size: METADATA_BYTES,
    });
    state.total_events = seq + 1;
    state.last_sequence = seq;

    model::put(
        env,
        &Key::EventData(id.clone()),
        &EventData {
            submitter: submitter.clone(),
            event_type: event_type.clone(),
            metadata: metadata(env, seq, METADATA_BYTES),
            category: None,
            timestamp,
            sequence: seq,
        },
    );
    model::put(env, &Key::EventOrder(seq), &id);
    model::put(
        env,
        &Key::EventHeader(id.clone()),
        &EventHeader {
            index: seq,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
            timestamp,
        },
    );
    model::put(
        env,
        &Key::EventMeta(id.clone()),
        &EventMeta {
            index: seq,
            timestamp,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
        },
    );
    model::put(
        env,
        &Key::EventMetadata(id.clone()),
        &Bytes::from_slice(env, &seq.to_le_bytes()),
    );

    // Rewritten in full: this is the operation whose cost grows with the ledger.
    let type_indices: Bytes = model::get(env, &Key::EventTypeIndices(name)).unwrap_or_else(|| Bytes::new(env));
    model::put(
        env,
        &Key::EventTypeIndices(name),
        &model::append_index(env, &type_indices, seq),
    );
    let submitter_indices: Bytes =
        model::get(env, &Key::SubmitterEventIndices(submitter.clone())).unwrap_or_else(|| Bytes::new(env));
    model::put(
        env,
        &Key::SubmitterEventIndices(submitter.clone()),
        &model::append_index(env, &submitter_indices, seq),
    );

    let mut rate: RateState = model::get(env, &Key::SubmitterRateState(submitter.clone())).unwrap_or(RateState {
        window_start: seed.timestamp,
        count: 0,
    });
    rate.count += 1;
    model::put(env, &Key::SubmitterRateState(submitter.clone()), &rate);

    let nonce: u32 = model::get(env, &Key::SubmitterNonce(submitter.clone())).unwrap_or(seq);
    model::put(env, &Key::SubmitterNonce(submitter.clone()), &(nonce + 1));

    model::put(env, &Key::RuntimeState, &state);

    for key in [
        Key::EventData(id.clone()),
        Key::EventOrder(seq),
        Key::EventHeader(id.clone()),
        Key::EventMeta(id.clone()),
        Key::EventMetadata(id.clone()),
    ] {
        model::bump_rent(env, &key, RENT_EXTEND_TO);
    }

    EventLogged {
        event_type: event_type.clone(),
        submitter: submitter.clone(),
        metadata: metadata(env, seq, 64),
        sequence: seq,
    }
    .publish(env);
}

/// Build one event-logging case at the given ledger size.
pub fn single_event(size: u32) -> Case {
    let id = format!("scenario:single_event/ledger={size}");
    let builder = Builder::new(id.clone(), move |env, contract| {
        let seed = seed(env, contract, size);
        let submitter = seed.submitters[0].clone();
        let name = seed.types[0];
        Box::new(move |env: &Env| append_event(env, &seed, size, &submitter, name))
    });
    Case::new(id, "scenario", "single_event", params(&[("ledger", size)]), builder)
}

/// Build one batched logging case.
pub fn batch_events(ledger: u32, batch: u32) -> Case {
    let id = format!("scenario:batch_events/ledger={ledger}/batch={batch}");
    let builder = Builder::new(id.clone(), move |env, contract| {
        let seed = seed(env, contract, ledger);
        Box::new(move |env: &Env| {
            for i in 0..batch {
                let submitter = seed.submitters[(i as usize) % seed.submitters.len()].clone();
                let name = seed.types[(i as usize) % seed.types.len()];
                append_event(env, &seed, ledger + i, &submitter, name);
            }
        })
    });
    Case::new(
        id,
        "scenario",
        "batch_events",
        params(&[("ledger", ledger), ("batch", batch)]),
        builder,
    )
}

/// Build one read-path case: existence check, single fetch, index walk, range scan.
pub fn queries(ledger: u32) -> Case {
    let id = format!("scenario:queries/ledger={ledger}");
    let builder = Builder::new(id.clone(), move |env, contract| {
        let seed = seed(env, contract, ledger);
        Box::new(move |env: &Env| {
            if ledger == 0 {
                return;
            }
            let probe = ledger / 2;
            let _ = model::has(env, &Key::EventData(event_id(env, probe)));
            if let Some(id) = model::get::<BytesN<32>>(env, &Key::EventOrder(probe)) {
                let _: Option<EventData> = model::get(env, &Key::EventData(id.clone()));
                let _: Option<EventHeader> = model::get(env, &Key::EventHeader(id.clone()));
                let _: Option<EventMeta> = model::get(env, &Key::EventMeta(id.clone()));
            }
            // Walking the order index is the shape a paginated query takes.
            for seq in 0..8u32.min(ledger) {
                let _: Option<BytesN<32>> = model::get(env, &Key::EventOrder(seq));
            }
            // The packed index is read whole to answer a per-type listing.
            if let Some(indices) = model::get::<Bytes>(env, &Key::EventTypeIndices(seed.types[0])) {
                let _ = indices.slice(0u32..indices.len().min(4 * 8));
            }
        })
    });
    Case::new(id, "scenario", "queries", params(&[("ledger", ledger)]), builder)
}

/// Build one governance case: propose, vote, tally, and the owner set reads.
pub fn governance(ledger: u32, voters: u32) -> Case {
    let id = format!("scenario:governance/ledger={ledger}/voters={voters}");
    let builder = Builder::new(id.clone(), move |env, contract| {
        let seed = seed(env, contract, ledger);
        fixture(env, contract, || {
            model::put(env, &Key::Owners, &vec![&env, seed.submitters[0].clone()]);
            model::put(env, &Key::RequiredSignatures, &1u32);
            model::put(env, &Key::ProposalCount, &0u32);
        });
        Box::new(move |env: &Env| {
            let proposal = 1u32;
            let proposer = seed.submitters[0].clone();
            GovernanceProposed {
                proposal_id: proposal,
                proposer,
            }
            .publish(env);
            // Voting is dominated by one write plus a read of the tally, which
            // is re-read and rewritten per voter: O(voters) entry writes.
            for i in 0..voters {
                let submitter = seed.submitters[(i as usize) % seed.submitters.len()].clone();
                let tally: u32 = model::get(env, &Key::ProposalCount).unwrap_or(0);
                model::put(env, &Key::ProposalCount, &(tally + 1));
                model::bump_rent(env, &Key::ProposalCount, RENT_EXTEND_TO);
                // The owner set is a list, and an owner check reads the whole
                // list — reading it back as a single address would not compile
                // against the type that was written.
                let _ = model::get::<Option<soroban_sdk::Vec<Address>>>(env, &Key::Owners);
                let _ = model::get::<u32>(env, &Key::RequiredSignatures);
                model::put(
                    env,
                    &Key::SubmitterRateState(submitter),
                    &RateState {
                        window_start: seed.timestamp,
                        count: i,
                    },
                );
            }
        })
    });
    Case::new(
        id,
        "scenario",
        "governance",
        params(&[("ledger", ledger), ("voters", voters)]),
        builder,
    )
}

/// Build one archival case: mark, copy to the archived space, bump the counters.
pub fn archive(ledger: u32, count: u32) -> Case {
    let id = format!("scenario:archive/ledger={ledger}/count={count}");
    let builder = Builder::new(id.clone(), move |env, contract| {
        // Seeded but not read: the archive path works from keys it can already
        // derive, which is exactly why it is cheaper than an append.
        seed(env, contract, ledger);
        Box::new(move |env: &Env| {
            for i in 0..count {
                let seq = ledger.saturating_sub(1).saturating_sub(i);
                let id = event_id(env, seq);
                let data: Option<EventData> = model::get(env, &Key::EventData(id.clone()));
                let header: Option<EventHeader> = model::get(env, &Key::EventHeader(id.clone()));
                let archived_total: u32 = model::get(env, &Key::ArchivedTotalEvents).unwrap_or(0);
                model::put(env, &Key::ArchivedTotalEvents, &(archived_total + 1));
                if let Some(data) = data {
                    model::put(env, &Key::ArchivedEventData(id.clone()), &data);
                    model::bump_rent(env, &Key::ArchivedEventData(id.clone()), RENT_EXTEND_TO);
                }
                if let Some(header) = header {
                    model::put(env, &Key::EventHeader(id.clone()), &header);
                }
                model::put(env, &Key::EventArchivedFlag(id.clone()), &true);
                model::bump_rent(env, &Key::EventArchivedFlag(id.clone()), RENT_EXTEND_TO);
            }
        })
    });
    Case::new(
        id,
        "scenario",
        "archive",
        params(&[("ledger", ledger), ("count", count)]),
        builder,
    )
}

/// Every scenario in the suite, across the required sizes.
pub fn all() -> Vec<Case> {
    let mut cases = Vec::new();
    for size in WRITE_LEDGER_SIZES {
        cases.push(single_event(size));
    }
    for size in WRITE_LEDGER_SIZES {
        for batch in BATCH_SIZES {
            cases.push(batch_events(size, batch));
        }
    }
    for size in LEDGER_SIZES {
        cases.push(queries(size));
    }
    for size in WRITE_LEDGER_SIZES {
        cases.push(governance(size, 5));
    }
    for size in WRITE_LEDGER_SIZES {
        cases.push(archive(size, 25));
    }
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_plausible(m: &Measurement, label: &str) {
        assert!(m.instructions > 0, "{label}: no instructions: {m:?}");
        assert!(m.write_entries > 0, "{label}: no writes: {m:?}");
        assert!(m.write_bytes > 0, "{label}: no write bytes: {m:?}");
    }

    #[test]
    fn single_event_writes_the_expected_entries() {
        let m = single_event(100).measure();
        assert_plausible(&m, "single_event");
        // Six records written, plus two counters: the order entry, primary
        // record, header, index row, metadata blob, the flag-free append, the
        // two packed indices, rate state, nonce, and cached runtime state.
        assert!(m.write_entries >= 9, "append must write every index: {m:?}");
    }

    #[test]
    fn queries_read_without_writing() {
        let m = queries(MAX_FIXTURE_EVENTS).measure();
        assert!(m.read_entries > 0, "query must read: {m:?}");
        assert_eq!(m.write_entries, 0, "a query must not write: {m:?}");
    }

    #[test]
    fn queries_at_an_empty_ledger_read_nothing_but_the_contract() {
        // Guards the scenario, not the contract: with nothing seeded there is
        // nothing to read. The one entry that is read is the contract instance
        // the frame is entered through, which every measurement pays and which
        // is not this scenario's doing — so the assertion is that reads do not
        // scale with the fixture, not that they are zero.
        let empty = queries(0).measure();
        let seeded = queries(LEDGER_SIZES[2]).measure();
        assert_eq!(empty.write_entries, 0, "a query must not write: {empty:?}");
        assert!(
            empty.read_entries <= 1,
            "an empty ledger should read only the contract instance: {empty:?}"
        );
        assert!(
            seeded.read_entries > empty.read_entries,
            "a seeded ledger must read more than an empty one: {empty:?} vs {seeded:?}"
        );
    }

    #[test]
    fn batch_cost_is_proportional_to_batch_size() {
        // Sizes come from the declared constant so the test cannot drift into a
        // batch the network would reject.
        let small = batch_events(100, 1).measure();
        let mid = batch_events(100, BATCH_SIZES[1]).measure();
        let large = batch_events(100, BATCH_SIZES[2]).measure();
        assert!(
            small.instructions < mid.instructions && mid.instructions < large.instructions,
            "batch cost must grow with batch size: {small:?} {mid:?} {large:?}"
        );
    }

    #[test]
    fn append_cost_grows_with_ledger_size() {
        // The property that makes the packed indices worth monitoring: a full
        // rewrite of a longer index array costs more, so a per-append regression
        // is a scaling problem, not just a constant-factor one.
        let small = single_event(100).measure();
        let large = single_event(WRITE_LEDGER_SIZES[1]).measure();
        assert!(
            large.write_bytes > small.write_bytes,
            "appending at the fixture cap must write more than at 100: {small:?} vs {large:?}"
        );
    }

    #[test]
    fn governance_and_archive_both_write() {
        assert_plausible(&governance(100, 5).measure(), "governance");
        assert_plausible(&archive(WRITE_LEDGER_SIZES[1], 25).measure(), "archive");
    }

    #[test]
    fn every_scenario_is_repeatable() {
        for case in all() {
            case.assert_repeatable();
        }
    }

    #[test]
    fn seeding_costs_nothing_measurable() {
        // The whole validity of these numbers rests on the fixture being excluded
        // from the measurement. If seeding leaked in, the figure for an append
        // would grow in proportion to the number of events already in the ledger
        // — three hundred times as many events, three hundred times the cost.
        //
        // It does grow with ledger size, and legitimately: an append rewrites the
        // whole packed index for its event type, so the bigger the ledger the
        // more index there is to rewrite. That is the scaling property this suite
        // exists to expose. What must not happen is growth anywhere near
        // proportional to the fixture, so the bound is loose enough to allow the
        // index rewrite and far too tight for a hundred leaked appends.
        let empty = single_event(0).measure();
        let seeded = single_event(WRITE_LEDGER_SIZES[1]).measure();
        let events = f64::from(WRITE_LEDGER_SIZES[1]);
        let growth = seeded.instructions as f64 / empty.instructions as f64;
        assert!(
            growth < events / 4.0,
            "an append must cost far less than the fixture it is measured on: \
             {growth:.1}x for {events:.0} seeded events, {empty:?} vs {seeded:?}"
        );
    }
}
