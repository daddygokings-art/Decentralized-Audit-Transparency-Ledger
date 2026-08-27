import type { NextFunction, Request, Response } from "express";
import { tokenBucketConsume, type TokenBucketConfig } from "./tokenBucket";
import { slidingWindowConsume, type SlidingWindowConfig } from "./slidingWindow";
import { adaptiveConsume, recordOutcome, type AdaptiveConfig } from "./adaptive";
import type { RateLimitStore } from "./types";

export type Algorithm = "token-bucket" | "sliding-window" | "adaptive";

export interface RateLimiterOptions {
  store: RateLimitStore;
  algorithm: Algorithm;
  keyFn?: (req: Request) => string;
  tokenBucket?: TokenBucketConfig;
  slidingWindow?: SlidingWindowConfig;
  adaptive?: AdaptiveConfig & { scopeKeyFn?: (req: Request) => string };
  /** cost of this request in tokens/count units; default 1 */
  cost?: number;
}

function defaultKeyFn(req: Request): string {
  const apiKey = req.headers["x-api-key"] as string | undefined;
  const auth = req.auth?.subject;
  const forwardedFor = (req.headers["x-forwarded-for"] as string | undefined)?.split(",")[0]?.trim();
  return auth ?? apiKey ?? `ip:${forwardedFor ?? req.ip ?? "unknown"}`;
}

function setHeaders(res: Response, limit: number, remaining: number, resetMs: number): void {
  res.setHeader("RateLimit-Limit", String(limit));
  res.setHeader("RateLimit-Remaining", String(Math.max(0, remaining)));
  res.setHeader("RateLimit-Reset", String(Math.ceil(resetMs / 1000)));
  // Legacy headers kept for existing clients/tests in this codebase.
  res.setHeader("X-RateLimit-Limit", String(limit));
  res.setHeader("X-RateLimit-Remaining", String(Math.max(0, remaining)));
  res.setHeader("X-RateLimit-Reset", String(Math.ceil(resetMs / 1000)));
}

/**
 * Builds an Express middleware for one of the three supported algorithms,
 * backed by whichever `RateLimitStore` fits the deployment (in-memory,
 * Redis Cluster, or Consul — see ./stores).
 */
export function createRateLimiter(options: RateLimiterOptions) {
  const keyFn = options.keyFn ?? defaultKeyFn;
  const cost = options.cost ?? 1;

  return async (req: Request, res: Response, next: NextFunction): Promise<void> => {
    const now = Date.now();
    const key = keyFn(req);

    try {
      if (options.algorithm === "token-bucket") {
        if (!options.tokenBucket) throw new Error("tokenBucket config required");
        const result = await tokenBucketConsume(options.store, key, cost, options.tokenBucket, now);
        setHeaders(res, result.limit, result.remaining, result.resetMs);
        if (!result.allowed) {
          res.setHeader("Retry-After", String(Math.ceil(result.resetMs / 1000)));
          res.status(429).json({ error: "rate_limited", algorithm: "token-bucket", retryAfterMs: result.resetMs });
          return;
        }
      } else if (options.algorithm === "sliding-window") {
        if (!options.slidingWindow) throw new Error("slidingWindow config required");
        const result = await slidingWindowConsume(options.store, key, options.slidingWindow, now);
        setHeaders(res, result.limit, result.remaining, result.resetMs);
        if (!result.allowed) {
          res.setHeader("Retry-After", String(Math.ceil(result.resetMs / 1000)));
          res.status(429).json({ error: "rate_limited", algorithm: "sliding-window", retryAfterMs: result.resetMs });
          return;
        }
      } else {
        if (!options.adaptive) throw new Error("adaptive config required");
        const scopeKey = options.adaptive.scopeKeyFn?.(req) ?? `route:${req.baseUrl}${req.path}`;
        const result = await adaptiveConsume(options.store, key, scopeKey, options.adaptive, now);
        setHeaders(res, result.limit, result.remaining, result.resetMs);
        res.setHeader("X-RateLimit-Adaptive-Capacity", String(result.effectiveCapacity));
        res.setHeader("X-RateLimit-Adaptive-ErrorRate", result.errorRate.toFixed(3));

        res.on("finish", () => {
          void recordOutcome(options.store, scopeKey, res.statusCode < 500, options.adaptive!.errorWindowMs ?? 30_000);
        });

        if (!result.allowed) {
          res.setHeader("Retry-After", String(Math.ceil(result.resetMs / 1000)));
          res.status(429).json({ error: "rate_limited", algorithm: "adaptive", retryAfterMs: result.resetMs });
          return;
        }
      }
      next();
    } catch (err) {
      // A rate-limit backend outage must never take down the API itself —
      // fail open, but surface the failure for observability.
      res.setHeader("X-RateLimit-Backend-Error", "true");
      next();
    }
  };
}
