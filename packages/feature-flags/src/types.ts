/**
 * Types and interfaces for AuditLedger Feature Flags & Progressive Delivery
 */

export type FlagType = 'boolean' | 'percentage_rollout' | 'multivariate' | 'kill_switch';
export type FlagStatus = 'active' | 'inactive' | 'killed' | 'graduated';

export interface EvaluationContext {
  userId: string;
  caller: string;
  environment: 'development' | 'testnet' | 'mainnet';
  clientVersion?: string;
  attributes?: Record<string, string | number | boolean>;
}

export interface CanaryConfig {
  isActive: boolean;
  currentPercentage: number;
  targetPercentage: number;
  stepPercentage: number;
  evaluationWindowSeconds: number;
  errorThresholdBps: number;
  currentStage: number;
  autoPromote: boolean;
  lastPromotedAt?: number;
}

export interface ExperimentConfig {
  isActive: boolean;
  experimentId: string;
  variants: string[];
  weights: number[];
  winnerVariant?: string;
}

export interface KillSwitchConfig {
  isTriggered: boolean;
  triggeredBy: string;
  reason: string;
  triggeredAt: number;
  affectedEventTypes: string[];
}

export interface FeatureFlag {
  key: string;
  type: FlagType;
  status: FlagStatus;
  defaultValue: boolean;
  canary: CanaryConfig;
  experiment: ExperimentConfig;
  killSwitch: KillSwitchConfig;
  updatedAt: number;
  updatedBy: string;
  description: string;
}

export interface EvaluationDetail<T = boolean | string> {
  flagKey: string;
  value: T;
  variant?: string;
  reason: 'kill_switch' | 'canary_rollout' | 'experiment' | 'graduated' | 'default' | 'error';
  isKillSwitchActive: boolean;
}
