import { EvaluationContext, EvaluationDetail, FeatureFlag } from './types.js';

/**
 * OpenFeature / LaunchDarkly compatible feature flag provider for AuditLedger
 */
export class AuditLedgerFlagProvider {
  private flags: Map<string, FeatureFlag> = new Map();
  private cacheTtlMs: number = 5000;
  private lastSyncTime: number = 0;

  constructor(private readonly rpcUrl?: string, private readonly contractId?: string) {}

  public registerFlag(flag: FeatureFlag): void {
    this.flags.set(flag.key, flag);
  }

  public getFlag(key: string): FeatureFlag | undefined {
    return this.flags.get(key);
  }

  public async evaluateBoolean(key: string, defaultValue: boolean, context: EvaluationContext): Promise<EvaluationDetail<boolean>> {
    const flag = this.flags.get(key);
    if (!flag) {
      return {
        flagKey: key,
        value: defaultValue,
        reason: 'default',
        isKillSwitchActive: false,
      };
    }

    // 1. Emergency Kill Switch Check
    if (flag.status === 'killed' || flag.killSwitch.isTriggered) {
      return {
        flagKey: key,
        value: false,
        reason: 'kill_switch',
        isKillSwitchActive: true,
      };
    }

    // 2. Inactive Flag
    if (flag.status === 'inactive') {
      return {
        flagKey: key,
        value: flag.defaultValue,
        reason: 'default',
        isKillSwitchActive: false,
      };
    }

    // 3. Graduated Flag (100% permanently on)
    if (flag.status === 'graduated') {
      return {
        flagKey: key,
        value: true,
        reason: 'graduated',
        isKillSwitchActive: false,
      };
    }

    // 4. Progressive Canary Rollout
    if (flag.canary.isActive && flag.canary.currentPercentage > 0) {
      const bucket = this.calculateBucket(key, context.userId || context.caller);
      const isIncluded = bucket < flag.canary.currentPercentage;
      return {
        flagKey: key,
        value: isIncluded,
        variant: isIncluded ? 'canary' : 'baseline',
        reason: 'canary_rollout',
        isKillSwitchActive: false,
      };
    }

    return {
      flagKey: key,
      value: flag.defaultValue,
      reason: 'default',
      isKillSwitchActive: false,
    };
  }

  public async evaluateVariant(key: string, fallback: string, context: EvaluationContext): Promise<EvaluationDetail<string>> {
    const flag = this.flags.get(key);
    if (!flag || !flag.experiment.isActive || flag.experiment.variants.length === 0) {
      return {
        flagKey: key,
        value: fallback,
        reason: 'default',
        isKillSwitchActive: false,
      };
    }

    if (flag.status === 'killed' || flag.killSwitch.isTriggered) {
      return {
        flagKey: key,
        value: 'killed',
        reason: 'kill_switch',
        isKillSwitchActive: true,
      };
    }

    const bucket = this.calculateBucket(flag.experiment.experimentId, context.userId || context.caller);
    let cumulative = 0;
    for (let i = 0; i < flag.experiment.variants.length; i++) {
      cumulative += flag.experiment.weights[i] || (100 / flag.experiment.variants.length);
      if (bucket < cumulative) {
        return {
          flagKey: key,
          value: flag.experiment.variants[i],
          variant: flag.experiment.variants[i],
          reason: 'experiment',
          isKillSwitchActive: false,
        };
      }
    }

    return {
      flagKey: key,
      value: flag.experiment.variants[0],
      variant: flag.experiment.variants[0],
      reason: 'experiment',
      isKillSwitchActive: false,
    };
  }

  private calculateBucket(seed: string, userKey: string): number {
    const combined = `${seed}:${userKey}`;
    let hash = 0;
    for (let i = 0; i < combined.length; i++) {
      const char = combined.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash |= 0;
    }
    return Math.abs(hash) % 100;
  }
}
