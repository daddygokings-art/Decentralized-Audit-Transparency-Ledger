import { describe, expect, it } from "vitest";
import { DataLoader } from "../src/dataloader";

describe("DataLoader", () => {
  it("batches and deduplicates keys while preserving order", async () => {
    const batches: string[][] = [];
    const loader = new DataLoader<string, number>(async (keys) => {
      batches.push([...keys]);
      return keys.map((key) => Number(key) * 2);
    });

    const values = await Promise.all([loader.load("1"), loader.load("2"), loader.load("1")]);

    expect(values).toEqual([2, 4, 2]);
    expect(batches).toEqual([["1", "2"]]);
  });

  it("caches successful loads and supports explicit invalidation", async () => {
    let calls = 0;
    const loader = new DataLoader<string, string>(async (keys) => {
      calls += 1;
      return keys.map((key) => `${key}-${calls}`);
    });

    expect(await loader.load("event-1")).toBe("event-1-1");
    expect(await loader.load("event-1")).toBe("event-1-1");
    expect(calls).toBe(1);

    loader.clear("event-1");
    expect(await loader.load("event-1")).toBe("event-1-2");
    expect(calls).toBe(2);
  });

  it("rejects failed loads and allows a retry", async () => {
    let attempt = 0;
    const loader = new DataLoader<string, string>(async (keys) => {
      attempt += 1;
      if (attempt === 1) return [new Error("temporary failure")];
      return keys.map((key) => key);
    });

    await expect(loader.load("event-1")).rejects.toThrow("temporary failure");
    expect(await loader.load("event-1")).toBe("event-1");
    expect(attempt).toBe(2);
  });
});
