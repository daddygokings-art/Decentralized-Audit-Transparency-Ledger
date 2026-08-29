import express, { Express } from 'express';
import cors from 'cors';
import { defaultStateContext, ProviderStateContext, setupProviderState } from './provider-states';

export function createProviderApp(context: ProviderStateContext = defaultStateContext): Express {
  const app = express();
  app.use(cors());
  app.use(express.json());

  // Provider state management endpoint for Pact
  app.post('/_pact/provider_states', (req, res) => {
    const { state } = req.body;
    setupProviderState(state, context);
    res.json({ result: 'State configured successfully' });
  });

  // Health and Readiness
  app.get('/healthz', (_req, res) => {
    res.json({ status: 'ok', version: '1.0.0' });
  });

  app.get('/readyz', (_req, res) => {
    res.json({ status: 'ready', database: 'connected' });
  });

  app.get('/metrics', (_req, res) => {
    res.json({
      status: 'healthy',
      metrics: {
        uptime: 3600
      }
    });
  });

  // Events API
  app.get('/events', (req, res) => {
    const limit = parseInt(req.query.limit as string || '50', 10);
    const events = context.events.slice(0, limit);
    res.json({
      events,
      total: context.events.length,
      has_more: false
    });
  });

  app.get('/events/:index', (req, res) => {
    const index = parseInt(req.params.index, 10);
    const found = context.events.find(e => e.index === index);
    if (!found) {
      return res.status(404).json({ error: 'Event not found' });
    }
    return res.json(found);
  });

  app.get('/events/type/:type', (req, res) => {
    const type = req.params.type;
    const events = context.events.filter(e => e.topic === type);
    res.json({
      type,
      events,
      count: events.length
    });
  });

  app.get('/stats', (_req, res) => {
    res.json({
      total_events: context.events.length,
      active_contracts: 1,
      uptime_seconds: 3600
    });
  });

  return app;
}
