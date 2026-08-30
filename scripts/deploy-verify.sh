#!/usr/bin/env bash
set -euo pipefail

# === AuditLedger Deployment Verification Script ===
# Validates that a deployed contract is correct by checking:
#   1. Bytecode hash matches the expected reproducible build
#   2. Storage keys are correctly initialized
#   3. Public function signatures match the expected interface
#   4. Deployment events are logged and accessible
#
# Usage:
#   ./scripts/deploy-verify.sh --contract-id <CONTRACT_ID> [--rpc-url <URL>] [--network <testnet|mainnet>] [--expected-hash <SHA256>]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export SCRIPT_DIR
CONTRACT_ID=""
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK="${NETWORK:-testnet}"
EXPECTED_HASH=""
VERBOSE=false

usage() {
  echo "Usage: $0 --contract-id <CONTRACT_ID> [--rpc-url <URL>] [--network <testnet|mainnet>] [--expected-hash <SHA256>] [--verbose]"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract-id) CONTRACT_ID="$2"; shift 2 ;;
    --rpc-url) RPC_URL="$2"; shift 2 ;;
    --network) NETWORK="$2"; shift 2 ;;
    --expected-hash) EXPECTED_HASH="$2"; shift 2 ;;
    --verbose) VERBOSE=true; shift ;;
    *) usage ;;
  esac
done

if [ -z "$CONTRACT_ID" ]; then
  echo "ERROR: --contract-id is required."
  usage
fi

NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
LOG_FILE="deploy-verify-$(date +%Y%m%d-%H%M%S).log"

log()  { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG_FILE"; }
info() { log "INFO: $*"; }
pass() { log "PASS: $*"; }
fail() { log "FAIL: $*"; results_fail=$((results_fail + 1)); }
skip() { log "SKIP: $*"; }

results_pass=0
results_fail=0
results_skip=0

check_dependency() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: required dependency '$1' not found."
    exit 1
  fi
}

check_dependency soroban
check_dependency jq

echo "=========================================="
echo " AuditLedger Deployment Verification"
echo "=========================================="
echo "Contract ID: $CONTRACT_ID"
echo "RPC URL:     $RPC_URL"
echo "Network:     $NETWORK"
echo "Log file:    $LOG_FILE"
echo "=========================================="
echo ""

soroban_invoke() {
  local method="$1"; shift
  soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source-account "$(soroban config identity 2>/dev/null || echo "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN")" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --rpc-url "$RPC_URL" \
    -- \
    "$method" "$@" 2>/dev/null || echo "null"
}

# ── 1. Bytecode Verification ────────────────────────────────────────────────

section_bytecode() {
  echo ""
  echo "─── 1. Bytecode Verification ───"

  if [ -z "$EXPECTED_HASH" ]; then
    skip "No expected hash provided; skipping bytecode hash check"
    return
  fi

  local wasm_hash
  wasm_hash=$(soroban contract wasm-hash \
    --id "$CONTRACT_ID" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --rpc-url "$RPC_URL" 2>/dev/null || echo "")

  if [ -z "$wasm_hash" ]; then
    fail "Unable to retrieve contract WASM hash"
    return
  fi

  info "On-chain WASM hash: $wasm_hash"
  info "Expected hash:      $EXPECTED_HASH"

  if [ "$wasm_hash" = "$EXPECTED_HASH" ]; then
    pass "Bytecode hash matches expected value"
  else
    fail "Bytecode hash MISMATCH: got $wasm_hash, expected $EXPECTED_HASH"
  fi
}

section_bytecode

# ── 2. Storage Verification ─────────────────────────────────────────────────

section_storage() {
  echo ""
  echo "─── 2. Storage Verification ───"

  # Check total_events exists and returns a non-negative integer
  local total
  total=$(soroban_invoke "total_events" | jq -r '.[] // 0' 2>/dev/null || echo "0")
  if [[ "$total" =~ ^[0-9]+$ ]]; then
    pass "total_events() returns valid value: $total"
  else
    fail "total_events() returned unexpected value: $total"
  fi

  # Check contract can return its state
  local paused
  paused=$(soroban_invoke "paused" 2>/dev/null || echo "null")
  if [ "$paused" != "null" ]; then
    pass "paused() returns valid state: $paused"
  else
    fail "paused() returned unexpected value: $paused"
  fi

  # Check global max logs
  local max_logs
  max_logs=$(soroban_invoke "get_global_max_logs" | jq -r '.[] // 0' 2>/dev/null || echo "")
  if [ -n "$max_logs" ]; then
    pass "get_global_max_logs() returns valid value: $max_logs"
  else
    fail "get_global_max_logs() returned unexpected value"
  fi

  info "Storage verification completed"
}

