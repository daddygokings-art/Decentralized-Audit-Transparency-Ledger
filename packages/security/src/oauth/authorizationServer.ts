import { randomUUID, randomBytes } from "crypto";
import { SignJWT, exportJWK, generateKeyPair, type JWK, type KeyLike } from "jose";
import type { Request, Response, Router } from "express";
import { Router as createRouter } from "express";
import { verifyCodeChallenge, type CodeChallengeMethod } from "./pkce";
import type { AccessTokenClaims, GrantType, OAuthClient, Role, Scope } from "./types";

interface AuthorizationCodeRecord {
  code: string;
  clientId: string;
  redirectUri: string;
  scope: Scope[];
  subject: string;
  codeChallenge?: string;
  codeChallengeMethod?: CodeChallengeMethod;
  expiresAt: number;
  used: boolean;
}

interface RefreshTokenRecord {
  token: string;
  clientId: string;
  subject: string;
  scope: Scope[];
  expiresAt: number;
  revoked: boolean;
}

export interface AuthorizationServerOptions {
  issuer: string;
  accessTokenTtlSeconds?: number;
  refreshTokenTtlSeconds?: number;
  authCodeTtlSeconds?: number;
}

/**
 * A minimal, self-contained OAuth 2.1 / OIDC-flavored Authorization Server.
 *
 * This exists so the Audit Ledger services have a real, spec-following
 * token issuer to develop and test against without depending on an external
 * IdP. In production, deployments should point `authenticateBearer` at an
 * external OIDC provider's JWKS URI instead of running this issuer — the
 * resource-server middleware in `middleware.ts` supports either.
 *
 * Implements:
 *  - Authorization Code grant with mandatory PKCE (RFC 7636) for public clients
 *  - Client Credentials grant (service-to-service)
 *  - Refresh Token grant
 *  - Token Exchange (RFC 8693) for delegated/on-behalf-of calls
 *  - JWKS + OIDC discovery documents
 */
export class AuthorizationServer {
  private clients = new Map<string, OAuthClient>();
  private authCodes = new Map<string, AuthorizationCodeRecord>();
  private refreshTokens = new Map<string, RefreshTokenRecord>();
  private revokedJti = new Set<string>();
  private keyPairPromise: Promise<{ publicKey: KeyLike; privateKey: KeyLike; kid: string }>;
  private opts: Required<AuthorizationServerOptions>;

  constructor(options: AuthorizationServerOptions) {
    this.opts = {
      accessTokenTtlSeconds: 900,
      refreshTokenTtlSeconds: 60 * 60 * 24 * 30,
      authCodeTtlSeconds: 60,
      ...options,
    };
    this.keyPairPromise = generateKeyPair("RS256").then(({ publicKey, privateKey }) => ({
      publicKey,
      privateKey,
      kid: randomUUID(),
    }));
  }

  registerClient(client: OAuthClient): void {
    if (client.isPublic && client.clientSecret) {
      throw new Error("Public clients must not have a client secret");
    }
    if (!client.isPublic && !client.clientSecret) {
      throw new Error("Confidential clients require a client secret");
    }
    this.clients.set(client.clientId, client);
  }

  getClient(clientId: string): OAuthClient | undefined {
    return this.clients.get(clientId);
  }

  isJtiRevoked(jti: string): boolean {
    return this.revokedJti.has(jti);
  }

  revokeJti(jti: string): void {
    this.revokedJti.add(jti);
  }

  async jwks(): Promise<{ keys: JWK[] }> {
    const { publicKey, kid } = await this.keyPairPromise;
    const jwk = await exportJWK(publicKey);
    return { keys: [{ ...jwk, kid, alg: "RS256", use: "sig" }] };
  }

  discoveryDocument(): Record<string, unknown> {
    return {
      issuer: this.opts.issuer,
      authorization_endpoint: `${this.opts.issuer}/authorize`,
      token_endpoint: `${this.opts.issuer}/token`,
      jwks_uri: `${this.opts.issuer}/jwks.json`,
      introspection_endpoint: `${this.opts.issuer}/introspect`,
      revocation_endpoint: `${this.opts.issuer}/revoke`,
      response_types_supported: ["code"],
      grant_types_supported: [
        "authorization_code",
        "client_credentials",
        "refresh_token",
        "urn:ietf:params:oauth:grant-type:token-exchange",
      ],
      code_challenge_methods_supported: ["S256", "plain"],
      subject_types_supported: ["public"],
      id_token_signing_alg_values_supported: ["RS256"],
    };
  }

