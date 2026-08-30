#!/usr/bin/env bash
# scripts/coverage.sh — Run cargo-llvm-cov, generate reports, enforce thresholds,
# and persist coverage trend data.
#
# Usage:
#   ./scripts/coverage.sh [OPTIONS]
#
# Options:
#   --html            Generate HTML report (default: enabled)
#   --no-html         Skip HTML report generation
#   --lcov            Generate LCOV report (default: enabled)
#   --no-lcov         Skip LCOV report generation
#   --json            Generate JSON summary (default: enabled)
#   --no-json         Skip JSON summary
#   --open            Open the HTML report in the default browser when done
#   --threshold-check Run the Python threshold checker after coverage (default: enabled)
#   --no-threshold    Skip threshold enforcement
#   --trend           Append the current run to coverage-trend.json (default: enabled)
#   --no-trend        Skip trend tracking
#   --output-dir DIR  Directory for generated reports (default: coverage/)
#   --help            Show this help and exit
#
# Environment variables:
#   COVERAGE_OUTPUT_DIR   Override the output directory (same as --output-dir)
#   SKIP_THRESHOLD_CHECK  Set to '1' to skip threshold enforcement
#   SKIP_TREND            Set to '1' to skip trend persistence
#
# Issue #495 — contract event test coverage reporting with thresholds
set -euo pipefail

# ── Helpers ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colour

log()    { echo -e "${BLUE}[coverage]${NC} $*"; }
ok()     { echo -e "${GREEN}[coverage]${NC} $*"; }
warn()   { echo -e "${YELLOW}[coverage]${NC} $*"; }
error()  { echo -e "${RED}[coverage]${NC} $*" >&2; }

die() { error "$*"; exit 1; }

# ── Defaults ──────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DO_HTML=1
DO_LCOV=1
DO_JSON=1
DO_OPEN=0
DO_THRESHOLD=${SKIP_THRESHOLD_CHECK:-0}   # 0 = run check; env override with 1 = skip
DO_TREND=${SKIP_TREND:-0}                 # 0 = run trend; env override with 1 = skip
OUTPUT_DIR="${COVERAGE_OUTPUT_DIR:-${REPO_ROOT}/coverage}"

# Flip env semantics: SKIP_* env=1 means skip → our DO_* flag should be 0.
[[ "$DO_THRESHOLD" == "1" ]] && DO_THRESHOLD=0 || DO_THRESHOLD=1
[[ "$DO_TREND"     == "1" ]] && DO_TREND=0     || DO_TREND=1

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --html)            DO_HTML=1 ;;
        --no-html)         DO_HTML=0 ;;
        --lcov)            DO_LCOV=1 ;;
        --no-lcov)         DO_LCOV=0 ;;
        --json)            DO_JSON=1 ;;
        --no-json)         DO_JSON=0 ;;
        --open)            DO_OPEN=1 ;;
        --threshold-check) DO_THRESHOLD=1 ;;
        --no-threshold)    DO_THRESHOLD=0 ;;
        --trend)           DO_TREND=1 ;;
        --no-trend)        DO_TREND=0 ;;
        --output-dir)      OUTPUT_DIR="$2"; shift ;;
        --output-dir=*)    OUTPUT_DIR="${1#*=}" ;;
        --help|-h)
            sed -n '2,/^set -/{ /^set -/d; s/^# \{0,3\}//; p }' "$0"
            exit 0 ;;
        *) die "Unknown option: $1" ;;
    esac
    shift
done

mkdir -p "${OUTPUT_DIR}"

# ── Pre-flight: check for cargo-llvm-cov ─────────────────────────────────────
if ! cargo llvm-cov --version &>/dev/null; then
    warn "cargo-llvm-cov not found. Attempting installation…"
    cargo install cargo-llvm-cov --locked \
        || die "Failed to install cargo-llvm-cov. Install it manually: cargo install cargo-llvm-cov"
fi

