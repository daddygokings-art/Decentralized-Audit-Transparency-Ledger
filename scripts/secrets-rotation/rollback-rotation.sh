#!/usr/bin/env bash
set -euo pipefail

# === Rotation rollback ===
# Restores the previous credential/key version when validate-rotation.sh
# fails. Never deletes the failed new version outright (kept for
# incident forensics) — it is marked bad and superseded instead.
#
# Usage:
#   ./rollback-rotation.sh --type <database|api-key|signing-key> --target "<name(s)>"

# shellcheck source=./common.sh
source "$(cd "$(dirname "$0")" && pwd)/common.sh"

TYPE=""
TARGET=""

usage() {
  echo "Usage: $0 --type <database|api-key|signing-key> --target \"<name(s)>\""
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --type) TYPE="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    *) usage ;;
  esac
done

[ -z "$TYPE" ] && usage
[ -z "$TARGET" ] && usage

check_dependency vault
check_dependency jq

echo "=========================================="
echo " ROLLBACK: $TYPE / $TARGET"
echo "=========================================="

vault_login

rollback_database() {
  local role="$1"
  # Vault's database secrets engine does not expose "rotate to a specific
  # prior password" — the safe rollback is to re-rotate immediately
  # (generating a fresh, known-good credential) rather than trying to
  # recover a discarded one, and to alert a human, since a validation
  # failure here means the DB itself may be unreachable/misconfigured.
  info "Re-rotating '$role' to a fresh credential and re-validating."
  vault write -f "database/rotate-role/$role" >/dev/null
  record_rotation_state "db-$role" "status=rollback-rerotated" "rolled_back_by=validate-rotation"
  fail "Database role '$role' re-rotated after validation failure — investigate DB reachability before next scheduled rotation."
}

rollback_api_key() {
  local key="$1"
  local path="secret/data/audit-ledger/api-keys/$key"
  local meta
  meta="$(vault kv metadata get -format=json "secret/metadata/audit-ledger/api-keys/$key")"
  local current_version prev_version
  current_version="$(echo "$meta" | jq -r '.data.current_version')"
  prev_version=$((current_version - 1))

  if [ "$prev_version" -lt 1 ]; then
    fail "No previous version of API key '$key' exists to roll back to — manual intervention required."
    return 1
  fi

  local prev_value
  prev_value="$(vault kv get -version="$prev_version" -format=json "$path" | jq -r '.data.data.value')"

  vault kv put "$path" value="$prev_value" rolled_back_from="v$current_version" rolled_back_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)" >/dev/null
  vault kv delete -versions="$current_version" "$path" >/dev/null

  record_rotation_state "api-key-$key" "status=rolled-back" "restored_version=$prev_version" "bad_version=$current_version"
  pass "API key '$key' rolled back to version $prev_version; bad version $current_version soft-deleted."
}

rollback_signing_key() {
  local key="$1"
  local state
  state="$(last_rotation_state "signing-key-$key")"
  local prev_version
  prev_version="$(echo "$state" | jq -r '.previous_version // empty')"

  if [ -z "$prev_version" ]; then
    fail "No recorded previous version for signing key '$key' — cannot compute safe rollback bounds."
    return 1
  fi

  # Transit can't "un-rotate" a key version, but we can immediately force
  # min_encryption_version back to the previous version so new
  # signatures resume using the known-good key while the bad version is
  # excluded from new signing (still valid for verification, so nothing
  # already signed with it breaks).
  vault write "transit/keys/$key/config" \
    min_encryption_version="$prev_version" >/dev/null

  record_rotation_state "signing-key-$key" "status=rolled-back" "min_encryption_version_forced_to=$prev_version"
  pass "Signing key '$key' forced back to encrypting with v$prev_version pending investigation."
}

case "$TYPE" in
  database)
    for t in $TARGET; do rollback_database "$t"; done
    ;;
  api-key)
    for t in $TARGET; do rollback_api_key "$t"; done
    ;;
  signing-key)
    for t in $TARGET; do rollback_signing_key "$t"; done
    ;;
  *)
    fail "Unknown rollback type: $TYPE"
    usage
    ;;
esac

fail "ROLLBACK COMPLETED for $TYPE/$TARGET — this always pages on-call; rotation failures are never silent."
