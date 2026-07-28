import { ContractStatistics, Event, EventHeader, AuditLedgerError, CacheStats, SnapshotMetadata, BatchSignature, BatchVerificationResult } from './types';
import { Logger, LogLevel, LoggerOptions } from './logger';
import { EventValidator, ValidatorOptions, EventInput } from './validator';
import { SubscriptionManager, SubscriptionOptions, EventCallback, Subscription } from './subscriptions';
import { BatchProcessor, BatchItem, BatchProcessorOptions, BatchProcessorResult, ProgressCallback } from './batch';
import { BatchSigner } from './batch-signing';

/** Transport function type: sends a Soroban RPC method call and returns the result. */
export type Transport = (method: string, params: unknown[]) => Promise<unknown>;

export interface RetryOptions {
  maxRetries?: number;
  baseDelayMs?: number;
}

export interface BatchProgress {
  completed: number;
  total: number;
}

export interface AuditLedgerClientOptions {
  retryOptions?: RetryOptions;
  /** Logging options (#237) */
  logging?: LoggerOptions;
  /** Validation options — pass an object to enable client-side validation (#232) */
  validation?: ValidatorOptions | false;
}

export class AuditLedgerClient {
  transport: Transport;
  contractId?: string;
  maxRetries: number;
  baseDelayMs: number;
  private eventCache: Map<number, Event>;
  private totalEventsCache: number | undefined;
  private cacheHits: number;
  private cacheMisses: number;
  private maxCacheSize: number;
  private maxPageSize: number;

  /** #237 — Logger */
  readonly logger: Logger;

  /** #232 — Validator (null when validation is disabled) */
  readonly validator: EventValidator | null;

  /** #231 — Subscription manager */
  readonly subscriptions: SubscriptionManager;

  constructor(transport: Transport, contractId?: string, retryOrOptions: RetryOptions | AuditLedgerClientOptions = {}) {
    this.transport = transport;
    this.contractId = contractId;

    // Support legacy RetryOptions shape as well as the new AuditLedgerClientOptions shape
    const opts = retryOrOptions as AuditLedgerClientOptions;
    const retry: RetryOptions =
      'maxRetries' in retryOrOptions || 'baseDelayMs' in retryOrOptions
        ? (retryOrOptions as RetryOptions)
        : (opts.retryOptions ?? {});

    this.maxRetries = retry.maxRetries ?? 3;
    this.baseDelayMs = retry.baseDelayMs ?? 500;

    // #237 — Logger
    this.logger = new Logger(opts.logging ?? {});

    // #232 — Validator (enabled by default)
    if (opts.validation === false) {
      this.validator = null;
    } else {
      this.validator = new EventValidator(opts.validation ?? {});
    }

    // #231 — Subscription manager
    this.subscriptions = new SubscriptionManager({ logger: this.logger });
  }

  static fromRpc(rpcUrl: string, contractId?: string, retryOptions: RetryOptions = {}) {
    const transport: Transport = async (method: string, params: unknown[]) => {
      try {
        const res = await fetch(rpcUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ method, params }),
        });
        if (!res.ok) throw new AuditLedgerError('Transport error', undefined, res.status);
        const json: { result?: unknown; error?: { message: string; code: number } } = await res.json() as { result?: unknown; error?: { message: string; code: number } };
        if (json.error) throw new AuditLedgerError(json.error.message, json.error.code, res.status);
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

