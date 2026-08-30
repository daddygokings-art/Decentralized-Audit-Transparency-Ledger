import { ExecutiveKPISummary } from './types';

export class ExecutiveReportGenerator {
  public static generateMarkdownReport(summary: ExecutiveKPISummary): string {
    const s = summary.submitters;
    const g = summary.growth;
    const gov = summary.governance;
    const b = summary.bridge;
    const a = summary.apiAdoption;

    return `
# Executive Business Metrics & KPI Report
**Generated At**: ${summary.timestamp}
**Platform Health Score**: ${summary.healthScore} / 100 ${summary.healthScore >= 90 ? '🟢 HEALTHY' : summary.healthScore >= 75 ? '🟡 DEGRADED' : '🔴 CRITICAL'}

---

## 1. Submitter Adoption & Decentralization
- **Daily Active Submitters (DAU)**: ${s.dau}
- **Weekly Active Submitters (WAU)**: ${s.wau}
- **Monthly Active Submitters (MAU)**: ${s.mau}
- **Stickiness (DAU/MAU)**: ${(s.dauToMauRatio * 100).toFixed(1)}%
- **7-Day Submitter Retention**: ${s.retentionRate7d}%
- **Gini Centralization Index**: ${s.giniCoefficient} (0 = Decentralized, 1 = Monopolized)
- **New vs Returning Submitters (24h)**: ${s.newSubmitters24h} New / ${s.returningSubmitters24h} Returning

## 2. Event Volume & Growth
- **Total Events Processed**: ${g.totalEvents}
- **Day-over-Day (DoD) Growth**: ${g.dodGrowthPct >= 0 ? '+' : ''}${g.dodGrowthPct}%
- **Week-over-Week (WoW) Growth**: ${g.wowGrowthPct >= 0 ? '+' : ''}${g.wowGrowthPct}%
- **Throughput**: Avg ${g.eventsPerSecondAvg} eps (Peak: ${g.eventsPerSecondPeak} eps)
- **Data Footprint**: ${(g.totalDataFootprintBytes / (1024 * 1024)).toFixed(2)} MB
- **Anomaly Status**: ${g.isAnomaly ? `⚠️ ANOMALY DETECTED (Z-Score: ${g.anomalyScore})` : '✅ Normal'}

## 3. Cross-Chain Bridge Throughput
- **Total Relayed Events**: ${b.totalRelayedEvents}
- **Bridged Volume (USD)**: $${b.volumeUsdTotal.toLocaleString()}
- **Verification Success Rate**: ${b.verificationSuccessRatePct}%
- **Avg Relay Latency**: ${b.avgRelayLatencySeconds}s
- **Proof Cache Hit Rate**: ${b.cacheHitRatePct}%

## 4. Governance & DAO Actions
- **Total Proposals**: ${gov.totalProposals} (${gov.activeProposals} Active)
- **Voter Turnout Rate**: ${gov.turnoutRatePct}%
- **Quorum Attainment**: ${gov.quorumAttainmentPct}%
- **Dispute Resolution Rate**: ${gov.disputeResolutionRatePct}%

## 5. Developer & API Adoption
- **24h API Calls**: ${a.totalApiCalls24h.toLocaleString()}
- **Active Developer Tokens**: ${a.activeDeveloperTokens}
- **SLA Compliance (<200ms)**: ${a.slaCompliancePct}%
- **p95 Latency**: ${a.p95LatencyMs}ms
- **API Error Rate**: ${a.errorRatePct}%
    `.trim();
  }

  public static toPrometheusMetrics(summary: ExecutiveKPISummary): string {
    const s = summary.submitters;
    const g = summary.growth;
    const gov = summary.governance;
    const b = summary.bridge;
    const a = summary.apiAdoption;

    return `
# HELP audit_kpi_platform_health_score Overall business health score (0-100)
# TYPE audit_kpi_platform_health_score gauge
audit_kpi_platform_health_score ${summary.healthScore}

# HELP audit_kpi_dau_submitters Daily active event submitters
# TYPE audit_kpi_dau_submitters gauge
audit_kpi_dau_submitters ${s.dau}

# HELP audit_kpi_mau_submitters Monthly active event submitters
# TYPE audit_kpi_mau_submitters gauge
audit_kpi_mau_submitters ${s.mau}

# HELP audit_kpi_submitter_gini Submitter centralization Gini index
# TYPE audit_kpi_submitter_gini gauge
audit_kpi_submitter_gini ${s.giniCoefficient}

# HELP audit_kpi_event_growth_rate_dod_pct Day-over-day event growth rate percentage
# TYPE audit_kpi_event_growth_rate_dod_pct gauge
audit_kpi_event_growth_rate_dod_pct ${g.dodGrowthPct}

# HELP audit_kpi_bridge_volume_usd_total Total bridged USD volume
# TYPE audit_kpi_bridge_volume_usd_total gauge
audit_kpi_bridge_volume_usd_total ${b.volumeUsdTotal}

# HELP audit_kpi_bridge_success_rate_pct Cross-chain verification success rate percentage
# TYPE audit_kpi_bridge_success_rate_pct gauge
audit_kpi_bridge_success_rate_pct ${b.verificationSuccessRatePct}

# HELP audit_kpi_api_active_developers Active developer API tokens
# TYPE audit_kpi_api_active_developers gauge
audit_kpi_api_active_developers ${a.activeDeveloperTokens}

# HELP audit_kpi_api_sla_compliance_pct API SLA compliance percentage
# TYPE audit_kpi_api_sla_compliance_pct gauge
audit_kpi_api_sla_compliance_pct ${a.slaCompliancePct}
    `.trim() + '\n';
  }
}
