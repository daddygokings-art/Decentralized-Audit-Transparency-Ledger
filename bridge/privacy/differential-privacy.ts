/**
 * Differential Privacy (DP) Engine
 *
 * Implements Laplace and Gaussian noise injection mechanisms, bounded sensitivity clipping,
 * and (epsilon, delta) privacy loss budget accounting.
 */

export interface DpBudgetStatus {
  maxEpsilon: number;
  maxDelta: number;
  spentEpsilon: number;
  spentDelta: number;
  remainingEpsilon: number;
  remainingDelta: number;
  totalQueries: number;
}

export class DifferentialPrivacyEngine {
  private maxEpsilon: number;
  private maxDelta: number;
  private spentEpsilon: number = 0;
  private spentDelta: number = 0;
  private totalQueries: number = 0;

  constructor(maxEpsilon: number = 10.0, maxDelta: number = 1e-5) {
    this.maxEpsilon = maxEpsilon;
    this.maxDelta = maxDelta;
  }

  /**
   * Sample noise from a Laplace distribution Lap(scale = sensitivity / epsilon)
   */
  public sampleLaplaceNoise(sensitivity: number, epsilon: number): number {
    const scale = sensitivity / epsilon;
    const u = Math.random() - 0.5;
    return -scale * Math.sign(u) * Math.log(1 - 2 * Math.abs(u));
  }

  /**
   * Sample noise from a Gaussian distribution N(0, sigma^2)
   * where sigma = sqrt(2 * ln(1.25 / delta)) * sensitivity / epsilon
   */
  public sampleGaussianNoise(sensitivity: number, epsilon: number, delta: number = 1e-5): number {
    const sigma = (Math.sqrt(2 * Math.log(1.25 / delta)) * sensitivity) / epsilon;
    // Box-Muller transform
    const u1 = Math.max(1e-15, Math.random());
    const u2 = Math.random();
    const z0 = Math.sqrt(-2.0 * Math.log(u1)) * Math.cos(2.0 * Math.PI * u2);
    return z0 * sigma;
  }

  /**
   * Execute DP count query
   */
  public queryCount(rawCount: number, epsilon: number = 0.5, mechanism: "laplace" | "gaussian" = "laplace"): {
    noisyResult: number;
    epsilonSpent: number;
    mechanism: string;
  } {
    this.deductBudget(epsilon, mechanism === "gaussian" ? 1e-6 : 0);

    const sensitivity = 1.0;
    const noise = mechanism === "laplace"
      ? this.sampleLaplaceNoise(sensitivity, epsilon)
      : this.sampleGaussianNoise(sensitivity, epsilon, 1e-6);

    return {
      noisyResult: Math.max(0, Math.round(rawCount + noise)),
      epsilonSpent: epsilon,
      mechanism,
    };
  }

  /**
   * Execute DP sum query with bounded clipping
   */
  public querySum(
    rawSum: number,
    clipUpperBound: number,
    epsilon: number = 1.0,
    mechanism: "laplace" | "gaussian" = "laplace"
  ): {
    noisyResult: number;
    epsilonSpent: number;
    mechanism: string;
  } {
    this.deductBudget(epsilon, mechanism === "gaussian" ? 1e-6 : 0);

    const sensitivity = clipUpperBound;
    const noise = mechanism === "laplace"
      ? this.sampleLaplaceNoise(sensitivity, epsilon)
      : this.sampleGaussianNoise(sensitivity, epsilon, 1e-6);

    return {
      noisyResult: Math.max(0, rawSum + noise),
      epsilonSpent: epsilon,
      mechanism,
    };
  }

  private deductBudget(epsilon: number, delta: number) {
    if (this.spentEpsilon + epsilon > this.maxEpsilon) {
      throw new Error(
        `Privacy Budget Exhausted: Attempted to spend epsilon ${epsilon}, remaining is ${(this.maxEpsilon - this.spentEpsilon).toFixed(3)}`
      );
    }
    this.spentEpsilon += epsilon;
    this.spentDelta += delta;
    this.totalQueries++;
  }

  public getBudgetStatus(): DpBudgetStatus {
    return {
      maxEpsilon: this.maxEpsilon,
      maxDelta: this.maxDelta,
      spentEpsilon: Number(this.spentEpsilon.toFixed(4)),
      spentDelta: Number(this.spentDelta.toFixed(8)),
      remainingEpsilon: Number(Math.max(0, this.maxEpsilon - this.spentEpsilon).toFixed(4)),
      remainingDelta: Number(Math.max(0, this.maxDelta - this.spentDelta).toFixed(8)),
      totalQueries: this.totalQueries,
    };
  }
}
