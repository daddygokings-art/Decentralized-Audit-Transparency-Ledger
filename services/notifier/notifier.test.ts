import { describe, expect, it, jest, beforeEach } from "@jest/globals";
import { Notifier, matches, AuditEvent, Rule } from "./notifier";

const baseEvent: AuditEvent = {
  index: 1,
  timestamp: 1_700_000_000,
  event_type: "payment",
  submitter: "GABCDEF123",
  metadata: "tx: 42 tokens",
};

const baseRule: Rule = {
  name: "test-rule",
  event_type: "*",
  channel: "webhook",
  template: "event {index}",
};

describe("matches()", () => {
  it("matches wildcard event_type to any event", () => {
    const rule: Rule = { ...baseRule, event_type: "*" };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("matches exact event_type", () => {
    const rule: Rule = { ...baseRule, event_type: "payment" };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("rejects mismatched event_type", () => {
    const rule: Rule = { ...baseRule, event_type: "compliance" };
    expect(matches(rule, baseEvent)).toBe(false);
  });

  it("matches with submitter_contains filter", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { submitter_contains: "GABC" },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("rejects when submitter_does_not_contain", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { submitter_contains: "XYZ" },
    };
    expect(matches(rule, baseEvent)).toBe(false);
  });

  it("matches with metadata_contains filter", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { metadata_contains: "tokens" },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("rejects when metadata_does_not_contain", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { metadata_contains: "alert" },
    };
    expect(matches(rule, baseEvent)).toBe(false);
  });

  it("matches when metadata meets min_metadata_size", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { min_metadata_size: 5 },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("rejects when metadata is below min_metadata_size", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { min_metadata_size: 100 },
    };
    expect(matches(rule, baseEvent)).toBe(false);
  });

  it("matches when min_metadata_size is 0", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { min_metadata_size: 0 },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("applies submitter_contains and metadata_contains together", () => {
    const rule: Rule = {
      ...baseRule,
      event_type: "payment",
      filters: { submitter_contains: "GABC", metadata_contains: "42" },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("rejects when combined filters partially fail", () => {
    const rule: Rule = {
      ...baseRule,
      filters: { submitter_contains: "GABC", metadata_contains: "alert" },
    };
    expect(matches(rule, baseEvent)).toBe(false);
  });

  it("applies all three filter types together", () => {
    const rule: Rule = {
      ...baseRule,
      filters: {
        submitter_contains: "GABC",
        metadata_contains: "42",
        min_metadata_size: 5,
      },
    };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("matches rule with no filters", () => {
    const rule: Rule = { ...baseRule, event_type: "payment" };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("handles undefined filters gracefully", () => {
    const rule: Rule = { ...baseRule, filters: undefined };
    expect(matches(rule, baseEvent)).toBe(true);
  });

  it("handles empty filters object", () => {
    const rule: Rule = { ...baseRule, filters: {} };
    expect(matches(rule, baseEvent)).toBe(true);
  });
});

describe("Notifier — rate limit enforcement", () => {
  beforeEach(() => {
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it("allows notifications within rate limit", async () => {
    const notifier = new Notifier({
      wsUrl: "ws://localhost:9999",
      channels: {},
      rules: [baseRule],
      rateLimitPerMinute: 5,
    });

    const emitSpy = jest.spyOn(notifier, "emit");

    for (let i = 0; i < 5; i++) {
      await notifier.processEvent({ ...baseEvent, index: i });
    }

    expect(emitSpy).toHaveBeenCalledTimes(5);
    emitSpy.mockRestore();
  });

  it("drops notifications after rate limit is exceeded", async () => {
    const notifier = new Notifier({
      wsUrl: "ws://localhost:9999",
      channels: {},
      rules: [baseRule],
      rateLimitPerMinute: 3,
    });

    const emitSpy = jest.spyOn(notifier, "emit");

    for (let i = 0; i < 6; i++) {
      await notifier.processEvent({ ...baseEvent, index: i });
    }

    const sent = emitSpy.mock.calls.filter(([event]) => event === "notification_sent").length;
    expect(sent).toBe(3);
    emitSpy.mockRestore();
  });

  it("resets rate limit after one minute", async () => {
    const notifier = new Notifier({
      wsUrl: "ws://localhost:9999",
      channels: {},
      rules: [baseRule],
      rateLimitPerMinute: 2,
    });

    const emitSpy = jest.spyOn(notifier, "emit");

    await notifier.processEvent(baseEvent);
    await notifier.processEvent(baseEvent);
    // third one should be dropped
    await notifier.processEvent(baseEvent);

    let sent = emitSpy.mock.calls.filter(([event]) => event === "notification_sent").length;
    expect(sent).toBe(2);

    // advance past the 1-minute window
    jest.advanceTimersByTime(60_001);
    await notifier.processEvent(baseEvent);

    sent = emitSpy.mock.calls.filter(([event]) => event === "notification_sent").length;
    expect(sent).toBe(3);

    emitSpy.mockRestore();
  });
});
