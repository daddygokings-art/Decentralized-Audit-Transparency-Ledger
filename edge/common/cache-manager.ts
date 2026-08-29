/**
 * Multi-Tier Edge Cache Manager (#521)
 *
 * Implements low-latency query caching with Stale-While-Revalidate (SWR),
 * cache-tag based invalidation, ETag generation, and SHA-256 key hashing.
 */

import { createHash } from "crypto";
import { CachedResponse, QueryCacheKey } from "./types";

export class EdgeCacheManager {
  private memoryCache: Map<string, CachedResponse> = new Map();
  private tagIndex: Map<string, Set<string>> = new Map();
  private defaultTtl: number;
  private defaultSwr: number;

  constructor(defaultTtlSeconds = 60, staleWhileRevalidateSeconds = 300) {
    this.defaultTtl = defaultTtlSeconds;
    this.defaultSwr = staleWhileRevalidateSeconds;
  }

  /**
   * Generates a deterministic hash for a query cache key
   */
  public generateCacheKey(query: QueryCacheKey): string {
    const sortedParams = Object.keys(query.params)
      .sort()
      .map((k) => `${k}=${query.params[k]}`)
      .join("&");
    const rawKey = `${query.path}?${sortedParams}`;
    return createHash("sha256").update(rawKey).digest("hex");
  }

  /**
   * Generates an ETag for cached content
   */
  public generateEtag(content: string): string {
    const hash = createHash("sha256").update(content).digest("hex").slice(0, 16);
    return `W/"${hash}"`;
  }

  /**
   * Look up cached query response with SWR evaluation
   */
  public get(
    key: string
  ): { response: CachedResponse | null; isStale: boolean; mustRevalidate: boolean } {
    const cached = this.memoryCache.get(key);
    if (!cached) {
      return { response: null, isStale: false, mustRevalidate: true };
    }

    const now = Date.now() / 1000;
    const age = now - cached.cachedAt;

    if (age <= cached.ttlSeconds) {
      // Fresh hit
      return { response: cached, isStale: false, mustRevalidate: false };
    } else if (age <= cached.ttlSeconds + cached.staleWhileRevalidateSeconds) {
      // Stale hit (can serve stale while revalidating asynchronously)
      return { response: cached, isStale: true, mustRevalidate: true };
    } else {
      // Expired
      this.memoryCache.delete(key);
      return { response: null, isStale: false, mustRevalidate: true };
    }
  }

  /**
   * Store query response in edge cache with tags
   */
  public set(
    key: string,
    body: string,
    statusCode = 200,
    headers: Record<string, string> = {},
    tags: string[] = [],
    ttlSeconds = this.defaultTtl,
    swrSeconds = this.defaultSwr
  ): CachedResponse {
    const etag = this.generateEtag(body);
    const entry: CachedResponse = {
      statusCode,
      headers: {
        ...headers,
        ETag: etag,
        "Cache-Control": `public, max-age=${ttlSeconds}, stale-while-revalidate=${swrSeconds}`,
      },
      body,
      cachedAt: Date.now() / 1000,
      ttlSeconds,
      staleWhileRevalidateSeconds: swrSeconds,
      etag,
      tags,
    };

    this.memoryCache.set(key, entry);

    // Index tags for invalidation
    for (const tag of tags) {
      if (!this.tagIndex.has(tag)) {
        this.tagIndex.set(tag, new Set());
      }
      this.tagIndex.get(tag)!.add(key);
    }

    return entry;
  }

  /**
   * Invalidate cache entries by cache tags
   */
  public invalidateByTag(tag: string): number {
    const keys = this.tagIndex.get(tag);
    if (!keys) return 0;

    let count = 0;
    for (const key of keys) {
      if (this.memoryCache.delete(key)) {
        count++;
      }
    }
    this.tagIndex.delete(tag);
    return count;
  }

  /**
   * Invalidate a single cache key
   */
  public invalidateKey(key: string): boolean {
    return this.memoryCache.delete(key);
  }

  /**
   * Clear all cached responses
   */
  public clear(): void {
    this.memoryCache.clear();
    this.tagIndex.clear();
  }
}