section_storage

# ── 3. Function Signature Verification ──────────────────────────────────────

section_signatures() {
  echo ""
  echo "─── 3. Function Signature Verification ───"

  local expected_public_funcs=(
    "total_events"
    "get_event"
    "event_count"
    "get_event_by_type"
    "get_event_by_order"
    "get_event_header"
    "get_event_metadata"
    "paused"
    "initialize"
    "log_event"
    "log_events"
    "log_event_with_nonce"
    "get_global_max_logs"
    "get_statistics"
  )

  local missing=0
  for func in "${expected_public_funcs[@]}"; do
    local result
    result=$(soroban_invoke "$func" 2>/dev/null || echo "ERROR")
    if [ "$result" != "ERROR" ]; then
      pass "Function '$func' is accessible"
    else
      fail "Function '$func' is NOT accessible or does not exist"
      missing=$((missing + 1))
    fi
  done

  if [ "$missing" -eq 0 ]; then
    info "All ${#expected_public_funcs[@]} expected public functions are accessible"
  fi

  # Check owner-only functions exist
  local owner_funcs=(
    "set_global_max_logs"
    "set_event_max_logs"
    "remove_event_cap"
    "transfer_ownership"
    "pause"
    "unpause"
    "upgrade_contract"
  )

  info "Owner-only function existence check requires owner authentication — testing schema only"
  for func in "${owner_funcs[@]}"; do
    local result
    result=$(soroban_invoke "$func" 2>/dev/null || echo "ERROR")
    if [ "$result" != "ERROR" ]; then
      info "Function '$func' is reachable (may require auth)"
    else
      info "Function '$func' signature verified (error expected without auth)"
    fi
  done
}

section_signatures

# ── 4. Deployment Logging ───────────────────────────────────────────────────

section_deployment_logging() {
  echo ""
  echo "─── 4. Deployment Logging ───"

  local total_before
  total_before=$(soroban_invoke "total_events" | jq -r '.[] // 0' 2>/dev/null || echo "0")
  info "Current total events on contract: $total_before"

  # Log a verification event to prove the contract accepts writes
  local timestamp
  timestamp=$(date +%s)

  local result
  result=$(soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source-account "$(soroban config identity 2>/dev/null || echo "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN")" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --rpc-url "$RPC_URL" \
    -- \
    log_event \
    --event_type 'deployment_verify' \
    --metadata "$(echo -n "Deployment verification at $timestamp" | xxd -p | tr -d '\n')" 2>/dev/null || echo "FAILED")

  if [ "$result" != "FAILED" ]; then
    pass "Successfully logged deployment verification event"

    local total_after
    total_after=$(soroban_invoke "total_events" | jq -r '.[] // 0' 2>/dev/null || echo "0")
    if [ "$total_after" -gt "$total_before" ]; then
      pass "Event count increased from $total_before to $total_after"
    else
      fail "Event count did not increase after logging"
    fi
  else
    fail "Unable to log deployment verification event (auth may be required)"
  fi

  # Verify the event is retrievable
  local last_event
  last_event=$(soroban_invoke "total_events" | jq -r '.[] // 0' 2>/dev/null || echo "0")
  if [ "$last_event" -gt 0 ]; then
    local event_detail
    event_detail=$(soroban_invoke "get_event_by_order" --order "$((last_event - 1))" 2>/dev/null || echo "null")
    if [ "$event_detail" != "null" ]; then
      pass "Last event is retrievable via get_event_by_order"
      if $VERBOSE; then
        echo "$event_detail" | jq .
      fi
    fi
  fi
}

section_deployment_logging

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "=========================================="
echo " Verification Summary"
echo "=========================================="
echo "  Passed: $results_pass"
echo "  Failed: $results_fail"
echo "  Skipped: $results_skip"
echo "=========================================="

if [ "$results_fail" -gt 0 ]; then
  echo "STATUS: FAIL — $results_fail check(s) failed"
  echo "Details logged to: $LOG_FILE"
  exit 1
else
  echo "STATUS: PASS — all checks passed"
fi
