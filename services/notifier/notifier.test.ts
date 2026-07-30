import { describe, expect, it, jest, beforeEach } from "@jest/globals";
import { Notifier, matches, render, validateTemplate, getAvailableTemplateVariables, DEFAULT_TEMPLATES, AuditEvent, Rule } from "./notifier";
import { TemplateEngine, buildTemplateContext } from "./template-engine";

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

describe("render() — template engine", () => {
  const event: AuditEvent = {
    index: 42,
    timestamp: 1_700_000_000,
    event_type: "payment",
    submitter: "GABCDEF123",
    metadata: '{"amount":100,"currency":"USD","items":["a","b"]}',
  };

  it("renders simple {{variable}} placeholders", () => {
    const result = render("Event: {{event_type}} #{{index}}", event);
    expect(result).toBe("Event: payment #42");
  });

  it("renders with chain info context", () => {
    const chain = { id: "stellar", network: "testnet", name: "Stellar" };
    const result = render("Chain: {{chain.name}} ({{chain.network}})", event, chain);
    expect(result).toBe("Chain: Stellar (testnet)");
  });

  it("renders with links context", () => {
    const links = { explorerUrl: "https://stellar.expert/explorer/testnet" };
    const result = render("Explorer: {{links.explorer_url}}", event, undefined, links);
    expect(result).toBe("Explorer: https://stellar.expert/explorer/testnet");
  });

  it("renders formatted timestamps", () => {
    const result = render("Date: {{formatted.date}}", event);
    expect(result).toContain("Nov");
    expect(result).toContain("2023");
  });

  it("renders blank for unknown variables", () => {
    const result = render("{{unknown_var}}", event);
    expect(result).toBe("");
  });

  it("handles empty template", () => {
    expect(render("", event)).toBe("");
  });

  it("handles template with no placeholders", () => {
    expect(render("plain text", event)).toBe("plain text");
  });
});

describe("TemplateEngine — helpers", () => {
  const event: AuditEvent = {
    index: 1,
    timestamp: 1_700_000_000,
    event_type: "payment",
    submitter: "gabc123",
    metadata: '{"role":"admin"}',
  };

  it("uppercase helper", () => {
    const result = render("{{uppercase submitter}}", event);
    expect(result).toBe("GABC123");
  });

  it("lowercase helper", () => {
    const result = render("{{lowercase event_type}}", event);
    expect(result).toBe("payment");
  });

  it("capitalize helper", () => {
    const result = render("{{capitalize event_type}}", event);
    expect(result).toBe("Payment");
  });

  it("date helper formats with timestamp argument", () => {
    const result = render("{{date timestamp}}", event);
    expect(result).toContain("Nov");
  });

  it("datetime helper formats with timestamp argument", () => {
    const result = render("{{datetime timestamp}}", event);
    expect(result).toContain("2023");
  });

  it("truncate helper truncates long strings", () => {
    const longEvent = { ...event, metadata: "a".repeat(100) };
    const result = render("{{truncate metadata 10}}", longEvent);
    expect(result).toBe("aaaaaaaaaa...");
  });

  it("truncate uses default length of 50", () => {
    const longEvent = { ...event, metadata: "a".repeat(100) };
    const result = render("{{truncate metadata}}", longEvent);
    expect(result.length).toBe(53);
    expect(result.endsWith("...")).toBe(true);
  });

  it("json helper pretty-prints JSON strings", () => {
    const result = render("{{json metadata}}", event);
    expect(result).toContain('"role"');
    expect(result).toContain('"admin"');
  });

  it("default helper provides fallback when variable is falsy", () => {
    const tpl = "{{#if unknown_var}}{{default unknown_var 'fallback'}}{{else}}fallback{{/if}}";
    const result = render(tpl, event);
    expect(result).toBe("fallback");
  });

  it("default helper passes through truthy value", () => {
    const result = render("{{default event_type 'fallback'}}", event);
    expect(result).toBe("payment");
  });

  it("pipe syntax: {{variable | helper}}", () => {
    const result = render("{{submitter | uppercase}}", event);
    expect(result).toBe("GABC123");
  });
});

