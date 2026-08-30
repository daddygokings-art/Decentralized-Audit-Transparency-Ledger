/**
 * Data Access Policies and Dynamic Masking Engine
 *
 * Implements RBAC/ABAC policy enforcement, PII redaction, hashing, and tokenization.
 */

import { createHash } from "crypto";

export interface AccessPolicyRule {
  policyId: string;
  role: string;
  allowedActions: Array<"read" | "write" | "export">;
  maskedColumns: Record<string, "redact" | "hash" | "tokenize">;
}

export class PolicyEnforcementEngine {
  private policies = new Map<string, AccessPolicyRule>();

  constructor() {
    this.seedDefaultPolicies();
  }

  private seedDefaultPolicies() {
    this.policies.set("public-viewer", {
      policyId: "pol-public",
      role: "public",
      allowedActions: ["read"],
      maskedColumns: {
        submitter: "hash",
        metadata: "redact",
      },
    });

    this.policies.set("compliance-auditor", {
      policyId: "pol-auditor",
      role: "auditor",
      allowedActions: ["read", "export"],
      maskedColumns: {
        metadata: "tokenize",
      },
    });

    this.policies.set("admin", {
      policyId: "pol-admin",
      role: "admin",
      allowedActions: ["read", "write", "export"],
      maskedColumns: {},
    });
  }

  public evaluateAccess(role: string, action: "read" | "write" | "export"): boolean {
    const policy = this.policies.get(role);
    if (!policy) return false;
    return policy.allowedActions.includes(action);
  }

  /**
   * Apply dynamic data masking to sensitive records based on role policy
   */
  public applyMasking(role: string, records: any[]): any[] {
    const policy = this.policies.get(role);
    if (!policy || Object.keys(policy.maskedColumns).length === 0) {
      return records;
    }

    return records.map((record) => {
      const masked = { ...record };
      for (const [col, maskType] of Object.entries(policy.maskedColumns)) {
        if (masked[col] !== undefined) {
          if (maskType === "redact") {
            masked[col] = "[REDACTED_PII]";
          } else if (maskType === "hash") {
            masked[col] = createHash("sha256").update(String(masked[col])).digest("hex").slice(0, 16) + "...";
          } else if (maskType === "tokenize") {
            masked[col] = `TOK_${Buffer.from(String(masked[col])).toString("base64").slice(0, 8)}`;
          }
        }
      }
      return masked;
    });
  }
}
