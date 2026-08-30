import {
  SubmitterActivityRecord,
  GovernanceActionRecord,
  BridgeTransferRecord,
  ApiCallRecord,
  ExecutiveKPISummary,
} from './types';
import { SubmitterMetricsCalculator } from './calculators/submitter';
import { EventGrowthMetricsCalculator, EventRecord } from './calculators/growth';
import { GovernanceMetricsCalculator } from './calculators/governance';
import { BridgeThroughputMetricsCalculator } from './calculators/bridge';
import { ApiAdoptionMetricsCalculator } from './calculators/apiAdoption';

export class BusinessMetricsAggregator {
  private submitterRecords: SubmitterActivityRecord[] = [];
  private eventRecords: EventRecord[] = [];
  private governanceRecords: GovernanceActionRecord[] = [];
  private bridgeRecords: BridgeTransferRecord[] = [];
  private apiRecords: ApiCallRecord[] = [];
  private historicalDailyVolumes: number[] = [];

  public recordSubmitterActivity(record: SubmitterActivityRecord): void {
    this.submitterRecords.push(record);
  }

  public recordEvent(record: EventRecord): void {
    this.eventRecords.push(record);
  }

  public recordGovernanceAction(record: GovernanceActionRecord): void {
    this.governanceRecords.push(record);
  }

  public recordBridgeTransfer(record: BridgeTransferRecord): void {
    this.bridgeRecords.push(record);
  }

  public recordApiCall(record: ApiCallRecord): void {
    this.apiRecords.push(record);
  }

  public setHistoricalDailyVolumes(volumes: number[]): void {
    this.historicalDailyVolumes = [...volumes];
  }

  /**
   * Computes an overall business health score from 0 to 100 based on composite KPIs:
   * - Retention & Submitter Activity (25%)
   * - Growth Rate & Anomaly State (25%)
   * - Bridge Success Rate & SLA (25%)
   * - API SLA Compliance & Error Rate (25%)
   */
  private calculateHealthScore(
    submitters: ReturnType<typeof SubmitterMetricsCalculator.calculate>,
    growth: ReturnType<typeof EventGrowthMetricsCalculator.calculate>,
    bridge: ReturnType<typeof BridgeThroughputMetricsCalculator.calculate>,
    api: ReturnType<typeof ApiAdoptionMetricsCalculator.calculate>
  ): number {
    let score = 100;

    // Submitter penalty if centralization is too high or retention < 50%
    if (submitters.giniCoefficient > 0.8) score -= 10;
    if (submitters.retentionRate7d < 50) score -= 10;

    // Growth penalty if volume is anomalous drop
    if (growth.isAnomaly && growth.anomalyScore < -2.5) score -= 20;

    // Bridge penalty if verification failure rate > 5%
    if (bridge.verificationSuccessRatePct < 95) score -= 20;

    // API penalty if SLA compliance < 95% or error rate > 5%
    if (api.slaCompliancePct < 95) score -= 15;
    if (api.errorRatePct > 5) score -= 15;

    return Math.max(0, Math.min(100, score));
  }

  public generateExecutiveSummary(now: number = Date.now()): ExecutiveKPISummary {
    const submitters = SubmitterMetricsCalculator.calculate(this.submitterRecords, now);
    const growth = EventGrowthMetricsCalculator.calculate(
      this.eventRecords,
      this.historicalDailyVolumes,
      now
    );
    const governance = GovernanceMetricsCalculator.calculate(this.governanceRecords);
    const bridge = BridgeThroughputMetricsCalculator.calculate(this.bridgeRecords);
    const apiAdoption = ApiAdoptionMetricsCalculator.calculate(this.apiRecords, 200, now);

    const healthScore = this.calculateHealthScore(submitters, growth, bridge, apiAdoption);

    return {
      timestamp: new Date(now).toISOString(),
      period: '24h',
      healthScore,
      submitters,
      growth,
      governance,
      bridge,
      apiAdoption,
    };
  }
}
