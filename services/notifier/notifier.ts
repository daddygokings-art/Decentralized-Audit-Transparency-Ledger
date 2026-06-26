/**
 * Audit Ledger Notification Service (#78)
 *
 * Connects to the WebSocket event stream, matches events against user-defined
 * rules, and dispatches notifications via Email, Slack, Telegram, or Webhook.
 */

import { EventEmitter } from "events";
import https from "https";
import http from "http";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AuditEvent {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
}

export interface RuleFilters {
  submitter_contains?: string;
  metadata_contains?: string;
  min_metadata_size?: number;
}

export type ChannelType = "email" | "slack" | "telegram" | "webhook";

export interface Rule {
  name: string;
  event_type: string;
  filters?: RuleFilters;
  channel: ChannelType;
  template: string; // supports {index}, {event_type}, {submitter}, {metadata}, {timestamp}
}

export interface ChannelConfig {
  email?: {
    host: string;
    port: number;
    secure: boolean;
    auth: { user: string; pass: string };
    from: string;
    to: string;
  };
  slack?: { webhookUrl: string };
  telegram?: { botToken: string; chatId: string };
  webhook?: { url: string; method?: string; headers?: Record<string, string> };
}

export interface NotifierConfig {
  wsUrl: string;
  channels: ChannelConfig;
  rules: Rule[];
  rateLimitPerMinute: number;
}

// ── Template rendering ────────────────────────────────────────────────────────

function render(template: string, event: AuditEvent): string {
  return template
    .replace(/\{index\}/g, String(event.index))
    .replace(/\{event_type\}/g, event.event_type)
    .replace(/\{submitter\}/g, event.submitter)
    .replace(/\{metadata\}/g, event.metadata)
    .replace(/\{timestamp\}/g, new Date(event.timestamp * 1000).toISOString());
}

// ── Rule matching ─────────────────────────────────────────────────────────────

function matches(rule: Rule, event: AuditEvent): boolean {
  if (rule.event_type !== "*" && rule.event_type !== event.event_type) return false;
  const f = rule.filters ?? {};
  if (f.submitter_contains && !event.submitter.includes(f.submitter_contains)) return false;
  if (f.metadata_contains && !event.metadata.includes(f.metadata_contains)) return false;
  if (f.min_metadata_size !== undefined && event.metadata.length < f.min_metadata_size) return false;
  return true;
}

// ── HTTP helper ───────────────────────────────────────────────────────────────

function httpPost(url: string, body: string, headers: Record<string, string>): Promise<void> {
  return new Promise((resolve, reject) => {
    const parsed = new URL(url);
    const lib = parsed.protocol === "https:" ? https : http;
    const req = lib.request(
      { hostname: parsed.hostname, port: parsed.port, path: parsed.pathname + parsed.search, method: "POST", headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), ...headers } },
      (res) => { res.resume(); res.on("end", resolve); }
    );
    req.on("error", reject);
    req.write(body);
    req.end();
  });
}

// ── Channel senders ───────────────────────────────────────────────────────────

async function sendSlack(cfg: NonNullable<ChannelConfig["slack"]>, message: string): Promise<void> {
  await httpPost(cfg.webhookUrl, JSON.stringify({ text: message }), {});
}

async function sendTelegram(cfg: NonNullable<ChannelConfig["telegram"]>, message: string): Promise<void> {
  const url = `https://api.telegram.org/bot${cfg.botToken}/sendMessage`;
  await httpPost(url, JSON.stringify({ chat_id: cfg.chatId, text: message }), {});
}

async function sendWebhook(cfg: NonNullable<ChannelConfig["webhook"]>, message: string): Promise<void> {
  await httpPost(cfg.url, JSON.stringify({ message }), cfg.headers ?? {});
}

async function sendEmail(cfg: NonNullable<ChannelConfig["email"]>, message: string): Promise<void> {
  // Nodemailer integration — dynamically imported so the service works without it installed
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const nodemailer = require("nodemailer");
    const transport = nodemailer.createTransport({ host: cfg.host, port: cfg.port, secure: cfg.secure, auth: cfg.auth });
    await transport.sendMail({ from: cfg.from, to: cfg.to, subject: "AuditLedger Alert", text: message });
  } catch {
    console.error("[notifier] email send failed — is nodemailer installed?");
  }
}

// ── Notifier ──────────────────────────────────────────────────────────────────

export class Notifier extends EventEmitter {
  private rules: Rule[];
  private channels: ChannelConfig;
  private rateLimitPerMinute: number;
  private notificationCount = 0;
  private windowStart = Date.now();
  private ws: any = null;

  constructor(private cfg: NotifierConfig) {
    super();
    this.rules = cfg.rules;
    this.channels = cfg.channels;
    this.rateLimitPerMinute = cfg.rateLimitPerMinute;
  }

  // ── Rule management ─────────────────────────────────────────────────────────

