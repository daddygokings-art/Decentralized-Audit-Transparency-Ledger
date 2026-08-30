/**
 * Edge Geolocation & Latency Router (#521)
 *
 * Routes incoming requests to the optimal regional origin or nearest edge cache.
 */

export interface GeoRegion {
  code: string;
  name: string;
  primaryOriginUrl: string;
  fallbackOriginUrl: string;
  latitude: number;
  longitude: number;
}

export const KNOWN_REGIONS: Record<string, GeoRegion> = {
  "us-east": {
    code: "us-east",
    name: "US East (N. Virginia)",
    primaryOriginUrl: "https://us-east.api.audit-ledger.network",
    fallbackOriginUrl: "https://eu-west.api.audit-ledger.network",
    latitude: 38.0,
    longitude: -78.0,
  },
  "eu-west": {
    code: "eu-west",
    name: "EU West (Frankfurt / Dublin)",
    primaryOriginUrl: "https://eu-west.api.audit-ledger.network",
    fallbackOriginUrl: "https://us-east.api.audit-ledger.network",
    latitude: 50.1,
    longitude: 8.6,
  },
  "ap-southeast": {
    code: "ap-southeast",
    name: "Asia Pacific (Singapore)",
    primaryOriginUrl: "https://ap-southeast.api.audit-ledger.network",
    fallbackOriginUrl: "https://eu-west.api.audit-ledger.network",
    latitude: 1.3,
    longitude: 103.8,
  },
};

export class GeoRouter {
  /**
   * Resolves the nearest region based on country code or coordinates
   */
  public static resolveRegion(countryCode?: string): GeoRegion {
    if (!countryCode) return KNOWN_REGIONS["us-east"];

    const country = countryCode.toUpperCase();
    const europeCodes = ["DE", "FR", "GB", "NL", "IE", "SE", "CH", "IT", "ES", "PL"];
    const asiaCodes = ["SG", "JP", "KR", "AU", "IN", "HK", "TW", "NZ", "ID", "MY"];

    if (europeCodes.includes(country)) {
      return KNOWN_REGIONS["eu-west"];
    } else if (asiaCodes.includes(country)) {
      return KNOWN_REGIONS["ap-southeast"];
    }
    return KNOWN_REGIONS["us-east"];
  }
}
