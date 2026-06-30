import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AuditLedgerClient } from '../AuditLedgerClient';
import { AuditLedgerError } from '../types';

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
});
