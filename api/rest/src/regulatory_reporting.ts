/**
 * Automated Regulatory Reporting REST API
 *
 * Endpoints covering the full regulatory reporting pipeline for:
 * FINRA, SEC, CFTC, FCA, BaFin, MAS, MiCA
 *
 * Routes:
 *   POST   /regulatory-reports/generate            – Generate a new report
 *   GET    /regulatory-reports                     – List reports (with filters)
 *   GET    /regulatory-reports/:id                 – Get report by ID
 *   POST   /regulatory-reports/:id/validate        – Run validation on a report
 *   POST   /regulatory-reports/:id/submit          – Trigger submission to regulator
 *   POST   /regulatory-reports/:id/acknowledge     – Ingest acknowledgment webhook
 *   POST   /regulatory-reports/:id/cancel          – Cancel a report
 *   GET    /regulatory-reports/:id/submissions     – List submission attempts for a report
 *   GET    /regulatory-reports/:id/audit-trail     – Get immutable audit trail for a report
 *   GET    /regulatory-reports/pending             – List reports awaiting submission
 *   GET    /regulatory-reports/overdue             – List overdue reports
 *   GET    /regulatory-reports/authorities         – List supported authorities and forms
 */

import express, { Request, Response, NextFunction } from "express";
import crypto from "crypto";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export type RegulatoryAuthority =
  | "FINRA"
  | "SEC"
  | "CFTC"
  | "FCA"
  | "BaFin"
  | "MAS"
  | "MiCA";

export type ReportFormat =
  // FINRA
  | "FINRA_OATS"
  | "FINRA_CAT"
  | "FINRA_RULE4370"
  | "FINRA_SAR"
  // SEC
  | "SEC_FORM_ADV"
  | "SEC_FORM_PF"
  | "SEC_FORM_13F"
  | "SEC_FORM_NPORT"
  | "SEC_SAR"
  // CFTC
  | "CFTC_LARGE_TRADER"
  | "CFTC_SWAP_DATA"
  | "CFTC_PART20"
  | "CFTC_FORM40"
  // FCA
  | "FCA_MIFID_II"
  | "FCA_EMIR"
  | "FCA_STOR"
  | "FCA_COREP"
  // BaFin
  | "BAFIN_WPHG"
  | "BAFIN_MELDEPFLICHT"
  | "BAFIN_ANACREDIT"
  | "BAFIN_AML"
  // MAS
  | "MAS_SGX"
  | "MAS_TRR"
  | "MAS_FORM610"
  | "MAS_CMS"
  // MiCA
  | "MICA_CASP"
  | "MICA_WHITE_PAPER"
  | "MICA_RESERVE_ASSET"
  | "MICA_SIGNIFICANT";

export type ReportStatus =
  | "draft"
  | "validated"
  | "submitted"
  | "acknowledged"
  | "accepted"
  | "rejected"
  | "cancelled"
  | "overdue";

export type ReportAction =
  | "generated"
  | "validated"
  | "submitted"
  | "acknowledgment_received"
  | "accepted"
  | "rejected"
  | "resubmitted"
  | "cancelled"
  | "marked_overdue"
  | "config_updated";

export interface ValidationError {
  code: string;
  field?: string;
  message: string;
}

export interface ValidationResult {
  passed: boolean;
  errorCount: number;
  warningCount: number;
  errors: ValidationError[];
  warnings: ValidationError[];
  validatedAt: string; // ISO 8601
}

export interface RegulatoryReport {
  id: string;
  authority: RegulatoryAuthority;
  format: ReportFormat;
  entity: string;
  lei: string;
  periodStart: string; // ISO 8601
  periodEnd: string;   // ISO 8601
  deadline: string;    // ISO 8601
  content: Record<string, unknown>;
  schemaVersion: number;
  status: ReportStatus;
  createdAt: string;
  updatedAt: string;
  lastValidation?: ValidationResult;
  prevReportHash: string;
  reportHash: string;
  sourceEventIds: string[];
}

