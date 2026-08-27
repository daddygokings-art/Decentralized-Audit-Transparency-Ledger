import type { NextFunction, Request, Response } from "express";
import { createRemoteJWKSet, jwtVerify, type JWTVerifyGetKey } from "jose";
import { hasMinRole, type AccessTokenClaims, type AuthContext, type Role, type Scope } from "./types";
import type { AuthorizationServer } from "./authorizationServer";

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Express {
    interface Request {
      auth?: AuthContext;
    }
  }
}

export interface BearerAuthOptions {
  issuer: string;
  audience?: string;
  /** Provide either a local dev/test AuthorizationServer instance... */
  localIssuer?: AuthorizationServer;
  /** ...or a remote JWKS endpoint URL for a real OIDC provider. */
  jwksUri?: string;
  /** Explicit algorithm allowlist. Defaults to RS256 only — this is what
   * prevents both the classic `alg: none` bypass and RS256/HS256 key
   * confusion attacks (a public RSA key can never validate as an HMAC key
   * because jose refuses to even try an algorithm outside this list). */
  allowedAlgorithms?: string[];
}

function resolveKeySource(options: BearerAuthOptions): JWTVerifyGetKey {
  if (options.jwksUri) {
    return createRemoteJWKSet(new URL(options.jwksUri));
  }
  if (options.localIssuer) {
    const server = options.localIssuer;
    return async (protectedHeader) => {
      const { keys } = await server.jwks();
      const { importJWK } = await import("jose");
      const match = keys.find((k) => k.kid === protectedHeader.kid) ?? keys[0];
      return importJWK(match, protectedHeader.alg ?? "RS256");
    };
  }
  throw new Error("BearerAuthOptions requires either jwksUri or localIssuer");
}

/**
 * Resource-server middleware: verifies a Bearer JWT (RS256 only, signature +
 * issuer + audience + expiry), rejects revoked tokens, and populates
 * `req.auth` with subject/scopes/role/actor for downstream authorization
 * checks. Missing/invalid tokens yield 401; a valid-but-insufficient token is
 * left to `requireScopes`/`requireRole` to reject with 403.
 */
export function authenticateBearer(options: BearerAuthOptions) {
  const keySource = resolveKeySource(options);
  const algorithms = options.allowedAlgorithms ?? ["RS256"];

  return async (req: Request, res: Response, next: NextFunction): Promise<void> => {
    const header = req.headers.authorization;
    if (!header?.startsWith("Bearer ")) {
      res.status(401).json({ error: "invalid_token", error_description: "missing Bearer token" });
      return;
    }
    const token = header.slice("Bearer ".length).trim();

    try {
      const { payload } = await jwtVerify(token, keySource, {
        issuer: options.issuer,
        audience: options.audience,
        algorithms,
      });
      const claims = payload as unknown as AccessTokenClaims;

      if (options.localIssuer?.isJtiRevoked(claims.jti)) {
        res.status(401).json({ error: "invalid_token", error_description: "token has been revoked" });
        return;
      }

      req.auth = {
        clientId: claims.client_id,
        subject: claims.sub,
        scopes: (claims.scope ?? "").split(" ").filter(Boolean) as Scope[],
        role: claims.role,
        tokenId: claims.jti,
        actor: claims.act ? { subject: claims.act.sub } : undefined,
      };
      next();
    } catch (err) {
      res.status(401).json({
        error: "invalid_token",
        error_description: err instanceof Error ? err.message : "token verification failed",
      });
    }
  };
}

export type ScopeMode = "all" | "any";

/** Fine-grained authorization: require the caller's token to carry the given
 * scope(s). `mode: "all"` (default) requires every scope; `"any"` requires
 * at least one — useful for endpoints reachable via multiple scope grants. */
export function requireScopes(scopes: Scope[], mode: ScopeMode = "all") {
  return (req: Request, res: Response, next: NextFunction): void => {
    if (!req.auth) {
      res.status(401).json({ error: "invalid_token", error_description: "authentication required" });
      return;
    }
    const granted = new Set(req.auth.scopes);
    const satisfied = mode === "all" ? scopes.every((s) => granted.has(s)) : scopes.some((s) => granted.has(s));
    if (!satisfied) {
      res.status(403).json({
        error: "insufficient_scope",
        error_description: `requires scope(s) [${mode}]: ${scopes.join(", ")}`,
      });
      return;
    }
    next();
  };
}

export function requireRole(role: Role) {
  return (req: Request, res: Response, next: NextFunction): void => {
    if (!req.auth) {
      res.status(401).json({ error: "invalid_token", error_description: "authentication required" });
      return;
    }
    if (!hasMinRole(req.auth.role, role)) {
      res.status(403).json({ error: "insufficient_role", error_description: `requires role >= ${role}` });
      return;
    }
    next();
  };
}
