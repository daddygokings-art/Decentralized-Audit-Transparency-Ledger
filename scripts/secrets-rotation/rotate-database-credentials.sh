#!/usr/bin/env bash
set -euo pipefail

# === Database credential rotation ===
# Rotates the Vault-managed static role backing the app's DB connection
# pool (dynamic per-connection creds are handled transparently by Vault's
# `database` secrets engine lease system and need no rotation script).
#
# Flow: rotate root/static role in Vault -> validate the new credential
# actually authenticates -> only then is it live (Vault's static role
# rotation is atomic from the DB's point of view: old password stops
# working the instant the new one is set, so validation happens against
# the *new* credential, and rollback means restoring the previous
# password Vault cached before rotating).
#
# Usage:
#   ./rotate-database-credentials.sh [--target <vault-static-role>] [--dry-run]

# shellcheck source=./common.sh
source "$(cd "$(dirname "$0")" && pwd)/common.sh"

TARGET="${ROTATION_TARGET:-audit-ledger-static}"
DRY_RUN=false

usage() {
  echo "Usage: $0 [--target <vault-static-role>] [--dry-run]"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    *) usage ;;
  esac
done

check_dependency vault
check_dependency jq

echo "=========================================="
echo " Database Credential Rotation: $TARGET"
echo "=========================================="

vault_login

info "Reading current static-role credential (pre-rotation snapshot)."
PREV_CREDS="$(vault read -format=json "database/static-creds/$TARGET")"
PREV_PASSWORD="$(echo "$PREV_CREDS" | jq -r '.data.password')"

if [ "$DRY_RUN" = true ]; then
  skip "Dry-run: would rotate database/rotate-role/$TARGET now."
  exit 0
fi

info "Triggering rotation."
vault write -f "database/rotate-role/$TARGET" >/dev/null

info "Reading new credential."
NEW_CREDS="$(vault read -format=json "database/static-creds/$TARGET")"
NEW_PASSWORD="$(echo "$NEW_CREDS" | jq -r '.data.password')"

if [ "$NEW_PASSWORD" = "$PREV_PASSWORD" ]; then
  fail "Rotation did not change the credential — aborting, not recording new state."
  exit 1
fi

record_rotation_state "db-$TARGET" \
  "status=rotated" \
  "previous_password_ref=vault-lease-history" \
  "rotated_by=cronjob"

pass "Rotated $TARGET. Handing off to validate-rotation.sh."

"$SCRIPT_DIR/validate-rotation.sh" --type database --target "$TARGET"
