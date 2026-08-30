const test = require('node:test');
const assert = require('node:assert');
const { SubmitterMetricsCalculator } = require('../dist/calculators/submitter.js');

test('Submitter Metrics & Centralization Calculator', async (t) => {
  const now = Date.now();
  const ONE_DAY = 24 * 60 * 60 * 1000;

  await t.test('calculates DAU, WAU, MAU and stickiness ratio', () => {
    const records = [
      { submitter: 'user_1', timestamp: now - 1000, contractId: 'C1', eventType: 'audit' },
      { submitter: 'user_2', timestamp: now - 2000, contractId: 'C1', eventType: 'audit' },
      { submitter: 'user_3', timestamp: now - 3 * ONE_DAY, contractId: 'C1', eventType: 'audit' },
      { submitter: 'user_4', timestamp: now - 15 * ONE_DAY, contractId: 'C1', eventType: 'audit' },
    ];

    const kpi = SubmitterMetricsCalculator.calculate(records, now);

    assert.strictEqual(kpi.dau, 2); // user_1, user_2
    assert.strictEqual(kpi.wau, 3); // user_1, user_2, user_3
    assert.strictEqual(kpi.mau, 4); // all 4
    assert.strictEqual(kpi.dauToMauRatio, 0.5);
  });

  await t.test('calculates Gini coefficient of submitter centralization', () => {
    // Uniform distribution: each user has 10 submissions -> Gini close to 0
    const uniformRecords = [];
    for (let i = 1; i <= 5; i++) {
      for (let j = 0; j < 10; j++) {
        uniformRecords.push({ submitter: `user_${i}`, timestamp: now - 1000, contractId: 'C1', eventType: 'audit' });
      }
    }
    const uniformKpi = SubmitterMetricsCalculator.calculate(uniformRecords, now);
    assert.strictEqual(uniformKpi.giniCoefficient, 0);

    // Highly centralized: user_1 has 100, user_2..5 have 1 each -> Gini close to 1
    const skewedRecords = [];
    for (let j = 0; j < 100; j++) {
      skewedRecords.push({ submitter: 'whale_user', timestamp: now - 1000, contractId: 'C1', eventType: 'audit' });
    }
    for (let i = 1; i <= 4; i++) {
      skewedRecords.push({ submitter: `small_user_${i}`, timestamp: now - 1000, contractId: 'C1', eventType: 'audit' });
    }
    const skewedKpi = SubmitterMetricsCalculator.calculate(skewedRecords, now);
    assert.ok(skewedKpi.giniCoefficient > 0.6);
    assert.ok(skewedKpi.topSubmitterSharePct > 90);
  });
});
