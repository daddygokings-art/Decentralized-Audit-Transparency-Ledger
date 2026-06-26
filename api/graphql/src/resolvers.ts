import { PubSub } from "graphql-subscriptions";

export const pubsub = new PubSub();
const EVENT_LOGGED = "EVENT_LOGGED";

// In-memory mock store (replace with JS SDK calls in production)
interface EventRecord {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: string;
  prev_hash: string;
}

const events: EventRecord[] = [];

function matchesFilter(e: EventRecord, filter: any): boolean {
  if (!filter) return true;
  if (filter.type && e.event_type !== filter.type) return false;
  if (filter.submitter && !e.submitter.includes(filter.submitter)) return false;
  if (filter.metadata && !e.metadata.includes(filter.metadata)) return false;
  if (filter.startTime && e.timestamp < filter.startTime) return false;
  if (filter.endTime && e.timestamp > filter.endTime) return false;
  return true;
}

export const resolvers = {
  Query: {
    events: (_: any, { limit = 50, offset = 0, filter }: any) =>
      events.filter((e) => matchesFilter(e, filter)).slice(offset, offset + limit),

    event: (_: any, { index }: any) =>
      events.find((e) => e.index === index) ?? null,

    eventByType: (_: any, { type, typeIndex }: any) => {
      const typed = events.filter((e) => e.event_type === type);
      return typed[typeIndex] ?? null;
    },

    statistics: () => {
      const byType: Record<string, number> = {};
      for (const e of events) {
        byType[e.event_type] = (byType[e.event_type] ?? 0) + 1;
      }
      return { totalEvents: events.length, globalMaxLogs: 100000, eventsByType: byType };
    },

    searchEvents: (_: any, { query }: any) =>
      events.filter((e) => e.metadata.toLowerCase().includes(query.toLowerCase())),
  },

  Mutation: {
    logEvent: (_: any, { submitter, eventType, metadata }: any, ctx: any) => {
      if (ctx.apiKey !== process.env.API_KEY) throw new Error("Unauthorized");
      const now = Math.floor(Date.now() / 1000);
      const idx = events.length;
      const prevHash = events.length > 0 ? events[events.length - 1].event_hash : "0".repeat(64);
      const hash = Buffer.from(`${idx}:${eventType}:${submitter}:${metadata}:${now}`).toString("hex").slice(0, 64).padEnd(64, "0");
      const ev: EventRecord = {
        index: idx,
        timestamp: now,
        event_type: eventType,
        submitter,
        metadata,
        event_hash: hash,
        prev_hash: prevHash,
      };
      events.push(ev);
      void pubsub.publish(EVENT_LOGGED, { eventLogged: ev });
      return ev;
    },
  },

  Subscription: {
    eventLogged: {
      subscribe: (_: any, { type }: any) => {
        const asyncIter = pubsub.asyncIterableIterator(EVENT_LOGGED);
        if (!type) return asyncIter;
        // Filter by event type
        return {
          [Symbol.asyncIterator]() {
            return {
              next: async () => {
                for await (const payload of asyncIter) {
                  if ((payload as any).eventLogged.event_type === type) {
                    return { value: payload, done: false };
                  }
                }
                return { value: undefined, done: true };
              },
              return: () => Promise.resolve({ value: undefined, done: true }),
            };
          },
        };
      },
    },
  },
};