  private isRetryableError(err: unknown): boolean {
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

  private async callTransport<T>(method: string, params: unknown[]): Promise<T> {
    // #237 — log request
    this.logger.logRequest(method, params);
    const start = Date.now();
    let attempt = 0;
    for (;;) {
      await this.rateLimiter?.acquire();
      try {
        const result = await this.transport(method, params);
        // #237 — log response + performance
        const durationMs = Date.now() - start;
        this.logger.logResponse(method, result, durationMs);
        this.logger.logPerformance(method, durationMs);
        return result;
      } catch (err) {
        // #237 — log error with context
        this.logger.logError(method, err, { attempt });
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

  async logEvent(submitter: string, eventType: string, metadata: string): Promise<string> {
    // #232 — validate before submitting
    if (this.validator) {
      this.validator.validateOrThrow({ submitter, type: eventType, metadata });
    }
    return this.callTransport('log_event', [submitter, eventType, metadata]);
  }

  async logEvents(events: { submitter: string; type: string; metadata: string }[]): Promise<number[]> {
    // #232 — validate all events before submitting
    if (this.validator) {
      for (const ev of events) {
        this.validator.validateOrThrow({ submitter: ev.submitter, type: ev.type, metadata: ev.metadata });
      }
    }
    return this.callTransport('log_events', [events]);
  }

  async getEvent(id: string): Promise<Event> {
    return this.callTransport('get_event', [id]);
  }

  async totalEvents(useCache = true): Promise<number> {
    if (useCache && this.totalEventsCache !== undefined) return this.totalEventsCache;
    const total = await this.callTransport<number>('total_events', []);
    this.totalEventsCache = total;
    return total;
  }

  async eventCount(type: string): Promise<number> {
    return this.callTransport('event_count', [type]);
  }

  async getEventByType(type: string, index: number): Promise<Event> {
    return this.callTransport('get_event_by_type', [type, index]);
  }

  async getEventByOrder(order: number): Promise<Event> {
    if (this.eventCache.has(order)) {
      this.cacheHits += 1;
      return this.eventCache.get(order)!;
    }
    this.cacheMisses += 1;
    const event = await this.callTransport<Event>('get_event_by_order', [order]);
    this.eventCache.set(order, event);
    while (this.eventCache.size > this.maxCacheSize) {
      const oldestKey = this.eventCache.keys().next().value as number | undefined;
      if (oldestKey === undefined) break;
      this.eventCache.delete(oldestKey);
    }
    return event;
  }

  async getEvents(offset = 0, limit = 50, cursor?: number): Promise<EventPage> {
    const start = cursor !== undefined ? Math.max(cursor, 0) : Math.max(offset, 0);
    const safeLimit = Math.max(1, Math.min(limit, this.maxPageSize));
    const total = await this.totalEvents();
    const end = Math.min(start + safeLimit, total);
    const items: Event[] = [];
    for (let index = start; index < end; index += 1) {
      items.push(await this.getEventByOrder(index));
    }
    return { items, total, offset: start, limit: safeLimit };
  }

  async *streamEvents(afterIndex = 0, pollIntervalMs = 5000): AsyncGenerator<Event> {
    let cursor = Math.max(afterIndex, 0);
    while (true) {
      const total = await this.totalEvents();
      while (cursor < total) {
        yield await this.getEventByOrder(cursor);
        cursor += 1;
      }
      if (pollIntervalMs <= 0) return;
      await this.sleep(pollIntervalMs);
    }
  }

  filterEvents(events: Event[], options: { eventType?: string; submitter?: string; startTime?: number; endTime?: number; metadataQuery?: string } = {}): Event[] {
    const query = options.metadataQuery?.toLowerCase();
    return events.filter((event) => {
      if (options.eventType && event.event_type !== options.eventType) return false;
      if (options.submitter && event.submitter !== options.submitter) return false;
      if (options.startTime !== undefined && event.timestamp < options.startTime) return false;
      if (options.endTime !== undefined && event.timestamp > options.endTime) return false;
      if (query) {
        const metadata = event.metadata.toLowerCase();
        if (!metadata.includes(query)) return false;
      }
      return true;
    });
  }

  exportEvents(events: Event[], fmt: 'json' | 'csv' = 'json', streaming = false, onProgress?: (progress: { completed: number; total: number }) => void): string {
    const total = events.length;
    const rows = events.map((event: Event, index: number) => {
      const metadataHex = Array.from(event.metadata)
        .map((c: string) => c.charCodeAt(0).toString(16).padStart(2, '0'))
        .join('');
      const record = {
        index: event.index,
        timestamp: event.timestamp,
        event_type: event.event_type,
        submitter: event.submitter,
        metadata: event.metadata,
        metadata_hex: metadataHex,
        event_hash: event.event_hash,
        prev_hash: event.prev_hash,
      };
      if (streaming) onProgress?.({ completed: index + 1, total });
      return record;
    });

    if (fmt === 'csv') {
      const headers = ['index', 'timestamp', 'event_type', 'submitter', 'metadata', 'metadata_hex', 'event_hash', 'prev_hash'];
      const lines = [headers.join(',')];
      for (const row of rows) {
        lines.push(headers.map((header) => String(row[header as keyof typeof row]).replace(/,/g, ' ')).join(','));
      }
      return lines.join('\n');
    }

    return JSON.stringify(rows);
  }

  cacheStats(): CacheStats {
    return { hits: this.cacheHits, misses: this.cacheMisses, size: this.eventCache.size };
  }

  invalidateCache() {
    this.eventCache.clear();
    this.totalEventsCache = undefined;
    this.cacheHits = 0;
    this.cacheMisses = 0;
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

  // ── Event watching via WebSocket ─────────────────────────────────────────

  watchEvents(wsUrl: string, type: string | null, cb: (evt: Event) => void) {
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => {
      const msg = type ? { action: 'subscribe', type } : { action: 'subscribe_all' };
      ws.send(JSON.stringify(msg));
    };
    ws.onmessage = (m) => {
      try {
        const data = JSON.parse(m.data as string);
        if (data.type === 'event_logged') {
          const event = data.event as Event;
          cb(event);
          // #231 — also publish to client-side subscription manager
          this.subscriptions.publish(event);
        }
      } catch (e) {
        this.logger.warn('watchEvents: failed to parse message', {
          error: e instanceof Error ? e.message : String(e),
        });
      }
    };
    return ws;
  }

  // ── #231 — Subscription management helpers ───────────────────────────────

  /**
   * Subscribe to events with optional filtering.
   * Events are delivered via `publish()` when using `watchEvents` or manual `publishEvent()`.
   */
  subscribe(options: SubscriptionOptions, callback: EventCallback): Subscription {
    return this.subscriptions.subscribe(options, callback);
  }

  /**
   * Cancel a subscription by ID.
   */
  cancelSubscription(id: string): boolean {
    return this.subscriptions.cancel(id);
  }

  /**
   * Manually publish an event to all active subscriptions (useful for testing/polling).
   */
  publishEvent(event: Event): void {
    this.subscriptions.publish(event);
  }

  // ── #233 — Concurrent batch submission ───────────────────────────────────

  /**
   * Submit events with configurable concurrency, progress tracking, and error isolation.
   * Each event is submitted independently; a failure does not abort remaining items.
   *
   * @param events     Events to submit
   * @param options    Batch processing options (concurrency, onProgress, etc.)
   */
  async submitBatchConcurrent<T = undefined>(
    events: BatchItem<T>[],
    options: BatchProcessorOptions = {},
  ): Promise<BatchProcessorResult<T>> {
    const processor = new BatchProcessor<T>({ logger: this.logger, ...options });
    return processor.process(events, (item) =>
      this.logEvent(item.submitter, item.type, item.metadata),
    );
  }

  /**
   * Legacy sequential batch submission with progress callback (preserved for backwards compat).
   */
  async submitBatch(
    events: { submitter: string; type: string; metadata: string }[],
    onProgress?: (p: BatchProgress) => void,
  ) {
    const total = events.length;
    let completed = 0;
    for (const ev of events) {
      await this.logEvent(ev.submitter, ev.type, ev.metadata);
      completed++;
      onProgress?.({ completed, total });
    }
  }

  // ── #218 — Batch Signing ────────────────────────────────────────────────

  /**
   * Get the batch signer instance for signing/verifying event batches.
   */
  getBatchSigner(): BatchSigner {
    return new BatchSigner();
  }
}

export default AuditLedgerClient;
