/**
 * Deterministic DAST target for the OWASP ZAP baseline scan (#735).
 *
 * The scan in `.github/workflows/dast.yml` needs a stable HTTP surface on
 * port 3000 that boots without a Stellar RPC endpoint. The headers it is
 * judged on, however, must be produced by real application code — not by an
 * inline `node -e* heredoc that silently drifts from production. This module
 * therefore reuses the exact same `securityHeaders()` + `cspMiddleware()`
 * middleware the REST adapter mounts, and mirrors the routes of
 * `src/server.ts` that the crawler visits.
 *
 * Run with:  npx ts-node -T src/dastTarget.ts
 */
import express from "express";

import { securityHeaders, cspMiddleware } from "@audit-ledger/security";
import { securityHeadersMiddleware } from "./middleware";

const app = express();

app.disable("x-powered-by");
app.disable("etag");

app.use(express.json());

// Same header stack as src/server.ts: Permissions-Policy, X-Powered-By
// removal and a default no-store Cache-Control (ZAP 10063 / 10037 / 10049).
app.use(securityHeaders());
app.use(securityHeadersMiddleware);

// Enforcing CSP with a `default-src 'self'` fallback, so every fetch
// directive inherits a source list (ZAP 10055).
app.use(
  cspMiddleware({
    reportOnly: process.env.CSP_REPORT_ONLY === "true",
    reportUri: "/csp-report",
    reportToGroup: "csp-endpoint",
  })
);

app.post(
  "/csp-report",
 express.json({ type: ["application/json", "application/csp-report", "application/reports+json"] }),
  (_req, res) => {
    res.status(204).end();
  }
);

// Minimal favicon response so ZAP finds a 200 (not a 404 that lacks
// security headers) when crawling /favicon.ico (issue #928).
// The middleware stack above already applied CSP, Cache-Control, and
// all other hardening headers to this response.
app.get("/favicon.ico", (_req, res) => {
  res.setHeader("Content-Type", "image/x-icon");
  res.status(200).end();
});

app.get(["/", "/robots.txt", "/sitemap.xml"], (req, res) => {
  if (req.path === "/robots.txt") {
    res.type("text/plain").send("User-agent: *\nDisallow: /");
    return;
  }
  if (req.path === "/sitemap.xml") {
    res
      .type("application/xml")
      .send('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>');
    return;
  }
  res.json({ status: "ok", name: "AuditLedger REST API", version: "v1" });
});

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

app.get("/api/events", (_req, res) => {
  res.json({ events: [], total: 0 });
});

app.get("/api/v1/events", (_req, res) => {
  res.json({ events: [], total: 0 });
});

const port = Number(process.env.PORT ?? 3000);

app.listen(port, () => {
  console.log(`DAST target listening on :${port}`);
});

export { app };
