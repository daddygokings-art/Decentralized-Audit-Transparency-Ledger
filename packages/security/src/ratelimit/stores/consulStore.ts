import type { RateLimitStore } from "../types";

/** The subset of node-consul's KV API this store depends on (promisified). */
export interface ConsulKvClient {
  kv: {
    get(options: { key: string }): Promise<{ Value: string; ModifyIndex: number } | undefined>;
    set(options: { key: string; value: string; cas?: number }): Promise<boolean>;
  };
}

interface StoredState {
  v: number; // counter or token count
  t: number; // timestamp (lastRefill, window start, or expiry)
  p?: number; // previous window count (sliding window only)
}

const MAX_CAS_RETRIES = 5;

/**
 * Store backed by Consul KV. Intended for deployments that already run
 * Consul as their service-mesh/config backbone and want rate-limit state
 * colocated there rather than standing up Redis. Consul KV has no native
 * atomic counter, so correctness comes from optimistic concurrency
 * (check-and-set on ModifyIndex) with bounded retries — a reasonable
 * trade-off given Consul's read-heavy, lower-throughput consistency model
 * versus Redis. For very high request-rate limiting, prefer the Redis
 * Cluster store; Consul is best suited to coarser, per-tenant limits.
 */
export class ConsulRateLimitStore implements RateLimitStore {
  constructor(
    private readonly client: ConsulKvClient,
    private readonly keyPrefix = "audit-ledger/ratelimit"
  ) {}

  private async casLoop<T>(key: string, mutate: (existing: StoredState | null) => StoredState, extract: (s: StoredState) => T): Promise<T> {
    const fullKey = `${this.keyPrefix}/${key}`;
    for (let attempt = 0; attempt < MAX_CAS_RETRIES; attempt++) {
      const entry = await this.client.kv.get({ key: fullKey });
      const existing: StoredState | null = entry ? JSON.parse(entry.Value) : null;
      const next = mutate(existing);
      const ok = await this.client.kv.set({
        key: fullKey,
        value: JSON.stringify(next),
        cas: entry?.ModifyIndex ?? 0,
      });
      if (ok) return extract(next);
    }
    throw new Error(`Consul CAS contention exceeded ${MAX_CAS_RETRIES} retries for key ${key}`);
  }

  async consumeTokenBucket(
    key: string,
    cost: number,
    capacity: number,
    refillTokens: number,
    refillIntervalMs: number,
    now: number
  ) {
    let allowedFlag = false;
    const result = await this.casLoop(
      `tb:${key}`,
      (existing) => {
        let tokens = existing?.v ?? capacity;
        let lastRefill = existing?.t ?? now;
        const elapsed = now - lastRefill;
        const refillCount = Math.floor(elapsed / refillIntervalMs) * refillTokens;
        if (refillCount > 0) {
          tokens = Math.min(capacity, tokens + refillCount);
          lastRefill = now;
        }
        allowedFlag = tokens >= cost;
        if (allowedFlag) tokens -= cost;
        return { v: tokens, t: lastRefill };
      },
      (s) => ({ tokens: s.v, lastRefill: s.t })
    );
    return { allowed: allowedFlag, ...result };
  }

  async slidingWindowHit(key: string, windowMs: number, now: number): Promise<number> {
    const windowStart = Math.floor(now / windowMs) * windowMs;
    return this.casLoop(
      `sw:${key}`,
      (existing) => {
        if (!existing || existing.t !== windowStart) {
          const rolledOver = existing && windowStart - existing.t === windowMs ? existing.v : 0;
          return { v: 1, t: windowStart, p: rolledOver };
        }
        return { v: existing.v + 1, t: existing.t, p: existing.p ?? 0 };
      },
      (s) => {
        const elapsedInWindow = now - s.t;
        const overlapWeight = Math.max(0, (windowMs - elapsedInWindow) / windowMs);
        return s.v + (s.p ?? 0) * overlapWeight;
      }
    );
  }

  async incrementAndGet(key: string, ttlMs: number): Promise<number> {
    const now = Date.now();
    return this.casLoop(
      `ctr:${key}`,
      (existing) => {
        if (!existing || existing.t < now) {
          return { v: 1, t: now + ttlMs };
        }
        return { v: existing.v + 1, t: existing.t };
      },
      (s) => s.v
    );
  }

  async get(key: string): Promise<number> {
    const entry = await this.client.kv.get({ key: `${this.keyPrefix}/ctr:${key}` });
    if (!entry) return 0;
    const state: StoredState = JSON.parse(entry.Value);
    return state.t < Date.now() ? 0 : state.v;
  }
}
