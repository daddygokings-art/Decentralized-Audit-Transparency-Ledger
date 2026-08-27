#!/usr/bin/env bash
set -euo pipefail

# === Post-rotation validation ===
# Confirms newly rotated material actually works before the rotation is
# considered complete. Called automatically by each rotate-*.sh, and safe
# to run standalone against the last recorded rotation for a target.
# On failure, invokes rollback-rotation.sh and exits non-zero so the
# owning CronJob is marked Failed and pages on-call.
#
# Usage:
#   ./validate-rotation.sh --type <database|api-key|signing-key> --target "<name(s)>"

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
echo " Rotation Validation: $TYPE / $TARGET"
echo "=========================================="

vault_login

validation_failed=false

validate_database() {
  local role="$1"
  check_dependency psql
  local creds host db
  creds="$(vault read -format=json "database/static-creds/$role")"
  host="${DB_VALIDATION_HOST:?DB_VALIDATION_HOST required for database validation}"
  db="${DB_VALIDATION_NAME:?DB_VALIDATION_NAME required for database validation}"
  local user pass
  user="$(echo "$creds" | jq -r '.data.username')"
  pass="$(echo "$creds" | jq -r '.data.password')"

  if PGPASSWORD="$pass" psql -h "$host" -U "$user" -d "$db" -c "SELECT 1;" >/dev/null 2>&1; then
    pass "Database credential for '$role' authenticates successfully."
  else
    fail "New database credential for '$role' FAILED to authenticate."
    validation_failed=true
  fi
}

validate_api_key() {
  local key="$1"
  local secret
  secret="$(vault kv get -format=json "secret/data/audit-ledger/api-keys/$key" | jq -r '.data.data.value')"
  if [ -n "$secret" ] && [ "$secret" != "null" ] && [ "${#secret}" -ge 32 ]; then
    pass "API key '$key' present in Vault and well-formed (${#secret} chars)."
  else
    fail "API key '$key' missing or malformed after rotation."
    validation_failed=true
  fi
}

validate_signing_key() {
  local key="$1"
  local sample
  sample="rotation-validation-$(date +%s)"
  local sig
  sig="$(vault write -field=signature "transit/sign/$key" input="$(echo -n "$sample" | base64)")"
  local verify_ok
  verify_ok="$(vault write -field=valid "transit/verify/$key" input="$(echo -n "$sample" | base64)" signature="$sig")"
  if [ "$verify_ok" = "true" ]; then
    pass "Signing key '$key' sign/verify round-trip succeeded."
  else
    fail "Signing key '$key' sign/verify round-trip FAILED."
    validation_failed=true
  fi
}

case "$TYPE" in
  database)
    for t in $TARGET; do validate_database "$t"; done
    ;;
  api-key)
    for t in $TARGET; do validate_api_key "$t"; done
    ;;
  signing-key)
    for t in $TARGET; do validate_signing_key "$t"; done
    ;;
  *)
    fail "Unknown validation type: $TYPE"
    usage
    ;;
esac

if [ "$validation_failed" = true ]; then
  fail "Validation failed for $TYPE/$TARGET — invoking rollback."
  "$SCRIPT_DIR/rollback-rotation.sh" --type "$TYPE" --target "$TARGET"
  exit 1
fi

pass "All validation checks passed for $TYPE/$TARGET."
