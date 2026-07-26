import { ContractStatistics, Event, AuditLedgerError } from './types';
import { RateLimitConfig, TokenBucket, parseRateLimitHeaders } from './rateLimit';
import {
  CompressionConfig,
  CompressionStats,
  CompressionStatsTracker,
  CompressionTotals,
  decodeMetadata,
  encodeMetadata,
} from './compression';
import { buildSignaturePayload, signBatch, verifySignaturePayload } from './signing';

export type Transport = (method: string, params: any[]) => Promise<any>;

export interface RetryOptions {
  maxRetries?: number;
  baseDelayMs?: number;
}

export interface BatchProgress {
  completed: number;
  total: number;
}

export interface ReplayFilter {
  eventType?: string;
  submitter?: string;
  predicate?: (evt: Event) => boolean;
}

export interface ReplayProgress {
  processed: number;
  total: number;
  matched: number;
}

export interface ReplayOptions {
  /** Resume replay starting at this sequential index (ignored if fromTimestamp is set). */
  fromIndex?: number;
  /** Resume replay starting at the first event at or after this unix timestamp. */
  fromTimestamp?: number;
  /** Stop replay at this unix timestamp (inclusive). Only used with fromTimestamp. */
  toTimestamp?: number;
  filter?: ReplayFilter;
  /** Number of events fetched per page. Default 50. */
  pageSize?: number;
  onProgress?: (progress: ReplayProgress) => void;
}

export class AuditLedgerClient {
  transport: Transport;
  contractId?: string;
  maxRetries: number;
  baseDelayMs: number;
  rateLimiter?: TokenBucket;
  compressionConfig?: CompressionConfig;
  private compressionStats = new CompressionStatsTracker();

  constructor(
    transport: Transport,
    contractId?: string,
    retryOptions: RetryOptions = {},
    rateLimit?: RateLimitConfig,
    compression?: CompressionConfig,
  ) {
    this.transport = transport;
    this.contractId = contractId;
    this.maxRetries = retryOptions.maxRetries ?? 3;
    this.baseDelayMs = retryOptions.baseDelayMs ?? 500;
    this.rateLimiter = rateLimit ? new TokenBucket(rateLimit) : undefined;
    this.compressionConfig = compression;
  }

