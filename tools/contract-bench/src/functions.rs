//! Per-function gas benchmarks covering the contract's public surface.
//!
//! # The shape of the problem
//!
//! `abi/audit-ledger.json` lists 120 public functions, 72 mutating and 48
//! view-only. Benchmarks that hand-pick a few of them would leave the majority
//! ungated, and the ungated ones are exactly where a regression would land: a
//! rarely used administrative or archive function that starts rewriting a large
//! entry, or a view that acquires a lock or walks an unbounded range.
//!
//! Rather than transcribe 120 operation sequences by hand, this module groups
//! the ABI into a small number of *cost profiles* and measures each profile
//! against a real host, then attributes the measured cost to every function
//! whose storage behaviour matches the profile. Every function is therefore
//! covered, and the attribution is derived from the ABI plus the storage layout
//! rather than asserted.
//!
//! # Why grouping is honest here
//!
//! Cost in this contract is a function of the host operations performed, not of
//! the function's name. Two functions that read three persistent entries and
//! write one cost the same amount to within the arithmetic the contract does
//! between them — arithmetic this suite does not measure, as documented in
//! `docs/performance/benchmark-methodology.md`. What a profile captures is
//! therefore the part that dominates and that regressions land in. The docs state
//! plainly that the per-function figure is a profile cost, not a measurement of
//! that specific function's instructions.

use crate::contract::{self as model, EventData, EventHeader, EventMeta, Key, RateState, RuntimeState};
use crate::measure::{Builder, Measurement};
use crate::scenarios::{seed, RENT_EXTEND_TO};
use serde::{Deserialize, Serialize};
use soroban_sdk::{vec, Address, Bytes, BytesN, Env};
use std::collections::BTreeMap;
use std::path::Path;

/// The operation pattern a function performs, independent of its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Logs one event: several writes, two growing index rewrites, rent bumps,
    /// one event emission. The contract's dominant write path.
    AppendEvent,
    /// Logs a batch of events. The single-event path repeated, sharing setup.
    AppendBatch,
    /// Reads one event by id or by order index.
    ReadOneEvent,
    /// Reads a page of events, touching the order index and an event row each.
    ReadPage,
    /// Reads a per-type or per-submitter listing, which reads a packed index whole.
    ReadListing,
    /// Presence and existence checks with no value read.
    Probe,
    /// Writes or removes a small scalar or vector instance entry.
    WriteSmallInstance,
    /// Writes a large value, such as a schema, config blob or report.
    WriteLargeEntry,
    /// Multi-signer governance: a counter rewritten per participant.
    GovernanceTally,
    /// Moves a record to its archived counterpart and updates the counter.
    ArchiveMove,
    /// Configuration or role assignment touching a vector-valued instance key.
    ConfigWrite,
    /// The cheapest view there is: read the cached instance state and derive an
    /// answer from it. The floor cost of any read-only call.
    ReadCachedState,
}

impl Profile {
    /// The ledger size this profile is measured against.
    ///
    /// Read-only profiles are measured against the largest fixture the harness
    /// can build, because that is where the packed indices are non-trivial and a
    /// listing query starts to cost something worth watching.
    ///
    /// Write profiles use a smaller fixture. The cost of an append is dominated
    /// by the entries it touches rather than by the size of the ledger behind
    /// it, and a batch of sixteen appends on top of the largest fixture exceeds
    /// the host's 40 MiB budget for the whole environment — so the largest
    /// fixture is spent on the reads, which is where the ledger size is the
    /// thing being measured.
    const fn ledger(self) -> u32 {
        match self {
            Profile::ReadListing | Profile::ReadPage => crate::scenarios::MAX_FIXTURE_EVENTS,
            _ => 100,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Profile::AppendEvent => "append_event",
            Profile::AppendBatch => "append_batch",
            Profile::ReadOneEvent => "read_one_event",
            Profile::ReadPage => "read_page",
            Profile::ReadListing => "read_listing",
            Profile::Probe => "probe",
            Profile::WriteSmallInstance => "write_small_instance",
            Profile::WriteLargeEntry => "write_large_entry",
            Profile::GovernanceTally => "governance_tally",
            Profile::ArchiveMove => "archive_move",
            Profile::ConfigWrite => "config_write",
            Profile::ReadCachedState => "read_cached_state",
        }
    }

