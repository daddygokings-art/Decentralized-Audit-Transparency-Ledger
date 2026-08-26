import { beforeAll, describe, expect, it } from "vitest";
import express from "express";
import request from "supertest";
import { AuthorizationServer } from "../src/oauth/authorizationServer";
import { authenticateBearer, requireRole, requireScopes } from "../src/oauth/middleware";
import { generateCodeChallenge, generateCodeVerifier } from "../src/oauth/pkce";

const ISSUER = "https://auth.test.local";

function buildResourceServer(authServer: AuthorizationServer) {
  const app = express();
  app.use(express.json());
  app.use(authServer.router());
  app.use(
    "/v1/events",
    authenticateBearer({ issuer: ISSUER, localIssuer: authServer }),
    requireScopes(["events:read"])
  );
  app.get("/v1/events", (_req, res) => res.json({ data: [] }));

  app.use(
    "/v1/admin",
    authenticateBearer({ issuer: ISSUER, localIssuer: authServer }),
    requireRole("admin")
  );
  app.get("/v1/admin/keys", (_req, res) => res.json({ data: [] }));

  return app;
}

describe("OAuth2/OIDC authorization server + resource server", () => {
  let authServer: AuthorizationServer;
  let app: express.Express;

  beforeAll(() => {
    authServer = new AuthorizationServer({ issuer: ISSUER });
    authServer.registerClient({
      clientId: "public-spa",
      name: "Public SPA",
      redirectUris: ["https://app.test.local/callback"],
      allowedScopes: ["events:read", "stats:read"],
      allowedGrantTypes: ["authorization_code", "refresh_token"],
      isPublic: true,
      defaultRole: "viewer",
    });
    authServer.registerClient({
      clientId: "backend-service",
      clientSecret: "s3cr3t",
      name: "Backend Service",
      redirectUris: [],
      allowedScopes: ["events:read", "events:write", "admin:keys"],
      allowedGrantTypes: ["client_credentials", "urn:ietf:params:oauth:grant-type:token-exchange"],
      isPublic: false,
      defaultRole: "admin",
    });
    app = buildResourceServer(authServer);
  });

  it("completes the authorization code + PKCE flow end to end", async () => {
    const verifier = generateCodeVerifier();
    const challenge = generateCodeChallenge(verifier, "S256");

    const authRes = await request(app).post("/authorize").send({
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      scope: "events:read",
      code_challenge: challenge,
      code_challenge_method: "S256",
      subject: "user-42",
    });
    expect(authRes.status).toBe(200);
    const { code } = authRes.body;

    const tokenRes = await request(app).post("/token").send({
      grant_type: "authorization_code",
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      code,
      code_verifier: verifier,
    });
    expect(tokenRes.status).toBe(200);
    expect(tokenRes.body.access_token).toBeDefined();
    expect(tokenRes.body.refresh_token).toBeDefined();

    const resourceRes = await request(app)
      .get("/v1/events")
      .set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(resourceRes.status).toBe(200);
  });

  it("rejects the authorization code grant without PKCE for a public client", async () => {
    const authRes = await request(app).post("/authorize").send({
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      scope: "events:read",
      subject: "user-42",
    });
    expect(authRes.status).toBe(400);
    expect(authRes.body.error).toBe("invalid_request");
  });

  it("rejects the token exchange when the code_verifier does not match the challenge", async () => {
    const challenge = generateCodeChallenge(generateCodeVerifier(), "S256");
    const authRes = await request(app).post("/authorize").send({
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      scope: "events:read",
      code_challenge: challenge,
      subject: "user-42",
    });
    const tokenRes = await request(app).post("/token").send({
      grant_type: "authorization_code",
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      code: authRes.body.code,
      code_verifier: "some-other-verifier-that-does-not-match-1234",
    });
    expect(tokenRes.status).toBe(400);
    expect(tokenRes.body.error).toBe("invalid_grant");
  });

  it("prevents authorization code replay (single use)", async () => {
    const verifier = generateCodeVerifier();
    const challenge = generateCodeChallenge(verifier, "S256");
    const authRes = await request(app).post("/authorize").send({
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      scope: "events:read",
      code_challenge: challenge,
      subject: "user-42",
    });
    const first = await request(app).post("/token").send({
      grant_type: "authorization_code",
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      code: authRes.body.code,
      code_verifier: verifier,
    });
    expect(first.status).toBe(200);

    const replay = await request(app).post("/token").send({
      grant_type: "authorization_code",
      client_id: "public-spa",
      redirect_uri: "https://app.test.local/callback",
      code: authRes.body.code,
      code_verifier: verifier,
    });
    expect(replay.status).toBe(400);
    expect(replay.body.error).toBe("invalid_grant");
  });

  it("issues client_credentials tokens with admin role and enforces requireRole", async () => {
    const tokenRes = await request(app).post("/token").send({
      grant_type: "client_credentials",
      client_id: "backend-service",
      client_secret: "s3cr3t",
      scope: "admin:keys",
    });
    expect(tokenRes.status).toBe(200);

    const adminRes = await request(app)
      .get("/v1/admin/keys")
      .set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(adminRes.status).toBe(200);
  });

  it("rejects requests with insufficient scope with 403, not 401", async () => {
    const tokenRes = await request(app).post("/token").send({
      grant_type: "client_credentials",
      client_id: "backend-service",
      client_secret: "s3cr3t",
      scope: "events:write",
    });
    const res = await request(app).get("/v1/events").set("Authorization", `Bearer ${tokenRes.body.access_token}`);
    expect(res.status).toBe(403);
    expect(res.body.error).toBe("insufficient_scope");
  });

  it("performs RFC 8693 token exchange producing a delegated token with `act` claim", async () => {
    const clientRes = await request(app).post("/token").send({
      grant_type: "client_credentials",
      client_id: "backend-service",
      client_secret: "s3cr3t",
      scope: "events:read",
    });
    const subjectToken = clientRes.body.access_token;

    const exchangeRes = await request(app).post("/token").send({
      grant_type: "urn:ietf:params:oauth:grant-type:token-exchange",
      client_id: "backend-service",
      client_secret: "s3cr3t",
      subject_token: subjectToken,
      subject_token_type: "urn:ietf:params:oauth:token-type:access_token",
      scope: "events:read",
    });
    expect(exchangeRes.status).toBe(200);
    expect(exchangeRes.body.issued_token_type).toBe("urn:ietf:params:oauth:token-type:access_token");

    const introspect = await request(app).post("/introspect").send({ token: exchangeRes.body.access_token });
    expect(introspect.body.active).toBe(true);
    expect(introspect.body.act.sub).toBe("backend-service");
  });

  it("rejects an unauthenticated request with 401", async () => {
    const res = await request(app).get("/v1/events");
    expect(res.status).toBe(401);
  });

  it("exposes JWKS and OIDC discovery documents", async () => {
    const jwks = await request(app).get("/jwks.json");
    expect(jwks.status).toBe(200);
    expect(jwks.body.keys).toHaveLength(1);

    const discovery = await request(app).get("/.well-known/openid-configuration");
    expect(discovery.body.issuer).toBe(ISSUER);
    expect(discovery.body.grant_types_supported).toContain("urn:ietf:params:oauth:grant-type:token-exchange");
  });
});
