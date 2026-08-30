import { ExperimentConfig } from './types.js';

export interface ExperimentStats {
  experimentId: string;
  variantCounts: Record<string, number>;
  conversions: Record<string, number>;
}

export class ExperimentationEngine {
  private statsStore: Map<string, ExperimentStats> = new Map();

  public trackConversion(experimentId: string, variant: string): void {
    const current = this.statsStore.get(experimentId) || {
      experimentId,
      variantCounts: {},
      conversions: {},
    };

    current.conversions[variant] = (current.conversions[variant] || 0) + 1;
    this.statsStore.set(experimentId, current);
  }

  public trackExposure(experimentId: string, variant: string): void {
    const current = this.statsStore.get(experimentId) || {
      experimentId,
      variantCounts: {},
      conversions: {},
    };

    current.variantCounts[variant] = (current.variantCounts[variant] || 0) + 1;
    this.statsStore.set(experimentId, current);
  }

  public getWinningVariant(config: ExperimentConfig): string | undefined {
    const stats = this.statsStore.get(config.experimentId);
    if (!stats) return config.winnerVariant;

    let bestVariant: string | undefined;
    let highestRate = -1;

    for (const variant of config.variants) {
      const exposures = stats.variantCounts[variant] || 0;
      const conversions = stats.conversions[variant] || 0;
      if (exposures > 100) {
        const rate = conversions / exposures;
        if (rate > highestRate) {
          highestRate = rate;
          bestVariant = variant;
        }
      }
    }

    return bestVariant || config.variants[0];
  }
}
