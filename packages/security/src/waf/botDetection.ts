import type { Request } from "express";

export interface BotSignal {
  reason: string;
  weight: number;
}

export interface BotAssessment {
  score: number; // 0 (human) - 100 (certain bot)
  signals: BotSignal[];
  classification: "human" | "suspicious" | "bot";
}

const KNOWN_BOT_UA_SUBSTRINGS = [
  "curl/",
  "wget/",
  "python-requests",
  "python-urllib",
  "scrapy",
  "go-http-client",
  "libwww-perl",
  "httpclient",
  "axios/",
  "node-fetch",
  "postmanruntime",
  "masscan",
  "nikto",
  "sqlmap",
  "nmap",
];

const KNOWN_GOOD_CRAWLER_UA_SUBSTRINGS = ["googlebot", "bingbot", "slurp", "duckduckbot"];

interface RequestTimestamps {
  timestamps: number[];
}

const clientHistory = new Map<string, RequestTimestamps>();
const HISTORY_WINDOW_MS = 10_000;
const HISTORY_MAX_ENTRIES = 20_000;

function pruneAndRecord(clientKey: string, now: number): number[] {
  let entry = clientHistory.get(clientKey);
  if (!entry) {
    entry = { timestamps: [] };
    clientHistory.set(clientKey, entry);
    if (clientHistory.size > HISTORY_MAX_ENTRIES) {
      const oldestKey = clientHistory.keys().next().value;
      if (oldestKey) clientHistory.delete(oldestKey);
    }
  }
  entry.timestamps = entry.timestamps.filter((t) => now - t < HISTORY_WINDOW_MS);
  entry.timestamps.push(now);
  return entry.timestamps;
}

/**
 * Heuristic bot/automation detector. Not a replacement for a managed bot
 * product (Cloudflare Bot Management, AWS WAF Bot Control) — this is the
 * layer that runs regardless of which upstream CDN/WAF is in front of the
 * service, and the signal both feed into the combined DDoS middleware.
 */
export function assessBot(req: Request, clientKey: string, now: number = Date.now()): BotAssessment {
  const signals: BotSignal[] = [];
  const ua = (req.headers["user-agent"] as string | undefined)?.toLowerCase() ?? "";

  if (!ua) {
    // Weak signal on its own — plenty of legitimate non-browser clients
    // (service-to-service calls, CLIs, mobile SDKs) omit it entirely.
    signals.push({ reason: "missing User-Agent header", weight: 15 });
  } else if (KNOWN_GOOD_CRAWLER_UA_SUBSTRINGS.some((s) => ua.includes(s))) {
    // Declared, well-behaved crawlers are not penalized further.
  } else if (KNOWN_BOT_UA_SUBSTRINGS.some((s) => ua.includes(s))) {
    signals.push({ reason: "known automation/tooling User-Agent", weight: 45 });
  }

  if (!req.headers.accept) {
    signals.push({ reason: "missing Accept header", weight: 10 });
  }
  if (!req.headers["accept-language"]) {
    signals.push({ reason: "missing Accept-Language header", weight: 5 });
  }

  const history = pruneAndRecord(clientKey, now);
  if (history.length >= 2) {
    const intervals = history.slice(1).map((t, i) => t - history[i]);
    const meanIntervalMs = intervals.reduce((a, b) => a + b, 0) / intervals.length;
    if (history.length >= 8 && meanIntervalMs < 150) {
      signals.push({ reason: "machine-speed request cadence", weight: 35 });
    }
    if (intervals.length >= 5) {
      const variance = intervals.reduce((a, b) => a + (b - meanIntervalMs) ** 2, 0) / intervals.length;
      const stddev = Math.sqrt(variance);
      if (stddev < 5 && meanIntervalMs < 1000) {
        signals.push({ reason: "suspiciously uniform request timing", weight: 30 });
      }
    }
  }

  const score = Math.min(100, signals.reduce((acc, s) => acc + s.weight, 0));
  const classification = score >= 60 ? "bot" : score >= 30 ? "suspicious" : "human";
  return { score, signals, classification };
}

export function clearBotHistory(): void {
  clientHistory.clear();
}