# ── Build the exclusion arguments from coverage.toml ─────────────────────────
# cargo-llvm-cov supports --ignore-filename-regex to skip files by regex.
# We read the [exclusions].files list from coverage.toml and combine into one regex.
EXCLUSION_REGEX=""
if [[ -f "${REPO_ROOT}/coverage.toml" ]]; then
    # Extract quoted strings from the files = [...] list
    while IFS= read -r line; do
        # Strip leading/trailing whitespace and quotes, skip comment lines
        file="${line//\"/}"
        file="${file//\'/}"
        file="${file//,/}"
        file="${file// /}"
        [[ -z "$file" || "$file" == \#* || "$file" == \[* || "$file" == files* ]] && continue
        [[ "$EXCLUSION_REGEX" == "" ]] && EXCLUSION_REGEX="$file" || EXCLUSION_REGEX="${EXCLUSION_REGEX}|${file}"
    done < <(awk '/^\[exclusions\]/,/^\[/' "${REPO_ROOT}/coverage.toml" \
              | grep -E '^\s+"' | sed 's/.*"\(.*\)".*/\1/')
fi

# Fallback default if parsing found nothing
if [[ -z "$EXCLUSION_REGEX" ]]; then
    EXCLUSION_REGEX="src/(fuzz|bench|proptest|comprehensive_fuzz|test|.*_tests)\.rs|tests/"
fi

log "Exclusion regex: ${EXCLUSION_REGEX}"

# ── Run cargo-llvm-cov ────────────────────────────────────────────────────────
log "Running tests with coverage instrumentation…"

LLVM_COV_ARGS=(
    llvm-cov
    --workspace
    --ignore-filename-regex "${EXCLUSION_REGEX}"
)

# Collect into a single run so we don't re-compile multiple times.
# We use --no-report first to build the raw data, then generate each format.
cd "${REPO_ROOT}"

cargo "${LLVM_COV_ARGS[@]}" \
    --no-report \
    -- --test-threads=1 2>&1 | tee "${OUTPUT_DIR}/test-output.log"

EXIT_CODE=${PIPESTATUS[0]}
if [[ $EXIT_CODE -ne 0 ]]; then
    error "Tests failed (exit code $EXIT_CODE). Coverage data may be incomplete."
    # Continue anyway so we still generate what we can for diagnostics.
fi

# ── Generate reports ──────────────────────────────────────────────────────────
if [[ $DO_JSON -eq 1 ]]; then
    log "Generating JSON summary…"
    cargo "${LLVM_COV_ARGS[@]}" \
        --json \
        --summary-only \
        --output-path "${OUTPUT_DIR}/coverage-summary.json" \
        2>/dev/null || warn "JSON summary generation failed"
    ok "JSON summary → ${OUTPUT_DIR}/coverage-summary.json"
fi

if [[ $DO_LCOV -eq 1 ]]; then
    log "Generating LCOV report…"
    cargo "${LLVM_COV_ARGS[@]}" \
        --lcov \
        --output-path "${OUTPUT_DIR}/lcov.info" \
        2>/dev/null || warn "LCOV generation failed"
    ok "LCOV report   → ${OUTPUT_DIR}/lcov.info"
fi

if [[ $DO_HTML -eq 1 ]]; then
    log "Generating HTML report…"
    cargo "${LLVM_COV_ARGS[@]}" \
        --html \
        --output-dir "${OUTPUT_DIR}/html" \
        2>/dev/null || warn "HTML report generation failed"
    ok "HTML report   → ${OUTPUT_DIR}/html/index.html"
fi

# Also generate a human-readable text summary to stdout / log file.
log "Generating text summary…"
cargo "${LLVM_COV_ARGS[@]}" \
    --summary-only \
    2>/dev/null | tee "${OUTPUT_DIR}/coverage-report.txt" || true

echo ""
log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cat "${OUTPUT_DIR}/coverage-report.txt"
log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Extract summary numbers for trend tracking ────────────────────────────────
LINE_PCT=""
BRANCH_PCT=""
if [[ -f "${OUTPUT_DIR}/coverage-summary.json" ]]; then
    LINE_PCT=$(python3 -c "
import json, sys
try:
    d = json.load(open('${OUTPUT_DIR}/coverage-summary.json'))
    totals = d.get('data', [{}])[0].get('totals', {})
    lines  = totals.get('lines',    {})
    print(f\"{lines.get('percent', 0):.2f}\")
except Exception as e:
    print('0.00')
" 2>/dev/null || echo "0.00")

    BRANCH_PCT=$(python3 -c "
import json, sys
try:
    d = json.load(open('${OUTPUT_DIR}/coverage-summary.json'))
    totals   = d.get('data', [{}])[0].get('totals', {})
    branches = totals.get('branches', {})
    print(f\"{branches.get('percent', 0):.2f}\")
except Exception as e:
    print('0.00')
" 2>/dev/null || echo "0.00")
fi

ok "Line coverage:   ${LINE_PCT}%"
ok "Branch coverage: ${BRANCH_PCT}%"

# ── Trend persistence ─────────────────────────────────────────────────────────
TREND_FILE="${REPO_ROOT}/coverage-trend.json"

if [[ $DO_TREND -eq 1 ]]; then
    log "Updating coverage trend…"
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    GIT_SHA=$(git -C "${REPO_ROOT}" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    GIT_REF=$(git -C "${REPO_ROOT}" symbolic-ref --short HEAD 2>/dev/null \
               || git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo "unknown")

    python3 - <<PYEOF
import json, os, sys
from pathlib import Path

trend_file = Path('${TREND_FILE}')
history_limit = 30  # from coverage.toml [global].trend_history_limit

# Load existing trend data
if trend_file.exists():
    try:
        trend = json.loads(trend_file.read_text())
    except json.JSONDecodeError:
        trend = {"runs": []}
else:
    trend = {"runs": []}

# Append new entry
entry = {
    "timestamp": "${TIMESTAMP}",
    "git_sha":   "${GIT_SHA}",
    "git_ref":   "${GIT_REF}",
    "line_pct":  float("${LINE_PCT}" or 0),
    "branch_pct": float("${BRANCH_PCT}" or 0),
}

# Try to load per-module data from JSON summary
summary_path = Path('${OUTPUT_DIR}/coverage-summary.json')
if summary_path.exists():
    try:
        summary = json.loads(summary_path.read_text())
        files_data = summary.get("data", [{}])[0].get("files", [])
        # Aggregate per-module from file list
        modules: dict[str, dict] = {}
        for f in files_data:
            name = f.get("filename", "")
            # strip leading path components to get src/module.rs
            rel = name
            for prefix in ["src/", "../src/"]:
                if prefix in name:
                    rel = name[name.index(prefix):]
                    break
            mod = rel.replace("src/", "").replace(".rs", "").replace("/", "::")
            summary_f = f.get("summary", {})
            lines_pct = summary_f.get("lines",    {}).get("percent", 0)
            branch_pct = summary_f.get("branches", {}).get("percent", 0)
            modules[mod] = {"line_pct": round(lines_pct, 2), "branch_pct": round(branch_pct, 2)}
        entry["modules"] = modules
    except Exception as e:
        pass

trend["runs"].append(entry)

# Keep only the last N entries
if len(trend["runs"]) > history_limit:
    trend["runs"] = trend["runs"][-history_limit:]

trend_file.write_text(json.dumps(trend, indent=2))
print(f"Trend file updated: {len(trend['runs'])} runs tracked")
PYEOF
    ok "Trend data     → ${TREND_FILE}"
fi

# ── Threshold enforcement ─────────────────────────────────────────────────────
THRESHOLD_EXIT=0
if [[ $DO_THRESHOLD -eq 1 ]]; then
    CHECKER="${SCRIPT_DIR}/check_coverage_thresholds.py"
    if [[ ! -f "$CHECKER" ]]; then
        warn "Threshold checker not found at ${CHECKER}. Skipping."
    elif [[ ! -f "${OUTPUT_DIR}/coverage-summary.json" ]]; then
        warn "No JSON summary at ${OUTPUT_DIR}/coverage-summary.json. Skipping threshold check."
    else
        log "Enforcing coverage thresholds…"
        python3 "${CHECKER}" \
            --summary "${OUTPUT_DIR}/coverage-summary.json" \
            --config  "${REPO_ROOT}/coverage.toml" \
            || THRESHOLD_EXIT=$?
    fi
fi

# ── Open report ───────────────────────────────────────────────────────────────
if [[ $DO_OPEN -eq 1 && $DO_HTML -eq 1 && -f "${OUTPUT_DIR}/html/index.html" ]]; then
    if command -v xdg-open &>/dev/null; then
        xdg-open "${OUTPUT_DIR}/html/index.html"
    elif command -v open &>/dev/null; then
        open "${OUTPUT_DIR}/html/index.html"
    fi
fi

# ── Final result ──────────────────────────────────────────────────────────────
echo ""
if [[ $EXIT_CODE -ne 0 ]]; then
    error "Tests failed. Fix failures before relying on coverage numbers."
    exit $EXIT_CODE
fi

if [[ $THRESHOLD_EXIT -ne 0 ]]; then
    error "Coverage thresholds not met. See output above."
    exit $THRESHOLD_EXIT
fi

ok "Coverage run complete."
ok "  Line:   ${LINE_PCT}%"
ok "  Branch: ${BRANCH_PCT}%"
ok "  Output: ${OUTPUT_DIR}/"