export interface SubmissionAttempt {
  id: string;
  reportId: string;
  attempt: number;
  submittedAt: string;
  endpoint: string;
  referenceNumber?: string;
  responseCode: number;
  responsePayload: unknown;
  status: ReportStatus;
  retryEligible: boolean;
  retryAfter?: string; // ISO 8601
}

export interface SubmissionAcknowledgment {
  id: string;
  submissionId: string;
  reportId: string;
  referenceNumber: string;
  accepted: boolean;
  rejectionReason?: string;
  errorCodes: string[];
  receivedAt: string;
  ackHash: string;
}

export interface AuditTrailEntry {
  sequence: number;
  reportId: string;
  action: ReportAction;
  actor: string;
  timestamp: string;
  prevEntryHash: string;
  entryHash: string;
  context: Record<string, unknown>;
  resultingStatus: ReportStatus;
}

// ─────────────────────────────────────────────────────────────────────────────
// In-memory store (replace with persistent DB in production)
// ─────────────────────────────────────────────────────────────────────────────

const reports    = new Map<string, RegulatoryReport>();
const submissions = new Map<string, SubmissionAttempt[]>(); // keyed by reportId
const auditTrails = new Map<string, AuditTrailEntry[]>();   // keyed by reportId
const ackStore   = new Map<string, SubmissionAcknowledgment>();

// ─────────────────────────────────────────────────────────────────────────────
// Utility helpers
// ─────────────────────────────────────────────────────────────────────────────

function newId(prefix: string, input: string): string {
  return prefix + "-" + crypto.createHash("sha256").update(input).digest("hex").slice(0, 16);
}

function now(): string {
  return new Date().toISOString();
}

function authorityForFormat(format: ReportFormat): RegulatoryAuthority {
  if (format.startsWith("FINRA_")) return "FINRA";
  if (format.startsWith("SEC_"))   return "SEC";
  if (format.startsWith("CFTC_"))  return "CFTC";
  if (format.startsWith("FCA_"))   return "FCA";
  if (format.startsWith("BAFIN_")) return "BaFin";
  if (format.startsWith("MAS_"))   return "MAS";
  if (format.startsWith("MICA_"))  return "MiCA";
  throw new Error(`Unknown format: ${format}`);
}

/** Supported authorities with their available forms. */
const AUTHORITY_FORMS: Record<RegulatoryAuthority, ReportFormat[]> = {
  FINRA:  ["FINRA_OATS", "FINRA_CAT", "FINRA_RULE4370", "FINRA_SAR"],
  SEC:    ["SEC_FORM_ADV", "SEC_FORM_PF", "SEC_FORM_13F", "SEC_FORM_NPORT", "SEC_SAR"],
  CFTC:   ["CFTC_LARGE_TRADER", "CFTC_SWAP_DATA", "CFTC_PART20", "CFTC_FORM40"],
  FCA:    ["FCA_MIFID_II", "FCA_EMIR", "FCA_STOR", "FCA_COREP"],
  BaFin:  ["BAFIN_WPHG", "BAFIN_MELDEPFLICHT", "BAFIN_ANACREDIT", "BAFIN_AML"],
  MAS:    ["MAS_SGX", "MAS_TRR", "MAS_FORM610", "MAS_CMS"],
  MiCA:   ["MICA_CASP", "MICA_WHITE_PAPER", "MICA_RESERVE_ASSET", "MICA_SIGNIFICANT"],
};

