import { randomUUID } from "crypto";
import { Router } from "express";
import { z } from "zod";

const rights = ["access", "rectification", "erasure", "portability", "restriction", "objection"] as const;
const mechanisms = ["adequacy", "scc", "bcr", "certification"] as const;

const RightsRequestSchema = z.object({
  subjectId: z.string().min(1).max(256),
  right: z.enum(rights),
  verificationToken: z.string().min(16).max(512),
  details: z.string().max(4096).optional(),
});

const TransferAssessmentSchema = z.object({
  destination: z.string().min(2).max(128),
  mechanism: z.enum(mechanisms),
  dataCategories: z.array(z.string().min(1).max(128)).min(1).max(50),
  supplementaryMeasures: z.array(z.string().min(1).max(256)).max(20).default([]),
  risk: z.enum(["low", "medium", "high"]),
  reviewedBy: z.string().min(1).max(256),
});

export type RightsRequest = z.infer<typeof RightsRequestSchema> & {
  id: string;
  status: "received" | "in_progress" | "fulfilled" | "rejected";
  createdAt: string;
  updatedAt: string;
  auditTrail: Array<{ action: string; at: string; actor: string }>;
};

export type TransferAssessment = z.infer<typeof TransferAssessmentSchema> & {
  id: string;
  status: "approved" | "review_required";
  createdAt: string;
};

const rightsRequests = new Map<string, RightsRequest>();
const transferAssessments = new Map<string, TransferAssessment>();

function now(): string {
  return new Date().toISOString();
}

export function createComplianceRouter(): Router {
  const router = Router();

  router.post("/privacy/requests", (req, res) => {
    const parsed = RightsRequestSchema.safeParse(req.body);
    if (!parsed.success) return res.status(400).json({ error: parsed.error.flatten() });
    const timestamp = now();
    const request: RightsRequest = {
      ...parsed.data,
      id: randomUUID(),
      status: "received",
      createdAt: timestamp,
      updatedAt: timestamp,
      auditTrail: [{ action: "request_received", at: timestamp, actor: "api" }],
    };
    rightsRequests.set(request.id, request);
    return res.status(201).json({ data: request });
  });

  router.get("/privacy/requests/:id", (req, res) => {
    const request = rightsRequests.get(req.params.id);
    return request ? res.json({ data: request }) : res.status(404).json({ error: "request not found" });
  });

  router.patch("/privacy/requests/:id", (req, res) => {
    const request = rightsRequests.get(req.params.id);
    if (!request) return res.status(404).json({ error: "request not found" });
    const status = z.enum(["in_progress", "fulfilled", "rejected"]).safeParse(req.body?.status);
    if (!status.success) return res.status(400).json({ error: "status must be in_progress, fulfilled, or rejected" });
    const timestamp = now();
    request.status = status.data;
    request.updatedAt = timestamp;
    request.auditTrail.push({ action: `request_${status.data}`, at: timestamp, actor: "operator" });
    return res.json({ data: request });
  });

  router.post("/compliance/transfers/assess", (req, res) => {
    const parsed = TransferAssessmentSchema.safeParse(req.body);
    if (!parsed.success) return res.status(400).json({ error: parsed.error.flatten() });
    const status = parsed.data.risk === "high" || parsed.data.supplementaryMeasures.length === 0
      ? "review_required"
      : "approved";
    const assessment: TransferAssessment = { ...parsed.data, id: randomUUID(), status, createdAt: now() };
    transferAssessments.set(assessment.id, assessment);
    return res.status(201).json({ data: assessment });
  });

  router.get("/compliance/transfers", (_req, res) => {
    res.json({ data: [...transferAssessments.values()] });
  });

  return router;
}

export function clearComplianceState(): void {
  rightsRequests.clear();
  transferAssessments.clear();
}