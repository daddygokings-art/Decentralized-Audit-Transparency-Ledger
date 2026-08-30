export type RegionIdentifier = 'us-east-1' | 'eu-central-1' | 'ap-southeast-1';

export type TopologyMode = 'ACTIVE_ACTIVE' | 'ACTIVE_PASSIVE';

export type RegionHealth = 'HEALTHY' | 'DEGRADED' | 'UNREACHABLE' | 'DRAINING' | 'OFFLINE';

export interface RegionalNodeConfig {
  region: RegionIdentifier;
  endpointUrl: string;
  isPrimary: boolean;
  health: RegionHealth;
  lastHeartbeat: string;
  processedLedgerSeq: number;
  stateRootHash: string;
  trafficWeight: number;
  latencyMs: number;
}

export interface ReplicationEventBatch {
  batchId: string;
  sourceRegion: RegionIdentifier;
  targetRegion: RegionIdentifier;
  fromSeq: number;
  toSeq: number;
  eventsCount: number;
  stateRootProof: string;
  replicationLagMs: number;
  timestamp: string;
}

export interface TrafficRoutingDecision {
  clientIp: string;
  clientCountry: string;
  routedRegion: RegionIdentifier;
  routingReason: 'GEO_PROXIMITY' | 'LOWEST_LATENCY' | 'FAILOVER_BACKUP' | 'WEIGHTED_CANARY';
  estimatedLatencyMs: number;
}

export interface FailoverExecutionResult {
  failoverId: string;
  previousPrimary: RegionIdentifier;
  newPrimary: RegionIdentifier;
  fencingToken: number;
  initiatedAt: string;
  completedAt: string;
  recoveryTimeSeconds: number;
  recoveryPointLedgerLag: number;
  isZeroDataLoss: boolean;
  dnsRecordsUpdated: boolean;
  status: 'SUCCESS' | 'FAILED' | 'ROLLED_BACK';
}

export interface DisasterRecoveryReport {
  timestamp: string;
  topology: TopologyMode;
  primaryRegion: RegionIdentifier;
  standbyRegions: RegionIdentifier[];
  replicationStatus: Record<string, { lagMs: number; lastSyncedSeq: number; inSync: boolean }>;
  rtoTargetSeconds: number;
  rpoTargetLedgers: number;
  overallHealthScorePercent: number;
}
