import { describe, it, expect } from "vitest";
import request from "supertest";
import type { Test } from "supertest";
import { app } from "../src/server";
import { authorizationServer, OAUTH_ISSUER } from "../src/security";

// A realistic client identifies itself — matches how the dashboard SPA and
// backend service clients actually call this API. Omitting these headers
// (supertest's default) looks like anonymous scripted traffic and is
// exactly what the bot-detection layer is designed to flag, so requests
// standing in for legitimate callers set them explicitly here.
function withClientHeaders(req: Test): Test {
  return req
    .set("User-Agent", "audit-ledger-integration-tests/1.0")
    .set("Accept", "application/json")
    .set("Accept-Language", "en-US");
}

const client = {
  get: (path: string) => withClientHeaders(request(app).get(path)),
  post: (path: string) => withClientHeaders(request(app).post(path)),
};

describe("REST API security integration", () => {
  it("applies security headers and a nonce-based CSP to every response", async () => {
    const res = await client.get("/healthz");
    expect(res.headers["x-frame-options"]).toBe("DENY");
    expect(res.headers["x-content-type-options"]).toBe("nosniff");
    expect(res.headers["content-security-policy"]).toMatch(/'nonce-/);
  });

  it("exposes OIDC discovery and JWKS under /oauth", async () => {
    const discovery = await client.get("/oauth/.well-known/openid-configuration");
    expect(discovery.status).toBe(200);
    expect(discovery.body.issuer).toBe(OAUTH_ISSUER);

    const jwks = await client.get("/oauth/jwks.json");
    expect(jwks.status).toBe(200);
    expect(jwks.body.keys.length).toBeGreaterThan(0);
  });

  it("rejects admin endpoints without a bearer token", async () => {
    const res = await client.get("/v1/admin/keys");
    expect(res.status).toBe(401);
  });

  it("allows the seeded service client to mint an admin token and manage keys", async () => {
    const tokenRes = await client.post("/oauth/token").send({
      grant_type: "client_credentials",
      client_id: "audit-ledger-ingest-service",
      client_secret: "dev-only-service-secret-change-me",
      scope: "admin:keys",
    });
    expect(tokenRes.status).toBe(200);

    const keysRes = await client
      .get("/v1/admin/keys")
      .set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(keysRes.status).toBe(200);
    expect(Array.isArray(keysRes.body.data)).toBe(true);
  });

  it("rejects a viewer-scoped token attempting to reach the admin key endpoint", async () => {
    const verifier = "a-verifier-that-is-at-least-43-characters-long-ok";
    const { generateCodeChallenge } = await import("@audit-ledger/security");
    const challenge = generateCodeChallenge(verifier, "S256");

    const authRes = await client.post("/oauth/authorize").send({
      client_id: "audit-ledger-dashboard",
      redirect_uri: "http://localhost:3000/callback",
      scope: "events:read",
      code_challenge: challenge,
      subject: "user-1",
    });
    const tokenRes = await client.post("/oauth/token").send({
      grant_type: "authorization_code",
      client_id: "audit-ledger-dashboard",
      redirect_uri: "http://localhost:3000/callback",
      code: authRes.body.code,
      code_verifier: verifier,
    });

    const res = await client
      .get("/v1/admin/keys")
      .set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(res.status).toBe(403);
  });

  it("blocks an obvious SQL injection attempt against a v1 endpoint", async () => {
    const res = await client.get("/v1/events").query({ filter: "1' OR '1'='1" });
    expect(res.status).toBe(403);
  });

  it("still serves the legacy unversioned redirect", async () => {
    const res = await client.get("/stats");
    expect(res.status).toBe(301);
    expect(res.headers.location).toBe("/v1/stats");
  });
});