  private async mintAccessToken(params: {
    clientId: string;
    subject: string;
    scope: Scope[];
    role: Role;
    audience?: string;
    actorSubject?: string;
  }): Promise<{ token: string; jti: string; expiresIn: number }> {
    const { privateKey, kid } = await this.keyPairPromise;
    const jti = randomUUID();
    const now = Math.floor(Date.now() / 1000);
    const expiresIn = this.opts.accessTokenTtlSeconds;

    let jwt = new SignJWT({
      scope: params.scope.join(" "),
      role: params.role,
      client_id: params.clientId,
      ...(params.actorSubject ? { act: { sub: params.actorSubject } } : {}),
    })
      .setProtectedHeader({ alg: "RS256", kid })
      .setIssuer(this.opts.issuer)
      .setSubject(params.subject)
      .setAudience(params.audience ?? "audit-ledger-api")
      .setJti(jti)
      .setIssuedAt(now)
      .setExpirationTime(now + expiresIn);

    const token = await jwt.sign(privateKey);
    return { token, jti, expiresIn };
  }

  /** Authorization endpoint: validates the request and issues a code. Real
   * user-consent UI is out of scope here — the resource owner is assumed to
   * be the bearer of the request (suitable for service accounts / dev). */
  authorize(params: {
    clientId: string;
    redirectUri: string;
    scope: string;
    subject: string;
    codeChallenge?: string;
    codeChallengeMethod?: CodeChallengeMethod;
  }): { code: string } | { error: string; description: string } {
    const client = this.clients.get(params.clientId);
    if (!client) return { error: "invalid_client", description: "unknown client_id" };
    if (!client.redirectUris.includes(params.redirectUri)) {
      return { error: "invalid_request", description: "redirect_uri mismatch" };
    }
    if (client.isPublic && !params.codeChallenge) {
      return { error: "invalid_request", description: "PKCE code_challenge is required for public clients" };
    }

    const requestedScopes = params.scope.split(" ").filter(Boolean) as Scope[];
    const disallowed = requestedScopes.filter((s) => !client.allowedScopes.includes(s));
    if (disallowed.length > 0) {
      return { error: "invalid_scope", description: `client not permitted: ${disallowed.join(", ")}` };
    }

    const code = randomBytes(24).toString("hex");
    this.authCodes.set(code, {
      code,
      clientId: params.clientId,
      redirectUri: params.redirectUri,
      scope: requestedScopes,
      subject: params.subject,
      codeChallenge: params.codeChallenge,
      codeChallengeMethod: params.codeChallengeMethod ?? "S256",
      expiresAt: Date.now() + this.opts.authCodeTtlSeconds * 1000,
      used: false,
    });
    return { code };
  }

  async token(body: Record<string, unknown>): Promise<
    | {
        access_token: string;
        token_type: "Bearer";
        expires_in: number;
        refresh_token?: string;
        scope: string;
        issued_token_type?: string;
      }
    | { error: string; error_description: string }
  > {
    const grantType = body.grant_type as GrantType | undefined;
    if (!grantType) return { error: "invalid_request", error_description: "grant_type is required" };

    if (grantType === "authorization_code") return this.handleAuthCodeGrant(body);
    if (grantType === "client_credentials") return this.handleClientCredentialsGrant(body);
    if (grantType === "refresh_token") return this.handleRefreshGrant(body);
    if (grantType === "urn:ietf:params:oauth:grant-type:token-exchange") {
      return this.handleTokenExchange(body);
    }
    return { error: "unsupported_grant_type", error_description: `unsupported grant_type: ${grantType}` };
  }

  private authenticateClient(body: Record<string, unknown>): OAuthClient | { error: string } {
    const clientId = body.client_id as string | undefined;
    if (!clientId) return { error: "invalid_client" };
    const client = this.clients.get(clientId);
    if (!client) return { error: "invalid_client" };
    if (!client.isPublic) {
      if (client.clientSecret !== body.client_secret) return { error: "invalid_client" };
    }
    return client;
  }

