//! End-to-end tests for the suite.
//!
//! These exercise the paths a CI run depends on: a full measurement pass, the
//! comparison against a baseline, the history round trip, and the artifacts the
//! reporters produce. They are integration tests rather than unit tests because
//! the property that matters most is that the whole pipeline agrees with itself —
//! a suite that measures correctly but reports inconsistently is still broken.

use contract_bench::{
    history::HistoryEntry,
    regress::{self, RegressionReport, Verdict, THRESHOLD_PCT},
    report, suite, wasm,
};
use std::collections::BTreeMap;

fn stamp() -> String {
    "2026-01-01T00:00:00Z".into()
}

#[test]
fn a_full_run_measures_everything_the_issue_requires() {
    let s = suite::run(stamp()).expect("suite should run");

    for scenario in ["single_event", "batch_events", "queries", "governance", "archive"] {
        assert!(
            s.results.iter().any(|r| r.name == scenario),
            "required scenario {scenario} is missing"
        );
    }
    assert!(!s.group("function").is_empty(), "per-function coverage is required");
    assert!(!s.group("storage").is_empty(), "storage benchmarks are required");
    assert!(s.results.iter().any(|r| r.group == "wasm"), "wasm size is required");
}

#[test]
fn the_suite_measures_real_host_work_everywhere_it_claims_to() {
    let s = suite::run(stamp()).expect("suite should run");
    for r in &s.results {
        if r.group == "wasm" {
            continue; // legitimately has no metrics when no artifact exists
        }
        assert!(
            r.metric("instructions").unwrap_or(0) > 0,
            "{} recorded no instructions",
            r.id
        );
    }
}

#[test]
fn two_runs_are_identical() {
    // The property every regression decision rests on. Asserted on the serialized
    // form, not just on `Eq`, so that nondeterminism in serialization order is
    // caught too: a suite that measures the same values but writes them out in a
    // different order every run would still make a byte-committed history and a
    // byte-stable chart impossible.
    let a = suite::run(stamp()).expect("run");
    let b = suite::run(stamp()).expect("run");
    assert_eq!(a, b, "the suite must be deterministic");
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap(),
        "the suite must serialise identically"
    );
}

#[test]
fn the_only_run_to_run_difference_is_the_timestamp() {
    // The doc's determinism claim is "byte-identical", and the only thing allowed
    // to differ between two real runs is the stamp. This is asserted with
    // different stamps, which is the situation the CLI is actually in.
    let a = suite::run("2026-01-01T00:00:00Z").expect("run");
    let b = suite::run("2026-01-01T00:00:01Z").expect("run");
    let mut va = serde_json::to_value(&a).unwrap();
    let mut vb = serde_json::to_value(&b).unwrap();
    for v in [&mut va, &mut vb] {
        v.as_object_mut().unwrap().remove("generated_at");
    }
    assert_eq!(va, vb, "only generated_at may differ between runs");
}

#[test]
fn an_unchanged_suite_reports_no_regression() {
    let a = suite::run(stamp()).expect("run");
    let b = suite::run(stamp()).expect("run");
    let report = RegressionReport::new(THRESHOLD_PCT, regress::compare(&a, &b, THRESHOLD_PCT));
    assert!(report.passed(), "an unchanged suite must pass: {:?}", report.notable());
    assert_eq!(report.regressions, 0);
}

#[test]
fn a_deliberately_inflated_suite_is_caught() {
    // Take a real measurement, inflate every cost metric past the threshold, and
    // confirm the gate fires. Without this the gate is untested: a comparison
    // routine that always returns "pass" would look identical in every other test.
    let honest = suite::run(stamp()).expect("run");
    let mut inflated = honest.clone();
    for r in &mut inflated.results {
        for (k, v) in r.metrics.iter_mut() {
            if *v > 0 {
                *v *= 2;
            }
            let _ = k;
        }
    }
    let report = RegressionReport::new(THRESHOLD_PCT, regress::compare(&inflated, &honest, THRESHOLD_PCT));
    assert!(!report.passed(), "doubling every cost must be caught");
    assert!(report.regressions > 0);
    let annotations = report.annotations();
    assert!(annotations.iter().any(|a| a.starts_with("::error")), "{annotations:?}");
}

#[test]
fn a_small_degradation_below_the_threshold_passes() {
    // 4% must not fail, or the gate becomes noise and gets disabled.
    let honest = suite::run(stamp()).expect("run");
    let mut nudged = honest.clone();
    for r in &mut nudged.results {
        for v in r.metrics.values_mut() {
            // Only metrics large enough to express 4% faithfully. Adding four
            // percent to an entry count of one is a hundred percent, and a gate
            // that called that a regression would be right to — so the test
            // nudges the byte and instruction counters, where a 4% move is 4%.
            if *v >= 100 {
                *v += (*v as f64 * 0.04).ceil() as i64;
            }
        }
    }
    let report = RegressionReport::new(THRESHOLD_PCT, regress::compare(&nudged, &honest, THRESHOLD_PCT));
    assert!(report.passed(), "a 4% move must stay under the gate");
}

