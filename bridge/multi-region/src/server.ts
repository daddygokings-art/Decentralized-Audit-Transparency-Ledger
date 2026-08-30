import express, { Request, Response } from 'express';
import cors from 'cors';
import client from 'prom-client';
import { GlobalTrafficManager } from './routing/traffic-manager';
import { CrossRegionReplicator } from './replication/replicator';
import { FailoverOrchestrator } from './failover/orchestrator';
import { FailoverDrillSuite } from './testing/failover-drill';
import { RegionIdentifier } from './types';

const app = express();
const port = process.env.PORT || 3010;

app.use(cors());
app.use(express.json());

const register = new client.Registry();
client.collectDefaultMetrics({ register });

const trafficManager = new GlobalTrafficManager();
const replicator = new CrossRegionReplicator();
const failoverOrchestrator = new FailoverOrchestrator(trafficManager, replicator);
const drillSuite = new FailoverDrillSuite(trafficManager, replicator, failoverOrchestrator);

// Prometheus Gauges
const replicationLagGauge = new client.Gauge({
  name: 'audit_ledger_cross_region_replication_lag_ms',
  help: 'Cross-region event replication lag in milliseconds',
  labelNames: ['source_region', 'target_region'],
  registers: [register],
});

const primaryRegionGauge = new client.Gauge({
  name: 'audit_ledger_primary_region_status',
  help: 'Active primary region status (1 = active)',
  labelNames: ['region'],
  registers: [register],
});

// ── Region Management Endpoints ──────────────────────────────────────────

app.get('/api/v1/regions', (req: Request, res: Response) => {
  res.json({
    nodes: trafficManager.getAllNodes(),
    primary: trafficManager.getPrimaryNode(),
  });
});

app.post('/api/v1/routing/decision', (req: Request, res: Response) => {
  const { ip, country } = req.body;
  const decision = trafficManager.routeClient(ip || req.ip || '127.0.0.1', country || 'US');
  res.json(decision);
});

app.post('/api/v1/replication/sync', (req: Request, res: Response) => {
  const { sourceRegion, targetRegion, batchSize } = req.body;
  const batch = replicator.replicateBatch(
    (sourceRegion as RegionIdentifier) || 'us-east-1',
    (targetRegion as RegionIdentifier) || 'eu-central-1',
    Number(batchSize || 50)
  );

  replicationLagGauge.set(
    { source_region: batch.sourceRegion, target_region: batch.targetRegion },
    batch.replicationLagMs
  );

  res.status(201).json({ message: 'Replication batch synchronized', batch });
});

app.post('/api/v1/failover/trigger', async (req: Request, res: Response) => {
  const { targetRegion, reason } = req.body;
  try {
    const result = await failoverOrchestrator.executeFailover(
      targetRegion as RegionIdentifier,
      reason || 'Manual operator failover trigger'
    );

    primaryRegionGauge.set({ region: result.newPrimary }, 1);
    primaryRegionGauge.set({ region: result.previousPrimary }, 0);

    res.json({ message: 'Failover executed successfully', result });
  } catch (err: unknown) {
    const errorMsg = err instanceof Error ? err.message : String(err);
    res.status(500).json({ error: errorMsg });
  }
});

app.post('/api/v1/dr/drill', async (req: Request, res: Response) => {
  const { targetRegion } = req.body;
  const drillResult = await drillSuite.runDrill((targetRegion as RegionIdentifier) || 'eu-central-1');
  res.json(drillResult);
});

app.get('/metrics', async (req: Request, res: Response) => {
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
});

app.get('/healthz', (req: Request, res: Response) => {
  const primary = trafficManager.getPrimaryNode();
  res.json({ status: 'ok', primaryRegion: primary?.region, health: primary?.health });
});

if (process.env.NODE_ENV !== 'test') {
  app.listen(port, () => {
    console.log(`[MultiRegionCoordinator] Service running on port ${port}`);
  });
}

export default app;
