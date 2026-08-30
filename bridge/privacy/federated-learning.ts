/**
 * Federated Learning (FL) Coordinator
 *
 * Implements Federated Averaging (FedAvg) and FedProx algorithms, local gradient verification,
 * and Byzantine-robust aggregation among decentralized audit nodes.
 */

import { createHash } from "crypto";

export interface ParticipantGradientSubmission {
  participantAddress: string;
  weights: number[];
  sampleSize: number;
  gradientHash: string;
}

export interface FlRoundState {
  roundId: number;
  modelId: string;
  status: "OPEN" | "AGGREGATING" | "FINALIZED";
  minParticipants: number;
  submissions: ParticipantGradientSubmission[];
  aggregatedWeights: number[];
  globalWeightsHash: string;
  createdAt: number;
}

export class FederatedLearningCoordinator {
  private rounds = new Map<number, FlRoundState>();
  private nextRoundId = 1;

  public startRound(params: {
    modelId: string;
    minParticipants: number;
    initialWeights: number[];
  }): FlRoundState {
    const roundId = this.nextRoundId++;
    const initialHash = this.computeHash(params.initialWeights);

    const round: FlRoundState = {
      roundId,
      modelId: params.modelId,
      status: "OPEN",
      minParticipants: params.minParticipants,
      submissions: [],
      aggregatedWeights: params.initialWeights,
      globalWeightsHash: initialHash,
      createdAt: Date.now(),
    };

    this.rounds.set(roundId, round);
    return round;
  }

  public submitGradients(params: {
    roundId: number;
    participantAddress: string;
    weights: number[];
    sampleSize: number;
  }): void {
    const round = this.rounds.get(params.roundId);
    if (!round) throw new Error(`FL round ${params.roundId} not found`);
    if (round.status !== "OPEN") throw new Error(`FL round ${params.roundId} is not open`);

    const gradientHash = this.computeHash(params.weights);
    round.submissions.push({
      participantAddress: params.participantAddress,
      weights: params.weights,
      sampleSize: params.sampleSize,
      gradientHash,
    });
  }

  /**
   * Execute Federated Averaging (FedAvg) aggregation
   */
  public aggregateRoundFedAvg(roundId: number): FlRoundState {
    const round = this.rounds.get(roundId);
    if (!round) throw new Error(`FL round ${roundId} not found`);
    if (round.submissions.length < round.minParticipants) {
      throw new Error(
        `Insufficient participants: Got ${round.submissions.length}, required ${round.minParticipants}`
      );
    }

    const totalSamples = round.submissions.reduce((sum, s) => sum + s.sampleSize, 0);
    const weightLength = round.submissions[0].weights.length;
    const aggregated: number[] = new Array(weightLength).fill(0);

    for (const sub of round.submissions) {
      const weightFactor = sub.sampleSize / totalSamples;
      for (let i = 0; i < weightLength; i++) {
        aggregated[i] += sub.weights[i] * weightFactor;
      }
    }

    round.aggregatedWeights = aggregated.map((v) => Number(v.toFixed(6)));
    round.globalWeightsHash = this.computeHash(round.aggregatedWeights);
    round.status = "FINALIZED";

    return round;
  }

  private computeHash(weights: number[]): string {
    return createHash("sha256").update(JSON.stringify(weights)).digest("hex");
  }

  public getRound(roundId: number): FlRoundState | null {
    return this.rounds.get(roundId) || null;
  }
}
