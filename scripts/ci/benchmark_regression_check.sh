#!/usr/bin/env bash
# Performance Benchmark Regression Validator (#476 #480 #478 #477)
#
# Delegates to scripts/bench/detect_regression.py so the incumbent CI pipeline
# and the new contract-benchmarks workflow share exactly one regression
# implementation. $1 is a benchmark report JSON (or a directory containing
# benchmark-report.json). With no comparable input it passes by default, keeping
# the historical behaviour of the step.
set -euo pipefail

THRESHOLD_PCT="${BENCH_REGRESSION_THRESHOLD:-10}" # Maximum permitted increase

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DETECTOR="$SCRIPT_DIR/../bench/detect_regression.py"
BASELINE="${BENCH_BASELINE:-$REPO_ROOT/benchmarks/baseline.json}"

INPUT="${1:-}"

is_dir_like() { [ -d "$INPUT" ]; }

if [ -z "$INPUT" ]; then
    echo "Notice: no benchmark report supplied. Baseline checks passed by default."
    exit 0
fi

RESOLVED="$INPUT"
if is_dir_like; then
    RESOLVED="$INPUT/benchmark-report.json"
fi

if [ ! -f "$RESOLVED" ] || [ ! -s "$RESOLVED" ]; then
    echo "Notice: benchmark report '$RESOLVED' is empty or absent. Baseline checks passed by default."
    exit 0
fi

if [ ! -f "$BASELINE" ]; then
    echo "Notice: baseline '$BASELINE' not found; cannot compare. Baseline checks passed by default."
    exit 0
fi

echo "Evaluating benchmark results from: $RESOLVED"
echo "Against baseline:                   $BASELINE"
echo "Threshold:                          < ${THRESHOLD_PCT}%"

python3 "$DETECTOR" \
    --baseline "$BASELINE" \
    --current "$RESOLVED" \
    --threshold "$THRESHOLD_PCT" \
    --markdown-out /dev/stdout
