//! The suite's result model, and the code that produces it.
//!
//! # One shape for every metric
//!
//! Gas, storage traffic, memory and WASM size are not the same kind of quantity,
//! but they are all "a number that got bigger" or "a number that got smaller", and
//! a regression gate only needs to compare one number against a previous one. So
//! results carry a flat, ordered map of named integer metrics rather than a
//! struct per kind. That keeps comparison, history, alerting and export generic —
//! adding a metric does not mean touching the comparison code, and a metric
//! nobody compares is impossible to add by accident.

use crate::functions;
use crate::measure::Measurement;
use crate::scenarios;
use crate::wasm;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Schema version of the result file. Bumped when the shape changes, so history
/// written by an older suite is not silently compared against a newer one.
pub const SCHEMA: u32 = 1;

/// Which way is bad for a metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// A rise is a regression. Correct for every cost metric.
    Higher,
    /// A fall is a regression. For metrics where less is worse.
    Lower,
}

impl Direction {
    /// Whether moving from `before` to `after` in the given direction is worse.
    pub fn is_worse(self, before: i64, after: i64) -> bool {
        match self {
            Direction::Higher => after > before,
            Direction::Lower => after < before,
        }
    }

    /// Direction to hold a named metric to.
    ///
    /// Every metric this suite records is a cost — instructions, bytes, entries,
    /// module size — so all of them are `Higher`. The `Lower` case exists so a
    /// genuinely inverted metric cannot be added without a decision being made
    /// about it here.
    pub fn for_metric(_name: &str) -> Self {
        Direction::Higher
    }
}

/// One measured benchmark.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchResult {
    /// Stable identifier, e.g. `scenario:single_event/ledger=1000`. History and
    /// regression comparison key on this, so it must not embed a timestamp or a
    /// run-to-run varying value.
    pub id: String,
    /// Which family this belongs to: `scenario`, `function`, `storage`, `wasm`.
    pub group: String,
    /// Short name without the parameters.
    pub name: String,
    /// Parameters, e.g. ledger size and batch size.
    pub params: BTreeMap<String, String>,
    /// Measured metrics, ordered for deterministic output.
    pub metrics: BTreeMap<String, i64>,
}

impl BenchResult {
    /// Build a result from a host measurement.
    pub fn measured(
        id: impl Into<String>,
        group: &str,
        name: impl Into<String>,
        params: BTreeMap<String, String>,
        m: Measurement,
    ) -> Self {
        let mut metrics = BTreeMap::new();
        metrics.insert("instructions".into(), m.instructions);
        metrics.insert("mem_bytes".into(), m.mem_bytes);
        metrics.insert("read_entries".into(), i64::from(m.read_entries));
        metrics.insert("write_entries".into(), i64::from(m.write_entries));
        metrics.insert("read_bytes".into(), i64::from(m.read_bytes));
        metrics.insert("write_bytes".into(), i64::from(m.write_bytes));
        metrics.insert("event_bytes".into(), i64::from(m.event_bytes));
        metrics.insert("rent_bumps".into(), i64::from(m.rent_bumps));
        metrics.insert("rent_ledger_bytes".into(), m.rent_ledger_bytes);
        Self {
            id: id.into(),
            group: group.into(),
            name: name.into(),
            params,
            metrics,
        }
    }

    /// One metric's value, or `None` if this result does not record it.
    pub fn metric(&self, name: &str) -> Option<i64> {
        self.metrics.get(name).copied()
    }
}

/// A complete suite run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchSuite {
    pub schema: u32,
    /// Suite name, so a result file is identifiable on its own.
    pub name: String,
    /// The `soroban-sdk` version the numbers were metered against. Recorded
    /// because SDK cost-model changes move every figure; comparing across SDK
    /// versions without noticing is a false regression.
    pub sdk_version: String,
    /// What the numbers are, stated in the artifact itself.
    ///
    /// This is the field that keeps the result honest: it says the figures come
    /// from an operation model of the contract rather than from executing the
    /// contract, so a result file cannot be mistaken later for a measurement of
    /// the real thing.
    pub model: String,
    /// ISO-8601 timestamp, supplied by the caller so the library stays clock-free.
    pub generated_at: String,
    pub results: Vec<BenchResult>,
}

impl BenchSuite {
    /// Look up one result by id.
    pub fn get(&self, id: &str) -> Option<&BenchResult> {
        self.results.iter().find(|r| r.id == id)
    }

    /// Every distinct metric name present across all results.
    pub fn metric_names(&self) -> Vec<String> {
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for r in &self.results {
            names.extend(r.metrics.keys().cloned());
        }
        names.into_iter().collect()
    }

    /// Results belonging to one group.
    pub fn group(&self, group: &str) -> Vec<&BenchResult> {
        self.results.iter().filter(|r| r.group == group).collect()
    }
}

/// Directory the per-group bench harnesses write their JSON into.
///
/// `cargo bench` prints to stdout, which a CI log swallows, so each harness also
/// drops a machine-readable file. The path is under `target/` rather than the
/// data directory because a bench run is an observation, not an accepted baseline:
/// only `audit-ledger-bench run` writes history, and only `promote` writes the
/// baseline.
pub fn bench_output_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bench-results")
}