#[test]
fn history_entries_round_trip_and_build_series() {
    let runs: Vec<HistoryEntry> = (0..3)
        .map(|i| {
            let s = suite::run(format!("2026-01-0{}T00:00:00Z", i + 1)).expect("run");
            HistoryEntry::new(&format!("2026-01-0{}T00:00:00Z", i + 1), s)
        })
        .collect();

    for entry in &runs {
        let json = contract_bench::history::to_pretty_json(entry);
        let back: HistoryEntry = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back.suite.results.len(), entry.suite.results.len());
    }

    let series = contract_bench::history::series(&runs, Some("scenario"), "instructions");
    assert!(!series.is_empty());
    assert!(
        series.iter().all(|s| s.points.len() == 3),
        "each series should span every run"
    );
}

#[test]
fn the_markdown_report_covers_every_group() {
    let s = suite::run(stamp()).expect("run");
    let text = report::report(&s, None, Some(&wasm::discover()));
    assert!(text.contains("## Scenarios and storage primitives"));
    assert!(text.contains("## Per-function coverage"));
    assert!(text.contains("## WASM size"));
    assert!(text.contains("does not compile"), "the caveat must reach the report");
}

#[test]
fn the_prometheus_export_is_parseable() {
    let s = suite::run(stamp()).expect("run");
    let text = report::prometheus(&s);
    let samples = text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).count();
    assert_eq!(samples, s.results.iter().map(|r| r.metrics.len()).sum::<usize>());
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let value = line.rsplit(' ').next().unwrap();
        assert!(value.parse::<i64>().is_ok(), "bad sample value in {line}");
    }
}

#[test]
fn the_grafana_dashboard_is_importable_json() {
    let v: serde_json::Value = serde_json::from_str(&report::grafana_dashboard()).expect("valid JSON");
    assert!(v["panels"].as_array().expect("panels").len() >= 5);
    assert_eq!(v["uid"], "audit-ledger-bench");
}

#[test]
fn wasm_budget_is_reported_as_unchecked_when_no_artifact_exists() {
    let r = wasm::discover();
    if !r.available {
        let check = wasm::check_budget(&r, &wasm::AUDIT_LEDGER_BUDGET);
        assert!(!check.passed(), "an unavailable artifact must never pass the budget");
        assert!(report::budget_line(&check).contains("NOT ENFORCED"));
    }
}

#[test]
fn a_removed_benchmark_is_surfaced_rather_than_ignored() {
    let full = suite::run(stamp()).expect("run");
    let mut trimmed = full.clone();
    trimmed.results.retain(|r| r.group != "storage");
    let report = RegressionReport::new(THRESHOLD_PCT, regress::compare(&trimmed, &full, THRESHOLD_PCT));
    assert!(report.removed > 0, "dropping a group must be reported as removed");
    assert!(
        report.annotations().iter().any(|a| a.contains("Benchmark removed")),
        "{:?}",
        report.annotations()
    );
}

#[test]
fn verdicts_that_do_not_fail_are_still_visible() {
    // A cheaper contract is good news and must be reported, but must not fail.
    let base = suite::run(stamp()).expect("run");
    let mut better = base.clone();
    for r in &mut better.results {
        for v in r.metrics.values_mut() {
            if *v > 10 {
                *v /= 2;
            }
        }
    }
    let report = RegressionReport::new(THRESHOLD_PCT, regress::compare(&better, &base, THRESHOLD_PCT));
    assert!(report.passed(), "getting cheaper must not fail the build");
    assert!(report.improvements > 0, "and it must still be reported");
}

#[test]
fn the_suite_describes_its_own_provenance() {
    let s = suite::run(stamp()).expect("run");
    // Read by a later reader of the JSON, so it has to be in the file.
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("does not compile"));
    assert!(json.contains("lower bound"));
}

#[test]
fn ids_carry_their_parameters() {
    let s = suite::run(stamp()).expect("run");
    let batch = s.results.iter().find(|r| r.name == "batch_events").expect("batch case");
    assert!(batch.id.contains("ledger="), "{}", batch.id);
    assert!(batch.id.contains("batch="), "{}", batch.id);
    let _ = BTreeMap::<String, String>::new();
}

#[test]
fn unavailable_verdict_never_fails() {
    assert!(!Verdict::Unavailable.is_failure());
    assert!(!Verdict::Removed.is_failure());
    assert!(!Verdict::Added.is_failure());
    assert!(!Verdict::Improved.is_failure());
    assert!(Verdict::Regressed.is_failure());
}

#[test]
fn the_gate_script_fails_on_an_inflated_results_file() {
    // The gate is only worth having if it can fail. It used to grep a report
    // that never contained a comparison, so it exited 0 whatever it was given;
    // this drives the real script with a deliberately inflated file and requires
    // a non-zero exit.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repository root")
        .to_path_buf();
    let script = root.join("scripts/ci/benchmark_regression_check.sh");
    if !script.exists() {
        eprintln!("skipping: {} not present", script.display());
        return;
    }

    let honest = suite::run(stamp()).expect("run");
    let mut inflated = honest.clone();
    for r in &mut inflated.results {
        for v in r.metrics.values_mut() {
            if *v >= 100 {
                *v += *v / 2;
            }
        }
    }
    let file = std::env::temp_dir().join("contract-bench-inflated.json");
    std::fs::write(&file, serde_json::to_string_pretty(&inflated).unwrap()).unwrap();

    let out = std::process::Command::new("bash")
        .arg(&script)
        .arg(&file)
        .arg("5")
        .current_dir(&root)
        .output()
        .expect("run the gate");
    let _ = std::fs::remove_file(&file);
    assert!(
        !out.status.success(),
        "the gate passed a 50% regression:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
