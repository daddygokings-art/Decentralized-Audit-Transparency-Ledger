import type { Request } from "express";
import type { WafRule } from "./ruleEngine";

/**
 * AWS WAFv2/Shield running in front of the API (via CloudFront or an ALB)
 * annotates requests it inspected with `x-amzn-*` headers before forwarding
 * to origin. This module reads those signals and, separately, exports the
 * locally-managed custom rules (see ./ruleEngine.ts) as AWS WAFv2 rule
 * statements so the same "custom rule management" surface can be pushed to
 * a managed WAF via IaC (Terraform/CDK/`aws wafv2 update-web-acl`) instead
 * of only enforcing them in-process.
 */
export interface AwsWafSignal {
  inspected: boolean;
  action?: "ALLOW" | "BLOCK" | "COUNT" | "CAPTCHA" | "CHALLENGE";
  ruleGroupTraceId?: string;
}

export function readAwsWafSignal(req: Request): AwsWafSignal {
  const action = req.headers["x-amzn-waf-action"] as string | undefined;
  return {
    inspected: !!action,
    action: action ? (action.toUpperCase() as AwsWafSignal["action"]) : undefined,
    ruleGroupTraceId: req.headers["x-amzn-trace-id"] as string | undefined,
  };
}

export function isBlockedByAwsWaf(req: Request): boolean {
  const signal = readAwsWafSignal(req);
  return signal.action === "BLOCK" || signal.action === "CHALLENGE";
}

/** Translates a WAF rule pattern into an AWS WAFv2 `RegexPatternSetReferenceStatement`
 * rule definition (JSON, matching the shape of `aws wafv2 create-rule-group`
 * / CloudFormation `AWS::WAFv2::RuleGroup`). Consumers apply this via their
 * IaC pipeline; this library does not call the AWS API directly so it has
 * no runtime AWS credential dependency. */
export function toAwsWafRuleStatement(rule: WafRule, priority: number): Record<string, unknown> {
  const fieldToMatch =
    rule.target === "path"
      ? { UriPath: {} }
      : rule.target === "query"
        ? { QueryString: {} }
        : rule.target === "headers"
          ? { Headers: { MatchPattern: { All: {} }, MatchScope: "ALL", OversizeHandling: "MATCH" } }
          : { Body: { OversizeHandling: "MATCH" } };

  return {
    Name: rule.id,
    Priority: priority,
    Statement: {
      RegexMatchStatement: {
        RegexString: rule.pattern.source,
        FieldToMatch: fieldToMatch,
        TextTransformations: [{ Priority: 0, Type: "URL_DECODE" }],
      },
    },
    Action: rule.action === "block" ? { Block: {} } : rule.action === "challenge" ? { Challenge: {} } : { Count: {} },
    VisibilityConfig: {
      SampledRequestsEnabled: true,
      CloudWatchMetricsEnabled: true,
      MetricName: `audit-ledger-${rule.id}`,
    },
  };
}

export function exportRuleGroupForAwsWaf(rules: WafRule[]): Record<string, unknown> {
  return {
    Name: "audit-ledger-custom-rules",
    Scope: "REGIONAL",
    Capacity: Math.max(10, rules.length * 5),
    Rules: rules.filter((r) => r.enabled).map((r, i) => toAwsWafRuleStatement(r, i + 1)),
  };
}
