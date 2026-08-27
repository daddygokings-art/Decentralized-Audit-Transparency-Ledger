import { describe, expect, it, beforeEach } from "vitest";
import express from "express";
import request from "supertest";
import { SignJWT, generateKeyPair } from "jose";
import { securityHeaders } from "../src/headers/securityHeaders";
import { cspMiddleware } from "../src/headers/csp";
import { AuthorizationServer } from "../src/oauth/authorizationServer";
import { authenticateBearer, requireScopes } from "../src/oauth/middleware";
import { generateCodeChallenge, generateCodeVerifier } from "../src/oauth/pkce";
import { MemoryRateLimitStore } from "../src/ratelimit/stores/memoryStore";
import { createRateLimiter } from "../src/ratelimit/middleware";
import { WafRuleEngine } from "../src/waf/ruleEngine";
import { ddosProtection, clearWafEvents } from "../src/waf/middleware";
import { clearBotHistory } from "../src/waf/botDetection";

const ISSUER = "https://auth.attacksim.local";

/**
 * Builds a representative service stack — headers, WAF/DDoS, rate limiting,
 * OAuth2 resource server — matching how the middleware is wired into the
 * real REST API in api/rest/src/server.ts, then throws a battery of common
 * attack techniques at it to verify each layer actually holds.
 */
function buildHardenedApp(authServer: AuthorizationServer) {
  const rateLimitStore = new MemoryRateLimitStore();
  const wafEngine = new WafRuleEngine();

  const app = express();
  app.use(express.json());
  app.use(securityHeaders());
  app.use(cspMiddleware());
  app.use(ddosProtection({ ruleEngine: wafEngine, botBlockThreshold: 70 }));

  // Token endpoint gets its own tight limiter to blunt credential stuffing / brute force.
  const tokenLimiter = createRateLimiter({
    store: rateLimitStore,
    algorithm: "token-bucket",
    tokenBucket: { capacity: 5, refillTokens: 5, refillIntervalMs: 60_000 },
    keyFn: (req) => `token-endpoint:${req.ip}`,
  });
  app.use("/token", tokenLimiter);
  app.use(authServer.router());

  app.use(
    createRateLimiter({
      store: rateLimitStore,
      algorithm: "sliding-window",
      slidingWindow: { limit: 20, windowMs: 1000 },
    })
  );

  app.use(
    "/v1/events",
    authenticateBearer({ issuer: ISSUER, localIssuer: authServer }),
    requireScopes(["events:read"])
  );
  app.get("/v1/events", (_req, res) => res.json({ data: [] }));

  return app;
}

function buildFreshAuthServer(): AuthorizationServer {
  const authServer = new AuthorizationServer({ issuer: ISSUER });
  authServer.registerClient({
    clientId: "public-spa",
    name: "Public SPA",
    redirectUris: ["https://app.local/callback"],
    allowedScopes: ["events:read"],
    allowedGrantTypes: ["authorization_code"],
    isPublic: true,
    defaultRole: "viewer",
  });
  authServer.registerClient({
    clientId: "svc",
    clientSecret: "correct-horse-battery-staple",
    name: "Service",
    redirectUris: [],
    allowedScopes: ["events:read"],
    allowedGrantTypes: ["client_credentials"],
    isPublic: false,
    defaultRole: "viewer",
  });
  return authServer;
}

