export interface AuditEvent {
  id: string; // hex-encoded Bytes32
  index: number;
  timestamp: number; // unix seconds
  event_type: string;
  submitter: string;
  metadata: string; // hex
  event_hash: string; // hex
  prev_hash: string; // hex
  tx_hash?: string; // Stellar transaction hash (when available)
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

export type DashboardTab = "overview" | "events" | "governance" | "performance" | "health";
export type DashboardTimeRange = "1h" | "24h" | "7d" | "30d" | "custom";

export interface GovernanceAction {
  id: string;
  type: "cap_change" | "role_assignment" | "pause" | "unpause" | "contract_upgrade" | "policy_update";
  actor: string;
  timestamp: number;
  details: string;
  status: "executed" | "queued" | "pending" | "rejected";
  tx_hash?: string;
}

export interface GovernanceProposal {
  id: string;
  title: string;
  action: string;
  proposer: string;
  approvals: number;
  requiredApprovals: number;
  status: "pending" | "approved" | "executed" | "vetoed" | "expired";
  createdAt: number;
  expiresAt: number;
}

export interface ContractHealthStatus {
  isPaused: boolean;
  ttlRemainingDays: number;
  globalCapUtilized: number;
  globalCapMax: number;
  perTypeCapUtilized: Record<string, { current: number; max: number }>;
  circuitBreakerActive: boolean;
  rpcLatencyMs: number;
  lastChecked: number;
}

export interface ContractAlert {
  id: string;
  severity: "critical" | "warning" | "info";
  title: string;
  description: string;
  timestamp: number;
  acknowledged: boolean;
  resolved: boolean;
  component: "contract" | "storage" | "rate_limit" | "bridge" | "governance";
}

export interface PerformanceMetrics {
  tps: number;
  peakTps: number;
  avgGasCpu: number;
  avgGasMem: number;
  avgFeeStroops: number;
  p50LatencyMs: number;
  p95LatencyMs: number;
  p99LatencyMs: number;
  bridgeLagMs: number;
  bridgeQueueDepth: number;
}

export interface MarketUpdateEvent {
  marketId: string;
  yesPrice: number;
  noPrice: number;
  totalLiquidity: number;
  volume24h: number;
  status: "Active" | "Closed" | "Pending" | "Resolved" | "Settled";
  resolvedOutcome?: "Yes" | "No" | "Invalid";
  timestamp: number;
  sequence: number;
}