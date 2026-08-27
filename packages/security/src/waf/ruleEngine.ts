import type { Request } from "express";

export type RuleTarget = "path" | "query" | "body" | "headers" | "any";
export type RuleAction = "block" | "log" | "challenge";
export type RuleSeverity = "low" | "medium" | "high" | "critical";

export interface WafRule {
  id: string;
  name: string;
  description?: string;
  target: RuleTarget;
  pattern: RegExp;
  action: RuleAction;
  severity: RuleSeverity;
  enabled: boolean;
  managed: boolean; // built-in default rule vs. custom-added
}

export interface RuleMatch {
  rule: WafRule;
  target: RuleTarget;
  sample: string;
}

export interface EvaluationResult {
  matches: RuleMatch[];
  action: RuleAction | "allow";
}

function flattenValues(value: unknown, depth = 0): string[] {
  if (depth > 5 || value == null) return [];
  if (typeof value === "string") return [value];
  if (typeof value === "number" || typeof value === "boolean") return [String(value)];
  if (Array.isArray(value)) return value.flatMap((v) => flattenValues(v, depth + 1));
  if (typeof value === "object") return Object.values(value as Record<string, unknown>).flatMap((v) => flattenValues(v, depth + 1));
  return [];
}

/** Default OWASP-style signatures covering the most common injection classes
 * targeting JSON/REST APIs. Not exhaustive — real deployments should layer
 * this with a managed WAF (Cloudflare/AWS) for signature freshness; see
 * ./cloudflare.ts and ./awsShield.ts. */
export function defaultRules(): WafRule[] {
  return [
    {
      id: "sqli-1",
      name: "SQL injection keywords",
      target: "any",
      pattern: /(\b(union\s+select|select\s+.*\s+from|drop\s+table|insert\s+into|or\s+1\s*=\s*1|'\s*or\s*'1'\s*=\s*'1)\b)/i,
      action: "block",
      severity: "critical",
      enabled: true,
      managed: true,
    },
    {
      id: "xss-1",
      name: "Script tag / event handler injection",
      target: "any",
      pattern: /(<script[\s>]|javascript:|on(error|load|click)\s*=)/i,
      action: "block",
      severity: "high",
      enabled: true,
      managed: true,
    },
    {
      id: "traversal-1",
      name: "Path traversal",
      target: "any",
      pattern: /(\.\.\/|\.\.\\|%2e%2e%2f)/i,
      action: "block",
      severity: "high",
      enabled: true,
      managed: true,
    },
    {
      id: "cmdi-1",
      name: "Command injection metacharacters",
      target: "any",
      pattern: /(;|\|\||&&)\s*(cat|ls|whoami|curl|wget|nc|rm\s+-rf)\b/i,
      action: "block",
      severity: "critical",
      enabled: true,
      managed: true,
    },
    {
      id: "ssrf-1",
      name: "Internal metadata endpoint probe",
      target: "any",
      pattern: /(169\.254\.169\.254|metadata\.google\.internal)/i,
      action: "block",
      severity: "critical",
      enabled: true,
      managed: true,
    },
  ];
}

/** Runtime-manageable rule set: add/remove/enable/disable custom rules
 * alongside the managed defaults, and evaluate incoming requests against
 * them. */
export class WafRuleEngine {
  private rules = new Map<string, WafRule>();

  constructor(initialRules: WafRule[] = defaultRules()) {
    for (const rule of initialRules) this.rules.set(rule.id, rule);
  }

  addRule(rule: Omit<WafRule, "managed">): WafRule {
    const full: WafRule = { ...rule, managed: false };
    this.rules.set(full.id, full);
    return full;
  }

  removeRule(id: string): boolean {
    const rule = this.rules.get(id);
    if (rule?.managed) throw new Error(`Cannot remove managed rule '${id}' — disable it instead`);
    return this.rules.delete(id);
  }

  setEnabled(id: string, enabled: boolean): boolean {
    const rule = this.rules.get(id);
    if (!rule) return false;
    rule.enabled = enabled;
    return true;
  }

  listRules(): WafRule[] {
    return Array.from(this.rules.values());
  }

  private extractTargets(req: Request, target: RuleTarget): string[] {
    switch (target) {
      case "path":
        return [req.path];
      case "query":
        return flattenValues(req.query);
      case "body":
        return flattenValues(req.body);
      case "headers":
        return flattenValues(req.headers);
      case "any":
        return [req.path, ...flattenValues(req.query), ...flattenValues(req.body)];
    }
  }

  evaluate(req: Request): EvaluationResult {
    const matches: RuleMatch[] = [];
    for (const rule of this.rules.values()) {
      if (!rule.enabled) continue;
      for (const sample of this.extractTargets(req, rule.target)) {
        if (rule.pattern.test(sample)) {
          matches.push({ rule, target: rule.target, sample: sample.slice(0, 200) });
          break;
        }
      }
    }
    const blocking = matches.find((m) => m.rule.action === "block");
    const challenging = matches.find((m) => m.rule.action === "challenge");
    const action: RuleAction | "allow" = blocking ? "block" : challenging ? "challenge" : matches.length > 0 ? "log" : "allow";
    return { matches, action };
  }
}
