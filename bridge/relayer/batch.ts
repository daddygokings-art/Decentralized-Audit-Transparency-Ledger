/**
 * Bridge Batch Processing (#256)
 *
 * Collects multiple events into batches, generates proofs for each member,
 * submits the batch as a unit, and tracks batch-level statistics.
 */

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AuditEvent {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: string;
  ledger_seq: number;
  tx_hash: string;
}

export interface EventProof {
  ledgerSeq: bigint;
  txHash: string;
  eventIndex: number;
  eventHash: string;
  signature: string;
}

export interface BatchConfig {
  maxBatchSize: number;
  maxWaitMs: number;
}

export interface BatchProofEntry {
  event: AuditEvent;
  proof: EventProof | null;
  error?: string;
}

export interface BatchSubmissionResult {
  batchId: number;
  submitted: number;
  failed: number;
  results: string[];
}

export interface BatchStatistics {
  batchesCollected: number;
  batchesSubmitted: number;
  eventsProcessed: number;
  eventsFailed: number;
  averageBatchSize: number;
  lastBatchSize: number;
  lastBatchDurationMs: number;
}

const DEFAULT_BATCH_CONFIG: BatchConfig = {
  maxBatchSize: 25,
  maxWaitMs: 10_000,
};

// ── Batch collection ──────────────────────────────────────────────────────────

export class BatchCollector {
  private config: BatchConfig;
  private pending: AuditEvent[] = [];
  private windowStart: number | null = null;

  constructor(config: Partial<BatchConfig> = {}) {
    this.config = { ...DEFAULT_BATCH_CONFIG, ...config };
  }

  add(event: AuditEvent, now: number = Date.now()): void {
    if (this.pending.length === 0) {
      this.windowStart = now;
    }
    this.pending.push(event);
  }

  /** A batch is ready when it's full or the collection window has elapsed. */
  isReady(now: number = Date.now()): boolean {
    if (this.pending.length === 0) return false;
    if (this.pending.length >= this.config.maxBatchSize) return true;
    if (this.windowStart !== null && now - this.windowStart >= this.config.maxWaitMs) return true;
    return false;
  }

  size(): number {
    return this.pending.length;
  }

  /** Drains and returns the currently collected events, resetting the window. */
  flush(): AuditEvent[] {
    const batch = this.pending;
    this.pending = [];
    this.windowStart = null;
    return batch;
  }

  getConfig(): BatchConfig {
    return { ...this.config };
  }
}

// ── Batch proof generation ───────────────────────────────────────────────────

export type ProofBuilder = (event: AuditEvent) => EventProof;

export function generateBatchProofs(events: AuditEvent[], buildProof: ProofBuilder): BatchProofEntry[] {
  return events.map((event) => {
    try {
      return { event, proof: buildProof(event) };
    } catch (err) {
      return { event, proof: null, error: err instanceof Error ? err.message : String(err) };
    }
  });
}

// ── Batch submission ──────────────────────────────────────────────────────────

export type BatchSubmitter = (entry: BatchProofEntry) => Promise<string>;

export class BatchProcessor {
  private collector: BatchCollector;
  private stats: BatchStatistics = {
    batchesCollected: 0,
    batchesSubmitted: 0,
    eventsProcessed: 0,
    eventsFailed: 0,
    averageBatchSize: 0,
    lastBatchSize: 0,
    lastBatchDurationMs: 0,
  };
  private nextBatchId = 1;

  constructor(config: Partial<BatchConfig> = {}) {
    this.collector = new BatchCollector(config);
  }

  collect(event: AuditEvent, now: number = Date.now()): void {
    this.collector.add(event, now);
  }

  isReady(now: number = Date.now()): boolean {
    return this.collector.isReady(now);
  }

  pendingCount(): number {
    return this.collector.size();
  }

  async processBatch(buildProof: ProofBuilder, submit: BatchSubmitter): Promise<BatchSubmissionResult> {
    const startedAt = Date.now();
    const events = this.collector.flush();
    const batchId = this.nextBatchId++;

    if (events.length === 0) {
      return { batchId, submitted: 0, failed: 0, results: [] };
    }

    this.stats.batchesCollected++;

    const entries = generateBatchProofs(events, buildProof);
    const results: string[] = [];
    let submitted = 0;
    let failed = 0;

    for (const entry of entries) {
      if (!entry.proof) {
        failed++;
        results.push(`error: ${entry.error ?? "proof generation failed"}`);
        continue;
      }

      try {
        const result = await submit(entry);
        results.push(result);
        submitted++;
      } catch (err) {
        failed++;
        results.push(`error: ${err instanceof Error ? err.message : String(err)}`);
      }
    }

    this.stats.batchesSubmitted++;
    this.stats.eventsProcessed += submitted;
    this.stats.eventsFailed += failed;
    this.stats.lastBatchSize = events.length;
    this.stats.lastBatchDurationMs = Date.now() - startedAt;
    this.stats.averageBatchSize =
      (this.stats.eventsProcessed + this.stats.eventsFailed) / this.stats.batchesSubmitted;

    return { batchId, submitted, failed, results };
  }

  getStatistics(): BatchStatistics {
    return { ...this.stats };
  }

  resetStatistics(): void {
    this.stats = {
      batchesCollected: 0,
      batchesSubmitted: 0,
      eventsProcessed: 0,
      eventsFailed: 0,
      averageBatchSize: 0,
      lastBatchSize: 0,
      lastBatchDurationMs: 0,
    };
  }
}
