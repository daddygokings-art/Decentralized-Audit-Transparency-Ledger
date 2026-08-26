import type { Request, Response } from "express";

export interface CspViolationReport {
  receivedAt: number;
  documentUri?: string;
  violatedDirective?: string;
  blockedUri?: string;
  sourceFile?: string;
  lineNumber?: number;
  disposition?: string;
  raw: unknown;
}

const MAX_REPORTS = 1000;
const MAX_BODY_BYTES = 32 * 1024;

export class ViolationReportStore {
  private reports: CspViolationReport[] = [];

  add(report: CspViolationReport): void {
    this.reports.push(report);
    if (this.reports.length > MAX_REPORTS) {
      this.reports.splice(0, this.reports.length - MAX_REPORTS);
    }
  }

  list(limit = 100): CspViolationReport[] {
    return this.reports.slice(-limit);
  }

  count(): number {
    return this.reports.length;
  }

  clear(): void {
    this.reports = [];
  }
}

function normalize(body: unknown): CspViolationReport[] {
  const now = Date.now();
  const entries: unknown[] = Array.isArray(body)
    ? body
    : body && typeof body === "object" && "csp-report" in (body as Record<string, unknown>)
      ? [(body as Record<string, unknown>)["csp-report"]]
      : [body];

  return entries
    .filter((e): e is Record<string, unknown> => !!e && typeof e === "object")
    .map((e) => {
      const body = (e.body as Record<string, unknown>) ?? e; // Reporting API wraps fields under `body`
      return {
        receivedAt: now,
        documentUri: (body["document-uri"] ?? body.documentURL) as string | undefined,
        violatedDirective: (body["violated-directive"] ?? body["effective-directive"]) as
          | string
          | undefined,
        blockedUri: (body["blocked-uri"] ?? body.blockedURL) as string | undefined,
        sourceFile: (body["source-file"] ?? body.sourceFile) as string | undefined,
        lineNumber: (body["line-number"] ?? body.lineNumber) as number | undefined,
        disposition: body.disposition as string | undefined,
        raw: e,
      };
    });
}

/**
 * Express handler for both the legacy `report-uri` CSP mechanism
 * (Content-Type: application/csp-report) and the modern Reporting API
 * (Content-Type: application/reports+json), plus plain JSON for testing.
 * Oversized or malformed bodies are rejected without throwing so a hostile
 * client can't use the report sink itself as an attack surface.
 */
export function createViolationReportHandler(store: ViolationReportStore) {
  return (req: Request, res: Response): void => {
    const contentLength = Number(req.headers["content-length"] ?? 0);
    if (contentLength > MAX_BODY_BYTES) {
      res.status(413).end();
      return;
    }

    try {
      const parsed = normalize(req.body);
      for (const report of parsed) store.add(report);
    } catch {
      // Never let a malformed report crash the process or leak an error to the reporter.
    }

    res.status(204).end();
  };
}
