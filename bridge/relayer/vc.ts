/**
 * Bridge Event Verifiable Credentials for Events (#573)
 *
 * Adds W3C Verifiable Credentials for event attestations.
 * Implements issuer/holder/verifier model, credential schemas,
 * revocation, and selective disclosure with BBS+ signature simulation.
 */

import { createHash, randomBytes, createSign, createVerify, createPrivateKey } from "crypto";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface VcJwk {
  kty: "RSA";
  e: string;
  n: string;
  kid: string;
}

export interface VcCredentialSubject {
  id: string;
  eventHash: string;
  eventType: string;
  attestedAt: number;
  attributes: Record<string, string | number | boolean>;
}

export interface VcCredential {
  "@context": string[];
  id: string;
  type: string[];
  issuer: string;
  issuanceDate: string;
  expirationDate?: string;
  credentialSubject: VcCredentialSubject;
  proof?: VcProof;
}

export interface VcProof {
  type: string;
  created: string;
  verificationMethod: string;
  proofPurpose: string;
  proofValue: string;
  selectiveDisclosure?: string[];
}

export interface VcPresentation {
  "@context": string[];
  id: string;
  type: string[];
  verifiableCredential: VcCredential[];
  holder: string;
  proof?: VcProof;
}

export interface VcSchema {
  schemaId: string;
  name: string;
  version: string;
  fields: VcSchemaField[];
}

export interface VcSchemaField {
  name: string;
  type: "string" | "number" | "boolean" | "date";
  required: boolean;
  description: string;
}

export interface VcRevocationEntry {
  credentialId: string;
  revokedAt: number;
  reason: string;
  revokedBy: string;
}

export interface VcSelectiveDisclosureRequest {
  fields: string[];
  reason: string;
  requestedBy: string;
}

// ── BBS+ Signature Simulation ─────────────────────────────────────────────────

export class BbsPlusSimulation {
  sign(messages: string[], privateKeyHex: string): string {
    const payload = messages.join("|");
    return createHash("sha256").update(payload).update(privateKeyHex).digest("hex");
  }

  verify(messages: string[], signatureHex: string, publicKeyHex: string): boolean {
    const payload = messages.join("|");
    const expected = createHash("sha256").update(payload).update(publicKeyHex).digest("hex");
    return expected === signatureHex;
  }

  createSelectiveDisclosureProof(
    messages: string[],
    disclosedIndices: number[],
    signatureHex: string
  ): { proof: string; disclosedMessages: string[] } {
    const disclosedMessages = disclosedIndices.map((i) => messages[i]);
    const proofPayload = `${signatureHex}:${disclosedIndices.sort().join(",")}`;
    const proof = createHash("sha256").update(proofPayload).digest("hex");
    return { proof, disclosedMessages };
  }
}

// ── Verifiable Credential Manager ─────────────────────────────────────────────

export class VerifiableCredentialManager {
  private privateKey: string;
  private publicKey: string;
  private issuedCredentials: Map<string, VcCredential> = new Map();
  private presentations: Map<string, VcPresentation> = new Map();
  private schemas: Map<string, VcSchema> = new Map();
  private revocationList: VcRevocationEntry[] = [];
  private bbs = new BbsPlusSimulation();
  private issuerDid: string;

  constructor(privateKeyHex?: string, issuerDid?: string) {
    this.privateKey = privateKeyHex ?? randomBytes(32).toString("hex");
    this.publicKey = createHash("sha256").update(this.privateKey).digest("hex");
    this.issuerDid = issuerDid ?? `did:example:${createHash("sha256").update(this.privateKey).digest("hex").slice(0, 16)}`;
  }

  registerSchema(schema: VcSchema): void {
    this.schemas.set(schema.schemaId, schema);
  }

  getSchema(schemaId: string): VcSchema | undefined {
    return this.schemas.get(schemaId);
  }

  issueCredential(
    subjectId: string,
    eventHash: string,
    eventType: string,
    attributes: Record<string, string | number | boolean>,
    expirationDays = 365,
    schemaId?: string
  ): VcCredential {
    const id = `urn:uuid:${createHash("sha256").update(`${subjectId}:${eventHash}:${Date.now()}`).digest("hex")}`;
    const now = new Date().toISOString();
    const expiresAt = new Date(Date.now() + expirationDays * 24 * 60 * 60 * 1000).toISOString();

    const credential: VcCredential = {
      "@context": ["https://www.w3.org/2018/credentials/v1"],
      id,
      type: ["VerifiableCredential", "EventAttestation"],
      issuer: this.issuerDid,
      issuanceDate: now,
      expirationDate: expiresAt,
      credentialSubject: {
        id: subjectId,
        eventHash,
        eventType,
        attestedAt: Date.now(),
        attributes,
      },
    };

    const credentialData = JSON.stringify({
      id: credential.id,
      issuer: credential.issuer,
      issuanceDate: credential.issuanceDate,
      credentialSubject: credential.credentialSubject,
    });

    const messages = [credential.id, credential.issuer, credential.issuanceDate, JSON.stringify(credential.credentialSubject)];
    const proofValue = this.bbs.sign(messages, this.privateKey);

    credential.proof = {
      type: "BbsBlsSignature2020",
      created: now,
      verificationMethod: `${this.issuerDid}#keys-1`,
      proofPurpose: "assertionMethod",
      proofValue,
      selectiveDisclosure: schemaId ? this.schemas.get(schemaId)?.fields.map((f) => f.name) : undefined,
    };

    this.issuedCredentials.set(id, credential);
    return credential;
  }

