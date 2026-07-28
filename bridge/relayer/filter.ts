/**
 * Bridge Event Filtering (#255)
 *
 * Provides selective bridging by filtering events on type, submitter,
 * and time range before they are handed to proof generation / submission.
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

export interface EventTypeFilter {
  include?: string[];
  exclude?: string[];
}

export interface SubmitterFilter {
  include?: string[];
  exclude?: string[];
}

export interface TimeRangeFilter {
  fromTimestamp?: number;
  toTimestamp?: number;
}

export interface FilterConfig {
  eventType?: EventTypeFilter;
  submitter?: SubmitterFilter;
  timeRange?: TimeRangeFilter;
}

export interface FilterResult {
  passed: AuditEvent[];
  rejected: Array<{ event: AuditEvent; reason: string }>;
}

// ── Filter implementations ────────────────────────────────────────────────────

function matchesEventType(event: AuditEvent, filter?: EventTypeFilter): string | null {
  if (!filter) return null;

  if (filter.include && filter.include.length > 0 && !filter.include.includes(event.event_type)) {
    return `event_type '${event.event_type}' not in include list`;
  }

  if (filter.exclude && filter.exclude.includes(event.event_type)) {
    return `event_type '${event.event_type}' is excluded`;
  }

  return null;
}

function matchesSubmitter(event: AuditEvent, filter?: SubmitterFilter): string | null {
  if (!filter) return null;

  if (filter.include && filter.include.length > 0 && !filter.include.includes(event.submitter)) {
    return `submitter '${event.submitter}' not in include list`;
  }

  if (filter.exclude && filter.exclude.includes(event.submitter)) {
    return `submitter '${event.submitter}' is excluded`;
  }

  return null;
}

function matchesTimeRange(event: AuditEvent, filter?: TimeRangeFilter): string | null {
  if (!filter) return null;

  if (filter.fromTimestamp !== undefined && event.timestamp < filter.fromTimestamp) {
    return `timestamp ${event.timestamp} before fromTimestamp ${filter.fromTimestamp}`;
  }

  if (filter.toTimestamp !== undefined && event.timestamp > filter.toTimestamp) {
    return `timestamp ${event.timestamp} after toTimestamp ${filter.toTimestamp}`;
  }

  return null;
}

// ── Filter class ──────────────────────────────────────────────────────────────

export class EventFilter {
  private config: FilterConfig;

  constructor(config: FilterConfig = {}) {
    this.config = { ...config };
  }

  configure(config: Partial<FilterConfig>): void {
    this.config = { ...this.config, ...config };
  }

  getConfig(): FilterConfig {
    return { ...this.config };
  }

  reset(): void {
    this.config = {};
  }

  /** Returns the rejection reason for an event, or null if it passes all filters. */
  test(event: AuditEvent): string | null {
    return (
      matchesEventType(event, this.config.eventType) ??
      matchesSubmitter(event, this.config.submitter) ??
      matchesTimeRange(event, this.config.timeRange) ??
      null
    );
  }

  matches(event: AuditEvent): boolean {
    return this.test(event) === null;
  }

  apply(events: AuditEvent[]): FilterResult {
    const passed: AuditEvent[] = [];
    const rejected: Array<{ event: AuditEvent; reason: string }> = [];

    for (const event of events) {
      const reason = this.test(event);
      if (reason === null) {
        passed.push(event);
      } else {
        rejected.push({ event, reason });
      }
    }

    return { passed, rejected };
  }
}

// ── Utility constructors ──────────────────────────────────────────────────────

export function createEventTypeFilter(include?: string[], exclude?: string[]): EventFilter {
  return new EventFilter({ eventType: { include, exclude } });
}

export function createSubmitterFilter(include?: string[], exclude?: string[]): EventFilter {
  return new EventFilter({ submitter: { include, exclude } });
}

export function createTimeRangeFilter(fromTimestamp?: number, toTimestamp?: number): EventFilter {
  return new EventFilter({ timeRange: { fromTimestamp, toTimestamp } });
}
