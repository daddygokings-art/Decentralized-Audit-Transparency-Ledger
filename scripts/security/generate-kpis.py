#!/usr/bin/env python3
"""Generate a stable security KPI and board-report JSON document."""

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path


def load_json(path, default):
    if not path:
        return default
    with Path(path).open(encoding="utf-8") as handle:
        return json.load(handle)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--findings", required=True)
    parser.add_argument("--training")
    parser.add_argument("--phishing")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    findings = load_json(args.findings, [])
    if isinstance(findings, dict):
        findings = findings.get("findings", findings.get("normalized_findings", []))
    open_findings = [item for item in findings if item.get("status", "open") != "closed"]
    resolved = [item for item in findings if item.get("resolved_at") and item.get("first_seen")]
    aging_days = [
        (datetime.fromisoformat(item["resolved_at"].replace("Z", "+00:00"))
         - datetime.fromisoformat(item["first_seen"].replace("Z", "+00:00"))).days
        for item in resolved
    ]
    severity = {level: sum(item.get("severity", "unknown").lower() == level for item in open_findings)
                for level in ("critical", "high", "medium", "low")}
    training = load_json(args.training, {})
    phishing = load_json(args.phishing, {})
    report = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "metrics": {
            "mttd_days": training.get("mttd_days"),
            "mttr_days": round(sum(aging_days) / len(aging_days), 2) if aging_days else None,
            "vulnerability_aging_days": max(aging_days, default=0),
            "patch_compliance_percent": training.get("patch_compliance_percent"),
            "phishing_click_rate_percent": phishing.get("click_rate_percent"),
            "security_training_completion_percent": training.get("completion_percent"),
        },
        "open_vulnerabilities": {"total": len(open_findings), **severity},
        "board_summary": {
            "risk_status": "attention_required" if severity["critical"] or severity["high"] else "within_target",
            "remediation_sample_size": len(resolved),
        },
    }
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()