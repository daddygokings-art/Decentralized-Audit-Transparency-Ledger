export interface TelemetryPoint {
  timestamp: number;
  tps: number;
  cpuUtilizationPercent: number;
  memoryMbUsed: number;
  storageBytesUsed: number;
  queueDepth: number;
  activeSubmitters: number;
  gasSpentStroops: number;
}

export interface ForecastResult {
  horizonMinutes: number;
  predictedTps: number;
  upperConfidenceTps: number;
  lowerConfidenceTps: number;
  trend: 'INCREASING' | 'STABLE' | 'DECREASING';
  seasonalFactor: number;
  anomalyScore: number;
  timestamp: string;
}

export interface ScalingDecision {
  currentReplicas: number;
  recommendedReplicas: number;
  minReplicas: number;
  maxReplicas: number;
  scalingReason: string;
  isScaleUp: boolean;
  isScaleDown: boolean;
  cpuRequestMillicores: number;
  memoryRequestMb: number;
  cooldownActive: boolean;
  confidenceScorePercent: number;
}

export interface CostOptimizationReport {
  currentMonthlyCostUsd: number;
  optimizedMonthlyCostUsd: number;
  estimatedMonthlySavingsUsd: number;
  savingsPercentage: number;
  recommendations: Array<{
    category: 'RIGHTSIZING' | 'SPOT_INSTANCES' | 'RESERVED_CAPACITY' | 'IDLE_CLEANUP';
    description: string;
    estimatedSavingsUsd: number;
    impactLevel: 'LOW' | 'MEDIUM' | 'HIGH';
  }>;
}

export interface SubmitterQuotaTier {
  id: number;
  name: string;
  maxDailyEvents: number;
  maxBurstTps: number;
  storageQuotaBytes: number;
  pricePerMillionEventsUsd: number;
}
