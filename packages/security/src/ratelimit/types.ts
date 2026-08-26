/** Result of attempting to consume `cost` units from a key's limit. */
export interface ConsumeResult {
  allowed: boolean;
  remaining: number;
  limit: number;
  resetMs: number;
}

/**
 * Storage backend abstraction so the same algorithm implementations run
 * against an in-memory Map (single instance), a Redis Cluster (horizontally
 * scaled deployments), or Consul KV (service-mesh deployments that already
 * run Consul for discovery/config and want rate-limit state colocated
 * there). All operations must be atomic per key to be safe under concurrency.
 */
export interface RateLimitStore {
  /** Token bucket: atomically refill, then consume `cost` tokens only if
   * enough are available. `tokens` reflects the balance after the operation
   * (unchanged if denied); `allowed` tells the caller whether to admit the
   * request. */
  consumeTokenBucket(
    key: string,
    cost: number,
    capacity: number,
    refillTokens: number,
    refillIntervalMs: number,
    now: number
  ): Promise<{ allowed: boolean; tokens: number; lastRefill: number }>;

  /** Sliding window: record a hit and return the weighted count within the
   * window (current window count + previous window count * overlap weight). */
  slidingWindowHit(key: string, windowMs: number, now: number): Promise<number>;

  /** Generic counter used by the adaptive algorithm to track recent error
   * rate / backpressure signal shared across instances. */
  incrementAndGet(key: string, ttlMs: number): Promise<number>;
  get(key: string): Promise<number>;
}
