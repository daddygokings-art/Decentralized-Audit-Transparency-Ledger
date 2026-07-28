/**
 * Bridge Event Replay (#253)
 *
 * Implements event replay for failed bridge submissions, including:
 *   - Replay queue with persistent in-memory state
 *   - Retry logic with exponential backoff
 *   - Replay from a specific event index
 *   - Replay statistics
 */

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ReplayQueueItem {
  /** Unique event hash (keccak256 of event data). */
  eventHash: string;
  /** Original event index on Stellar. */
  eventIndex: number;
  /** Serialised event payload to replay. */
  payload: string;
  /** ISO timestamp of first failure. */
  firstFailedAt: string;
  /** ISO timestamp of most recent attempt. */
  lastAttemptAt: string;
  /** Number of attempts so far (including the initial one). */
  attempts: number;
  /** Unix ms timestamp after which the next attempt is allowed. */
  nextRetryAfter: number;
  /** Last error message, for diagnostics. */
  lastError: string;
}

export interface ReplayStats {
  /** Total items ever enqueued. */
  totalEnqueued: number;
  /** Items currently in the queue (pending retry). */
  queueDepth: number;
  /** Items successfully replayed. */
  totalSucceeded: number;
  /** Items permanently failed (exceeded max retries). */
  totalFailed: number;
  /** Total replay attempts performed. */
  totalAttempts: number;
  /** Timestamp (ISO) of the last successful replay. */
  lastSuccessAt: string | null;
  /** Timestamp (ISO) of the last failure. */
  lastFailureAt: string | null;
}

export interface ReplayConfig {
  /** Maximum number of retry attempts before giving up. Default: 5 */
  maxRetries: number;
  /** Base delay for exponential backoff in ms. Default: 1000 */
  baseDelayMs: number;
  /** Maximum delay cap in ms. Default: 60_000 */
  maxDelayMs: number;
  /** Maximum queue depth before oldest items are dropped. Default: 5000 */
  maxQueueSize: number;
}

const DEFAULT_REPLAY_CONFIG: ReplayConfig = {
  maxRetries: 5,
  baseDelayMs: 1000,
  maxDelayMs: 60_000,
  maxQueueSize: 5000,
};

// ── Exponential backoff helper ────────────────────────────────────────────────

/**
 * Calculates the delay (ms) for the given attempt number using
 * full jitter exponential backoff: delay = random(0, min(maxDelay, base * 2^attempt))
 */
export function calcBackoffDelay(
  attempt: number,
  baseDelayMs: number,
  maxDelayMs: number
): number {
  const cap = Math.min(maxDelayMs, baseDelayMs * Math.pow(2, attempt));
  return Math.floor(Math.random() * cap);
}

// ── Replay Queue ──────────────────────────────────────────────────────────────

/**
 * ReplayQueue manages failed bridge submissions and retries them with
 * exponential backoff up to a configurable retry limit.
 *
 * Usage:
 *   const queue = new ReplayQueue(config, submitFn);
 *   queue.enqueue(eventHash, eventIndex, payload);
 *   await queue.processOnce();           // drain one pass
 *   queue.startAutoProcess(5000);        // poll every 5 s
 */
export class ReplayQueue {
  private queue: Map<string, ReplayQueueItem> = new Map();
  private config: ReplayConfig;
  private submitFn: (eventHash: string, payload: string) => Promise<void>;
  private timer: ReturnType<typeof setInterval> | null = null;

  private stats: ReplayStats = {
    totalEnqueued: 0,
    queueDepth: 0,
    totalSucceeded: 0,
    totalFailed: 0,
    totalAttempts: 0,
    lastSuccessAt: null,
    lastFailureAt: null,
  };

  constructor(
    submitFn: (eventHash: string, payload: string) => Promise<void>,
    config: Partial<ReplayConfig> = {}
  ) {
    this.submitFn = submitFn;
    this.config = { ...DEFAULT_REPLAY_CONFIG, ...config };
  }

  // ── Queue management ────────────────────────────────────────────────────────

  /**
   * Adds a failed event to the replay queue.
   * If the event is already in the queue, the entry is retained unchanged
   * (re-enqueueing is a no-op to avoid resetting the attempt counter).
   */
  enqueue(eventHash: string, eventIndex: number, payload: string, error = ""): void {
    if (this.queue.has(eventHash)) return;

    // Drop oldest item if at capacity
    if (this.queue.size >= this.config.maxQueueSize) {
      const oldestKey = this.queue.keys().next().value;
      if (oldestKey !== undefined) {
        this.queue.delete(oldestKey);
        console.warn(`[replay] queue full — dropped oldest item (hash ${oldestKey})`);
      }
    }

    const now = new Date().toISOString();
    const item: ReplayQueueItem = {
      eventHash,
      eventIndex,
      payload,
      firstFailedAt: now,
      lastAttemptAt: now,
      attempts: 1,
      nextRetryAfter: Date.now() + calcBackoffDelay(1, this.config.baseDelayMs, this.config.maxDelayMs),
      lastError: error,
    };

    this.queue.set(eventHash, item);
    this.stats.totalEnqueued++;
    this.stats.queueDepth = this.queue.size;

    console.log(
      `[replay] enqueued event #${eventIndex} (hash ${eventHash}), ` +
      `next retry in ~${Math.round((item.nextRetryAfter - Date.now()) / 1000)}s`
    );
  }

