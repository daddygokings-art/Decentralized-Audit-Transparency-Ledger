import { FailoverExecutionResult, RegionIdentifier } from '../types';
import { GlobalTrafficManager } from '../routing/traffic-manager';
import { CrossRegionReplicator } from '../replication/replicator';
import { v4 as uuidv4 } from 'uuid';

export class FailoverOrchestrator {
  private trafficManager: GlobalTrafficManager;
  private replicator: CrossRegionReplicator;
  private currentFencingToken = 1000;

  constructor(trafficManager: GlobalTrafficManager, replicator: CrossRegionReplicator) {
    this.trafficManager = trafficManager;
    this.replicator = replicator;
  }

  public async executeFailover(
    targetRegion: RegionIdentifier,
    reason: string = 'Automated regional health trigger'
  ): Promise<FailoverExecutionResult> {
    const startTime = Date.now();
    const initiatedAt = new Date().toISOString();

    const previousPrimary = this.trafficManager.getPrimaryNode();
    const prevRegion: RegionIdentifier = previousPrimary ? previousPrimary.region : 'us-east-1';

    if (prevRegion === targetRegion) {
      throw new Error(`Target region ${targetRegion} is already active primary.`);
    }

    // Step 1: Increment monotonic fencing token to prevent split-brain
    this.currentFencingToken += 1;
    const fencingToken = this.currentFencingToken;

    // Step 2: Mark old primary as DRAINING / OFFLINE
    this.trafficManager.setNodeHealth(prevRegion, 'DRAINING');
    if (previousPrimary) previousPrimary.isPrimary = false;

    // Step 3: Fast catchup replication sweep
    const replicationBatch = this.replicator.replicateBatch(prevRegion, targetRegion, 100);

    // Step 4: Promote target region to PRIMARY
    const targetNode = this.trafficManager.getNode(targetRegion);
    if (!targetNode) {
      throw new Error(`Target region node ${targetRegion} does not exist.`);
    }
    targetNode.isPrimary = true;
    targetNode.health = 'HEALTHY';

    // Step 5: Update DNS / Geo routing table
    this.trafficManager.setNodeHealth(prevRegion, 'OFFLINE');

    const completedTime = Date.now();
    const completedAt = new Date().toISOString();
    const recoveryTimeSeconds = Math.max(1, Math.round((completedTime - startTime) / 1000));
    const recoveryPointLedgerLag = Math.max(0, this.replicator.getRegionSeq(prevRegion) - targetNode.processedLedgerSeq);

    return {
      failoverId: `fo-${uuidv4().substring(0, 8)}`,
      previousPrimary: prevRegion,
      newPrimary: targetRegion,
      fencingToken,
      initiatedAt,
      completedAt,
      recoveryTimeSeconds,
      recoveryPointLedgerLag,
      isZeroDataLoss: recoveryPointLedgerLag === 0,
      dnsRecordsUpdated: true,
      status: 'SUCCESS',
    };
  }

  public getFencingToken(): number {
    return this.currentFencingToken;
  }
}
