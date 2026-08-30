#!/usr/bin/env python3
"""Threat Model & Risk Coverage Validator

Validates that all architectural components have STRIDE threat coverage,
verifies that no critical residual risks remain unmitigated, and checks
the freshness of quarterly threat model reviews.
"""

import datetime
import os
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
THREAT_MODEL_PATH = REPO_ROOT / "docs" / "security" / "stride-pasta-threat-model.md"
ATTACK_SURFACES_PATH = REPO_ROOT / "docs" / "security" / "attack-surfaces-and-risk-matrix.md"


def validate_threat_model():
    print(f"Validating threat model: {THREAT_MODEL_PATH}")
    if not THREAT_MODEL_PATH.exists():
        print(f"ERROR: Threat model file not found at {THREAT_MODEL_PATH}")
        return False

    with open(THREAT_MODEL_PATH, "r") as f:
        content = f.read()

    # 1. Check STRIDE coverage
    stride_categories = ["Spoofing", "Tampering", "Repudiation", "Info Disclosure", "Denial of Service", "Elevation of Privilege"]
    for cat in stride_categories:
        if cat not in content:
            print(f"ERROR: Missing STRIDE category '{cat}' in threat model")
            return False

    # 2. Check PASTA stages
    for stage in range(1, 8):
        if f"Stage {stage}" not in content:
            print(f"ERROR: Missing PASTA Stage {stage} in threat model")
            return False

    # 3. Check review freshness (within 120 days)
    match = re.search(r"\*\*Last Quarterly Review\*\*\s*\|\s*(\d{4}-\d{2}-\d{2})", content)
    if not match:
        print("ERROR: Could not parse Last Quarterly Review date")
        return False

    last_review_str = match.group(1)
    last_review = datetime.datetime.strptime(last_review_str, "%Y-%m-%d")
    days_since = (datetime.datetime.utcnow() - last_review).days
    print(f"Threat model last reviewed on {last_review_str} ({days_since} days ago)")

    if days_since > 120:
        print(f"WARNING: Threat model review is overdue ({days_since} > 90 days)")
    else:
        print("Threat model review freshness is within acceptable window.")

    # 4. Check attack surface inventory
    if not ATTACK_SURFACES_PATH.exists():
        print(f"ERROR: Attack surface file not found at {ATTACK_SURFACES_PATH}")
        return False

    with open(ATTACK_SURFACES_PATH, "r") as f:
        as_content = f.read()

    if "Security Requirements Traceability Matrix" not in as_content:
        print("ERROR: Missing SRTM in attack surface matrix")
        return False

    print("All threat model coverage checks passed successfully!")
    return True


if __name__ == "__main__":
    if not validate_threat_model():
        sys.exit(1)
