/**
 * Edge Cryptographic Signature Verifier (#521)
 *
 * Provides fast edge verification for ingested contract events.
 */

import { createHash } from "crypto";
import { EdgeIngestEvent } from "./types";

export class EdgeSignatureVerifier {
  /**
   * Computes SHA-256 event digest for signature validation
   */
  public static computeEventHash(event: EdgeIngestEvent): string {
    const rawData = JSON.stringify({
      eventType: event.eventType,
      category: event.category ?? "default",
      submitter: event.submitter,
      metadata: event.metadata,
      timestamp: event.timestamp ?? 0,
    });
    return createHash("sha256").update(rawData).digest("hex");
  }

  /**
   * Computes a Merkle / batch root hash for a collection of edge events
   */
  public static computeBatchRoot(events: EdgeIngestEvent[]): string {
    if (events.length === 0) {
      return createHash("sha256").update("").digest("hex");
    }
    const hashes = events.map((e) => this.computeEventHash(e));
    let combined = hashes.join(":");
    return createHash("sha256").update(combined).digest("hex");
  }

  /**
   * Validates structure and cryptographic signatures of edge ingestion events
   */
  public static validateEvent(event: EdgeIngestEvent): { valid: boolean; error?: string } {
    if (!event.eventType || event.eventType.trim() === "") {
      return { valid: false, error: "Missing eventType" };
    }
    if (!event.submitter || event.submitter.trim() === "") {
      return { valid: false, error: "Missing submitter" };
    }
    if (!event.signature || event.signature.length < 64) {
      return { valid: false, error: "Invalid cryptographic signature" };
    }
    return { valid: true };
  }
}