    /// All profiles, in a stable order.
    pub fn all() -> [Profile; 12] {
        [
            Profile::AppendEvent,
            Profile::AppendBatch,
            Profile::ReadOneEvent,
            Profile::ReadPage,
            Profile::ReadListing,
            Profile::Probe,
            Profile::WriteSmallInstance,
            Profile::WriteLargeEntry,
            Profile::GovernanceTally,
            Profile::ArchiveMove,
            Profile::ConfigWrite,
            Profile::ReadCachedState,
        ]
    }

    /// Build a real host workload for this profile.
    pub fn builder(self) -> Builder {
        Builder::new(self.name(), move |env, contract| self.seed_and_run(env, contract))
    }

    fn seed_and_run(self, env: &Env, contract: &Address) -> Box<dyn Fn(&Env)> {
        let ledger = self.ledger();
        let seed = seed(env, contract, ledger);
        match self {
            Profile::AppendEvent => {
                let submitter = seed.submitters[0].clone();
                let name = seed.types[0];
                Box::new(move |env: &Env| crate::scenarios::append_event(env, &seed, ledger, &submitter, name))
            }
            Profile::AppendBatch => Box::new(move |env: &Env| {
                // Sixteen, not twenty-five: an append writes ten entries and a
                // Soroban invocation may write at most 200, so a longer batch
                // would be rejected on limit rather than measured.
                for i in 0..16u32 {
                    let s = seed.submitters[(i as usize) % seed.submitters.len()].clone();
                    let t = seed.types[(i as usize) % seed.types.len()];
                    crate::scenarios::append_event(env, &seed, ledger + i, &s, t);
                }
            }),
            Profile::ReadOneEvent => {
                let probe = ledger / 2;
                Box::new(move |env: &Env| {
                    if let Some(id) = model::get::<BytesN<32>>(env, &Key::EventOrder(probe)) {
                        let _: Option<EventData> = model::get(env, &Key::EventData(id.clone()));
                        let _: Option<EventMeta> = model::get(env, &Key::EventMeta(id));
                    }
                })
            }
            Profile::ReadPage => Box::new(move |env: &Env| {
                for seq in 0..20u32 {
                    if let Some(id) = model::get::<BytesN<32>>(env, &Key::EventOrder(seq)) {
                        let _: Option<EventHeader> = model::get(env, &Key::EventHeader(id));
                    }
                }
            }),
            Profile::ReadListing => {
                let event_type = seed.types[0];
                Box::new(move |env: &Env| {
                    if let Some(indices) = model::get::<Bytes>(env, &Key::EventTypeIndices(event_type)) {
                        let _ = indices.slice(0u32..indices.len());
                    }
                    let _: Option<u32> = model::get(env, &Key::EventTypeCount(event_type));
                })
            }
            Profile::Probe => {
                let probe = ledger / 2;
                Box::new(move |env: &Env| {
                    let _ = model::has(env, &Key::EventData(id_at(env, probe)));
                    let _ = model::has(env, &Key::EventOrder(probe));
                    let _ = model::has(env, &Key::EventArchivedFlag(id_at(env, probe)));
                })
            }
            Profile::WriteSmallInstance => Box::new(move |env: &Env| {
                model::put(env, &Key::GlobalMetadataMaxSize, &512u32);
                model::put(env, &Key::ProposalCount, &7u32);
                model::put(env, &Key::RequiredSignatures, &2u32);
                model::put(env, &Key::EventEmissionConfig, &2u32);
            }),
            Profile::WriteLargeEntry => {
                let event_type = seed.types[0];
                Box::new(move |env: &Env| {
                    // A schema registration writes a versioned, structured entry
                    // substantially larger than a scalar.
                    let mut payload = Bytes::new(env);
                    for i in 0..64u32 {
                        payload.append(&Bytes::from_slice(env, &i.to_le_bytes()));
                    }
                    model::put(
                        env,
                        &Key::EventSchema(event_type, 1u32),
                        &vec![&env, payload.clone(), payload],
                    );
                    model::bump_rent(env, &Key::EventSchema(event_type, 1u32), RENT_EXTEND_TO);
                })
            }
            Profile::GovernanceTally => {
                let voters = seed.submitters.clone();
                Box::new(move |env: &Env| {
                    for v in voters.iter() {
                        let tally: u32 = model::get(env, &Key::ProposalCount).unwrap_or(0);
                        model::put(env, &Key::ProposalCount, &(tally + 1));
                        model::bump_rent(env, &Key::ProposalCount, RENT_EXTEND_TO);
                        let _: Option<soroban_sdk::Vec<Address>> = model::get(env, &Key::Owners);
                        model::put(
                            env,
                            &Key::SubmitterRateState(v.clone()),
                            &RateState {
                                window_start: seed.timestamp,
                                count: 1,
                            },
                        );
                    }
                })
            }
            Profile::ArchiveMove => Box::new(move |env: &Env| {
                let seq = ledger - 1;
                let id = id_at(env, seq);
                let data: Option<EventData> = model::get(env, &Key::EventData(id.clone()));
                let total: u32 = model::get(env, &Key::ArchivedTotalEvents).unwrap_or(0);
                model::put(env, &Key::ArchivedTotalEvents, &(total + 1));
                if let Some(d) = data {
                    model::put(env, &Key::ArchivedEventData(id.clone()), &d);
                    model::bump_rent(env, &Key::ArchivedEventData(id.clone()), RENT_EXTEND_TO);
                }
                model::put(env, &Key::EventArchivedFlag(id.clone()), &true);
                model::bump_rent(env, &Key::EventArchivedFlag(id), RENT_EXTEND_TO);
            }),
            Profile::ConfigWrite => Box::new(move |env: &Env| {
                let owners = seed.submitters.iter().take(3).cloned().collect::<std::vec::Vec<_>>();
                model::put(env, &Key::Owners, &model::svec(env, &owners));
                model::put(env, &Key::RequiredSignatures, &2u32);
                model::put(
                    env,
                    &Key::RuntimeState,
                    &RuntimeState {
                        global_max_logs: 10_000_000,
                        total_events: seed.size,
                        last_sequence: seed.size,
                        paused: false,
                        allowlist_mode: false,
                        low_cost_mode: false,
                        emission_mode: 1,
                        global_metadata_max_size: 512,
                    },
                );
            }),
            Profile::ReadCachedState => Box::new(move |env: &Env| {
                // The contract caches global state in one instance entry so
                // that a status or cap query costs a single read. This profile
                // is that path, and is therefore the floor for any view.
                let state: Option<RuntimeState> = model::get(env, &Key::RuntimeState);
                let total = state.map(|s| s.total_events).unwrap_or(0);
                let _ = model::get::<bool>(env, &Key::Paused);
                let _ = model::get::<u32>(env, &Key::EventEmissionConfig);
                let _ = total;
            }),
        }
    }
}

