//! Reporting: human-readable tables, charts, and metrics for external tooling.
//!
//! Three outputs, for three audiences, all derived from the same result data so
//! they cannot disagree:
//!
//! * a Markdown report for a reviewer reading a CI log or a pull request,
//! * an SVG chart, written as text with no dependencies, so history is visible in
//!   a pull request diff with no tooling and no external service,
//! * Prometheus exposition text, for pushing into an existing Grafana stack.
//!
//! A hand-rolled SVG writer is used rather than a charting dependency because the
//! suite has to run in a minimal CI container and stay reproducible; a chart that
//! renders slightly differently between runs is worse than no chart when the
//! output is committed to history.

use crate::history::{series, HistoryEntry, Series};
use crate::regress::RegressionReport;
use crate::suite::BenchSuite;
use crate::wasm::{self, BudgetCheck, WasmReport};
use std::fmt::Write as _;

/// Metric rendered in the main table, chosen because instruction count is what a
/// transaction fee is ultimately driven by.
pub const HEADLINE_METRIC: &str = "instructions";

/// Metrics shown as a breakdown, in the order a reader wants them.
pub const BREAKDOWN: [&str; 6] = [
    "instructions",
    "read_entries",
    "write_entries",
    "write_bytes",
    "mem_bytes",
    "rent_bumps",
];

/// A Markdown table of the suite's results.
pub fn markdown_table(suite: &BenchSuite) -> String {
    let mut out = String::new();
    let mut header = String::from("| result | params |");
    let mut rule = String::from("|---|---|");
    for m in BREAKDOWN {
        let _ = write!(header, " {m} |");
        let _ = write!(rule, "---:|");
    }
    out.push_str(&header);
    out.push('\n');
    out.push_str(&rule);
    out.push('\n');

    for group in ["scenario", "storage", "function", "wasm"] {
        let results = suite.group(group);
        if results.is_empty() {
            continue;
        }
        for r in results {
            let params = if r.params.is_empty() {
                String::new()
            } else {
                r.params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            // Function rows are numerous and one line each would swamp the table;
            // they are summarised by profile instead, below.
            if group == "function" {
                continue;
            }
            let _ = write!(out, "| `{}` | {} |", r.id, params);
            for m in BREAKDOWN {
                let _ = write!(
                    out,
                    " {} |",
                    r.metric(m).map(|v| v.to_string()).unwrap_or_else(|| "-".into())
                );
            }
            out.push('\n');
        }
    }
    out
}

/// A per-profile summary of the per-function coverage.
pub fn function_summary(suite: &BenchSuite) -> String {
    use std::collections::BTreeMap;
    let mut by_profile: BTreeMap<String, (usize, i64, i64)> = BTreeMap::new();
    for r in suite.group("function") {
        let profile = r.params.get("profile").cloned().unwrap_or_else(|| "?".into());
        let e = by_profile.entry(profile).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += r.metric(HEADLINE_METRIC).unwrap_or(0);
        e.2 += r.metric("write_entries").unwrap_or(0);
    }
    let mut out = String::from("| profile | functions | instructions | write_entries |\n|---|---:|---:|---:|\n");
    for (profile, (n, instr, writes)) in by_profile {
        let _ = writeln!(out, "| {profile} | {n} | {instr} | {writes} |");
    }
    out
}

/// A human-readable summary of a run.
pub fn report(suite: &BenchSuite, regression: Option<&RegressionReport>, wasm: Option<&WasmReport>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Contract benchmark report");
    let _ = writeln!(out);
    let _ = writeln!(out, "- generated: {}", suite.generated_at);
    let _ = writeln!(out, "- soroban-sdk: {}", suite.sdk_version);
    let _ = writeln!(out, "- results: {}", suite.results.len());
    let _ = writeln!(out);
    let _ = writeln!(out, "> {}", suite.model);
    let _ = writeln!(out);

    let _ = writeln!(out, "## Scenarios and storage primitives");
    let _ = writeln!(out);
    out.push_str(&markdown_table(suite));
    let _ = writeln!(out);

    let _ = writeln!(out, "## Per-function coverage");
    let _ = writeln!(out);
    out.push_str(&function_summary(suite));
    let _ = writeln!(out);

    if let Some(w) = wasm {
        let _ = writeln!(out, "## WASM size");
        let _ = writeln!(out);
        if w.available {
            let _ = writeln!(out, "- total: {} bytes", w.total_bytes.unwrap_or(0));
            for s in &w.sections {
                let _ = writeln!(out, "- {} section: {} bytes", s.name, s.size);
            }
        } else {
            let _ = writeln!(
                out,
                "- unavailable: {}",
                w.unavailable_reason.clone().unwrap_or_default()
            );
        }
        let _ = writeln!(out);
    }

    if let Some(r) = regression {
        let _ = writeln!(out, "## Regression check (threshold {}%)", r.threshold_pct);
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "regressions {} | improvements {} | added {} | removed {} | unavailable {}",
            r.regressions, r.improvements, r.added, r.removed, r.unavailable
        );
        let _ = writeln!(out);
        let notable = r.notable();
        if notable.is_empty() {
            let _ = writeln!(out, "No notable changes.");
        } else {
            for c in notable.iter().take(40) {
                let _ = writeln!(out, "- {}", c.describe());
            }
            if notable.len() > 40 {
                let _ = writeln!(out, "- ... and {} more", notable.len() - 40);
            }
        }
        let _ = writeln!(out);
    }
    out
}

