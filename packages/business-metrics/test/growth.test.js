const test = require('node:test');
const assert = require('node:assert');
const { EventGrowthMetricsCalculator } = require('../dist/calculators/growth.js');

test('Event Volume Growth & Anomaly Calculator', async (t) => {
  const now = Date.now();
  const ONE_DAY = 24 * 60 * 60 * 1000;

  await t.test('computes DoD, WoW, MoM growth and category breakdown', () => {
    const events = [];
    // 20 events today in 'financial'
    for (let i = 0; i < 20; i++) {
      events.push({ timestamp: now - 1000 * i, eventType: 'payment', category: 'financial', bytesCount: 500 });
    }
    // 10 events yesterday in 'governance'
    for (let i = 0; i < 10; i++) {
      events.push({ timestamp: now - ONE_DAY - 1000 * i, eventType: 'vote', category: 'governance', bytesCount: 300 });
    }

    const kpi = EventGrowthMetricsCalculator.calculate(events, [], now);

    assert.strictEqual(kpi.totalEvents, 30);
    assert.strictEqual(kpi.dodGrowthPct, 100); // 20 vs 10 = +100%
    assert.strictEqual(kpi.categoryBreakdown.financial.count, 20);
    assert.strictEqual(kpi.categoryBreakdown.financial.percentage, 66.67);
  });

  await t.test('detects statistical volume anomalies via Z-scores', () => {
    const historicalVolumes = [100, 105, 98, 102, 101, 99, 100]; // Mean ~100, stddev ~2
    const events = [];

    // Today's volume: 200 events (huge spike!)
    for (let i = 0; i < 200; i++) {
      events.push({ timestamp: now - 1000 * i, eventType: 'payment', category: 'financial' });
    }

    const kpi = EventGrowthMetricsCalculator.calculate(events, historicalVolumes, now);

    assert.ok(kpi.anomalyScore > 10);
    assert.strictEqual(kpi.isAnomaly, true);
  });
});
