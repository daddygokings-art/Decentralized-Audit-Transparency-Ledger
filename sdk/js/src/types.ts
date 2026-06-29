export type Bytes32 = string; // hex/base64 representation

export interface Event {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: Bytes32;
  prev_hash: Bytes32;
}

export interface ContractStatistics {
  total_events: number;
  events_by_type: Array<[string, number]>;
  events_last_hour: number;
  events_last_day: number;
  events_last_week: number;
  top_submitters: Array<[string, number]>;
}

export enum ContractError {
  CallerNotOwner = 1,
  GlobalMaxLogsReached = 2,
  EventTypeMaxLogsReached = 3,
  EventDoesNotExist = 4,
  EventTypeIndexOutOfBounds = 5,
  NewOwnerIsZero = 6,
  CapNotSet = 7,
  MetadataTooLarge = 8,
  InvalidSignature = 9,
  ContractPaused = 10,
  RateLimitExceeded = 11,
  NoEventsForType = 14,
  AlreadyInitialized = 15,
}

export class AuditLedgerError extends Error {
  code?: number;
  status?: number;
  constructor(message: string, code?: number, status?: number) {
    super(message);
    this.name = 'AuditLedgerError';
    this.code = code;
    this.status = status;
  }
}