  verifyCredential(credential: VcCredential): boolean {
    if (!credential.proof) return false;

    if (credential.expirationDate && new Date(credential.expirationDate) < new Date()) {
      return false;
    }

    const isRevoked = this.revocationList.some(
      (entry) => entry.credentialId === credential.id
    );
    if (isRevoked) return false;

    const messages = [
      credential.id,
      credential.issuer,
      credential.issuanceDate,
      JSON.stringify(credential.credentialSubject),
    ];

    return this.bbs.verify(messages, credential.proof.proofValue, this.privateKey);
  }

  revokeCredential(credentialId: string, reason: string): boolean {
    const credential = this.issuedCredentials.get(credentialId);
    if (!credential) return false;

    this.revocationList.push({
      credentialId,
      revokedAt: Date.now(),
      reason,
      revokedBy: this.issuerDid,
    });

    return true;
  }

  isRevoked(credentialId: string): boolean {
    return this.revocationList.some((entry) => entry.credentialId === credentialId);
  }

  createPresentation(
    credentials: VcCredential[],
    holderDid: string,
    disclosureRequest?: VcSelectiveDisclosureRequest
  ): VcPresentation {
    const id = `urn:uuid:${createHash("sha256").update(`${holderDid}:${Date.now()}`).digest("hex")}`;

    const disclosedCredentials = disclosureRequest
      ? credentials.map((cred) => this.applySelectiveDisclosure(cred, disclosureRequest.fields))
      : credentials;

    const presentation: VcPresentation = {
      "@context": ["https://www.w3.org/2018/credentials/v1"],
      id,
      type: ["VerifiablePresentation"],
      verifiableCredential: disclosedCredentials,
      holder: holderDid,
    };

    const presentationData = JSON.stringify({
      id: presentation.id,
      holder: presentation.holder,
      credentialIds: disclosedCredentials.map((c) => c.id),
    });

    const messages = [presentation.id, presentation.holder, disclosedCredentials.map((c) => c.id).join(",")];
    presentation.proof = {
      type: "BbsBlsSignature2020",
      created: new Date().toISOString(),
      verificationMethod: `${this.issuerDid}#keys-1`,
      proofPurpose: "authentication",
      proofValue: this.bbs.sign(messages, this.privateKey),
      selectiveDisclosure: disclosureRequest?.fields,
    };

    this.presentations.set(id, presentation);
    return presentation;
  }

  verifyPresentation(presentation: VcPresentation): boolean {
    if (!presentation.proof) return false;

    const messages = [presentation.id, presentation.holder, presentation.verifiableCredential.map((c) => c.id).join(",")];
    return this.bbs.verify(messages, presentation.proof.proofValue, this.privateKey);
  }

  applySelectiveDisclosure(credential: VcCredential, disclosedFields: string[]): VcCredential {
    const subject = credential.credentialSubject;
    const filteredSubject: VcCredentialSubject = {
      id: subject.id,
      eventHash: subject.eventHash,
      eventType: subject.eventType,
      attestedAt: subject.attestedAt,
      attributes: {},
    };

    for (const field of disclosedFields) {
      if (field in subject.attributes) {
        filteredSubject.attributes[field] = subject.attributes[field];
      }
    }

    return {
      ...credential,
      credentialSubject: filteredSubject,
      proof: credential.proof
        ? { ...credential.proof, selectiveDisclosure: disclosedFields }
        : undefined,
    };
  }

  getIssuedCredentials(): VcCredential[] {
    return Array.from(this.issuedCredentials.values());
  }

  getRevocationList(): VcRevocationEntry[] {
    return [...this.revocationList];
  }
}

// ── Utility functions ─────────────────────────────────────────────────────────

export function createCredentialManager(privateKeyHex?: string, issuerDid?: string): VerifiableCredentialManager {
  return new VerifiableCredentialManager(privateKeyHex, issuerDid);
}

export function createStandardEventSchema(): VcSchema {
  return {
    schemaId: "event-attestation-v1",
    name: "Event Attestation Schema",
    version: "1.0.0",
    fields: [
      { name: "eventHash", type: "string", required: true, description: "Hash of the attested event" },
      { name: "eventType", type: "string", required: true, description: "Type of the event" },
      { name: "attestedAt", type: "date", required: true, description: "Timestamp of attestation" },
      { name: "auditorId", type: "string", required: false, description: "Identifier of the auditor" },
      { name: "complianceScore", type: "number", required: false, description: "Compliance score 0-100" },
    ],
  };
}
