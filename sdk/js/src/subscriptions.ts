/**
 * #231 — SDK Event Subscription Management
 *
 * Manages event stream subscriptions: creation, cancellation, filtering,
 * state tracking, and statistics.
 */

import { Event } from './types';
import { Logger } from './logger';

export type SubscriptionState = 'active' | 'paused' | 'cancelled';

export interface SubscriptionFilter {
  /** Only deliver events whose event_type matches this value */
  eventType?: string;
  /** Only deliver events from this submitter address */
  submitter?: string;
  /** Only deliver events at or after this timestamp (ms) */
  fromTimestamp?: number;
  /** Only deliver events at or before this timestamp (ms) */
  toTimestamp?: number;
  /** Custom predicate — return true to accept the event */
  predicate?: (event: Event) => boolean;
}

export interface Subscription {
  readonly id: string;
  readonly filter: SubscriptionFilter;
  state: SubscriptionState;
  /** Total events delivered to this subscription */
  delivered: number;
  /** Total events filtered out (received but not delivered) */
  filtered: number;
  /** Unix-ms timestamp when the subscription was created */
  readonly createdAt: number;
  /** Unix-ms timestamp of the last delivered event (or null) */
  lastDeliveredAt: number | null;
}

export interface SubscriptionStats {
  total: number;
  active: number;
  paused: number;
  cancelled: number;
  totalDelivered: number;
  totalFiltered: number;
}

export type EventCallback = (event: Event, subscription: Subscription) => void;

export interface SubscriptionOptions {
  filter?: SubscriptionFilter;
  /** Start in paused state (default: false) */
  paused?: boolean;
}

/**
 * Manages client-side event subscriptions.
 *
 * Usage:
 *   const mgr = new SubscriptionManager();
 *   const sub = mgr.subscribe({ filter: { eventType: 'payment' } }, (event, sub) => { … });
 *   mgr.publish(event);           // dispatch to all matching active subscriptions
 *   mgr.cancel(sub.id);
 */
export class SubscriptionManager {
  private subscriptions: Map<string, Subscription> = new Map();
  private callbacks: Map<string, EventCallback> = new Map();
  private counter = 0;
  private logger?: Logger;

  constructor(options: { logger?: Logger } = {}) {
    this.logger = options.logger;
  }

  /**
   * Create a new subscription and return its descriptor.
   */
  subscribe(options: SubscriptionOptions, callback: EventCallback): Subscription {
    const id = `sub_${++this.counter}_${Date.now()}`;
    const sub: Subscription = {
      id,
      filter: options.filter ?? {},
      state: options.paused ? 'paused' : 'active',
      delivered: 0,
      filtered: 0,
      createdAt: Date.now(),
      lastDeliveredAt: null,
    };
    this.subscriptions.set(id, sub);
    this.callbacks.set(id, callback);
    this.logger?.info('Subscription created', { id, filter: sub.filter, state: sub.state });
    return sub;
  }

  /**
   * Cancel a subscription by ID. Cancelled subscriptions no longer receive events.
   */
  cancel(id: string): boolean {
    const sub = this.subscriptions.get(id);
    if (!sub) return false;
    sub.state = 'cancelled';
    this.logger?.info('Subscription cancelled', { id });
    return true;
  }

  /**
   * Pause a subscription (stops delivery without removing it).
   */
  pause(id: string): boolean {
    const sub = this.subscriptions.get(id);
    if (!sub || sub.state === 'cancelled') return false;
    sub.state = 'paused';
    this.logger?.debug('Subscription paused', { id });
    return true;
  }

  /**
   * Resume a paused subscription.
   */
  resume(id: string): boolean {
    const sub = this.subscriptions.get(id);
    if (!sub || sub.state === 'cancelled') return false;
    sub.state = 'active';
    this.logger?.debug('Subscription resumed', { id });
    return true;
  }

  /**
   * Remove a subscription entirely (after cancelling it).
   */
  remove(id: string): boolean {
    this.cancel(id);
    const existed = this.subscriptions.has(id);
    this.subscriptions.delete(id);
    this.callbacks.delete(id);
    this.logger?.debug('Subscription removed', { id });
    return existed;
  }

  /**
   * Retrieve a subscription descriptor by ID.
   */
  get(id: string): Subscription | undefined {
    return this.subscriptions.get(id);
  }

  /**
   * List all subscriptions, optionally filtered by state.
   */
  list(state?: SubscriptionState): Subscription[] {
    const all = Array.from(this.subscriptions.values());
    return state ? all.filter((s) => s.state === state) : all;
  }

  /**
   * Publish an event to all active subscriptions whose filters match.
   */
  publish(event: Event): void {
    for (const [id, sub] of this.subscriptions) {
      if (sub.state !== 'active') continue;
      if (this.matchesFilter(event, sub.filter)) {
        sub.delivered++;
        sub.lastDeliveredAt = Date.now();
        const cb = this.callbacks.get(id);
        try {
          cb?.(event, sub);
        } catch (err) {
          this.logger?.error('Subscription callback error', {
            id,
            error: err instanceof Error ? err.message : String(err),
          });
        }
      } else {
        sub.filtered++;
      }
    }
  }

  /**
   * Return aggregate statistics across all subscriptions.
   */
  getStats(): SubscriptionStats {
    let active = 0, paused = 0, cancelled = 0, totalDelivered = 0, totalFiltered = 0;
    for (const sub of this.subscriptions.values()) {
      if (sub.state === 'active') active++;
      else if (sub.state === 'paused') paused++;
      else if (sub.state === 'cancelled') cancelled++;
      totalDelivered += sub.delivered;
      totalFiltered += sub.filtered;
    }
    return {
      total: this.subscriptions.size,
      active,
      paused,
      cancelled,
      totalDelivered,
      totalFiltered,
    };
  }

  /**
   * Cancel and remove all subscriptions.
   */
  clear(): void {
    for (const id of this.subscriptions.keys()) {
      this.cancel(id);
    }
    this.subscriptions.clear();
    this.callbacks.clear();
    this.logger?.info('All subscriptions cleared');
  }

  // ── Filter matching ──────────────────────────────────────────────────────

  private matchesFilter(event: Event, filter: SubscriptionFilter): boolean {
    if (filter.eventType !== undefined && event.event_type !== filter.eventType) return false;
    if (filter.submitter !== undefined && event.submitter !== filter.submitter) return false;
    if (filter.fromTimestamp !== undefined && event.timestamp < filter.fromTimestamp) return false;
    if (filter.toTimestamp !== undefined && event.timestamp > filter.toTimestamp) return false;
    if (filter.predicate !== undefined && !filter.predicate(event)) return false;
    return true;
  }
}
