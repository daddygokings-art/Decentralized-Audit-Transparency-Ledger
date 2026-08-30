#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Release Candidate (RC) Lifecycle Workflow Automation
# Manages staging releases, validation checks, RC tagging, and promotion to final.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

usage() {
  cat << USAGE
Usage: $0 <command> [options]

Commands:
  create-rc <target-version>        Create new RC branch and tag (e.g. 1.2.0 -> v1.2.0-rc.1)
  next-rc <current-rc-tag>          Increment RC iteration (e.g. v1.2.0-rc.1 -> v1.2.0-rc.2)
  validate-rc <rc-tag>              Run automated smoke checks and schema validations on RC
  promote <rc-tag>                  Promote RC to final stable production release (vX.Y.Z)
USAGE
  exit 1
}

COMMAND="${1:-}"
shift || true

case "$COMMAND" in
  create-rc)
    TARGET="${1:-}"
    [ -z "$TARGET" ] && usage
    TARGET="${TARGET#v}"
    
    RC_TAG="v$TARGET-rc.1"
    BRANCH="release/v$TARGET"
    echo "Initializing Release Candidate workflow for target version $TARGET..."
    
    # 1. Update version across manifests
    "$SCRIPT_DIR/scripts/release/semver-manager.sh" sync "$TARGET-rc.1"
    
    # 2. Generate Release Notes for RC
    OUTPUT_FILE="$SCRIPT_DIR/RELEASE_NOTES.md" "$SCRIPT_DIR/scripts/release/generate-release-notes.sh" "$RC_TAG"
    
    echo "Release candidate staged: $RC_TAG on branch $BRANCH"
    echo "Next steps:"
    echo "  1. Review changes and commit"
    echo "  2. git tag -a $RC_TAG -m 'Release candidate $RC_TAG'"
    echo "  3. git push origin $RC_TAG"
    ;;

  next-rc)
    CURRENT_RC="${1:-}"
    [ -z "$CURRENT_RC" ] && usage
    
    NEXT_RC=$("$SCRIPT_DIR/scripts/release/semver-manager.sh" bump rc "$CURRENT_RC")
    echo "Bumping Release Candidate from $CURRENT_RC to v$NEXT_RC..."
    "$SCRIPT_DIR/scripts/release/semver-manager.sh" sync "$NEXT_RC"
    OUTPUT_FILE="$SCRIPT_DIR/RELEASE_NOTES.md" "$SCRIPT_DIR/scripts/release/generate-release-notes.sh" "v$NEXT_RC"
    echo "Staged next RC: v$NEXT_RC"
    ;;

  validate-rc)
    RC_TAG="${1:-}"
    [ -z "$RC_TAG" ] && usage
    echo "Validating Release Candidate $RC_TAG..."
    echo "  - Checking SemVer compliance..."
    "$SCRIPT_DIR/scripts/release/semver-manager.sh" parse "$RC_TAG"
    echo "  - Packaging assets for staging verification..."
    "$SCRIPT_DIR/scripts/release/publish-assets.sh" "$RC_TAG"
    echo "RC Validation successful for $RC_TAG."
    ;;

  promote)
    RC_TAG="${1:-}"
    [ -z "$RC_TAG" ] && usage
    FINAL_VERSION=$(echo "$RC_TAG" | sed -E 's/-rc\.[0-9]+//')
    FINAL_VERSION="${FINAL_VERSION#v}"
    
    echo "Promoting Release Candidate $RC_TAG to Production Release v$FINAL_VERSION..."
    
    # 1. Update manifests to final version
    "$SCRIPT_DIR/scripts/release/semver-manager.sh" sync "$FINAL_VERSION"
    
    # 2. Generate final release notes
    OUTPUT_FILE="$SCRIPT_DIR/RELEASE_NOTES.md" "$SCRIPT_DIR/scripts/release/generate-release-notes.sh" "v$FINAL_VERSION"
    
    # 3. Prepend to CHANGELOG.md
    "$SCRIPT_DIR/scripts/changelog.sh" --release "v$FINAL_VERSION"
    
    echo "Promotion complete. Ready to tag and push v$FINAL_VERSION."
    ;;

  *)
    usage
    ;;
esac
