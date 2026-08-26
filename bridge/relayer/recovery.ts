/**
 * Bridge Error Recovery (#258)
 *
 * Classifies relayer errors, applies a recovery strategy per error class,
 * routes exhausted items to a dead letter queue, and notifies observers.
 */

// ── Error classification ─────────────────────────────────────────────────────

export type ErrorCategory = "network" | "rate_limit" | "validation" | "contract" | "unknown";

export interface ClassifiedError {
  category: ErrorCategory;
  message: string;
  retryable: boolean;
}

const CATEGORY_PATTERNS: Array<{ category: ErrorCategory; retryable: boolean; test: (msg: string) => boolean }> = [
  {
    category: "network",
    retryable: true,
    test: (msg) => /ECONNREFUSED|ETIMEDOUT|ENOTFOUND|ECONNRESET|socket hang up|network/i.test(msg),
  },
  {
    category: "rate_limit",
    retryable: true,
    test: (msg) => /rate limit|too many requests|429/i.test(msg),
  },
  {
    category: "validation",
    retryable: false,
    test: (msg) => /invalid|missing required field|validation/i.test(msg),
  },
  {
    category: "contract",
    retryable: false,
    test: (msg) => /revert|InvalidProof|execution reverted|contract/i.test(msg),
  },
];

export function classifyError(err: unknown): ClassifiedError {
  const message = err instanceof Error ? err.message : String(err);

  for (const pattern of CATEGORY_PATTERNS) {
    if (pattern.test(message)) {
      return { category: pattern.category, message, retryable: pattern.retryable };
    }
  }

  return { category: "unknown", message, retryable: false };
}

// ── Recovery strategies ───────────────────────────────────────────────────────

export interface RecoveryStrategy {
  maxRetries: number;
  backoffMs: (attempt: number) => number;
}

const DEFAULT_STRATEGIES: Record<ErrorCategory, RecoveryStrategy> = {
  network: { maxRetries: 5, backoffMs: (attempt) => Math.min(30_000, 500 * 2 ** attempt) },
  rate_limit: { maxRetries: 5, backoffMs: (attempt) => Math.min(60_000, 1_000 * 2 ** attempt) },
  validation: { maxRetries: 0, backoffMs: () => 0 },
  contract: { maxRetries: 1, backoffMs: () => 2_000 },
  unknown: { maxRetries: 2, backoffMs: (attempt) => 1_000 * (attempt + 1) },
};

export interface RecoveryDecision {
  shouldRetry: boolean;
  delayMs: number;
  attempt: number;
}

export class RecoveryPlanner {
  private strategies: Record<ErrorCategory, RecoveryStrategy>;
  private attempts: Map<string, number> = new Map();

  constructor(overrides: Partial<Record<ErrorCategory, RecoveryStrategy>> = {}) {
    this.strategies = { ...DEFAULT_STRATEGIES, ...overrides };
  }

  /** Decides whether `key` (e.g. an event hash) should be retried for the given error. */
  decide(key: string, error: ClassifiedError): RecoveryDecision {
    const strategy = this.strategies[error.category];
    const attempt = this.attempts.get(key) ?? 0;

    if (!error.retryable || attempt >= strategy.maxRetries) {
      return { shouldRetry: false, delayMs: 0, attempt };
    }

    this.attempts.set(key, attempt + 1);
    return { shouldRetry: true, delayMs: strategy.backoffMs(attempt), attempt: attempt + 1 };
  }

  clear(key: string): void {
    this.attempts.delete(key);
  }

  attemptsFor(key: string): number {
    return this.attempts.get(key) ?? 0;
  }
}

// ── Dead letter queue ─────────────────────────────────────────────────────────

export interface DeadLetterEntry<T> {
  item: T;
  error: ClassifiedError;
  attempts: number;
  firstFailedAt: number;
  lastFailedAt: number;
}

export class DeadLetterQueue<T> {
  private entries: DeadLetterEntry<T>[] = [];
  private maxSize: number;

  constructor(maxSize: number = 1000) {
    this.maxSize = maxSize;
  }

  push(item: T, error: ClassifiedError, attempts: number, now: number = Date.now()): void {
    this.entries.push({ item, error, attempts, firstFailedAt: now, lastFailedAt: now });
    if (this.entries.length > this.maxSize) {
      this.entries.shift();
    }
  }

  size(): number {
    return this.entries.length;
  }

  list(): DeadLetterEntry<T>[] {
    return [...this.entries];
  }

  /** Removes and returns all entries so they can be requeued elsewhere. */
  drain(): DeadLetterEntry<T>[] {
    const drained = this.entries;
    this.entries = [];
    return drained;
  }

  clear(): void {
    this.entries = [];
  }
}

// ── Error notification ────────────────────────────────────────────────────────

export interface ErrorNotification {
  category: ErrorCategory;
  message: string;
  key: string;
  attempts: number;
  timestamp: number;
}

export type ErrorNotifier = (notification: ErrorNotification) => void;

export class ErrorNotificationHub {
  private listeners: ErrorNotifier[] = [];

  subscribe(listener: ErrorNotifier): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((l) => l !== listener);
    };
  }

  notify(key: string, error: ClassifiedError, attempts: number, now: number = Date.now()): void {
    const notification: ErrorNotification = {
      category: error.category,
      message: error.message,
      key,
      attempts,
      timestamp: now,
    };
    for (const listener of this.listeners) {
      try {
        listener(notification);
      } catch (err) {
        console.error("[recovery] notifier threw:", err);
      }
    }
  }
}

export function consoleNotifier(notification: ErrorNotification): void {
  console.error(
    `[recovery] ${notification.category} error for ${notification.key} (attempt ${notification.attempts}): ${notification.message}`
  );
}

// ── Orchestration ─────────────────────────────────────────────────────────────

/**
 * Ties classification, retry planning, the dead letter queue, and
 * notification together for a single failed operation.
 */
export class ErrorRecoveryManager<T> {
  readonly planner: RecoveryPlanner;
  readonly deadLetterQueue: DeadLetterQueue<T>;
  readonly notifications: ErrorNotificationHub;

  constructor(options: {
    strategies?: Partial<Record<ErrorCategory, RecoveryStrategy>>;
    deadLetterMaxSize?: number;
  } = {}) {
    this.planner = new RecoveryPlanner(options.strategies);
    this.deadLetterQueue = new DeadLetterQueue<T>(options.deadLetterMaxSize);
    this.notifications = new ErrorNotificationHub();
  }

  handle(key: string, item: T, err: unknown): RecoveryDecision {
    const classified = classifyError(err);
    const decision = this.planner.decide(key, classified);

    this.notifications.notify(key, classified, decision.attempt);

    if (!decision.shouldRetry) {
      this.deadLetterQueue.push(item, classified, decision.attempt);
      this.planner.clear(key);
    }

    return decision;
  }

  resolved(key: string): void {
    this.planner.clear(key);
  }
}
