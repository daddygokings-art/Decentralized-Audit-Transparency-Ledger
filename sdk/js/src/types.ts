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
}

export class AuditLedgerError extends Error {
  code?: number;
  constructor(message: string, code?: number) {
    super(message);
    this.name = 'AuditLedgerError';
    this.code = code;
  }
}
