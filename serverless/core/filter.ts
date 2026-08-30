/**
 * Event Filtering Engine (#522)
 *
 * Evaluates multi-attribute predicates (eq, neq, gt, in, regex) to include or drop events.
 */

import { ContractEvent, FilterRule, FilterPredicate } from "./types";

export class EventFilter {
  /**
   * Evaluates whether an event passes the filter rules
   */
  public static evaluate(
    event: ContractEvent,
    rules: FilterRule[]
  ): { pass: boolean; matchedRule?: string; reason?: string } {
    if (rules.length === 0) return { pass: true };

    for (const rule of rules) {
      const isAnd = rule.logicalOp !== "OR";
      let ruleMatch = isAnd;

      for (const pred of rule.predicates) {
        const predResult = this.evaluatePredicate(event, pred);
        if (isAnd) {
          ruleMatch = ruleMatch && predResult;
          if (!ruleMatch) break;
        } else {
          if (predResult) {
            ruleMatch = true;
            break;
          }
        }
      }

      if (ruleMatch) {
        if (rule.mode === "exclude") {
          return { pass: false, matchedRule: rule.name, reason: "Matched exclude filter rule" };
        }
        return { pass: true, matchedRule: rule.name };
      }
    }

    return { pass: true };
  }

  private static evaluatePredicate(event: any, pred: FilterPredicate): boolean {
    const fieldValue = event[pred.field];
    if (fieldValue === undefined || fieldValue === null) return false;

    switch (pred.operator) {
      case "eq":
        return fieldValue === pred.value;
      case "neq":
        return fieldValue !== pred.value;
      case "gt":
        return Number(fieldValue) > Number(pred.value);
      case "gte":
        return Number(fieldValue) >= Number(pred.value);
      case "lt":
        return Number(fieldValue) < Number(pred.value);
      case "lte":
        return Number(fieldValue) <= Number(pred.value);
      case "in":
        return Array.isArray(pred.value) && pred.value.includes(fieldValue);
      case "contains":
        return String(fieldValue).includes(String(pred.value));
      case "regex":
        return new RegExp(pred.value).test(String(fieldValue));
      default:
        return false;
    }
  }
}
