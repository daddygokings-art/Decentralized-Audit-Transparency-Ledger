//! `audit-ledger-bench` — the benchmark suite's command line.

use contract_bench::{
    history::{self, HistoryEntry},
    regress::{self, RegressionReport, THRESHOLD_PCT},
    report,
    suite::{self, BenchSuite},
    wasm,
};
use std::io::Write;
use std::process::ExitCode;

/// Everything the CLI can do.
const USAGE: &str = "\
audit-ledger-bench — benchmark suite for the AuditLedger contract

USAGE
  audit-ledger-bench <command> [file] [options]

COMMANDS
  run       Measure every scenario, compare against the baseline, retain history
  compare   Compare a results file against the baseline and fail on a regression
  report    Print a Markdown report for a retained run
  dashboard Emit a Grafana dashboard definition as JSON
  history   Chart a metric across retained runs
  promote   Accept the latest retained run as the new baseline
  wasm      Report WASM size and budget status
  export    Write Prometheus exposition text for a retained run
  help      Show this text

OPTIONS
  --threshold <pct>   Regression threshold, percent (default 5)
  --metric <name>     Metric to chart or report (default instructions)
  --output <path>     Write to a file instead of stdout
  --ascii             Plain-text table instead of SVG, for terminals
";

/// Parsed arguments.
struct Args {
    command: String,
    threshold: f64,
    metric: String,
    output: Option<String>,
    ascii: bool,
    /// Positional argument: the results file for `compare`, or the path to chart
    /// for `history`.
    path: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut command = "help".to_string();
    let mut threshold = THRESHOLD_PCT;
    let mut metric = report::HEADLINE_METRIC.to_string();
    let mut output = None;
    let mut ascii = false;
    let mut path = None;
    let mut seen_command = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "run" | "report" | "history" | "promote" | "wasm" | "export" | "compare" | "dashboard" | "help" => {
                command = arg.clone();
                seen_command = true;
            }
            // The first bare word after the command is its file or path argument.
            other if seen_command && !other.starts_with('-') && path.is_none() => {
                path = Some(other.to_string());
            }
            "--threshold" => {
                let v = it.next().ok_or("--threshold needs a value")?;
                threshold = v
                    .parse::<f64>()
                    .map_err(|_| format!("--threshold {v} is not a number"))?;
                if threshold <= 0.0 {
                    return Err("--threshold must be greater than zero".into());
                }
            }
            "--metric" => metric = it.next().ok_or("--metric needs a value")?,
            "--output" => output = Some(it.next().ok_or("--output needs a path")?),
            "--ascii" => ascii = true,
            "-h" | "--help" => command = "help".into(),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        command,
        threshold,
        metric,
        output,
        ascii,
        path,
    })
}

/// An ISO timestamp. Kept local so a run is reproducible in a fixed environment.
fn now_iso() -> String {
    match std::process::Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".into(),
    }
}

/// The current git revision, recorded so a history point can be traced to a commit.
fn revision() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn emit(text: &str, output: Option<&str>) -> std::io::Result<()> {
    match output {
        Some(path) => {
            std::fs::write(path, text)?;
            eprintln!("wrote {path}");
        }
        None => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            writeln!(lock, "{text}")?;
        }
    }
    Ok(())
}

