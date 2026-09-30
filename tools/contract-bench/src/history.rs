//! Historical benchmark tracking.
//!
//! # Layout
//!
//! ```text
//! data/baseline.json        the accepted reference every run compares against
//! data/history/<stamp>.json every run, retained
//! ```
//!
//! The baseline is deliberately a single committed file rather than the most
//! recent history entry. A benchmark suite that compares against whatever ran last
//! cannot detect a slow drift: each run would bless the previous regression and the
//! numbers would ratchet upward one sub-threshold step at a time. Only an
//! explicitly accepted baseline catches that, and accepting one is a decision
//! somebody has to make on purpose.

use crate::suite::BenchSuite;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File name of the accepted baseline.
pub const BASELINE: &str = "baseline.json";

/// Directory holding retained runs.
pub const HISTORY_DIR: &str = "history";

/// Where a suite's data lives, relative to the crate.
pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn history_dir() -> PathBuf {
    data_dir().join(HISTORY_DIR)
}

/// A recorded run, as kept in history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The run's own timestamp, as the suite recorded it.
    pub generated_at: String,
    /// Optional provenance: commit, workflow run, or author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub suite: BenchSuite,
}

impl HistoryEntry {
    pub fn new(generated_at: &str, suite: BenchSuite) -> Self {
        Self {
            generated_at: generated_at.to_string(),
            revision: None,
            suite,
        }
    }

    pub fn with_revision(mut self, revision: impl Into<String>) -> Self {
        self.revision = Some(revision.into());
        self
    }
}

/// Serialise a run to a stable, human-diffable JSON document.
pub fn to_pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("benchmark data is always serialisable")
}

/// A filesystem-safe stamp derived from an ISO timestamp.
fn stamp(generated_at: &str) -> String {
    generated_at
        .chars()
        .map(|c| if c == ':' || c == 'Z' { '-' } else { c })
        .collect()
}

/// Retain a run in history.
pub fn retain(entry: &HistoryEntry) -> std::io::Result<PathBuf> {
    let dir = history_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", stamp(&entry.generated_at)));
    std::fs::write(&path, format!("{}\n", to_pretty_json(entry)))?;
    Ok(path)
}

/// Read the accepted baseline, or `None` when there is not one yet.
pub fn load_baseline() -> Option<BenchSuite> {
    let path = data_dir().join(BASELINE);
    let raw = std::fs::read_to_string(path).ok()?;
    let entry: HistoryEntry = serde_json::from_str(&raw).ok()?;
    Some(entry.suite)
}

/// Accept the current run as the new baseline.
///
/// Deliberately a separate command from `run`. Promoting a run to baseline is the
/// act of saying "this cost is intended", and it should not be possible to do by
/// merely running the suite.
pub fn promote_baseline(entry: &HistoryEntry) -> std::io::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(BASELINE);
    std::fs::write(&path, format!("{}\n", to_pretty_json(entry)))?;
    Ok(path)
}

/// Every retained run, oldest first.
pub fn history() -> std::io::Result<Vec<HistoryEntry>> {
    let dir = history_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<HistoryEntry>(&raw) {
                entries.push(parsed);
            }
        }
    }
    entries.sort_by(|a, b| a.generated_at.cmp(&b.generated_at));
    Ok(entries)
}

/// The most recent retained run, if any.
pub fn latest() -> Option<HistoryEntry> {
    history().ok().and_then(|mut e| e.pop())
}

/// A point in a metric's history, for charting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Series {
    pub id: String,
    pub metric: String,
    /// `(timestamp, value)` pairs, oldest first.
    pub points: Vec<(String, i64)>,
}

/// Build chart series for one metric across all retained runs.
pub fn series(entries: &[HistoryEntry], group: Option<&str>, metric: &str) -> Vec<Series> {
    let mut out: Vec<Series> = Vec::new();
    for entry in entries {
        for result in &entry.suite.results {
            if group.is_some_and(|g| result.group != g) {
                continue;
            }
            let Some(value) = result.metric(metric) else { continue };
            match out.iter_mut().find(|s| s.id == result.id) {
                Some(s) => s.points.push((entry.generated_at.clone(), value)),
                None => out.push(Series {
                    id: result.id.clone(),
                    metric: metric.to_string(),
                    points: vec![(entry.generated_at.clone(), value)],
                }),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Measurement;
    use crate::suite::BenchResult;
    use std::collections::BTreeMap;

    fn suite_with(instructions: i64) -> BenchSuite {
        let r = BenchResult::measured(
            "scenario:single_event/ledger=100",
            "scenario",
            "single_event",
            BTreeMap::new(),
            Measurement {
                instructions,
                write_entries: 3,
                ..Default::default()
            },
        );
        BenchSuite {
            schema: crate::suite::SCHEMA,
            name: "t".into(),
            sdk_version: "27.0.6".into(),
            model: "m".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            results: vec![r],
        }
    }

    #[test]
    fn entries_round_trip_through_json() {
        let e = HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1000)).with_revision("abc123");
        let json = to_pretty_json(&e);
        let back: HistoryEntry = serde_json::from_str(&json).expect("history entry should round-trip");
        assert_eq!(back.generated_at, e.generated_at);
        assert_eq!(back.revision.as_deref(), Some("abc123"));
        assert_eq!(back.suite.results[0].metric("instructions"), Some(1000));
    }

    #[test]
    fn stamps_are_filesystem_safe() {
        let s = stamp("2026-01-01T12:34:56Z");
        assert!(!s.contains(':'), "colons are not portable in filenames: {s}");
        assert!(!s.contains('Z'));
    }

    #[test]
    fn json_output_is_pretty_and_diffable() {
        let json = to_pretty_json(&HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1)));
        assert!(json.contains("\n  "), "output should be indented for review in a diff");
    }

    #[test]
    fn series_accumulates_one_metric_across_runs() {
        let runs = vec![
            HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1000)),
            HistoryEntry::new("2026-01-02T00:00:00Z", suite_with(1200)),
        ];
        let s = series(&runs, Some("scenario"), "instructions");
        assert_eq!(s.len(), 1);
        assert_eq!(
            s[0].points,
            vec![
                ("2026-01-01T00:00:00Z".to_string(), 1000),
                ("2026-01-02T00:00:00Z".to_string(), 1200),
            ]
        );
    }

    #[test]
    fn series_can_be_filtered_by_group() {
        let runs = vec![HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1000))];
        assert_eq!(series(&runs, Some("scenario"), "instructions").len(), 1);
        assert_eq!(series(&runs, Some("wasm"), "instructions").len(), 0);
    }

    #[test]
    fn series_omits_metrics_a_run_did_not_record() {
        let runs = vec![HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1000))];
        assert!(
            series(&runs, None, "total_bytes").is_empty(),
            "an unrecorded metric has no series"
        );
    }
}
