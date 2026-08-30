const test = require('node:test');
const assert = require('node:assert');
const { FeatureAdoptionAnalyzer } = require('../dist/features/adoption.js');
const { CohortRetentionAnalyzer } = require('../dist/cohorts/retention.js');

test('Feature Adoption & Cohort Retention Analysis', async (t) => {
  const now = Date.now();
  const ONE_DAY = 24 * 60 * 60 * 1000;

  await t.test('calculates feature adoption and power user stickiness', () => {
    const events = [];
    // User A uses 'token_gating' 12 times (power user)
    for (let i = 0; i < 12; i++) {
      events.push({
        eventId: `e_a_${i}`,
        anonymousId: 'user_a',
        sessionId: 's1',
        eventName: 'feature_used',
        timestamp: now - 1000 * i,
        properties: { feature: 'token_gating' },
      });
    }

    // User B uses 'token_gating' 2 times and 'tax_engine' 1 time
    for (let i = 0; i < 2; i++) {
      events.push({
        eventId: `e_b_${i}`,
        anonymousId: 'user_b',
        sessionId: 's2',
        eventName: 'feature_used',
        timestamp: now - 1000 * i,
        properties: { feature: 'token_gating' },
      });
    }
    events.push({
      eventId: 'e_b_tax',
      anonymousId: 'user_b',
      sessionId: 's2',
      eventName: 'feature_used',
      timestamp: now - 1000,
      properties: { feature: 'tax_engine' },
    });

    const metrics = FeatureAdoptionAnalyzer.analyzeFeatures(events, now);

    assert.strictEqual(metrics.length, 2);
    const tokenGating = metrics.find((m) => m.featureName === 'token_gating');
    assert.ok(tokenGating);
    assert.strictEqual(tokenGating.uniqueUsers, 2);
    assert.strictEqual(tokenGating.totalEvents, 14);
    assert.strictEqual(tokenGating.powerUsers, 1); // User A has >= 10 interactions
  });

  await t.test('calculates cohort retention heatmap matrix across daily intervals', () => {
    const cohortStart = now - 7 * ONE_DAY;
    const cohortEnd = cohortStart + ONE_DAY;

    const events = [
      // Day 0: users acquired
      { eventId: '1', anonymousId: 'u1', sessionId: 's', eventName: 'signup', timestamp: cohortStart + 1000, properties: {} },
      { eventId: '2', anonymousId: 'u2', sessionId: 's', eventName: 'signup', timestamp: cohortStart + 2000, properties: {} },
      // Day 1: u1 returns
      { eventId: '3', anonymousId: 'u1', sessionId: 's', eventName: 'view', timestamp: cohortStart + ONE_DAY + 1000, properties: {} },
      // Day 2: u1 and u2 return
      { eventId: '4', anonymousId: 'u1', sessionId: 's', eventName: 'view', timestamp: cohortStart + 2 * ONE_DAY + 1000, properties: {} },
      { eventId: '5', anonymousId: 'u2', sessionId: 's', eventName: 'view', timestamp: cohortStart + 2 * ONE_DAY + 2000, properties: {} },
    ];

    const cohort = CohortRetentionAnalyzer.calculateRetention(
      events,
      cohortStart,
      cohortEnd,
      'daily',
      4,
      now
    );

    assert.strictEqual(cohort.cohortSize, 2);
    assert.strictEqual(cohort.intervals[0].retentionRatePct, 100.0); // Day 0 = 100%
    assert.strictEqual(cohort.intervals[1].retentionRatePct, 50.0); // Day 1 = 1 of 2
    assert.strictEqual(cohort.intervals[2].retentionRatePct, 100.0); // Day 2 = 2 of 2
    assert.strictEqual(cohort.intervals[3].retentionRatePct, 0.0); // Day 3 = 0 of 2
  });
});
