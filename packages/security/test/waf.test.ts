import { describe, expect, it, beforeEach } from "vitest";
import express from "express";
import request from "supertest";
import { WafRuleEngine } from "../src/waf/ruleEngine";
import { assessBot, clearBotHistory } from "../src/waf/botDetection";
import { ddosProtection, wafAdminRouter, clearWafEvents } from "../src/waf/middleware";
import { toAwsWafRuleStatement, exportRuleGroupForAwsWaf } from "../src/waf/awsShield";
import { isFromCloudflare, resolveClientIp } from "../src/waf/cloudflare";

describe("WafRuleEngine", () => {
  it("blocks a SQL injection payload in the query string", () => {
    const engine = new WafRuleEngine();
    const req = { path: "/v1/events", query: { filter: "1' OR '1'='1" }, body: {}, headers: {} } as any;
    const result = engine.evaluate(req);
    expect(result.action).toBe("block");
    expect(result.matches[0].rule.id).toBe("sqli-1");
  });

  it("blocks an XSS payload in the request body", () => {
    const engine = new WafRuleEngine();
    const req = { path: "/v1/events", query: {}, body: { comment: "<script>alert(1)</script>" }, headers: {} } as any;
    const result = engine.evaluate(req);
    expect(result.action).toBe("block");
    expect(result.matches[0].rule.id).toBe("xss-1");
  });

  it("blocks path traversal", () => {
    const engine = new WafRuleEngine();
    const req = { path: "/v1/export/../../etc/passwd", query: {}, body: {}, headers: {} } as any;
    const result = engine.evaluate(req);
    expect(result.action).toBe("block");
    expect(result.matches[0].rule.id).toBe("traversal-1");
  });

  it("allows a benign request", () => {
    const engine = new WafRuleEngine();
    const req = { path: "/v1/events", query: { limit: "50" }, body: {}, headers: {} } as any;
    const result = engine.evaluate(req);
    expect(result.action).toBe("allow");
  });

  it("supports adding, disabling, and removing custom rules", () => {
    const engine = new WafRuleEngine();
    engine.addRule({
      id: "custom-1",
      name: "Block a specific header value",
      target: "headers",
      pattern: /malicious-tool\/1\.0/i,
      action: "block",
      severity: "medium",
      enabled: true,
    });
    const req = { path: "/", query: {}, body: {}, headers: { "user-agent": "malicious-tool/1.0" } } as any;
    expect(engine.evaluate(req).action).toBe("block");

    engine.setEnabled("custom-1", false);
    expect(engine.evaluate(req).action).toBe("allow");

    expect(engine.removeRule("custom-1")).toBe(true);
    expect(() => engine.removeRule("sqli-1")).toThrow();
  });
});

describe("Bot detection", () => {
  beforeEach(() => clearBotHistory());

  it("flags a well-known scripting tool user agent", () => {
    const req = { headers: { "user-agent": "python-requests/2.31.0", accept: "*/*" } } as any;
    const result = assessBot(req, "client-a");
    expect(result.classification).not.toBe("human");
  });

  it("flags missing User-Agent and Accept headers", () => {
    const req = { headers: {} } as any;
    const result = assessBot(req, "client-b");
    expect(result.score).toBeGreaterThan(0);
  });

  it("flags machine-speed request cadence", () => {
    const req = { headers: { "user-agent": "Mozilla/5.0", accept: "text/html", "accept-language": "en" } } as any;
    let last;
    for (let i = 0; i < 10; i++) {
      last = assessBot(req, "client-c", 1000 + i * 10); // 10ms apart
    }
    expect(last!.classification).toBe("bot");
  });

  it("does not penalize a normal browser-like request", () => {
    const req = {
      headers: { "user-agent": "Mozilla/5.0 (X11; Linux x86_64)", accept: "text/html", "accept-language": "en-US" },
    } as any;
    const result = assessBot(req, "client-d", 1);
    expect(result.classification).toBe("human");
  });
});

describe("ddosProtection middleware", () => {
  beforeEach(() => {
    clearBotHistory();
    clearWafEvents();
  });

  it("returns 403 for a request matching a blocking rule", async () => {
    const engine = new WafRuleEngine();
    const app = express();
    app.use(express.json());
    app.use(ddosProtection({ ruleEngine: engine }));
    app.get("/v1/events", (_req, res) => res.json({ ok: true }));

    const res = await request(app).get("/v1/events").query({ q: "'; DROP TABLE users; --" });
    expect(res.status).toBe(403);
  });

  it("passes through and tags a clean request with a bot score header", async () => {
    const engine = new WafRuleEngine();
    const app = express();
    app.use(ddosProtection({ ruleEngine: engine, botBlockThreshold: 95 }));
    app.get("/", (_req, res) => res.send("ok"));

    const res = await request(app).get("/").set("User-Agent", "Mozilla/5.0").set("Accept", "text/html").set("Accept-Language", "en");
    expect(res.status).toBe(200);
    expect(res.headers["x-bot-score"]).toBeDefined();
  });

  it("blocks obvious scripted bot traffic once the score threshold is crossed", async () => {
    const engine = new WafRuleEngine();
    const app = express();
    app.use(ddosProtection({ ruleEngine: engine, botBlockThreshold: 40 }));
    app.get("/", (_req, res) => res.send("ok"));

    const res = await request(app).get("/").set("User-Agent", "sqlmap/1.6");
    expect(res.status).toBe(403);
  });

  it("supports custom rule management via the admin router", async () => {
    const engine = new WafRuleEngine();
    const app = express();
    app.use(express.json());
    app.use("/admin/waf", wafAdminRouter(engine));

    const create = await request(app).post("/admin/waf/rules").send({
      id: "block-ua",
      name: "Block internal tool UA",
      target: "headers",
      pattern: "internal-scanner",
      action: "block",
      severity: "low",
    });
    expect(create.status).toBe(201);

    const list = await request(app).get("/admin/waf/rules");
    expect(list.body.data.some((r: any) => r.id === "block-ua")).toBe(true);

    const disable = await request(app).patch("/admin/waf/rules/block-ua").send({ enabled: false });
    expect(disable.status).toBe(200);

    const remove = await request(app).delete("/admin/waf/rules/block-ua");
    expect(remove.status).toBe(204);
  });

  it("exports custom rules in AWS WAFv2 rule-group shape", () => {
    const engine = new WafRuleEngine();
    const exported = exportRuleGroupForAwsWaf(engine.listRules());
    expect(exported.Name).toBe("audit-ledger-custom-rules");
    expect((exported.Rules as unknown[]).length).toBeGreaterThan(0);

    const rule = toAwsWafRuleStatement(engine.listRules()[0], 1);
    expect(rule.Priority).toBe(1);
    expect((rule.Statement as any).RegexMatchStatement.RegexString).toBeDefined();
  });
});

describe("Cloudflare integration", () => {
  it("only trusts CF-Connecting-IP when the peer is a real Cloudflare IP", () => {
    expect(isFromCloudflare("104.16.1.1")).toBe(true);
    expect(isFromCloudflare("8.8.8.8")).toBe(false);

    const spoofed = { socket: { remoteAddress: "8.8.8.8" }, headers: { "cf-connecting-ip": "6.6.6.6" } } as any;
    expect(resolveClientIp(spoofed)).toBe("8.8.8.8");

    const legit = { socket: { remoteAddress: "104.16.1.1" }, headers: { "cf-connecting-ip": "9.9.9.9" } } as any;
    expect(resolveClientIp(legit)).toBe("9.9.9.9");
  });
});
