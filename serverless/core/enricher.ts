/**
 * Event Enrichment Engine (#522)
 *
 * Enriches contract events with DID identity, risk scoring, compliance classification,
 * and geolocation context.
 */

import { ContractEvent, EnrichedEvent } from "./types";

export class EventEnricher {
  /**
   * Enriches an event with auxiliary metadata and risk classification
   */
  public static async enrich(event: ContractEvent): Promise<EnrichedEvent> {
    const submitter = event.submitter;
    const did = `did:stellar:${submitter}`;

    // Calculate baseline risk score based on metadata characteristics
    let riskScore = 10;
    const metadataStr = typeof event.metadata === "string" ? event.metadata : JSON.stringify(event.metadata);
    if (metadataStr.includes("critical") || metadataStr.includes("override")) {
      riskScore = 85;
    } else if (metadataStr.length > 500) {
      riskScore = 40;
    }

    const complianceTags: string[] = [];
    if (event.category) {
      complianceTags.push(`category:${event.category}`);
    }
    if (event.eventType.toLowerCase().includes("tax")) {
      complianceTags.push("vat-compliance", "financial-reporting");
    }
    if (event.eventType.toLowerCase().includes("kyc")) {
      complianceTags.push("aml-bsa", "identity-attestation");
    }

    return {
      ...event,
      enrichedAt: Date.now(),
      enrichments: {
        submitterDid: did,
        geoCountry: "US",
        riskScore,
        complianceTags,
        contractDomain: "audit.ledger.stellar",
        entityName: `Entity_${submitter.slice(0, 8)}`,
      },
    };
  }
}
