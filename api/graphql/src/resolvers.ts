import { PubSub, withFilter } from "graphql-subscriptions";
import { requireRole, Role } from "./auth";
import { deliverEvent } from "../../rest/src/webhooks";
import { DataLoader } from "./dataloader";

export const pubsub = new PubSub();
export const EVENT_LOGGED = "EVENT_LOGGED";

// In-memory mock store (replace with JS SDK calls in production)
export interface EventRecord {
  id: string;
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: string;
  prev_hash: string;
}

interface GovernanceEventRecord {
  action: string;
  caller: string;
  oldValue?: string;
  newValue?: string;
  timestamp: number;
}

const events: EventRecord[] = [];
const governanceEvents: GovernanceEventRecord[] = [];

export type EventLoader = DataLoader<string, EventRecord | null>;
export type SubmitterEventLoader = DataLoader<string, EventRecord[]>;

export interface EventLoaders {
  byId: EventLoader;
  bySubmitter: SubmitterEventLoader;
}

export function createEventLoaders(source: readonly EventRecord[] = events): EventLoaders {
  return {
    byId: new DataLoader<string, EventRecord | null>(async (ids) =>
      ids.map((id) => source.find((event) => event.id === id) ?? null)
    ),
    bySubmitter: new DataLoader<string, EventRecord[]>(async (submitters) =>
      submitters.map((submitter) => source.filter((event) => event.submitter === submitter))
    ),
  };
}

function matchesFilter(e: EventRecord, filter: any): boolean {
  if (!filter) return true;
  if (filter.type && e.event_type.toLowerCase() !== filter.type.toLowerCase()) return false;
  if (filter.submitter && !e.submitter.toLowerCase().includes(filter.submitter.toLowerCase())) return false;
  if (filter.metadata && !e.metadata.toLowerCase().includes(filter.metadata.toLowerCase())) return false;
  if (filter.startTime != null && e.timestamp < filter.startTime) return false;
  if (filter.endTime != null && e.timestamp > filter.endTime) return false;
  return true;
}

export function resetEvents() {
  events.length = 0;
}

export async function publishEventLogged(event: EventRecord) {
  await pubsub.publish(EVENT_LOGGED, { eventLogged: event });
}

export const resolvers = {
  Query: {
    events: (_: any, { limit = 50, offset = 0, filter }: any) =>
      events.filter((e) => matchesFilter(e, filter)).slice(offset, offset + limit),

    event: (_: any, { index }: any, ctx: any) => {
      requireRole(ctx, Role.Viewer);
      return events.find((e) => e.index === index) ?? null;
    },

    eventByType: (_: any, { type, typeIndex }: any, ctx: any) => {
      requireRole(ctx, Role.Viewer);
      const typed = events.filter((e) => e.event_type === type);
      return typed[typeIndex] ?? null;
    },

    statistics: (_: any, __: any, ctx: any) => {
      requireRole(ctx, Role.Auditor);
      const byType: Record<string, number> = {};
      for (const e of events) {
        byType[e.event_type] = (byType[e.event_type] ?? 0) + 1;
      }
      return { totalEvents: events.length, globalMaxLogs: 100000, eventsByType: byType };
    },

    searchEvents: (_: any, { query }: any) =>
      events.filter((e) => e.metadata.toLowerCase().includes(query.toLowerCase())),

    governanceHistory: (_: any, { types, limit = 50, offset = 0 }: any) => {
      const filtered = types && types.length > 0
        ? governanceEvents.filter((g) => types.includes(g.action))
        : governanceEvents;
      return filtered.slice(offset, offset + limit);
    },

    _service: () => ({ sdl: typeDefs }),
    _entities: (_: any, { representations }: { representations: Array<{ __typename?: string; id?: string }> }) =>
      representations
        .filter((representation) => representation.__typename === "Event" && typeof representation.id === "string")
        .map((representation) => events.find((event) => event.id === representation.id) ?? null),
  },

  Event: {
    __resolveReference: (reference: { id: string }) => events.find((event) => event.id === reference.id) ?? null,

    relatedEvents: async (event: EventRecord, { type, limit = 10 }: any, ctx: any) => {
      const loaders = ctx?.eventLoaders ?? createEventLoaders();
      const related = await loaders.bySubmitter.load(event.submitter);
      return related
        .filter((candidate) => candidate.id !== event.id && (!type || candidate.event_type === type))
        .slice(0, Math.max(0, limit));
    },
  },

  Mutation: {
    logEvent: (_: any, { submitter, eventType, metadata }: any, ctx: any) => {
      if (ctx.apiKey !== process.env.API_KEY) throw new Error("Unauthorized");
      const now = Math.floor(Date.now() / 1000);
      const idx = events.length;
      const prevHash = events.length > 0 ? events[events.length - 1].event_hash : "0".repeat(64);
      const hash = Buffer.from(`${idx}:${eventType}:${submitter}:${metadata}:${now}`).toString("hex").slice(0, 64).padEnd(64, "0");
      const ev: EventRecord = {
        id: String(idx),
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
      void deliverEvent(ev).catch((error) => {
        console.error("Webhook delivery failed", error);
      });

      // Track governance actions in the governance history
      const GOVERNANCE_TYPES = new Set([
        "transfer_ownership", "set_global_max_logs", "set_event_max_logs",
        "remove_event_cap", "contract_paused", "contract_unpaused",
      ]);
      if (GOVERNANCE_TYPES.has(eventType)) {
        governanceEvents.unshift({
          action: eventType,
          caller: submitter,
          newValue: metadata || undefined,
          timestamp: now,
        });
      }

      return ev;
    },
  },

  Subscription: {
    eventLogged: {
      subscribe: withFilter(
        (_root: unknown, _args: unknown, context: { role?: Role; subscriptionAuthRequired?: boolean }) => {
          if (context?.subscriptionAuthRequired) {
            requireRole(context, Role.Viewer);
          }
          return pubsub.asyncIterableIterator(EVENT_LOGGED);
        },
        (
          payload: { eventLogged: EventRecord } | undefined,
          variables: {
            filter?: Record<string, unknown>
            type?: string
            submitter?: string
            startTime?: number
            endTime?: number
          } | undefined,
          _context: unknown,
        ) => {
          if (!payload) return false;
          return matchesFilter(payload.eventLogged, {
            ...(variables?.filter ?? {}),
            ...(variables?.type ? { type: variables.type } : {}),
            ...(variables?.submitter ? { submitter: variables.submitter } : {}),
            ...(variables?.startTime != null ? { startTime: variables.startTime } : {}),
            ...(variables?.endTime != null ? { endTime: variables.endTime } : {}),
          });
        }
      ),
    },
  },
};
