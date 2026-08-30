/**
 * Contract Event Data Catalog Manager
 *
 * Provides metadata registration, search and discovery, dataset categorization,
 * compliance classification, and tagging for audit events.
 */

export type DataClassification = "Public" | "Internal" | "Confidential" | "Restricted";
export type ComplianceTag = "GDPR" | "CCPA" | "HIPAA" | "ESG" | "SOC2" | "PCI_DSS";

export interface CatalogAssetEntry {
  assetId: string;
  name: string;
  description: string;
  classification: DataClassification;
  owner: string;
  steward: string;
  tags: ComplianceTag[];
  schemaFields: Array<{
    name: string;
    type: string;
    pii: boolean;
    description: string;
  }>;
  retentionDays: number;
  version: number;
  createdAt: number;
  updatedAt: number;
}

export class DataCatalogManager {
  private assets = new Map<string, CatalogAssetEntry>();

  constructor() {
    this.seedDefaultAssets();
  }

  private seedDefaultAssets() {
    this.registerAsset({
      assetId: "asset-stellar-audit-events",
      name: "stellar_audit_events_primary",
      description: "Primary immutable audit ledger events from Soroban smart contract",
      classification: "Confidential",
      owner: "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFTGOBMAOTQTVHXBMSYL5",
      steward: "GACWTA5HYR7FUKVNYX4UGLYJ3K2XFNDQPMQJ62Q2H5F2YJ5I6T5N32Z4",
      tags: ["GDPR", "SOC2", "ESG"],
      schemaFields: [
        { name: "event_hash", type: "string", pii: false, description: "Cryptographic SHA256 event hash" },
        { name: "submitter", type: "string", pii: true, description: "Stellar account address of submitter" },
        { name: "metadata", type: "bytes", pii: true, description: "Raw event payload" },
        { name: "timestamp", type: "timestamp", pii: false, description: "Ledger consensus timestamp" },
      ],
      retentionDays: 2555, // 7 years
      version: 1,
      createdAt: Date.now() - 86400000,
      updatedAt: Date.now(),
    });
  }

  public registerAsset(asset: CatalogAssetEntry): void {
    this.assets.set(asset.assetId, asset);
  }

  public getAsset(assetId: string): CatalogAssetEntry | null {
    return this.assets.get(assetId) || null;
  }

  public searchAssets(query: {
    keyword?: string;
    classification?: DataClassification;
    tag?: ComplianceTag;
  }): CatalogAssetEntry[] {
    return Array.from(this.assets.values()).filter((asset) => {
      if (query.classification && asset.classification !== query.classification) return false;
      if (query.tag && !asset.tags.includes(query.tag)) return false;
      if (query.keyword) {
        const kw = query.keyword.toLowerCase();
        const matches =
          asset.name.toLowerCase().includes(kw) ||
          asset.description.toLowerCase().includes(kw) ||
          asset.tags.some((t) => t.toLowerCase().includes(kw));
        if (!matches) return false;
      }
      return true;
    });
  }

  public getAllAssets(): CatalogAssetEntry[] {
    return Array.from(this.assets.values());
  }
}
