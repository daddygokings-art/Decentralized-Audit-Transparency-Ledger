#!/usr/bin/env bash
set -euo pipefail

# === Continuous compliance evidence collection ===
# Walks docs/compliance/control-matrix.yaml and pulls current evidence
# for every control marked `automated: true`, writing one timestamped
# JSON bundle per control under evidence/<control-id>/<date>.json.
# Manual controls are listed but not collected — they point at the
# human-maintained doc that IS their evidence (see the doc itself).
#
# Intended to run on a schedule (see
# .github/workflows/compliance-evidence.yml) so auditor access
# (docs/compliance/auditor-access.md) always has a current evidence
# trail rather than a point-in-time snapshot gathered right before an
# audit.
#
# Usage:
#   ./collect-evidence.sh [--out-dir <dir>]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MATRIX="$SCRIPT_DIR/../../docs/compliance/control-matrix.yaml"
OUT_DIR="${1:-$SCRIPT_DIR/../../evidence}"
[ "${1:-}" = "--out-dir" ] && OUT_DIR="$2"

DATE="$(date -u +%Y-%m-%d)"

command -v jq >/dev/null || { echo "ERROR: jq required" >&2; exit 1; }
command -v yq >/dev/null || { echo "ERROR: yq required (mikefarah/yq)" >&2; exit 1; }

echo "=========================================="
echo " Compliance Evidence Collection — $DATE"
echo "=========================================="

CONTROLS_JSON="$(yq -o=json '.controls' "$MATRIX")"

echo "$CONTROLS_JSON" | jq -c '.[]' | while read -r control; do
  id="$(echo "$control" | jq -r '.id')"
  automated="$(echo "$control" | jq -r '.automated')"

  if [ "$automated" != "true" ]; then
    echo "SKIP: $id is manual — evidence is its referenced doc, not collected here."
    continue
  fi

  ctrl_dir="$OUT_DIR/$id"
  mkdir -p "$ctrl_dir"
  out_file="$ctrl_dir/$DATE.json"

  case "$id" in
    CTRL-SEC-01) # secrets rotation
      STATE_DIR="${STATE_DIR:-/var/lib/audit-ledger/rotation-state}"
      states="{}"
      if [ -d "$STATE_DIR" ]; then
        states="$(for f in "$STATE_DIR"/*.jsonl; do
          [ -f "$f" ] || continue
          target="$(basename "$f" .jsonl)"
          last="$(tail -n1 "$f")"
          echo "{\"$target\": $last}"
        done | jq -s 'add // {}')"
      fi
      echo "$control" | jq --argjson evidence "$states" '. + {collected_at: (now | todate), evidence_payload: {rotation_state: $evidence}}' >"$out_file"
      ;;
    CTRL-SEC-02) # cert-manager status
      certs="[]"
      if command -v kubectl >/dev/null; then
        certs="$(kubectl get certificates -A -o json 2>/dev/null | jq '[.items[] | {name: .metadata.name, namespace: .metadata.namespace, ready: (.status.conditions[]? | select(.type=="Ready") | .status), notAfter: .status.notAfter}]' || echo '[]')"
      fi
      echo "$control" | jq --argjson evidence "$certs" '. + {collected_at: (now | todate), evidence_payload: {certificates: $evidence}}' >"$out_file"
      ;;
    CTRL-SEC-03) # vulnerability management
      metrics="{}"
      [ -f "$SCRIPT_DIR/../../vuln-metrics.json" ] && metrics="$(cat "$SCRIPT_DIR/../../vuln-metrics.json")"
      exceptions="$(yq -o=json '.exceptions // []' "$SCRIPT_DIR/../vulnerability-management/exceptions.yaml")"
      echo "$control" | jq --argjson m "$metrics" --argjson e "$exceptions" '. + {collected_at: (now | todate), evidence_payload: {metrics: $m, active_exceptions: $e}}' >"$out_file"
      ;;
    CTRL-SEC-04) # dependency review — evidence is CI run history, link only
      echo "$control" | jq '. + {collected_at: (now | todate), evidence_payload: {note: "See GitHub Actions run history for dependency-review.yml; not pulled locally to avoid embedding a GitHub token in evidence bundles."}}' >"$out_file"
      ;;
    CTRL-AUD-01) # on-chain audit trail — evidence is a pointer to the verification tool, not a full ledger dump
      echo "$control" | jq '. + {collected_at: (now | todate), evidence_payload: {note: "Run scripts/deploy-verify.sh against the production contract ID for a fresh integrity check; full event log is public on-chain."}}' >"$out_file"
      ;;
    *)
      echo "$control" | jq '. + {collected_at: (now | todate), evidence_payload: {note: "No collector implemented for this control id yet."}}' >"$out_file"
      ;;
  esac

  echo "COLLECTED: $id -> $out_file"
done

echo "------------------------------------------"
echo "Evidence written under $OUT_DIR/"