/** Required fields per format (subset — full schema lives in the Rust validator). */
const REQUIRED_FIELDS: Partial<Record<ReportFormat, string[]>> = {
  FINRA_OATS:         ["mpid", "orderCount", "routeCount"],
  FINRA_CAT:          ["mpid", "catReporterId", "eventCount"],
  FINRA_RULE4370:     ["bcpVersion", "emergencyContacts"],
  SEC_FORM_ADV:       ["adviserName", "aumUsd", "clientCount"],
  SEC_FORM_PF:        ["fundCount", "navUsd", "strategyType"],
  SEC_FORM_13F:       ["cusipCount", "totalValueUsd", "confidentialTreatment"],
  CFTC_LARGE_TRADER:  ["commodity", "positionLong", "positionShort", "specialAccount"],
  CFTC_SWAP_DATA:     ["swapType", "notionalUsd", "counterpartyLei", "uti"],
  FCA_MIFID_II:       ["isin", "quantity", "price", "venueMic", "executingEntityId"],
  FCA_EMIR:           ["tradeId", "assetClass", "notionalEur", "counterpartyLei"],
  BAFIN_WPHG:         ["isin", "votingRightsPct", "thresholdCrossed", "direction"],
  BAFIN_ANACREDIT:    ["loanCount", "totalExposureEur", "creditFacilityType"],
  MAS_TRR:            ["productType", "notionalSgd", "counterpartyLei", "uti"],
  MAS_FORM610:        ["balanceSheetTotalSgd", "loanBookSgd", "nplRatio"],
  MICA_CASP:          ["serviceType", "userCount", "transactionVolumeEur", "countriesServed"],
  MICA_RESERVE_ASSET: ["tokenSymbol", "tokensOutstanding", "reserveValueEur", "reserveComposition", "custodianLei"],
  MICA_WHITE_PAPER:   ["assetName", "assetClass", "offerType", "issuerCountry"],
};

// ─────────────────────────────────────────────────────────────────────────────
// Validation helper
// ─────────────────────────────────────────────────────────────────────────────

