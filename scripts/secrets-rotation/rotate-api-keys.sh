#!/usr/bin/env bash
set -euo pipefail

# === API key rotation ===
# Rotates externally-issued API keys (notifier webhook tokens, SDK
# publish tokens, third-party integration keys) stored in Vault's KV v2
# engine. KV v2 keeps prior versions, so rollback is "read version N-1
# and re-issue it" rather than needing a separate backup mechanism.
#
# No-ops unless ROTATION_INTERVAL_DAYS have elapsed since the last
# recorded rotation for a given key, so the CronJob can run monthly while
# the actual rotation cadence per key stays configurable (default 90d).
#
# Usage:
#   ./rotate-api-keys.sh [--keys <space-separated KV names>] [--interval-days N] [--dry-run]

# shellcheck source=./common.sh
source "$(cd "$(dirname "$0")" && pwd)/common.sh"

KV_MOUNT="secret/data/audit-ledger/api-keys"
KEYS="${API_KEYS:-notifier-webhook sdk-publish integration-default}"
INTERVAL_DAYS="${ROTATION_INTERVAL_DAYS:-90}"
DRY_RUN=false

usage() {
  echo "Usage: $0 [--keys <names>] [--interval-days N] [--dry-run]"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keys) KEYS="$2"; shift 2 ;;
    --interval-days) INTERVAL_DAYS="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    *) usage ;;
  esac
done

check_dependency vault
check_dependency jq
check_dependency openssl

echo "=========================================="
echo " API Key Rotation (interval: ${INTERVAL_DAYS}d)"
echo "=========================================="

vault_login

results_rotated=0
results_skipped=0

for key in $KEYS; do
  path="$KV_MOUNT/$key"
  state="$(last_rotation_state "api-key-$key")"
  last_ts="$(echo "$state" | jq -r '.timestamp // empty')"
  elapsed="$(days_since "$last_ts")"

  if [ "$elapsed" -lt "$INTERVAL_DAYS" ]; then
    skip "$key last rotated ${elapsed}d ago (< ${INTERVAL_DAYS}d) — no-op."
    results_skipped=$((results_skipped + 1))
    continue
  fi

  if [ "$DRY_RUN" = true ]; then
    skip "Dry-run: would rotate $key (${elapsed}d since last rotation)."
    continue
  fi

  NEW_KEY="$(openssl rand -hex 32)"
  PREV_VERSION="$(vault kv get -format=json "$path" 2>/dev/null | jq -r '.data.metadata.version // 0')"

  vault kv put "$path" value="$NEW_KEY" rotated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)" >/dev/null

  record_rotation_state "api-key-$key" \
    "status=rotated" \
    "previous_version=$PREV_VERSION" \
    "rotated_by=cronjob"

  pass "Rotated API key '$key' (previous KV version: $PREV_VERSION)."
  results_rotated=$((results_rotated + 1))
done

info "Rotated: $results_rotated, Skipped (within interval): $results_skipped"

if [ "$results_rotated" -gt 0 ] && [ "$DRY_RUN" = false ]; then
  "$SCRIPT_DIR/validate-rotation.sh" --type api-key --target "$KEYS"
fi
