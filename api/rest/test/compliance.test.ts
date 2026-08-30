import { describe, it, expect, beforeEach } from "vitest";
import request from "supertest";
import type { Test } from "supertest";
import express from "express";
import { createComplianceRouter } from "../src/compliance";
import { clearComplianceState } from "../src/compliance";

const app = express();
app.use(express.json());
app.use("/v1", createComplianceRouter());

function client(req: Test): Test {
  return req.set("User-Agent", "audit-ledger-compliance-tests/1.0").set("Accept", "application/json");
}

describe("compliance automation", () => {
  beforeEach(() => clearComplianceState());

  it("verifies and tracks a data subject request", async () => {
    const created = await client(request(app).post("/v1/privacy/requests")).send({
      subjectId: "subject-42",
      right: "portability",
      verificationToken: "verified-token-123456",
    });
    expect(created.status).toBe(201);
    expect(created.body.data.status).toBe("received");
    expect(created.body.data.auditTrail[0].action).toBe("request_received");

    const fulfilled = await client(request(app).patch(`/v1/privacy/requests/${created.body.data.id}`)).send({ status: "fulfilled" });
    expect(fulfilled.status).toBe(200);
    expect(fulfilled.body.data.status).toBe("fulfilled");
    expect(fulfilled.body.data.auditTrail).toHaveLength(2);
  });

  it("requires a supplementary measure for high-risk transfers", async () => {
    const response = await client(request(app).post("/v1/compliance/transfers/assess")).send({
      destination: "US",
      mechanism: "scc",
      dataCategories: ["accounting"],
      risk: "high",
      reviewedBy: "privacy-team",
    });
    expect(response.status).toBe(201);
    expect(response.body.data.status).toBe("review_required");
  });

  it("approves a documented low-risk transfer", async () => {
    const response = await client(request(app).post("/v1/compliance/transfers/assess")).send({
      destination: "EEA",
      mechanism: "adequacy",
      dataCategories: ["audit-events"],
      supplementaryMeasures: ["encryption-at-rest"],
      risk: "low",
      reviewedBy: "privacy-team",
    });
    expect(response.status).toBe(201);
    expect(response.body.data.status).toBe("approved");
  });
});
