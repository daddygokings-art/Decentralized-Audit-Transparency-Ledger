#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
status=0

while IFS= read -r lockfile; do
  package_dir=$(dirname "$lockfile")
  echo "Verifying $lockfile"
  if ! (cd "$repo_root/$package_dir" && npm ci --ignore-scripts --audit=false --fund=false); then
    echo "Dependency verification failed for $lockfile" >&2
    status=1
  fi
done < <(find "$repo_root" -name package-lock.json -not -path '*/node_modules/*' | sed "s#^$repo_root/##" | sort)

if [[ -n "${NPM_PRIVATE_REGISTRY_URL:-}" ]]; then
  case "$NPM_PRIVATE_REGISTRY_URL" in
    https://*) ;;
    *) echo "NPM_PRIVATE_REGISTRY_URL must use HTTPS" >&2; status=1 ;;
  esac
fi

if grep -RInE "git\\+|github:[^/]+/|http://|https?://[^[:space:]\"']+\\.tgz" \
  --exclude-dir=node_modules --include='package.json' "$repo_root"; then
  echo "Unverified git, HTTP, or direct tarball dependency in package manifest" >&2
  status=1
fi

exit "$status"