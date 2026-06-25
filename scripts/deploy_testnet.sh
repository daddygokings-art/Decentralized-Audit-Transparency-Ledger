#!/usr/bin/env bash
set -euo pipefail

# deploy_testnet.sh
#
# Builds the WASM contract and deploys it to Stellar testnet using the Soroban CLI.
#
# Prerequisites:
#   - soroban CLI installed (see https://soroban.stellar.org/docs)
#   - Rust WASM target: rustup target add wasm32-unknown-unknown
#
# Usage:
#   export SOROBAN_SECRET_KEY="<your_secret_key>"
#   ./scripts/deploy_testnet.sh
#
# Optional environment variables:
#   WASM_DIR     – output directory for the WASM binary (default: target/wasm32-unknown-unknown/release)
#   NETWORK      – Stellar network passphrase (default: testnet)
#   RPC_URL      – Soroban RPC URL (default: https://soroban-testnet.stellar.org)

: "${SOROBAN_SECRET_KEY:?Must set SOROBAN_SECRET_KEY}"

WASM_DIR="${WASM_DIR:-target/wasm32-unknown-unknown/release}"
NETWORK="${NETWORK:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Building WASM contract..."
cargo build --target wasm32-unknown-unknown --release --manifest-path "$PROJECT_DIR/Cargo.toml"

WASM_FILE="$PROJECT_DIR/$WASM_DIR/audit_ledger.wasm"
if [ ! -f "$WASM_FILE" ]; then
    echo "ERROR: WASM file not found at $WASM_FILE"
    exit 1
fi

echo "==> Deploying to $NETWORK..."
soroban contract deploy \
    --wasm "$WASM_FILE" \
    --source "$SOROBAN_SECRET_KEY" \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL"

echo "==> Done."
