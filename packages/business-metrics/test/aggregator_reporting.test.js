const test = require('node:test');
const assert = require('node:assert');
const { BusinessMetricsAggregator } = require('../dist/aggregator.js');
const { ExecutiveReportGenerator } = require('../dist/reporting.js');

test('Business Metrics Aggregator & Executive Reporting', async (t) => {
  const now = Date.now();
  const aggregator = new BusinessMetricsAggregator();

  aggregator.recordSubmitterActivity({
    submitter: 'submitter_1',
    timestamp: now - 1000,
    contractId: 'CTR_A',
    eventType: 'AuditLog',
  });

  aggregator.recordEvent({
    timestamp: now - 1000,
    eventType: 'AuditLog',
    category: 'compliance',
    bytesCount: 512,
  });

  aggregator.recordBridgeTransfer({
    txHash: '0xabc',
    sourceChain: 'stellar',
    targetChain: 'ethereum',
    timestamp: now - 5000,
    verifiedAt: now - 3000,
    amountUsd: 10000,
    gasCostUsd: 5.0,
    status: 'verified',
  });

  aggregator.recordApiCall({
    timestamp: now - 1000,
    endpoint: '/api/v1/events',
    protocol: 'rest',
    clientToken: 'dev_1',
    tier: 'pro',
    durationMs: 80,
    statusCode: 200,
  });

  const summary = aggregator.generateExecutiveSummary(now);

  await t.test('generates complete executive summary with health score', () => {
    assert.strictEqual(summary.period, '24h');
    assert.ok(summary.healthScore >= 90);
    assert.strictEqual(summary.submitters.dau, 1);
    assert.strictEqual(summary.growth.totalEvents, 1);
    assert.strictEqual(summary.bridge.volumeUsdTotal, 10000);
    assert.strictEqual(summary.apiAdoption.totalApiCalls24h, 1);
  });

  await t.test('generates markdown executive briefing report', () => {
    const report = ExecutiveReportGenerator.generateMarkdownReport(summary);
    assert.ok(report.includes('# Executive Business Metrics & KPI Report'));
    assert.ok(report.includes('Platform Health Score'));
    assert.ok(report.includes('DAU'));
    assert.ok(report.includes('Bridged Volume (USD)'));
  });

  await t.test('exports Prometheus KPI exposition format', () => {
    const prometheus = ExecutiveReportGenerator.toPrometheusMetrics(summary);
    assert.ok(prometheus.includes('audit_kpi_platform_health_score'));
    assert.ok(prometheus.includes('audit_kpi_dau_submitters 1'));
    assert.ok(prometheus.includes('audit_kpi_bridge_volume_usd_total 10000'));
  });
});