/// Render a bar chart as standalone SVG.
///
/// Written by hand so the output is byte-stable across runs and needs no
/// rendering toolchain in CI. Series are normalised within a chart, so the reader
/// compares shapes rather than absolute values; the axis labels carry the numbers.
pub fn svg_chart(series: &[Series], title: &str) -> String {
    const W: usize = 900;
    const LEFT: usize = 320;
    const RIGHT: usize = 40;
    const TOP: usize = 48;
    const ROW: usize = 26;
    let height = TOP + series.len() * ROW + 40;
    let plot = W.saturating_sub(LEFT + RIGHT).max(1);

    let max = series
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.1))
        .max()
        .unwrap_or(1)
        .max(1);

    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}\" height=\"{height}\" viewBox=\"0 0 {W} {height}\" font-family=\"ui-monospace,SFMono-Regular,Menlo,monospace\" font-size=\"12\">"
    );
    let _ = write!(out, "<rect width=\"{W}\" height=\"{height}\" fill=\"#ffffff\"/>");
    let _ = write!(
        out,
        "<text x=\"16\" y=\"26\" font-size=\"15\" font-weight=\"600\" fill=\"#111\">{}</text>",
        escape(title)
    );
    let _ = write!(out, "<text x=\"{}\" y=\"26\" fill=\"#666\">peak {max}</text>", LEFT);

    if series.is_empty() {
        let _ = write!(
            out,
            "<text x=\"16\" y=\"{}\" fill=\"#666\">no history recorded yet</text>",
            TOP + 20
        );
        let _ = write!(out, "</svg>");
        return out;
    }

    for (i, s) in series.iter().enumerate() {
        let y = TOP + i * ROW;
        let label = escape(&short_id(&s.id));
        let _ = write!(out, "<text x=\"16\" y=\"{}\" fill=\"#333\">{}</text>", y + 14, label);

        // One segment per recorded point, laid end to end: the chart reads as a
        // timeline, so a single spike is visible in the shape of the bar.
        //
        // Each point is normalised against its own series' peak rather than the
        // chart-wide peak, so a metric whose absolute values dwarf every other
        // still shows its own trend. Delta metrics can be negative, so the
        // fraction is clamped: a negative point gets minimum width instead of
        // wrapping around to an enormous one.
        let peak = s.points.iter().map(|p| p.1).max().unwrap_or(1).max(1);
        let n = s.points.len().max(1);
        let seg = (plot / n).max(1);
        for (j, (stamp, value)) in s.points.iter().enumerate() {
            let frac = (*value as f64 / peak as f64).clamp(0.0, 1.0);
            let w = ((frac * seg as f64).round() as usize).max(1);
            let h = 16;
            // Later points darker, so progress through history is readable.
            let shade = 90 + (j * 120) / n.max(1);
            let _ = write!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" fill=\"rgb({shade},{shade},230)\"><title>{}: {}</title></rect>",
                LEFT + j * seg,
                y,
                escape(&short_stamp(stamp)),
                value
            );
        }
        let last = s.points.last().map(|p| p.1).unwrap_or(0);
        let _ = write!(
            out,
            "<text x=\"{}\" y=\"{}\" fill=\"#111\">{}</text>",
            LEFT,
            y + 30,
            last
        );
    }
    let _ = write!(out, "</svg>");
    out
}