  /**
   * Removes an item from the queue (e.g. after manual resolution).
   */
  remove(eventHash: string): boolean {
    const deleted = this.queue.delete(eventHash);
    this.stats.queueDepth = this.queue.size;
    return deleted;
  }

  /**
   * Returns a read-only snapshot of the queue.
   */
  getQueue(): ReadonlyArray<ReplayQueueItem> {
    return Array.from(this.queue.values());
  }

  /**
   * Returns current replay statistics.
   */
  getStats(): Readonly<ReplayStats> {
    return { ...this.stats, queueDepth: this.queue.size };
  }

  // ── Processing ──────────────────────────────────────────────────────────────

  /**
   * Processes all items whose `nextRetryAfter` has passed.
   * Items that succeed are removed. Items that fail are rescheduled
   * with backoff, or permanently discarded after maxRetries.
   */
  async processOnce(): Promise<void> {
    const now = Date.now();
    const due = Array.from(this.queue.values()).filter(
      (item) => item.nextRetryAfter <= now
    );

    for (const item of due) {
      this.stats.totalAttempts++;
      item.lastAttemptAt = new Date().toISOString();

      try {
        await this.submitFn(item.eventHash, item.payload);

        // Success — remove from queue
        this.queue.delete(item.eventHash);
        this.stats.totalSucceeded++;
        this.stats.lastSuccessAt = new Date().toISOString();
        this.stats.queueDepth = this.queue.size;

        console.log(
          `[replay] event #${item.eventIndex} replayed successfully ` +
          `(attempt ${item.attempts})`
        );
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        item.lastError = errorMsg;
        item.attempts++;

        if (item.attempts > this.config.maxRetries) {
          // Give up
          this.queue.delete(item.eventHash);
          this.stats.totalFailed++;
          this.stats.lastFailureAt = new Date().toISOString();
          this.stats.queueDepth = this.queue.size;

          console.error(
            `[replay] event #${item.eventIndex} permanently failed after ` +
            `${item.attempts - 1} attempts: ${errorMsg}`
          );
        } else {
          // Schedule next retry with backoff
          item.nextRetryAfter =
            Date.now() +
            calcBackoffDelay(
              item.attempts,
              this.config.baseDelayMs,
              this.config.maxDelayMs
            );

          console.warn(
            `[replay] event #${item.eventIndex} failed (attempt ${item.attempts - 1}/${this.config.maxRetries}): ` +
            `${errorMsg}. Retry in ~${Math.round((item.nextRetryAfter - Date.now()) / 1000)}s`
          );
        }
      }
    }
  }

  /**
   * Starts a background interval that calls processOnce() periodically.
   * @param intervalMs  Poll interval in milliseconds. Default: 5000.
   */
  startAutoProcess(intervalMs = 5000): void {
    if (this.timer) return;
    this.timer = setInterval(() => {
      this.processOnce().catch((err) =>
        console.error("[replay] processOnce error:", err)
      );
    }, intervalMs);
    console.log(`[replay] auto-processing started (interval: ${intervalMs}ms)`);
  }

  /**
   * Stops the background processing interval.
   */
  stopAutoProcess(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
      console.log("[replay] auto-processing stopped");
    }
  }

  // ── Replay from specific index (#253) ────────────────────────────────────────

  /**
   * Re-enqueues all events whose `eventIndex` is >= `fromIndex` by fetching
   * them from the provided source function and submitting them for replay.
   *
   * This allows an operator to replay events that were missed due to a gap,
   * restarts, or a bug, starting from a known-good checkpoint.
   *
   * @param fromIndex   The first event index to replay (inclusive).
   * @param fetchFn     Function that returns raw event payloads for indices >= fromIndex.
   */
  async replayFromIndex(
    fromIndex: number,
    fetchFn: (fromIndex: number) => Promise<Array<{ eventIndex: number; eventHash: string; payload: string }>>
  ): Promise<number> {
    console.log(`[replay] replaying all events from index ${fromIndex}`);

    const events = await fetchFn(fromIndex);
    let enqueued = 0;

    for (const event of events) {
      if (!this.queue.has(event.eventHash)) {
        this.enqueue(event.eventHash, event.eventIndex, event.payload, "manual replay");
        enqueued++;
      }
    }

    console.log(`[replay] replayFromIndex(${fromIndex}): enqueued ${enqueued} event(s)`);
    return enqueued;
  }
}

// ── Module export ─────────────────────────────────────────────────────────────

export { DEFAULT_REPLAY_CONFIG };
