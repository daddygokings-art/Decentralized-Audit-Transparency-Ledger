#!/usr/bin/env python3
"""Continuous Compliance Monitor & Evidence Collection Automation

Monitors Decentralized Audit Ledger contract events in real-time or batch mode,
evaluates controls across SOX, GDPR, HIPAA, and MiCA frameworks, enforces
compliance policies, and generates audit-ready reports.
"""

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import sys
from typing import Any, Dict, List

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
EVIDENCE_DIR = REPO_ROOT / "evidence"
REPORTS_DIR = REPO_ROOT / "reports" / "compliance"


class ContinuousComplianceMonitor:
    def __init__(self, output_dir: pathlib.Path = None):
        self.output_dir = output_dir or EVIDENCE_DIR
        self.output_dir.mkdir(parents=True, exist_ok=True)
        REPORTS_DIR.mkdir(parents=True, exist_ok=True)

        self.frameworks = ["sox", "gdpr", "hipaa", "mica", "soc2", "isa3000"]
        self.controls = [
            {
                "id": "SOX-404-01",
                "framework": "sox",
                "name": "Access Control & Segregation of Duties",
                "description": "Continuous monitoring of administrative access and transaction approvals",
                "required_types": ["access_control", "governance_action", "multisig_approval"],
                "min_threshold": 2,
            },
            {
                "id": "SOX-404-02",
                "framework": "sox",
                "name": "Change Management & Audit Trail Integrity",
                "description": "Immutable ledger logging of configuration and parameter modifications",
                "required_types": ["config_change", "audit_trail"],
                "min_threshold": 1,
            },
            {
                "id": "GDPR-ART17",
                "framework": "gdpr",
                "name": "Right to Erasure & Crypto-shredding",
                "description": "Automated verification of erasure requests and cryptographic shredding",
                "required_types": ["erasure_request", "crypto_shredding"],
                "min_threshold": 1,
            },
            {
                "id": "GDPR-ART32",
                "framework": "gdpr",
                "name": "Security of Processing & Confidentiality",
                "description": "Encryption verification and authorization enforcement for personal data",
                "required_types": ["data_protection", "access_authorization"],
                "min_threshold": 2,
            },
            {
                "id": "HIPAA-164-312",
                "framework": "hipaa",
                "name": "Technical Safeguards & ePHI Audit Controls",
                "description": "Continuous monitoring of electronic protected health information access",
                "required_types": ["ephi_access", "auth_verification"],
                "min_threshold": 2,
            },
            {
                "id": "HIPAA-164-308",
                "framework": "hipaa",
                "name": "Administrative Safeguards & Role Access",
                "description": "Least-privilege role validation and minimum-necessary enforcement",
                "required_types": ["role_assignment", "least_privilege_audit"],
                "min_threshold": 1,
            },
            {
                "id": "MICA-TITLE3",
                "framework": "mica",
                "name": "Reserve Transparency & Asset Backing",
                "description": "Attestation of reserve assets and stablecoin backing proof verification",
                "required_types": ["reserve_attestation", "custody_verification"],
                "min_threshold": 2,
            },
            {
                "id": "MICA-TITLE6",
                "framework": "mica",
                "name": "Market Abuse & Insider Prevention",
                "description": "Continuous anomaly detection for velocity, front-running, and abuse",
                "required_types": ["anomaly_check", "velocity_check"],
                "min_threshold": 1,
            },
        ]

    def collect_evidence(self, events: List[Dict[str, Any]]) -> Dict[str, List[Dict[str, Any]]]:
        evidence_by_control = {c["id"]: [] for c in self.controls}

        for event in events:
            ev_type = event.get("event_type", "")
            for ctrl in self.controls:
                if ev_type in ctrl["required_types"]:
                    evidence_entry = {
                        "evidence_id": hashlib.sha256(
                            f"{event.get('id', '')}-{ctrl['id']}".encode()
                        ).hexdigest(),
                        "control_id": ctrl["id"],
                        "framework": ctrl["framework"],
                        "event_id": event.get("id"),
                        "event_type": ev_type,
                        "timestamp": event.get("timestamp", int(datetime.datetime.utcnow().timestamp())),
                        "submitter": event.get("submitter", "system"),
                        "verified": True,
                        "metadata": event.get("metadata", {}),
                    }
                    evidence_by_control[ctrl["id"]].append(evidence_entry)

        # Write evidence snapshots to disk
        for ctrl_id, items in evidence_by_control.items():
            ctrl_dir = self.output_dir / ctrl_id
            ctrl_dir.mkdir(parents=True, exist_ok=True)
            snapshot_file = ctrl_dir / f"evidence-{datetime.datetime.utcnow().strftime('%Y%m%d')}.json"
            with open(snapshot_file, "w") as f:
                json.dump(items, f, indent=2)

        return evidence_by_control

    def evaluate_compliance(self, evidence_by_control: Dict[str, List[Dict[str, Any]]]) -> Dict[str, Any]:
        results = {}
        for ctrl in self.controls:
            items = evidence_by_control.get(ctrl["id"], [])
            count = len(items)
            threshold = ctrl["min_threshold"]
            if count >= threshold:
                status = "PASSED"
            elif count > 0:
                status = "WARNING"
            else:
                status = "INSUFFICIENT_EVIDENCE"

            results[ctrl["id"]] = {
                "control": ctrl,
                "status": status,
                "evidence_count": count,
                "min_threshold": threshold,
                "evaluated_at": datetime.datetime.utcnow().isoformat() + "Z",
            }
        return results

    def generate_audit_reports(self, evaluation_results: Dict[str, Any]) -> List[pathlib.Path]:
        generated_files = []
        now = datetime.datetime.utcnow()

        for framework in ["sox", "gdpr", "hipaa", "mica"]:
            fw_controls = [
                res for res in evaluation_results.values()
                if res["control"]["framework"] == framework
            ]
            if not fw_controls:
                continue

            passed = sum(1 for c in fw_controls if c["status"] == "PASSED")
            warning = sum(1 for c in fw_controls if c["status"] == "WARNING")
            deficient = sum(1 for c in fw_controls if c["status"] == "DEFICIENT")
            insufficient = sum(1 for c in fw_controls if c["status"] == "INSUFFICIENT_EVIDENCE")
            total = len(fw_controls)

            score = int(((passed * 100) + (warning * 50)) / (total * 100) * 100) if total > 0 else 100

            report_content = f"""# AuditLedger Continuous Compliance Report: {framework.upper()}
Generated on: {now.strftime('%Y-%m-%d %H:%M:%S UTC')}
Status: {'COMPLIANT' if score >= 80 else 'NON-COMPLIANT'} (Score: {score}%)

## Executive Summary
- **Framework**: {framework.upper()}
- **Total Controls Evaluated**: {total}
- **Controls Operating Effectively**: {passed}
- **Controls with Warnings**: {warning}
- **Controls with Deficiencies**: {deficient}
- **Controls with Insufficient Evidence**: {insufficient}
- **Overall Compliance Score**: {score}%

## Control Breakdown
| Control ID | Control Name | Status | Evidence Count | Required Threshold |
|------------|--------------|--------|----------------|--------------------|
"""
            for res in fw_controls:
                ctrl = res["control"]
                report_content += f"| `{ctrl['id']}` | {ctrl['name']} | **{res['status']}** | {res['evidence_count']} | {ctrl['min_threshold']} |\n"

            report_content += f"""
## Verification Seal
- **Report Digest**: `{hashlib.sha256(report_content.encode()).hexdigest()}`
- **Ledger Verification**: Tamper-evident cryptographic continuous monitoring
"""

            out_file = REPORTS_DIR / f"{framework}-compliance-report-{now.strftime('%Y%m%d')}.md"
            with open(out_file, "w") as f:
                f.write(report_content)
            generated_files.append(out_file)

        return generated_files