/// A history chart across every retained run.
pub fn history_chart(entries: &[HistoryEntry], metric: &str) -> String {
    let s = series(entries, Some("scenario"), metric);
    svg_chart(
        &s,
        &format!("AuditLedger — {metric} by scenario, over {n} run(s)", n = entries.len()),
    )
}

fn short_id(id: &str) -> String {
    // The group is already the chart's subject, so drop the `group:` prefix. The
    // parameters after it are what tell two rows apart: a chart labelling
    // `batch_events/ledger=100/batch=4` and `batch_events/ledger=300/batch=16`
    // as just `batch_events` is two identical rows.
    let body = id.split_once(':').map_or(id, |(_, rest)| rest);
    // Truncate on a character boundary: a raw byte slice would panic on a
    // multi-byte character at the cut point.
    if body.chars().count() > 40 {
        let cut = body.char_indices().nth(39).map_or(body.len(), |(i, _)| i);
        format!("{}\u{2026}", &body[..cut])
    } else {
        body.to_string()
    }
}

fn short_stamp(stamp: &str) -> String {
    stamp.chars().take(10).collect()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Prometheus exposition text for the suite.
///
/// Emitted in the standard text format so it can be pushed to a Pushgateway, or
/// served as a scrape target, and graphed in the Grafana instance the project
/// already runs. Names are sanitised to Prometheus' `[a-zA-Z_:][a-zA-Z0-9_:]*`
/// rule, and the benchmark id becomes a label rather than part of the name so the
/// series stay joinable across runs.
pub fn prometheus(suite: &BenchSuite) -> String {
    let mut out = String::new();
    for r in &suite.results {
        for (metric, value) in &r.metrics {
            let safe = sanitize(&r.id);
            let m = sanitize(metric);
            let _ = writeln!(
                out,
                "# HELP audit_ledger_bench_{m} AuditLedger benchmark {metric} for {id}",
                id = r.id
            );
            let _ = writeln!(out, "# TYPE audit_ledger_bench_{m} gauge");
            let _ = writeln!(
                out,
                "audit_ledger_bench_{m}{{id=\"{safe}\",group=\"{group}\",name=\"{name}\",sdk=\"{sdk}\"}} {value}",
                group = r.group,
                name = sanitize(&r.name),
                sdk = sanitize(&suite.sdk_version),
            );
        }
    }
    out
}

/// Convert an arbitrary string to a Prometheus label or name fragment.
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

/// A Grafana dashboard definition for the metrics [`prometheus`] emits.
///
/// Written as JSON so it can be applied with `grafana-cli` or imported through the
/// API without a hand-built dashboard. Panes cover the headline instruction cost
/// per scenario, storage traffic, the regression threshold, and the WASM budget.
pub fn grafana_dashboard() -> String {
    let panels = r#"[
  {"id":1,"type":"timeseries","title":"Instructions per scenario","gridPos":{"h":9,"w":24,"x":0,"y":0},
   "targets":[{"expr":"audit_ledger_bench_instructions{group=\"scenario\"}","legendFormat":"{{name}} {{id}}"}]},
  {"id":2,"type":"timeseries","title":"Ledger entries written per scenario","gridPos":{"h":9,"w":12,"x":0,"y":9},
   "targets":[{"expr":"audit_ledger_bench_write_entries{group=\"scenario\"}","legendFormat":"{{name}}"}]},
  {"id":3,"type":"timeseries","title":"Bytes written per scenario","gridPos":{"h":9,"w":12,"x":12,"y":9},
   "targets":[{"expr":"audit_ledger_bench_write_bytes{group=\"scenario\"}","legendFormat":"{{name}}"}]},
  {"id":4,"type":"timeseries","title":"Metred memory per scenario","gridPos":{"h":9,"w":12,"x":0,"y":18},
   "targets":[{"expr":"audit_ledger_bench_mem_bytes{group=\"scenario\"}","legendFormat":"{{name}}"}]},
  {"id":5,"type":"stat","title":"WASM size against budget","gridPos":{"h":9,"w":12,"x":12,"y":18},
   "targets":[{"expr":"audit_ledger_bench_total_bytes{id=\"wasm_contract\"}"}],
   "fieldConfig":{"defaults":{"thresholds":{"steps":[{"color":"green","value":null},{"color":"red","value":65536}]},
   "unit":"bytes"}}},
  {"id":6,"type":"table","title":"Per-function instruction cost by profile","gridPos":{"h":10,"w":24,"x":0,"y":27},
   "targets":[{"expr":"audit_ledger_bench_instructions{group=\"function\"}","format":"table"}]}
]"#;
    format!(
        r#"{{
  "title": "AuditLedger contract benchmarks",
  "uid": "audit-ledger-bench",
  "schemaVersion": 39,
  "editable": true,
  "time": {{ "from": "now-30d", "to": "now" }},
  "templating": {{
    "list": [
      {{ "name": "sdk", "type": "query", "query": "label_values(audit_ledger_bench_instructions, sdk)", "includeAll": true, "current": {{"text":"All","value":"$__all"}} }}
    ]
  }},
  "panels": {panels}
}}
"#
    )
}

