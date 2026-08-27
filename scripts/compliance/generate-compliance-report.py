#!/usr/bin/env python3
"""Generate a per-framework compliance report from
docs/compliance/control-matrix.yaml.

The control matrix is the single source of truth; this script produces
a derived, point-in-time Markdown report per framework (which controls
map to it, whether each is automated, and whether current evidence
exists under evidence/<control-id>/). It does not edit the matrix.

Usage:
    python3 generate-compliance-report.py --framework soc2 [--evidence-dir evidence] [--out report.md]
    python3 generate-compliance-report.py --all   # one report per framework
"""
import argparse
import datetime
import pathlib
import sys

try:
    import yaml
except ImportError:
    sys.exit("ERROR: PyYAML required (pip install pyyaml)")

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "docs" / "compliance" / "control-matrix.yaml"

FRAMEWORK_NAMES = {
    "soc2": "SOC 2 Type II",
    "iso27001": "ISO/IEC 27001:2022",
    "pci-dss": "PCI DSS v4.0",
    "gdpr": "GDPR",
    "mica": "MiCA (EU Markets in Crypto-Assets Regulation)",
}


def load_controls():
    with open(MATRIX_PATH) as f:
        data = yaml.safe_load(f)
    return data["controls"]


def has_recent_evidence(control_id: str, evidence_dir: pathlib.Path, max_age_days: int = 8) -> bool:
    ctrl_dir = evidence_dir / control_id
    if not ctrl_dir.is_dir():
        return False
    files = sorted(ctrl_dir.glob("*.json"))
    if not files:
        return False
    latest = files[-1].stem  # YYYY-MM-DD
    try:
        latest_date = datetime.date.fromisoformat(latest)
    except ValueError:
        return False
    return (datetime.date.today() - latest_date).days <= max_age_days


def render_report(framework: str, controls: list, evidence_dir: pathlib.Path) -> str:
    name = FRAMEWORK_NAMES.get(framework, framework)
    today = datetime.date.today().isoformat()
    lines = [
        f"# {name} — Control Coverage Report",
        "",
        f"Generated: {today}",
        "",
        "Source of truth: `docs/compliance/control-matrix.yaml`. This report is derived — edit the matrix, not this file.",
        "",
        "| Control | Framework ref(s) | Owner | Automated | Evidence current (<=8d) |",
        "|---|---|---|---|---|",
    ]
    matched = [c for c in controls if c.get("frameworks", {}).get(framework)]
    if not matched:
        lines.append(f"| _no controls mapped to {framework} yet_ | | | | |")
    for c in matched:
        refs = ", ".join(c["frameworks"][framework])
        automated = "Yes" if c.get("automated") else "No (manual)"
        evidence_ok = "N/A (manual)" if not c.get("automated") else (
            "Yes" if has_recent_evidence(c["id"], evidence_dir) else "**STALE/MISSING**"
        )
        lines.append(f"| {c['id']}: {c['title']} | {refs} | {c.get('owner', '')} | {automated} | {evidence_ok} |")

    stale = [
        c for c in matched
        if c.get("automated") and not has_recent_evidence(c["id"], evidence_dir)
    ]
    lines += ["", "## Gaps"]
    if stale:
        for c in stale:
            lines.append(f"- **{c['id']}** ({c['title']}): evidence missing or older than 8 days — check `.github/workflows/compliance-evidence.yml` run history.")
    else:
        lines.append("- None: all automated controls mapped to this framework have current evidence.")

    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--framework", choices=list(FRAMEWORK_NAMES) + ["all"], default="all")
    ap.add_argument("--evidence-dir", default=str(REPO_ROOT / "evidence"))
    ap.add_argument("--out-dir", default=str(REPO_ROOT / "compliance-reports"))
    args = ap.parse_args()

    controls = load_controls()
    evidence_dir = pathlib.Path(args.evidence_dir)
    out_dir = pathlib.Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    frameworks = list(FRAMEWORK_NAMES) if args.framework == "all" else [args.framework]
    for fw in frameworks:
        report = render_report(fw, controls, evidence_dir)
        out_path = out_dir / f"{fw}.md"
        out_path.write_text(report)
        print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()