function validateReportContent(
  format: ReportFormat,
  content: Record<string, unknown>,
  lei: string,
  periodStart: string,
  periodEnd: string,
  deadline: string,
): ValidationResult {
  const errors: ValidationError[] = [];
  const warnings: ValidationError[] = [];

  // LEI check
  if (!/^[A-Z0-9]{20}$/.test(lei)) {
    errors.push({ code: "INVALID_LEI", field: "lei", message: "LEI must be exactly 20 uppercase alphanumeric characters (ISO 17442)" });
  }

  // Period check
  const ps = new Date(periodStart).getTime();
  const pe = new Date(periodEnd).getTime();
  const dl = new Date(deadline).getTime();
  if (isNaN(ps) || isNaN(pe)) {
    errors.push({ code: "INVALID_PERIOD", message: "periodStart and periodEnd must be valid ISO 8601 dates" });
  } else if (ps >= pe) {
    errors.push({ code: "INVALID_PERIOD", message: "periodStart must be before periodEnd" });
  }
  if (!isNaN(dl) && !isNaN(pe) && dl < pe) {
    errors.push({ code: "INVALID_DEADLINE", field: "deadline", message: "deadline must not be earlier than periodEnd" });
  }
  if (!isNaN(dl) && !isNaN(pe) && pe > dl) {
    warnings.push({ code: "DEADLINE_TIGHT", message: "periodEnd is after the submission deadline — review urgently" });
  }

  // Required fields
  const required = REQUIRED_FIELDS[format] ?? [];
  for (const field of required) {
    if (content[field] === undefined || content[field] === null || content[field] === "") {
      errors.push({ code: "MISSING_REQUIRED_FIELD", field, message: `Missing required field: ${field}` });
    }
  }

  return {
    passed: errors.length === 0,
    errorCount: errors.length,
    warningCount: warnings.length,
    errors,
    warnings,
    validatedAt: now(),
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Audit trail helper
// ─────────────────────────────────────────────────────────────────────────────

function appendAuditEntry(
  reportId: string,
  action: ReportAction,
  actor: string,
  resultingStatus: ReportStatus,
  context: Record<string, unknown>,
): AuditTrailEntry {
  const trail = auditTrails.get(reportId) ?? [];
  const sequence = trail.length;
  const prevHash = sequence === 0 ? "0".repeat(64) : trail[sequence - 1].entryHash;

  const payload = JSON.stringify({ reportId, action, sequence, actor, timestamp: now(), context, prevHash });
  const entryHash = crypto.createHash("sha256").update(payload).digest("hex");

  const entry: AuditTrailEntry = {
    sequence,
    reportId,
    action,
    actor,
    timestamp: now(),
    prevEntryHash: prevHash,
    entryHash,
    context,
    resultingStatus,
  };

  trail.push(entry);
  auditTrails.set(reportId, trail);
  return entry;
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

export const regulatoryReportingRouter = express.Router();

// ── GET /authorities — list supported authorities and forms ───────────────────

regulatoryReportingRouter.get("/authorities", (_req: Request, res: Response) => {
  const result = Object.entries(AUTHORITY_FORMS).map(([authority, forms]) => ({
    authority,
    jurisdiction: jurisdictionFor(authority as RegulatoryAuthority),
    forms,
  }));
  res.json({ authorities: result });
});

function jurisdictionFor(a: RegulatoryAuthority): string {
  const map: Record<RegulatoryAuthority, string> = {
    FINRA: "US", SEC: "US", CFTC: "US",
    FCA: "GB", BaFin: "DE", MAS: "SG", MiCA: "EU",
  };
  return map[a];
}

// ── POST /generate — generate a new report ────────────────────────────────────

regulatoryReportingRouter.post("/generate", (req: Request, res: Response) => {
  const {
    format,
    entity,
    lei,
    periodStart,
    periodEnd,
    deadline,
    content = {},
    sourceEventIds = [],
    prevReportHash = "0".repeat(64),
    actor = "system",
  } = req.body as {
    format: ReportFormat;
    entity: string;
    lei: string;
    periodStart: string;
    periodEnd: string;
    deadline: string;
    content: Record<string, unknown>;
    sourceEventIds?: string[];
    prevReportHash?: string;
    actor?: string;
  };

  if (!format || !entity || !lei || !periodStart || !periodEnd || !deadline) {
    return res.status(400).json({ error: "Missing required fields: format, entity, lei, periodStart, periodEnd, deadline" });
  }

  let authority: RegulatoryAuthority;
  try {
    authority = authorityForFormat(format);
  } catch {
    return res.status(400).json({ error: `Unsupported format: ${format}` });
  }

  const ts = now();
  const hashInput = `${authority}|${format}|${entity}|${periodStart}|${periodEnd}|${ts}`;
  const id = newId("rpt", hashInput);
  const reportHash = crypto.createHash("sha256")
    .update(prevReportHash + JSON.stringify(content))
    .digest("hex");

  const report: RegulatoryReport = {
    id,
    authority,
    format,
    entity,
    lei,
    periodStart,
    periodEnd,
    deadline,
    content,
    schemaVersion: 1,
    status: "draft",
    createdAt: ts,
    updatedAt: ts,
    prevReportHash,
    reportHash,
    sourceEventIds,
  };

  reports.set(id, report);
  appendAuditEntry(id, "generated", actor, "draft", { format, authority });

  return res.status(201).json({ report });
});

// ── GET / — list reports ──────────────────────────────────────────────────────

regulatoryReportingRouter.get("/", (req: Request, res: Response) => {
  const { authority, status, entity, limit = "50", offset = "0" } = req.query as Record<string, string>;

  let result = Array.from(reports.values());

  if (authority) result = result.filter((r) => r.authority === authority);
  if (status)    result = result.filter((r) => r.status === status);
  if (entity)    result = result.filter((r) => r.entity === entity);

  const total = result.length;
  const page = result.slice(Number(offset), Number(offset) + Number(limit));

  res.json({ reports: page, total, offset: Number(offset), limit: Number(limit) });
});

// ── GET /pending — reports awaiting submission ────────────────────────────────

regulatoryReportingRouter.get("/pending", (_req: Request, res: Response) => {
  const pending = Array.from(reports.values()).filter(
    (r) => r.status === "validated" || r.status === "rejected",
  );
  res.json({ reports: pending, total: pending.length });
});

// ── GET /overdue — reports past deadline ─────────────────────────────────────

regulatoryReportingRouter.get("/overdue", (_req: Request, res: Response) => {
  const ts = Date.now();
  const overdue = Array.from(reports.values()).filter(
    (r) =>
      new Date(r.deadline).getTime() < ts &&
      !["accepted", "cancelled", "overdue"].includes(r.status),
  );

  // Mark them as overdue in the store
  for (const r of overdue) {
    if (r.status !== "overdue") {
      r.status = "overdue";
      r.updatedAt = now();
      appendAuditEntry(r.id, "marked_overdue", "system", "overdue", { deadline: r.deadline });
    }
  }

  res.json({ reports: overdue, total: overdue.length });
});

// ── GET /:id — get single report ──────────────────────────────────────────────

regulatoryReportingRouter.get("/:id", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });
  res.json({ report });
});

