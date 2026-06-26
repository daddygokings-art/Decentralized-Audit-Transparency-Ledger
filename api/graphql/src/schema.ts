import { makeExecutableSchema } from "@graphql-tools/schema";

export const typeDefs = `
  type Event {
    id: String!
    index: Int!
    timestamp: Int!
    event_type: String!
    submitter: String!
    metadata: String!
    event_hash: String!
    prev_hash: String!
  }

  type ContractStats {
    totalEvents: Int!
    globalMaxLogs: Int!
    eventsByType: JSON!
  }

  input EventFilter {
    type: String
    submitter: String
    metadata: String
    startTime: Int
    endTime: Int
  }

  input EventInput {
    submitter: String!
    eventType: String!
    metadata: String!
  }

  type Query {
    events(limit: Int = 50, offset: Int = 0, filter: EventFilter): [Event!]!
    event(index: Int!): Event
    eventByType(type: String!, typeIndex: Int!): Event
    statistics: ContractStats!
    searchEvents(query: String!): [Event!]!
  }

  type Mutation {
    logEvent(submitter: String!, eventType: String!, metadata: String!): Event!
  }

  type Subscription {
    eventLogged(type: String): Event!
  }

  scalar JSON
`;

export const schema = makeExecutableSchema({ typeDefs });
