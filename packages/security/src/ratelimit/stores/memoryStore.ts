import type { RateLimitStore } from "../types";

interface Bucket {
  tokens: number;
  lastRefill: number;
}
interface Window {
  currentCount: number;
  currentStart: number;
  previousCount: number;
}
interface Counter {
  value: number;
  expiresAt: number;
}

/**
 * Single-process store. Appropriate for local development, tests, or a
 * standalone deployment with no horizontal scaling requirement.
 */
export class MemoryRateLimitStore implements RateLimitStore {
  private buckets = new Map<string, Bucket>();
  private windows = new Map<string, Window>();
  private counters = new Map<string, Counter>();

  async consumeTokenBucket(
    key: string,
    cost: number,
    capacity: number,
    refillTokens: number,
    refillIntervalMs: number,
    now: number
  ) {
    let bucket = this.buckets.get(key);
    if (!bucket) {
      bucket = { tokens: capacity, lastRefill: now };
      this.buckets.set(key, bucket);
    }
    const elapsed = now - bucket.lastRefill;
    const refillCount = Math.floor(elapsed / refillIntervalMs) * refillTokens;
    if (refillCount > 0) {
      bucket.tokens = Math.min(capacity, bucket.tokens + refillCount);
      bucket.lastRefill = now;
    }
    const allowed = bucket.tokens >= cost;
    if (allowed) bucket.tokens -= cost;
    return { allowed, tokens: bucket.tokens, lastRefill: bucket.lastRefill };
  }

  async slidingWindowHit(key: string, windowMs: number, now: number): Promise<number> {
    let w = this.windows.get(key);
    const windowStart = Math.floor(now / windowMs) * windowMs;
    if (!w || w.currentStart !== windowStart) {
      const rollingOverFromPrevious = w && windowStart - w.currentStart === windowMs ? w.currentCount : 0;
      w = { currentCount: 0, currentStart: windowStart, previousCount: rollingOverFromPrevious };
      this.windows.set(key, w);
    }
    w.currentCount++;
    const elapsedInWindow = now - w.currentStart;
    const overlapWeight = Math.max(0, (windowMs - elapsedInWindow) / windowMs);
    return w.currentCount + w.previousCount * overlapWeight;
  }

  async incrementAndGet(key: string, ttlMs: number): Promise<number> {
    const now = Date.now();
    let c = this.counters.get(key);
    if (!c || c.expiresAt < now) {
      c = { value: 0, expiresAt: now + ttlMs };
      this.counters.set(key, c);
    }
    c.value++;
    return c.value;
  }

  async get(key: string): Promise<number> {
    const c = this.counters.get(key);
    if (!c || c.expiresAt < Date.now()) return 0;
    return c.value;
  }

  clear(): void {
    this.buckets.clear();
    this.windows.clear();
    this.counters.clear();
  }
}