/// Write one group's results where CI can pick them up.
pub fn write_group_json(group: &str, results: &[BenchResult]) {
    let dir = bench_output_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("warning: cannot create {}: {e}", dir.display());
        return;
    }
    let path = dir.join(format!("{group}.json"));
    match std::fs::write(&path, crate::history::to_pretty_json(&results)) {
        Ok(()) => println!("wrote {}", path.display()),
        Err(e) => eprintln!("warning: cannot write {}: {e}", path.display()),
    }
}

/// The value of the `model` field: what these numbers are.
pub const MODEL: &str =
    "host-operation-model: figures are metered from a real Soroban host executing the contract's storage \
     and event operations, transcribed from the contract source. The contract crate itself does not compile, \
     so its own instruction count is not measured and the figures are a lower bound on production cost. \
     See docs/performance/benchmark-methodology.md.";

/// Storage layout benchmarks, isolating each host operation on its own.
///
/// The scenarios measure the contract's workflows; these measure the primitives
/// those workflows are built from, so a regression can be attributed to a
/// specific operation rather than to a scenario that moved.
pub fn storage_cases() -> Vec<scenarios::Case> {
    use crate::contract::{self as model, Key};
    use crate::measure::{fixture, Builder};
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env};

    fn id_at(env: &Env, seq: u32) -> BytesN<32> {
        let mut raw = [0u8; 32];
        raw[..4].copy_from_slice(&seq.to_le_bytes());
        BytesN::from_array(env, &raw)
    }

    let cases = vec![
        Builder::new("write_instance_scalar", |env: &Env, contract: &Address| {
            fixture(env, contract, || model::put(env, &Key::GlobalMetadataMaxSize, &512u32));
            Box::new(|env: &Env| model::put(env, &Key::GlobalMetadataMaxSize, &512u32))
        }),
        Builder::new("write_persistent_bytes32", |env: &Env, contract: &Address| {
            let id = id_at(env, 1);
            fixture(env, contract, || {
                model::put(env, &Key::EventData(id.clone()), &Bytes::from_slice(env, &[3u8; 32]));
            });
            Box::new(move |env: &Env| model::put(env, &Key::EventData(id.clone()), &Bytes::from_slice(env, &[3u8; 32])))
        }),
        Builder::new("read_persistent", |env: &Env, contract: &Address| {
            let id = id_at(env, 1);
            let key = Key::EventData(id);
            fixture(env, contract, || {
                model::put(env, &key, &Bytes::from_slice(env, &[3u8; 32]));
            });
            Box::new(move |env: &Env| {
                let _: Option<Bytes> = model::get(env, &key);
            })
        }),
        Builder::new("has_persistent", |env: &Env, contract: &Address| {
            let id = id_at(env, 1);
            let key = Key::EventData(id);
            fixture(env, contract, || {
                model::put(env, &key, &Bytes::from_slice(env, &[3u8; 32]));
            });
            Box::new(move |env: &Env| {
                let _ = model::has(env, &key);
            })
        }),
        Builder::new("rewrite_packed_index", |env: &Env, contract: &Address| {
            let t = "trade";
            fixture(env, contract, || {
                model::put(env, &Key::EventTypeIndices(t), &model::packed_indices(env, 1000));
            });
            Box::new(move |env: &Env| {
                let existing: Bytes = model::get(env, &Key::EventTypeIndices(t)).unwrap();
                model::put(
                    env,
                    &Key::EventTypeIndices(t),
                    &model::append_index(env, &existing, 1000),
                );
            })
        }),
        Builder::new("rent_extension", |env: &Env, contract: &Address| {
            let id = id_at(env, 1);
            let key = Key::EventData(id);
            fixture(env, contract, || {
                model::put(env, &key, &Bytes::from_slice(env, &[3u8; 32]));
            });
            Box::new(move |env: &Env| model::bump_rent(env, &key, scenarios::RENT_EXTEND_TO))
        }),
        Builder::new("emit_event", |env: &Env, _contract: &Address| {
            let submitter = Address::generate(env);
            Box::new(move |env: &Env| {
                model::EventLogged {
                    event_type: soroban_sdk::Symbol::new(env, "trade"),
                    submitter: submitter.clone(),
                    metadata: Bytes::from_slice(env, &[1u8, 8]),
                    sequence: 1,
                }
                .publish(env)
            })
        }),
    ];

    cases
        .into_iter()
        .map(|builder| {
            let name = builder.name().to_string();
            scenarios::Case::new(format!("storage:{name}"), "storage", name, BTreeMap::new(), builder)
        })
        .collect()
}

