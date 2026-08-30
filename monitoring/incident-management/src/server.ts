import express, { Request, Response } from 'express';
import cors from 'cors';
import { v4 as uuidv4 } from 'uuid';
import { Incident, IncidentAlertPayload, SeverityLevel } from './types';
import { PagerDutyClient } from './pagerduty';
import { OpsgenieClient } from './opsgenie';
import { OnCallScheduler } from './on-call-scheduler';
import { EscalationEngine } from './escalation-engine';
import { TimelineTracker } from './timeline-tracker';
import { PostmortemGenerator } from './postmortem-generator';

const app = express();
const port = process.env.PORT || 3008;

app.use(cors());
app.use(express.json());

const pdClient = new PagerDutyClient();
const ogClient = new OpsgenieClient();
const scheduler = new OnCallScheduler();
const escalationEngine = new EscalationEngine(pdClient, ogClient, scheduler);
const timelineTracker = new TimelineTracker();

const incidents: Map<string, Incident> = new Map();

// ── Webhook / Alert Ingestion ─────────────────────────────────────────────

app.post('/api/v1/incidents/alerts', async (req: Request, res: Response) => {
  const { alertName, source, contractAddress, severity, summary, details } = req.body;

  const incidentId = `inc-${Date.now()}-${uuidv4().substring(0, 6)}`;
  const sev: SeverityLevel = severity || 'SEV-2';

  const alertPayload: IncidentAlertPayload = {
    alertName: alertName || 'Contract Event Anomaly',
    source: source || 'stellar-indexer',
    contractAddress,
    severity: sev,
    summary: summary || 'Elevated transaction anomaly detected',
    details: details || {},
    timestamp: new Date().toISOString(),
    dedupKey: incidentId,
  };

  const pdRes = await pdClient.triggerAlert(alertPayload);
  const ogRes = await ogClient.createAlert(alertPayload);

  const initialTimeline = timelineTracker.addEntry(
    incidentId,
    'ALERT_FIRED',
    'system-alertmanager',
    `Triggered alert: ${alertPayload.summary}`,
    { severity: sev, source }
  );

  const incident: Incident = {
    id: incidentId,
    title: alertPayload.summary,
    severity: sev,
    status: 'TRIGGERED',
    source: alertPayload.source,
    contractAddress: alertPayload.contractAddress,
    reporter: 'monitoring-system',
    createdAt: new Date().toISOString(),
    circuitBreakerActive: sev === 'SEV-1',
    timeline: [initialTimeline],
    pagerDutyIncidentId: pdRes.dedupKey,
    opsgenieAlertId: ogRes.alertId,
    currentEscalationTier: 1,
  };

  incidents.set(incidentId, incident);

  res.status(201).json({
    message: 'Incident triggered successfully',
    incident,
  });
});

// ── Incident Management Endpoints ────────────────────────────────────────

app.get('/api/v1/incidents', (req: Request, res: Response) => {
  res.json({
    total: incidents.size,
    incidents: Array.from(incidents.values()),
  });
});

app.get('/api/v1/incidents/:id', (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });
  inc.timeline = timelineTracker.getTimeline(inc.id);
  res.json(inc);
});

app.post('/api/v1/incidents/:id/acknowledge', async (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });

  const { commander } = req.body;
  inc.status = 'ACKNOWLEDGED';
  inc.commander = commander || 'On-Call Responder';
  inc.acknowledgedAt = new Date().toISOString();

  timelineTracker.addEntry(
    inc.id,
    'COMMANDER_ASSIGNED',
    inc.commander,
    `Incident acknowledged and commander assigned: ${inc.commander}`
  );

  if (inc.pagerDutyIncidentId) await pdClient.acknowledgeAlert(inc.pagerDutyIncidentId);
  if (inc.opsgenieAlertId) await ogClient.acknowledgeAlert(inc.opsgenieAlertId, inc.commander);

  inc.timeline = timelineTracker.getTimeline(inc.id);
  res.json({ message: 'Incident acknowledged', incident: inc });
});

