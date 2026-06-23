export interface AuditEvent {
  id: string; // hex-encoded Bytes32
  index: number;
  timestamp: number; // unix seconds
  event_type: string;
  submitter: string;
  metadata: string; // hex
  event_hash: string; // hex
  prev_hash: string; // hex
}

export interface ContractStats {
  totalEvents: number;
  globalMaxLogs: number;
  eventsByType: Record<string, number>;
}

export interface SearchFilters {
  event_type?: string;
  submitter?: string;
  metadata?: string;
  dateFrom?: string;
  dateTo?: string;
}

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}
