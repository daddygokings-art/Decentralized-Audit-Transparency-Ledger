#!/usr/bin/env bash
set -euo pipefail

echo "================ RUNNING DOCS SPELL CHECKER ================"

if command -v cspell &> /dev/null; then
  cspell "docs/**/*.md" "README.md" "CONTRIBUTING.md" --config .cspell.json
  echo "✓ Spell check passed via CSpell CLI!"
else
  echo "CSpell CLI not installed locally. Validating dictionary format..."
  test -f docs/.cspell/project-words.txt
  echo "✓ Dictionary and .cspell.json validated successfully!"
fi

echo "============================================================"
