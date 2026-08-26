import { describe, expect, it } from "vitest";
import express from "express";
import request from "supertest";
import { MemoryRateLimitStore } from "../src/ratelimit/stores/memoryStore";
import { tokenBucketConsume } from "../src/ratelimit/tokenBucket";
import { slidingWindowConsume } from "../src/ratelimit/slidingWindow";
import { adaptiveConsume, recordOutcome } from "../src/ratelimit/adaptive";
import { createRateLimiter } from "../src/ratelimit/middleware";

describe("Token bucket algorithm", () => {
  it("allows bursts up to capacity then throttles", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = { capacity: 3, refillTokens: 1, refillIntervalMs: 1000 };
    const now = 1_000_000;

    const r1 = await tokenBucketConsume(store, "k", 1, cfg, now);
    const r2 = await tokenBucketConsume(store, "k", 1, cfg, now);
    const r3 = await tokenBucketConsume(store, "k", 1, cfg, now);
    const r4 = await tokenBucketConsume(store, "k", 1, cfg, now);

    expect([r1, r2, r3].every((r) => r.allowed)).toBe(true);
    expect(r4.allowed).toBe(false);
  });

  it("refills tokens over time", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = { capacity: 2, refillTokens: 2, refillIntervalMs: 1000 };
    const now = 1_000_000;
    await tokenBucketConsume(store, "k", 2, cfg, now);
    const exhausted = await tokenBucketConsume(store, "k", 1, cfg, now);
    expect(exhausted.allowed).toBe(false);

    const afterRefill = await tokenBucketConsume(store, "k", 1, cfg, now + 1000);
    expect(afterRefill.allowed).toBe(true);
  });
});

describe("Sliding window algorithm", () => {
  it("blocks once the weighted count exceeds the limit within a window", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = { limit: 3, windowMs: 1000 };
    const now = 5000;
    const results = [];
    for (let i = 0; i < 5; i++) {
      results.push(await slidingWindowConsume(store, "k", cfg, now));
    }
    expect(results.filter((r) => r.allowed)).toHaveLength(3);
    expect(results.filter((r) => !r.allowed)).toHaveLength(2);
  });

  it("smooths across a window boundary using the previous window's weight", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = { limit: 4, windowMs: 1000 };
    // Fill window [0,1000) with 4 hits at the very end of the window.
    for (let i = 0; i < 4; i++) await slidingWindowConsume(store, "k", cfg, 950);
    // Immediately into the next window — most of the previous window still overlaps,
    // so this should still be constrained rather than getting a fresh full quota.
    const boundaryHit = await slidingWindowConsume(store, "k", cfg, 1005);
    expect(boundaryHit.allowed).toBe(false);
  });
});

describe("Adaptive algorithm", () => {
  it("shrinks effective capacity as the backend error rate rises", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = {
      baseCapacity: 100,
      minCapacity: 10,
      refillTokens: 100,
      refillIntervalMs: 1000,
      errorWindowMs: 30_000,
      errorRateThreshold: 0.2,
    };
    const scope = "route:/v1/events";

    // Simulate a 50% error rate upstream (above the 20% threshold -> min capacity).
    for (let i = 0; i < 10; i++) {
      await recordOutcome(store, scope, i % 2 === 0, cfg.errorWindowMs);
    }

    const result = await adaptiveConsume(store, "client-1", scope, cfg, Date.now());
    expect(result.effectiveCapacity).toBe(cfg.minCapacity);
    expect(result.errorRate).toBeCloseTo(0.5, 1);
  });

  it("keeps full capacity when the backend is healthy", async () => {
    const store = new MemoryRateLimitStore();
    const cfg = {
      baseCapacity: 100,
      minCapacity: 10,
      refillTokens: 100,
      refillIntervalMs: 1000,
      errorWindowMs: 30_000,
      errorRateThreshold: 0.2,
    };
    const scope = "route:/v1/stats";
    for (let i = 0; i < 10; i++) await recordOutcome(store, scope, true, cfg.errorWindowMs);

    const result = await adaptiveConsume(store, "client-1", scope, cfg, Date.now());
    expect(result.effectiveCapacity).toBe(cfg.baseCapacity);
  });
});

describe("createRateLimiter middleware", () => {
  it("sets RateLimit-* headers and returns 429 with Retry-After once exhausted", async () => {
    const store = new MemoryRateLimitStore();
    const app = express();
    app.use(
      createRateLimiter({
        store,
        algorithm: "token-bucket",
        tokenBucket: { capacity: 2, refillTokens: 1, refillIntervalMs: 60_000 },
      })
    );
    app.get("/", (_req, res) => res.send("ok"));

    const r1 = await request(app).get("/");
    const r2 = await request(app).get("/");
    const r3 = await request(app).get("/");

    expect(r1.status).toBe(200);
    expect(r1.headers["ratelimit-limit"]).toBe("2");
    expect(r2.status).toBe(200);
    expect(r3.status).toBe(429);
    expect(r3.headers["retry-after"]).toBeDefined();
  });

  it("isolates limits per client key (different IPs don't share a bucket)", async () => {
    const store = new MemoryRateLimitStore();
    const app = express();
    app.set("trust proxy", true);
    app.use(
      createRateLimiter({
        store,
        algorithm: "token-bucket",
        tokenBucket: { capacity: 1, refillTokens: 1, refillIntervalMs: 60_000 },
      })
    );
    app.get("/", (_req, res) => res.send("ok"));

    const a1 = await request(app).get("/").set("X-Forwarded-For", "1.1.1.1");
    const a2 = await request(app).get("/").set("X-Forwarded-For", "1.1.1.1");
    const b1 = await request(app).get("/").set("X-Forwarded-For", "2.2.2.2");

    expect(a1.status).toBe(200);
    expect(a2.status).toBe(429);
    expect(b1.status).toBe(200);
  });

  it("fails open if the store throws", async () => {
    const brokenStore = {
      consumeTokenBucket: async () => {
        throw new Error("backend down");
      },
      slidingWindowHit: async () => 0,
      incrementAndGet: async () => 0,
      get: async () => 0,
    };
    const app = express();
    app.use(
      createRateLimiter({
        store: brokenStore,
        algorithm: "token-bucket",
        tokenBucket: { capacity: 1, refillTokens: 1, refillIntervalMs: 60_000 },
      })
    );
    app.get("/", (_req, res) => res.send("ok"));

    const res = await request(app).get("/");
    expect(res.status).toBe(200);
    expect(res.headers["x-ratelimit-backend-error"]).toBe("true");
  });
});
