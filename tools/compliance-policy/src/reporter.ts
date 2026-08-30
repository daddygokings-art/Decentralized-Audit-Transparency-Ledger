import * as fs from 'fs';
import * as path from 'path';
import { AuditReport } from './types';

export class AuditReporter {
  public generateMarkdownReport(report: AuditReport): string {
    const statusBadge = report.overall_status === 'COMPLIANT'
      ? '🟢 **COMPLIANT**'
      : report.overall_status === 'NEEDS_ATTENTION'
      ? '🟡 **NEEDS ATTENTION**'
      : '🔴 **NON-COMPLIANT**';

    let md = `# Regulatory Compliance & Policy Audit Report

| Parameter | Value |
|---|---|
| **Report ID** | \`${report.report_id}\` |
| **Generated At** | \`${report.generated_at}\` |
| **Environment** | \`${report.environment}\` |
| **Overall Status** | ${statusBadge} |
| **Compliance Score** | **${report.overall_compliance_score}%** |
| **Total Events Evaluated** | ${report.total_events} |
| **Total Policy Violations** | ${report.total_violations} |

---

## 1. Executive Summary

- **Critical Violations**: ${report.severity_breakdown.CRITICAL}
- **High Violations**: ${report.severity_breakdown.HIGH}
- **Medium Violations**: ${report.severity_breakdown.MEDIUM}
- **Low Violations**: ${report.severity_breakdown.LOW}

`;

    md += `## 2. Regulatory Framework Breakdown\n\n`;
    md += `| Framework | Total Controls | Passing | Failing | Compliance Rate | Status |\n`;
    md += `|---|---|---|---|---|---|\n`;

    for (const fw of report.frameworks) {
      const fwStatus = fw.status === 'COMPLIANT'
        ? '🟢 Compliant'
        : fw.status === 'WARNING'
        ? '🟡 Warning'
        : '🔴 Non-Compliant';

      md += `| **${fw.framework_name}** (\`${fw.framework_id}\`) | ${fw.total_controls} | ${fw.passing_controls} | ${fw.failing_controls} | ${fw.compliance_pct}% | ${fwStatus} |\n`;
    }

    if (report.drift) {
      md += `\n---\n\n## 3. Drift Detection Analysis\n\n`;
      md += `- **Baseline ID**: \`${report.drift.baseline_id}\`\n`;
      md += `- **Drift Detected**: ${report.drift.has_drift ? '⚠️ **YES**' : '✅ **NO**'}\n`;
      md += `- **Current Score**: ${report.drift.current_score_pct}%\n`;
      md += `- **Baseline Score**: ${report.drift.baseline_score_pct}%\n`;
      md += `- **Delta**: ${report.drift.score_delta > 0 ? `+${report.drift.score_delta}` : report.drift.score_delta}%\n\n`;

      if (report.drift.findings.length > 0) {
        md += `### Drift Findings\n\n`;
        md += `| Drift ID | Category | Severity | Framework | Description |\n`;
        md += `|---|---|---|---|---|\n`;
        for (const f of report.drift.findings) {
          md += `| \`${f.drift_id}\` | ${f.category} | **${f.severity}** | ${f.framework} | ${f.message} |\n`;
        }
        md += `\n`;
      }
    }

    md += `---\n\n## 4. Policy Package Evaluation Details\n\n`;
    for (const pkg of report.policy_results) {
      const pkgStatus = pkg.compliant ? '✅ Pass' : '❌ Fail';
      md += `### \`${pkg.policy_package}\` — ${pkgStatus}\n`;
      md += `- **Events Evaluated**: ${pkg.metrics.total_events_evaluated}\n`;
      md += `- **Violations Count**: ${pkg.metrics.total_violations}\n`;

      if (pkg.violations.length > 0) {
        md += `\n| Rule ID | Title | Severity | Event ID | Message |\n`;
        md += `|---|---|---|---|---|\n`;
        for (const v of pkg.violations) {
          md += `| \`${v.rule_id}\` | ${v.title} | **${v.severity}** | \`${v.event_id}\` | ${v.message} |\n`;
        }
      }
      md += `\n`;
    }

    md += `---\n\n*Generated automatically by AuditLedger Continuous Compliance Engine (@audit-ledger/compliance-policy)*\n`;
    return md;
  }

  public saveReport(report: AuditReport, outputDir: string): { jsonPath: string; mdPath: string } {
    if (!fs.existsSync(outputDir)) {
      fs.mkdirSync(outputDir, { recursive: true });
    }

    const baseName = `compliance-audit-report-${Date.now()}`;
    const jsonPath = path.join(outputDir, `${baseName}.json`);
    const mdPath = path.join(outputDir, `${baseName}.md`);

    fs.writeFileSync(jsonPath, JSON.stringify(report, null, 2), 'utf-8');
    fs.writeFileSync(mdPath, this.generateMarkdownReport(report), 'utf-8');

    return { jsonPath, mdPath };
  }
}
