import { RegionIdentifier, ReplicationEventBatch } from '../types';
import { v4 as uuidv4 } from 'uuid';

export class CrossRegionReplicator {
  private syncHistory: ReplicationEventBatch[] = [];
  private regionSeqs: Map<RegionIdentifier, number> = new Map([
    ['us-east-1', 104500],
    ['eu-central-1', 104498],
    ['ap-southeast-1', 104495],
  ]);

  public replicateBatch(
    source: RegionIdentifier,
    target: RegionIdentifier,
    count: number = 50
  ): ReplicationEventBatch {
    const currentSourceSeq = this.regionSeqs.get(source) || 100000;
    const currentTargetSeq = this.regionSeqs.get(target) || 99990;

    const fromSeq = currentTargetSeq + 1;
    const toSeq = Math.min(currentSourceSeq, fromSeq + count - 1);
    const eventsCount = Math.max(0, toSeq - fromSeq + 1);

    // Update target sequence
    this.regionSeqs.set(target, toSeq);

    const lagLedgers = currentSourceSeq - toSeq;
    const replicationLagMs = lagLedgers * 50; // ~50ms per ledger lag

    const batch: ReplicationEventBatch = {
      batchId: `sync-${uuidv4().substring(0, 8)}`,
      sourceRegion: source,
      targetRegion: target,
      fromSeq,
      toSeq,
      eventsCount,
      stateRootProof: `0xproof_${source}_to_${target}_seq_${toSeq}_${Date.now()}`,
      replicationLagMs,
      timestamp: new Date().toISOString(),
    };

    this.syncHistory.push(batch);
    if (this.syncHistory.length > 500) this.syncHistory.shift();

    return batch;
  }

  public getReplicationLag(source: RegionIdentifier, target: RegionIdentifier): { lagLedgers: number; lagMs: number } {
    const srcSeq = this.regionSeqs.get(source) || 0;
    const tgtSeq = this.regionSeqs.get(target) || 0;
    const lagLedgers = Math.max(0, srcSeq - tgtSeq);
    return {
      lagLedgers,
      lagMs: lagLedgers * 50,
    };
  }

  public getRegionSeq(region: RegionIdentifier): number {
    return this.regionSeqs.get(region) || 0;
  }

  public incrementSourceSeq(source: RegionIdentifier, count: number = 1): number {
    const curr = this.regionSeqs.get(source) || 100000;
    const next = curr + count;
    this.regionSeqs.set(source, next);
    return next;
  }
}
