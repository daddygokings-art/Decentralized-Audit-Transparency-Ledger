#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Contract Event Release Notes Generator
# Generates comprehensive release documentation with categorized commits,
# asset digests, schema compatibility information, and rollback instructions.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
OUTPUT_FILE="${OUTPUT_FILE:-$SCRIPT_DIR/RELEASE_NOTES.md}"
RELEASE_TAG="${1:-}"
PREV_TAG="${2:-}"

if [ -z "$RELEASE_TAG" ]; then
  RELEASE_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.1.0")
fi

if [ -z "$PREV_TAG" ]; then
  PREV_TAG=$(git describe --tags --abbrev=0 "${RELEASE_TAG}^" 2>/dev/null || git rev-list --max-parents=0 HEAD)
fi

RELEASE_DATE=$(date -u +"%Y-%m-%d")
IS_PRERELEASE=false
if [[ "$RELEASE_TAG" == *"-"* ]]; then
  IS_PRERELEASE=true
fi

echo "Generating release notes for $RELEASE_TAG (from $PREV_TAG)..."

mkdir -p "$(dirname "$OUTPUT_FILE")"

cat << HEADER > "$OUTPUT_FILE"
# Release Notes — $RELEASE_TAG

**Release Date:** $RELEASE_DATE  
**Status:** $( [ "$IS_PRERELEASE" = true ] && echo "🟡 **Release Candidate (Pre-release)**" || echo "🟢 **Production Release (Stable)**" )  
**Previous Baseline:** \`$PREV_TAG\`  
**Commit Range:** [\`$PREV_TAG...$RELEASE_TAG\`](https://github.com/ComputerOracle/Decentralized-Audit-Transparency-Ledger/compare/$PREV_TAG...$RELEASE_TAG)

---

## 📋 Overview & Highlights

HEADER

if [ "$IS_PRERELEASE" = true ]; then
  cat << RC_NOTICE >> "$OUTPUT_FILE"
> [!IMPORTANT]
> **Release Candidate Notice**: This is a pre-release candidate intended for staging validation,
> integration testing, and formal verification on testnet. Do not deploy to production until final sign-off.

RC_NOTICE
fi

# Categorized Commit Groups
generate_changelog_section() {
  local title="$1"
  local pattern="$2"
  local commits
  commits=$(git log "$PREV_TAG..$RELEASE_TAG" --grep="$pattern" -E --oneline --format="- %s ([%h](https://github.com/ComputerOracle/Decentralized-Audit-Transparency-Ledger/commit/%H)) by @%an" 2>/dev/null || true)
  
  if [ -n "$commits" ]; then
    echo "### $title" >> "$OUTPUT_FILE"
    echo "$commits" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
  fi
}

echo "## 🚀 Changelog" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

generate_changelog_section "💥 Breaking Changes" "(^[a-f0-9]+ [a-z]+(\([^\)]+\))?!:|BREAKING CHANGE:)"
generate_changelog_section "✨ New Features & Capabilities" "^[a-f0-9]+ feat(\([^\)]+\))?:"
generate_changelog_section "🐛 Bug Fixes & Security Hardening" "^[a-f0-9]+ (fix|security)(\([^\)]+\))?:"
generate_changelog_section "⚡ Performance Improvements" "^[a-f0-9]+ perf(\([^\)]+\))?:"
generate_changelog_section "🛠️ Infrastructure, DevOps & CI/CD" "^[a-f0-9]+ (ci|chore|build|devops)(\([^\)]+\))?:"
generate_changelog_section "📚 Documentation" "^[a-f0-9]+ docs(\([^\)]+\))?:"

cat << ASSETS >> "$OUTPUT_FILE"
## 📦 Release Artifacts & Verification

| Artifact Name | Type | SHA-256 Digest | Status |
|---|---|---|---|
| \`audit_ledger.wasm\` | Optimized WASM Bytecode | \`$( [ -f "$SCRIPT_DIR/target/wasm32v1-none/release/audit_ledger.wasm" ] && sha256sum "$SCRIPT_DIR/target/wasm32v1-none/release/audit_ledger.wasm" | awk '{print $1}' || echo "Calculated at build" )\` | Built |
| \`audit-ledger-sbom.json\` | CycloneDX 1.5 SBOM | \`$( [ -f "$SCRIPT_DIR/sbom/audit-ledger-sbom.json" ] && sha256sum "$SCRIPT_DIR/sbom/audit-ledger-sbom.json" | awk '{print $1}' || echo "Generated on CI" )\` | Generated |
| \`event-schema.json\` | Soroban Event Schema | \`$( [ -f "$SCRIPT_DIR/docs/event-schema.json" ] && sha256sum "$SCRIPT_DIR/docs/event-schema.json" | awk '{print $1}' || echo "Anchored" )\` | Validated |

## 🔄 Rollback & Recovery Instructions

If this release exhibits critical regression in production, execute the automated rollback playbook:

\`\`\`bash
# 1. Rollback on-chain release pointer to previous stable version
./scripts/release/rollback-release.sh --from "$RELEASE_TAG" --target "$PREV_TAG" --reason "Critical incident"

# 2. Trigger automated GitHub release deprecation workflow
gh workflow run release-rollback.yml -f from_version="$RELEASE_TAG" -f target_version="$PREV_TAG"
\`\`\`

---

## 👥 Contributors in this Release

$(git log "$PREV_TAG..$RELEASE_TAG" --format="- @%an" | sort -u || echo "- Core Engineering Team")

ASSETS

echo "Release notes written to $OUTPUT_FILE"