describe("TemplateEngine — conditionals", () => {
  const event: AuditEvent = {
    index: 1,
    timestamp: 1_700_000_000,
    event_type: "payment",
    submitter: "GABCDEF123",
    metadata: "high_value",
  };

  it("{{#if variable}} renders body when truthy", () => {
    const tpl = "{{#if event_type}}has type{{/if}}";
    expect(render(tpl, event)).toBe("has type");
  });

  it("{{#if variable}} skips body when falsy", () => {
    const tpl = "{{#if unknown_var}}shown{{/if}}";
    expect(render(tpl, event)).toBe("");
  });

  it("{{#unless variable}} renders body when falsy", () => {
    const tpl = "{{#unless unknown_var}}shown{{/unless}}";
    expect(render(tpl, event)).toBe("shown");
  });

  it("{{#unless variable}} skips body when truthy", () => {
    const tpl = "{{#unless event_type}}hidden{{/unless}}";
    expect(render(tpl, event)).toBe("");
  });

  it("{{#if}}...{{else}}...{{/if}} works", () => {
    const tpl = "{{#if event_type}}yes{{else}}no{{/if}}";
    expect(render(tpl, event)).toBe("yes");
  });

  it("{{#if}}...{{else}}...{{/if}} renders else branch when falsy", () => {
    const tpl = "{{#if unknown_var}}yes{{else}}no{{/if}}";
    expect(render(tpl, event)).toBe("no");
  });
});

describe("TemplateEngine — each loops", () => {
  const event: AuditEvent = {
    index: 1,
    timestamp: 1_700_000_000,
    event_type: "payment",
    submitter: "GABCDEF123",
    metadata: '{"tags":["urgent","high","audit"],"count":3}',
  };

  it("iterates over arrays", () => {
    const tpl = "{{#each metadata_json.tags}}{{this}} {{/each}}";
    const result = render(tpl, event);
    expect(result).toBe("urgent high audit ");
  });

  it("iterates over object keys", () => {
    const tpl = "{{#each metadata_json}}{{@key}}: {{this}}; {{/each}}";
    const result = render(tpl, event);
    expect(result).toContain("tags:");
    expect(result).toContain("count: 3");
  });
});

describe("TemplateEngine — validation", () => {
  it("validates a correct template", () => {
    const result = validateTemplate("{{event_type}} - {{index}}");
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it("detects unclosed block tags", () => {
    const result = validateTemplate("{{#if event_type}}content");
    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(0);
  });

  it("detects mismatched closing tags", () => {
    const result = validateTemplate("{{#if event_type}}{{/each}}");
    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(0);
  });
});

describe("getAvailableTemplateVariables()", () => {
  it("returns variable definitions", () => {
    const vars = getAvailableTemplateVariables();
    expect(vars.length).toBeGreaterThan(0);
    expect(vars.find((v) => v.name === "event_type")).toBeDefined();
    expect(vars.find((v) => v.name === "chain.id")).toBeDefined();
    expect(vars.find((v) => v.name === "links.explorer_url")).toBeDefined();
    expect(vars.find((v) => v.name === "formatted.datetime")).toBeDefined();
    expect(vars.find((v) => v.name === "metadata_json")).toBeDefined();
    vars.forEach((v) => {
      expect(v.name).toBeDefined();
      expect(v.description).toBeDefined();
      expect(v.example).toBeDefined();
    });
  });
});

describe("TemplateEngine — custom helpers", () => {
  it("supports registered custom helpers", () => {
    const engine = new TemplateEngine();
    engine.registerHelper("stars", (s: string) => `***${s}***`);
    const context = buildTemplateContext(baseEvent);
    const result = engine.render("{{stars event_type}}", context);
    expect(result).toBe("***payment***");
  });

  it("supports custom helper with multiple args", () => {
    const engine = new TemplateEngine();
    engine.registerHelper("repeat", (s: string, n?: string) => s.repeat(parseInt(n ?? "1", 10)));
    const context = buildTemplateContext(baseEvent);
    const result = engine.render("{{repeat event_type 3}}", context);
    expect(result).toBe("paymentpaymentpayment");
  });
});

describe("TemplateEngine — backward compatibility", () => {
  it("does not interpret {var} as template syntax", () => {
    const result = render("use {index} as a literal", baseEvent);
    expect(result).toBe("use {index} as a literal");
  });

  it("mixed content renders correctly", () => {
    const tpl = "Event #{{index}}: {{event_type}} by {{submitter}}";
    const result = render(tpl, baseEvent);
    expect(result).toBe("Event #1: payment by GABCDEF123");
  });
});

describe("render() — default templates", () => {
  const event: AuditEvent = {
    index: 5,
    timestamp: 1_700_000_000,
    event_type: "compliance_alert",
    submitter: "GCONTRACT123",
    metadata: "unusual activity detected",
  };

  it("compliance_alert template renders all fields", () => {
    const result = render(DEFAULT_TEMPLATES.compliance_alert, event);
    expect(result).toContain("compliance_alert");
    expect(result).toContain("GCONTRACT123");
    expect(result).toContain("unusual activity detected");
  });

  it("detailed template includes chain info", () => {
    const chain = { id: "stellar", network: "testnet", name: "Stellar" };
    const result = render(DEFAULT_TEMPLATES.detailed, event, chain);
    expect(result).toContain("Stellar");
    expect(result).toContain("testnet");
  });
});