/// Run the whole suite.
pub fn run(generated_at: impl Into<String>) -> std::result::Result<BenchSuite, String> {
    let mut results = Vec::new();

    for case in scenarios::all() {
        let m = case.measure();
        results.push(BenchResult::measured(
            &case.id,
            &case.group,
            &case.name,
            case.params(),
            m,
        ));
    }

    for case in storage_cases() {
        let m = case.measure();
        results.push(BenchResult::measured(
            &case.id,
            &case.group,
            &case.name,
            case.params(),
            m,
        ));
    }

    // Per-function coverage: every ABI function attributed to a measured profile.
    let table = functions::table()?;
    for cost in &table.costs {
        let mut params = BTreeMap::new();
        params.insert("profile".into(), cost.profile.clone());
        params.insert("mutating".into(), cost.mutating.to_string());
        results.push(BenchResult::measured(
            format!("function:{}", cost.function),
            "function",
            cost.function.clone(),
            params,
            cost.measurement,
        ));
    }

    // WASM size.
    let report = wasm::discover();
    let mut params = BTreeMap::new();
    params.insert("path".into(), report.path.clone());
    if report.available {
        let mut metrics = BTreeMap::new();
        metrics.insert("total_bytes".into(), report.total_bytes.unwrap_or(0) as i64);
        if let Some(code) = report.section("code") {
            metrics.insert("code_bytes".into(), i64::from(code));
        }
        if let Some(data) = report.section("data") {
            metrics.insert("data_bytes".into(), i64::from(data));
        }
        results.push(BenchResult {
            id: "wasm:contract".into(),
            group: "wasm".into(),
            name: "contract".into(),
            params,
            metrics,
        });
    } else {
        // Recorded as unavailable, not as zero. A zero-byte module would compare
        // equal to any other zero and quietly pass every regression check.
        params.insert(
            "unavailable".into(),
            report.unavailable_reason.clone().unwrap_or_default(),
        );
        results.push(BenchResult {
            id: "wasm:contract".into(),
            group: "wasm".into(),
            name: "contract".into(),
            params,
            metrics: BTreeMap::new(),
        });
    }

    Ok(BenchSuite {
        schema: SCHEMA,
        name: "audit-ledger contract benchmarks".into(),
        sdk_version: sdk_version(),
        model: MODEL.into(),
        generated_at: generated_at.into(),
        results,
    })
}

/// The `soroban-sdk` version in use, read from this crate's own lockfile.
pub fn sdk_version() -> String {
    crate::version::sdk_version_from_lockfile()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_results_expand_every_counter() {
        let r = BenchResult::measured("x", "scenario", "x", BTreeMap::new(), Measurement::default());
        for key in [
            "instructions",
            "mem_bytes",
            "read_entries",
            "write_entries",
            "read_bytes",
            "write_bytes",
            "event_bytes",
            "rent_bumps",
            "rent_ledger_bytes",
        ] {
            assert!(r.metric(key).is_some(), "missing metric {key}");
        }
    }

    #[test]
    fn result_ids_are_unique_within_a_suite() {
        let suite = run("2026-01-01T00:00:00Z").expect("suite should run");
        let mut ids: Vec<&str> = suite.results.iter().map(|r| r.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate result ids would corrupt comparison");
    }

    #[test]
    fn suite_covers_every_required_scenario() {
        let suite = run("2026-01-01T00:00:00Z").expect("suite should run");
        for required in ["single_event", "batch_events", "queries", "governance", "archive"] {
            assert!(
                suite.results.iter().any(|r| r.name == required),
                "missing required scenario {required}"
            );
        }
    }

    #[test]
    fn suite_covers_all_abi_functions() {
        let suite = run("2026-01-01T00:00:00Z").expect("suite should run");
        let n = suite.group("function").len();
        assert_eq!(n, 120, "every ABI function needs a figure");
    }

    #[test]
    fn storage_primitives_are_all_measured() {
        let suite = run("2026-01-01T00:00:00Z").expect("suite should run");
        for primitive in [
            "write_instance_scalar",
            "write_persistent_bytes32",
            "read_persistent",
            "has_persistent",
            "rewrite_packed_index",
            "rent_extension",
            "emit_event",
        ] {
            assert!(
                suite.results.iter().any(|r| r.name == primitive),
                "missing storage primitive {primitive}"
            );
        }
    }

    #[test]
    fn suite_states_its_provenance() {
        let suite = run("2026-01-01T00:00:00Z").expect("suite should run");
        assert!(
            suite.model.contains("does not compile"),
            "the model field must state the limitation"
        );
        assert!(!suite.sdk_version.is_empty());
    }

    #[test]
    fn suite_is_byte_identical_across_runs() {
        // The property the whole regression gate rests on.
        let a = run("2026-01-01T00:00:00Z").expect("suite should run");
        let b = run("2026-01-01T00:00:00Z").expect("suite should run");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "two runs of the suite must produce identical output"
        );
    }

    #[test]
    fn direction_encodes_which_way_is_bad() {
        assert!(Direction::Higher.is_worse(100, 110));
        assert!(!Direction::Higher.is_worse(110, 100));
        assert!(Direction::Lower.is_worse(100, 90));
        assert!(!Direction::Lower.is_worse(90, 100));
    }
}
