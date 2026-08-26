import { randomBytes } from "crypto";
import type { NextFunction, Request, Response } from "express";

export type CspDirectives = Record<string, string[]>;

export interface CspOptions {
  /** Directive name -> list of sources. Use the literal string "'nonce'" as a
   * placeholder anywhere a per-request nonce should be substituted. */
  directives?: CspDirectives;
  /** When true, sends Content-Security-Policy-Report-Only instead of the
   * enforcing header. Useful for rolling out a new policy safely. */
  reportOnly?: boolean;
  /** Path the browser should POST violation reports to. */
  reportUri?: string;
  /** Reporting-API group name (modern `report-to` mechanism). */
  reportToGroup?: string;
  /** Skip nonce generation/injection entirely. */
  useNonce?: boolean;
}

const DEFAULT_DIRECTIVES: CspDirectives = {
  "default-src": ["'self'"],
  "script-src": ["'self'", "'nonce'"],
  "style-src": ["'self'", "'nonce'"],
  "img-src": ["'self'", "data:"],
  "font-src": ["'self'"],
  "connect-src": ["'self'"],
  "object-src": ["'none'"],
  "base-uri": ["'self'"],
  "frame-ancestors": ["'none'"],
  "form-action": ["'self'"],
  "upgrade-insecure-requests": [],
};

export function generateNonce(): string {
  return randomBytes(16).toString("base64");
}

export function buildCspHeader(directives: CspDirectives, nonce?: string): string {
  return Object.entries(directives)
    .map(([name, sources]) => {
      const resolved = sources.map((s) => (s === "'nonce'" && nonce ? `'nonce-${nonce}'` : s));
      // Drop the placeholder if no nonce was generated for this response.
      const cleaned = resolved.filter((s) => s !== "'nonce'");
      return cleaned.length > 0 ? `${name} ${cleaned.join(" ")}` : name;
    })
    .join("; ");
}

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Express {
    interface Locals {
      cspNonce?: string;
    }
  }
}

/**
 * Express middleware that issues a fresh per-request nonce, builds the CSP
 * header (merging caller overrides onto sane defaults), and supports
 * report-only rollout plus both legacy (report-uri) and modern
 * (Reporting-API report-to) violation reporting.
 */
export function cspMiddleware(options: CspOptions = {}) {
  const directives: CspDirectives = { ...DEFAULT_DIRECTIVES, ...(options.directives ?? {}) };
  const useNonce = options.useNonce ?? true;

  return (req: Request, res: Response, next: NextFunction): void => {
    const nonce = useNonce ? generateNonce() : undefined;
    if (nonce) res.locals.cspNonce = nonce;

    const merged: CspDirectives = { ...directives };
    if (options.reportUri) {
      merged["report-uri"] = [options.reportUri];
    }
    if (options.reportToGroup) {
      merged["report-to"] = [options.reportToGroup];
      res.setHeader(
        "Reporting-Endpoints",
        `${options.reportToGroup}="${options.reportUri ?? "/csp-report"}"`
      );
    }

    const header = buildCspHeader(merged, nonce);
    const headerName = options.reportOnly
      ? "Content-Security-Policy-Report-Only"
      : "Content-Security-Policy";
    res.setHeader(headerName, header);
    next();
  };
}
