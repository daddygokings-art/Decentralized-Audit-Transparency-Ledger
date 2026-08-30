/**
 * Tests for Federated Learning Anomaly Detection (#571)
 *
 * Covers model training, aggregation, differential privacy,
 * anomaly detection, and incentive mechanisms.
 */

import { expect } from "chai";
import {
  FederatedAnomalyDetector,
  DifferentialPrivacyNoise,
  createParticipant,
  createFederatedAnomalyDetector,
} from "../relayer/federated";

describe("DifferentialPrivacyNoise", function () {
  it("adds laplace noise to a value", function () {
    const dp = new DifferentialPrivacyNoise({ mechanism: "laplace", epsilon: 1.0 });
    const noisy = dp.addNoise(100);
    expect(typeof noisy).to.equal("number");
    expect(noisy).to.not.equal(100);
  });

  it("adds gaussian noise to a value", function () {
    const dp = new DifferentialPrivacyNoise({ mechanism: "gaussian", epsilon: 1.0 });
    const noisy = dp.addNoise(100);
    expect(typeof noisy).to.equal("number");
  });

  it("clips gradients to max norm", function () {
    const dp = new DifferentialPrivacyNoise();
    const clipped = dp.clipGradient([10, 20, 30], 5.0);
    const norm = Math.sqrt(clipped.reduce((sum, g) => sum + g * g, 0));
    expect(norm).to.be.at.most(5.0);
  });

  it("does not clip gradients below max norm", function () {
    const dp = new DifferentialPrivacyNoise();
    const clipped = dp.clipGradient([1, 2, 3], 10.0);
    expect(clipped).to.deep.equal([1, 2, 3]);
  });
});

describe("FederatedAnomalyDetector", function () {
  let detector: FederatedAnomalyDetector;

  beforeEach(function () {
    detector = createFederatedAnomalyDetector({ anomalyThreshold: 0.5 });
  });

  it("registers participants", function () {
    const participant = createParticipant("p1", "org-1");
    detector.registerParticipant(participant);
    expect(detector.getGlobalModel().version).to.equal(0);
  });

  it("starts and completes a training round", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    detector.registerParticipant(createParticipant("p2", "org-2"));

    const round = detector.startRound("round-1");
    expect(round.roundId).to.equal("round-1");
    expect(round.completed).to.be.false;

    detector.submitContribution("round-1", "p1", [0.1, -0.2], 0.05, 100, 0.8);
    detector.submitContribution("round-1", "p2", [0.15, -0.1], 0.03, 150, 0.6);

    const model = detector.aggregateRound("round-1");
    expect(model).to.not.be.null;
    expect(model!.version).to.equal(1);
    expect(model!.weights).to.have.length(2);
  });

  it("rejects zero sample contributions", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    detector.startRound("round-1");
    const accepted = detector.submitContribution("round-1", "p1", [0.1], 0.05, 0, 0.8);
    expect(accepted).to.be.false;
  });

  it("returns null when aggregating empty round", function () {
    detector.startRound("round-1");
    const model = detector.aggregateRound("round-1");
    expect(model).to.be.null;
  });

  it("detects normal events", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    const round = detector.startRound("round-1");
    detector.submitContribution("round-1", "p1", [-1, -1], -1, 100, 0.1);
    detector.aggregateRound("round-1");

    detector.setAnomalyThreshold(0.9);
    const result = detector.detectAnomaly([0.1, 0.1]);
    expect(result.isAnomalous).to.be.false;
    expect(result.modelVersion).to.equal(1);
  });

  it("detects anomalous events", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    const round = detector.startRound("round-1");
    detector.submitContribution("round-1", "p1", [1, 1], 1, 100, 0.1);
    detector.aggregateRound("round-1");

    const result = detector.detectAnomaly([10, 10]);
    expect(result.isAnomalous).to.be.true;
  });

  it("handles untrained model gracefully", function () {
    const result = detector.detectAnomaly([1, 2, 3]);
    expect(result.isAnomalous).to.be.false;
    expect(result.score).to.equal(0);
  });

  it("returns updated global model", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    const round = detector.startRound("round-1");
    detector.submitContribution("round-1", "p1", [0.5, -0.5], 0.1, 50, 0.5);
    detector.aggregateRound("round-1");

    const model = detector.getGlobalModel();
    expect(model.version).to.equal(1);
    expect(model.weights).to.have.length(2);
  });

  it("distributes incentives after aggregation", function () {
    detector.registerParticipant(createParticipant("p1", "org-1"));
    const round = detector.startRound("round-1");
    detector.submitContribution("round-1", "p1", [0.1], 0.01, 100, 0.5);
    detector.aggregateRound("round-1");

    const incentives = detector.getIncentiveRecords();
    expect(incentives).to.have.length(1);
    expect(incentives[0].participantId).to.equal("p1");
    expect(incentives[0].reward).to.be.greaterThan(0);
  });

  it("updates anomaly threshold", function () {
    detector.setAnomalyThreshold(0.9);
    expect(detector.getGlobalModel().version).to.equal(0);
  });
});
