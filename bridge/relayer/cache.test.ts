/**
 * ProofCache tests for Issue #142: Proof caching to bridge relayer
 */

import { ProofCache, EventProof } from "./index";

describe("ProofCache (Issue #142)", () => {
  let cache: ProofCache;

  beforeEach(() => {
    cache = new ProofCache(3, 5000); // max 3 entries, 5s TTL
  });

  test("should store and retrieve a proof", () => {
    const eventHash = "0xabcd1234";
    const proof: EventProof = {
      ledgerSeq: BigInt(1),
      txHash: "0x1111",
      eventIndex: 0,
      eventHash: "0xabcd1234",
      signature: "0x2222",
    };

    cache.set(eventHash, proof);
    const retrieved = cache.get(eventHash);

    expect(retrieved).toEqual(proof);
  });

  test("should return null for missing entry", () => {
    const retrieved = cache.get("0xnonexistent");
    expect(retrieved).toBeNull();
  });

  test("should evict least recently used when over capacity", () => {
    const proof1: EventProof = {
      ledgerSeq: BigInt(1),
      txHash: "0x1111",
      eventIndex: 0,
      eventHash: "0xhash1",
      signature: "0x2222",
    };
    const proof2: EventProof = {
      ledgerSeq: BigInt(2),
      txHash: "0x3333",
      eventIndex: 1,
      eventHash: "0xhash2",
      signature: "0x4444",
    };
    const proof3: EventProof = {
      ledgerSeq: BigInt(3),
      txHash: "0x5555",
      eventIndex: 2,
      eventHash: "0xhash3",
      signature: "0x6666",
    };
    const proof4: EventProof = {
      ledgerSeq: BigInt(4),
      txHash: "0x7777",
      eventIndex: 3,
      eventHash: "0xhash4",
      signature: "0x8888",
    };

    cache.set("0xhash1", proof1);
    cache.set("0xhash2", proof2);
    cache.set("0xhash3", proof3);

    expect(cache.size()).toBe(3);

    // Adding 4th proof should evict the least recently used (hash1)
    cache.set("0xhash4", proof4);

    expect(cache.size()).toBe(3);
    expect(cache.get("0xhash1")).toBeNull();
    expect(cache.get("0xhash2")).toEqual(proof2);
    expect(cache.get("0xhash3")).toEqual(proof3);
    expect(cache.get("0xhash4")).toEqual(proof4);
  });

  test("should respect LRU ordering on get", () => {
    const proof1: EventProof = {
      ledgerSeq: BigInt(1),
      txHash: "0x1111",
      eventIndex: 0,
      eventHash: "0xhash1",
      signature: "0x2222",
    };
    const proof2: EventProof = {
      ledgerSeq: BigInt(2),
      txHash: "0x3333",
      eventIndex: 1,
      eventHash: "0xhash2",
      signature: "0x4444",
    };
    const proof3: EventProof = {
      ledgerSeq: BigInt(3),
      txHash: "0x5555",
      eventIndex: 2,
      eventHash: "0xhash3",
      signature: "0x6666",
    };
    const proof4: EventProof = {
      ledgerSeq: BigInt(4),
      txHash: "0x7777",
      eventIndex: 3,
      eventHash: "0xhash4",
      signature: "0x8888",
    };

    cache.set("0xhash1", proof1);
    cache.set("0xhash2", proof2);
    cache.set("0xhash3", proof3);

    // Accessing hash1 makes it recently used
    cache.get("0xhash1");

    // Adding 4th should evict hash2 (now least recently used)
    cache.set("0xhash4", proof4);

    expect(cache.get("0xhash1")).toEqual(proof1);
    expect(cache.get("0xhash2")).toBeNull();
    expect(cache.get("0xhash3")).toEqual(proof3);
    expect(cache.get("0xhash4")).toEqual(proof4);
  });

  test("should clear all entries", () => {
    const proof: EventProof = {
      ledgerSeq: BigInt(1),
      txHash: "0x1111",
      eventIndex: 0,
      eventHash: "0xhash1",
      signature: "0x2222",
    };

    cache.set("0xhash1", proof);
    expect(cache.size()).toBe(1);

    cache.clear();
    expect(cache.size()).toBe(0);
    expect(cache.get("0xhash1")).toBeNull();
  });

  test("should skip expired entries", (done) => {
    const shortTtlCache = new ProofCache(10, 100); // 100ms TTL

    const proof: EventProof = {
      ledgerSeq: BigInt(1),
      txHash: "0x1111",
      eventIndex: 0,
      eventHash: "0xhash1",
      signature: "0x2222",
    };

    shortTtlCache.set("0xhash1", proof);
    expect(shortTtlCache.get("0xhash1")).toEqual(proof);

    // Wait for TTL to expire
    setTimeout(() => {
      expect(shortTtlCache.get("0xhash1")).toBeNull();
      done();
    }, 150);
  });

  test("should support configurable max size and TTL", () => {
    const largeCache = new ProofCache(100, 10000);
    expect(largeCache.size()).toBe(0);

    // Fill cache
    for (let i = 0; i < 100; i++) {
      const proof: EventProof = {
        ledgerSeq: BigInt(i),
        txHash: `0x${i.toString(16)}`,
        eventIndex: i,
        eventHash: `0xhash${i}`,
        signature: `0xsig${i}`,
      };
      largeCache.set(`0xhash${i}`, proof);
    }

    expect(largeCache.size()).toBe(100);

    // Adding 101st should evict the first
    const proof101: EventProof = {
      ledgerSeq: BigInt(100),
      txHash: "0x64",
      eventIndex: 100,
      eventHash: "0xhash100",
      signature: "0xsig100",
    };
    largeCache.set("0xhash100", proof101);

    expect(largeCache.size()).toBe(100);
    expect(largeCache.get("0xhash0")).toBeNull();
  });
});
