const test = require('node:test');
const assert = require('node:assert');
const { ConsentManager } = require('../dist/privacy/consent.js');
const { AnalyticsTracker, InMemoryAnalyticsStore } = require('../dist/tracking/tracker.js');
const { FunnelAnalyzer } = require('../dist/funnel/analyzer.js');

test('User Behavior Tracking & Funnel Analysis', async (t) => {
  const consentManager = new ConsentManager();
  const store = new InMemoryAnalyticsStore();
  const tracker = new AnalyticsTracker(consentManager, store);

  const anonUser1 = 'anon_user_1';
  const anonUser2 = 'anon_user_2';
  const now = Date.now();

  await t.test('enforces consent before recording events', async () => {
    const sess1 = tracker.startSession(anonUser1, now);

    // Track without consent -> should be rejected
    const res1 = await tracker.track(anonUser1, sess1, 'page_view', {}, {}, now);
    assert.strictEqual(res1.tracked, false);
    assert.strictEqual(store.getEvents().length, 0);

    // Grant consent and track -> should succeed
    consentManager.setConsent(anonUser1, true, ['analytics']);
    const res2 = await tracker.track(anonUser1, sess1, 'page_view', {}, {}, now);
    assert.strictEqual(res2.tracked, true);
    assert.strictEqual(store.getEvents().length, 1);
  });

  await t.test('performs multi-stage funnel conversion and drop-off analysis', async () => {
    consentManager.setConsent(anonUser1, true, ['analytics']);
    consentManager.setConsent(anonUser2, true, ['analytics']);

    const sess1 = tracker.startSession(anonUser1, now);
    const sess2 = tracker.startSession(anonUser2, now);

    // User 1 completes all 4 steps: connect -> form -> submit -> verify
    await tracker.track(anonUser1, sess1, 'connect_wallet', {}, {}, now);
    await tracker.track(anonUser1, sess1, 'view_audit_form', {}, {}, now + 5000);
    await tracker.track(anonUser1, sess1, 'submit_event', {}, {}, now + 15000);
    await tracker.track(anonUser1, sess1, 'verify_proof', {}, {}, now + 25000);

    // User 2 only completes first 2 steps: connect -> form -> drops off
    await tracker.track(anonUser2, sess2, 'connect_wallet', {}, {}, now);
    await tracker.track(anonUser2, sess2, 'view_audit_form', {}, {}, now + 4000);

    const funnelDef = {
      id: 'audit_submission_funnel',
      name: 'Audit Submission & Verification Flow',
      steps: [
        { step: 'Connect Wallet', eventName: 'connect_wallet' },
        { step: 'View Form', eventName: 'view_audit_form' },
        { step: 'Submit Event', eventName: 'submit_event' },
        { step: 'Verify Proof', eventName: 'verify_proof' },
      ],
      maxConversionWindowHours: 24,
    };

    const analysis = FunnelAnalyzer.analyze(store.getEvents(), funnelDef);

    assert.strictEqual(analysis.totalUsersEntered, 2);
    assert.strictEqual(analysis.totalUsersCompleted, 1);
    assert.strictEqual(analysis.overallConversionRatePct, 50.0);

    assert.strictEqual(analysis.stepResults[0].usersCompleted, 2);
    assert.strictEqual(analysis.stepResults[1].usersCompleted, 2);
    assert.strictEqual(analysis.stepResults[2].usersCompleted, 1); // User 2 dropped off here
    assert.strictEqual(analysis.stepResults[2].dropoffPct, 50.0);
    assert.strictEqual(analysis.biggestDropoffStep, 'Submit Event');
  });
});
