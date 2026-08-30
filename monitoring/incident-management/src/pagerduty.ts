import axios from 'axios';
import { IncidentAlertPayload, SeverityLevel } from './types';

export class PagerDutyClient {
  private routingKey: string;
  private apiEndpoint: string;

  constructor(routingKey: string = process.env.PAGERDUTY_ROUTING_KEY || 'mock-pd-key') {
    this.routingKey = routingKey;
    this.apiEndpoint = process.env.PAGERDUTY_EVENTS_API || 'https://events.pagerduty.com/v2/enqueue';
  }

  private mapSeverity(sev: SeverityLevel): 'critical' | 'error' | 'warning' | 'info' {
    switch (sev) {
      case 'SEV-1':
        return 'critical';
      case 'SEV-2':
        return 'error';
      case 'SEV-3':
        return 'warning';
      case 'SEV-4':
      case 'SEV-5':
        return 'info';
      default:
        return 'warning';
    }
  }

  public async triggerAlert(payload: IncidentAlertPayload): Promise<{ dedupKey: string; status: string }> {
    const dedupKey = payload.dedupKey || `audit-ledger-${payload.source}-${Date.now()}`;
    const pdPayload = {
      routing_key: this.routingKey,
      event_action: 'trigger',
      dedup_key: dedupKey,
      payload: {
        summary: `[${payload.severity}] ${payload.summary}`,
        source: payload.source,
        severity: this.mapSeverity(payload.severity),
        timestamp: payload.timestamp,
        custom_details: {
          contractAddress: payload.contractAddress,
          ...payload.details,
        },
      },
      links: [
        {
          href: `https://ledger-ops.internal/incidents/${dedupKey}`,
          text: 'Incident Console',
        },
      ],
    };

    if (process.env.NODE_ENV === 'test' || this.routingKey === 'mock-pd-key') {
      return { dedupKey, status: 'simulated_triggered' };
    }

    try {
      const resp = await axios.post(this.apiEndpoint, pdPayload, {
        headers: { 'Content-Type': 'application/json' },
        timeout: 5000,
      });
      return { dedupKey: resp.data.dedup_key || dedupKey, status: resp.data.status };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[PagerDuty] Failed to trigger alert: ${errorMsg}`);
      return { dedupKey, status: 'fallback_queued' };
    }
  }

  public async acknowledgeAlert(dedupKey: string): Promise<{ status: string }> {
    const pdPayload = {
      routing_key: this.routingKey,
      event_action: 'acknowledge',
      dedup_key: dedupKey,
    };

    if (process.env.NODE_ENV === 'test' || this.routingKey === 'mock-pd-key') {
      return { status: 'simulated_acknowledged' };
    }

    try {
      const resp = await axios.post(this.apiEndpoint, pdPayload, { timeout: 5000 });
      return { status: resp.data.status };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[PagerDuty] Failed to ack alert: ${errorMsg}`);
      return { status: 'fallback_ack' };
    }
  }

  public async resolveAlert(dedupKey: string): Promise<{ status: string }> {
    const pdPayload = {
      routing_key: this.routingKey,
      event_action: 'resolve',
      dedup_key: dedupKey,
    };

    if (process.env.NODE_ENV === 'test' || this.routingKey === 'mock-pd-key') {
      return { status: 'simulated_resolved' };
    }

    try {
      const resp = await axios.post(this.apiEndpoint, pdPayload, { timeout: 5000 });
      return { status: resp.data.status };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[PagerDuty] Failed to resolve alert: ${errorMsg}`);
      return { status: 'fallback_resolved' };
    }
  }
}