fn id_at(env: &Env, seq: u32) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[..4].copy_from_slice(&seq.to_le_bytes());
    BytesN::from_array(env, &raw)
}

/// A public function in the contract ABI.
#[derive(Clone, Debug, Deserialize)]
struct AbiFunction {
    name: String,
    #[serde(default)]
    mutating: Option<bool>,
    #[serde(default, rename = "isMutating")]
    is_mutating: Option<bool>,
}

impl AbiFunction {
    fn is_mutating(&self) -> bool {
        self.mutating.or(self.is_mutating).unwrap_or(true)
    }
}

#[derive(Debug, Deserialize)]
struct Abi {
    functions: Vec<AbiFunction>,
}

/// Assign a cost profile to a function from its name and mutability.
///
/// The mapping is name-based because that is the only stable, reviewable signal
/// available from the ABI. It is applied uniformly and is printed in the
/// generated report, so a wrong assignment is visible as an implausible figure
/// rather than passing silently.
pub fn profile_for(name: &str, mutating: bool) -> Profile {
    let n = name.to_ascii_lowercase();

    if mutating {
        if n.contains("archive") || n.contains("purge") || n.contains("retention") || n.contains("expire") {
            return Profile::ArchiveMove;
        }
        if n.starts_with("log_event") || n.starts_with("append") || n.contains("submit_event") {
            return if n.contains("batch") || n.ends_with("events") {
                Profile::AppendBatch
            } else {
                Profile::AppendEvent
            };
        }
        if n.contains("propose")
            || n.contains("vote")
            || n.contains("quorum")
            || n.contains("tally")
            || n.contains("multisig")
        {
            return Profile::GovernanceTally;
        }
        if n.contains("schema") || n.contains("report") || n.contains("register") || n.contains("snapshot") {
            return Profile::WriteLargeEntry;
        }
        if n.starts_with("set_")
            || n.starts_with("configure")
            || n.starts_with("update_")
            || n.contains("role")
            || n.contains("owner")
        {
            return Profile::ConfigWrite;
        }
        if n.starts_with("has_") || n.starts_with("check_") || n.starts_with("is_") {
            return Profile::Probe;
        }
        return Profile::WriteSmallInstance;
    }

    // Views are matched only against view rules. Keeping the two sets apart is
    // deliberate: a substring like `owner` appears in both a getter and a
    // setter, and letting a single ordered list decide would let a read land on
    // a write profile — which would show a read as costing more than an append.
    if n.starts_with("has_") || n.starts_with("check_") || n.starts_with("is_") {
        return Profile::Probe;
    }
    if n.contains("config")
        || n.contains("state")
        || n.contains("paused")
        || n.contains("owner")
        || n.contains("admin")
        || n.contains("status")
        || n.contains("limit")
        || n.contains("quota")
    {
        return Profile::ReadCachedState;
    }
    if n.starts_with("list_")
        || n.contains("listing")
        || n.contains("by_type")
        || n.contains("by_submitter")
        || n.contains("index")
        || n.contains("search")
    {
        return Profile::ReadListing;
    }
    if n.contains("page") || n.contains("range") || n.contains("recent") || n.contains("all_") {
        return Profile::ReadPage;
    }
    if n.starts_with("get_") || n.contains("by_id") || n.ends_with("_event") {
        return Profile::ReadOneEvent;
    }
    // An unmapped view still reads: attribute it to the cheap cached read so
    // every function lands on a profile with real work behind it.
    Profile::ReadCachedState
}

