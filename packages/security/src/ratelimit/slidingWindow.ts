import type { ConsumeResult, RateLimitStore } from "./types";

export interface SlidingWindowConfig {
  limit: number;
  windowMs: number;
}

/** Weighted sliding-window counter: smooths the "burst at window boundary"
 * problem of fixed windows by blending the previous window's count in
 * proportion to how much of it still overlaps the current window. More
 * accurate than a fixed window under bursty traffic, cheaper than a full
 * sliding log. */
export async function slidingWindowConsume(
  store: RateLimitStore,
  key: string,
  cfg: SlidingWindowConfig,
  now: number
): Promise<ConsumeResult> {
  const weightedCount = await store.slidingWindowHit(key, cfg.windowMs, now);
  const allowed = weightedCount <= cfg.limit;
  const windowStart = Math.floor(now / cfg.windowMs) * cfg.windowMs;
  const resetMs = cfg.windowMs - (now - windowStart);
  return {
    allowed,
    remaining: Math.max(0, Math.floor(cfg.limit - weightedCount)),
    limit: cfg.limit,
    resetMs,
  };
}
