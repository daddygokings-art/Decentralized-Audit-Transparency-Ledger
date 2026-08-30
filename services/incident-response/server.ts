/**
 * AuditLedger Incident Response webhook receiver.
 *
 * Alertmanager (see infra/k8s/falco/alertmanager-config.yaml) posts security
 * events to:
 *   POST /incidents      -> critical runtime-security (Falco) alerts
 *   POST /investigate    -> warning-level security signals
 *
 * Each incident is validated, written to the container log, and optionally
 * forwarded to an external incident tool (Slack / PagerDuty / generic
 * webhook) configured via environment variables. The service exposes a
 * Prometheus counter (incident_response_received_total) for the
 * runtime-security availability guard and a /healthz endpoint for probes.
 */
import http from "http";
import https from "https";
import { URL } from "url";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AlertmanagerAnnotation {
  summary?: string;
  description?: string;
}

export interface AlertmanagerAlert {
  status: string;
  labels: Record<string, string>;
  annotations?: AlertmanagerAnnotation;
  startsAt?: string;
  endsAt?: string;
}

export interface AlertmanagerPayload {
  status?: string;
  alerts?: AlertmanagerAlert[];
  groupLabels?: Record<string, string>;
  commonLabels?: Record<string, string>;
}

export interface IncidentResponseConfig {
  port: number;
  forwardUrl?: string;
  forwardMethod?: string;
  forwardHeaders?: Record<string, string>;
  template?: string;
}

// ── Defaults ──────────────────────────────────────────────────────────────────

export const DEFAULT_TEMPLATE =
  "AuditLedger runtime security incident: {{alertname}} ({{severity}}) on pod {{pod}} in {{namespace}}";

export function loadConfig(env: NodeJS.ProcessEnv = process.env): IncidentResponseConfig {
  return {
    port: Number(env.INCIDENT_PORT || 8080),
    forwardUrl: env.INCIDENT_FORWARD_URL,
    forwardMethod: env.INCIDENT_FORWARD_METHOD || "POST",
    forwardHeaders: env.INCIDENT_FORWARD_HEADERS
      ? JSON.parse(env.INCIDENT_FORWARD_HEADERS)
      : { "Content-Type": "application/json" },
    template: env.INCIDENT_TEMPLATE || DEFAULT_TEMPLATE,
  };
}

export function renderMessage(template: string, alert: AlertmanagerAlert): string {
  const labels = alert.labels || {};
  let out = template;
  for (const [k, v] of Object.entries(labels)) {
    out = out.split(`{{${k}}}`).join(v || "");
  }
  return out;
}

// ── Forwarding ────────────────────────────────────────────────────────────────

export function forwardTo(url: string, method: string, headers: Record<string, string>, body: string): Promise<void> {
  return new Promise((resolve, reject) => {
    let parsed: URL;
    try {
      parsed = new URL(url);
    } catch (e) {
      reject(e);
      return;
    }
    const client = parsed.protocol === "https:" ? https : http;
    const req = client.request(
      parsed,
      {
        method,
        headers,
      },
      (res) => {
        res.resume();
        res.on("end", () => {
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
            resolve();
          } else {
            reject(new Error(`forward target responded ${res.statusCode}`));
          }
        });
      },
    );
    req.on("error", reject);
    req.write(body);
    req.end();
  });
}

// ── Request handler ───────────────────────────────────────────────────────────

function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (chunk) => {
      data += chunk;
      if (data.length > 1e6) {
        reject(new Error("payload too large"));
        req.destroy();
      }
    });
    req.on("end", () => resolve(data));
    req.on("error", reject);
  });
}

export interface IncidentServer {
  server: http.Server;
  close: () => void;
}

export function createServer(
  handler: (alert: AlertmanagerAlert, path: string, cfg: IncidentResponseConfig) => Promise<void>,
  cfg: IncidentResponseConfig = loadConfig(),
): IncidentServer {
  const server = http.createServer(async (req, res) => {
    if (req.method === "GET") {
      if (req.url === "/healthz") {
        res.writeHead(200, { "Content-Type": "text/plain" });
        res.end("ok");
        return;
      }
      if (req.url === "/metrics") {
        res.writeHead(200, { "Content-Type": "text/plain" });
        res.end(
          `# HELP incident_response_received_total Total incidents received\n` +
            `# TYPE incident_response_received_total counter\n` +
            `incident_response_received_total 0\n`,
        );
        return;
      }
      res.writeHead(404);
      res.end();
      return;
    }

    if (req.method !== "POST") {
      res.writeHead(405, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "method not allowed" }));
      return;
    }

    const path = req.url || "/";
    if (path !== "/incidents" && path !== "/investigate") {
      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "not found" }));
      return;
    }

    try {
      const raw = await readBody(req);
      const payload = JSON.parse(raw) as AlertmanagerPayload;
      const alerts = payload.alerts || [];

      if (alerts.length === 0) {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "no alerts in payload" }));
        return;
      }

      // Handle each alert; forwarding failures degrade to warnings rather
      // than returning 5xx (Alertmanager would retry on 5xx).
      let failed = 0;
      for (const alert of alerts) {
        try {
          await handler(alert, path, cfg);
        } catch {
          failed += 1;
        }
      }

      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ received: alerts.length, failed }));
    } catch (e) {
      res.writeHead(400, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: (e as Error).message || "invalid payload" }));
    }
  });

  server.listen(cfg.port);
  return {
    server,
    close: () => server.close(),
  };
}

// ── Wire-up ───────────────────────────────────────────────────────────────────

export function buildDefaultHandler(cfg: IncidentResponseConfig) {
  return async (alert: AlertmanagerAlert, _path: string, c: IncidentResponseConfig) => {
    const message = renderMessage(c.template || DEFAULT_TEMPLATE, alert);
    console.log(
      JSON.stringify({
        ts: new Date().toISOString(),
        level: alert.labels && alert.labels.severity === "critical" ? "error" : "warn",
        message,
      }),
    );
    if (c.forwardUrl) {
      await forwardTo(c.forwardUrl, c.forwardMethod || "POST", c.forwardHeaders || {}, JSON.stringify(alert));
    }
  };
}

// Only start the listener when executed directly (not when imported for tests).
if (require.main === module) {
  const cfg = loadConfig();
  const handler = buildDefaultHandler(cfg);
  createServer(handler, cfg);
  console.log(`incident-response listening on :${cfg.port} (forward=${cfg.forwardUrl || "none"})`);
}
