export type Role = "viewer" | "auditor" | "admin";

export const ROLE_HIERARCHY: Record<Role, number> = {
  viewer: 1,
  auditor: 2,
  admin: 3,
};

export function hasMinRole(userRole: Role | undefined | null, requiredRole: Role): boolean {
  if (!userRole) return false;
  return (ROLE_HIERARCHY[userRole] ?? 0) >= ROLE_HIERARCHY[requiredRole];
}

/** Scopes recognized by the Audit Ledger APIs. Services may extend this set. */
export type Scope =
  | "events:read"
  | "events:write"
  | "stats:read"
  | "export:read"
  | "admin:keys"
  | "admin:waf";

export interface OAuthClient {
  clientId: string;
  clientSecret?: string; // absent for public clients (PKCE required)
  name: string;
  redirectUris: string[];
  allowedScopes: Scope[];
  allowedGrantTypes: GrantType[];
  isPublic: boolean;
  defaultRole: Role;
}

export type GrantType =
  | "authorization_code"
  | "client_credentials"
  | "refresh_token"
  | "urn:ietf:params:oauth:grant-type:token-exchange";

export interface AccessTokenClaims {
  iss: string;
  sub: string;
  aud: string;
  client_id: string;
  scope: string;
  role: Role;
  exp: number;
  iat: number;
  jti: string;
  /** Present only on tokens minted via RFC 8693 token exchange. */
  act?: { sub: string };
}

export interface AuthContext {
  clientId: string;
  subject: string;
  scopes: Scope[];
  role: Role;
  tokenId: string;
  actor?: { subject: string };
}