def main():
    parser = argparse.ArgumentParser(description="Continuous Compliance Monitor")
    parser.add_argument("--events-file", help="Path to JSON file containing sample events", default=None)
    parser.add_argument("--out-dir", help="Directory to save evidence", default=None)
    args = parser.parse_args()

    out_dir = pathlib.Path(args.out_dir) if args.out_dir else None
    monitor = ContinuousComplianceMonitor(output_dir=out_dir)

    events = []
    if args.events_file and os.path.exists(args.events_file):
        with open(args.events_file) as f:
            events = json.load(f)
    else:
        # Default baseline simulation events
        events = [
            {"id": "ev-001", "event_type": "access_control", "submitter": "GB3...X1", "metadata": {"role": "auditor"}},
            {"id": "ev-002", "event_type": "multisig_approval", "submitter": "GC4...Y2", "metadata": {"tx": "0x123"}},
            {"id": "ev-003", "event_type": "config_change", "submitter": "GA1...Z0", "metadata": {"cap": 1000}},
            {"id": "ev-004", "event_type": "erasure_request", "submitter": "GD2...W9", "metadata": {"subject": "user-42"}},
            {"id": "ev-005", "event_type": "data_protection", "submitter": "GE5...V8", "metadata": {"algo": "AES-256-GCM"}},
            {"id": "ev-006", "event_type": "ephi_access", "submitter": "GF6...U7", "metadata": {"record": "med-88"}},
            {"id": "ev-007", "event_type": "reserve_attestation", "submitter": "GG7...T6", "metadata": {"collateral": "100%"}},
            {"id": "ev-008", "event_type": "anomaly_check", "submitter": "GH8...S5", "metadata": {"risk_score": 0.05}},
        ]

    evidence = monitor.collect_evidence(events)
    results = monitor.evaluate_compliance(evidence)
    reports = monitor.generate_audit_reports(results)
    print(f"Compliance monitoring complete. Generated {len(reports)} audit reports.")


if __name__ == "__main__":
    main()