/// A plain-text rendering of a chart, for terminals that cannot show SVG.
fn ascii_chart(series: &[contract_bench::history::Series], metric: &str) -> String {
    let peak = series
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.1))
        .max()
        .unwrap_or(1)
        .max(1);
    let width = 40usize;
    let mut out = format!("{metric} (peak {peak})\n");
    for s in series {
        let last = s.points.last().map(|p| p.1).unwrap_or(0);
        let bars = (last as f64 / peak as f64 * width as f64).round() as usize;
        out.push_str(&format!("{:<44}{:<10}{}\n", s.id, last, "#".repeat(bars.max(1))));
    }
    out
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &Args) -> Result<ExitCode, String> {
    match args.command.as_str() {
        "help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }

        "run" => cmd_run(args),

        // The gate needs to compare a results file it was handed, without
        // re-measuring: CI collects the run once and checks it separately, and a
        // script that greps a report for annotations finds none, because
        // annotations only exist where a comparison happened.
        "compare" => cmd_compare(args),

        "report" => {
            let entry = latest_entry()?;
            let regression = regression_for(&entry.suite)?;
            let text = report::report(&entry.suite, Some(&regression), Some(&wasm::discover()));
            emit(&text, args.output.as_deref()).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }

        // A dashboard is a separate artifact from the Markdown report: it is
        // imported into Grafana rather than read by a person.
        "dashboard" => {
            let text = report::grafana_dashboard();
            emit(&text, args.output.as_deref()).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }

        "history" => {
            let entries = history::history().map_err(|e| e.to_string())?;
            if entries.is_empty() {
                println!("no runs retained yet; run `audit-ledger-bench run` first");
                return Ok(ExitCode::SUCCESS);
            }
            let series = history::series(&entries, Some("scenario"), &args.metric);
            let text = if args.ascii {
                ascii_chart(&series, &args.metric)
            } else {
                report::history_chart(&entries, &args.metric)
            };
            emit(text.trim_end(), args.output.as_deref()).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }

        "promote" => {
            let entry = latest_entry()?;
            let path = history::promote_baseline(&entry).map_err(|e| e.to_string())?;
            println!(
                "baseline updated from the run of {} ({} results)",
                entry.generated_at,
                entry.suite.results.len()
            );
            println!("  {}", path.display());
            Ok(ExitCode::SUCCESS)
        }

        "wasm" => {
            let report = wasm::discover();
            let check = wasm::check_budget(&report, &wasm::AUDIT_LEDGER_BUDGET);
            let mut text = String::new();
            if report.available {
                text.push_str(&format!("total: {} bytes\n", report.total_bytes.unwrap_or(0)));
                for s in &report.sections {
                    text.push_str(&format!("  {:<12} {}\n", s.name, s.size));
                }
                if let Some(name) = &report.module_name {
                    text.push_str(&format!("module: {name}\n"));
                }
            } else {
                text.push_str(&format!(
                    "unavailable: {}\n",
                    report.unavailable_reason.unwrap_or_default()
                ));
            }
            text.push_str(&format!("{}\n", report::budget_line(&check)));
            emit(text.trim_end(), args.output.as_deref()).map_err(|e| e.to_string())?;
            // An unenforced budget is not a pass, but it is also not a failure of
            // the contract: it is a signal that the artifact was not built.
            Ok(if check.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }

        "export" => {
            let entry = latest_entry()?;
            let text = report::prometheus(&entry.suite);
            emit(text.trim_end(), args.output.as_deref()).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }

        other => Err(format!("unknown command {other}")),
    }
}

fn cmd_run(args: &Args) -> Result<ExitCode, String> {
    let stamp = now_iso();
    let suite = suite::run(&stamp)?;

    let entry = match revision() {
        Some(r) => HistoryEntry::new(&stamp, suite).with_revision(r),
        None => HistoryEntry::new(&stamp, suite),
    };

    let baseline = history::load_baseline();
    let regression = match &baseline {
        Some(b) => RegressionReport::new(args.threshold, regress::compare(&entry.suite, b, args.threshold)),
        None => RegressionReport::new(args.threshold, Vec::new()),
    };

    let wasm_report = wasm::discover();
    let budget = wasm::check_budget(&wasm_report, &wasm::AUDIT_LEDGER_BUDGET);
    history::retain(&entry).map_err(|e| format!("cannot retain history: {e}"))?;

    println!(
        "{}",
        report::report(&entry.suite, Some(&regression), Some(&wasm_report))
    );
    println!("{}", report::budget_line(&budget));
    println!("history retained: {}", stamp);

    if !regression.annotations().is_empty() {
        for a in regression.annotations() {
            println!("{a}");
        }
    }

    match &baseline {
        None => {
            println!("no baseline on record; this run is now history, and `promote` will accept it");
            Ok(ExitCode::SUCCESS)
        }
        Some(_) if regression.passed() => {
            println!("no regression beyond {:.0}%", args.threshold);
            Ok(ExitCode::SUCCESS)
        }
        Some(_) => {
            eprintln!("{} regression(s) beyond {:.0}%", regression.regressions, args.threshold);
            Ok(ExitCode::from(1))
        }
    }
}

/// Compare a results file against the recorded baseline.
///
/// Exits non-zero when anything regressed past the threshold, so this can be
/// used directly as a build gate. Annotations go to stdout, because that is what
/// a CI log reads.
fn cmd_compare(args: &Args) -> Result<ExitCode, String> {
    let path = args
        .path
        .as_deref()
        .ok_or("compare needs the path to a results file: audit-ledger-bench compare <file>")?;
    let raw = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    if raw.trim().is_empty() {
        return Err(format!("{path} is empty; that is a broken pipeline, not a pass"));
    }

    // Accept either a bare suite or a retained history entry, because both are
    // written by this tool and a caller should not have to know which it has.
    let suite: BenchSuite = serde_json::from_str(&raw)
        .or_else(|_| serde_json::from_str::<HistoryEntry>(&raw).map(|e| e.suite))
        .map_err(|e| format!("cannot parse {path} as a benchmark suite: {e}"))?;

    let Some(baseline) = history::load_baseline() else {
        println!("no baseline on record; nothing to compare against");
        return Ok(ExitCode::SUCCESS);
    };

    let regression = RegressionReport::new(args.threshold, regress::compare(&suite, &baseline, args.threshold));
    for a in regression.annotations() {
        println!("{a}");
    }
    println!(
        "compared {} result(s) against the baseline at {:.0}%: {} regression(s), {} improvement(s), \
         {} added, {} removed, {} unavailable",
        suite.results.len(),
        args.threshold,
        regression.regressions,
        regression.improvements,
        regression.added,
        regression.removed,
        regression.unavailable
    );

    if regression.passed() {
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("{} regression(s) beyond {:.0}%", regression.regressions, args.threshold);
        Ok(ExitCode::from(1))
    }
}

fn latest_entry() -> Result<HistoryEntry, String> {
    history::latest().ok_or_else(|| "no retained run; run `audit-ledger-bench run` first".to_string())
}

/// Compare a suite against the stored baseline, if there is one.
fn regression_for(suite: &BenchSuite) -> Result<RegressionReport, String> {
    match history::load_baseline() {
        Some(b) => Ok(RegressionReport::new(
            THRESHOLD_PCT,
            regress::compare(suite, &b, THRESHOLD_PCT),
        )),
        None => Ok(RegressionReport::new(THRESHOLD_PCT, Vec::new())),
    }
}
