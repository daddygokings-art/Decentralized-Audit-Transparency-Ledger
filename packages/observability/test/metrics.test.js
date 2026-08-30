const test = require('node:test');
const assert = require('node:assert');
const {
  MetricsRegistry,
  createStandardObservabilityMetrics,
} = require('../dist/metrics.js');

test('Prometheus RED & USE Metrics Standardization', async (t) => {
  await t.test('records counters, gauges, and histograms with label sets', () => {
    const registry = new MetricsRegistry();
    const metrics = createStandardObservabilityMetrics(registry);

    // Ingestion RED metrics
    metrics.eventIngestionTotal.inc({ contract_id: 'C123', event_type: 'payment', status: 'success' }, 5);
    metrics.eventIngestionTotal.inc({ contract_id: 'C123', event_type: 'payment', status: 'failed' }, 1);
    metrics.eventIngestionDuration.observe({ event_type: 'payment' }, 0.045);
    metrics.eventIngestionDuration.observe({ event_type: 'payment' }, 0.12);

    assert.strictEqual(
      metrics.eventIngestionTotal.get({ contract_id: 'C123', event_type: 'payment', status: 'success' }),
      5
    );
    assert.strictEqual(
      metrics.eventIngestionTotal.get({ contract_id: 'C123', event_type: 'payment', status: 'failed' }),
      1
    );

    // Gauges
    metrics.activeSubmittersGauge.set({ window: '24h' }, 42);
    metrics.deadLetterQueueSize.set({ service: 'relayer' }, 3);
    assert.strictEqual(metrics.activeSubmittersGauge.get({ window: '24h' }), 42);
    assert.strictEqual(metrics.deadLetterQueueSize.get({ service: 'relayer' }), 3);
  });

  await t.test('exports valid Prometheus plaintext format', () => {
    const registry = new MetricsRegistry();
    const metrics = createStandardObservabilityMetrics(registry);

    metrics.eventIngestionTotal.inc({ contract_id: 'CTR_1', event_type: 'transfer', status: 'success' }, 10);
    metrics.activeSubmittersGauge.set({ window: '1h' }, 15);
    metrics.eventIngestionDuration.observe({ event_type: 'transfer' }, 0.05);

    const output = registry.toPrometheusFormat();

    assert.ok(output.includes('# TYPE audit_event_ingestion_total counter'));
    assert.ok(output.includes('audit_event_ingestion_total{contract_id="CTR_1",event_type="transfer",status="success"} 10'));
    assert.ok(output.includes('# TYPE audit_active_submitters_gauge gauge'));
    assert.ok(output.includes('audit_active_submitters_gauge{window="1h"} 15'));
    assert.ok(output.includes('# TYPE audit_event_ingestion_duration_seconds histogram'));
    assert.ok(output.includes('audit_event_ingestion_duration_seconds_bucket{event_type="transfer",le="0.05"} 1'));
    assert.ok(output.includes('audit_event_ingestion_duration_seconds_count{event_type="transfer"} 1'));
  });
});
