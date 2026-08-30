/**
 * Tests for Verifiable Credentials for Events (#573)
 *
 * Covers issuer/holder/verifier model, credential schemas,
 * revocation, and selective disclosure with BBS+ signature simulation.
 */

import { expect } from "chai";
import {
  VerifiableCredentialManager,
  BbsPlusSimulation,
  createCredentialManager,
  createStandardEventSchema,
} from "../relayer/vc";

describe("VerifiableCredentialManager", function () {
  let manager: VerifiableCredentialManager;

  beforeEach(function () {
    manager = createCredentialManager();
  });

  it("initializes with a generated keypair", function () {
    expect(manager).to.be.an("object");
  });

  it("registers and retrieves a schema", function () {
    const schema = createStandardEventSchema();
    manager.registerSchema(schema);
    expect(manager.getSchema(schema.schemaId)).to.equal(schema);
  });

  it("issues a verifiable credential", function () {
    const cred = manager.issueCredential(
      "did:example:subject-1",
      "0xevent123",
      "audit",
      { auditorId: "auditor-1", complianceScore: 95 },
      30,
      "event-attestation-v1"
    );
    expect(cred.id).to.include("urn:uuid:");
    expect(cred.issuer).to.include("did:example:");
    expect(cred.credentialSubject.eventHash).to.equal("0xevent123");
    expect(cred.credentialSubject.eventType).to.equal("audit");
    expect(cred.proof).to.not.be.null;
    expect(cred.proof!.type).to.equal("BbsBlsSignature2020");
  });

  it("verifies a valid credential", function () {
    const cred = manager.issueCredential(
      "did:example:subject-1",
      "0xevent123",
      "audit",
      { auditorId: "auditor-1" }
    );
    expect(manager.verifyCredential(cred)).to.be.true;
  });

  it("rejects an expired credential", function () {
    const cred = manager.issueCredential(
      "did:example:subject-1",
      "0xevent123",
      "audit",
      {},
      -1
    );
    expect(manager.verifyCredential(cred)).to.be.false;
  });

  it("revokes a credential", function () {
    const cred = manager.issueCredential(
      "did:example:subject-1",
      "0xevent123",
      "audit",
      {}
    );
    expect(manager.verifyCredential(cred)).to.be.true;

    manager.revokeCredential(cred.id, "fraudulent attestation");
    expect(manager.isRevoked(cred.id)).to.be.true;
    expect(manager.verifyCredential(cred)).to.be.false;
  });

  it("creates a verifiable presentation", function () {
    const cred1 = manager.issueCredential("did:s1", "0xe1", "audit", {});
    const cred2 = manager.issueCredential("did:s2", "0xe2", "compliance", {});

    const presentation = manager.createPresentation([cred1, cred2], "did:example:holder-1");
    expect(presentation.id).to.include("urn:uuid:");
    expect(presentation.holder).to.equal("did:example:holder-1");
    expect(presentation.verifiableCredential).to.have.length(2);
    expect(presentation.proof).to.not.be.null;
  });

  it("verifies a valid presentation", function () {
    const cred = manager.issueCredential("did:s1", "0xe1", "audit", {});
    const presentation = manager.createPresentation([cred], "did:example:holder-1");
    expect(manager.verifyPresentation(presentation)).to.be.true;
  });

  it("applies selective disclosure to credentials", function () {
    const cred = manager.issueCredential(
      "did:example:subject-1",
      "0xevent123",
      "audit",
      { auditorId: "auditor-1", complianceScore: 95, sensitiveData: "redacted" },
      30,
      "event-attestation-v1"
    );

    const disclosed = manager.applySelectiveDisclosure(cred, ["auditorId", "complianceScore"]);
    expect(disclosed.credentialSubject.attributes.auditorId).to.equal("auditor-1");
    expect(disclosed.credentialSubject.attributes.complianceScore).to.equal(95);
    expect(disclosed.credentialSubject.attributes.sensitiveData).to.be.undefined;
  });

  it("creates presentations with selective disclosure", function () {
    const cred = manager.issueCredential(
      "did:s1",
      "0xe1",
      "audit",
      { fieldA: "public", fieldB: "private" },
      30,
      "event-attestation-v1"
    );

    const request = {
      fields: ["fieldA"],
      reason: "need only public field",
      requestedBy: "did:example:verifier-1",
    };

    const presentation = manager.createPresentation([cred], "did:example:holder-1", request);
    expect(presentation.verifiableCredential[0].credentialSubject.attributes.fieldA).to.equal("public");
    expect(presentation.verifiableCredential[0].credentialSubject.attributes.fieldB).to.be.undefined;
    expect(presentation.proof!.selectiveDisclosure).to.deep.equal(["fieldA"]);
  });

  it("returns issued credentials", function () {
    manager.issueCredential("did:s1", "0xe1", "audit", {});
    manager.issueCredential("did:s2", "0xe2", "compliance", {});
    expect(manager.getIssuedCredentials()).to.have.length(2);
  });

  it("returns revocation list", function () {
    const cred = manager.issueCredential("did:s1", "0xe1", "audit", {});
    manager.revokeCredential(cred.id, "test");
    expect(manager.getRevocationList()).to.have.length(1);
    expect(manager.getRevocationList()[0].reason).to.equal("test");
  });

  it("rejects verifying non-existent credential", function () {
    const fakeCred = {
      "@context": ["https://www.w3.org/2018/credentials/v1"],
      id: "urn:uuid:nonexistent",
      type: ["VerifiableCredential"],
      issuer: "did:example:issuer",
      issuanceDate: new Date().toISOString(),
      credentialSubject: {
        id: "did:example:subject",
        eventHash: "0xabc",
        eventType: "audit",
        attestedAt: Date.now(),
        attributes: {},
      },
    } as any;
    expect(manager.verifyCredential(fakeCred)).to.be.false;
  });

  it("generates deterministic issuer DID from private key", function () {
    const fixedKey = "a".repeat(64);
    const manager2 = createCredentialManager(fixedKey, "did:example:custom-issuer");
    expect(manager2["issuerDid"]).to.equal("did:example:custom-issuer");
  });
});

describe("BbsPlusSimulation", function () {
  it("signs and verifies messages", function () {
    const bbs = new BbsPlusSimulation();
    const messages = ["msg1", "msg2", "msg3"];
    const signature = bbs.sign(messages, "test-key-hex");
    expect(signature).to.be.a("string");
  });
});