  static fromRpc(
    rpcUrl: string,
    contractId?: string,
    retryOptions: RetryOptions = {},
    rateLimit?: RateLimitConfig,
    compression?: CompressionConfig,
  ) {
    const transport: Transport = async (method, params) => {
      try {
        const res = await fetch(rpcUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ method, params }),
        });
        const rateLimitHeaders = parseRateLimitHeaders(res.headers);
        if (!res.ok) {
          throw new AuditLedgerError(
            'Transport error',
            undefined,
            res.status,
            rateLimitHeaders.retryAfterSeconds,
          );
        }
        const json = await res.json();
        if (json.error) {
          throw new AuditLedgerError(
            json.error.message,
            json.error.code,
            res.status,
            rateLimitHeaders.retryAfterSeconds,
          );
        }
        return json.result;
      } catch (err) {
        if (err instanceof AuditLedgerError) throw err;
        throw err;
      }
    };
    return new AuditLedgerClient(transport, contractId, retryOptions, rateLimit, compression);
  }

  private async sleep(ms: number) {
    await new Promise((resolve) => setTimeout(resolve, ms));
  }

  private isRetryableError(err: unknown) {
    if (err instanceof AuditLedgerError) {
      return err.status === 429 || err.status === 503;
    }
    if (err instanceof TypeError) return true;
    if (typeof err === 'object' && err !== null) {
      const error = err as { name?: string; code?: string; status?: number };
      if (error.status === 429 || error.status === 503) return true;
      if (error.name === 'FetchError' || error.name === 'NetworkError') return true;
      if (error.code && ['ECONNRESET', 'ETIMEDOUT', 'EAI_AGAIN', 'ENOTFOUND'].includes(error.code)) return true;
    }
    return false;
  }

  private async callTransport<T>(method: string, params: any[]): Promise<T> {
    let attempt = 0;
    for (;;) {
      await this.rateLimiter?.acquire();
      try {
        return await this.transport(method, params);
      } catch (err) {
        if (err instanceof AuditLedgerError && err.retryAfterSeconds !== undefined) {
          this.rateLimiter?.blockFor(err.retryAfterSeconds);
        }
        if (attempt >= this.maxRetries || !this.isRetryableError(err)) {
          throw err;
        }
        const delay =
          err instanceof AuditLedgerError && err.retryAfterSeconds !== undefined
            ? err.retryAfterSeconds * 1000
            : this.baseDelayMs * (2 ** attempt);
        attempt += 1;
        await this.sleep(delay);
      }
    }
  }

  async initialize(owner: string, globalMaxLogs: number, maxMetadataBytes: number = 4096) {
    return this.callTransport('initialize', [owner, globalMaxLogs, maxMetadataBytes]);
  }

  async logEvent(submitter: string, eventType: string, metadata: string) : Promise<string> {
    return this.callTransport('log_event', [submitter, eventType, metadata]);
  }

  async logEvents(events: { submitter: string; type: string; metadata: string }[]): Promise<number[]> {
    return this.callTransport('log_events', [events]);
  }

  async getEvent(id: string): Promise<Event> {
    return this.callTransport('get_event', [id]);
  }

  async totalEvents(): Promise<number> {
    return this.callTransport('total_events', []);
  }

  async eventCount(type: string): Promise<number> {
    return this.callTransport('event_count', [type]);
  }

  async getEventByType(type: string, index: number): Promise<Event> {
    return this.callTransport('get_event_by_type', [type, index]);
  }

  async getStatistics(): Promise<ContractStatistics> {
    return this.callTransport('get_statistics', []);
  }

  async getEventByOrder(order: number): Promise<Event> {
    return this.callTransport('get_event_by_order', [order]);
  }

  async listEvents(offset: number, limit: number): Promise<Event[]> {
    return this.callTransport('list_events', [offset, limit]);
  }

  async getEventsByTimeRange(
    startTime: number,
    endTime: number,
    offset: number,
    limit: number,
  ): Promise<Event[]> {
    return this.callTransport('get_events_by_time_range', [startTime, endTime, offset, limit]);
  }

  // ── Event replay (issue #238) ─────────────────────────────────────────

  /**
   * Replay events in order, from a specific index or timestamp, optionally
   * filtered, invoking `onEvent` for each match and reporting progress.
   */
  async replayEvents(
    options: ReplayOptions,
    onEvent: (evt: Event) => void | Promise<void>,
  ): Promise<ReplayProgress> {
    const pageSize = options.pageSize ?? 50;
    const total = await this.totalEvents();
    let processed = 0;
    let matched = 0;

    const matchesFilter = (evt: Event): boolean => {
      const filter = options.filter;
      if (!filter) return true;
      if (filter.eventType !== undefined && evt.event_type !== filter.eventType) return false;
      if (filter.submitter !== undefined && evt.submitter !== filter.submitter) return false;
      if (filter.predicate && !filter.predicate(evt)) return false;
      return true;
    };

    const handle = async (evt: Event) => {
      processed += 1;
      if (matchesFilter(evt)) {
        matched += 1;
        await onEvent(evt);
      }
      options.onProgress?.({ processed, total, matched });
    };

    if (options.fromTimestamp !== undefined) {
      const endTime = options.toTimestamp ?? Number.MAX_SAFE_INTEGER;
      let offset = 0;
      for (;;) {
        const batch = await this.getEventsByTimeRange(options.fromTimestamp, endTime, offset, pageSize);
        if (batch.length === 0) break;
        for (const evt of batch) await handle(evt);
        offset += batch.length;
        if (batch.length < pageSize) break;
      }
      return { processed, total, matched };
    }

    let offset = options.fromIndex ?? 0;
    for (;;) {
      const batch = await this.listEvents(offset, pageSize);
      if (batch.length === 0) break;
      for (const evt of batch) await handle(evt);
      offset += batch.length;
      if (batch.length < pageSize) break;
    }
    return { processed, total, matched };
  }

  // ── Event signing (issue #236) ────────────────────────────────────────

  /** Sign an event ID/message and submit it via `log_event_signed`. */
  async logEventSigned(
    submitter: string,
    eventType: string,
    metadata: string,
    privateKey: Buffer | Uint8Array,
    message: Buffer | Uint8Array,
  ): Promise<string> {
    const signaturePayload = buildSignaturePayload(privateKey, message);
    return this.callTransport('log_event_signed', [
      submitter,
      eventType,
      metadata,
      signaturePayload.toString('base64'),
    ]);
  }

  /** Verify a 96-byte (pubkey || signature) payload against the signed message. */
  verifyEventSignature(payload: Buffer | Uint8Array, message: Buffer | Uint8Array): boolean {
    return verifySignaturePayload(payload, message);
  }

  /** Sign a batch of messages with one private key, e.g. for a batch of pending event IDs. */
  signEventBatch(privateKey: Buffer | Uint8Array, messages: Array<Buffer | Uint8Array>): Buffer[] {
    return signBatch(privateKey, messages);
  }

  async getEventSignature(eventId: string): Promise<string | null> {
    return this.callTransport('get_event_signature', [eventId]);
  }

  // ── Event compression (issue #234) ────────────────────────────────────

  /** Compress metadata (per this client's compression config, or an override) and log the event. */
  async logEventCompressed(
    submitter: string,
    eventType: string,
    metadata: Buffer | Uint8Array,
    config?: CompressionConfig,
  ): Promise<{ id: string; stats: CompressionStats }> {
    const { payload, stats } = encodeMetadata(metadata, config ?? this.compressionConfig ?? {});
    this.compressionStats.record(stats);
    const id = await this.logEvent(submitter, eventType, payload.toString('base64'));
    return { id, stats };
  }

  /** Fetch an event and decompress its metadata (auto-detected from the envelope tag). */
  async getEventDecompressed(id: string): Promise<{ event: Event; metadata: Buffer }> {
    const event = await this.getEvent(id);
    const raw = Buffer.from(event.metadata, 'base64');
    return { event, metadata: decodeMetadata(raw) };
  }

  /** Cumulative compression statistics across all `logEventCompressed` calls on this client. */
  getCompressionStatistics(): CompressionTotals {
    return this.compressionStats.totals();
  }

  // Governance helpers (examples)
  async setGlobalMaxLogs(caller: string, newMax: number) {
    return this.callTransport('set_global_max_logs', [caller, newMax]);
  }

  // Event watching via WebSocket
  watchEvents(wsUrl: string, type: string | null, cb: (evt: Event) => void) {
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => {
      const msg = type ? { action: 'subscribe', type } : { action: 'subscribe_all' };
      ws.send(JSON.stringify(msg));
    };
    ws.onmessage = (m) => {
      try {
        const data = JSON.parse(m.data as string);
        if (data.type === 'event_logged') cb(data.event as Event);
      } catch (e) {
        // ignore parse errors
      }
    };
    return ws;
  }

  // Batch submission with progress callback
  async submitBatch(events: { submitter: string; type: string; metadata: string }[], onProgress?: (p: BatchProgress) => void) {
    const total = events.length;
    let completed = 0;
    for (const ev of events) {
      await this.logEvent(ev.submitter, ev.type, ev.metadata);
      completed++;
      onProgress?.({ completed, total });
    }
  }
}

export default AuditLedgerClient;
