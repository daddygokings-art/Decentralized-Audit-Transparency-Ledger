#!/usr/bin/env bash
set -euo pipefail

TOTAL_SHARDS="${TOTAL_SHARDS:-4}"
SHARD_INDEX="${SHARD_INDEX:-0}"
MAX_RETRIES="${MAX_RETRIES:-2}"
CARGO_BIN="${CARGO_BIN:-cargo}"
LOG_DIR="${LOG_DIR:-target/ci}"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

if ! [[ "$TOTAL_SHARDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TOTAL_SHARDS must be a positive integer; got: $TOTAL_SHARDS" >&2
  exit 2
fi

if ! [[ "$SHARD_INDEX" =~ ^[0-9]+$ ]]; then
  echo "SHARD_INDEX must be a non-negative integer; got: $SHARD_INDEX" >&2
  exit 2
fi

if (( SHARD_INDEX >= TOTAL_SHARDS )); then
  echo "SHARD_INDEX must be less than TOTAL_SHARDS; got shard $SHARD_INDEX of $TOTAL_SHARDS" >&2
  exit 2
fi

if ! [[ "$MAX_RETRIES" =~ ^[1-9][0-9]*$ ]]; then
  echo "MAX_RETRIES must be a positive integer; got: $MAX_RETRIES" >&2
  exit 2
fi

if ! [[ "$CARGO_BUILD_JOBS" =~ ^[1-9][0-9]*$ ]]; then
  echo "CARGO_BUILD_JOBS must be a positive integer; got: $CARGO_BUILD_JOBS" >&2
  exit 2
fi

if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
  echo "cargo not found on PATH; ensure the Rust toolchain is installed before running this script." >&2
  exit 127
fi

export CARGO_BUILD_JOBS
export RUST_TEST_THREADS=1

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/shard-${SHARD_INDEX}.log"
: > "$LOG_FILE"

list_tests() {
  local output
  output=$("$CARGO_BIN" test -- --list 2>&1) || {
    echo "$output" | tee -a "$LOG_FILE" >&2
    return 1
  }

  printf '%s\n' "$output" | awk '
    $1 == "test" && $2 != "result:" && $2 != "result" {
      name = $2
      sub(/:$/, "", name)
      if (length(name) > 0) print name
    }
  ' | awk 'NF'
}

mapfile -t all_tests < <(list_tests)

if (( ${#all_tests[@]} == 0 )); then
  echo "No tests discovered for shard $SHARD_INDEX/$TOTAL_SHARDS." | tee -a "$LOG_FILE"
  exit 0
fi

declare -a shard_counts=()
for ((i = 0; i < TOTAL_SHARDS; i++)); do
  shard_counts[i]=0
  shard_members[i]=""
done

declare -a shard_tests=()
for test_name in "${all_tests[@]}"; do
  best_shard=0
  for ((s = 1; s < TOTAL_SHARDS; s++)); do
    if (( shard_counts[s] < shard_counts[best_shard] )); then
      best_shard=$s
    fi
  done

  if [[ -n "${shard_members[best_shard]}" ]]; then
    shard_members[best_shard]+=$'\n'
  fi
  shard_members[best_shard]+="$test_name"
  shard_counts[best_shard]=$((shard_counts[best_shard] + 1))
done

if [[ -n "${shard_members[SHARD_INDEX]}" ]]; then
  mapfile -t shard_tests < <(printf '%s\n' "${shard_members[SHARD_INDEX]}")
else
  shard_tests=()
fi

if (( ${#shard_tests[@]} == 0 )); then
  echo "Shard $SHARD_INDEX/$TOTAL_SHARDS has no assigned tests." | tee -a "$LOG_FILE"
  exit 0
fi

echo "Running ${#shard_tests[@]} tests for shard $SHARD_INDEX/$TOTAL_SHARDS" | tee -a "$LOG_FILE"

run_test_with_retry() {
  local test_name="$1"
  local attempt=1

  while (( attempt <= MAX_RETRIES )); do
    echo "=== $test_name (shard ${SHARD_INDEX}/${TOTAL_SHARDS}, attempt ${attempt}/${MAX_RETRIES}) ===" | tee -a "$LOG_FILE"
    if "$CARGO_BIN" test "$test_name" -- --nocapture --test-threads=1 >> "$LOG_FILE" 2>&1; then
      echo "PASS: $test_name" | tee -a "$LOG_FILE"
      return 0
    fi

    if (( attempt == MAX_RETRIES )); then
      echo "FAIL: $test_name after ${MAX_RETRIES} attempts" | tee -a "$LOG_FILE"
      return 1
    fi

    echo "Flaky test detected for $test_name; retrying after previous failure." | tee -a "$LOG_FILE"
    attempt=$((attempt + 1))
  done
}

for test_name in "${shard_tests[@]}"; do
  run_test_with_retry "$test_name" || exit 1
done

echo "Shard $SHARD_INDEX/$TOTAL_SHARDS passed: ${#shard_tests[@]} tests" | tee -a "$LOG_FILE"
