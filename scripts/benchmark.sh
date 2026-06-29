#!/usr/bin/env bash
# scripts/benchmark.sh
#
# Benchmark log_event (single) vs log_events (batched) throughput on Stellar testnet.
# Measures fee (XLM stroops) and approximate ops/ledger for batch sizes 1, 10, 50, 100.
#
# Requirements:
#   - soroban-cli installed and on PATH
#   - CONTRACT_ID, SOROBAN_SECRET_KEY (submitter), OWNER_SECRET_KEY env vars set
#   - NETWORK defaults to "testnet"
#
# Usage:
#   export CONTRACT_ID=<contract_id>
#   export SOROBAN_SECRET_KEY=<submitter_secret>
#   export OWNER_SECRET_KEY=<owner_secret>
#   ./scripts/benchmark.sh

set -euo pipefail

: "${CONTRACT_ID:?CONTRACT_ID is required}"
: "${SOROBAN_SECRET_KEY:?SOROBAN_SECRET_KEY is required}"
NETWORK="${NETWORK:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"

SUBMITTER_ADDRESS=$(soroban keys address "$SOROBAN_SECRET_KEY" 2>/dev/null || \
  stellar keys address "$SOROBAN_SECRET_KEY" 2>/dev/null)

SAMPLE_METADATA="$(echo -n 'benchmark-metadata-payload' | base64)"

echo "=============================================="
echo " AuditLedger Benchmark — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo " Contract : $CONTRACT_ID"
echo " Network  : $NETWORK"
echo "=============================================="
echo ""

# Helper: simulate a transaction and extract the fee in stroops
simulate_fee() {
  local output="$1"
  # soroban cli returns minResourceFee in the simulation output
  echo "$output" | grep -oE '"minResourceFee"\s*:\s*"?[0-9]+"?' | grep -oE '[0-9]+' | head -1
}

# ── Single log_event benchmark ──────────────────────────────────────────────
echo "--- Single log_event ---"
SINGLE_FEE=$(soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOROBAN_SECRET_KEY" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --fee 1000000 \
  --simulate-only \
  -- log_event \
  --submitter "$SUBMITTER_ADDRESS" \
  --event_type "benchmark" \
  --metadata "$SAMPLE_METADATA" \
  --category null \
  --sub_event_type null 2>&1 | grep -oE 'minResourceFee[^0-9]*[0-9]+' | grep -oE '[0-9]+' | head -1 || echo "N/A")

echo "  Batch size 1 (single):  fee = ${SINGLE_FEE} stroops"
echo ""

# ── Batched log_events benchmark ─────────────────────────────────────────────
echo "--- Batched log_events ---"

build_batch() {
  local size="$1"
  local items=""
  for i in $(seq 1 "$size"); do
    items="${items}[\"${SUBMITTER_ADDRESS}\",\"benchmark\",\"${SAMPLE_METADATA}\"]"
    [ "$i" -lt "$size" ] && items="${items},"
  done
  echo "[${items}]"
}

for BATCH_SIZE in 10 50 100; do
  BATCH_JSON="$(build_batch "$BATCH_SIZE")"
  BATCH_FEE=$(soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$SOROBAN_SECRET_KEY" \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --fee 1000000 \
    --simulate-only \
    -- log_events \
    --events "$BATCH_JSON" 2>&1 | grep -oE 'minResourceFee[^0-9]*[0-9]+' | grep -oE '[0-9]+' | head -1 || echo "N/A")

  if [ "$BATCH_FEE" != "N/A" ] && [ -n "$BATCH_FEE" ] && [ "$SINGLE_FEE" != "N/A" ] && [ -n "$SINGLE_FEE" ]; then
    PER_EVENT_FEE=$(( BATCH_FEE / BATCH_SIZE ))
    BASELINE=$(( SINGLE_FEE * BATCH_SIZE ))
    SAVINGS_PCT=$(( (BASELINE - BATCH_FEE) * 100 / BASELINE ))
    echo "  Batch size ${BATCH_SIZE}:  total fee = ${BATCH_FEE} stroops | per-event = ${PER_EVENT_FEE} stroops | savings vs ${BATCH_SIZE}× single = ${SAVINGS_PCT}%"
  else
    echo "  Batch size ${BATCH_SIZE}:  total fee = ${BATCH_FEE} stroops"
  fi
done

echo ""
echo "=============================================="
echo " Results summary"
echo "=============================================="
echo ""
echo "| Batch size | Mode        | Fee (stroops) | Per-event (stroops) |"
echo "|------------|-------------|---------------|---------------------|"
echo "| 1          | log_event   | ${SINGLE_FEE}            | ${SINGLE_FEE}                  |"
echo "| 10         | log_events  | see above     | see above           |"
echo "| 50         | log_events  | see above     | see above           |"
echo "| 100        | log_events  | see above     | see above           |"
echo ""
echo "Note: Re-run against a live testnet to capture real fee data."
echo "      Pipe output to docs/fees-benchmark.txt for archiving."
