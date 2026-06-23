#!/usr/bin/env bash
set -euo pipefail

# === AuditLedger Backup Script ===
# Exports all on-chain events to a local JSON file with metadata.
#
# Usage:
#   ./backup.sh [--config config.json] [--output backup.json]
#
# Dependencies: soroban-cli, jq

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG_FILE="${SCRIPT_DIR}/config.json"
OUTPUT_FILE=""
S3_UPLOAD=false

usage() {
    echo "Usage: $0 [--config <path>] [--output <path>] [--s3-upload]"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --config) CONFIG_FILE="$2"; shift 2 ;;
        --output) OUTPUT_FILE="$2"; shift 2 ;;
        --s3-upload) S3_UPLOAD=true; shift ;;
        *) usage ;;
    esac
done

# shellcheck disable=SC1090
source /dev/stdin <<<"$(cat <<RCFG
$(jq -r '
    "RPC_URL=\(.rpc_url)\nNETWORK=\(.network_passphrase)\nCONTRACT_ID=\(.contract_id)\nBACKUP_DIR=\(.backup_dir)\nBATCH_SIZE=\(.max_batch_size)\nS3_BUCKET=\(.s3_bucket)\nS3_REGION=\(.s3_region)\nS3_ACCESS_KEY=\(.s3_access_key)\nS3_SECRET_KEY=\(.s3_secret_key)"
' "$CONFIG_FILE")"
RCFG

BACKUP_DIR="${BACKUP_DIR:-./backups}"
mkdir -p "$BACKUP_DIR"

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
OUTPUT_FILE="${OUTPUT_FILE:-${BACKUP_DIR}/audit-ledger-backup-$(date -u +%Y%m%d_%H%M%S).json}"

echo "=== AuditLedger Backup ==="
echo "Contract:  $CONTRACT_ID"
echo "RPC:       $RPC_URL"
echo "Output:    $OUTPUT_FILE"
echo ""

# 1. Fetch total event count
echo "Fetching total_events..."
TOTAL_EVENTS=$(soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$(soroban config identity)" \
    --network-passphrase "$NETWORK" \
    --rpc-url "$RPC_URL" \
    -- \
    total_events \
    | jq '.[]' 2>/dev/null || echo 0)

echo "Found $TOTAL_EVENTS events."

# 2. Fetch all events in batches
EVENTS_ARRAY="[]"
OFFSET=0

while [ "$OFFSET" -lt "$TOTAL_EVENTS" ]; do
    BATCH_END=$((OFFSET + BATCH_SIZE))
    if [ "$BATCH_END" -gt "$TOTAL_EVENTS" ]; then
        BATCH_END="$TOTAL_EVENTS"
    fi
    echo -ne "Fetching events $OFFSET..$((BATCH_END - 1)) ...\r"

    for i in $(seq "$OFFSET" $((BATCH_END - 1))); do
        EVT=$(soroban contract invoke \
            --id "$CONTRACT_ID" \
            --source "$(soroban config identity)" \
            --network-passphrase "$NETWORK" \
            --rpc-url "$RPC_URL" \
            -- \
            get_event_by_order \
            --order "$i" 2>/dev/null || echo "null")

        if [ "$EVT" != "null" ]; then
            EVENTS_ARRAY=$(echo "$EVENTS_ARRAY" | jq --argjson evt "$EVT" '. += [$evt]')
        fi
    done

    OFFSET=$((OFFSET + BATCH_SIZE))
done

echo ""

# 3. Build the backup JSON
TIMESTAMP_CURRENT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
BLOCK_HEIGHT=$(soroban rpc get-health --rpc-url "$RPC_URL" 2>/dev/null | jq '.ledger.num' || echo "unknown")

BACKUP_JSON=$(jq -n \
    --arg ts "$TIMESTAMP_CURRENT" \
    --arg cid "$CONTRACT_ID" \
    --arg bh "$BLOCK_HEIGHT" \
    --argjson total "$TOTAL_EVENTS" \
    --argjson events "$EVENTS_ARRAY" \
    '{
        backup_timestamp: $ts,
        contract_id: $cid,
        block_height: $bh,
        total_events: $total,
        events: $events
    }')

echo "$BACKUP_JSON" > "$OUTPUT_FILE"
echo "Backup written to $OUTPUT_FILE"
echo "Events backed up: $(echo "$BACKUP_JSON" | jq '.events | length')"

# 4. Optional S3 upload
if [ "$S3_UPLOAD" = true ] && [ -n "$S3_BUCKET" ]; then
    echo "Uploading to s3://${S3_BUCKET}/ ..."
    if command -v aws &>/dev/null; then
        export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY"
        export AWS_SECRET_ACCESS_KEY="$S3_SECRET_KEY"
        export AWS_DEFAULT_REGION="$S3_REGION"
        aws s3 cp "$OUTPUT_FILE" "s3://${S3_BUCKET}/$(basename "$OUTPUT_FILE")"
        echo "Upload complete."
    else
        echo "WARNING: 'aws' CLI not found. Skipping S3 upload."
    fi
fi

echo ""
echo "Backup complete."
