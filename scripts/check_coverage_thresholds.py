#!/usr/bin/env python3
"""
scripts/check_coverage_thresholds.py

Parse a cargo-llvm-cov JSON summary report and enforce per-module coverage
thresholds defined in coverage.toml.

Exit codes:
  0  — all thresholds met
  1  — one or more thresholds violated
  2  — configuration / input error

Issue #495 — contract event test coverage reporting with thresholds
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

# ── Optional TOML support ─────────────────────────────────────────────────────
try:
    import tomllib  # Python 3.11+
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ImportError:
        tomllib = None  # type: ignore[assignment]

# ── Colour helpers ────────────────────────────────────────────────────────────
_COLOURS = sys.stdout.isatty()


def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _COLOURS else text


def red(t: str) -> str:    return _c("0;31", t)
def green(t: str) -> str:  return _c("0;32", t)
def yellow(t: str) -> str: return _c("1;33", t)
def bold(t: str) -> str:   return _c("1",    t)
def dim(t: str) -> str:    return _c("2",    t)


# ── TOML fallback parser ──────────────────────────────────────────────────────
def _parse_toml_fallback(text: str) -> dict[str, Any]:
    """Minimal hand-rolled TOML parser for the subset used in coverage.toml.

    Supports:
    - [section] and [[array-of-tables]]
    - key = value  (strings, floats, integers, lists of strings)
    - Inline comments (# ...)
    """
    result: dict[str, Any] = {}
    current_section: dict[str, Any] | None = None
    current_section_path: list[str] = []
    current_array_key: str | None = None

    for raw_line in text.splitlines():
        line = raw_line.strip()
        # Strip inline comments (naively — doesn't handle # inside strings)
        comment_pos = line.find(" #")
        if comment_pos != -1:
            line = line[:comment_pos].strip()
        if not line or line.startswith("#"):
            continue

        # [[array-of-tables]]
        if line.startswith("[[") and line.endswith("]]"):
            key = line[2:-2].strip()
            if key not in result:
                result[key] = []
            new_entry: dict[str, Any] = {}
            result[key].append(new_entry)
            current_section = new_entry
            current_section_path = [key]
            current_array_key = key
            continue

        # [section]
        if line.startswith("[") and line.endswith("]") and not line.startswith("[["):
            key = line[1:-1].strip()
            parts = key.split(".")
            d = result
            for part in parts:
                if part not in d:
                    d[part] = {}
                d = d[part]
            current_section = d
            current_section_path = parts
            current_array_key = None
            continue

        # key = value
        if "=" in line:
            k, _, v = line.partition("=")
            k = k.strip()
            v = v.strip()

            # Parse value
            parsed: Any
            if v.startswith('"') or v.startswith("'"):
                parsed = v.strip("\"'")
            elif v.startswith("["):
                # List of strings
                inner = v.strip("[]")
                parsed = [item.strip().strip("\"'") for item in inner.split(",") if item.strip()]
            elif v.lower() == "true":
                parsed = True
            elif v.lower() == "false":
                parsed = False
            else:
                try:
                    parsed = float(v) if "." in v else int(v)
                except ValueError:
                    parsed = v

            if current_section is not None:
                current_section[k] = parsed
            else:
                result[k] = parsed

    return result


# ── Config loading ────────────────────────────────────────────────────────────
def load_config(path: Path) -> dict[str, Any]:
    text = path.read_text()
    if tomllib is not None:
        return tomllib.loads(text)
    return _parse_toml_fallback(text)


# ── JSON summary loading ──────────────────────────────────────────────────────
def load_summary(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as e:
        print(f"ERROR: Could not parse JSON summary at {path}: {e}", file=sys.stderr)
        sys.exit(2)


# ── Per-file coverage extraction ──────────────────────────────────────────────
def extract_file_coverage(summary: dict[str, Any]) -> dict[str, dict[str, float]]:
    """Return {relative_path: {line_pct, branch_pct}} for every file in the report."""
    result: dict[str, dict[str, float]] = {}
    for data_block in summary.get("data", []):
        for file_entry in data_block.get("files", []):
            filename: str = file_entry.get("filename", "")
            # Normalise to a src/-relative path
            for marker in ["src/", "../src/"]:
                if marker in filename:
                    filename = "src/" + filename[filename.index(marker) + len(marker):]
                    break
            s = file_entry.get("summary", {})
            line_pct   = s.get("lines",    {}).get("percent", 0.0)
            branch_pct = s.get("branches", {}).get("percent", 0.0)
            result[filename] = {"line_pct": line_pct, "branch_pct": branch_pct}
    return result


def extract_totals(summary: dict[str, Any]) -> dict[str, float]:
    """Return overall {line_pct, branch_pct}."""
    totals = summary.get("data", [{}])[0].get("totals", {})
    return {
        "line_pct":   totals.get("lines",    {}).get("percent", 0.0),
        "branch_pct": totals.get("branches", {}).get("percent", 0.0),
    }


# ── Threshold checking ────────────────────────────────────────────────────────
def _pct_str(val: float, threshold: float) -> str:
    s = f"{val:6.2f}%"
    if val < threshold:
        return red(s)
    return green(s)


def check_thresholds(
    config: dict[str, Any],
    file_coverage: dict[str, dict[str, float]],
    totals: dict[str, float],
) -> tuple[bool, list[str]]:
    """
    Enforce thresholds from *config* against *file_coverage*.

    Returns (passed: bool, failure_messages: list[str]).
    """
    failures: list[str] = []
    globalcfg = config.get("global", {})
    global_line     = float(globalcfg.get("line_threshold", 60.0))
    global_branch   = float(globalcfg.get("branch_threshold", 0.0))
    hard_floor      = float(globalcfg.get("hard_floor", 50.0))

    # ── Hard floor on totals ────────────────────────────────────────────────
    total_line = totals["line_pct"]
    if total_line < hard_floor:
        failures.append(
            f"HARD FLOOR VIOLATED: overall line coverage {total_line:.2f}% "
            f"< {hard_floor:.2f}% (hard_floor)"
        )

    # ── Global totals ───────────────────────────────────────────────────────
    row_pass_char = "✓"
    row_fail_char = "✗"

    col_w = 40

    print()
    print(bold("Coverage Threshold Report"))
    print("=" * 72)
    print(f"  {'Module':<{col_w}}  {'Line':>7}  {'Threshold':>9}  {'Branch':>7}  Status")
    print("  " + "-" * 68)

    # Overall line
    status_overall = row_pass_char if total_line >= global_line else row_fail_char
    status_col     = green(status_overall) if status_overall == row_pass_char else red(status_overall)
    line_col_str   = _pct_str(total_line, global_line)
    print(f"  {'[overall]':<{col_w}}  {line_col_str}  {global_line:>8.1f}%  "
          f"{'n/a':>7}   {status_col}")
    if total_line < global_line:
        failures.append(
            f"[overall] line coverage {total_line:.2f}% < {global_line:.2f}% (global threshold)"
        )

    # ── Per-module thresholds ───────────────────────────────────────────────
    modules: list[dict[str, Any]] = config.get("module", [])
    matched_files: set[str] = set()

    for mod in modules:
        name        = mod.get("name", "?")
        path_prefix = mod.get("path_prefix", "")
        mod_line    = float(mod.get("line",   global_line))
        mod_branch  = float(mod.get("branch", global_branch))
        description = mod.get("description", "")

        # Find all files matching this module's path_prefix
        matching = {
            fp: cov for fp, cov in file_coverage.items()
            if fp.startswith(path_prefix) or path_prefix in fp
        }

        if not matching:
            # Module not found in coverage data — warn but don't fail
            print(f"  {name:<{col_w}}  {dim('  N/A  '):>7}  {mod_line:>8.1f}%  "
                  f"{'n/a':>7}   {dim('(not found)')}")
            continue

        # Aggregate: take the minimum across matching files (most conservative)
        min_line   = min(v["line_pct"]   for v in matching.values())
        min_branch = min(v["branch_pct"] for v in matching.values())

        line_ok   = min_line   >= mod_line
        branch_ok = (mod_branch == 0.0) or (min_branch >= mod_branch)

        status = row_pass_char if (line_ok and branch_ok) else row_fail_char
        status_col = green(status) if status == row_pass_char else red(status)

        line_str   = _pct_str(min_line, mod_line)
        branch_str = (
            _pct_str(min_branch, mod_branch)
            if mod_branch > 0.0
            else dim("   n/a ")
        )

        desc_suffix = f"  {dim(description)}" if description else ""
        print(f"  {name:<{col_w}}  {line_str}  {mod_line:>8.1f}%  "
              f"{branch_str}   {status_col}{desc_suffix}")

        if not line_ok:
            failures.append(
                f"[{name}] line coverage {min_line:.2f}% < {mod_line:.2f}% "
                f"(path: {path_prefix})"
            )
        if not branch_ok:
            failures.append(
                f"[{name}] branch coverage {min_branch:.2f}% < {mod_branch:.2f}% "
                f"(path: {path_prefix})"
            )

        matched_files.update(matching.keys())

    # ── Unmatched files warning ─────────────────────────────────────────────
    unmatched = set(file_coverage.keys()) - matched_files
    # Filter out excluded patterns (test/fuzz files)
    exclusions_cfg = config.get("exclusions", {})
    excluded_patterns: list[str] = exclusions_cfg.get("files", [])
    truly_unmatched = [
        fp for fp in sorted(unmatched)
        if not any(pat.rstrip("/") in fp for pat in excluded_patterns)
    ]
    if truly_unmatched:
        print()
        print(yellow(f"  Warning: {len(truly_unmatched)} file(s) not matched by any module threshold:"))
        for fp in truly_unmatched[:10]:
            cov = file_coverage[fp]
            print(f"    {fp}: {cov['line_pct']:.2f}% lines")
        if len(truly_unmatched) > 10:
            print(f"    … and {len(truly_unmatched) - 10} more")

    print("  " + "-" * 68)

    # ── Summary ─────────────────────────────────────────────────────────────
    print()
    if failures:
        print(red(f"  FAILED: {len(failures)} threshold violation(s):"))
        for f in failures:
            print(f"    {red('✗')} {f}")
        print()
        return False, failures
    else:
        print(green("  All coverage thresholds met ✓"))
        print()
        return True, []


# ── Entry point ───────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="Enforce per-module coverage thresholds from coverage.toml."
    )
    parser.add_argument(
        "--summary", "-s",
        required=True,
        type=Path,
        help="Path to the cargo-llvm-cov JSON summary file.",
    )
    parser.add_argument(
        "--config", "-c",
        default=Path("coverage.toml"),
        type=Path,
        help="Path to coverage.toml (default: ./coverage.toml).",
    )
    parser.add_argument(
        "--github-output",
        action="store_true",
        help="Emit GitHub Actions step-summary and output variables.",
    )
    parser.add_argument(
        "--output-markdown", "-m",
        type=Path,
        default=None,
        help="Write a Markdown coverage table to this file (for PR comments).",
    )
    args = parser.parse_args()

    # ── Load inputs ─────────────────────────────────────────────────────────
    if not args.summary.exists():
        print(f"ERROR: Summary file not found: {args.summary}", file=sys.stderr)
        return 2
    if not args.config.exists():
        print(f"ERROR: Config file not found: {args.config}", file=sys.stderr)
        return 2

    summary = load_summary(args.summary)
    config  = load_config(args.config)

    file_coverage = extract_file_coverage(summary)
    totals        = extract_totals(summary)

    # ── Run checks ──────────────────────────────────────────────────────────
    passed, failures = check_thresholds(config, file_coverage, totals)

    # ── Markdown output ─────────────────────────────────────────────────────
    if args.output_markdown:
        write_markdown_report(
            args.output_markdown, config, file_coverage, totals, failures
        )
        print(f"Markdown report written to {args.output_markdown}")

    # ── GitHub Actions outputs ───────────────────────────────────────────────
    if args.github_output:
        _emit_github_outputs(totals, passed)

    return 0 if passed else 1


def write_markdown_report(
    path: Path,
    config: dict[str, Any],
    file_coverage: dict[str, dict[str, float]],
    totals: dict[str, float],
    failures: list[str],
) -> None:
    """Write a Markdown table suitable for a GitHub PR comment."""
    lines: list[str] = []

    globalcfg = config.get("global", {})
    global_line = float(globalcfg.get("line_threshold", 60.0))

    total_line   = totals["line_pct"]
    total_branch = totals["branch_pct"]

    status_emoji = "✅" if not failures else "❌"
    lines.append(f"## {status_emoji} Coverage Report\n")
    lines.append(f"| Metric | Value | Threshold |")
    lines.append(f"|--------|-------|-----------|")
    lines.append(
        f"| **Line coverage**   | **{total_line:.2f}%** | {global_line:.1f}% |"
    )
    lines.append(
        f"| **Branch coverage** | {total_branch:.2f}% | — |"
    )
    lines.append("")

    modules: list[dict[str, Any]] = config.get("module", [])
    if modules:
        lines.append("### Per-Module Breakdown\n")
        lines.append("| Module | Line % | Threshold | Status |")
        lines.append("|--------|--------|-----------|--------|")

        for mod in modules:
            name        = mod.get("name", "?")
            path_prefix = mod.get("path_prefix", "")
            mod_line    = float(mod.get("line", global_line))

            matching = {
                fp: cov for fp, cov in file_coverage.items()
                if fp.startswith(path_prefix) or path_prefix in fp
            }
            if not matching:
                lines.append(f"| `{name}` | — | {mod_line:.1f}% | ⚪ not found |")
                continue

            min_line = min(v["line_pct"] for v in matching.values())
            ok_sym = "✅" if min_line >= mod_line else "❌"
            lines.append(f"| `{name}` | {min_line:.2f}% | {mod_line:.1f}% | {ok_sym} |")

    if failures:
        lines.append("\n### ❌ Threshold Violations\n")
        for f in failures:
            lines.append(f"- {f}")

    lines.append(
        "\n<details><summary>About this report</summary>\n\n"
        "Generated by `scripts/check_coverage_thresholds.py` using "
        "[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov). "
        "Thresholds are defined in `coverage.toml`.\n\n</details>"
    )

    path.write_text("\n".join(lines) + "\n")


def _emit_github_outputs(totals: dict[str, float], passed: bool) -> None:
    """Write to $GITHUB_OUTPUT and $GITHUB_STEP_SUMMARY if available."""
    import os

    line_pct   = totals["line_pct"]
    branch_pct = totals["branch_pct"]
    status     = "pass" if passed else "fail"

    # GITHUB_OUTPUT
    github_output = os.environ.get("GITHUB_OUTPUT", "")
    if github_output:
        with open(github_output, "a") as f:
            f.write(f"line_coverage={line_pct:.2f}\n")
            f.write(f"branch_coverage={branch_pct:.2f}\n")
            f.write(f"coverage_status={status}\n")

    # GITHUB_STEP_SUMMARY
    step_summary = os.environ.get("GITHUB_STEP_SUMMARY", "")
    if step_summary:
        emoji = "✅" if passed else "❌"
        with open(step_summary, "a") as f:
            f.write(f"## {emoji} Coverage Summary\n\n")
            f.write(f"| Metric | Value |\n")
            f.write(f"|--------|-------|\n")
            f.write(f"| Line coverage | **{line_pct:.2f}%** |\n")
            f.write(f"| Branch coverage | {branch_pct:.2f}% |\n")
            f.write(f"| Status | {status} |\n\n")


if __name__ == "__main__":
    sys.exit(main())
