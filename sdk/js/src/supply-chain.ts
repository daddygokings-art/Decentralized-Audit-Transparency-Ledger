/**
 * Supply Chain Module SDK Bindings (issue #872)
 *
 * Provides TypeScript bindings for cursor-based paginated queries
 * of supply chain records (brands, products, provenance events,
 * certifications, labor reports, environmental reports).
 */

import { PageCursor, MAX_PAGE_LIMIT } from './types';

/** Transport function type: sends a Soroban RPC method call and returns the result. */
export type Transport = (method: string, params: unknown[]) => Promise<unknown>;

// ────────────────────────────────────────────────────────────────────────────
// Supply Chain Types
// ────────────────────────────────────────────────────────────────────────────

/** Geographical location or facility. */
export interface Location {
  name: string;
  country: string;
  coordinates: string;
  facility_id: string;
}

/** Custody transfer record. */
export interface CustodyTransfer {
  from_address: string;
  to_address: string;
  timestamp: number;
  location: Location;
  transfer_notes: string;
}

/** Provenance data for product origin tracking. */
export interface Provenance {
  origin_location: Location;
  timestamp: number;
  raw_material_source: string;
  producer_address: string;
  batch_id: string;
  chain_of_custody: CustodyTransfer[];
  is_verified: boolean;
}

/** Audit entry for certifications. */
export interface AuditEntry {
  timestamp: number;
  auditor: string;
  status: string;
  notes: string;
}

/** Certification record (ISO, organic, fair trade, etc.). */
export interface Certification {
  cert_id: string;
  cert_type: string;
  issuer: string;
  issued_date: number;
  expiry_date: number;
  scope: string;
  is_active: boolean;
  verification_hash: string;
  audit_trail: AuditEntry[];
}

/** Labor conditions report. */
export interface LaborConditions {
  facility_id: string;
  report_date: number;
  reporter: string;
  worker_count: number;
  wage_compliance: boolean;
  working_hours_compliance: boolean;
  child_labor_free: boolean;
  safety_standards_met: boolean;
  freedom_of_association: boolean;
  report_hash: string;
  certifications: string[];
}

/** Environmental impact report. */
export interface EnvironmentalImpact {
  facility_id: string;
  report_period: [number, number];
  carbon_footprint: number;
  water_usage: number;
  waste_generated: number;
  renewable_energy_percent: number;
  emissions_reduction_percent: number;
  certifications: string[];
  report_hash: string;
}

/** Brand registration. */
export interface Brand {
  brand_id: string;
  name: string;
  owner: string;
  verified: boolean;
  description: string;
  website: string;
  support_contact: string;
}

/** Product SKU tracked for supply chain. */
export interface ProductSKU {
  sku: string;
  brand_id: string;
  product_name: string;
  description: string;
  provenance_id: string;
  certifications: string[];
  labor_reports: string[];
  environmental_reports: string[];
  created_date: number;
  last_updated: number;
}

// ────────────────────────────────────────────────────────────────────────────
// Pagination Types
// ────────────────────────────────────────────────────────────────────────────

/** Multi-predicate filter for supply chain queries. */
export interface SupplyChainQueryFilters {
  /** Inclusive lower bound on timestamp. */
  timestamp_min?: number;
  /** Inclusive upper bound on timestamp. */
  timestamp_max?: number;
  /** Filter by status (e.g., 'active', 'expired', 'verified', 'unverified'). */
  status?: string;
  /** Filter by submitter/creator/reporter address. */
  submitter?: string;
  /** Filter by brand ID (for products). */
  brand_id?: string;
  /** Filter by facility ID (for labor/environmental reports). */
  facility_id?: string;
}

/** Options for paginated supply chain queries. */
export interface SupplyChainQueryOptions extends SupplyChainQueryFilters {
  /** Opaque cursor from previous page; omit or pass null for first page. */
  cursor?: PageCursor;
  /** Requested page size; clamped to MAX_PAGE_LIMIT (100). */
  limit?: number;
}

