#!/usr/bin/env bash
set -euo pipefail

# === AuditLedger Restore Script ===
# Replays events from a backup file onto a contract instance.
#
# Usage:
#   ./restore.sh --backup backup.json [--config config.json] [--dry-run]
#
# Dependencies: soroban-cli, jq

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG_FILE="${SCRIPT_DIR}/config.json"
BACKUP_FILE=""
DRY_RUN=false

usage() {
    echo "Usage: $0 --backup <backup.json> [--config <config.json>] [--dry-run]"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --backup) BACKUP_FILE="$2"; shift 2 ;;
        --config) CONFIG_FILE="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        *) usage ;;
    esac
done

if [ -z "$BACKUP_FILE" ]; then
    echo "ERROR: --backup is required."
    usage
fi

if [ ! -f "$BACKUP_FILE" ]; then
    echo "ERROR: Backup file '$BACKUP_FILE' not found."
    exit 1
fi

# shellcheck disable=SC1090
source /dev/stdin <<<"$(cat <<RCFG
$(jq -r '
    "RPC_URL=\(.rpc_url)\nNETWORK=\(.network_passphrase)\nCONTRACT_ID=\(.contract_id)\nBATCH_SIZE=\(.max_batch_size)"
' "$CONFIG_FILE")"
RCFG

echo "=== AuditLedger Restore ==="
echo "Backup:     $BACKUP_FILE"
echo "Contract:   $CONTRACT_ID"
echo "RPC:        $RPC_URL"
echo "Dry run:    $DRY_RUN"
echo ""

TOTAL_BACKED_UP=$(jq '.total_events' "$BACKUP_FILE")
echo "Events in backup: $TOTAL_BACKED_UP"

if [ "$DRY_RUN" = true ]; then
    echo "DRY RUN: would restore $TOTAL_BACKED_UP events."
    exit 0
fi

# Check current on-chain count
echo "Checking current on-chain event count..."
CURRENT_COUNT=$(soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$(soroban config identity)" \
    --network-passphrase "$NETWORK" \
    --rpc-url "$RPC_URL" \
    -- \
    total_events 2>/dev/null | jq '.[]' || echo 0)

echo "Current on-chain: $CURRENT_COUNT events"

# Deduplication: skip events that already exist (by index)
SKIPPED=0
RESTORED=0

jq -c '.events[]' "$BACKUP_FILE" | while read -r evt; do
    INDEX=$(echo "$evt" | jq '.index')
    EVENT_TYPE=$(echo "$evt" | jq -r '.event_type')
    SUBMITTER=$(echo "$evt" | jq -r '.submitter')
    METADATA=$(echo "$evt" | jq -r '.metadata // ""')
    METADATA_HEX=$(echo -n "$METADATA" | xxd -p | tr -d '\n')

    # Check if the event is already on-chain
    if [ "$INDEX" -lt "$CURRENT_COUNT" ]; then
       SKIPPED=$((SKIPPED + 1))
       continue
    fi

    echo "Restoring event $INDEX: $EVENT_TYPE"

    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source "$(soroban config identity)" \
        --network-passphrase "$NETWORK" \
        --rpc-url "$RPC_URL" \
        -- \
        log_event \
        --submitter "$SUBMITTER" \
        --event_type "$EVENT_TYPE" \
        --metadata "$(echo -n "$METADATA" | base64)" \
        2>/dev/null || echo "WARNING: event $INDEX restore failed"

    RESTORED=$((RESTORED + 1))
done

echo ""
echo "Restore summary:"
echo "  Restored: $RESTORED"
echo "  Skipped:  $SKIPPED"
echo "Done."
