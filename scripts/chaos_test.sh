#!/usr/bin/env bash
# scripts/chaos_test.sh
#
# Security chaos engineering test runner for AuditLedger.
# Runs contract-level chaos tests against a live testnet or local Soroban node.
#
# Prerequisites:
#   - soroban CLI installed and on PATH
#   - Rust toolchain with wasm32 target
#   - CONTRACT_ID, SOROBAN_SECRET_KEY env vars set
#   - NETWORK defaults to "testnet"
#
# Usage:
#   export CONTRACT_ID=<contract_id>
#   export SOROBAN_SECRET_KEY=<owner_secret>
#   ./scripts/chaos_test.sh
#
# Optional environment variables:
#   NETWORK      - Stellar network passphrase (default: testnet)
#   RPC_URL      - Soroban RPC URL (default: https://soroban-testnet.stellar.org)
#   CHAOS_CATEGORY - Run only a specific category: key_rotation, permissions, recovery

set -euo pipefail

: "${CONTRACT_ID:?CONTRACT_ID is required}"
: "${SOROBAN_SECRET_KEY:?SOROBAN_SECRET_KEY is required}"

NETWORK="${NETWORK:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
CHAOS_CATEGORY="${CHAOS_CATEGORY:-all}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=============================================="
echo " AuditLedger Chaos Engineering Tests"
echo " Network : $NETWORK"
echo " Contract: $CONTRACT_ID"
echo " Category: $CHAOS_CATEGORY"
echo "=============================================="
echo ""

cd "$PROJECT_DIR"

# Helper: run a specific chaos test category
run_category() {
    local category="$1"
    echo "--- Running chaos category: $category ---"
    cargo test chaos_${category} -- --nocapture
    local status=$?
    if [ $status -ne 0 ]; then
        echo "FAILED: chaos category $category exited with code $status"
        return $status
    fi
    echo "PASSED: chaos category $category"
    echo ""
    return 0
}

# Run chaos tests by category
case "$CHAOS_CATEGORY" in
    key_rotation)
        run_category "key_rotation"
        ;;
    permissions)
        run_category "permission_change"
        ;;
    recovery)
        run_category "recovery"
        ;;
    all)
        run_category "key_rotation"
        run_category "permission_change"
        run_category "recovery"
        run_category "metadata_schema"
        run_category "event_cap"
        run_category "ttl"
        run_category "nonce"
        run_category "rate_limit"
        ;;
    *)
        echo "Unknown chaos category: $CHAOS_CATEGORY"
        echo "Valid categories: key_rotation, permissions, recovery, all"
        exit 1
        ;;
esac

echo "=============================================="
echo " Chaos Engineering Tests Complete"
echo "=============================================="
