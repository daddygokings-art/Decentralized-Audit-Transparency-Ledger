import { FeatureFlag } from './types.js';

export interface CanaryMetrics {
  totalRequests: number;
  errorCount: number;
  errorRateBps: number;
  p99LatencyMs: number;
}

export class ProgressiveDeliveryController {
  private metricsStore: Map<string, CanaryMetrics> = new Map();

  public recordMetric(flagKey: string, isError: boolean, latencyMs: number): void {
    const current = this.metricsStore.get(flagKey) || {
      totalRequests: 0,
      errorCount: 0,
      errorRateBps: 0,
      p99LatencyMs: latencyMs,
    };

    current.totalRequests++;
    if (isError) current.errorCount++;
    current.errorRateBps = Math.round((current.errorCount / current.totalRequests) * 10000);
    current.p99LatencyMs = Math.max(current.p99LatencyMs, latencyMs);

    this.metricsStore.set(flagKey, current);
  }

  public shouldAutoAdvance(flag: FeatureFlag): boolean {
    if (!flag.canary.isActive || !flag.canary.autoPromote) return false;

    const metrics = this.metricsStore.get(flag.key);
    if (!metrics || metrics.totalRequests < 100) return false;

    return metrics.errorRateBps <= flag.canary.errorThresholdBps;
  }

  public shouldRollback(flag: FeatureFlag): boolean {
    if (!flag.canary.isActive) return false;

    const metrics = this.metricsStore.get(flag.key);
    if (!metrics || metrics.totalRequests < 50) return false;

    return metrics.errorRateBps > flag.canary.errorThresholdBps;
  }

  public advanceStage(flag: FeatureFlag): FeatureFlag {
    const nextPercentage = Math.min(100, Math.min(flag.canary.targetPercentage, flag.canary.currentPercentage + flag.canary.stepPercentage));
    
    return {
      ...flag,
      canary: {
        ...flag.canary,
        currentPercentage: nextPercentage,
        currentStage: flag.canary.currentStage + 1,
        lastPromotedAt: Date.now(),
        isActive: nextPercentage < 100,
      },
      status: nextPercentage >= 100 ? 'graduated' : 'active',
    };
  }

  public rollbackCanary(flag: FeatureFlag, reason: string): FeatureFlag {
    return {
      ...flag,
      canary: {
        ...flag.canary,
        currentPercentage: 0,
        isActive: false,
      },
      status: 'inactive',
      description: `${flag.description} [Rolled back: ${reason}]`,
    };
  }
}
