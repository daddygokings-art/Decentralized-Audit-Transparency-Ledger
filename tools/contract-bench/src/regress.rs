//! Regression detection.
//!
//! # The threshold, and why 5% is defensible for these metrics
//!
//! The suite is required to alert on a degradation above 5%. For wall-clock
//! timings that threshold would be unusable: run-to-run variance on a shared CI
//! runner routinely exceeds it, so the gate would fail at random, get muted, and
//! protect nothing. The quantities compared here are metered by the host and are
//! deterministic — the suite's tests assert that two runs of the whole thing
//! serialise byte-identically — so a 5% move is a real change in behaviour rather
//! than noise.
//!
//! # What counts as a regression
//!
//! A metric moving *up* is a regression, because every metric in the suite is a
//! cost. A metric moving *down* is an improvement and never fails the build: a
//! suite that fails when the contract gets cheaper is a suite that gets disabled.
//! The direction is explicit per metric rather than inferred, so adding a
//! genuinely inverted metric forces a decision instead of defaulting.
//!
//! # Missing data is not a pass
//!
//! Three cases get deliberate, distinct treatment, because collapsing them into
//! "equal" is how a benchmark suite rots:
//!
//! * **Added** — the metric is new. Reported, never a failure; a new benchmark
//!   should not block a PR.
//! * **Removed** — a benchmark disappeared. Reported as a warning. Silently
//!   losing coverage is a regression in the suite itself.
//! * **Unavailable** — a result exists but recorded no value for the metric,
//!   which is what a missing WASM artifact looks like. Reported as a warning, and
//!   never compared against zero.

use crate::suite::{BenchResult, BenchSuite, Direction};
use serde::{Deserialize, Serialize};

/// Default regression threshold, in percent.
pub const THRESHOLD_PCT: f64 = 5.0;

/// The outcome of comparing one metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Moved in the good direction, past the threshold.
    Improved,
    /// Moved, but not past the threshold.
    Within,
    /// Moved against us, past the threshold. Fails the build.
    Regressed,
    /// Not present in the baseline. Informational.
    Added,
    /// Present in the baseline but gone now. Informational, and a coverage loss.
    Removed,
    /// Recorded without a value in one of the two suites. Not comparable.
    Unavailable,
}

impl Verdict {
    /// Whether this verdict should fail a build.
    pub fn is_failure(self) -> bool {
        matches!(self, Verdict::Regressed)
    }

    /// Whether this verdict should be surfaced at all.
    pub fn is_noticeable(self) -> bool {
        !matches!(self, Verdict::Within)
    }
}

/// One metric comparison.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    pub id: String,
    pub group: String,
    pub metric: String,
    pub baseline: Option<i64>,
    pub current: Option<i64>,
    /// Percentage change, rounded to two decimals. `None` when not computable.
    pub change_pct: Option<f64>,
    pub threshold_pct: f64,
    pub verdict: Verdict,
}

impl Comparison {
    /// A one-line summary, used in CLI output and CI annotations.
    pub fn describe(&self) -> String {
        let base = self.baseline.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let cur = self.current.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let change = match self.change_pct {
            Some(p) if p > 0.0 => format!("+{p:.2}%"),
            Some(p) => format!("{p:.2}%"),
            None => "n/a".to_string(),
        };
        format!(
            "{} {}: {} -> {} ({change}) [{:?}]",
            self.group, self.metric, base, cur, self.verdict
        )
    }
}

/// Compare two suites metric by metric.
pub fn compare(current: &BenchSuite, baseline: &BenchSuite, threshold_pct: f64) -> Vec<Comparison> {
    if current.schema != baseline.schema {
        // Comparing across schema versions would attribute a shape change to the
        // code under test. Recorded as unavailable rather than compared.
        return vec![Comparison {
            id: "*".into(),
            group: "suite".into(),
            metric: "schema".into(),
            baseline: Some(i64::from(baseline.schema)),
            current: Some(i64::from(current.schema)),
            change_pct: None,
            threshold_pct,
            verdict: Verdict::Unavailable,
        }];
    }

    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();

    for result in &current.results {
        seen.insert(result.id.as_str());
        let prior = baseline.get(&result.id);
        out.extend(compare_result(result, prior, threshold_pct));
    }

    // Anything in the baseline that this run no longer produces.
    for prior in &baseline.results {
        if !seen.contains(prior.id.as_str()) {
            for metric in prior.metrics.keys() {
                out.push(Comparison {
                    id: prior.id.clone(),
                    group: prior.group.clone(),
                    metric: metric.clone(),
                    baseline: prior.metric(metric),
                    current: None,
                    change_pct: None,
                    threshold_pct,
                    verdict: Verdict::Removed,
                });
            }
        }
    }
    out
}

