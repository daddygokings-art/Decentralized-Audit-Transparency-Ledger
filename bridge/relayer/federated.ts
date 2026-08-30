/**
 * Bridge Event Federated Learning for Anomaly Detection (#571)
 *
 * Adds federated learning for anomaly detection across organizations
 * without sharing event data. Implements model aggregation, differential
 * privacy, and incentive mechanisms.
 */

import { createHash, randomBytes } from "crypto";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface FlModelWeights {
  bias: number;
  weights: number[];
  version: number;
}

export interface FlParticipant {
  id: string;
  organization: string;
  reputation: number;
  contributions: number;
  lastContribution: number;
}

export interface FlContribution {
  participantId: string;
  roundId: string;
  weightUpdate: number[];
  biasUpdate: number;
  sampleCount: number;
  loss: number;
  timestamp: number;
}

export interface FlRound {
  roundId: string;
  modelVersion: number;
  participants: string[];
  contributions: FlContribution[];
  aggregatedModel: FlModelWeights | null;
  globalModel: FlModelWeights;
  completed: boolean;
  startedAt: number;
  completedAt: number;
}

export interface FlAnomalyResult {
  eventIndex: number;
  score: number;
  isAnomalous: boolean;
  modelVersion: number;
  detectedAt: number;
}

export interface FlIncentiveRecord {
  participantId: string;
  roundId: string;
  reward: number;
  reason: string;
  timestamp: number;
}

export interface FlDifferentialPrivacyConfig {
  epsilon: number;
  delta: number;
  mechanism: "laplace" | "gaussian";
  sensitivity: number;
}

// ── Differential Privacy Noise Generator ─────────────────────────────────────

export class DifferentialPrivacyNoise {
  private config: FlDifferentialPrivacyConfig;

  constructor(config: Partial<FlDifferentialPrivacyConfig> = {}) {
    this.config = {
      epsilon: config.epsilon ?? 1.0,
      delta: config.delta ?? 1e-5,
      mechanism: config.mechanism ?? "laplace",
      sensitivity: config.sensitivity ?? 1.0,
    };
  }

  addNoise(value: number): number {
    if (this.config.mechanism === "laplace") {
      const scale = this.config.sensitivity / this.config.epsilon;
      return value + this.laplaceNoise(scale);
    }
    const scale = this.config.sensitivity * Math.sqrt(2 * Math.log(1.25 / this.config.delta)) / this.config.epsilon;
    return value + this.gaussianNoise(scale);
  }

  clipGradient(gradient: number[], maxNorm: number): number[] {
    const norm = Math.sqrt(gradient.reduce((sum, g) => sum + g * g, 0));
    if (norm > maxNorm) {
      const scale = maxNorm / norm;
      return gradient.map((g) => g * scale);
    }
    return gradient;
  }

  private laplaceNoise(scale: number): number {
    const u = Math.random() - 0.5;
    return -scale * Math.sign(u) * Math.log(1 - 2 * Math.abs(u));
  }

  private gaussianNoise(scale: number): number {
    let u = 0;
    let v = 0;
    while (u === 0) u = Math.random();
    while (v === 0) v = Math.random();
    return Math.sqrt(-2.0 * Math.log(u)) * Math.cos(2.0 * Math.PI * v) * scale;
  }
}

// ── Federated Learning Anomaly Detector ──────────────────────────────────────

export class FederatedAnomalyDetector {
  private participants: Map<string, FlParticipant> = new Map();
  private rounds: Map<string, FlRound> = new Map();
  private globalModel: FlModelWeights = { bias: 0, weights: [], version: 0 };
  private noiseGenerator = new DifferentialPrivacyNoise();
  private incentiveRecords: FlIncentiveRecord[] = [];
  private anomalyThreshold = 0.7;
  private maxGradientNorm = 5.0;

  registerParticipant(participant: FlParticipant): void {
    this.participants.set(participant.id, participant);
  }

  startRound(roundId: string): FlRound {
    const round: FlRound = {
      roundId,
      modelVersion: this.globalModel.version,
      participants: [],
      contributions: [],
      aggregatedModel: null,
      globalModel: { ...this.globalModel, weights: [...this.globalModel.weights] },
      completed: false,
      startedAt: Date.now(),
      completedAt: 0,
    };
    this.rounds.set(roundId, round);
    return round;
  }