/// Render the WASM budget check as a one-line status.
pub fn budget_line(check: &BudgetCheck) -> String {
    if !check.enforced {
        return format!("wasm budget: NOT ENFORCED — {}", check.violations.join("; "));
    }
    if check.within_budget == Some(true) {
        return "wasm budget: within budget".to_string();
    }
    format!("wasm budget: EXCEEDED — {}", check.violations.join("; "))
}

/// Re-exported so the CLI can measure WASM without importing two modules.
pub fn measure_wasm() -> WasmReport {
    wasm::discover()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Measurement;
    use crate::suite::BenchResult;
    use std::collections::BTreeMap;

    fn suite_with(instructions: i64, writes: i64) -> BenchSuite {
        let r = BenchResult::measured(
            "scenario:single_event/ledger=100",
            "scenario",
            "single_event",
            BTreeMap::from([("ledger".to_string(), "100".to_string())]),
            Measurement {
                instructions,
                write_entries: writes as u32,
                mem_bytes: 512,
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
    fn markdown_table_has_a_cell_per_metric() {
        let md = markdown_table(&suite_with(1000, 3));
        assert!(md.contains("| result | params |"));
        for m in BREAKDOWN {
            assert!(md.contains(m), "table must label column {m}");
        }
        assert!(md.contains("scenario:single_event/ledger=100"));
        assert!(md.contains("ledger=100"), "params must be shown");
    }

    #[test]
    fn function_rows_are_summarised_not_listed() {
        let mut suite = suite_with(1000, 3);
        for i in 0..120 {
            let mut params = BTreeMap::new();
            params.insert("profile".to_string(), "append_event".to_string());
            params.insert("mutating".to_string(), (i % 2 == 0).to_string());
            suite.results.push(BenchResult::measured(
                format!("function:f{i}"),
                "function",
                format!("f{i}"),
                params,
                Measurement {
                    instructions: 500,
                    ..Default::default()
                },
            ));
        }
        let md = markdown_table(&suite);
        assert!(
            !md.contains("function:f0"),
            "individual functions must not each get a row"
        );
        let summary = function_summary(&suite);
        assert!(summary.contains("append_event | 120"), "{summary}");
    }

    #[test]
    fn report_states_the_model_limitation() {
        let suite = BenchSuite {
            model: "operation model: contract does not compile".into(),
            ..suite_with(1, 1)
        };
        let out = report(&suite, None, None);
        assert!(
            out.contains("does not compile"),
            "the report must carry the caveat: {out}"
        );
    }

    #[test]
    fn svg_is_wellformed_and_stable() {
        let runs = vec![
            HistoryEntry::new("2026-01-01T00:00:00Z", suite_with(1000, 3)),
            HistoryEntry::new("2026-01-02T00:00:00Z", suite_with(1200, 4)),
        ];
        let a = history_chart(&runs, "instructions");
        let b = history_chart(&runs, "instructions");
        assert_eq!(a, b, "chart output must be byte-stable for history to be reviewable");
        assert!(
            a.starts_with("<svg") && a.ends_with("</svg>"),
            "chart must be a single svg element"
        );
        assert_eq!(a.matches("<svg").count(), 1, "no nested svg elements");
        assert!(a.contains("scenario"), "series should be labelled: {a}");
    }

    #[test]
    fn chart_labels_keep_the_parameters_that_tell_rows_apart() {
        // Two batch rows that both label as "batch_events" cannot be told apart.
        let rows = [
            "scenario:batch_events/ledger=100/batch=4",
            "scenario:batch_events/ledger=300/batch=16",
        ];
        let labels: Vec<String> = rows.iter().map(|r| short_id(r)).collect();
        assert_eq!(labels[0], "batch_events/ledger=100/batch=4");
        assert_ne!(
            labels[0], labels[1],
            "rows with different parameters need different labels"
        );
    }

    #[test]
    fn a_long_id_is_truncated_without_panicking() {
        let long = format!("scenario:{}/ledger={}", "x".repeat(80), 300);
        let label = short_id(&long);
        assert!(label.chars().count() <= 41, "got {} chars", label.chars().count());
        assert!(label.ends_with('\u{2026}'), "a truncated label must be marked as such");
    }

    #[test]
    fn svg_escapes_untrusted_label_text() {
        let s = vec![Series {
            id: "x".into(),
            metric: "m".into(),
            points: vec![("<script>".to_string(), 1)],
        }];
        let svg = svg_chart(&s, "t");
        assert!(!svg.contains("<script>"), "labels must be escaped: {svg}");
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn svg_handles_an_empty_history() {
        let svg = svg_chart(&[], "nothing yet");
        assert!(svg.contains("no history recorded yet"), "{svg}");
    }

    #[test]
    fn prometheus_output_is_valid_exposition() {
        let text = prometheus(&suite_with(1000, 3));
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            assert!(
                line.contains('{') && line.contains('}'),
                "sample must be labelled: {line}"
            );
            let value = line.rsplit(' ').next().unwrap();
            assert!(value.parse::<i64>().is_ok(), "value must be an integer: {line}");
        }
        assert!(text.contains("# TYPE audit_ledger_bench_instructions gauge"));
    }

    #[test]
    fn prometheus_sanitises_ids() {
        let mut suite = suite_with(1, 1);
        suite.results[0].id = "scenario:single_event/ledger=100".into();
        let text = prometheus(&suite);
        assert!(text.contains("id=\"scenario_single_event_ledger_100\""), "{text}");
        assert!(
            !text.contains("id=\"scenario:single_event"),
            "raw punctuation must not reach a label"
        );
    }

    #[test]
    fn grafana_dashboard_is_valid_json() {
        let json = grafana_dashboard();
        serde_json::from_str::<serde_json::Value>(&json).expect("dashboard must be valid JSON");
    }

    #[test]
    fn budget_line_reflects_enforcement() {
        let report = wasm::measure(std::path::Path::new("/nonexistent/x.wasm"));
        let check = wasm::check_budget(&report, &wasm::AUDIT_LEDGER_BUDGET);
        assert!(budget_line(&check).contains("NOT ENFORCED"));
    }
}
