import * as fs from 'fs';
import { PolicyEngine } from './engine';
import {
  AuditReport,
  ContractEvent,
  FrameworkComplianceSummary,
  PolicyEvaluationResult,
  PolicyViolation,
  RegulatoryFrameworkConfig,
  Severity
} from './types';
import * as yaml from 'js-yaml';
import * as path from 'path';

export class ComplianceEvaluator {
  private engine: PolicyEngine;
  private frameworksConfig: RegulatoryFrameworkConfig[];

  constructor(policyDir?: string, frameworksConfigPath?: string) {
    this.engine = new PolicyEngine(policyDir);
    const configPath = frameworksConfigPath || path.resolve(__dirname, '../../../policies/compliance/config/regulatory-frameworks.yaml');
    this.frameworksConfig = this.loadFrameworksConfig(configPath);
  }

  private loadFrameworksConfig(configPath: string): RegulatoryFrameworkConfig[] {
    try {
      if (fs.existsSync(configPath)) {
        const fileContent = fs.readFileSync(configPath, 'utf8');
        const parsed = yaml.load(fileContent) as { frameworks: RegulatoryFrameworkConfig[] };
        return parsed?.frameworks || [];
      }
    } catch {
      // Return default framework list if yaml fails
    }
    return [];
  }

  public evaluateEvents(events: ContractEvent[]): {
    policyResults: PolicyEvaluationResult[];
    allViolations: PolicyViolation[];
    frameworkSummaries: FrameworkComplianceSummary[];
    overallScore: number;
  } {
    const policyResults = this.engine.evaluate(events);
    const allViolations: PolicyViolation[] = [];
    for (const result of policyResults) {
      allViolations.push(...result.violations);
    }

    const frameworkSummaries = this.calculateFrameworkSummaries(allViolations);
    const totalChecks = policyResults.reduce((acc, r) => acc + r.metrics.total_events_evaluated, 0) || 1;
    const overallScore = Math.max(0, Math.round(((totalChecks - allViolations.length) / totalChecks) * 1000) / 10);

    return {
      policyResults,
      allViolations,
      frameworkSummaries,
      overallScore
    };
  }

  private calculateFrameworkSummaries(violations: PolicyViolation[]): FrameworkComplianceSummary[] {
    const summaries: FrameworkComplianceSummary[] = [];

    for (const fw of this.frameworksConfig) {
      const fwRuleIds = new Set(fw.controls.map(c => c.rule_id));
      const fwViolations = violations.filter(v => fwRuleIds.has(v.rule_id));
      const totalControls = fw.controls.length;
      const failingControls = new Set(fwViolations.map(v => v.rule_id)).size;
      const passingControls = Math.max(0, totalControls - failingControls);
      const compliancePct = totalControls > 0
        ? Math.round((passingControls / totalControls) * 100)
        : 100;

      let status: 'COMPLIANT' | 'NON_COMPLIANT' | 'WARNING' = 'COMPLIANT';
      if (fwViolations.some(v => v.severity === 'CRITICAL' || v.severity === 'HIGH')) {
        status = 'NON_COMPLIANT';
      } else if (fwViolations.length > 0) {
        status = 'WARNING';
      }

      summaries.push({
        framework_id: fw.id,
        framework_name: fw.name,
        total_controls: totalControls,
        passing_controls: passingControls,
        failing_controls: failingControls,
        compliance_pct: compliancePct,
        status,
        violations: fwViolations
      });
    }

    return summaries;
  }

  public generateReportObject(
    events: ContractEvent[],
    environment: string = 'production'
  ): AuditReport {
    const { policyResults, allViolations, frameworkSummaries, overallScore } = this.evaluateEvents(events);

    const severityBreakdown: Record<Severity, number> = {
      CRITICAL: 0,
      HIGH: 0,
      MEDIUM: 0,
      LOW: 0
    };

    for (const v of allViolations) {
      severityBreakdown[v.severity] = (severityBreakdown[v.severity] || 0) + 1;
    }

    let overallStatus: 'COMPLIANT' | 'NON_COMPLIANT' | 'NEEDS_ATTENTION' = 'COMPLIANT';
    if (severityBreakdown.CRITICAL > 0 || severityBreakdown.HIGH > 0) {
      overallStatus = 'NON_COMPLIANT';
    } else if (severityBreakdown.MEDIUM > 0 || severityBreakdown.LOW > 0) {
      overallStatus = 'NEEDS_ATTENTION';
    }

    return {
      report_id: `AUDIT-REP-${Date.now()}`,
      generated_at: new Date().toISOString(),
      environment,
      total_events: events.length,
      overall_compliance_score: overallScore,
      overall_status: overallStatus,
      total_violations: allViolations.length,
      severity_breakdown: severityBreakdown,
      frameworks: frameworkSummaries,
      policy_results: policyResults
    };
  }
}
