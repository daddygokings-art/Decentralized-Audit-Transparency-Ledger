/**
 * Serverless Event Processing Engine Types (#522)
 */

export interface ContractEvent {
  id: string;
  index: number;
  timestamp: number;
  eventType: string;
  category?: string;
  subEventType?: string;
  submitter: string;
  metadata: Record<string, any> | string;
  eventHash: string;
  prevHash?: string;
  parentEventId?: string;
  ledgerSeq?: number;
  txHash?: string;
}

export interface EnrichedEvent extends ContractEvent {
  enrichedAt: number;
  enrichments: {
    submitterDid?: string;
    geoCountry?: string;
    riskScore?: number;
    complianceTags?: string[];
    contractDomain?: string;
    entityName?: string;
  };
}

export interface TransformationRule {
  targetFormat: "json" | "protobuf" | "avro" | "cloudevents";
  fieldMappings?: Record<string, string>;
  excludedFields?: string[];
  anonymizeFields?: string[];
}

export interface FilterPredicate {
  field: string;
  operator: "eq" | "neq" | "gt" | "gte" | "lt" | "lte" | "in" | "contains" | "regex";
  value: any;
}

export interface FilterRule {
  name: string;
  mode: "include" | "exclude";
  predicates: FilterPredicate[];
  logicalOp?: "AND" | "OR";
}

export type DestinationType =
  | "aws-eventbridge"
  | "aws-sqs"
  | "gcp-pubsub"
  | "azure-servicebus"
  | "knative-eventing"
  | "kafka"
  | "webhook";

export interface RouteTarget {
  destination: DestinationType;
  endpointUrl?: string;
  topicOrQueueArn?: string;
  headers?: Record<string, string>;
  retryAttempts?: number;
}

export interface ProcessingResult {
  success: boolean;
  eventId: string;
  transformedData?: any;
  dropped?: boolean;
  dropReason?: string;
  routedDestinations?: string[];
  durationMs: number;
  error?: string;
}
