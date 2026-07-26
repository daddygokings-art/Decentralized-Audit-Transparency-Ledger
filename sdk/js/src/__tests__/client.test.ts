import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AuditLedgerClient } from '../AuditLedgerClient';
import { AuditLedgerError } from '../types';

function makeEvent(index: number): any {
  return {
    index,
    timestamp: 1_700_000_000 + index,
    event_type: 'payment',
    submitter: index % 2 === 0 ? 'GABC' : 'GXYZ',
    metadata: `payload-${index}`,
    event_hash: '00'.repeat(32),
    prev_hash: '11'.repeat(32),
  };
}

describe('AuditLedgerClient', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('calls transport for totalEvents', async () => {
    const transport = async (method: string, params: any[]) => {
      if (method === 'total_events') return 42;
      return null;
    };
    const c = new AuditLedgerClient(transport);
    const total = await c.totalEvents();
    expect(total).toBe(42);
  });

  it('calls transport for logEvents', async () => {
    const transport = async (method: string, params: any[]) => {
      if (method === 'log_events') return [0, 1, 2];
      return null;
    };
    const c = new AuditLedgerClient(transport);
    const indices = await c.logEvents([
      { submitter: 'GABC', type: 'payment', metadata: 'data' },
    ]);
    expect(indices).toEqual([0, 1, 2]);
  });

  it('retries network errors with exponential backoff', async () => {
    const transport = vi.fn()
      .mockRejectedValueOnce(new TypeError('network down'))
      .mockRejectedValueOnce(new TypeError('still down'))
      .mockResolvedValueOnce(42);

    const c = new AuditLedgerClient(transport);
    const promise = c.totalEvents();

    await vi.advanceTimersByTimeAsync(500);
    await vi.advanceTimersByTimeAsync(1000);

    await expect(promise).resolves.toBe(42);
    expect(transport).toHaveBeenCalledTimes(3);
  });

  it('retries 429 and stops after maxRetries', async () => {
    const transport = vi.fn()
      .mockRejectedValueOnce(new AuditLedgerError('rate limited', undefined, 429))
      .mockRejectedValueOnce(new AuditLedgerError('still limited', undefined, 429))
      .mockResolvedValueOnce(7);

    const c = new AuditLedgerClient(transport, undefined, { maxRetries: 2, baseDelayMs: 25 });
    const promise = c.eventCount('payment');

    await vi.advanceTimersByTimeAsync(25);
    await vi.advanceTimersByTimeAsync(50);

    await expect(promise).resolves.toBe(7);
    expect(transport).toHaveBeenCalledTimes(3);
  });

  it('does not retry non-retryable status codes', async () => {
    const transport = vi.fn().mockRejectedValue(new AuditLedgerError('bad request', undefined, 400));
    const c = new AuditLedgerClient(transport, undefined, { maxRetries: 3, baseDelayMs: 10 });

    await expect(c.getEvent('1')).rejects.toThrow('bad request');
    expect(transport).toHaveBeenCalledTimes(1);
  });

  it('supports paginated retrieval with cursor-based offsets', async () => {
    const transport = vi.fn().mockImplementation(async (method: string, params: any[]) => {
      if (method === 'total_events') return 5;
      if (method === 'get_event_by_order') return makeEvent(params[0]);
      return null;
    });
    const client = new AuditLedgerClient(transport);
    const page = await client.getEvents(0, 3, 2);
    expect(page.items.map((event) => event.index)).toEqual([2, 3, 4]);
    expect(page.offset).toBe(2);
  });

  it('filters and exports events', () => {
    const client = new AuditLedgerClient(async () => null);
    const events = [makeEvent(0), makeEvent(1), makeEvent(2)];
    const filtered = client.filterEvents(events, { eventType: 'payment', submitter: 'GABC', metadataQuery: '0' });
    expect(filtered.map((event) => event.index)).toEqual([0]);
    const csv = client.exportEvents([events[0]], 'csv');
    expect(csv.startsWith('index,timestamp')).toBe(true);
    const json = client.exportEvents([events[0]], 'json');
    expect(JSON.parse(json)[0].event_type).toBe('payment');
  });

  it('streams events with progress updates', async () => {
    const transport = vi.fn().mockImplementation(async (method: string, params: any[]) => {
      if (method === 'total_events') return 2;
      if (method === 'get_event_by_order') return makeEvent(params[0]);
      return null;
    });
    const client = new AuditLedgerClient(transport);
    const progress: Array<{ completed: number; total: number }> = [];
    const exported = client.exportEvents([makeEvent(0), makeEvent(1)], 'json', true, (p) => progress.push(p));
    expect(JSON.parse(exported).length).toBe(2);
    expect(progress[progress.length - 1]).toEqual({ completed: 2, total: 2 });
  });

  it('tracks cache hits and invalidates cache entries', async () => {
    const transport = vi.fn().mockImplementation(async (method: string, params: any[]) => {
      if (method === 'get_event_by_order') return makeEvent(params[0]);
      return null;
    });
    const client = new AuditLedgerClient(transport);
    await client.getEventByOrder(0);
    await client.getEventByOrder(0);
    const stats = client.cacheStats();
    expect(stats.hits).toBe(1);
    expect(stats.size).toBe(1);
    client.invalidateCache();
    expect(client.cacheStats().size).toBe(0);
  });
});
