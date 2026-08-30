#!/usr/bin/env bash
# === Shared helpers for secrets-rotation scripts ===
# Sourced by rotate-*.sh, validate-rotation.sh, rollback-rotation.sh.
# Not meant to be executed directly.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SCRIPT_DIR
LOG_FILE="${LOG_FILE:-rotation-$(date +%Y%m%d-%H%M%S).log}"
STATE_DIR="${STATE_DIR:-/var/lib/audit-ledger/rotation-state}"

log()  { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG_FILE"; }
info() { log "INFO: $*"; }
pass() { log "PASS: $*"; }
fail() { log "FAIL: $*"; }
skip() { log "SKIP: $*"; }

check_dependency() {
  if ! command -v "$1" &>/dev/null; then
    fail "required dependency '$1' not found."
    exit 1
  fi
}

# vault_login authenticates via Kubernetes auth using the pod's projected
# ServiceAccount token. Falls back to VAULT_TOKEN (already set) for local
# dry-runs against a dev Vault instance.
vault_login() {
  if [ -n "${VAULT_TOKEN:-}" ]; then
    info "Using pre-set VAULT_TOKEN (local/dry-run mode)."
    return 0
  fi
  check_dependency vault
  local sa_token="/var/run/secrets/kubernetes.io/serviceaccount/token"
  if [ ! -f "$sa_token" ]; then
    fail "no VAULT_TOKEN and no Kubernetes ServiceAccount token found at $sa_token"
    exit 1
  fi
  local login_out
  login_out="$(vault write -format=json auth/kubernetes/login \
    role="${VAULT_K8S_ROLE:?VAULT_K8S_ROLE is required}" \
    jwt="$(cat "$sa_token")")"
  export VAULT_TOKEN
  VAULT_TOKEN="$(echo "$login_out" | jq -r '.auth.client_token')"
  if [ -z "$VAULT_TOKEN" ] || [ "$VAULT_TOKEN" = "null" ]; then
    fail "Vault Kubernetes auth login failed."
    exit 1
  fi
  pass "Authenticated to Vault as role '$VAULT_K8S_ROLE'."
}

# record_rotation_state <target> <k=v> [k=v...] appends a JSON line to the
# per-target state file so validate-rotation.sh / rollback-rotation.sh can
# find the previous and current material without re-querying Vault history
# under time pressure during an incident.
record_rotation_state() {
  local target="$1"; shift
  mkdir -p "$STATE_DIR"
  local ts
  ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local fields="\"timestamp\":\"$ts\""
  for kv in "$@"; do
    local k="${kv%%=*}"
    local v="${kv#*=}"
    fields="$fields,\"$k\":\"$v\""
  done
  echo "{$fields}" >>"$STATE_DIR/${target}.jsonl"
  write_rotation_metric "$target"
}

# write_rotation_metric emits a node_exporter textfile-collector metric so
# infra/k8s/monitoring/secrets-rotation-alerts.yaml can alert on rotation
# staleness without querying Vault directly. TEXTFILE_COLLECTOR_DIR
# defaults to node_exporter's standard scrape path; set it to /dev/null-safe
# no-op location in local/dry-run use.
write_rotation_metric() {
  local target="$1"
  local dir="${TEXTFILE_COLLECTOR_DIR:-/var/lib/node_exporter/textfile_collector}"
  mkdir -p "$dir" 2>/dev/null || return 0
  local tmp="$dir/audit_ledger_rotation.prom.$$"
  {
    echo "# HELP audit_ledger_last_rotation_timestamp_seconds Unix timestamp of the last recorded rotation per target."
    echo "# TYPE audit_ledger_last_rotation_timestamp_seconds gauge"
    while IFS= read -r line; do
      local t
      t="${line%%.jsonl}"
      t="$(basename "$t")"
      local ts
      ts="$(tail -n 1 "$line" 2>/dev/null | jq -r '.timestamp // empty')"
      [ -z "$ts" ] && continue
      local epoch
      epoch="$(date -u -d "$ts" +%s 2>/dev/null || echo 0)"
      echo "audit_ledger_last_rotation_timestamp_seconds{target=\"$t\"} $epoch"
    done < <(find "$STATE_DIR" -maxdepth 1 -name '*.jsonl' 2>/dev/null)
  } >"$tmp"
  mv -f "$tmp" "$dir/audit_ledger_rotation.prom"
}

last_rotation_state() {
  local target="$1"
  local f="$STATE_DIR/${target}.jsonl"
  [ -f "$f" ] && tail -n 1 "$f" || echo "{}"
}

# days_since_epoch_field extracts an ISO8601 "timestamp" field from a JSON
# line and returns whole days elapsed since then.
days_since() {
  local iso="$1"
  [ -z "$iso" ] && { echo 999999; return; }
  local then_epoch now_epoch
  then_epoch="$(date -u -d "$iso" +%s 2>/dev/null || echo 0)"
  now_epoch="$(date -u +%s)"
  echo $(( (now_epoch - then_epoch) / 86400 ))
}
