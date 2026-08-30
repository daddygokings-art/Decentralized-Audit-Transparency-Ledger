import { describe, expect, it, jest } from "@jest/globals";
import {
  AlertmanagerAlert,
  IncidentResponseConfig,
  loadConfig,
  renderMessage,
  createServer,
  buildDefaultHandler,
  DEFAULT_TEMPLATE,
} from "./server";

const baseAlert: AlertmanagerAlert = {
  status: "firing",
  labels: {
    alertname: "AuditLedgerFalcoPrivilegeEscalation",
    severity: "critical",
    pod: "relayer-abc123",
    namespace: "audit-ledger",
  },
};

type Handler = (alert: AlertmanagerAlert, path: string, cfg: IncidentResponseConfig) => Promise<void>;

const testConfig = (port: number): IncidentResponseConfig => ({
  port,
  forwardMethod: "POST",
  forwardHeaders: { "Content-Type": "application/json" },
  template: DEFAULT_TEMPLATE,
});

describe("loadConfig", () => {
  it("uses defaults when no env is provided", () => {
    const cfg = loadConfig({});
    expect(cfg.port).toBe(8080);
    expect(cfg.forwardMethod).toBe("POST");
    expect(cfg.template).toBe(DEFAULT_TEMPLATE);
  });

  it("reads forwarding configuration from env", () => {
    const cfg = loadConfig({ INCIDENT_FORWARD_URL: "https://hooks.example.com/x", INCIDENT_PORT: "9000" });
    expect(cfg.port).toBe(9000);
    expect(cfg.forwardUrl).toBe("https://hooks.example.com/x");
  });
});

describe("renderMessage", () => {
  it("substitutes alert label placeholders", () => {
    const msg = renderMessage(DEFAULT_TEMPLATE, baseAlert);
    expect(msg).toContain("relayer-abc123");
    expect(msg).toContain("audit-ledger");
    expect(msg).toContain("critical");
  });

  it("leaves unknown placeholders untouched", () => {
    const msg = renderMessage("{{unknown}}", baseAlert);
    expect(msg).toBe("{{unknown}}");
  });
});

describe("createServer", () => {
  it("answers GET /healthz with ok", async () => {
    const handler = jest.fn<Handler>().mockResolvedValue(undefined);
    const { server, close } = createServer(handler, testConfig(0));
    const addr = server.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    const res = await fetch(`http://127.0.0.1:${port}/healthz`);
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("ok");
    close();
  });

  it("returns 404 for unknown routes", async () => {
    const handler = jest.fn<Handler>().mockResolvedValue(undefined);
    const { server, close } = createServer(handler, testConfig(0));
    const addr = server.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    const res = await fetch(`http://127.0.0.1:${port}/nope`);
    expect(res.status).toBe(404);
    close();
  });

  it("invokes the handler for POST /incidents", async () => {
    const handler = jest.fn<Handler>().mockResolvedValue(undefined);
    const { server, close } = createServer(handler, testConfig(0));
    const addr = server.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    const res = await fetch(`http://127.0.0.1:${port}/incidents`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alerts: [baseAlert] }),
    });
    expect(res.status).toBe(200);
    expect(handler).toHaveBeenCalledTimes(1);
    close();
  });

  it("returns 400 for an empty alerts payload", async () => {
    const handler = jest.fn<Handler>().mockResolvedValue(undefined);
    const { server, close } = createServer(handler, testConfig(0));
    const addr = server.address();
    const port = typeof addr === "object" && addr ? addr.port : 0;
    const res = await fetch(`http://127.0.0.1:${port}/incidents`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alerts: [] }),
    });
    expect(res.status).toBe(400);
    expect(handler).not.toHaveBeenCalled();
    close();
  });
});
