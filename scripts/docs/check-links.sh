#!/usr/bin/env bash
set -euo pipefail

echo "================ RUNNING DOCS LINK VALIDATOR ================"

BROKEN_LINKS=0

# Check local relative markdown links
while IFS= read -r file; do
  dir=$(dirname "$file")
  # Extract markdown links [text](path)
  links=$(grep -oE '\[[^]]+\]\([^)]+\)' "$file" | grep -oE '\([^)]+\)' | tr -d '()' || true)
  
  for link in $links; do
    # Ignore web URLs, anchors, mailto
    if [[ "$link" =~ ^https?:// ]] || [[ "$link" =~ ^mailto: ]] || [[ "$link" =~ ^# ]] || [[ "$link" =~ ^conversation:// ]]; then
      continue
    fi
    
    # Strip anchor fragment
    target_path="${link%%#*}"
    if [[ -z "$target_path" ]]; then
      continue
    fi
    
    # Resolve relative path
    resolved_path="$dir/$target_path"
    if [[ ! -e "$resolved_path" ]] && [[ ! -e "$target_path" ]]; then
      echo "❌ Broken link in $file: '$link' (Target: '$resolved_path' does not exist)"
      BROKEN_LINKS=$((BROKEN_LINKS + 1))
    fi
  done
done < <(find docs -name "*.md" -type f)

if [[ $BROKEN_LINKS -gt 0 ]]; then
  echo "Found $BROKEN_LINKS broken internal links."
  exit 1
else
  echo "✓ All internal markdown links validated successfully!"
fi

echo "=============================================================="
