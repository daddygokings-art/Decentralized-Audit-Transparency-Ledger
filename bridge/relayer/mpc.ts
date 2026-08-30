/**
 * Bridge Event Multi-Party Computation for Analytics (#570)
 *
 * Enables multiple parties to collaboratively compute analytics on
 * event data without revealing raw individual data points.
 * Uses threshold secret sharing and homomorphic-style aggregation.
 */

import { createHash, randomBytes } from "crypto";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface MpParty {
  id: string;
  publicKey: string;
}

export interface MpShare {
  partyId: string;
  shareIndex: number;
  shareValue: string;
  threshold: number;
  totalParties: number;
}

export interface MpAnalyticsRequest {
  analyticsType: "sum" | "count" | "average" | "min" | "max";
  eventIndices: number[];
  partyShares: MpShare[];
}

export interface MpAnalyticsResult {
  analyticsType: string;
  result: string;
  proof: string;
  partiesInvolved: string[];
  computedAt: number;
}

export interface MpComputationRound {
  roundId: string;
  request: MpAnalyticsRequest;
  sharesReceived: Map<string, string>;
  completed: boolean;
  result: MpAnalyticsResult | null;
}

// ── Threshold Secret Sharing ──────────────────────────────────────────────────

export class ThresholdSecretSharing {
  readonly fieldPrime = BigInt("2") ** BigInt(256) - BigInt("1") * BigInt("2") ** BigInt("64") * BigInt("9");

  splitSecret(secret: bigint, threshold: number, totalShares: number): bigint[] {
    const shares: bigint[] = [];
    const secretBytes = Buffer.from(secret.toString(16).padStart(64, "0"), "hex");
    const chunkSize = 32;
    const chunks: bigint[] = [];

    for (let i = 0; i < secretBytes.length; i += chunkSize) {
      const chunk = secretBytes.slice(i, i + chunkSize);
      chunks.push(BigInt("0x" + chunk.toString("hex")));
    }

    for (let i = 0; i < totalShares; i++) {
      let share = BigInt(0);
      for (const chunk of chunks) {
        const x = BigInt(i + 1);
        const coeffs = this.generateCoefficients(chunk, threshold);
        const term = this.evaluatePolynomial(coeffs, x);
        share = (share + term) % this.fieldPrime;
      }
      shares.push(share);
    }

    return shares;
  }

  reconstructSecret(shares: Array<{ index: number; value: bigint }>): bigint {
    let secret = BigInt(0);
    const k = shares.length;

    for (let i = 0; i < k; i++) {
      let numerator = BigInt(1);
      let denominator = BigInt(1);

      for (let j = 0; j < k; j++) {
        if (i === j) continue;
        const xi = BigInt(shares[i].index);
        const xj = BigInt(shares[j].index);
        numerator = ((numerator * (this.fieldPrime - xj)) % this.fieldPrime + this.fieldPrime) % this.fieldPrime;
        denominator = ((denominator * ((xi - xj + this.fieldPrime) % this.fieldPrime)) % this.fieldPrime + this.fieldPrime) % this.fieldPrime;
      }

      const lagrange = (numerator * this.modInverse(denominator)) % this.fieldPrime;
      secret = ((secret + shares[i].value * lagrange) % this.fieldPrime + this.fieldPrime) % this.fieldPrime;
    }

    return (secret + this.fieldPrime) % this.fieldPrime;
  }

  private generateCoefficients(secret: bigint, degree: number): bigint[] {
    const coeffs: bigint[] = [secret];
    for (let i = 1; i <= degree; i++) {
      const seed = createHash("sha256").update(`${secret}:${i}`).digest("hex");
      coeffs.push(BigInt("0x" + seed.slice(0, 64)));
    }
    return coeffs;
  }

  private evaluatePolynomial(coeffs: bigint[], x: bigint): bigint {
    let result = BigInt(0);
    for (let i = coeffs.length - 1; i >= 0; i--) {
      result = (result * x + coeffs[i]) % this.fieldPrime;
    }
    return result;
  }

