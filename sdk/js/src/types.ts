/** Hex or base64 encoded 32-byte hash string. */
export type Bytes32 = string;

/** Hex or base64 encoded byte payload. */
export type Bytes = string;

export interface Event {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: Bytes32;
  prev_hash: Bytes32;
}

export interface EventHeader {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
}

export interface NonceState {
  last_nonce: number;
  window_size: number;
  max_nonce: number;
}

export interface SnapshotMetadata {
  id: number;
  timestamp: number;
  event_count: number;
  event_hash: Bytes32;
  description: string;
}

export interface ContractStatistics {
  total_events: number;
  events_by_type: Array<[string, number]>;
  events_last_hour: number;
  events_last_day: number;
  events_last_week: number;
  top_submitters: Array<[string, number]>;
}

export interface EventPage {
  items: Event[];
  total: number;
  offset: number;
  limit: number;
}

export interface CacheStats {
  hits: number;
  misses: number;
  size: number;
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

/** Batch signing types (issue #218). */
export interface SignedEvent {
  event_type: string;
  submitter: string;
  metadata: string;
  timestamp: number;
  nonce: number;
}

export interface BatchSignature {
  /** Public key of the signer (hex-encoded Ed25519 pubkey). */
  pubkey: string;
  /** Ed25519 signature over the SHA-256 of the concatenated event hashes. */
  signature: string;
  /** Number of events covered by this batch signature. */
  event_count: number;
  /** SHA-256 hash of the concatenated event preimages. */
  batch_hash: Bytes32;
}

export interface BatchVerificationResult {
  valid: boolean;
  event_count: number;
  batch_hash: Bytes32;
  error?: string;
}
