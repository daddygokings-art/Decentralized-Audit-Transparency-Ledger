import {
  AuthorizationServer,
  MemoryRateLimitStore,
  RedisClusterRateLimitStore,
  ConsulRateLimitStore,
  WafRuleEngine,
  type RateLimitStore,
  type OAuthClient,
} from "@audit-ledger/security";

export const OAUTH_ISSUER = process.env.OAUTH_ISSUER ?? "http://localhost:3002/oauth";

/**
 * The OIDC/OAuth2 issuer. When `OIDC_JWKS_URI` is configured, deployments
 * are expected to run a real external IdP (Auth0/Okta/Keycloak/etc) and the
 * `authenticateBearer` resource-server middleware verifies against that
 * instead — this local issuer only exists to make the API self-sufficient
 * for local development, CI, and the demo clients below.
 */
export const authorizationServer = new AuthorizationServer({ issuer: OAUTH_ISSUER });

const DEFAULT_CLIENTS: OAuthClient[] = [
  {
    clientId: process.env.OAUTH_SPA_CLIENT_ID ?? "audit-ledger-dashboard",
    name: "Audit Ledger Dashboard (public SPA)",
    redirectUris: (process.env.OAUTH_SPA_REDIRECT_URIS ?? "http://localhost:3000/callback").split(","),
    allowedScopes: ["events:read", "stats:read", "export:read"],
    allowedGrantTypes: ["authorization_code", "refresh_token"],
    isPublic: true, // no client secret — MUST use PKCE
    defaultRole: "viewer",
  },
  {
    clientId: process.env.OAUTH_SERVICE_CLIENT_ID ?? "audit-ledger-ingest-service",
    clientSecret: process.env.OAUTH_SERVICE_CLIENT_SECRET ?? "dev-only-service-secret-change-me",
    name: "Backend ingest/automation service",
    redirectUris: [],
    allowedScopes: ["events:read", "events:write", "stats:read", "export:read", "admin:keys", "admin:waf"],
    allowedGrantTypes: ["client_credentials", "urn:ietf:params:oauth:grant-type:token-exchange"],
    isPublic: false,
    defaultRole: "admin",
  },
];

for (const client of DEFAULT_CLIENTS) authorizationServer.registerClient(client);

export const wafRuleEngine = new WafRuleEngine();

/**
 * Selects a rate-limit backend to match the deployment topology:
 *  - `redis-cluster` — multiple horizontally-scaled API instances behind a
 *    load balancer, sharing limits via Redis Cluster (REDIS_CLUSTER_NODES).
 *  - `consul` — deployments that already run Consul for service mesh /
 *    config and prefer not to add Redis as another moving part
 *    (CONSUL_HTTP_ADDR).
 *  - `memory` (default) — single instance / local dev / CI.
 */
export function createConfiguredRateLimitStore(): RateLimitStore {
  const backend = (process.env.RATE_LIMIT_BACKEND ?? "memory").toLowerCase();

  if (backend === "redis-cluster" || backend === "redis") {
    // Loaded lazily, via `require` rather than a static import, so ioredis
    // (and an actual Redis endpoint) are only required when this backend is
    // selected — and so this file doesn't statically pull in ioredis's own
    // type declarations, which would otherwise collide with the separate
    // copy of ioredis's types resolved inside @audit-ledger/security's own
    // node_modules (two structurally-similar but nominally distinct
    // `Cluster`/`Redis` classes, since this repo has no npm workspace to
    // dedupe them).
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const IORedis = require("ioredis");
    const nodes = (process.env.REDIS_CLUSTER_NODES ?? "127.0.0.1:6379")
      .split(",")
      .map((hostPort) => {
        const [host, port] = hostPort.trim().split(":");
        return { host, port: Number(port) || 6379 };
      });

    const client = backend === "redis-cluster" && nodes.length > 1
      ? new IORedis.Cluster(nodes)
      : new IORedis(nodes[0].port, nodes[0].host);

    return new RedisClusterRateLimitStore(client);
  }

  if (backend === "consul") {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const Consul = require("consul");
    const client = new Consul({
      host: process.env.CONSUL_HTTP_ADDR_HOST ?? "127.0.0.1",
      port: process.env.CONSUL_HTTP_ADDR_PORT ?? "8500",
      promisify: true,
    });
    return new ConsulRateLimitStore(client);
  }

  return new MemoryRateLimitStore();
}