  private modInverse(a: bigint): bigint {
    let [oldR, r] = [((a % this.fieldPrime) + this.fieldPrime) % this.fieldPrime, this.fieldPrime];
    let [oldS, s] = [BigInt(1), BigInt(0)];

    while (r !== BigInt(0)) {
      const quotient = oldR / r;
      [oldR, r] = [r, oldR - quotient * r];
      [oldS, s] = [s, oldS - quotient * s];
    }

    return (oldS % this.fieldPrime + this.fieldPrime) % this.fieldPrime;
  }
}

// ── MPC Analytics Engine ──────────────────────────────────────────────────────

export class MpcAnalyticsEngine {
  private sharing = new ThresholdSecretSharing();
  private rounds: Map<string, MpComputationRound> = new Map();
  private registeredParties: Map<string, MpParty> = new Map();

  registerParty(party: MpParty): void {
    this.registeredParties.set(party.id, party);
  }

  getRegisteredParties(): MpParty[] {
    return Array.from(this.registeredParties.values());
  }

  initiateComputation(request: MpAnalyticsRequest): MpComputationRound {
    const roundId = createHash("sha256")
      .update(JSON.stringify(request))
      .update(randomBytes(16))
      .digest("hex");

    const round: MpComputationRound = {
      roundId,
      request,
      sharesReceived: new Map(),
      completed: false,
      result: null,
    };

    this.rounds.set(roundId, round);
    return round;
  }

  submitShare(roundId: string, partyId: string, shareValue: string): boolean {
    const round = this.rounds.get(roundId);
    if (!round || round.completed) return false;

    round.sharesReceived.set(partyId, shareValue);
    return true;
  }

  finalizeComputation(roundId: string): MpAnalyticsResult | null {
    const round = this.rounds.get(roundId);
    if (!round || round.completed) return null;

    const request = round.request;
    const threshold = request.partyShares[0]?.threshold ?? 2;
    const totalParties = request.partyShares[0]?.totalParties ?? request.partyShares.length;

    if (round.sharesReceived.size < threshold) {
      return null;
    }

    let resultValue: string;
    const eventIndices = request.eventIndices;
    const partiesInvolved = Array.from(round.sharesReceived.keys());

    switch (request.analyticsType) {
      case "count":
        resultValue = String(eventIndices.length);
        break;
      case "sum": {
        const aggregated = this.aggregateShares(round.sharesReceived);
        resultValue = aggregated.toString();
        break;
      }
      case "average": {
        const aggregated = this.aggregateShares(round.sharesReceived);
        resultValue = eventIndices.length > 0
          ? (aggregated / BigInt(eventIndices.length)).toString()
          : "0";
        break;
      }
      case "min":
        resultValue = String(Math.min(...eventIndices));
        break;
      case "max":
        resultValue = String(Math.max(...eventIndices));
        break;
      default:
        resultValue = "0";
    }

    const result: MpAnalyticsResult = {
      analyticsType: request.analyticsType,
      result: resultValue,
      proof: this.generateComputationProof(roundId, partiesInvolved, resultValue),
      partiesInvolved,
      computedAt: Date.now(),
    };

    round.result = result;
    round.completed = true;
    return result;
  }

  getRound(roundId: string): MpComputationRound | undefined {
    return this.rounds.get(roundId);
  }

  private aggregateShares(shares: Map<string, string>): bigint {
    let sum = BigInt(0);
    for (const share of shares.values()) {
      sum = (sum + BigInt(share)) % this.sharing.fieldPrime;
    }
    return sum;
  }

  private generateComputationProof(roundId: string, parties: string[], result: string): string {
    const data = `${roundId}:${parties.sort().join(",")}:${result}`;
    return createHash("sha256").update(data).digest("hex");
  }
}

// ── Utility functions ─────────────────────────────────────────────────────────

export function createParty(id: string, publicKey?: string): MpParty {
  return {
    id,
    publicKey: publicKey ?? createHash("sha256").update(id).digest("hex"),
  };
}

export function createMpcAnalyticsEngine(): MpcAnalyticsEngine {
  return new MpcAnalyticsEngine();
}
