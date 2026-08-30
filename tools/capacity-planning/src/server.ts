import express, { Request, Response } from 'express';
import cors from 'cors';
import client from 'prom-client';
import { MLForecaster } from './forecasting/ml-forecaster';
import { ScalingPolicyEvaluator } from './autoscaling/policy-evaluator';
import { CostOptimizer } from './cost/cost-optimizer';
import { CustomMetricsAdapter } from './k8s/custom-metrics-adapter';
import { TelemetryPoint } from './types';

const app = express();
const port = process.env.PORT || 3009;

app.use(cors());
app.use(express.json());

const register = new client.Registry();
client.collectDefaultMetrics({ register });

const forecaster = new MLForecaster();
const policyEvaluator = new ScalingPolicyEvaluator();
const costOptimizer = new CostOptimizer();
const metricsAdapter = new CustomMetricsAdapter(register);

const telemetryBuffer: TelemetryPoint[] = [];

// Seed sample historical metrics
for (let i = 24; i >= 1; i--) {
  telemetryBuffer.push({
    timestamp: Date.now() - i * 3600000,
    tps: 15 + Math.sin(i / 3) * 10 + Math.random() * 5,
    cpuUtilizationPercent: 45 + Math.random() * 20,
    memoryMbUsed: 320 + Math.random() * 80,
    storageBytesUsed: 1024 * 1024 * (500 + (24 - i) * 10),
    queueDepth: Math.round(Math.random() * 20),
    activeSubmitters: 8 + Math.round(Math.random() * 4),
    gasSpentStroops: 1200000 + Math.round(Math.random() * 500000),
  });
}

// ── Ingestion Endpoint ───────────────────────────────────────────────────

app.post('/api/v1/capacity/telemetry', (req: Request, res: Response) => {
  const { tps, cpuUtilizationPercent, memoryMbUsed, storageBytesUsed, queueDepth, activeSubmitters, gasSpentStroops } = req.body;

  const point: TelemetryPoint = {
    timestamp: Date.now(),
    tps: Number(tps || 10),
    cpuUtilizationPercent: Number(cpuUtilizationPercent || 40),
    memoryMbUsed: Number(memoryMbUsed || 300),
    storageBytesUsed: Number(storageBytesUsed || 500000000),
    queueDepth: Number(queueDepth || 0),
    activeSubmitters: Number(activeSubmitters || 5),
    gasSpentStroops: Number(gasSpentStroops || 1000000),
  };

  telemetryBuffer.push(point);
  if (telemetryBuffer.length > 288) telemetryBuffer.shift(); // Keep 24 hours of 5-min intervals

  const forecast = forecaster.forecast(telemetryBuffer, 15);
  const decision = policyEvaluator.evaluate(point, forecast, 3);
  const cost = costOptimizer.analyze(telemetryBuffer, decision.recommendedReplicas);

  metricsAdapter.updateMetrics(
    forecast.predictedTps,
    decision.recommendedReplicas,
    82.5,
    cost.optimizedMonthlyCostUsd
  );

  res.status(201).json({ message: 'Telemetry recorded', forecast, decision });
});

// ── Forecasting & Scaling Endpoints ──────────────────────────────────────

app.get('/api/v1/capacity/forecast', (req: Request, res: Response) => {
  const horizon = Number(req.query.horizonMinutes || 15);
  const forecast = forecaster.forecast(telemetryBuffer, horizon);
  res.json({ forecast, historyLength: telemetryBuffer.length });
});

app.get('/api/v1/capacity/scaling-decision', (req: Request, res: Response) => {
  const currentReplicas = Number(req.query.replicas || 3);
  const latest = telemetryBuffer[telemetryBuffer.length - 1];
  const forecast = forecaster.forecast(telemetryBuffer, 15);
  const decision = policyEvaluator.evaluate(latest, forecast, currentReplicas);
  res.json(decision);
});

app.get('/api/v1/cost/optimization-report', (req: Request, res: Response) => {
  const replicas = Number(req.query.replicas || 3);
  const report = costOptimizer.analyze(telemetryBuffer, replicas);
  res.json(report);
});

// ── Prometheus Metrics Endpoint ──────────────────────────────────────────

app.get('/metrics', async (req: Request, res: Response) => {
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
});

app.get('/healthz', (req: Request, res: Response) => {
  res.json({ status: 'ok', dataPointsBuffered: telemetryBuffer.length });
});

if (process.env.NODE_ENV !== 'test') {
  app.listen(port, () => {
    console.log(`[CapacityPlanner] Service running on port ${port}`);
  });
}

export default app;
