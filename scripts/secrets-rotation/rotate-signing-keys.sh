#!/usr/bin/env bash
set -euo pipefail

# === Signing key rotation ===
# Rotates Vault `transit` engine keys used to sign audit-ledger events and
# API JWTs. Transit versions keys instead of overwriting them: after
# rotation, new signatures use the latest version while verification
# continues to accept the last MIN_DECRYPTION_VERSION_GRACE_ROTATIONS
# versions, so tokens/signatures issued just before rotation don't break.
#
# Usage:
#   ./rotate-signing-keys.sh [--keys "<space-separated transit key names>"] [--dry-run]

# shellcheck source=./common.sh
source "$(cd "$(dirname "$0")" && pwd)/common.sh"

KEYS="${SIGNING_KEYS:-audit-ledger-event-signing audit-ledger-jwt-signing}"
GRACE_ROTATIONS="${MIN_DECRYPTION_VERSION_GRACE_ROTATIONS:-3}"
DRY_RUN=false

usage() {
  echo "Usage: $0 [--keys \"<names>\"] [--dry-run]"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keys) KEYS="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    *) usage ;;
  esac
done

check_dependency vault
check_dependency jq

echo "=========================================="
echo " Signing Key Rotation (grace: keep last $GRACE_ROTATIONS versions live)"
echo "=========================================="

vault_login

for key in $KEYS; do
  before="$(vault read -format=json "transit/keys/$key")"
  prev_version="$(echo "$before" | jq -r '.data.latest_version')"

  if [ "$DRY_RUN" = true ]; then
    skip "Dry-run: would rotate transit key '$key' (currently v$prev_version)."
    continue
  fi

  vault write -f "transit/keys/$key/rotate" >/dev/null

  after="$(vault read -format=json "transit/keys/$key")"
  new_version="$(echo "$after" | jq -r '.data.latest_version')"

  if [ "$new_version" -le "$prev_version" ]; then
    fail "Rotation of '$key' did not advance the key version — aborting."
    exit 1
  fi

  min_decrypt=$(( new_version - GRACE_ROTATIONS ))
  [ "$min_decrypt" -lt 1 ] && min_decrypt=1

  vault write "transit/keys/$key/config" \
    min_decryption_version="$min_decrypt" \
    min_encryption_version="$new_version" >/dev/null

  record_rotation_state "signing-key-$key" \
    "status=rotated" \
    "previous_version=$prev_version" \
    "new_version=$new_version" \
    "min_decryption_version=$min_decrypt" \
    "rotated_by=cronjob"

  pass "Rotated '$key': v$prev_version -> v$new_version (verification accepts >= v$min_decrypt)."
done

"$SCRIPT_DIR/validate-rotation.sh" --type signing-key --target "$KEYS"
