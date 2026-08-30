#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Contract Event Semantic Versioning Manager
# Handles SemVer calculation, validation, conventional-commit inspection,
# release candidate increments, and multi-package synchronization.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CARGO_TOML="$SCRIPT_DIR/Cargo.toml"
JS_PKG="$SCRIPT_DIR/sdk/js/package.json"
PY_PKG="$SCRIPT_DIR/sdk/python/pyproject.toml"

usage() {
  cat << USAGE
Usage: $0 <command> [options]

Commands:
  parse <version>                   Parse and validate a semantic version string
  bump <major|minor|patch|rc> [ver] Calculate next version from given version or latest tag
  auto-bump [from-ref]              Determine next SemVer automatically from conventional commits
  validate <current> <next>         Verify SemVer upgrade compatibility (breaking change rules)
  sync <version>                    Update version in Cargo.toml, JS SDK, and Python SDK
  get-current                       Get current active version from repository tags/files

Options:
  --rc-num <n>                      Explicit RC number when bumping RC
  --allow-breaking                  Acknowledge breaking changes on major bump
USAGE
  exit 1
}

parse_semver() {
  local version="${1#v}"
  local regex="^([0-9]+)\.([0-9]+)\.([0-9]+)(-([0-9A-Za-z.-]+))?(\+([0-9A-Za-z.-]+))?$"
  
  if [[ ! "$version" =~ $regex ]]; then
    echo "ERROR: Invalid semantic version '$1'" >&2
    return 1
  fi

  local major="${BASH_REMATCH[1]}"
  local minor="${BASH_REMATCH[2]}"
  local patch="${BASH_REMATCH[3]}"
  local prerelease="${BASH_REMATCH[5]:-}"
  local build="${BASH_REMATCH[7]:-}"

  echo "MAJOR=$major"
  echo "MINOR=$minor"
  echo "PATCH=$patch"
  echo "PRERELEASE=$prerelease"
  echo "BUILD=$build"
}

get_current_version() {
  local tag
  tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
  if [ -n "$tag" ]; then
    echo "${tag#v}"
  elif [ -f "$CARGO_TOML" ]; then
    grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/'
  else
    echo "0.1.0"
  fi
}

bump_semver() {
  local bump_type="$1"
  local current="${2:-$(get_current_version)}"
  current="${current#v}"

  local major minor patch prerelease
  IFS='.' read -r major minor rest <<< "$current"
  if [[ "$rest" =~ ^([0-9]+)(-(.*))?$ ]]; then
    patch="${BASH_REMATCH[1]}"
    prerelease="${BASH_REMATCH[3]:-}"
  else
    patch="$rest"
    prerelease=""
  fi

  case "$bump_type" in
    major)
      echo "$((major + 1)).0.0"
      ;;
    minor)
      echo "$major.$((minor + 1)).0"
      ;;
    patch)
      echo "$major.$minor.$((patch + 1))"
      ;;
    rc)
      if [[ "$prerelease" =~ ^rc\.([0-9]+)$ ]]; then
        local rc_num="${BASH_REMATCH[1]}"
        echo "$major.$minor.$patch-rc.$((rc_num + 1))"
      else
        echo "$major.$((minor + 1)).0-rc.1"
      fi
      ;;
    *)
      echo "ERROR: Unknown bump type '$bump_type'" >&2
      exit 1
      ;;
  esac
}

auto_determine_bump() {
  local from_ref="${1:-}"
  if [ -z "$from_ref" ]; then
    from_ref=$(git describe --tags --abbrev=0 2>/dev/null || git rev-list --max-parents=0 HEAD)
  fi

  local commits
  commits=$(git log "$from_ref..HEAD" --oneline 2>/dev/null || true)
  if [ -z "$commits" ]; then
    echo "patch"
    return 0
  fi

  if echo "$commits" | grep -Eq "^[a-f0-9]+ [a-z]+(\([^\)]+\))?!:|BREAKING CHANGE:"; then
    echo "major"
  elif echo "$commits" | grep -Eq "^[a-f0-9]+ feat(\([^\)]+\))?:"; then
    echo "minor"
  else
    echo "patch"
  fi
}

validate_upgrade() {
  local curr="${1#v}"
  local next="${2#v}"

  local curr_major curr_minor curr_patch
  local next_major next_minor next_patch

  IFS='.' read -r curr_major curr_minor curr_patch <<< "${curr%%-*}"
  IFS='.' read -r next_major next_minor next_patch <<< "${next%%-*}"

  if [ "$next_major" -gt "$curr_major" ]; then
    echo "VALIDATION: Major version upgrade ($curr -> $next). Breaking changes permitted."
    return 0
  elif [ "$next_major" -lt "$curr_major" ]; then
    echo "ERROR: Target version $next is lower than current version $curr (downgrade)." >&2
    return 1
  fi

  if [ "$next_minor" -gt "$curr_minor" ]; then
    echo "VALIDATION: Minor version upgrade ($curr -> $next). Backward-compatible features."
    return 0
  elif [ "$next_minor" -lt "$curr_minor" ]; then
    echo "ERROR: Minor version decreased ($curr -> $next)." >&2
    return 1
  fi

  if [ "$next_patch" -ge "$curr_patch" ]; then
    echo "VALIDATION: Patch upgrade ($curr -> $next). Backward-compatible bug fixes."
    return 0
  else
    echo "ERROR: Patch version decreased ($curr -> $next)." >&2
    return 1
  fi
}

sync_version() {
  local new_ver="${1#v}"
  echo "Synchronizing version '$new_ver' across project manifests..."

  if [ -f "$CARGO_TOML" ]; then
    sed -i "s/^version = \".*\"/version = \"$new_ver\"/" "$CARGO_TOML"
    echo "  Updated $CARGO_TOML"
  fi

  if [ -f "$JS_PKG" ]; then
    sed -i "s/\"version\": \".*\"/\"version\": \"$new_ver\"/" "$JS_PKG"
    echo "  Updated $JS_PKG"
  fi

  if [ -f "$PY_PKG" ]; then
    sed -i "s/^version = \".*\"/version = \"$new_ver\"/" "$PY_PKG"
    echo "  Updated $PY_PKG"
  fi

  echo "Synchronization complete."
}

# Main Command Dispatcher
COMMAND="${1:-}"
shift || true

case "$COMMAND" in
  parse)
    [ -z "${1:-}" ] && usage
    parse_semver "$1"
    ;;
  bump)
    [ -z "${1:-}" ] && usage
    bump_semver "$1" "${2:-}"
    ;;
  auto-bump)
    auto_type=$(auto_determine_bump "${1:-}")
    current=$(get_current_version)
    bump_semver "$auto_type" "$current"
    ;;
  validate)
    [ $# -lt 2 ] && usage
    validate_upgrade "$1" "$2"
    ;;
  sync)
    [ -z "${1:-}" ] && usage
    sync_version "$1"
    ;;
  get-current)
    get_current_version
    ;;
  *)
    usage
    ;;
esac