  addRule(rule: Rule): void { this.rules.push(rule); }
  removeRule(name: string): void { this.rules = this.rules.filter((r) => r.name !== name); }
  getRules(): Rule[] { return [...this.rules]; }
  updateRule(name: string, patch: Partial<Rule>): void {
    const idx = this.rules.findIndex((r) => r.name === name);
    if (idx !== -1) this.rules[idx] = { ...this.rules[idx], ...patch };
  }

  // ── Rate limiting ───────────────────────────────────────────────────────────

  private checkRateLimit(): boolean {
    const now = Date.now();
    if (now - this.windowStart > 60_000) {
      this.windowStart = now;
      this.notificationCount = 0;
    }
    if (this.notificationCount >= this.rateLimitPerMinute) return false;
    this.notificationCount++;
    return true;
  }

  // ── Dispatch ─────────────────────────────────────────────────────────────────

  private async dispatch(rule: Rule, event: AuditEvent): Promise<void> {
    if (!this.checkRateLimit()) {
      console.warn(`[notifier] rate limit reached — dropping notification for rule "${rule.name}"`);
      return;
    }
    const message = render(rule.template, event);
    try {
      switch (rule.channel) {
        case "slack":
          if (this.channels.slack) await sendSlack(this.channels.slack, message);
          break;
        case "telegram":
          if (this.channels.telegram) await sendTelegram(this.channels.telegram, message);
          break;
        case "webhook":
          if (this.channels.webhook) await sendWebhook(this.channels.webhook, message);
          break;
        case "email":
          if (this.channels.email) await sendEmail(this.channels.email, message);
          break;
      }
      this.emit("notification_sent", { rule: rule.name, channel: rule.channel, event });
    } catch (err) {
      this.emit("notification_error", { rule: rule.name, channel: rule.channel, err });
    }
  }

  // ── Event processing ──────────────────────────────────────────────────────────

  async processEvent(event: AuditEvent): Promise<void> {
    const matched = this.rules.filter((r) => matches(r, event));
    await Promise.all(matched.map((r) => this.dispatch(r, event)));
  }

  // ── WebSocket connection ──────────────────────────────────────────────────────

  connect(): void {
    // Dynamic import of 'ws' so this file compiles without it in test envs
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const WebSocket = require("ws");
    this.ws = new WebSocket(this.cfg.wsUrl);

    this.ws.on("open", () => {
      console.log(`[notifier] connected to ${this.cfg.wsUrl}`);
      this.emit("connected");
    });

    this.ws.on("message", (data: Buffer) => {
      try {
        const event: AuditEvent = JSON.parse(data.toString());
        void this.processEvent(event);
      } catch {
        console.error("[notifier] failed to parse event", data.toString());
      }
    });

    this.ws.on("close", () => {
      console.log("[notifier] disconnected — reconnecting in 5s");
      this.emit("disconnected");
      setTimeout(() => this.connect(), 5000);
    });

    this.ws.on("error", (err: Error) => {
      console.error("[notifier] ws error:", err.message);
      this.emit("error", err);
    });
  }

  disconnect(): void {
    this.ws?.close();
    this.ws = null;
  }
}

// ── Default templates ─────────────────────────────────────────────────────────

export const DEFAULT_TEMPLATES: Record<string, string> = {
  compliance_alert: "⚠️ Compliance event [{event_type}] logged at {timestamp} by {submitter}. Metadata: {metadata}",
  large_transaction: "💰 Large transaction event [{event_type}] at index {index}. Submitter: {submitter}",
  generic: "AuditLedger event [{event_type}] #{index} at {timestamp}",
};

// ── Entry point ───────────────────────────────────────────────────────────────

if (require.main === module) {
  const cfg: NotifierConfig = {
    wsUrl: process.env.WS_URL ?? "ws://localhost:4000/graphql",
    rateLimitPerMinute: parseInt(process.env.RATE_LIMIT ?? "60", 10),
    channels: {
      slack: process.env.SLACK_WEBHOOK ? { webhookUrl: process.env.SLACK_WEBHOOK } : undefined,
      telegram: process.env.TELEGRAM_TOKEN && process.env.TELEGRAM_CHAT
        ? { botToken: process.env.TELEGRAM_TOKEN, chatId: process.env.TELEGRAM_CHAT }
        : undefined,
      webhook: process.env.WEBHOOK_URL ? { url: process.env.WEBHOOK_URL } : undefined,
    },
    rules: [
      { name: "all-events", event_type: "*", channel: "webhook", template: DEFAULT_TEMPLATES.generic },
    ],
  };

  const notifier = new Notifier(cfg);
  notifier.on("notification_sent", ({ rule, channel }) => console.log(`[notifier] sent via ${channel} (rule: ${rule})`));
  notifier.on("notification_error", ({ rule, err }) => console.error(`[notifier] error in rule "${rule}":`, err));
  notifier.connect();
}
