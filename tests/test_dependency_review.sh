#!/bin/bash
# Test: Verify dependency-review.yml workflow configuration (Issue #143)

set -e

WORKFLOW_FILE=".github/workflows/dependency-review.yml"

echo "[test] Checking dependency-review.yml exists..."
if [ ! -f "$WORKFLOW_FILE" ]; then
  echo "ERROR: $WORKFLOW_FILE not found"
  exit 1
fi
echo "✓ Workflow file exists"

echo "[test] Checking workflow triggers on pull_request..."
if ! grep -q "pull_request:" "$WORKFLOW_FILE"; then
  echo "ERROR: Workflow does not trigger on pull_request"
  exit 1
fi
echo "✓ Workflow triggers on pull_request"

echo "[test] Checking workflow targets main branch..."
if ! grep -q "main" "$WORKFLOW_FILE"; then
  echo "ERROR: Workflow does not specify main branch"
  exit 1
fi
echo "✓ Workflow targets main branch"

echo "[test] Checking dependency-review-action@v4..."
if ! grep -q "actions/dependency-review-action@v4" "$WORKFLOW_FILE"; then
  echo "ERROR: Workflow does not use dependency-review-action@v4"
  exit 1
fi
echo "✓ Workflow uses dependency-review-action@v4"

echo "[test] Checking fail-on-severity: high..."
if ! grep -q "fail-on-severity: high" "$WORKFLOW_FILE"; then
  echo "ERROR: Workflow does not fail on high-severity vulnerabilities"
  exit 1
fi
echo "✓ Workflow fails on high-severity CVEs"

echo "[test] Checking allow-licenses configuration..."
if ! grep -q "allow-licenses:" "$WORKFLOW_FILE"; then
  echo "ERROR: Workflow does not specify allowed licenses"
  exit 1
fi
echo "✓ Workflow specifies allowed licenses"

echo ""
echo "✓ All dependency-review.yml tests passed!"
exit 0
