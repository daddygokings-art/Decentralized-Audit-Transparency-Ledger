/**
 * Contract Event Business Metrics and KPIs Types
 */

export interface SubmitterActivityRecord {
  submitter: string;
  timestamp: number; // epoch ms
  contractId: string;
  eventType: string;
  bytesCount?: number;
}

export interface SubmitterKPIs {
  dau: number;
  wau: number;
  mau: number;
  dauToMauRatio: number;
  retentionRate7d: number;
  retentionRate30d: number;
  giniCoefficient: number; // 0 = equality, 1 = monopoly
  newSubmitters24h: number;
  returningSubmitters24h: number;
  topSubmitterSharePct: number;
}

export interface EventGrowthKPIs {
  totalEvents: number;
  dodGrowthPct: number;
  wowGrowthPct: number;
  momGrowthPct: number;
  eventsPerSecondAvg: number;
  eventsPerSecondPeak: number;
  totalDataFootprintBytes: number;
  categoryBreakdown: Record<string, { count: number; percentage: number }>;
  anomalyScore: number; // Z-score
  isAnomaly: boolean;
}

export interface GovernanceActionRecord {
  id: string;
  type: 'proposal_created' | 'vote_cast' | 'proposal_executed' | 'dispute_raised' | 'dispute_resolved';
  timestamp: number;
  proposalId?: string;
  voter?: string;
  weight?: number;
  quorumRequired?: number;
  votesFor?: number;
  votesAgainst?: number;
  latencyHours?: number;
}

export interface GovernanceKPIs {
  totalProposals: number;
  activeProposals: number;
  turnoutRatePct: number;
  quorumAttainmentPct: number;
  avgExecutionLatencyHours: number;
  disputesInitiated: number;
  disputesResolved: number;
  disputeResolutionRatePct: number;
}

export interface BridgeTransferRecord {
  txHash: string;
  sourceChain: string;
  targetChain: string;
  timestamp: number;
  verifiedAt?: number;
  amountUsd?: number;
  gasCostUsd?: number;
  status: 'pending' | 'verified' | 'failed';
  cachedProof?: boolean;
}

export interface BridgeKPIs {
  totalRelayedEvents: number;
  volumeUsdTotal: number;
  avgRelayLatencySeconds: number;
  verificationSuccessRatePct: number;
  cacheHitRatePct: number;
  avgGasCostUsd: number;
  chainBreakdown: Record<string, { count: number; volumeUsd: number }>;
}

export interface ApiCallRecord {
  timestamp: number;
  endpoint: string;
  protocol: 'rest' | 'graphql' | 'ws';
  clientToken: string;
  tier: 'free' | 'pro' | 'enterprise';
  durationMs: number;
  statusCode: number;
  quotaUsedPct?: number;
}

export interface ApiAdoptionKPIs {
  totalApiCalls24h: number;
  activeDeveloperTokens: number;
  protocolBreakdown: { rest: number; graphql: number; ws: number };
  tierBreakdown: { free: number; pro: number; enterprise: number };
  slaCompliancePct: number;
  p95LatencyMs: number;
  errorRatePct: number;
  quotaUtilizationPct: number;
}

export interface ExecutiveKPISummary {
  timestamp: string;
  period: string;
  healthScore: number;
  submitters: SubmitterKPIs;
  growth: EventGrowthKPIs;
  governance: GovernanceKPIs;
  bridge: BridgeKPIs;
  apiAdoption: ApiAdoptionKPIs;
}
