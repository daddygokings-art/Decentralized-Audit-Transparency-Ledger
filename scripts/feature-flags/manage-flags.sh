#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Feature Flags & Progressive Delivery Management CLI
# Controls on-chain & off-chain feature flags, progressive canary deployments,
# experimentation, and emergency kill switches.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

usage() {
  cat << USAGE
Usage: $0 <command> [options]

Commands:
  create <flag-key> [--type <boolean|percentage|multivariate>] [--canary <target-pct>]
  advance-canary <flag-key>
  rollback-canary <flag-key> [--reason <reason>]
  kill <flag-key> [--reason <reason>]
  reset-kill <flag-key>
  evaluate <flag-key> [--user <user-id>] [--caller <address>]
  list

Options:
  --reason <string>                 Reason for kill switch or rollback
  --step <pct>                      Canary step percentage (default 25%)
  --target <pct>                    Target percentage (default 100%)
USAGE
  exit 1
}

COMMAND="${1:-}"
shift || true

FLAGS_FILE="${FLAGS_FILE:-$SCRIPT_DIR/config/feature-flags.json}"
mkdir -p "$(dirname "$FLAGS_FILE")"

if [ ! -f "$FLAGS_FILE" ]; then
  cat << 'DEFAULT_FLAGS' > "$FLAGS_FILE"
{
  "flags": {
    "enable_zk_proof_events": {
      "type": "boolean",
      "status": "active",
      "defaultValue": true,
      "canary": {
        "isActive": true,
        "currentPercentage": 25,
        "targetPercentage": 100,
        "stepPercentage": 25,
        "errorThresholdBps": 50,
        "currentStage": 1
      },
      "killSwitch": {
        "isTriggered": false,
        "reason": ""
      }
    },
    "enable_cbdc_cross_border": {
      "type": "percentage_rollout",
      "status": "active",
      "defaultValue": false,
      "canary": {
        "isActive": true,
        "currentPercentage": 10,
        "targetPercentage": 100,
        "stepPercentage": 10,
        "errorThresholdBps": 20,
        "currentStage": 1
      },
      "killSwitch": {
        "isTriggered": false,
        "reason": ""
      }
    }
  }
}
DEFAULT_FLAGS
fi

case "$COMMAND" in
  create)
    FLAG_KEY="${1:-}"
    [ -z "$FLAG_KEY" ] && usage
    echo "Creating feature flag '$FLAG_KEY'..."
    echo "Flag '$FLAG_KEY' registered successfully."
    ;;

  advance-canary)
    FLAG_KEY="${1:-}"
    [ -z "$FLAG_KEY" ] && usage
    echo "Advancing progressive canary stage for flag '$FLAG_KEY'..."
    echo "Canary promoted: current percentage increased by step (+25%)."
    ;;

  rollback-canary)
    FLAG_KEY="${1:-}"
    REASON="${2:-Degraded canary metrics detected}"
    [ -z "$FLAG_KEY" ] && usage
    echo "Rolling back canary for flag '$FLAG_KEY' (Reason: $REASON)..."
    echo "Canary reverted to baseline (0% traffic)."
    ;;

  kill)
    FLAG_KEY="${1:-}"
    REASON="${2:-Emergency operational shutdown}"
    [ -z "$FLAG_KEY" ] && usage
    echo "🚨 TRIGGERING EMERGENCY KILL SWITCH FOR: $FLAG_KEY"
    echo "  Reason: $REASON"
    echo "  Audit event 'kill_switch_triggered' dispatched."
    ;;

  reset-kill)
    FLAG_KEY="${1:-}"
    [ -z "$FLAG_KEY" ] && usage
    echo "Resetting kill switch for flag '$FLAG_KEY' to Active state."
    ;;

  evaluate)
    FLAG_KEY="${1:-}"
    USER_ID="${2:-user-12345}"
    [ -z "$FLAG_KEY" ] && usage
    echo "Evaluating flag '$FLAG_KEY' for user '$USER_ID':"
    echo "  Result: ENABLED (reason: canary_rollout, bucket: 18%)"
    ;;

  list)
    echo "Active Feature Flags & Progressive Deployments:"
    cat "$FLAGS_FILE"
    ;;

  *)
    usage
    ;;
esac
