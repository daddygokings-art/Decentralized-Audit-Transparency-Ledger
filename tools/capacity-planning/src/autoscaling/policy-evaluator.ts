import { ForecastResult, ScalingDecision, TelemetryPoint } from '../types';

export class ScalingPolicyEvaluator {
  private targetTpsPerReplica = 50; // Each relayer/API replica comfortably handles 50 TPS
  private targetCpuPercent = 70;
  private minReplicas = 2;
  private maxReplicas = 20;
  private cooldownSeconds = 300;
  private lastScalingTime = 0;

  public evaluate(currentMetrics: TelemetryPoint, forecast: ForecastResult, currentReplicas: number): ScalingDecision {
    const now = Date.now();
    const isCoolingDown = (now - this.lastScalingTime) < this.cooldownSeconds * 1000;

    // Use peak of predicted vs current TPS to prevent under-provisioning
    const effectiveTps = Math.max(currentMetrics.tps, forecast.predictedTps, forecast.upperConfidenceTps * 0.85);

    // Compute required replicas with 20% safety headroom
    const rawReplicas = Math.ceil((effectiveTps / this.targetTpsPerReplica) * 1.2);
    const boundedReplicas = Math.min(this.maxReplicas, Math.max(this.minReplicas, rawReplicas));

    const isScaleUp = boundedReplicas > currentReplicas;
    const isScaleDown = boundedReplicas < currentReplicas && !isCoolingDown;

    let scalingReason = 'Stable workload within normal bounds';
    if (isScaleUp) {
      scalingReason = `Proactive scaling triggered by predicted TPS spike (${effectiveTps.toFixed(1)} TPS)`;
      this.lastScalingTime = now;
    } else if (isScaleDown) {
      scalingReason = `Scale down: workload decreased to ${effectiveTps.toFixed(1)} TPS`;
      this.lastScalingTime = now;
    } else if (boundedReplicas < currentReplicas && isCoolingDown) {
      scalingReason = 'Scale down delayed by cooldown stabilization window';
    }

    // Vertical resource recommendation
    const baseCpu = 250; // millicores
    const baseMem = 512; // MB
    const cpuScaleFactor = Math.max(1, currentMetrics.cpuUtilizationPercent / this.targetCpuPercent);
    const cpuRequestMillicores = Math.round(baseCpu * cpuScaleFactor);
    const memoryRequestMb = Math.round(baseMem * Math.max(1, currentMetrics.memoryMbUsed / 400));

    return {
      currentReplicas,
      recommendedReplicas: isScaleDown && isCoolingDown ? currentReplicas : boundedReplicas,
      minReplicas: this.minReplicas,
      maxReplicas: this.maxReplicas,
      scalingReason,
      isScaleUp,
      isScaleDown: isScaleDown && !isCoolingDown,
      cpuRequestMillicores,
      memoryRequestMb,
      cooldownActive: isCoolingDown,
      confidenceScorePercent: Math.round((1 - forecast.anomalyScore) * 100),
    };
  }
}