// ── POST /:id/validate — run validation ───────────────────────────────────────

regulatoryReportingRouter.post("/:id/validate", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  const { actor = "system" } = req.body as { actor?: string };

  const result = validateReportContent(
    report.format,
    report.content,
    report.lei,
    report.periodStart,
    report.periodEnd,
    report.deadline,
  );

  report.lastValidation = result;
  report.updatedAt = now();

  if (result.passed) {
    report.status = "validated";
    appendAuditEntry(report.id, "validated", actor, "validated", {
      passed: true,
      errorCount: 0,
    });
  } else {
    appendAuditEntry(report.id, "validated", actor, report.status, {
      passed: false,
      errorCount: result.errorCount,
      errors: result.errors.map((e) => e.code),
    });
  }

  res.json({ validation: result, report });
});

// ── POST /:id/submit — trigger submission ─────────────────────────────────────

regulatoryReportingRouter.post("/:id/submit", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  if (!["validated", "rejected"].includes(report.status)) {
    return res.status(409).json({
      error: `Cannot submit report in status '${report.status}'. Must be 'validated' or 'rejected'.`,
    });
  }

  if (new Date(report.deadline).getTime() < Date.now()) {
    return res.status(422).json({ error: "Submission deadline has passed" });
  }

  const { endpoint, actor = "system" } = req.body as { endpoint?: string; actor?: string };

  const existing = submissions.get(report.id) ?? [];
  const attempt = existing.length + 1;

  const subId = newId("sub", `${report.id}|${attempt}`);
  const submission: SubmissionAttempt = {
    id: subId,
    reportId: report.id,
    attempt,
    submittedAt: now(),
    endpoint: endpoint ?? `https://regulatory-gateway.example/${report.authority.toLowerCase()}/submit`,
    responseCode: 0,
    responsePayload: null,
    status: "submitted",
    retryEligible: attempt < 3,
  };

  existing.push(submission);
  submissions.set(report.id, existing);

  report.status = "submitted";
  report.updatedAt = now();

  appendAuditEntry(report.id, attempt === 1 ? "submitted" : "resubmitted", actor, "submitted", {
    submissionId: subId,
    attempt,
    endpoint: submission.endpoint,
  });

  res.status(202).json({ submission, report });
});

// ── POST /:id/acknowledge — ingest acknowledgment webhook ─────────────────────

