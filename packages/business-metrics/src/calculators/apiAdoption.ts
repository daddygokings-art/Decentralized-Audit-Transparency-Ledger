import { ApiCallRecord, ApiAdoptionKPIs } from '../types';

export class ApiAdoptionMetricsCalculator {
  public static calculate(
    calls: ApiCallRecord[],
    slaThresholdMs = 200,
    now: number = Date.now()
  ): ApiAdoptionKPIs {
    const ONE_DAY = 24 * 60 * 60 * 1000;
    const dayStart = now - ONE_DAY;

    const devTokens = new Set<string>();
    const protocolBreakdown = { rest: 0, graphql: 0, ws: 0 };
    const tierBreakdown = { free: 0, pro: 0, enterprise: 0 };

    let totalCalls = 0;
    let underSlaCount = 0;
    let errorCount = 0;
    let totalQuotaPct = 0;
    const durations: number[] = [];

    for (const c of calls) {
      if (c.timestamp >= dayStart && c.timestamp <= now) {
        totalCalls++;
        devTokens.add(c.clientToken);
        protocolBreakdown[c.protocol] = (protocolBreakdown[c.protocol] || 0) + 1;
        tierBreakdown[c.tier] = (tierBreakdown[c.tier] || 0) + 1;

        durations.push(c.durationMs);
        if (c.durationMs <= slaThresholdMs) {
          underSlaCount++;
        }

        if (c.statusCode >= 400) {
          errorCount++;
        }

        if (c.quotaUsedPct !== undefined) {
          totalQuotaPct += c.quotaUsedPct;
        }
      }
    }

    durations.sort((a, b) => a - b);
    const p95Index = Math.floor(durations.length * 0.95);
    const p95LatencyMs = durations.length > 0 ? durations[p95Index] : 0;

    const slaCompliancePct =
      totalCalls > 0 ? Number(((underSlaCount / totalCalls) * 100).toFixed(2)) : 100.0;
    const errorRatePct = totalCalls > 0 ? Number(((errorCount / totalCalls) * 100).toFixed(2)) : 0.0;
    const quotaUtilizationPct =
      totalCalls > 0 ? Number((totalQuotaPct / totalCalls).toFixed(2)) : 0.0;

    return {
      totalApiCalls24h: totalCalls,
      activeDeveloperTokens: devTokens.size,
      protocolBreakdown,
      tierBreakdown,
      slaCompliancePct,
      p95LatencyMs,
      errorRatePct,
      quotaUtilizationPct,
    };
  }
}
