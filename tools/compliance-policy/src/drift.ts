import {
  ContractEvent,
  DriftDetectionResult,
  DriftFinding
} from './types';

export class DriftDetector {
  /**
   * Compares current contract state and event stream against an approved baseline snapshot
   */
  public detectDrift(
    currentEvents: ContractEvent[],
    baselineSnapshot: any,
    currentConfig?: Record<string, any>
  ): DriftDetectionResult {
    const findings: DriftFinding[] = [];
    const baselineConfig = baselineSnapshot.config || {};
    const effectiveConfig = currentConfig || baselineConfig;

    // 1. Quorum Drift
    if (
      typeof effectiveConfig.min_multisig_quorum === 'number' &&
      typeof baselineConfig.min_multisig_quorum === 'number' &&
      effectiveConfig.min_multisig_quorum < baselineConfig.min_multisig_quorum
    ) {
      findings.push({
        drift_id: 'DRIFT-CFG-001',
        category: 'Governance Quorum Downgrade',
        severity: 'CRITICAL',
        framework: 'SOC 2 CC6.1',
        current_value: effectiveConfig.min_multisig_quorum,
        baseline_value: baselineConfig.min_multisig_quorum,
        message: `Governance quorum weakened from baseline ${baselineConfig.min_multisig_quorum} to ${effectiveConfig.min_multisig_quorum}`
      });
    }

    // 2. Unregistered Event Topic / Schema Drift
    const knownTopics = new Set([
      'anti_corruption',
      'export_controls',
      'trade_compliance',
      'data_retention',
      'stablecoin_reserves',
      'rwa_compliance',
      'rwa_asset',
      'cbdc_logging',
      'asset_lifecycle',
      'defi_auditing',
      'tax_audit_trail',
      'governance',
      'admin',
      'core_ledger',
      'tamper_evidence',
      'bridge'
    ]);

    for (const evt of currentEvents) {
      if (!knownTopics.has(evt.topic)) {
        findings.push({
          drift_id: 'DRIFT-SCHEMA-003',
          category: 'Unregistered Event Schema Drift',
          severity: 'MEDIUM',
          framework: 'SOC 2 CC6.8',
          current_value: evt.topic,
          baseline_value: 'Registered Event Schema',
          message: `Event '${evt.id}' emitted with undocumented topic '${evt.topic}'`
        });
      }
    }

    // 3. Score degradation drift
    const baselineScore = baselineSnapshot.metrics?.compliance_score_pct ?? 100.0;
    // Calculate current compliance score based on drift findings and event anomalies
    const currentScore = Math.max(0, Math.round((100 - findings.length * 3.5) * 10) / 10);
    const scoreDelta = Math.round((currentScore - baselineScore) * 10) / 10;

    if (currentScore < 95.0) {
      findings.push({
        drift_id: 'DRIFT-METRIC-002',
        category: 'Compliance Score Degradation',
        severity: 'HIGH',
        framework: 'ISO 27001 / SOC 2',
        current_value: currentScore,
        baseline_value: 95.0,
        message: `Aggregate compliance score dropped to ${currentScore}% (baseline threshold: 95.0%)`
      });
    }

    return {
      baseline_id: baselineSnapshot.baseline_id || 'UNKNOWN_BASELINE',
      detected_at: new Date().toISOString(),
      has_drift: findings.length > 0,
      total_findings: findings.length,
      findings,
      score_delta: scoreDelta,
      current_score_pct: currentScore,
      baseline_score_pct: baselineScore
    };
  }
}
