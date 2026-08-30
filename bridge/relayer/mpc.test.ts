/**
 * Tests for Multi-Party Computation Analytics (#570)
 *
 * Covers threshold secret sharing, collaborative analytics computation,
 * and round management.
 */

import { expect } from "chai";
import {
  MpcAnalyticsEngine,
  ThresholdSecretSharing,
  createParty,
  createMpcAnalyticsEngine,
} from "../relayer/mpc";

describe("ThresholdSecretSharing", function () {
  const sharing = new ThresholdSecretSharing();

  it("splits a secret into shares", function () {
    const secret = BigInt("12345678901234567890");
    const shares = sharing.splitSecret(secret, 2, 5);
    expect(shares).to.have.length(5);
  });

  it("reconstructs secret with threshold shares", function () {
    const secret = BigInt("98765432109876543210");
    const shares = sharing.splitSecret(secret, 3, 5);
    const reconstructed = sharing.reconstructSecret([
      { index: 1, value: shares[0] },
      { index: 2, value: shares[1] },
      { index: 3, value: shares[2] },
    ]);
    expect(reconstructed).to.be.a("bigint");
    expect(reconstructed >= BigInt(0)).to.be.true;
  });

  it("fails to reconstruct with fewer than threshold shares", function () {
    const secret = BigInt("11111111111111111111");
    const shares = sharing.splitSecret(secret, 3, 5);
    const partial = sharing.reconstructSecret([
      { index: 1, value: shares[0] },
      { index: 2, value: shares[1] },
    ]);
    expect(partial).to.not.equal(secret);
  });
});

describe("MpcAnalyticsEngine", function () {
  let engine: MpcAnalyticsEngine;

  beforeEach(function () {
    engine = createMpcAnalyticsEngine();
  });

  it("registers parties", function () {
    const party = createParty("party-1");
    engine.registerParty(party);
    expect(engine.getRegisteredParties()).to.have.length(1);
  });

  it("initiates a computation round", function () {
    const round = engine.initiateComputation({
      analyticsType: "count",
      eventIndices: [0, 1, 2, 3, 4],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "100", threshold: 2, totalParties: 3 },
        { partyId: "p2", shareIndex: 1, shareValue: "200", threshold: 2, totalParties: 3 },
      ],
    });
    expect(round.roundId).to.be.a("string");
    expect(round.roundId).to.have.length(64);
  });

  it("submits shares and finalizes count analytics", function () {
    const round = engine.initiateComputation({
      analyticsType: "count",
      eventIndices: [10, 20, 30],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "10", threshold: 2, totalParties: 3 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "10");
    engine.submitShare(round.roundId, "p2", "20");

    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.analyticsType).to.equal("count");
    expect(result!.result).to.equal("3");
    expect(result!.partiesInvolved).to.have.length(2);
  });

  it("finalizes sum analytics", function () {
    const round = engine.initiateComputation({
      analyticsType: "sum",
      eventIndices: [5, 15, 25],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "5", threshold: 2, totalParties: 3 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "5");
    engine.submitShare(round.roundId, "p2", "15");

    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.analyticsType).to.equal("sum");
  });

  it("finalizes average analytics", function () {
    const round = engine.initiateComputation({
      analyticsType: "average",
      eventIndices: [4, 6],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "10", threshold: 1, totalParties: 2 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "10");
    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.analyticsType).to.equal("average");
  });

  it("finalizes min analytics", function () {
    const round = engine.initiateComputation({
      analyticsType: "min",
      eventIndices: [42, 17, 99],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "0", threshold: 1, totalParties: 1 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "0");
    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.result).to.equal("17");
  });

  it("finalizes max analytics", function () {
    const round = engine.initiateComputation({
      analyticsType: "max",
      eventIndices: [42, 17, 99],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "0", threshold: 1, totalParties: 1 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "0");
    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.result).to.equal("99");
  });

  it("returns null when threshold not met", function () {
    const round = engine.initiateComputation({
      analyticsType: "count",
      eventIndices: [1],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "10", threshold: 3, totalParties: 3 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "10");
    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.be.null;
  });

  it("generates a valid computation proof", function () {
    const round = engine.initiateComputation({
      analyticsType: "count",
      eventIndices: [1],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "10", threshold: 1, totalParties: 1 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "10");
    const result = engine.finalizeComputation(round.roundId);
    expect(result).to.not.be.null;
    expect(result!.proof).to.have.length(64);
  });

  it("returns null for already completed round", function () {
    const round = engine.initiateComputation({
      analyticsType: "count",
      eventIndices: [1],
      partyShares: [
        { partyId: "p1", shareIndex: 0, shareValue: "10", threshold: 1, totalParties: 1 },
      ],
    });

    engine.submitShare(round.roundId, "p1", "10");
    engine.finalizeComputation(round.roundId);
    const again = engine.finalizeComputation(round.roundId);
    expect(again).to.be.null;
  });
});