  private async handleAuthCodeGrant(body: Record<string, unknown>) {
    const client = this.authenticateClient(body);
    if ("error" in client) return { error: client.error, error_description: "client authentication failed" };

    const code = body.code as string | undefined;
    const record = code ? this.authCodes.get(code) : undefined;
    if (!record || record.used || record.expiresAt < Date.now() || record.clientId !== client.clientId) {
      return { error: "invalid_grant", error_description: "authorization code is invalid, expired, or reused" };
    }
    if (record.redirectUri !== body.redirect_uri) {
      return { error: "invalid_grant", error_description: "redirect_uri mismatch" };
    }

    if (client.isPublic || record.codeChallenge) {
      const verifier = body.code_verifier as string | undefined;
      if (!verifier || !record.codeChallenge) {
        return { error: "invalid_grant", error_description: "code_verifier (PKCE) is required" };
      }
      if (!verifyCodeChallenge(verifier, record.codeChallenge, record.codeChallengeMethod)) {
        return { error: "invalid_grant", error_description: "PKCE verification failed" };
      }
    }

    record.used = true; // authorization codes are single-use (RFC 6749 §4.1.2)

    const { token, expiresIn } = await this.mintAccessToken({
      clientId: client.clientId,
      subject: record.subject,
      scope: record.scope,
      role: client.defaultRole,
    });

    const refreshToken = randomBytes(32).toString("hex");
    this.refreshTokens.set(refreshToken, {
      token: refreshToken,
      clientId: client.clientId,
      subject: record.subject,
      scope: record.scope,
      expiresAt: Date.now() + this.opts.refreshTokenTtlSeconds * 1000,
      revoked: false,
    });

    return {
      access_token: token,
      token_type: "Bearer" as const,
      expires_in: expiresIn,
      refresh_token: refreshToken,
      scope: record.scope.join(" "),
    };
  }

  private async handleClientCredentialsGrant(body: Record<string, unknown>) {
    const client = this.authenticateClient(body);
    if ("error" in client) return { error: client.error, error_description: "client authentication failed" };
    if (client.isPublic) {
      return { error: "unauthorized_client", error_description: "public clients cannot use client_credentials" };
    }
    if (!client.allowedGrantTypes.includes("client_credentials")) {
      return { error: "unauthorized_client", error_description: "grant type not permitted for this client" };
    }

    const requested = ((body.scope as string) ?? client.allowedScopes.join(" "))
      .split(" ")
      .filter(Boolean) as Scope[];
    const disallowed = requested.filter((s) => !client.allowedScopes.includes(s));
    if (disallowed.length > 0) {
      return { error: "invalid_scope", error_description: `not permitted: ${disallowed.join(", ")}` };
    }

    const { token, expiresIn } = await this.mintAccessToken({
      clientId: client.clientId,
      subject: `service:${client.clientId}`,
      scope: requested,
      role: client.defaultRole,
    });

    return { access_token: token, token_type: "Bearer" as const, expires_in: expiresIn, scope: requested.join(" ") };
  }

  private async handleRefreshGrant(body: Record<string, unknown>) {
    const client = this.authenticateClient(body);
    if ("error" in client) return { error: client.error, error_description: "client authentication failed" };

    const token = body.refresh_token as string | undefined;
    const record = token ? this.refreshTokens.get(token) : undefined;
    if (!record || record.revoked || record.expiresAt < Date.now() || record.clientId !== client.clientId) {
      return { error: "invalid_grant", error_description: "refresh token is invalid, expired, or revoked" };
    }

    // Rotate: revoke the old refresh token and issue a new one (mitigates replay).
    record.revoked = true;
    const { token: accessToken, expiresIn } = await this.mintAccessToken({
      clientId: client.clientId,
      subject: record.subject,
      scope: record.scope,
      role: client.defaultRole,
    });
    const newRefreshToken = randomBytes(32).toString("hex");
    this.refreshTokens.set(newRefreshToken, {
      token: newRefreshToken,
      clientId: client.clientId,
      subject: record.subject,
      scope: record.scope,
      expiresAt: Date.now() + this.opts.refreshTokenTtlSeconds * 1000,
      revoked: false,
    });

    return {
      access_token: accessToken,
      token_type: "Bearer" as const,
      expires_in: expiresIn,
      refresh_token: newRefreshToken,
      scope: record.scope.join(" "),
    };
  }