/// Compare one result against its prior version.
fn compare_result(current: &BenchResult, baseline: Option<&BenchResult>, threshold_pct: f64) -> Vec<Comparison> {
    let mut out = Vec::new();
    for (metric, value) in &current.metrics {
        let prior = baseline.and_then(|b| b.metric(metric));
        let verdict = match (prior, baseline) {
            (None, None) => Verdict::Added,
            (None, Some(_)) => Verdict::Added,
            (Some(_), None) => Verdict::Unavailable,
            (Some(p), Some(_)) => classify(p, *value, threshold_pct, metric),
        };
        let change_pct = match (prior, *value) {
            (Some(p), c) => change_pct(p, c),
            _ => None,
        };
        out.push(Comparison {
            id: current.id.clone(),
            group: current.group.clone(),
            metric: metric.clone(),
            baseline: prior,
            current: Some(*value),
            change_pct,
            threshold_pct,
            verdict,
        });
    }

    // Metrics the baseline recorded that this run dropped, e.g. a WASM size that
    // is no longer available. Distinct from a removed benchmark.
    if let Some(prior) = baseline {
        for metric in prior.metrics.keys() {
            if current.metric(metric).is_none() {
                out.push(Comparison {
                    id: current.id.clone(),
                    group: current.group.clone(),
                    metric: metric.clone(),
                    baseline: prior.metric(metric),
                    current: None,
                    change_pct: None,
                    threshold_pct,
                    verdict: Verdict::Unavailable,
                });
            }
        }
    }
    out
}

/// Classify one baseline/current pair.
fn classify(baseline: i64, current: i64, threshold_pct: f64, metric: &str) -> Verdict {
    // A metric that was absent and is now present cannot be expressed as a
    // percentage change, and treating it as "no change" would let a benchmark
    // start reporting a cost without anyone noticing.
    if baseline == 0 {
        return if current == 0 {
            Verdict::Within
        } else {
            Verdict::Regressed
        };
    }
    let change = (current as f64 - baseline as f64) / baseline.abs() as f64 * 100.0;
    let direction = Direction::for_metric(metric);
    if direction.is_worse(baseline, current) {
        if change.abs() > threshold_pct {
            Verdict::Regressed
        } else {
            Verdict::Within
        }
    } else if change.abs() > threshold_pct {
        Verdict::Improved
    } else {
        Verdict::Within
    }
}

/// Percentage change from `baseline` to `current`, rounded to two decimals.
pub fn change_pct(baseline: i64, current: i64) -> Option<f64> {
    if baseline == 0 {
        return None;
    }
    let change = (current as f64 - baseline as f64) / baseline.abs() as f64 * 100.0;
    Some((change * 100.0).round() / 100.0)
}

/// The full comparison report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionReport {
    pub threshold_pct: f64,
    pub comparisons: Vec<Comparison>,
    pub regressions: usize,
    pub improvements: usize,
    pub added: usize,
    pub removed: usize,
    pub unavailable: usize,
}

impl RegressionReport {
    /// Summarise a list of comparisons.
    pub fn new(threshold_pct: f64, comparisons: Vec<Comparison>) -> Self {
        let count = |v: Verdict| comparisons.iter().filter(|c| c.verdict == v).count();
        Self {
            threshold_pct,
            regressions: count(Verdict::Regressed),
            improvements: count(Verdict::Improved),
            added: count(Verdict::Added),
            removed: count(Verdict::Removed),
            unavailable: count(Verdict::Unavailable),
            comparisons,
        }
    }

    /// Whether the run should fail.
    pub fn passed(&self) -> bool {
        self.regressions == 0
    }