  submitContribution(
    roundId: string,
    participantId: string,
    weightUpdate: number[],
    biasUpdate: number,
    sampleCount: number,
    loss: number
  ): boolean {
    const round = this.rounds.get(roundId);
    if (!round || round.completed) return false;

    if (sampleCount <= 0) return false;

    round.participants.push(participantId);
    round.contributions.push({
      participantId,
      roundId,
      weightUpdate: this.noiseGenerator.clipGradient(weightUpdate, this.maxGradientNorm),
      biasUpdate: this.noiseGenerator.clipGradient([biasUpdate], this.maxGradientNorm)[0],
      sampleCount,
      loss,
      timestamp: Date.now(),
    });

    return true;
  }

  aggregateRound(roundId: string): FlModelWeights | null {
    const round = this.rounds.get(roundId);
    if (!round || round.completed || round.contributions.length === 0) return null;

    const totalSamples = round.contributions.reduce((sum, c) => sum + c.sampleCount, 0);
    if (totalSamples === 0) return null;

    const weightDim = round.contributions[0].weightUpdate.length;
    const aggregatedWeights = new Array(weightDim).fill(0);
    let aggregatedBias = 0;

    for (const contribution of round.contributions) {
      const scale = contribution.sampleCount / totalSamples;
      for (let i = 0; i < weightDim; i++) {
        aggregatedWeights[i] += contribution.weightUpdate[i] * scale;
      }
      aggregatedBias += contribution.biasUpdate * scale;
    }

    const noisyWeights = aggregatedWeights.map((w) => this.noiseGenerator.addNoise(w));
    const noisyBias = this.noiseGenerator.addNoise(aggregatedBias);

    const aggregatedModel: FlModelWeights = {
      bias: noisyBias,
      weights: noisyWeights,
      version: this.globalModel.version + 1,
    };

    round.aggregatedModel = aggregatedModel;
    round.completed = true;
    round.completedAt = Date.now();

    this.globalModel = aggregatedModel;

    this.distributeIncentives(round);
    return aggregatedModel;
  }

  detectAnomaly(eventFeatures: number[]): FlAnomalyResult {
    if (this.globalModel.weights.length === 0 || eventFeatures.length !== this.globalModel.weights.length) {
      return {
        eventIndex: -1,
        score: 0,
        isAnomalous: false,
        modelVersion: this.globalModel.version,
        detectedAt: Date.now(),
      };
    }

    let score = this.globalModel.bias;
    for (let i = 0; i < eventFeatures.length; i++) {
      score += this.globalModel.weights[i] * eventFeatures[i];
    }

    score = 1 / (1 + Math.exp(-score));

    const result: FlAnomalyResult = {
      eventIndex: -1,
      score,
      isAnomalous: score > this.anomalyThreshold,
      modelVersion: this.globalModel.version,
      detectedAt: Date.now(),
    };

    return result;
  }

  setAnomalyThreshold(threshold: number): void {
    this.anomalyThreshold = Math.max(0, Math.min(1, threshold));
  }

  getGlobalModel(): FlModelWeights {
    return { ...this.globalModel, weights: [...this.globalModel.weights] };
  }

  getIncentiveRecords(): FlIncentiveRecord[] {
    return [...this.incentiveRecords];
  }

  private distributeIncentives(round: FlRound): void {
    const totalLoss = round.contributions.reduce((sum, c) => sum + c.loss, 0);
    if (totalLoss === 0) return;

    for (const contribution of round.contributions) {
      const participant = this.participants.get(contribution.participantId);
      if (!participant) continue;

      const lossShare = contribution.loss / totalLoss;
      const sampleBonus = contribution.sampleCount * 0.01;
      const reward = Math.max(0, 1 - lossShare) + sampleBonus;

      participant.reputation += reward;
      participant.contributions += contribution.sampleCount;
      participant.lastContribution = Date.now();

      this.incentiveRecords.push({
        participantId: contribution.participantId,
        roundId: round.roundId,
        reward,
        reason: `contributed ${contribution.sampleCount} samples with loss ${contribution.loss.toFixed(4)}`,
        timestamp: Date.now(),
      });
    }
  }
}

// ── Utility functions ─────────────────────────────────────────────────────────

export function createParticipant(id: string, organization: string): FlParticipant {
  return {
    id,
    organization,
    reputation: 0,
    contributions: 0,
    lastContribution: 0,
  };
}

export function createFederatedAnomalyDetector(
  config?: { anomalyThreshold?: number; epsilon?: number; delta?: number }
): FederatedAnomalyDetector {
  const detector = new FederatedAnomalyDetector();
  if (config?.anomalyThreshold !== undefined) {
    detector.setAnomalyThreshold(config.anomalyThreshold);
  }
  return detector;
}
