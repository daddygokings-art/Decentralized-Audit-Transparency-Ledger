import { describe, expect, it } from "vitest";
import express from "express";
import request from "supertest";
import { cspMiddleware, generateNonce, buildCspHeader } from "../src/headers/csp";
import { securityHeaders } from "../src/headers/securityHeaders";
import { ViolationReportStore, createViolationReportHandler } from "../src/headers/violationReporting";

describe("CSP", () => {
  it("generates unique nonces", () => {
    const a = generateNonce();
    const b = generateNonce();
    expect(a).not.toEqual(b);
    expect(a.length).toBeGreaterThan(10);
  });

  it("substitutes the nonce placeholder into script-src/style-src", () => {
    const header = buildCspHeader({ "script-src": ["'self'", "'nonce'"] }, "abc123");
    expect(header).toBe("script-src 'self' 'nonce-abc123'");
  });

  it("emits a distinct nonce per request and matches it in the header", async () => {
    const app = express();
    app.use(cspMiddleware());
    app.get("/", (_req, res) => res.json({ nonce: res.locals.cspNonce }));

    const res1 = await request(app).get("/");
    const res2 = await request(app).get("/");

    const header1 = res1.headers["content-security-policy"];
    const header2 = res2.headers["content-security-policy"];
    expect(header1).toContain(`'nonce-${res1.body.nonce}'`);
    expect(header2).toContain(`'nonce-${res2.body.nonce}'`);
    expect(res1.body.nonce).not.toEqual(res2.body.nonce);
  });

  it("supports report-only mode without blocking behavior", async () => {
    const app = express();
    app.use(cspMiddleware({ reportOnly: true, reportUri: "/csp-report" }));
    app.get("/", (_req, res) => res.send("ok"));

    const res = await request(app).get("/");
    expect(res.headers["content-security-policy"]).toBeUndefined();
    expect(res.headers["content-security-policy-report-only"]).toContain("report-uri /csp-report");
  });

  it("accepts and stores a legacy csp-report violation", async () => {
    const store = new ViolationReportStore();
    const app = express();
    app.use(express.json({ type: ["application/json", "application/csp-report", "application/reports+json"] }));
    app.post("/csp-report", createViolationReportHandler(store));

    await request(app)
      .post("/csp-report")
      .set("Content-Type", "application/csp-report")
      .send(
        JSON.stringify({
          "csp-report": { "document-uri": "https://example.com", "violated-directive": "script-src", "blocked-uri": "eval" },
        })
      );

    expect(store.count()).toBe(1);
    expect(store.list()[0].violatedDirective).toBe("script-src");
  });

  it("rejects oversized violation reports without crashing", async () => {
    const store = new ViolationReportStore();
    const app = express();
    app.use(express.json({ limit: "1mb" }));
    app.post("/csp-report", createViolationReportHandler(store));

    const bigPayload = { "csp-report": { "document-uri": "x".repeat(40 * 1024) } };
    const res = await request(app).post("/csp-report").send(bigPayload);
    expect(res.status).toBe(413);
    expect(store.count()).toBe(0);
  });
});

describe("Security headers", () => {
  it("sets the standard hardening headers", async () => {
    const app = express();
    app.use(securityHeaders());
    app.get("/", (_req, res) => res.send("ok"));

    const res = await request(app).get("/");
    expect(res.headers["x-frame-options"]).toBe("DENY");
    expect(res.headers["x-content-type-options"]).toBe("nosniff");
    expect(res.headers["referrer-policy"]).toBe("no-referrer");
    expect(res.headers["cross-origin-opener-policy"]).toBe("same-origin");
    expect(res.headers["permissions-policy"]).toContain("geolocation=()");
    expect(res.headers["x-powered-by"]).toBeUndefined();
  });

  it("only sends HSTS over an already-secure connection", async () => {
    const app = express();
    app.use(securityHeaders());
    app.get("/", (_req, res) => res.send("ok"));

    const plain = await request(app).get("/");
    expect(plain.headers["strict-transport-security"]).toBeUndefined();

    const viaProxy = await request(app).get("/").set("X-Forwarded-Proto", "https");
    expect(viaProxy.headers["strict-transport-security"]).toContain("max-age=");
  });
});
