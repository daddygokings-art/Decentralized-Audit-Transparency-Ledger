import { BridgeTransferRecord, BridgeKPIs } from '../types';

export class BridgeThroughputMetricsCalculator {
  public static calculate(transfers: BridgeTransferRecord[]): BridgeKPIs {
    let totalRelayed = transfers.length;
    let verifiedCount = 0;
    let volumeUsdTotal = 0;
    let totalLatencySec = 0;
    let latencyCount = 0;
    let cacheHits = 0;
    let totalGasUsd = 0;

    const chainBreakdown: Record<string, { count: number; volumeUsd: number }> = {};

    for (const t of transfers) {
      const vol = t.amountUsd || 0;
      volumeUsdTotal += vol;
      totalGasUsd += t.gasCostUsd || 0;

      const chain = t.targetChain || 'unknown';
      if (!chainBreakdown[chain]) {
        chainBreakdown[chain] = { count: 0, volumeUsd: 0 };
      }
      chainBreakdown[chain].count++;
      chainBreakdown[chain].volumeUsd += vol;

      if (t.status === 'verified') {
        verifiedCount++;
      }

      if (t.verifiedAt && t.timestamp && t.verifiedAt >= t.timestamp) {
        totalLatencySec += (t.verifiedAt - t.timestamp) / 1000;
        latencyCount++;
      }

      if (t.cachedProof) {
        cacheHits++;
      }
    }

    const verificationSuccessRatePct =
      totalRelayed > 0 ? Number(((verifiedCount / totalRelayed) * 100).toFixed(2)) : 100.0;
    const avgRelayLatencySeconds = latencyCount > 0 ? Number((totalLatencySec / latencyCount).toFixed(2)) : 0;
    const cacheHitRatePct = totalRelayed > 0 ? Number(((cacheHits / totalRelayed) * 100).toFixed(2)) : 0;
    const avgGasCostUsd = totalRelayed > 0 ? Number((totalGasUsd / totalRelayed).toFixed(4)) : 0;

    return {
      totalRelayedEvents: totalRelayed,
      volumeUsdTotal: Number(volumeUsdTotal.toFixed(2)),
      avgRelayLatencySeconds,
      verificationSuccessRatePct,
      cacheHitRatePct,
      avgGasCostUsd,
      chainBreakdown,
    };
  }
}
