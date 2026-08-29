import { ConsentManager } from './privacy/consent';
import { Pseudonymizer } from './privacy/pseudonymizer';
import { DataErasureManager } from './privacy/erasure';
import { AnalyticsTracker } from './tracking/tracker';
import { FunnelAnalyzer } from './funnel/analyzer';
import { FeatureAdoptionAnalyzer } from './features/adoption';
import { CohortRetentionAnalyzer } from './cohorts/retention';
import { FunnelDefinition } from './types';

export interface AnalyticsRouterOptions {
  consentManager: ConsentManager;
  tracker: AnalyticsTracker;
  pseudonymizer: Pseudonymizer;
  funnels?: FunnelDefinition[];
}

export function createAnalyticsApiRouter(options: AnalyticsRouterOptions) {
  const { consentManager, tracker, pseudonymizer, funnels = [] } = options;
  const erasureManager = new DataErasureManager(consentManager, tracker.getStore());

  return async (req: any, res: any) => {
    const path = req.path || req.url || '';
    const method = req.method || 'GET';

    // POST /api/v1/analytics/consent
    if (path.endsWith('/consent') && method === 'POST') {
      let body: any = {};
      if (req.body) body = req.body;

      const rawId = body.userId || body.walletAddress || '';
      const anonId = rawId.startsWith('anon_') ? rawId : pseudonymizer.pseudonymize(rawId);
      const pref = consentManager.setConsent(
        anonId,
        !!body.optedIn,
        body.categories || ['necessary', 'analytics']
      );

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({ success: true, preferences: pref }));
    }

    // POST /api/v1/analytics/track
    if (path.endsWith('/track') && method === 'POST') {
      let body: any = {};
      if (req.body) body = req.body;

      const rawId = body.userId || body.walletAddress || body.anonymousId || '';
      const anonId = rawId.startsWith('anon_') ? rawId : pseudonymizer.pseudonymize(rawId);
      const sessionId = body.sessionId || tracker.startSession(anonId);

      const result = await tracker.track(
        anonId,
        sessionId,
        body.eventName || 'page_view',
        body.properties || {},
        {
          userAgent: req.headers ? req.headers['user-agent'] : undefined,
          ipHash: pseudonymizer.anonymizeIp(req.ip || '127.0.0.1'),
        }
      );

      res.writeHead(result.tracked ? 200 : 403, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(result));
    }

    // POST /api/v1/analytics/erasure (Right to be Forgotten)
    if (path.endsWith('/erasure') && method === 'POST') {
      let body: any = {};
      if (req.body) body = req.body;

      const rawId = body.userId || body.walletAddress || body.anonymousId || '';
      const anonId = rawId.startsWith('anon_') ? rawId : pseudonymizer.pseudonymize(rawId);
      const erasureResult = await erasureManager.executeRightToBeForgotten(anonId);

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(erasureResult));
    }

    // GET /api/v1/analytics/funnels
    if (path.endsWith('/funnels') && method === 'GET') {
      const events = tracker.getStore().getEvents();
      const results = funnels.map((f) => FunnelAnalyzer.analyze(events, f));

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(results));
    }

    // GET /api/v1/analytics/features
    if (path.endsWith('/features') && method === 'GET') {
      const events = tracker.getStore().getEvents();
      const features = FeatureAdoptionAnalyzer.analyzeFeatures(events);

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(features));
    }

    // GET /api/v1/analytics/cohorts
    if (path.endsWith('/cohorts') && method === 'GET') {
      const events = tracker.getStore().getEvents();
      const now = Date.now();
      const ONE_DAY = 24 * 60 * 60 * 1000;
      const cohort = CohortRetentionAnalyzer.calculateRetention(
        events,
        now - 7 * ONE_DAY,
        now,
        'daily',
        7,
        now
      );

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(cohort));
    }

    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Endpoint not found' }));
  };
}
