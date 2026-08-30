# Coverage Reporting

This document describes the test coverage system introduced in [issue #495](https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger/issues/495): how it works, how to run it locally, how thresholds and exclusions are configured, and how to interpret the results.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Configuration (`coverage.toml`)](#configuration-coveragetoml)
  - [Global settings](#global-settings)
  - [Exclusion rules](#exclusion-rules)
  - [Per-module thresholds](#per-module-thresholds)
- [Threshold Checker (`scripts/check_coverage_thresholds.py`)](#threshold-checker)
- [Coverage Script (`scripts/coverage.sh`)](#coverage-script)
- [GitHub Actions Workflow](#github-actions-workflow)
  - [PR comments](#pr-comments)
  - [Coverage trends](#coverage-trends)
  - [Artifacts](#artifacts)
- [Interpreting the Report](#interpreting-the-report)
- [Adjusting Thresholds](#adjusting-thresholds)
- [Troubleshooting](#troubleshooting)

---

## Overview

Coverage is measured using [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), which instruments the compiled binary with LLVM's source-based coverage and produces:

| Report format | Location | Purpose |
|---|---|---|
| JSON summary | `coverage/coverage-summary.json` | Machine-readable; consumed by threshold checker |
| LCOV | `coverage/lcov.info` | IDE plugins (VS Code Coverage Gutters, etc.) |
| HTML | `coverage/html/index.html` | Human browsing of individual lines |
| Text summary | `coverage/coverage-report.txt` | Quick CLI read |

Two metrics are tracked:

- **Line coverage** — percentage of executable lines executed during the test suite.
- **Branch coverage** — percentage of conditional branches (both sides of `if`/`match`/`&&`/`||`) taken at least once.

---

## Quick Start

### Prerequisites

```bash
# Rust stable toolchain with LLVM tools
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

Python 3.8+ is required for the threshold checker. No extra packages are needed (Python 3.11's built-in `tomllib` is used; `tomli` is used as a fallback on older versions; the script includes a hand-rolled TOML parser as a final fallback so no `pip install` is required).

### Run locally

```bash
# Full run: tests → HTML + LCOV + JSON + threshold check + trend update
./scripts/coverage.sh

# Open the HTML report in your browser when done
./scripts/coverage.sh --open

# Skip threshold enforcement (useful during WIP)
./scripts/coverage.sh --no-threshold

# Skip the HTML report (faster)
./scripts/coverage.sh --no-html
```

### Run threshold checker standalone

```bash
# Against the last generated summary
python3 scripts/check_coverage_thresholds.py \
    --summary coverage/coverage-summary.json \
    --config  coverage.toml

# Also write the PR comment markdown
python3 scripts/check_coverage_thresholds.py \
    --summary coverage/coverage-summary.json \
    --config  coverage.toml \
    --output-markdown coverage/pr-comment.md
```

---

## Configuration (`coverage.toml`)

All coverage settings live in [`coverage.toml`](../coverage.toml) at the repository root.

### Global settings

```toml
[global]
line_threshold   = 60.0   # Minimum line % required for the whole crate
branch_threshold = 0.0    # Minimum branch % required (0 = disabled)
hard_floor       = 50.0   # Absolute minimum; failing this is always an error
trend_history_limit = 30  # How many runs to keep in coverage-trend.json
```

| Key | Type | Description |
|---|---|---|
| `line_threshold` | float | Default minimum line coverage for any module not listed in `[[module]]`. |
| `branch_threshold` | float | Default minimum branch coverage; set to `0.0` to disable globally. |
| `hard_floor` | float | Hard lower bound for overall line coverage. A drop below this value always fails, even if individual module thresholds are met. |
| `trend_history_limit` | integer | Maximum number of historical data-points kept in `coverage-trend.json`. Older entries are pruned automatically. |

### Exclusion rules

```toml
[exclusions]
files = [
    "src/fuzz.rs",
    "src/bench.rs",
    "src/test.rs",
    # ...
]

functions = [
    "__call",
    "fmt",
    "clone",
]
```

The `files` list contains path fragments (matched against the path reported by `llvm-cov`). Any file whose path contains one of these strings is excluded from coverage measurement. This lets you exclude:

- **Test modules** (`src/*_tests.rs`, `tests/`) — we measure production code only.
- **Fuzz harnesses** (`src/fuzz.rs`, `src/comprehensive_fuzz.rs`) — these are entry points for `cargo-fuzz` and are not exercised by `cargo test`.
- **Benchmark harnesses** (`src/bench.rs`) — similarly not run by the unit test suite.
- **Proptest strategies** (`src/proptest.rs`) — declarative strategy definitions generate dead code branches by design.

The `functions` list is informational metadata used in documentation; `cargo-llvm-cov` does not support per-function exclusion via CLI flags in current versions. To exclude individual functions from coverage, use the `#[cfg(not(coverage))]` attribute or `// no coverage` comments on nightly.

### Per-module thresholds

Each `[[module]]` entry specifies an independent threshold for a source file or directory prefix:

```toml
[[module]]
name        = "supply_chain"
path_prefix = "src/supply_chain.rs"
line        = 65.0
branch      = 40.0
description = "Product provenance, certifications, chain of custody"
```

| Field | Required | Description |
|---|---|---|
| `name` | yes | Human-readable identifier used in reports and error messages. |
| `path_prefix` | yes | All files whose path starts with (or contains) this string are matched. |
| `line` | no | Minimum line coverage % for this module. Inherits `global.line_threshold` if omitted. |
| `branch` | no | Minimum branch coverage % for this module. Inherits `global.branch_threshold` (default `0.0` = disabled) if omitted. |
| `description` | no | Freetext description shown in the coverage table. |

**Matching semantics:** The first matching module entry wins. If a file is not matched by any `[[module]]` entry (and is not excluded), it contributes to the overall totals but is not checked against a dedicated threshold — the global threshold applies instead. Unmatched files are printed as a warning in the report.

---

## Threshold Checker

`scripts/check_coverage_thresholds.py` is a standalone Python script that:

1. Parses `coverage/coverage-summary.json` (produced by `cargo llvm-cov --json --summary-only`).
2. Loads thresholds from `coverage.toml`.
3. Matches each file in the coverage report to a `[[module]]` entry.
4. Prints a formatted table to stdout.
5. Optionally writes a Markdown table to a file (`--output-markdown`).
6. Optionally emits `GITHUB_OUTPUT` variables (`--github-output`).

### Exit codes

| Code | Meaning |
|---|---|
| `0` | All thresholds met. |
| `1` | One or more thresholds violated. |
| `2` | Configuration or input error (missing files, malformed TOML/JSON). |

### CLI reference

```
usage: check_coverage_thresholds.py [-h]
    --summary PATH
    [--config PATH]
    [--output-markdown PATH]
    [--github-output]
```

| Flag | Default | Description |
|---|---|---|
| `--summary` | — | Path to the JSON summary file (required). |
| `--config` | `./coverage.toml` | Path to `coverage.toml`. |
| `--output-markdown` | — | Write a PR-comment-ready Markdown file here. |
| `--github-output` | false | Set `GITHUB_OUTPUT` env vars and append to `GITHUB_STEP_SUMMARY`. |

---

## Coverage Script

`scripts/coverage.sh` orchestrates the full pipeline:

```
./scripts/coverage.sh [OPTIONS]
```

| Option | Default | Description |
|---|---|---|
| `--html` / `--no-html` | enabled | Generate HTML report. |
| `--lcov` / `--no-lcov` | enabled | Generate LCOV report. |
| `--json` / `--no-json` | enabled | Generate JSON summary. |
| `--open` | disabled | Open the HTML report after generation. |
| `--threshold-check` / `--no-threshold` | enabled | Run the Python threshold checker. |
| `--trend` / `--no-trend` | enabled | Append to `coverage-trend.json`. |
| `--output-dir DIR` | `./coverage/` | Directory for all generated files. |

Environment variable overrides:

| Variable | Effect |
|---|---|
| `COVERAGE_OUTPUT_DIR` | Equivalent to `--output-dir`. |
| `SKIP_THRESHOLD_CHECK=1` | Skip threshold enforcement. |
| `SKIP_TREND=1` | Skip trend persistence. |

---

## GitHub Actions Workflow

The workflow is defined in [`.github/workflows/coverage.yml`](../.github/workflows/coverage.yml) and runs on:

- Every push to `main`, `master`, or `develop` that touches Rust source, `Cargo.toml`, or the coverage configuration.
- Every pull request that touches those paths.
- Manual trigger (`workflow_dispatch`) with an optional `fail_on_threshold` input.

### Steps

```
Checkout → Install Rust (llvm-tools-preview) → Cache → Install cargo-llvm-cov
  → Run tests with coverage
  → Generate JSON summary
  → Generate LCOV report
  → Generate HTML report
  → Check coverage thresholds     ← continues-on-error so PR comment still posts
  → Write step summary
  → Update coverage trend         ← push events only
  → Upload artifacts
  → Post PR comment               ← PR events only
  → Fail if thresholds not met    ← respects fail_on_threshold input
```

### PR comments

On every PR run the workflow posts (or updates) a single bot comment containing:

- Overall line and branch coverage numbers.
- A per-module breakdown table with pass/fail status for each module.
- A list of any threshold violations.
- A link to the HTML report artifact.

The comment uses a hidden marker (`<!-- coverage-report-bot -->`) to identify and update an existing comment, so the PR does not accumulate duplicate coverage posts.

### Coverage trends

On pushes to `main`/`master`/`develop`, the workflow appends an entry to `coverage-trend.json`:

```json
{
  "runs": [
    {
      "timestamp":  "2026-08-29T15:45:41Z",
      "git_sha":    "a1b2c3d",
      "git_ref":    "main",
      "line_pct":   72.45,
      "branch_pct": 41.20,
      "run_id":     "9876543210",
      "run_number": "47"
    }
  ]
}
```

The file is uploaded as a workflow artifact alongside the coverage reports. To view the trend over time, download the artifact from any coverage run and inspect the JSON. The last 30 data-points are retained.

### Artifacts

| Artifact | Retention | Contents |
|---|---|---|
| `coverage-reports-<run_id>` | 30 days | `coverage-summary.json`, `lcov.info`, `coverage-report.txt`, `coverage-comment.md`, `coverage-trend.json` |
| `coverage-html-<run_id>` | 14 days | Full HTML report directory |

---

## Interpreting the Report

### Table columns

```
  Module                            Line    Threshold   Branch   Status
  ──────────────────────────────────────────────────────────────────────
  [overall]                        70.00%      60.0%       n/a   ✓
  core                             80.00%      75.0%     58.30%  ✓
  supply_chain                     50.00%      65.0%     37.50%  ✗
```

- **Module** — the `name` from the matching `[[module]]` entry, or `[overall]` for the whole crate.
- **Line** — the measured line coverage for the worst-covered file matching this module. Shown in green (✓) or red (✗).
- **Threshold** — the minimum acceptable value from `coverage.toml`.
- **Branch** — branch coverage (shown as `n/a` when the module threshold is `0.0` / disabled).
- **Status** — ✓ passed / ✗ failed.

### "not found" entries

A module listed in `coverage.toml` shows `N/A` and `(not found)` when its source file produced no instrumented data. This typically means:

- The file was excluded by the `--ignore-filename-regex` flag (check the exclusions list).
- The module's `path_prefix` is misspelled.
- The file exists but contains no instrumented code (e.g., pure `#[contracttype]` struct definitions with no function bodies).

"not found" entries do not count as failures — they are informational warnings.

---

## Adjusting Thresholds

To raise the bar for a specific module, edit its `[[module]]` entry in `coverage.toml`:

```toml
[[module]]
name = "supply_chain"
path_prefix = "src/supply_chain.rs"
line = 75.0   # raised from 65.0
branch = 50.0 # raised from 40.0
```

To lower a threshold temporarily while improving tests (without breaking CI):

```toml
[[module]]
name = "some_wip_module"
path_prefix = "src/some_wip_module.rs"
line = 40.0   # temporary; tracked in issue #NNN
```

Leave a comment explaining why and include an issue reference so the temporary relaxation is not forgotten.

### Adding a new module

When a new source file is added to the crate, add a corresponding `[[module]]` entry to `coverage.toml`. Without an entry the file is still measured and counted in the overall totals, but no dedicated per-module threshold is enforced.

Recommended starting threshold for a new module: **50–60% line coverage**. Raise it once the initial test suite is in place.

---

## Troubleshooting

### `cargo llvm-cov` not found

```
error: no such subcommand: `llvm-cov`
```

Install the tool:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

### `llvm-tools-preview` missing

```
error: Failed to find llvm-profdata. Install llvm-tools-preview: ...
```

```bash
rustup component add llvm-tools-preview
```

### `No JSON summary at coverage/coverage-summary.json`

The test run failed before `--json` could be generated. Check `coverage/test-output.log` for panics or compilation errors.

### Very low branch coverage reported

Branch coverage is naturally lower than line coverage — it requires both the `true` and `false` paths of every conditional to be exercised. Start by targeting line coverage and incrementally add branch-coverage thresholds as you add more test cases for error paths.

### Script exits with code 2 (config error)

The TOML fallback parser does not support all TOML features. If you use multi-line strings or advanced TOML syntax in `coverage.toml`, install `tomli`:

```bash
pip install tomli
```

On Python 3.11+, `tomllib` is part of the standard library and this is not necessary.

### Stale PR comment shows old numbers

The bot comment is identified by the hidden marker `<!-- coverage-report-bot -->`. If a previous comment was deleted manually, the next run will create a fresh one. If there are multiple bot comments (e.g., from different forks), the first one found is updated.
