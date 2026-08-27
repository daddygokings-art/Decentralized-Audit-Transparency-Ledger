import type { NextFunction, Request, Response, Router } from "express";
import { Router as createRouter } from "express";
import { WafRuleEngine } from "./ruleEngine";
import { assessBot } from "./botDetection";
import { isBlockedByAwsWaf, readAwsWafSignal, exportRuleGroupForAwsWaf } from "./awsShield";
import { readCloudflareSignal, resolveClientIp } from "./cloudflare";

export interface DdosProtectionOptions {
  ruleEngine: WafRuleEngine;
  /** Trust and honor upstream Cloudflare edge signals (client IP, threat score). */
  trustCloudflare?: boolean;
  /** Trust and honor upstream AWS WAF/Shield block decisions. */
  trustAwsWaf?: boolean;
  /** Bot score (0-100) at or above which a request is blocked outright. */
  botBlockThreshold?: number;
  /** Cloudflare threat score (0-100) at or above which a request is blocked. */
  cloudflareThreatBlockThreshold?: number;
}

export interface WafEvent {
  timestamp: number;
  clientIp: string;
  path: string;
  method: string;
  decision: "allow" | "block" | "challenge";
  reasons: string[];
  botScore: number;
}

const MAX_EVENTS = 2000;
const recentEvents: WafEvent[] = [];

function recordEvent(event: WafEvent): void {
  recentEvents.push(event);
  if (recentEvents.length > MAX_EVENTS) recentEvents.splice(0, recentEvents.length - MAX_EVENTS);
}

/**
 * Combined DDoS/WAF protection layer: merges the local rule engine, bot
 * heuristics, and (when present) upstream Cloudflare/AWS Shield signals into
 * a single allow/challenge/block decision. Designed to run defense-in-depth
 * *behind* a real edge DDoS product — Cloudflare/AWS Shield absorb
 * volumetric (L3/L4) attacks upstream; this middleware handles
 * application-layer (L7) abuse the edge product passed through or wasn't
 * configured to catch.
 */
export function ddosProtection(options: DdosProtectionOptions) {
  const botBlockThreshold = options.botBlockThreshold ?? 75;
  const cfThreatThreshold = options.cloudflareThreatBlockThreshold ?? 80;

  return (req: Request, res: Response, next: NextFunction): void => {
    const clientIp = options.trustCloudflare ? resolveClientIp(req) : req.ip ?? "unknown";
    const reasons: string[] = [];

    if (options.trustAwsWaf && isBlockedByAwsWaf(req)) {
      const signal = readAwsWafSignal(req);
      recordEvent({
        timestamp: Date.now(),
        clientIp,
        path: req.path,
        method: req.method,
        decision: "block",
        reasons: [`aws-waf:${signal.action}`],
        botScore: 0,
      });
      res.status(403).json({ error: "blocked_by_waf", source: "aws" });
      return;
    }

    if (options.trustCloudflare) {
      const cf = readCloudflareSignal(req);
      if (cf.trusted && cf.threatScore !== undefined && cf.threatScore >= cfThreatThreshold) {
        recordEvent({
          timestamp: Date.now(),
          clientIp,
          path: req.path,
          method: req.method,
          decision: "block",
          reasons: [`cloudflare-threat-score:${cf.threatScore}`],
          botScore: 0,
        });
        res.status(403).json({ error: "blocked_by_waf", source: "cloudflare" });
        return;
      }
    }

    const ruleResult = options.ruleEngine.evaluate(req);
    const bot = assessBot(req, clientIp);

    if (ruleResult.action === "block") {
      reasons.push(...ruleResult.matches.map((m) => `rule:${m.rule.id}`));
      recordEvent({ timestamp: Date.now(), clientIp, path: req.path, method: req.method, decision: "block", reasons, botScore: bot.score });
      res.status(403).json({ error: "blocked_by_waf", source: "rule-engine", rules: ruleResult.matches.map((m) => m.rule.id) });
      return;
    }

    if (bot.score >= botBlockThreshold) {
      reasons.push(...bot.signals.map((s) => `bot:${s.reason}`));
      recordEvent({ timestamp: Date.now(), clientIp, path: req.path, method: req.method, decision: "block", reasons, botScore: bot.score });
      res.status(403).json({ error: "blocked_by_waf", source: "bot-detection", score: bot.score });
      return;
    }

    if (ruleResult.action === "challenge") {
      reasons.push(...ruleResult.matches.map((m) => `rule:${m.rule.id}`));
      recordEvent({ timestamp: Date.now(), clientIp, path: req.path, method: req.method, decision: "challenge", reasons, botScore: bot.score });
      res.status(429).json({ error: "challenge_required", rules: ruleResult.matches.map((m) => m.rule.id) });
      return;
    }

    res.setHeader("X-Bot-Score", String(bot.score));
    if (ruleResult.matches.length > 0) {
      recordEvent({
        timestamp: Date.now(),
        clientIp,
        path: req.path,
        method: req.method,
        decision: "allow",
        reasons: ruleResult.matches.map((m) => `logged:${m.rule.id}`),
        botScore: bot.score,
      });
    }
    next();
  };
}

export function getRecentWafEvents(limit = 200): WafEvent[] {
  return recentEvents.slice(-limit);
}

export function clearWafEvents(): void {
  recentEvents.length = 0;
}

/** Admin endpoints for custom rule management and visibility into recent
 * WAF decisions. Mount this behind an admin-role auth guard. */
export function wafAdminRouter(ruleEngine: WafRuleEngine): Router {
  const router = createRouter();

  router.get("/rules", (_req, res) => {
    res.json({ data: ruleEngine.listRules() });
  });

  router.post("/rules", (req: Request, res: Response) => {
    const { id, name, target, pattern, flags, action, severity } = req.body ?? {};
    if (!id || !name || !target || !pattern || !action || !severity) {
      res.status(400).json({ error: "id, name, target, pattern, action, and severity are required" });
      return;
    }
    try {
      const rule = ruleEngine.addRule({
        id,
        name,
        target,
        pattern: new RegExp(pattern, flags ?? "i"),
        action,
        severity,
        enabled: true,
      });
      res.status(201).json({ data: rule });
    } catch (err) {
      res.status(400).json({ error: err instanceof Error ? err.message : "invalid rule" });
    }
  });

  router.patch("/rules/:id", (req: Request, res: Response) => {
    const enabled = req.body?.enabled;
    if (typeof enabled !== "boolean") {
      res.status(400).json({ error: "enabled (boolean) is required" });
      return;
    }
    const ok = ruleEngine.setEnabled(req.params.id, enabled);
    if (!ok) return res.status(404).json({ error: "rule not found" });
    res.json({ data: { id: req.params.id, enabled } });
  });

  router.delete("/rules/:id", (req: Request, res: Response) => {
    try {
      const ok = ruleEngine.removeRule(req.params.id);
      if (!ok) return res.status(404).json({ error: "rule not found" });
      res.status(204).end();
    } catch (err) {
      res.status(409).json({ error: err instanceof Error ? err.message : "cannot remove rule" });
    }
  });

  router.get("/events", (req: Request, res: Response) => {
    const limit = Math.min(parseInt(req.query.limit as string) || 200, MAX_EVENTS);
    res.json({ data: getRecentWafEvents(limit) });
  });

  router.get("/export/aws-waf", (_req, res) => {
    res.json(exportRuleGroupForAwsWaf(ruleEngine.listRules()));
  });

  return router;
}
