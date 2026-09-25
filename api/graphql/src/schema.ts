import { makeExecutableSchema } from "@graphql-tools/schema";

export const typeDefs = `
  """
  An immutable audit event recorded on-chain in the Decentralized Audit
  & Transparency Ledger. Each event is linked to its predecessor via
  \`prev_hash\`, forming a tamper-evident hash chain.
  """
  directive @key(fields: _FieldSet!) repeatable on OBJECT | INTERFACE
  directive @extends on OBJECT | INTERFACE
  directive @external on FIELD_DEFINITION
  directive @requires(fields: _FieldSet!) on FIELD_DEFINITION
  directive @provides(fields: _FieldSet!) on FIELD_DEFINITION

  type Event @key(fields: "id") {
    """Content-addressed identifier (hex-encoded SHA-256)."""
    id: String!
    """Sequential index assigned when the event was logged."""
    index: Int!
    """Unix timestamp (seconds) when the event was recorded on-chain."""
    timestamp: Int!
    """Event category (e.g. "payment", "governance", "audit")."""
    event_type: String!
    """Stellar address of the account that submitted the event."""
    submitter: String!
    """Hex-encoded event payload. Decodes to UTF-8 for human-readable metadata."""
    metadata: String!
    """SHA-256 hash of this event's contents for integrity verification."""
    event_hash: String!
    """Hash of the immediately preceding event, forming the tamper-evident chain."""
    prev_hash: String!
    """Other events submitted by the same account, optionally filtered by type."""
    relatedEvents(type: String, limit: Int = 10): [Event!]!
  }

  """
  Aggregate statistics for the smart contract. Provides a snapshot
  of total events logged, capacity limits, and per-type breakdowns.
  """
  type ContractStats {
    """Total number of events currently stored on-chain."""
    totalEvents: Int!
    """Maximum events the contract will accept (0 = unlimited)."""
    globalMaxLogs: Int!
    """Map of event type to the number of events of that type."""
    eventsByType: JSON!
  }

  """
  A record of a governance action taken on the contract (e.g. ownership
  transfer, max-logs change, pause/unpause).
  """
  type GovernanceEvent {
    """The governance action performed (e.g. "transfer_ownership")."""
    action: String!
    """Stellar address of the account that performed the action."""
    caller: String!
    """Previous value before the change (null for new settings)."""
    oldValue: String
    """New value after the change."""
    newValue: String
    """Unix timestamp (seconds) when the governance action was executed."""
    timestamp: Int!
  }

  """
  Filter input for narrowing event queries. All fields are optional;
  when multiple fields are provided, results must match ALL of them (AND logic).
  """
  input EventFilter {
    """Exact event type to match."""
    type: String
    """Submitter address substring to match (case-insensitive)."""
    submitter: String
    """Metadata hex substring to match (case-insensitive)."""
    metadata: String
    """Include only events at or after this unix timestamp."""
    startTime: Int
    """Include only events at or before this unix timestamp."""
    endTime: Int
  }

  """
  Input payload for the \`logEvent\` mutation. Requires an API key
  sent via the \`x-api-key\` header or \`Authorization: Bearer <key>\`.
  """
  input EventInput {
    """Stellar address of the submitting account."""
    submitter: String!
    """Event category string (e.g. "payment")."""
    eventType: String!
    """Hex-encoded metadata payload."""
    metadata: String!
  }

  type Query {
    """
    Retrieve a paginated list of events. Optionally apply server-side
    filtering via the \`filter\` argument.

    **Example:**
    \`\`\`graphql
    query {
      events(limit: 10, offset: 0, filter: { type: "payment" }) {
        index
        event_type
        submitter
        timestamp
        event_hash
      }
    }
    \`\`\`
    """
    events(limit: Int = 50, offset: Int = 0, filter: EventFilter): [Event!]!

    """
    Fetch a single event by its sequential index.

    **Example:**
    \`\`\`graphql
    query {
      event(index: 42) {
        id
        index
        event_type
        submitter
        metadata
        event_hash
        prev_hash
      }
    }
    \`\`\`
    """
    event(index: Int!): Event

    """
    Fetch an event by its type and type-local index. Useful for
    iterating over events of a specific category.

    **Example:**
    \`\`\`graphql
    query {
      eventByType(type: "payment", typeIndex: 0) {
        index
        event_type
        metadata
      }
    }
    \`\`\`
    """
    eventByType(type: String!, typeIndex: Int!): Event

    """
    Get aggregate contract statistics including total events,
    global max-logs cap, and per-type event counts.

    **Example:**
    \`\`\`graphql
    query {
      statistics {
        totalEvents
        globalMaxLogs
        eventsByType
      }
    }
    \`\`\`
    """
    statistics: ContractStats!

    """
    Full-text search across event metadata. The query string is
    matched case-insensitively against the hex-encoded metadata field.

    **Example:**
    \`\`\`graphql
    query {
      searchEvents(query: "invoice") {
        index
        event_type
        metadata
        timestamp
      }
    }
    \`\`\`
    """
    searchEvents(query: String!): [Event!]!

    """
    Retrieve governance history (ownership transfers, cap changes,
    pause events). Filter by action types or return all.

    **Example:**
    \`\`\`graphql
    query {
      governanceHistory(
        types: ["transfer_ownership", "set_global_max_logs"]
        limit: 20
        offset: 0
      ) {
        action
        caller
        oldValue
        newValue
        timestamp
      }
    }
    \`\`\`
    """
    governanceHistory(types: [String!], limit: Int = 50, offset: Int = 0): [GovernanceEvent!]!

    _service: _Service!
    _entities(representations: [_Any!]!): [_Entity]!
  }

  type Mutation {
    """
    Log a new event on-chain. Requires a valid API key.

    **Example:**
    \`\`\`graphql
    mutation {
      logEvent(
        submitter: "GABC1234..."
        eventType: "payment"
        metadata: "696e766f6963655f303031"
      ) {
        id
        index
        event_type
        event_hash
        prev_hash
      }
    }
    \`\`\`
    """
    logEvent(submitter: String!, eventType: String!, metadata: String!): Event!
  }

  type Subscription {
    """
    Subscribe to real-time event notifications. All supplied filters must
    match (AND logic). When no filter is provided, all events are pushed.
    Authentication is required when using the network server; local in-memory
    schemas can opt out for tests.

    **Example (subscribe with advanced filters):**
    \`\`\`graphql
    subscription($filter: EventFilter) {
      eventLogged(filter: $filter) {
        index
        event_type
        submitter
        metadata
        timestamp
        event_hash
      }
    }
    \`\`\`

    **WebSocket transport:**
    Connect to \`ws://localhost:4000/graphql\` with the \`graphql-ws\`
    protocol, then send the subscription query over the socket. Pass the API
    key as \`connectionParams: { "x-api-key": "<key>" }\`.
    """
    eventLogged(
      filter: EventFilter
      type: String
      submitter: String
      startTime: Int
      endTime: Int
    ): Event!
  }

  """
  Arbitrary JSON scalar used for flexible data structures
  such as the \`eventsByType\` map in contract statistics.
  """
  scalar JSON

  scalar _FieldSet

  type _Service {
    sdl: String
  }

  union _Entity = Event

  scalar _Any
`;

export const schema = makeExecutableSchema({ typeDefs });
