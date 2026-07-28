export interface RateLimitConfig {
  /** Maximum burst size (max tokens the bucket can hold). */
  capacity: number;
  /** Tokens added back per second. */
  refillRatePerSec: number;
  /** Respect a `Retry-After` header by blocking the bucket until it elapses. Default true. */
  respectRetryAfter?: boolean;
}

export interface RateLimitHeaders {
  limit?: number;
  remaining?: number;
  resetSeconds?: number;
  retryAfterSeconds?: number;
}

type HeaderSource = Headers | Record<string, string | string[] | undefined>;

function readHeader(headers: HeaderSource, name: string): string | undefined {
  if (typeof (headers as Headers).get === 'function') {
    return (headers as Headers).get(name) ?? undefined;
  }
  const record = headers as Record<string, string | string[] | undefined>;
  const value = record[name] ?? record[name.toLowerCase()] ?? record[name.toUpperCase()];
  return Array.isArray(value) ? value[0] : value;
}

function toNumber(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  const n = Number(value);
  return Number.isFinite(n) ? n : undefined;
}

/**
 * Parse standard rate-limit response headers:
 * `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`, `Retry-After`.
 */
export function parseRateLimitHeaders(headers: HeaderSource): RateLimitHeaders {
  return {
    limit: toNumber(readHeader(headers, 'X-RateLimit-Limit')),
    remaining: toNumber(readHeader(headers, 'X-RateLimit-Remaining')),
    resetSeconds: toNumber(readHeader(headers, 'X-RateLimit-Reset')),
    retryAfterSeconds: toNumber(readHeader(headers, 'Retry-After')),
  };
}

/** Client-side token bucket rate limiter. */
export class TokenBucket {
  readonly capacity: number;
  readonly refillRatePerSec: number;
  private readonly respectRetryAfter: boolean;
  private tokens: number;
  private lastRefill: number;
  private blockedUntil = 0;

  constructor(config: RateLimitConfig) {
    if (config.capacity <= 0) throw new Error('capacity must be > 0');
    if (config.refillRatePerSec <= 0) throw new Error('refillRatePerSec must be > 0');
    this.capacity = config.capacity;
    this.refillRatePerSec = config.refillRatePerSec;
    this.respectRetryAfter = config.respectRetryAfter ?? true;
    this.tokens = config.capacity;
    this.lastRefill = Date.now();
  }

  private refill(): void {
    const now = Date.now();
    const elapsedSec = (now - this.lastRefill) / 1000;
    if (elapsedSec > 0) {
      this.tokens = Math.min(this.capacity, this.tokens + elapsedSec * this.refillRatePerSec);
      this.lastRefill = now;
    }
  }

  /** Attempt to consume `count` tokens immediately. Returns false if unavailable. */
  tryConsume(count = 1): boolean {
    if (Date.now() < this.blockedUntil) return false;
    this.refill();
    if (this.tokens >= count) {
      this.tokens -= count;
      return true;
    }
    return false;
  }

  /** Milliseconds until `count` tokens will be available. */
  msUntilAvailable(count = 1): number {
    const now = Date.now();
    if (now < this.blockedUntil) return this.blockedUntil - now;
    this.refill();
    if (this.tokens >= count) return 0;
    return Math.ceil(((count - this.tokens) / this.refillRatePerSec) * 1000);
  }

  /** Resolve once `count` tokens are available, waiting as needed. */
  async acquire(count = 1): Promise<void> {
    for (;;) {
      if (this.tryConsume(count)) return;
      const wait = Math.max(this.msUntilAvailable(count), 1);
      await new Promise((resolve) => setTimeout(resolve, wait));
    }
  }

  /** Block all consumption until the given number of seconds elapse (e.g. from a Retry-After header). */
  blockFor(seconds: number): void {
    if (seconds <= 0) return;
    this.blockedUntil = Math.max(this.blockedUntil, Date.now() + seconds * 1000);
  }

  /** Apply a Retry-After hint from parsed rate-limit headers, if configured to do so. */
  applyHeaders(headers: RateLimitHeaders): void {
    if (this.respectRetryAfter && headers.retryAfterSeconds !== undefined) {
      this.blockFor(headers.retryAfterSeconds);
    }
  }

  get availableTokens(): number {
    this.refill();
    return this.tokens;
  }
}
