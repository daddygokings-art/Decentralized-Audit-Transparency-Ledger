#!/usr/bin/env bash
set -euo pipefail

RPC_URL=${1:-"https://soroban-testnet.stellar.org"}
echo "=== Running Decentralized Audit Ledger Synthetic Test Suite ==="
echo "Target Network / RPC: ${RPC_URL}"

python3 "$(dirname "$0")/synthetic-prober.py" --rpc-url "${RPC_URL}" --iterations 3 --interval 1
python3 "$(dirname "$0")/generate-sla-report.py"

echo "=== Synthetic Suite Execution Completed Successfully ==="
exit 0
