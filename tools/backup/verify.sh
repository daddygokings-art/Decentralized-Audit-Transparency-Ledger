#!/usr/bin/env bash
set -euo pipefail

# === AuditLedger Verification Script ===
# Compares on-chain events with a backup file and reports discrepancies.
#
# Usage:
#   ./verify.sh --backup backup.json [--config config.json]
#
# Dependencies: soroban-cli, jq

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG_FILE="${SCRIPT_DIR}/config.json"
BACKUP_FILE=""

usage() {
    echo "Usage: $0 --backup <backup.json> [--config <config.json>]"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --backup) BACKUP_FILE="$2"; shift 2 ;;
        --config) CONFIG_FILE="$2"; shift 2 ;;
        *) usage ;;
    esac
done

if [ -z "$BACKUP_FILE" ]; then
    echo "ERROR: --backup is required."
    usage
fi

# shellcheck disable=SC1090
source /dev/stdin <<<"$(cat <<RCFG
$(jq -r '
    "RPC_URL=\(.rpc_url)\nNETWORK=\(.network_passphrase)\nCONTRACT_ID=\(.contract_id)"
' "$CONFIG_FILE")"
RCFG

echo "=== AuditLedger Verification ==="
echo "Backup:     $BACKUP_FILE"
echo "Contract:   $CONTRACT_ID"
echo ""

TOTAL_BACKED_UP=$(jq '.total_events' "$BACKUP_FILE")

# Get on-chain totals
TOTAL_ONCHAIN=$(soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$(soroban config identity)" \
    --network-passphrase "$NETWORK" \
    --rpc-url "$RPC_URL" \
    -- \
    total_events 2>/dev/null | jq '.[]' || echo 0)

echo "Total events - Backup: $TOTAL_BACKED_UP, On-chain: $TOTAL_ONCHAIN"

if [ "$TOTAL_BACKED_UP" -ne "$TOTAL_ONCHAIN" ]; then
    echo "WARNING: Event count mismatch!"
fi

# Compare each event
MISMATCHES=0
MATCHES=0

jq -c '.events[]' "$BACKUP_FILE" | while read -r evt; do
    INDEX=$(echo "$evt" | jq '.index')
    EVENT_TYPE=$(echo "$evt" | jq -r '.event_type')
    SUBMITTER=$(echo "$evt" | jq -r '.submitter')
    METADATA=$(echo "$evt" | jq -r '.metadata // ""')

    ONCHAIN_EVT=$(soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source "$(soroban config identity)" \
        --network-passphrase "$NETWORK" \
        --rpc-url "$RPC_URL" \
        -- \
        get_event_by_order \
        --order "$INDEX" 2>/dev/null || echo "null")

    if [ "$ONCHAIN_EVT" = "null" ]; then
        echo "MISMATCH: Event $INDEX not found on-chain"
        MISMATCHES=$((MISMATCHES + 1))
        continue
    fi

    ONCHAIN_TYPE=$(echo "$ONCHAIN_EVT" | jq -r '.event_type')
    ONCHAIN_SUBMITTER=$(echo "$ONCHAIN_EVT" | jq -r '.submitter')
    ONCHAIN_METADATA=$(echo "$ONCHAIN_EVT" | jq -r '.metadata // ""')

    if [ "$EVENT_TYPE" != "$ONCHAIN_TYPE" ] || \
       [ "$SUBMITTER" != "$ONCHAIN_SUBMITTER" ]; then
        echo "MISMATCH: Event $INDEX differs"
        echo "  backup     type=$EVENT_TYPE submitter=$SUBMITTER"
        echo "  on-chain   type=$ONCHAIN_TYPE submitter=$ONCHAIN_SUBMITTER"
        MISMATCHES=$((MISMATCHES + 1))
    else
        MATCHES=$((MATCHES + 1))
    fi
done

echo ""
echo "Verification summary:"
echo "  Matches:    $MATCHES"
echo "  Mismatches: $MISMATCHES"

if [ "$MISMATCHES" -eq 0 ]; then
    echo "STATUS: PASS"
else
    echo "STATUS: FAIL"
    exit 1
fi