app.post('/api/v1/incidents/:id/notes', (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });

  const { author, message, entryType } = req.body;
  const item = timelineTracker.addEntry(
    inc.id,
    entryType || 'NOTE',
    author || inc.commander || 'Engineer',
    message || 'Progress update'
  );

  res.status(201).json({ message: 'Timeline note recorded', entry: item });
});

app.post('/api/v1/incidents/:id/circuit-breaker', (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });

  const { trip, reason, actor } = req.body;
  inc.circuitBreakerActive = !!trip;

  timelineTracker.addEntry(
    inc.id,
    trip ? 'CIRCUIT_BREAKER_TRIPPED' : 'CIRCUIT_BREAKER_RESET',
    actor || inc.commander || 'Admin',
    `${trip ? 'TRIPPED' : 'RESET'} circuit breaker: ${reason || 'Manual override'}`
  );

  res.json({
    message: `Circuit breaker ${trip ? 'activated' : 'deactivated'}`,
    circuitBreakerActive: inc.circuitBreakerActive,
  });
});

app.post('/api/v1/incidents/:id/resolve', async (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });

  const { resolutionNote, author } = req.body;
  inc.status = 'RESOLVED';
  inc.resolvedAt = new Date().toISOString();
  inc.circuitBreakerActive = false;

  timelineTracker.addEntry(
    inc.id,
    'STATUS_CHANGED',
    author || inc.commander || 'Responder',
    `Incident resolved: ${resolutionNote || 'All checks passing'}`
  );

  if (inc.pagerDutyIncidentId) await pdClient.resolveAlert(inc.pagerDutyIncidentId);
  if (inc.opsgenieAlertId) await ogClient.closeAlert(inc.opsgenieAlertId, resolutionNote);

  inc.timeline = timelineTracker.getTimeline(inc.id);
  res.json({ message: 'Incident resolved', incident: inc });
});

// ── On-Call & Escalation Endpoints ───────────────────────────────────────

app.get('/api/v1/on-call/current', (req: Request, res: Response) => {
  const shift = scheduler.getCurrentShift();
  res.json({
    shift,
    primary: scheduler.getActivePrimary(),
    secondary: scheduler.getActiveSecondary(),
  });
});

app.post('/api/v1/on-call/override', (req: Request, res: Response) => {
  const { team, user, durationHours } = req.body;
  const override = scheduler.setShiftOverride(team || 'audit-ledger-core', user, durationHours || 8);
  res.json({ message: 'Shift override registered', shift: override });
});

// ── Postmortem Endpoints ─────────────────────────────────────────────────

app.post('/api/v1/incidents/:id/postmortem', (req: Request, res: Response) => {
  const inc = incidents.get(req.params.id);
  if (!inc) return res.status(404).json({ error: 'Incident not found' });

  const {
    investigator,
    executiveSummary,
    rootCause,
    fiveWhys,
    actionItems,
    whatWentWell,
    whatWentWrong,
    whereWeGotLucky,
  } = req.body;

  inc.timeline = timelineTracker.getTimeline(inc.id);

  const report = PostmortemGenerator.generateTemplate(
    inc,
    investigator || 'Incident Review Team',
    executiveSummary || 'Post-incident analysis of contract event anomaly.',
    rootCause || 'Unforeseen ledger congestion and queue saturation.',
    fiveWhys || ['High event throughput spike', 'Rate limiter hit capacity', 'Relayer batching backed up'],
    actionItems || [],
    whatWentWell || ['Fast automated detection by Alertmanager'],
    whatWentWrong || ['Escalation tier 1 took 8 minutes to respond'],
    whereWeGotLucky || ['No transactions were lost or dropped']
  );

  const markdown = PostmortemGenerator.formatToMarkdown(report);

  res.json({
    report,
    markdown,
  });
});

app.get('/healthz', (req: Request, res: Response) => {
  res.json({ status: 'ok', activeIncidents: incidents.size });
});

if (process.env.NODE_ENV !== 'test') {
  app.listen(port, () => {
    console.log(`[IncidentManager] Service running on port ${port}`);
  });
}

export default app;
