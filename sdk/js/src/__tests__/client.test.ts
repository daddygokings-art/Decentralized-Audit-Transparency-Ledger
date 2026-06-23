import { describe, it, expect } from 'vitest';
import { AuditLedgerClient } from '../AuditLedgerClient';

describe('AuditLedgerClient', () => {
  it('calls transport for totalEvents', async () => {
    const transport = async (method: string, params: any[]) => {
      if (method === 'total_events') return 42;
      return null;
    };
    const c = new AuditLedgerClient(transport);
    const total = await c.totalEvents();
    expect(total).toBe(42);
  });
});
