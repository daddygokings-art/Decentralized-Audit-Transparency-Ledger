import type { ConsumeResult, RateLimitStore } from "./types";

export interface TokenBucketConfig {
  capacity: number;
  refillTokens: number;
  refillIntervalMs: number;
}

/** Classic token bucket: allows short bursts up to `capacity`, then throttles
 * to a steady `refillTokens` per `refillIntervalMs`. Good default for public
 * APIs — bursty but bounded traffic is normal client behavior. */
export async function tokenBucketConsume(
  store: RateLimitStore,
  key: string,
  cost: number,
  cfg: TokenBucketConfig,
  now: number
): Promise<ConsumeResult> {
  const { allowed, tokens, lastRefill } = await store.consumeTokenBucket(
    key,
    cost,
    cfg.capacity,
    cfg.refillTokens,
    cfg.refillIntervalMs,
    now
  );
  const resetMs = Math.max(0, cfg.refillIntervalMs - (now - lastRefill));
  return {
    allowed,
    remaining: Math.max(0, tokens),
    limit: cfg.capacity,
    resetMs,
  };
}
