#!/usr/bin/env python3
"""
Simulate and Orchestrate Developer Phishing Campaigns
Logs responses, calculates resiliency metrics, and exports audit records.
"""

import argparse
import json
import random
import time
from datetime import datetime

CAMPAIGN_TEMPLATES = {
    "PS-01": {
        "title": "Emergency soroban-sdk zero-day patch notice",
        "vector": "Dependency Compromise",
        "difficulty": "Hard",
    },
    "PS-02": {
        "title": "Urgent PR review request from core maintainer",
        "vector": "Contributor Impersonation",
        "difficulty": "Medium",
    },
    "PS-03": {
        "title": "Docker Hub container registry token expiration",
        "vector": "Credential Harvesting",
        "difficulty": "Medium",
    },
    "PS-04": {
        "title": "Unauthorized AWS IAM role assumption alert",
        "vector": "Cloud Infrastructure",
        "difficulty": "Hard",
    },
}

def simulate_campaign(campaign_id: int, template_id: str, num_targets: int):
    template = CAMPAIGN_TEMPLATES.get(template_id, CAMPAIGN_TEMPLATES["PS-01"])
    print(f"[*] Launching Phishing Campaign #{campaign_id}: {template['title']}")
    print(f"[*] Vector: {template['vector']} | Difficulty: {template['difficulty']}")
    print(f"[*] Target Group Size: {num_targets} developers\n")

    results = {
        "reported": 0,
        "ignored": 0,
        "clicked": 0,
        "compromised": 0,
    }

    # Simulate responses based on typical champion-trained distribution
    for _ in range(num_targets):
        roll = random.random()
        if roll < 0.88:
            results["reported"] += 1
        elif roll < 0.96:
            results["ignored"] += 1
        elif roll < 0.99:
            results["clicked"] += 1
        else:
            results["compromised"] += 1

    report_rate = (results["reported"] / num_targets) * 100
    click_rate = (results["clicked"] / num_targets) * 100
    compromise_rate = (results["compromised"] / num_targets) * 100

    report = {
        "campaign_id": campaign_id,
        "template": template_id,
        "timestamp": int(time.time()),
        "total_targets": num_targets,
        "metrics": {
            "reported_count": results["reported"],
            "report_rate_pct": round(report_rate, 2),
            "clicked_count": results["clicked"],
            "click_rate_pct": round(click_rate, 2),
            "compromised_count": results["compromised"],
            "compromise_rate_pct": round(compromise_rate, 2),
        },
        "status": "PASSED" if report_rate >= 85.0 and compromise_rate == 0.0 else "REMEDIATION_REQUIRED",
    }

    print("=== Simulation Results ===")
    print(f"Reported (Positive Response): {results['reported']} ({report['metrics']['report_rate_pct']}%)")
    print(f"Clicked Link (Failure):        {results['clicked']} ({report['metrics']['click_rate_pct']}%)")
    print(f"Compromised (Critical):        {results['compromised']} ({report['metrics']['compromise_rate_pct']}%)")
    print(f"Overall Campaign Status:       {report['status']}")

    return report

def main():
    parser = argparse.ArgumentParser(description="Phishing simulation campaign runner")
    parser.add_argument("--campaign-id", type=int, default=101, help="Simulation Campaign ID")
    parser.add_argument("--template", type=str, default="PS-01", choices=CAMPAIGN_TEMPLATES.keys())
    parser.add_argument("--targets", type=int, default=50, help="Number of simulated target developers")
    parser.add_argument("--output", type=str, default="phishing_report.json", help="Output report JSON file")

    args = parser.parse_args()
    report = simulate_campaign(args.campaign_id, args.template, args.targets)

    with open(args.output, "w") as f:
        json.dump(report, f, indent=2)
    print(f"\n[+] Saved report to {args.output}")

if __name__ == "__main__":
    main()
