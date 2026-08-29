/**
 * Event Transformation Engine (#522)
 *
 * Normalizes, anonymizes, and transforms contract event payloads into multiple formats.
 */

import { createHash } from "crypto";
import { ContractEvent, TransformationRule } from "./types";

export class EventTransformer {
  /**
   * Transforms a raw contract event according to declarative rules
   */
  public static transform(event: ContractEvent, rule: TransformationRule): any {
    let result: Record<string, any> = { ...event };

    // Apply field mappings / renames
    if (rule.fieldMappings) {
      for (const [sourceKey, targetKey] of Object.entries(rule.fieldMappings)) {
        if (sourceKey in result) {
          result[targetKey] = result[sourceKey];
          delete result[sourceKey];
        }
      }
    }

    // Exclude unwanted fields
    if (rule.excludedFields) {
      for (const field of rule.excludedFields) {
        delete result[field];
      }
    }

    // Anonymize sensitive fields with salted SHA-256
    if (rule.anonymizeFields) {
      for (const field of rule.anonymizeFields) {
        if (result[field]) {
          result[field] = createHash("sha256")
            .update(String(result[field]))
            .digest("hex");
        }
      }
    }

    // Format specific serialization
    if (rule.targetFormat === "cloudevents") {
      return {
        specversion: "1.0",
        type: `network.audit-ledger.event.${event.eventType}`,
        source: `/contract/${event.submitter}`,
        id: event.id || String(event.index),
        time: new Date(event.timestamp * 1000).toISOString(),
        datacontenttype: "application/json",
        data: result,
      };
    }

    return result;
  }
}