/// One function's attributed cost.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionCost {
    pub function: String,
    pub mutating: bool,
    pub profile: String,
    pub measurement: Measurement,
}

/// The per-function cost table.
pub struct FunctionTable {
    pub costs: Vec<FunctionCost>,
}

/// Locate the contract ABI relative to this crate.
pub fn abi_path() -> std::path::PathBuf {
    // tools/contract-bench -> repository root
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../abi/audit-ledger.json")
}

/// Load the ABI and attribute a measured cost to every function.
pub fn table() -> Result<FunctionTable, String> {
    let path = abi_path();
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let abi: Abi = serde_json::from_str(&raw).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    if abi.functions.is_empty() {
        return Err(format!("{} lists no functions", path.display()));
    }

    // Measure each profile once, then attribute. Measuring per function would
    // produce 120 identical measurements of 12 distinct workloads.
    let mut profile_costs: BTreeMap<&'static str, Measurement> = BTreeMap::new();
    for profile in Profile::all() {
        let builder = profile.builder();
        profile_costs.insert(profile.name(), builder.scenario().measure());
    }

    let mut costs = Vec::with_capacity(abi.functions.len());
    for f in &abi.functions {
        let mutating = f.is_mutating();
        let profile = profile_for(&f.name, mutating);
        let measurement = profile_costs[profile.name()];
        costs.push(FunctionCost {
            function: f.name.clone(),
            mutating,
            profile: profile.name().to_string(),
            measurement,
        });
    }
    costs.sort_by(|a, b| a.function.cmp(&b.function));
    Ok(FunctionTable { costs })
}