/** Paginated result wrapper. */
export interface PaginatedResult<T> {
  items: T[];
  /** Cursor for next page, or null when last page reached. */
  next_cursor: PageCursor;
  /** Total count of items in collection (may exceed page size). */
  total_count: number;
}

// ────────────────────────────────────────────────────────────────────────────
// Supply Chain Client
// ────────────────────────────────────────────────────────────────────────────

/**
 * Supply chain module client for paginated queries.
 *
 * Provides cursor-based pagination with deterministic ordering and
 * multi-predicate filtering for brands, products, provenance events,
 * certifications, labor reports, and environmental reports.
 *
 * All queries enforce a maximum page size of 100 items and return a
 * `next_cursor` token for fetching subsequent pages.
 *
 * @example
 * ```typescript
 * const client = new SupplyChainClient(transport);
 *
 * // Fetch first page of brands
 * const page1 = await client.listBrands({ limit: 50 });
 * console.log(`Found ${page1.items.length} brands`);
 *
 * // Fetch next page using cursor
 * if (page1.next_cursor) {
 *   const page2 = await client.listBrands({
 *     cursor: page1.next_cursor,
 *     limit: 50
 *   });
 * }
 *
 * // Filter products by brand and timestamp range
 * const products = await client.listProducts({
 *   brand_id: 'acme',
 *   timestamp_min: 1672531200,
 *   timestamp_max: 1704067200,
 *   limit: 25
 * });
 * ```
 */
export class SupplyChainClient {
  private transport: Transport;

  constructor(transport: Transport) {
    this.transport = transport;
  }

