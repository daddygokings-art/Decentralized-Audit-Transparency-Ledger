/**
 * #233 — SDK Batch Processing with Concurrency
 *
 * Concurrent batch submission of events with a configurable worker pool,
 * work queue, per-event progress tracking, and error isolation
 * (one failure does not abort the rest of the batch).
 */

import { Logger } from './logger';

export interface BatchItem<T> {
  submitter: string;
  type: string;
  metadata: string;
  /** Optional caller-supplied tag passed through to results */
  tag?: T;
}

export type BatchResultStatus = 'fulfilled' | 'rejected';

export interface BatchResult<T> {
  item: BatchItem<T>;
  status: BatchResultStatus;
  /** Returned event ID on success */
  value?: string;
  /** Error encountered on failure */
  error?: unknown;
  /** Wall-clock ms taken to process this item */
  durationMs: number;
}

export interface BatchProgress {
  completed: number;
  total: number;
  succeeded: number;
  failed: number;
}

export type ProgressCallback = (progress: BatchProgress) => void;

export interface BatchProcessorOptions {
  /** Number of concurrent workers (default: 5, min: 1) */
  concurrency?: number;
  /** Called after each item finishes (success or failure) */
  onProgress?: ProgressCallback;
  logger?: Logger;
}

export interface BatchProcessorResult<T> {
  results: BatchResult<T>[];
  succeeded: number;
  failed: number;
  totalDurationMs: number;
}

type WorkerFn<T> = (item: BatchItem<T>) => Promise<string>;

/**
 * Processes a batch of event submissions with bounded concurrency.
 *
 * Usage:
 *   const processor = new BatchProcessor({ concurrency: 10 });
 *   const result = await processor.process(items, (item) => client.logEvent(…));
 */
export class BatchProcessor<T = undefined> {
  private concurrency: number;
  private onProgress?: ProgressCallback;
  private logger?: Logger;

  constructor(options: BatchProcessorOptions = {}) {
    this.concurrency = Math.max(1, options.concurrency ?? 5);
    this.onProgress = options.onProgress;
    this.logger = options.logger;
  }

  /**
   * Process all items using up to `concurrency` parallel workers.
   * Errors are isolated — a failing item does not abort the batch.
   *
   * @param items  Events to submit
   * @param worker A function that submits one event and returns the event ID
   */
  async process(items: BatchItem<T>[], worker: WorkerFn<T>): Promise<BatchProcessorResult<T>> {
    const total = items.length;
    const results: BatchResult<T>[] = new Array(total);
    const batchStart = Date.now();

    let completed = 0;
    let succeeded = 0;
    let failed = 0;

    // Build a shared queue
    const queue = [...items.entries()]; // [index, item][]

    this.logger?.info('Batch processing started', { total, concurrency: this.concurrency });

    // Worker pump: each worker pulls from the queue until empty
    const runWorker = async (): Promise<void> => {
      while (queue.length > 0) {
        const next = queue.shift();
        if (!next) break;
        const [index, item] = next;

        const itemStart = Date.now();
        try {
          const value = await worker(item);
          const durationMs = Date.now() - itemStart;
          results[index] = { item, status: 'fulfilled', value, durationMs };
          succeeded++;
          this.logger?.debug('Batch item succeeded', { index, value, durationMs });
        } catch (error) {
          const durationMs = Date.now() - itemStart;
          results[index] = { item, status: 'rejected', error, durationMs };
          failed++;
          this.logger?.warn('Batch item failed', {
            index,
            error: error instanceof Error ? error.message : String(error),
            durationMs,
          });
        }

        completed++;
        this.onProgress?.({ completed, total, succeeded, failed });
      }
    };

    // Spin up `concurrency` workers and wait for all to drain
    const workers = Array.from({ length: Math.min(this.concurrency, total) }, () => runWorker());
    await Promise.all(workers);

    const totalDurationMs = Date.now() - batchStart;
    this.logger?.logPerformance('batch_process', totalDurationMs, { total, succeeded, failed });

    return { results, succeeded, failed, totalDurationMs };
  }

  /**
   * Convenience: like `process` but throws if any item failed.
   */
  async processStrict(items: BatchItem<T>[], worker: WorkerFn<T>): Promise<BatchResult<T>[]> {
    const result = await this.process(items, worker);
    if (result.failed > 0) {
      const firstError = result.results.find((r) => r.status === 'rejected')?.error;
      throw new BatchProcessingError(
        `Batch processing failed: ${result.failed}/${result.results.length} items rejected`,
        result.results,
        firstError,
      );
    }
    return result.results;
  }
}

export class BatchProcessingError extends Error {
  readonly results: BatchResult<unknown>[];
  readonly cause: unknown;

  constructor(message: string, results: BatchResult<unknown>[], cause?: unknown) {
    super(message);
    this.name = 'BatchProcessingError';
    this.results = results;
    this.cause = cause;
  }
}
