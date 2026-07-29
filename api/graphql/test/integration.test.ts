import { describe, it, expect, beforeEach, vi } from 'vitest';
import { requireRole, Role } from '../src/auth';

// Mock resolvers with an in-memory store
const events: any[] = [];
const resolvers = {
  Query: {
    events: (_: any, { limit, offset }: any) => events.slice(offset, offset + limit),
    event: (_: any, { index }: any, ctx: any) => {
      requireRole(ctx, Role.Viewer);
      return events.find((e: any) => e.index === index) || null;
    },
    eventByType: (_: any, { type, typeIndex }: any, ctx: any) => {
      requireRole(ctx, Role.Viewer);
      const filtered = events.filter((e: any) => e.event_type === type);
      return filtered[typeIndex] || null;
    },
    statistics: (_: any, __: any, ctx: any) => {
      requireRole(ctx, Role.Auditor);
      return {
        totalEvents: events.length,
        globalMaxLogs: 1000,
        eventsByType: events.reduce((acc: any, e: any) => {
          acc[e.event_type] = (acc[e.event_type] || 0) + 1;
          return acc;
        }, {}),
      };
    },
    searchEvents: (_: any, { query }: any) =>
      events.filter((e: any) => e.metadata.toLowerCase().includes(query.toLowerCase())),
    governanceHistory: (_: any, { types, limit = 50, offset = 0 }: any) => {
      const filtered = types && types.length > 0
        ? [{ action: 'transfer_ownership', caller: 'GA', timestamp: 1000 }].filter((g) => types.includes(g.action))
        : [{ action: 'transfer_ownership', caller: 'GA', timestamp: 1000 }];
      return filtered.slice(offset, offset + limit);
    },
  },
  Mutation: {
    logEvent: (_: any, { input }: any) => {
      const evt = {
        index: events.length,
        timestamp: Math.floor(Date.now() / 1000),
        event_type: input.event_type,
        submitter: input.submitter,
        metadata: input.metadata,
        event_hash: '0x' + '00'.repeat(32),
        prev_hash: events.length === 0 ? '0x' + '00'.repeat(32) : events[events.length - 1].event_hash,
      };
      events.push(evt);
      return evt;
    },
  },
};

const viewerCtx = { role: Role.Viewer };
const auditorCtx = { role: Role.Auditor };
const adminCtx = { role: Role.Admin };

describe('GraphQL API Integration Tests', () => {
  beforeEach(() => {
    events.length = 0;
  });

  describe('Public Queries (no auth required)', () => {
    it('should return empty events list initially without auth', () => {
      const result = resolvers.Query.events(null, { limit: 50, offset: 0 });
      expect(result).toEqual([]);
    });

    it('should return paginated events without auth', () => {
      for (let i = 0; i < 10; i++) {
        resolvers.Mutation.logEvent(null, {
          input: { event_type: 'payment', submitter: 'GA', metadata: `tx${i}` },
        });
      }
      const result = resolvers.Query.events(null, { limit: 5, offset: 0 });
      expect(result).toHaveLength(5);
    });

    it('should support searchEvents without auth', () => {
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'invoice-001' },
      });
      const result = resolvers.Query.searchEvents(null, { query: 'invoice' });
      expect(result).toHaveLength(1);
    });

    it('should support governanceHistory without auth', () => {
      const result = resolvers.Query.governanceHistory(null, { types: ['transfer_ownership'], limit: 50, offset: 0 });
      expect(result).toHaveLength(1);
    });
  });

  describe('Event Query (gated by Viewer role)', () => {
    beforeEach(() => {
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'tx0' },
      });
    });

    it('should reject event query without auth', () => {
      expect(() => resolvers.Query.event(null, { index: 0 }, null)).toThrow('Forbidden');
    });

    it('should reject event query with empty context', () => {
      expect(() => resolvers.Query.event(null, { index: 0 }, {})).toThrow('Forbidden');
    });

    it('should allow event query with Viewer role', () => {
      const result = resolvers.Query.event(null, { index: 0 }, viewerCtx);
      expect(result).not.toBeNull();
      expect(result.index).toBe(0);
    });

    it('should allow event query with Auditor role', () => {
      const result = resolvers.Query.event(null, { index: 0 }, auditorCtx);
      expect(result).not.toBeNull();
      expect(result.index).toBe(0);
    });

    it('should allow event query with Admin role', () => {
      const result = resolvers.Query.event(null, { index: 0 }, adminCtx);
      expect(result).not.toBeNull();
      expect(result.index).toBe(0);
    });

    it('should return null for non-existent index', () => {
      const result = resolvers.Query.event(null, { index: 999 }, viewerCtx);
      expect(result).toBeNull();
    });
  });

  describe('Event By Type Query (gated by Viewer role)', () => {
    beforeEach(() => {
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'p1' },
      });
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'refund', submitter: 'GA', metadata: 'r1' },
      });
    });

    it('should reject without auth', () => {
      expect(() => resolvers.Query.eventByType(null, { type: 'payment', typeIndex: 0 }, null)).toThrow('Forbidden');
    });

    it('should allow with Viewer role', () => {
      const result = resolvers.Query.eventByType(null, { type: 'payment', typeIndex: 0 }, viewerCtx);
      expect(result.event_type).toBe('payment');
    });
  });

  describe('Statistics Query (gated by Auditor role)', () => {
    beforeEach(() => {
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'p1' },
      });
    });

    it('should reject without auth', () => {
      expect(() => resolvers.Query.statistics(null, {}, null)).toThrow('Forbidden');
    });

    it('should reject with Viewer role', () => {
      expect(() => resolvers.Query.statistics(null, {}, viewerCtx)).toThrow('Forbidden');
    });

    it('should allow with Auditor role', () => {
      const stats = resolvers.Query.statistics(null, {}, auditorCtx);
      expect(stats.totalEvents).toBe(1);
    });

    it('should allow with Admin role', () => {
      const stats = resolvers.Query.statistics(null, {}, adminCtx);
      expect(stats.totalEvents).toBe(1);
    });

    it('should return correct statistics with Auditor role', () => {
      resolvers.Mutation.logEvent(null, {
        input: { event_type: 'refund', submitter: 'GA', metadata: 'r1' },
      });
      const stats = resolvers.Query.statistics(null, {}, auditorCtx);
      expect(stats.totalEvents).toBe(2);
      expect(stats.eventsByType['payment']).toBe(1);
      expect(stats.eventsByType['refund']).toBe(1);
    });
  });

  describe('Event Logging Mutation', () => {
    it('should create event with correct fields', () => {
      const result = resolvers.Mutation.logEvent(null, {
        input: { event_type: 'audit', submitter: 'GSUBMITTER', metadata: 'test-data' },
      });
      expect(result.index).toBe(0);
      expect(result.event_type).toBe('audit');
      expect(result.submitter).toBe('GSUBMITTER');
      expect(result.metadata).toBe('test-data');
    });

    it('should chain prev_hash correctly', () => {
      const e1 = resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'first' },
      });
      const e2 = resolvers.Mutation.logEvent(null, {
        input: { event_type: 'payment', submitter: 'GA', metadata: 'second' },
      });
      expect(e2.prev_hash).toBe(e1.event_hash);
    });
  });
});
