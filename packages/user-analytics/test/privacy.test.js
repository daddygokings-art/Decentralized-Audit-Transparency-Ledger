const test = require('node:test');
const assert = require('node:assert');
const { ConsentManager } = require('../dist/privacy/consent.js');
const { Pseudonymizer } = require('../dist/privacy/pseudonymizer.js');
const { DataErasureManager } = require('../dist/privacy/erasure.js');
const { InMemoryAnalyticsStore } = require('../dist/tracking/tracker.js');

test('Privacy Compliance, Consent, and Anonymization', async (t) => {
  await t.test('manages consent categories and honors DNT/GPC headers', () => {
    const manager = new ConsentManager();
    const anonId = 'anon_12345';

    assert.strictEqual(manager.hasConsent(anonId, 'analytics'), false);

    // Opt-in to analytics
    manager.setConsent(anonId, true, ['necessary', 'analytics']);
    assert.strictEqual(manager.hasConsent(anonId, 'analytics'), true);
    assert.strictEqual(manager.hasConsent(anonId, 'marketing'), false);

    // Opt-out
    manager.optOut(anonId);
    assert.strictEqual(manager.hasConsent(anonId, 'analytics'), false);

    // Header detection
    const dntActive = ConsentManager.isDntOrGpcEnabled({ DNT: '1' });
    assert.strictEqual(dntActive, true);
    const gpcActive = ConsentManager.isDntOrGpcEnabled({ 'sec-gpc': '1' });
    assert.strictEqual(gpcActive, true);
    const normalHeaders = ConsentManager.isDntOrGpcEnabled({ 'user-agent': 'Mozilla' });
    assert.strictEqual(normalHeaders, false);
  });

  await t.test('pseudonymizes wallet addresses and anonymizes IPs', () => {
    const pseudonymizer = new Pseudonymizer('test-salt-secret');
    const wallet = 'GDQJUT...STELLAR_WALLET';

    const anonId1 = pseudonymizer.pseudonymize(wallet);
    const anonId2 = pseudonymizer.pseudonymize(wallet);
    assert.strictEqual(anonId1, anonId2); // Deterministic with same salt
    assert.ok(anonId1.startsWith('anon_'));
    assert.strictEqual(anonId1.includes(wallet), false); // Irreversible

    const maskedIp4 = pseudonymizer.anonymizeIp('192.168.1.150');
    assert.strictEqual(maskedIp4, '192.168.1.0');
  });

  await t.test('executes GDPR Right to be Forgotten data erasure', async () => {
    const consentManager = new ConsentManager();
    const store = new InMemoryAnalyticsStore();
    const erasureManager = new DataErasureManager(consentManager, store);

    const anonId = 'anon_purge_target';
    consentManager.setConsent(anonId, true, ['analytics']);

    await store.saveEvent({
      eventId: 'e1',
      anonymousId: anonId,
      sessionId: 's1',
      eventName: 'page_view',
      timestamp: Date.now(),
      properties: {},
    });
    await store.saveEvent({
      eventId: 'e2',
      anonymousId: 'anon_other_user',
      sessionId: 's2',
      eventName: 'page_view',
      timestamp: Date.now(),
      properties: {},
    });

    const erasure = await erasureManager.executeRightToBeForgotten(anonId);

    assert.strictEqual(erasure.success, true);
    assert.strictEqual(erasure.eventsDeleted, 1);
    assert.strictEqual(erasure.consentDeleted, true);

    // Verify user events are deleted and other users' events remain
    assert.strictEqual(store.getEvents().length, 1);
    assert.strictEqual(store.getEvents()[0].anonymousId, 'anon_other_user');
  });
});
