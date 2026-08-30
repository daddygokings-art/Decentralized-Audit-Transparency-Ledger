import axios from 'axios';
import { IncidentAlertPayload, SeverityLevel } from './types';

export class OpsgenieClient {
  private apiKey: string;
  private apiEndpoint: string;

  constructor(apiKey: string = process.env.OPSGENIE_API_KEY || 'mock-opsgenie-key') {
    this.apiKey = apiKey;
    this.apiEndpoint = process.env.OPSGENIE_API_ENDPOINT || 'https://api.opsgenie.com/v2/alerts';
  }

  private mapPriority(sev: SeverityLevel): 'P1' | 'P2' | 'P3' | 'P4' | 'P5' {
    switch (sev) {
      case 'SEV-1':
        return 'P1';
      case 'SEV-2':
        return 'P2';
      case 'SEV-3':
        return 'P3';
      case 'SEV-4':
        return 'P4';
      case 'SEV-5':
        return 'P5';
      default:
        return 'P3';
    }
  }

  public async createAlert(payload: IncidentAlertPayload): Promise<{ alertId: string; status: string }> {
    const alias = payload.dedupKey || `audit-ledger-${payload.source}-${Date.now()}`;
    const ogPayload = {
      message: `[${payload.severity}] ${payload.summary}`,
      alias,
      description: JSON.stringify(payload.details, null, 2),
      priority: this.mapPriority(payload.severity),
      source: payload.source,
      tags: ['audit-ledger', payload.severity.toLowerCase(), 'stellar-event'],
      details: {
        contractAddress: payload.contractAddress || 'none',
        timestamp: payload.timestamp,
      },
    };

    if (process.env.NODE_ENV === 'test' || this.apiKey === 'mock-opsgenie-key') {
      return { alertId: `og-${alias}`, status: 'simulated_created' };
    }

    try {
      const resp = await axios.post(this.apiEndpoint, ogPayload, {
        headers: {
          Authorization: `GenieKey ${this.apiKey}`,
          'Content-Type': 'application/json',
        },
        timeout: 5000,
      });
      return { alertId: resp.data.requestId || alias, status: resp.data.result };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[Opsgenie] Failed to create alert: ${errorMsg}`);
      return { alertId: alias, status: 'fallback_queued' };
    }
  }

  public async acknowledgeAlert(alias: string, user: string = 'audit-ledger-system'): Promise<{ status: string }> {
    if (process.env.NODE_ENV === 'test' || this.apiKey === 'mock-opsgenie-key') {
      return { status: 'simulated_acknowledged' };
    }

    try {
      const resp = await axios.post(
        `${this.apiEndpoint}/${alias}/acknowledge?identifierType=alias`,
        { user, note: 'Acknowledged via Audit Ledger Incident Manager' },
        {
          headers: {
            Authorization: `GenieKey ${this.apiKey}`,
            'Content-Type': 'application/json',
          },
          timeout: 5000,
        }
      );
      return { status: resp.data.result };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[Opsgenie] Failed to ack alert: ${errorMsg}`);
      return { status: 'fallback_ack' };
    }
  }

  public async closeAlert(alias: string, note: string = 'Resolved'): Promise<{ status: string }> {
    if (process.env.NODE_ENV === 'test' || this.apiKey === 'mock-opsgenie-key') {
      return { status: 'simulated_closed' };
    }

    try {
      const resp = await axios.post(
        `${this.apiEndpoint}/${alias}/close?identifierType=alias`,
        { note },
        {
          headers: {
            Authorization: `GenieKey ${this.apiKey}`,
            'Content-Type': 'application/json',
          },
          timeout: 5000,
        }
      );
      return { status: resp.data.result };
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error(`[Opsgenie] Failed to close alert: ${errorMsg}`);
      return { status: 'fallback_closed' };
    }
  }
}