  /**
   * List brands with cursor-based pagination.
   *
   * Returns at most `limit` brands (default 50, max 100) in deterministic
   * insertion order. Use the returned `next_cursor` to fetch subsequent pages.
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated brands result
   */
  async listBrands(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<Brand>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_brands_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * List products with cursor-based pagination and filtering.
   *
   * Supports filtering by:
   * - Brand ID (`brand_id`)
   * - Timestamp range (`timestamp_min`, `timestamp_max`)
   * - Submitter address (`submitter`)
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated products result
   */
  async listProducts(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<ProductSKU>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_products_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * List provenance events with cursor-based pagination and filtering.
   *
   * Supports filtering by:
   * - Timestamp range (`timestamp_min`, `timestamp_max`)
   * - Submitter/producer address (`submitter`)
   * - Verification status (`status`: 'verified' or 'unverified')
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated provenance events result
   */
  async listProvenanceEvents(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<Provenance>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_provenance_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * List certifications with cursor-based pagination and expiry filtering.
   *
   * Supports filtering by:
   * - Active/expired status (`status`: 'active' or 'expired')
   * - Issuer address (`submitter`)
   * - Timestamp range for issued date (`timestamp_min`, `timestamp_max`)
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated certifications result
   */
  async listCertifications(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<Certification>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_certifications_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * List labor reports with cursor-based pagination and facility filtering.
   *
   * Supports filtering by:
   * - Facility ID (`facility_id`)
   * - Reporter address (`submitter`)
   * - Timestamp range for report date (`timestamp_min`, `timestamp_max`)
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated labor reports result
   */
  async listLaborReports(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<LaborConditions>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_labor_reports_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * List environmental reports with cursor-based pagination and facility filtering.
   *
   * Supports filtering by:
   * - Facility ID (`facility_id`)
   * - Reporter address (`submitter`)
   * - Timestamp range for report period start (`timestamp_min`, `timestamp_max`)
   *
   * @param options Query options including cursor, limit, and filters
   * @returns Paginated environmental reports result
   */
  async listEnvironmentalReports(
    options: SupplyChainQueryOptions = {}
  ): Promise<PaginatedResult<EnvironmentalImpact>> {
    const limit = this.clampLimit(options.limit);
    const params = this.buildParams(options, limit);

    const result = await this.transport('list_environmental_reports_paginated', [
      params.cursor,
      params.limit,
      params.filters,
    ]);

    return this.parsePaginatedResult(result);
  }

  /**
   * Fetch all items from a paginated collection by automatically following cursors.
   *
   * **WARNING**: Use with caution on large collections. This will make multiple
   * RPC calls until all pages are exhausted. Consider using manual pagination
   * with cursors for better control over resource usage.
   *
   * @param fetchPage Function that fetches a single page given options
   * @param options Initial query options (filters only; cursor/limit managed internally)
   * @returns Array of all items across all pages
   */
  async fetchAll<T>(
    fetchPage: (options: SupplyChainQueryOptions) => Promise<PaginatedResult<T>>,
    options: SupplyChainQueryFilters = {}
  ): Promise<T[]> {
    const items: T[] = [];
    let cursor: PageCursor = null;

    do {
      const page = await fetchPage({ ...options, cursor, limit: MAX_PAGE_LIMIT });
      items.push(...page.items);
      cursor = page.next_cursor;
    } while (cursor !== null);

    return items;
  }

  // ──────────────────────────────────────────────────────────────────────────
  // Private helpers
  // ──────────────────────────────────────────────────────────────────────────

  private clampLimit(requested?: number): number {
    if (!requested || requested <= 0) {
      return 50; // default
    }
    return Math.min(requested, MAX_PAGE_LIMIT);
  }

  private buildParams(
    options: SupplyChainQueryOptions,
    limit: number
  ): {
    cursor: PageCursor;
    limit: number;
    filters: Record<string, unknown>;
  } {
    const filters: Record<string, unknown> = {};

    if (options.timestamp_min !== undefined) {
      filters.timestamp_min = options.timestamp_min;
    }
    if (options.timestamp_max !== undefined) {
      filters.timestamp_max = options.timestamp_max;
    }
    if (options.status !== undefined) {
      filters.status = options.status;
    }
    if (options.submitter !== undefined) {
      filters.submitter = options.submitter;
    }
    if (options.brand_id !== undefined) {
      filters.brand_id = options.brand_id;
    }
    if (options.facility_id !== undefined) {
      filters.facility_id = options.facility_id;
    }

    return {
      cursor: options.cursor ?? null,
      limit,
      filters,
    };
  }

  private parsePaginatedResult<T>(result: unknown): PaginatedResult<T> {
    if (typeof result !== 'object' || result === null) {
      throw new Error('Invalid paginated result: expected object');
    }

    const typed = result as {
      items?: unknown;
      next_cursor?: unknown;
      total_count?: unknown;
    };

    if (!Array.isArray(typed.items)) {
      throw new Error('Invalid paginated result: items must be array');
    }

    const next_cursor =
      typed.next_cursor === null || typed.next_cursor === undefined
        ? null
        : String(typed.next_cursor);

    const total_count =
      typeof typed.total_count === 'number' ? typed.total_count : 0;

    return {
      items: typed.items as T[],
      next_cursor,
      total_count,
    };
  }
}

/**
 * Convenience helper: iterate through all pages of a paginated collection.
 *
 * Automatically handles cursor management and yields items one page at a time.
 *
 * @example
 * ```typescript
 * const client = new SupplyChainClient(transport);
 *
 * for await (const page of paginateAll(
 *   (opts) => client.listProducts(opts),
 *   { brand_id: 'acme', limit: 50 }
 * )) {
 *   console.log(`Processing ${page.items.length} products`);
 *   for (const product of page.items) {
 *     // Process each product
 *   }
 * }
 * ```
 */
export async function* paginateAll<T>(
  fetchPage: (options: SupplyChainQueryOptions) => Promise<PaginatedResult<T>>,
  initialOptions: SupplyChainQueryOptions = {}
): AsyncGenerator<PaginatedResult<T>, void, undefined> {
  let cursor: PageCursor = initialOptions.cursor ?? null;
  const options = { ...initialOptions };

  do {
    const page = await fetchPage({ ...options, cursor });
    yield page;
    cursor = page.next_cursor;
  } while (cursor !== null);
}