  /** RFC 8693 Token Exchange — lets a trusted service (e.g. an API gateway)
   * swap a subject_token for a new, narrower-scoped, delegated access token
   * (impersonation/on-behalf-of), recording the original caller in `act`. */
  private async handleTokenExchange(body: Record<string, unknown>) {
    const client = this.authenticateClient(body);
    if ("error" in client) return { error: client.error, error_description: "client authentication failed" };
    if (!client.allowedGrantTypes.includes("urn:ietf:params:oauth:grant-type:token-exchange")) {
      return { error: "unauthorized_client", error_description: "token exchange not permitted for this client" };
    }

    const subjectToken = body.subject_token as string | undefined;
    const subjectTokenType = body.subject_token_type as string | undefined;
    if (!subjectToken || subjectTokenType !== "urn:ietf:params:oauth:token-type:access_token") {
      return { error: "invalid_request", error_description: "subject_token and subject_token_type are required" };
    }

    let claims: AccessTokenClaims;
    try {
      const { jwtVerify } = await import("jose");
      const { publicKey } = await this.keyPairPromise;
      const { payload } = await jwtVerify(subjectToken, publicKey, { issuer: this.opts.issuer });
      claims = payload as unknown as AccessTokenClaims;
    } catch {
      return { error: "invalid_grant", error_description: "subject_token is invalid or expired" };
    }
    if (this.isJtiRevoked(claims.jti)) {
      return { error: "invalid_grant", error_description: "subject_token has been revoked" };
    }

    const requestedScope = ((body.scope as string) ?? claims.scope).split(" ").filter(Boolean) as Scope[];
    const disallowed = requestedScope.filter((s) => !client.allowedScopes.includes(s) || !claims.scope.split(" ").includes(s));
    if (disallowed.length > 0) {
      return {
        error: "invalid_scope",
        error_description: `exchanged token cannot exceed subject_token or client scope: ${disallowed.join(", ")}`,
      };
    }

    const { token, expiresIn } = await this.mintAccessToken({
      clientId: client.clientId,
      subject: claims.sub,
      scope: requestedScope,
      role: client.defaultRole,
      audience: (body.audience as string) ?? undefined,
      actorSubject: client.clientId,
    });

    return {
      access_token: token,
      token_type: "Bearer" as const,
      expires_in: expiresIn,
      scope: requestedScope.join(" "),
      issued_token_type: "urn:ietf:params:oauth:token-type:access_token",
    };
  }

  /** Mounts /authorize, /token, /jwks.json, /introspect, /revoke and the
   * discovery documents. Intended for local development and CI; production
   * deployments should use an external, hardened OIDC provider instead. */
  router(): Router {
    const router = createRouter();

    router.get("/jwks.json", async (_req: Request, res: Response) => {
      res.json(await this.jwks());
    });

    router.get("/.well-known/openid-configuration", (_req, res) => {
      res.json(this.discoveryDocument());
    });
    router.get("/.well-known/oauth-authorization-server", (_req, res) => {
      res.json(this.discoveryDocument());
    });

    router.post("/authorize", (req: Request, res: Response) => {
      const subject = (req.body.subject as string) || (req.headers["x-debug-subject"] as string) || "anonymous";
      const result = this.authorize({
        clientId: req.body.client_id,
        redirectUri: req.body.redirect_uri,
        scope: req.body.scope ?? "",
        subject,
        codeChallenge: req.body.code_challenge,
        codeChallengeMethod: req.body.code_challenge_method,
      });
      if ("error" in result) return res.status(400).json(result);
      res.json(result);
    });

    router.post("/token", async (req: Request, res: Response) => {
      const result = await this.token(req.body ?? {});
      if ("error" in result) return res.status(400).json(result);
      res.json(result);
    });

    router.post("/introspect", async (req: Request, res: Response) => {
      const token = req.body.token as string | undefined;
      if (!token) return res.status(400).json({ error: "invalid_request" });
      try {
        const { jwtVerify } = await import("jose");
        const { publicKey } = await this.keyPairPromise;
        const { payload } = await jwtVerify(token, publicKey, { issuer: this.opts.issuer });
        const claims = payload as unknown as AccessTokenClaims;
        if (this.isJtiRevoked(claims.jti)) return res.json({ active: false });
        res.json({ active: true, ...claims });
      } catch {
        res.json({ active: false });
      }
    });

    router.post("/revoke", (req: Request, res: Response) => {
      const jti = req.body.jti as string | undefined;
      if (jti) this.revokeJti(jti);
      res.status(200).json({ revoked: !!jti });
    });

    return router;
  }
}