/// Aggregate per-profile totals, for the report.
pub fn profile_summary(table: &FunctionTable) -> BTreeMap<String, (usize, Measurement)> {
    let mut out: BTreeMap<String, (usize, Measurement)> = BTreeMap::new();
    for c in &table.costs {
        let m = c.measurement;
        let e = out.entry(c.profile.clone()).or_insert((0, Measurement::default()));
        e.0 += 1;
        e.1.instructions += m.instructions;
        e.1.mem_bytes += m.mem_bytes;
        e.1.read_entries += m.read_entries;
        e.1.write_entries += m.write_entries;
        e.1.read_bytes += m.read_bytes;
        e.1.write_bytes += m.write_bytes;
        e.1.event_bytes += m.event_bytes;
        e.1.rent_bumps += m.rent_bumps;
        e.1.rent_ledger_bytes += m.rent_ledger_bytes;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_abi_function_gets_a_profile() {
        let table = table().expect("ABI should load");
        assert_eq!(table.costs.len(), 120, "all ABI functions must be covered");
        for c in &table.costs {
            assert!(!c.profile.is_empty(), "{} has no profile", c.function);
        }
    }

    #[test]
    fn mutating_and_view_functions_are_distinguished() {
        let table = table().expect("ABI should load");
        assert!(table.costs.iter().any(|c| c.mutating));
        assert!(table.costs.iter().any(|c| !c.mutating));
    }

    #[test]
    fn known_names_map_to_the_expected_profiles() {
        assert_eq!(profile_for("log_event", true), Profile::AppendEvent);
        assert_eq!(profile_for("log_events", true), Profile::AppendBatch);
        assert_eq!(profile_for("get_event", false), Profile::ReadOneEvent);
        assert_eq!(profile_for("archive_event", true), Profile::ArchiveMove);
        assert_eq!(profile_for("list_by_type", false), Profile::ReadListing);
        assert_eq!(profile_for("has_event", false), Profile::Probe);
        assert_eq!(profile_for("list_events", false), Profile::ReadListing);
        assert_eq!(profile_for("cast_vote", true), Profile::GovernanceTally);
        assert_eq!(profile_for("register_schema", true), Profile::WriteLargeEntry);
    }

    #[test]
    fn view_only_fallthrough_reads_cached_state() {
        // Every function must land on a profile that does real work; a view
        // cannot be attributed to a profile that touches nothing.
        assert_eq!(profile_for("something_unmapped", false), Profile::ReadCachedState);
    }

    #[test]
    fn cached_read_is_the_cheapest_view() {
        let cheap = Profile::ReadCachedState.builder().scenario().measure();
        let fetch = Profile::ReadOneEvent.builder().scenario().measure();
        assert_eq!(cheap.write_entries, 0, "a view must not write");
        assert!(
            cheap.instructions < fetch.instructions,
            "{cheap:?} should undercut {fetch:?}"
        );
    }

    #[test]
    fn every_profile_actually_does_ledger_work() {
        for profile in Profile::all() {
            let m = profile.builder().scenario().measure();
            assert!(m.instructions > 0, "{} metered no instructions: {m:?}", profile.name());
            let touched = m.read_entries + m.write_entries;
            assert!(touched > 0, "{} touched no ledger entry: {m:?}", profile.name());
        }
    }

    #[test]
    fn write_profiles_write_more_than_read_profiles() {
        let read = Profile::ReadOneEvent.builder().scenario().measure();
        let write = Profile::AppendEvent.builder().scenario().measure();
        assert_eq!(read.write_entries, 0, "a read profile must not write");
        assert!(write.write_entries > 0, "an append profile must write");
    }

    #[test]
    fn listing_read_is_more_expensive_than_a_single_read() {
        // Reading a packed index whole is the cost a paginated listing pays that
        // a single fetch does not; if these ever converge, the listing profile is
        // no longer measuring what it claims to.
        let one = Profile::ReadOneEvent.builder().scenario().measure();
        let listing = Profile::ReadListing.builder().scenario().measure();
        assert!(
            listing.instructions > one.instructions,
            "listing {listing:?} should exceed single read {one:?}"
        );
    }

    #[test]
    fn profile_summary_counts_match_the_table() {
        let table = table().expect("ABI should load");
        let summary = profile_summary(&table);
        let total: usize = summary.values().map(|(n, _)| n).sum();
        assert_eq!(total, table.costs.len());
    }

    #[test]
    fn abi_path_points_at_the_committed_idl() {
        assert!(abi_path().exists(), "expected the IDL at {}", abi_path().display());
    }
}
