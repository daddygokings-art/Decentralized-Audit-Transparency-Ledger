/**
 * Contract Event Edge Computing Integration Types (#521)
 */

export interface EdgeIngestEvent {
  eventType: string;
  category?: string;
  submitter: string;
  metadata: Record<string, unknown> | string;
  timestamp?: number;
  signature: string;
  nonce?: string;
}

export interface EdgeBatchIngestPayload {
  batchId: string;
  events: EdgeIngestEvent[];
  edgeNodeId: string;
  region: string;
  timestamp: number;
}

export interface EdgeIngestResult {
  success: boolean;
  batchId: string;
  eventCount: number;
  rootHash: string;
  processedAt: number;
  edgeRegion: string;
  error?: string;
}

export interface QueryCacheKey {
  path: string;
  params: Record<string, string>;
  headers?: Record<string, string>;
}

export interface CachedResponse {
  statusCode: number;
  headers: Record<string, string>;
  body: string;
  cachedAt: number;
  ttlSeconds: number;
  staleWhileRevalidateSeconds: number;
  etag: string;
  tags: string[];
}

export interface EdgeNodeConfig {
  nodeId: string;
  platform: "cloudflare-workers" | "aws-lambda-edge" | "fastly-compute-edge";
  region: string;
  upstreamRpcUrl: string;
  defaultTtlSeconds: number;
  staleWhileRevalidateSeconds: number;
  allowedOrigins: string[];
  rateLimitPerMinute: number;
}
