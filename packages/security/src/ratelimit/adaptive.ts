import { tokenBucketConsume } from "./tokenBucket";
import type { ConsumeResult, RateLimitStore } from "./types";

export interface AdaptiveConfig {
  /** Token bucket capacity/refill under normal (healthy) conditions. */
  baseCapacity: number;
  /** Floor the effective capacity is never throttled below, even under
   * sustained failure — keeps the API from going fully dark. */
  minCapacity: number;
  refillTokens: number;
  refillIntervalMs: number;
  /** Window over which the upstream error rate is measured. */
  errorWindowMs: number;
  /** Error rate (0-1) at which capacity is throttled to `minCapacity`.
   * Scales linearly between baseCapacity (0% errors) and minCapacity
   * (>= this threshold). */
  errorRateThreshold: number;
}

const DEFAULT_ADAPTIVE: Omit<AdaptiveConfig, "baseCapacity" | "minCapacity" | "refillTokens" | "refillIntervalMs"> = {
  errorWindowMs: 30_000,
  errorRateThreshold: 0.2,
};

/** Call from response middleware (e.g. on `res.on("finish")`) so the
 * adaptive limiter has a live signal of upstream health per scope
 * (typically per route or per backend service, not per client — the whole
 * point is to protect a struggling backend from the aggregate of all
 * clients, unlike the per-client token bucket / sliding window). */
export async function recordOutcome(
  store: RateLimitStore,
  scopeKey: string,
  success: boolean,
  windowMs: number
): Promise<void> {
  await store.incrementAndGet(`${scopeKey}:total`, windowMs);
  if (!success) await store.incrementAndGet(`${scopeKey}:errors`, windowMs);
}

async function currentErrorRate(store: RateLimitStore, scopeKey: string): Promise<number> {
  const [total, errors] = await Promise.all([store.get(`${scopeKey}:total`), store.get(`${scopeKey}:errors`)]);
  if (total === 0) return 0;
  return errors / total;
}

function scaleCapacity(cfg: AdaptiveConfig, errorRate: number): number {
  if (errorRate <= 0) return cfg.baseCapacity;
  const severity = Math.min(1, errorRate / cfg.errorRateThreshold);
  return Math.round(cfg.baseCapacity - severity * (cfg.baseCapacity - cfg.minCapacity));
}

/**
 * Adaptive rate limiting: a token bucket whose capacity contracts toward
 * `minCapacity` as the backend's recent error rate (shared across all
 * clients via `scopeKey`) rises, and relaxes back to `baseCapacity` as
 * health recovers. This protects an already-struggling service from being
 * pushed over by traffic that was individually well-behaved.
 */
export async function adaptiveConsume(
  store: RateLimitStore,
  clientKey: string,
  scopeKey: string,
  cfg: AdaptiveConfig,
  now: number
): Promise<ConsumeResult & { effectiveCapacity: number; errorRate: number }> {
  const merged = { ...DEFAULT_ADAPTIVE, ...cfg };
  const errorRate = await currentErrorRate(store, scopeKey);
  const effectiveCapacity = Math.max(merged.minCapacity, scaleCapacity(merged, errorRate));

  const result = await tokenBucketConsume(
    store,
    `adaptive:${clientKey}`,
    1,
    {
      capacity: effectiveCapacity,
      refillTokens: merged.refillTokens,
      refillIntervalMs: merged.refillIntervalMs,
    },
    now
  );

  return { ...result, effectiveCapacity, errorRate };
}