    /// The comparisons worth printing, worst first.
    pub fn notable(&self) -> Vec<&Comparison> {
        let mut v: Vec<&Comparison> = self.comparisons.iter().filter(|c| c.verdict.is_noticeable()).collect();
        v.sort_by(|a, b| {
            let key = |c: &Comparison| match c.verdict {
                Verdict::Regressed => 0,
                Verdict::Removed => 1,
                Verdict::Unavailable => 2,
                Verdict::Added => 3,
                Verdict::Improved => 4,
                Verdict::Within => 5,
            };
            key(a).cmp(&key(b)).then_with(|| {
                b.change_pct
                    .unwrap_or(0.0)
                    .partial_cmp(&a.change_pct.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        v
    }

    /// GitHub Actions workflow annotations, so a regression is visible inline on
    /// the diff rather than only in a log the reviewer has to find.
    pub fn annotations(&self) -> Vec<String> {
        self.notable()
            .iter()
            .map(|c| match c.verdict {
                Verdict::Regressed => format!(
                    "::error title=Benchmark regression::{} {}: {} -> {} ({:+.2}%, threshold {}%)",
                    c.id,
                    c.metric,
                    c.baseline.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    c.current.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    c.change_pct.unwrap_or(0.0),
                    c.threshold_pct
                ),
                Verdict::Removed => {
                    format!("::warning title=Benchmark removed::{} is no longer measured", c.id)
                }
                Verdict::Unavailable => format!(
                    "::warning title=Benchmark unavailable::{} {} could not be compared against the baseline",
                    c.id, c.metric
                ),
                _ => format!("::notice title=Benchmark::{}", c.describe()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Measurement;
    use std::collections::BTreeMap;

    fn result(id: &str, instructions: i64, write_entries: i64) -> BenchResult {
        let mut metrics = BTreeMap::new();
        metrics.insert("instructions".to_string(), instructions);
        metrics.insert("write_entries".to_string(), write_entries);
        BenchResult {
            id: id.into(),
            group: "scenario".into(),
            name: "x".into(),
            params: BTreeMap::new(),
            metrics,
        }
    }

    fn suite(results: Vec<BenchResult>) -> BenchSuite {
        BenchSuite {
            schema: crate::suite::SCHEMA,
            name: "t".into(),
            sdk_version: "27.0.6".into(),
            model: "m".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            results,
        }
    }

    fn verdict_for(baseline: i64, current: i64) -> Verdict {
        classify(baseline, current, THRESHOLD_PCT, "instructions")
    }

    #[test]
    fn a_move_under_the_threshold_is_within() {
        assert_eq!(verdict_for(1000, 1040), Verdict::Within, "4% is under the gate");
    }

    #[test]
    fn a_move_over_the_threshold_regresses() {
        assert_eq!(verdict_for(1000, 1051), Verdict::Regressed, "5.1% must fail");
    }

    #[test]
    fn the_threshold_is_exactly_five_percent() {
        assert_eq!(THRESHOLD_PCT, 5.0);
        // At exactly 5% the comparison is not "over", so it passes.
        assert_eq!(
            verdict_for(1000, 1050),
            Verdict::Within,
            "5.0% is at, not over, the gate"
        );
        // The first failing value is one whole unit past it. Metrics are integers,
        // so the boundary is asserted on integers rather than by truncating a
        // float: `1050.5 as i64` is 1050, which is *within* the gate.
        assert_eq!(verdict_for(1000, 1051), Verdict::Regressed, "5.1% must fail");
        // A large baseline still gates on the same 5%, not on an absolute step.
        assert_eq!(verdict_for(100_000, 105_000), Verdict::Within);
        assert_eq!(verdict_for(100_000, 105_001), Verdict::Regressed);
    }

    #[test]
    fn getting_cheaper_never_fails() {
        assert_eq!(verdict_for(1000, 100), Verdict::Improved);
        assert_eq!(verdict_for(1000, 0), Verdict::Improved);
        assert!(
            !verdict_for(1000, 10).is_failure(),
            "a large improvement must not fail the build"
        );
    }

    #[test]
    fn going_from_zero_to_nonzero_is_a_regression() {
        // A benchmark that starts costing something was previously free. A
        // percentage change is undefined, and treating it as neutral would let a
        // new cost appear unnoticed.
        assert_eq!(verdict_for(0, 500), Verdict::Regressed);
    }

    #[test]
    fn zero_to_zero_is_neutral() {
        assert_eq!(verdict_for(0, 0), Verdict::Within);
    }

    #[test]
    fn change_pct_is_rounded_and_signed() {
        assert_eq!(change_pct(1000, 1050), Some(5.0));
        assert_eq!(change_pct(1000, 950), Some(-5.0));
        assert_eq!(change_pct(1000, 1010), Some(1.0));
        assert_eq!(change_pct(0, 5), None, "no percentage exists from zero");
    }

    #[test]
    fn a_new_benchmark_is_added_not_regressed() {
        let r = compare(&suite(vec![result("a", 100, 1)]), &suite(vec![]), THRESHOLD_PCT);
        assert!(r.iter().all(|c| c.verdict == Verdict::Added), "{r:?}");
        assert!(
            RegressionReport::new(THRESHOLD_PCT, r).passed(),
            "a new benchmark must not fail the build"
        );
    }

    #[test]
    fn a_disappeared_benchmark_is_reported_as_removed() {
        let r = compare(&suite(vec![]), &suite(vec![result("a", 100, 1)]), THRESHOLD_PCT);
        assert!(r.iter().all(|c| c.verdict == Verdict::Removed), "{r:?}");
        let rep = RegressionReport::new(THRESHOLD_PCT, r);
        assert_eq!(rep.removed, 2, "both metrics of the removed result are reported");
        assert!(rep.passed(), "losing coverage must not fail, but must be visible");
    }

    #[test]
    fn a_metric_that_stops_being_recorded_is_unavailable() {
        // This is the WASM case: a result exists but recorded no size because no
        // artifact was built. It must not be compared against zero.
        let mut current = result("wasm", 0, 0);
        current.metrics.clear();
        let r = compare(
            &suite(vec![current]),
            &suite(vec![result("wasm", 42, 0)]),
            THRESHOLD_PCT,
        );
        assert!(r.iter().any(|c| c.verdict == Verdict::Unavailable), "{r:?}");
        assert!(RegressionReport::new(THRESHOLD_PCT, r).passed());
    }

    #[test]
    fn a_real_regression_fails_the_run() {
        let r = compare(
            &suite(vec![result("a", 2000, 5)]),
            &suite(vec![result("a", 1000, 2)]),
            THRESHOLD_PCT,
        );
        let rep = RegressionReport::new(THRESHOLD_PCT, r);
        assert!(!rep.passed());
        assert!(rep.regressions >= 1);
    }

    #[test]
    fn an_unchanged_suite_passes_cleanly() {
        let r = compare(
            &suite(vec![result("a", 1000, 2)]),
            &suite(vec![result("a", 1000, 2)]),
            THRESHOLD_PCT,
        );
        let rep = RegressionReport::new(THRESHOLD_PCT, r);
        assert!(rep.passed());
        assert_eq!(rep.regressions, 0);
        assert!(
            rep.notable().is_empty(),
            "an unchanged run should print nothing notable"
        );
    }

    #[test]
    fn schema_mismatch_is_not_compared() {
        let mut current = suite(vec![result("a", 2000, 5)]);
        current.schema = 99;
        let r = compare(&current, &suite(vec![result("a", 1000, 2)]), THRESHOLD_PCT);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].verdict, Verdict::Unavailable);
    }

    #[test]
    fn notable_sorts_regressions_first_and_biggest_first() {
        let r = compare(
            &suite(vec![
                result("small", 1010, 1),
                result("big", 2000, 1),
                result("gone_drop", 1, 1),
            ]),
            &suite(vec![
                result("small", 1000, 1),
                result("big", 1000, 1),
                result("other", 1, 1),
            ]),
            THRESHOLD_PCT,
        );
        let report = RegressionReport::new(THRESHOLD_PCT, r);
        let notable = report.notable();
        assert_eq!(notable[0].id, "big");
    }

    #[test]
    fn annotations_use_the_github_error_channel_for_regressions() {
        let r = compare(
            &suite(vec![result("a", 2000, 2)]),
            &suite(vec![result("a", 1000, 2)]),
            THRESHOLD_PCT,
        );
        let report = RegressionReport::new(THRESHOLD_PCT, r);
        let anns = report.annotations();
        assert!(anns.iter().any(|a| a.starts_with("::error")), "{anns:?}");
    }

    #[test]
    fn direction_is_configurable_for_inverted_metrics() {
        // A hypothetical metric where less is worse must be able to say so.
        assert!(Direction::Lower.is_worse(100, 90));
        assert!(!Direction::for_metric("anything").is_worse(100, 90));
    }

    #[test]
    fn comparison_covers_every_metric_of_every_result() {
        let m = Measurement {
            instructions: 10,
            write_entries: 2,
            ..Default::default()
        };
        let r = BenchResult::measured("x", "scenario", "x", BTreeMap::new(), m);
        let expected = r.metrics.len();
        let cmp = compare(&suite(vec![r.clone()]), &suite(vec![r.clone()]), THRESHOLD_PCT);
        assert_eq!(cmp.len(), expected, "every recorded metric must be compared");
    }
}
