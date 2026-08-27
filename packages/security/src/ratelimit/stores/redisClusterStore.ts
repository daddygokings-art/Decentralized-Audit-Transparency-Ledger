import type { Cluster, Redis } from "ioredis";
import type { RateLimitStore } from "../types";

// Atomic token-bucket refill+consume. KEYS[1] = tokens key, KEYS[2] = lastRefill key.
const TOKEN_BUCKET_SCRIPT = `
local tokensKey = KEYS[1]
local refillKey = KEYS[2]
local cost = tonumber(ARGV[1])
local capacity = tonumber(ARGV[2])
local refillTokens = tonumber(ARGV[3])
local refillIntervalMs = tonumber(ARGV[4])
local now = tonumber(ARGV[5])
local ttlSec = tonumber(ARGV[6])

local tokens = tonumber(redis.call("GET", tokensKey))
local lastRefill = tonumber(redis.call("GET", refillKey))
if tokens == nil then
  tokens = capacity
  lastRefill = now
end

local elapsed = now - lastRefill
local refillCount = math.floor(elapsed / refillIntervalMs) * refillTokens
if refillCount > 0 then
  tokens = math.min(capacity, tokens + refillCount)
  lastRefill = now
end

local allowed = 0
if tokens >= cost then
  tokens = tokens - cost
  allowed = 1
end

redis.call("SET", tokensKey, tokens, "EX", ttlSec)
redis.call("SET", refillKey, lastRefill, "EX", ttlSec)
return {allowed, tokens, lastRefill}
`;

// Fixed-window-with-overlap sliding counter. KEYS[1] = current window key, KEYS[2] = previous.
const SLIDING_WINDOW_SCRIPT = `
local curKey = KEYS[1]
local prevKey = KEYS[2]
local windowMs = tonumber(ARGV[1])
local ttlSec = tonumber(ARGV[2])
local elapsedInWindow = tonumber(ARGV[3])

local cur = redis.call("INCR", curKey)
if cur == 1 then
  redis.call("EXPIRE", curKey, ttlSec)
end
local prev = tonumber(redis.call("GET", prevKey)) or 0
local overlapWeight = math.max(0, (windowMs - elapsedInWindow) / windowMs)
return cur + prev * overlapWeight
`;

/**
 * Distributed store backed by Redis Cluster (or a single Redis node — same
 * client interface). Rate-limit state is shared atomically across every API
 * instance behind the load balancer via Lua scripts (EVAL), so a client
 * can't evade limits by being routed to a different pod.
 */
export class RedisClusterRateLimitStore implements RateLimitStore {
  constructor(private readonly client: Cluster | Redis) {}

  async consumeTokenBucket(
    key: string,
    cost: number,
    capacity: number,
    refillTokens: number,
    refillIntervalMs: number,
    now: number
  ) {
    const ttlSec = Math.max(60, Math.ceil((refillIntervalMs * 2) / 1000));
    const [allowed, tokens, lastRefill] = (await this.client.eval(
      TOKEN_BUCKET_SCRIPT,
      2,
      `rl:tb:${key}:tokens`,
      `rl:tb:${key}:refill`,
      cost,
      capacity,
      refillTokens,
      refillIntervalMs,
      now,
      ttlSec
    )) as [number, number, number];
    return { allowed: allowed === 1, tokens, lastRefill };
  }

  async slidingWindowHit(key: string, windowMs: number, now: number): Promise<number> {
    const windowIndex = Math.floor(now / windowMs);
    const elapsedInWindow = now - windowIndex * windowMs;
    const ttlSec = Math.max(1, Math.ceil((windowMs * 2) / 1000));
    const result = (await this.client.eval(
      SLIDING_WINDOW_SCRIPT,
      2,
      `rl:sw:${key}:${windowIndex}`,
      `rl:sw:${key}:${windowIndex - 1}`,
      windowMs,
      ttlSec,
      elapsedInWindow
    )) as number;
    return result;
  }

  async incrementAndGet(key: string, ttlMs: number): Promise<number> {
    const redisKey = `rl:ctr:${key}`;
    const value = await this.client.incr(redisKey);
    if (value === 1) {
      await this.client.pexpire(redisKey, ttlMs);
    }
    return value;
  }

  async get(key: string): Promise<number> {
    const value = await this.client.get(`rl:ctr:${key}`);
    return value ? parseInt(value, 10) : 0;
  }
}
