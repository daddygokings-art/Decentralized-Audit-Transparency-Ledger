#!/usr/bin/env bash
set -euo pipefail

echo "================ BUILDING MULTI-VERSION DOCUMENTATION ================"

SITE_DIR="site"
VERSIONS=("v1.0.0" "v2.0.0" "latest" "dev")

mkdir -p "$SITE_DIR"

if command -v mike &> /dev/null; then
  echo "Building with mike multi-version manager..."
  mike deploy --push --update-aliases v2.0.0 latest
  mike set-default latest
else
  echo "Building static multi-version preview structure..."
  for version in "${VERSIONS[@]}"; do
    mkdir -p "$SITE_DIR/$version"
    cat <<EOF > "$SITE_DIR/$version/index.html"
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>AuditLedger Documentation - ${version}</title>
  <meta http-equiv="refresh" content="0; url=../../README.html">
</head>
<body>
  <h1>Decentralized Audit Transparency Ledger Docs (${version})</h1>
  <p>Redirecting to documentation...</p>
</body>
</html>
EOF
  done

  # Root redirect to latest
  cat <<EOF > "$SITE_DIR/index.html"
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>AuditLedger Documentation</title>
  <meta http-equiv="refresh" content="0; url=./latest/index.html">
</head>
<body>
  <h1>AuditLedger Documentation</h1>
  <p>Redirecting to <a href="./latest/index.html">latest version</a>...</p>
</body>
</html>
EOF
fi

echo "✓ Multi-version documentation built successfully in '$SITE_DIR'!"
echo "======================================================================"
