/**
 * Tests for Zero-Knowledge Compliance Proofs (#572)
 *
 * Covers proof generation, verification, commitment schemes,
 * and specific compliance proof types (audit, regulatory, cross-border).
 */

import { expect } from "chai";
import { createHash } from "crypto";
import {
  ZkComplianceProof,
  CommitmentScheme,
  createZkComplianceProof,
  createZkWitness,
  createZkPublicInput,
} from "../relayer/zkp";

describe("CommitmentScheme", function () {
  it("creates a commitment for a value", function () {
    const scheme = new CommitmentScheme();
    const { commitment, nonce } = scheme.commit("secret-data");
    expect(commitment).to.have.length(64);
    expect(nonce).to.have.length(64);
  });

  it("verifies a commitment with the correct nonce", function () {
    const scheme = new CommitmentScheme();
    const { commitment, nonce } = scheme.commit("secret-data");
    expect(scheme.verify("secret-data", nonce, commitment)).to.be.true;
  });

  it("rejects a commitment with the wrong nonce", function () {
    const scheme = new CommitmentScheme();
    const { commitment } = scheme.commit("secret-data");
    expect(scheme.verify("secret-data", "wrong-nonce", commitment)).to.be.false;
  });

  it("rejects a commitment with the wrong value", function () {
    const scheme = new CommitmentScheme();
    const { commitment, nonce } = scheme.commit("secret-data");
    expect(scheme.verify("different-data", nonce, commitment)).to.be.false;
  });
});

describe("ZkComplianceProof", function () {
  let prover: ZkComplianceProof;

  beforeEach(function () {
    prover = createZkComplianceProof();
  });

  it("generates an audit proof", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance");
    expect(proof.proofType).to.equal("audit");
    expect(proof.proofId).to.have.length(64);
    expect(proof.commitment).to.have.length(64);
    expect(proof.nullifier).to.have.length(64);
    expect(proof.expiresAt).to.be.greaterThan(Date.now());
  });

  it("generates a regulatory proof", function () {
    const proof = prover.generateRegulatoryProof("0xdef456", "GDPR-ART5", "EU");
    expect(proof.proofType).to.equal("regulatory");
    expect(proof.publicInputs.regulationId).to.equal("GDPR-ART5");
  });

  it("generates a cross-border proof", function () {
    const proof = prover.generateCrossBorderProof("0xghi789", "US", "EU");
    expect(proof.proofType).to.equal("cross-border");
    expect(proof.publicInputs.sourceJurisdiction).to.equal("US");
    expect(proof.publicInputs.targetJurisdiction).to.equal("EU");
  });

  it("verifies a valid audit proof", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance");
    const context = {
      jurisdiction: "global",
      regulationId: "audit-standard-v1",
      complianceDomain: "financial-audit",
      dataHash: createHash("sha256").update("0xabc123").digest("hex"),
    };
    const result = prover.verifyProof(proof, context);
    expect(result.valid).to.be.true;
    expect(result.proofId).to.equal(proof.proofId);
  });

  it("rejects a proof with mismatched context", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance");
    const wrongContext = {
      jurisdiction: "wrong-jurisdiction",
      regulationId: "wrong-regulation",
      complianceDomain: "wrong-domain",
      dataHash: "wrong-hash",
    };
    const result = prover.verifyProof(proof, wrongContext);
    expect(result.valid).to.be.false;
  });

  it("rejects an expired proof", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance", -1);
    const context = {
      jurisdiction: "global",
      regulationId: "audit-standard-v1",
      complianceDomain: "financial-audit",
      dataHash: createHash("sha256").update("0xabc123").digest("hex"),
    };
    const result = prover.verifyProof(proof, context);
    expect(result.valid).to.be.false;
  });

  it("generates proofs with private witness data hidden", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance");
    expect(proof.publicInputs.eventHash).to.equal("0xabc123");
    expect(proof.publicInputs.auditorId).to.equal("auditor-1");
    expect(proof.publicInputs.findings).to.be.undefined;
  });

  it("tracks verified proofs", function () {
    const proof = prover.generateAuditProof("0xabc123", "auditor-1", "financial-compliance");
    const context = {
      jurisdiction: "global",
      regulationId: "audit-standard-v1",
      complianceDomain: "financial-audit",
      dataHash: createHash("sha256").update("0xabc123").digest("hex"),
    };
    prover.verifyProof(proof, context);
    expect(prover.isProofValid(proof.proofId)).to.be.true;
    expect(prover.getVerifiedProofs()).to.have.length(1);
  });

  it("returns multiple proof types from same prover", function () {
    const audit = prover.generateAuditProof("0x1", "a1", "scope-1");
    const regulatory = prover.generateRegulatoryProof("0x2", "reg-1", "US");
    const crossBorder = prover.generateCrossBorderProof("0x3", "US", "EU");
    expect(audit.proofType).to.equal("audit");
    expect(regulatory.proofType).to.equal("regulatory");
    expect(crossBorder.proofType).to.equal("cross-border");
  });
});