regulatoryReportingRouter.post("/:id/acknowledge", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  if (!["submitted", "acknowledged"].includes(report.status)) {
    return res.status(409).json({
      error: `Cannot acknowledge report in status '${report.status}'.`,
    });
  }

  const {
    referenceNumber,
    accepted,
    rejectionReason = "",
    errorCodes = [],
    actor = "regulator-webhook",
  } = req.body as {
    referenceNumber: string;
    accepted: boolean;
    rejectionReason?: string;
    errorCodes?: string[];
    actor?: string;
  };

  if (!referenceNumber) {
    return res.status(400).json({ error: "referenceNumber is required in acknowledgment" });
  }

  const reportSubmissions = submissions.get(report.id) ?? [];
  const latestSub = reportSubmissions[reportSubmissions.length - 1];

  const ackId = newId("ack", `${report.id}|${referenceNumber}|${now()}`);
  const ackHash = crypto
    .createHash("sha256")
    .update(JSON.stringify({ referenceNumber, accepted, rejectionReason, errorCodes }))
    .digest("hex");

  const ack: SubmissionAcknowledgment = {
    id: ackId,
    submissionId: latestSub?.id ?? "unknown",
    reportId: report.id,
    referenceNumber,
    accepted,
    rejectionReason: rejectionReason || undefined,
    errorCodes,
    receivedAt: now(),
    ackHash,
  };

  ackStore.set(ackId, ack);

  // Update submission
  if (latestSub) {
    latestSub.referenceNumber = referenceNumber;
    latestSub.status = accepted ? "accepted" : "rejected";
  }

  // Transition report
  report.status = "acknowledged";
  report.updatedAt = now();

  appendAuditEntry(report.id, "acknowledgment_received", actor, "acknowledged", {
    ackId,
    referenceNumber,
    accepted,
  });

  const finalStatus: ReportStatus = accepted ? "accepted" : "rejected";
  report.status = finalStatus;
  report.updatedAt = now();

  appendAuditEntry(
    report.id,
    accepted ? "accepted" : "rejected",
    actor,
    finalStatus,
    accepted
      ? { referenceNumber }
      : { referenceNumber, rejectionReason, errorCodes },
  );

  res.json({ acknowledgment: ack, report });
});

// ── POST /:id/cancel — cancel a report ───────────────────────────────────────

regulatoryReportingRouter.post("/:id/cancel", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  if (["accepted", "cancelled", "overdue"].includes(report.status)) {
    return res.status(409).json({
      error: `Cannot cancel report in terminal status '${report.status}'.`,
    });
  }

  const { reason = "Operator request", actor = "operator" } = req.body as {
    reason?: string;
    actor?: string;
  };

  report.status = "cancelled";
  report.updatedAt = now();

  appendAuditEntry(report.id, "cancelled", actor, "cancelled", { reason });

  res.json({ report });
});

// ── GET /:id/submissions — list submission attempts ───────────────────────────

regulatoryReportingRouter.get("/:id/submissions", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  const subs = submissions.get(req.params.id) ?? [];
  res.json({ submissions: subs, total: subs.length });
});

// ── GET /:id/audit-trail — get immutable audit trail ─────────────────────────

regulatoryReportingRouter.get("/:id/audit-trail", (req: Request, res: Response) => {
  const report = reports.get(req.params.id);
  if (!report) return res.status(404).json({ error: "Report not found" });

  const trail = auditTrails.get(req.params.id) ?? [];

  // Verify chain integrity before returning
  let chainValid = true;
  for (let i = 1; i < trail.length; i++) {
    if (trail[i].prevEntryHash !== trail[i - 1].entryHash) {
      chainValid = false;
      break;
    }
  }

  res.json({
    reportId: req.params.id,
    trail,
    total: trail.length,
    chainIntegrity: chainValid ? "valid" : "compromised",
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Error handler
// ─────────────────────────────────────────────────────────────────────────────

export function regulatoryReportingErrorHandler(
  err: Error,
  _req: Request,
  res: Response,
  _next: NextFunction,
): void {
  console.error("[regulatory-reporting]", err.message);
  res.status(500).json({ error: "Internal server error", detail: err.message });
}

// ─────────────────────────────────────────────────────────────────────────────
// Mount helper
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attach the regulatory reporting router to an Express app under a given prefix.
 *
 * Example:
 * ```ts
 * import { mountRegulatoryReporting } from "./regulatory_reporting";
 * mountRegulatoryReporting(app, "/v1/regulatory-reports");
 * ```
 */
export function mountRegulatoryReporting(
  app: express.Application,
  prefix = "/regulatory-reports",
): void {
  app.use(prefix, regulatoryReportingRouter);
  app.use(prefix, regulatoryReportingErrorHandler);
}
