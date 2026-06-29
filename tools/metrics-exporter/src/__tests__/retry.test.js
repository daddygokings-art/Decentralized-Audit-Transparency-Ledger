"use strict";

// Mock prom-client and @stellar/stellar-sdk before requiring the module
jest.mock("prom-client", () => {
  const gaugeSet = jest.fn();
  const counterInc = jest.fn();
  const Gauge = jest.fn(() => ({ set: gaugeSet }));
  const Counter = jest.fn(() => ({ inc: counterInc }));
  const Registry = jest.fn(() => ({
    contentType: "text/plain",
    metrics: jest.fn().mockResolvedValue(""),
  }));
  return {
    Gauge,
    Counter,
    Registry,
    collectDefaultMetrics: jest.fn(),
    _gaugeSet: gaugeSet,
    _counterInc: counterInc,
  };
});

jest.mock("@stellar/stellar-sdk", () => ({
  SorobanRpc: { Server: jest.fn(() => ({})), Api: { isSimulationError: jest.fn(() => false) } },
  Contract: jest.fn(() => ({ call: jest.fn() })),
  Networks: { TESTNET: "Test SDF Network ; September 2015", PUBLIC: "Public Global Stellar Network ; September 2015" },
  xdr: { ScVal: { scvSymbol: jest.fn() } },
  TransactionBuilder: jest.fn(() => ({ addOperation: jest.fn().mockReturnThis(), setTimeout: jest.fn().mockReturnThis(), build: jest.fn() })),
  Account: jest.fn(),
}));

// Must require AFTER mocks
process.env.CONTRACT_ID = "CTEST000000000000000000000000000000000000000000000000000000";
const {
  scrapeWithRetry,
  BACKOFF_BASE_MS,
  BACKOFF_MAX_MS,
  FAILURE_THRESHOLD,
} = require("../src/index");

const promClient = require("prom-client");

describe("scrapeWithRetry", () => {
  let sleepCalls;
  let noopSleep;

  beforeEach(() => {
    sleepCalls = [];
    noopSleep = (ms) => { sleepCalls.push(ms); return Promise.resolve(); };
    jest.spyOn(console, "error").mockImplementation(() => {});
    promClient._gaugeSet.mockClear();
    promClient._counterInc.mockClear();
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  test("resets consecutiveFailures and sets scrapeError=0 on success", async () => {
    const state = { consecutiveFailures: 5 };
    const fn = jest.fn().mockResolvedValue(undefined);

    await scrapeWithRetry(fn, state, noopSleep);

    expect(state.consecutiveFailures).toBe(0);
    // scrapeError.set(0) should have been called
    expect(promClient._gaugeSet).toHaveBeenCalledWith(0);
    expect(sleepCalls).toHaveLength(0);
  });

  test("backs off with exponential delays: 1s, 2s, 4s…", async () => {
    const state = { consecutiveFailures: 0 };
    let callCount = 0;
    // Fail 3 times then succeed
    const fn = jest.fn().mockImplementation(async () => {
      if (callCount++ < 3) throw new Error("RPC error");
    });

    await scrapeWithRetry(fn, state, noopSleep);

    expect(sleepCalls).toEqual([
      BACKOFF_BASE_MS * 1,  // 1s after failure 1
      BACKOFF_BASE_MS * 2,  // 2s after failure 2
      BACKOFF_BASE_MS * 4,  // 4s after failure 3
    ]);
    expect(state.consecutiveFailures).toBe(0);
  });

  test("caps backoff at BACKOFF_MAX_MS (60s)", async () => {
    const state = { consecutiveFailures: 0 };
    let callCount = 0;
    // Fail enough times to exceed the cap
    const fn = jest.fn().mockImplementation(async () => {
      if (callCount++ < 8) throw new Error("RPC error");
    });

    await scrapeWithRetry(fn, state, noopSleep);

    for (let i = 0; i < sleepCalls.length; i++) {
      expect(sleepCalls[i]).toBeLessThanOrEqual(BACKOFF_MAX_MS);
    }
  });

  test("sets scrapeError=1 after FAILURE_THRESHOLD consecutive failures", async () => {
    const state = { consecutiveFailures: 0 };
    let callCount = 0;
    // Fail exactly FAILURE_THRESHOLD times then succeed
    const fn = jest.fn().mockImplementation(async () => {
      if (callCount++ < FAILURE_THRESHOLD) throw new Error("RPC error");
    });

    await scrapeWithRetry(fn, state, noopSleep);

    // scrapeError.set(1) must have been called
    expect(promClient._gaugeSet).toHaveBeenCalledWith(1);
    // And then scrapeError.set(0) on final success
    expect(promClient._gaugeSet).toHaveBeenCalledWith(0);
  });

  test("continues retrying after FAILURE_THRESHOLD — does not exit", async () => {
    const state = { consecutiveFailures: 0 };
    let callCount = 0;
    // Fail well past threshold then succeed
    const fn = jest.fn().mockImplementation(async () => {
      if (callCount++ < FAILURE_THRESHOLD + 3) throw new Error("RPC error");
    });

    await scrapeWithRetry(fn, state, noopSleep);

    expect(fn).toHaveBeenCalledTimes(FAILURE_THRESHOLD + 4);
    expect(state.consecutiveFailures).toBe(0);
  });

  test("increments errorCount on each failure", async () => {
    const state = { consecutiveFailures: 0 };
    let callCount = 0;
    const fn = jest.fn().mockImplementation(async () => {
      if (callCount++ < 2) throw new Error("RPC error");
    });

    await scrapeWithRetry(fn, state, noopSleep);

    expect(promClient._counterInc).toHaveBeenCalledTimes(2);
  });
});