describe("Attack simulation", () => {
  // Each test gets its own AuthorizationServer + app + stores so that
  // rate-limit buckets, WAF event logs, and bot-history heuristics from one
  // attack scenario never bleed into another and make results order-dependent.
  let authServer: AuthorizationServer;
  let app: express.Express;

  beforeEach(() => {
    clearWafEvents();
    clearBotHistory();
    authServer = buildFreshAuthServer();
    app = buildHardenedApp(authServer);
  });

  it("blocks reflected XSS payloads in query parameters", async () => {
    const res = await request(app).get("/v1/events").query({ q: "<script>document.location='//evil.example'</script>" });
    expect(res.status).toBe(403);
  });

  it("blocks SQL injection in a JSON body", async () => {
    const res = await request(app).post("/authorize").send({ client_id: "1 UNION SELECT * FROM users" });
    expect(res.status).toBe(403);
  });

  it("blocks path traversal attempts", async () => {
    // Passed as a query value rather than a literal path segment because
    // HTTP clients (and Express itself) normalize `..` out of the URL path
    // before the server ever sees it — the query string is the realistic
    // vector for traversal payloads reaching application code unmodified.
    const res = await request(app).get("/v1/events").query({ export: "../../../../etc/passwd" });
    expect(res.status).toBe(403);
  });

  it("blocks SSRF probes at the cloud metadata endpoint", async () => {
    const res = await request(app).get("/v1/events").query({ callback: "http://169.254.169.254/latest/meta-data/" });
    expect(res.status).toBe(403);
  });

  it("throttles brute-force / credential-stuffing attempts against the token endpoint", async () => {
    // A legitimate-looking browser UA isolates this test to the rate-limit
    // layer specifically; bot-detection's own blocking of scripted clients
    // is covered separately below.
    const attempts = [];
    for (let i = 0; i < 10; i++) {
      attempts.push(
        await request(app)
          .post("/token")
          .set("User-Agent", "Mozilla/5.0")
          .set("Accept", "application/json")
          .set("Accept-Language", "en-US")
          .send({ grant_type: "client_credentials", client_id: "svc", client_secret: `guess-${i}` })
      );
    }
    const rejected = attempts.filter((r) => r.status === 400); // invalid_client
    const throttled = attempts.filter((r) => r.status === 429);
    expect(rejected.length).toBeGreaterThan(0);
    expect(throttled.length).toBeGreaterThan(0); // rate limiter kicks in before all 10 complete
  });

  it("rejects a JWT with alg:none (classic signature-bypass attack)", async () => {
    const forged = Buffer.from(JSON.stringify({ alg: "none", typ: "JWT" })).toString("base64url") +
      "." +
      Buffer.from(JSON.stringify({ sub: "attacker", scope: "events:read", role: "admin", iss: ISSUER, aud: "audit-ledger-api", client_id: "x", jti: "1", exp: Math.floor(Date.now() / 1000) + 3600 })).toString("base64url") +
      ".";

    const res = await request(app).get("/v1/events").set("Authorization", `Bearer ${forged}`);
    expect(res.status).toBe(401);
  });

  it("rejects an RS256->HS256 algorithm-confusion attack using the public key as an HMAC secret", async () => {
    const jwks = await authServer.jwks();
    const publicKeyMaterial = JSON.stringify(jwks.keys[0]);

    const forged = await new SignJWT({ scope: "events:read", role: "admin", client_id: "public-spa" })
      .setProtectedHeader({ alg: "HS256" })
      .setIssuer(ISSUER)
      .setSubject("attacker")
      .setAudience("audit-ledger-api")
      .setJti("forged-1")
      .setIssuedAt()
      .setExpirationTime("1h")
      .sign(new TextEncoder().encode(publicKeyMaterial));

    const res = await request(app).get("/v1/events").set("Authorization", `Bearer ${forged}`);
    expect(res.status).toBe(401);
  });

  it("rejects a tampered (bit-flipped signature) token", async () => {
    const tokenRes = await request(app).post("/token").send({
      grant_type: "client_credentials",
      client_id: "svc",
      client_secret: "correct-horse-battery-staple",
    });
    const parts = tokenRes.body.access_token.split(".");
    const tamperedSignature = parts[2].slice(0, -4) + "abcd";
    const tampered = `${parts[0]}.${parts[1]}.${tamperedSignature}`;

    const res = await request(app).get("/v1/events").set("Authorization", `Bearer ${tampered}`);
    expect(res.status).toBe(401);
  });

  it("rejects an expired token", async () => {
    const shortLivedServer = new AuthorizationServer({ issuer: ISSUER, accessTokenTtlSeconds: 1 });
    shortLivedServer.registerClient({
      clientId: "svc",
      clientSecret: "x",
      name: "Service",
      redirectUris: [],
      allowedScopes: ["events:read"],
      allowedGrantTypes: ["client_credentials"],
      isPublic: false,
      defaultRole: "viewer",
    });
    const shortApp = buildHardenedApp(shortLivedServer);
    const tokenRes = await request(shortApp)
      .post("/token")
      .send({ grant_type: "client_credentials", client_id: "svc", client_secret: "x" });

    await new Promise((r) => setTimeout(r, 1200));

    const res = await request(shortApp).get("/v1/events").set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(res.status).toBe(401);
  });

  it("rejects a token signed by a completely different (attacker-controlled) key", async () => {
    const { privateKey } = await generateKeyPair("RS256");
    const forged = await new SignJWT({ scope: "events:read", role: "admin", client_id: "attacker" })
      .setProtectedHeader({ alg: "RS256" })
      .setIssuer(ISSUER)
      .setSubject("attacker")
      .setAudience("audit-ledger-api")
      .setJti("forged-2")
      .setIssuedAt()
      .setExpirationTime("1h")
      .sign(privateKey);

    const res = await request(app).get("/v1/events").set("Authorization", `Bearer ${forged}`);
    expect(res.status).toBe(401);
  });

  it("rejects a PKCE downgrade attempt (public client omits code_verifier at redemption)", async () => {
    const challenge = generateCodeChallenge(generateCodeVerifier(), "S256");
    const authRes = await request(app).post("/authorize").send({
      client_id: "public-spa",
      redirect_uri: "https://app.local/callback",
      scope: "events:read",
      code_challenge: challenge,
      subject: "victim",
    });
    const tokenRes = await request(app).post("/token").send({
      grant_type: "authorization_code",
      client_id: "public-spa",
      redirect_uri: "https://app.local/callback",
      code: authRes.body.code,
      // code_verifier deliberately omitted — simulates an attacker who
      // intercepted the auth code but not the verifier trying to downgrade.
    });
    expect(tokenRes.status).toBe(400);
    expect(tokenRes.body.error).toBe("invalid_grant");
  });

  it("flags and blocks a scripted DDoS-style burst from a single client", async () => {
    const results = [];
    for (let i = 0; i < 30; i++) {
      results.push(
        await request(app)
          .get("/v1/events")
          .set("User-Agent", "python-requests/2.31.0")
      );
    }
    const blocked = results.filter((r) => r.status === 403 || r.status === 429);
    expect(blocked.length).toBeGreaterThan(0);
  });

  it("does not block a normal, well-behaved browser client", async () => {
    const res = await request(app)
      .get("/v1/events")
      .set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")
      .set("Accept", "application/json")
      .set("Accept-Language", "en-US,en;q=0.9");
    // No token supplied, so 401 is expected from the auth layer — the point
    // of this test is that WAF/bot/rate-limit layers did NOT reject it first.
    expect(res.status).toBe(401);
  });

  it("CSP violation reporting endpoint tolerates a hostile/malformed report", async () => {
    const reportApp = express();
    reportApp.use(express.json({ type: () => true }));
    const { ViolationReportStore, createViolationReportHandler } = await import("../src/headers/violationReporting");
    const store = new ViolationReportStore();
    reportApp.post("/csp-report", createViolationReportHandler(store));

    // Well-formed JSON that doesn't match any expected report shape —
    // exercises the handler's tolerance for garbage input without crashing.
    const res = await request(reportApp)
      .post("/csp-report")
      .type("application/json")
      .send(JSON.stringify([123, null, "unexpected"]));
    expect(res.status).toBe(204);
    expect(store.count()).toBe(0);
  });
});
