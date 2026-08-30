import { FeatureFlag } from './types.js';

export class KillSwitchCoordinator {
  private activeKillSwitches: Map<string, { reason: string; triggeredAt: number; triggeredBy: string }> = new Map();

  public trigger(flag: FeatureFlag, reason: string, triggeredBy: string): FeatureFlag {
    this.activeKillSwitches.set(flag.key, {
      reason,
      triggeredAt: Date.now(),
      triggeredBy,
    });

    return {
      ...flag,
      status: 'killed',
      killSwitch: {
        isTriggered: true,
        triggeredBy,
        reason,
        triggeredAt: Date.now(),
        affectedEventTypes: flag.killSwitch?.affectedEventTypes || [],
      },
    };
  }

  public reset(flag: FeatureFlag): FeatureFlag {
    this.activeKillSwitches.delete(flag.key);

    return {
      ...flag,
      status: 'active',
      killSwitch: {
        ...flag.killSwitch,
        isTriggered: false,
        reason: '',
      },
    };
  }

  public isKilled(flagKey: string): boolean {
    return this.activeKillSwitches.has(flagKey);
  }
}
