import { Event, AuditLedgerError } from './types';

export type Transport = (method: string, params: any[]) => Promise<any>;

export interface BatchProgress {
  completed: number;
  total: number;
}

export class AuditLedgerClient {
  transport: Transport;
  contractId?: string;

  constructor(transport: Transport, contractId?: string) {
    this.transport = transport;
    this.contractId = contractId;
  }

  static fromRpc(rpcUrl: string, contractId?: string) {
    const transport: Transport = async (method, params) => {
      const res = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ method, params }),
      });
      if (!res.ok) throw new AuditLedgerError('Transport error');
      const json = await res.json();
      if (json.error) throw new AuditLedgerError(json.error.message, json.error.code);
      return json.result;
    };
    return new AuditLedgerClient(transport, contractId);
  }

  async initialize(owner: string, globalMaxLogs: number) {
    return this.transport('initialize', [owner, globalMaxLogs]);
  }

  async logEvent(submitter: string, eventType: string, metadata: string) : Promise<string> {
    return this.transport('log_event', [submitter, eventType, metadata]);
  }

  async getEvent(id: string): Promise<Event> {
    return this.transport('get_event', [id]);
  }

  async totalEvents(): Promise<number> {
    return this.transport('total_events', []);
  }

  async eventCount(type: string): Promise<number> {
    return this.transport('event_count', [type]);
  }

  async getEventByType(type: string, index: number): Promise<Event> {
    return this.transport('get_event_by_type', [type, index]);
  }

  // Governance helpers (examples)
  async setGlobalMaxLogs(caller: string, newMax: number) {
    return this.transport('set_global_max_logs', [caller, newMax]);
  }

  // Event watching via WebSocket
  watchEvents(wsUrl: string, type: string | null, cb: (evt: Event) => void) {
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => {
      const msg = type ? { action: 'subscribe', type } : { action: 'subscribe_all' };
      ws.send(JSON.stringify(msg));
    };
    ws.onmessage = (m) => {
      try {
        const data = JSON.parse(m.data as string);
        if (data.type === 'event_logged') cb(data.event as Event);
      } catch (e) {
        // ignore parse errors
      }
    };
    return ws;
  }

  // Batch submission with progress callback
  async submitBatch(events: { submitter: string; type: string; metadata: string }[], onProgress?: (p: BatchProgress) => void) {
    const total = events.length;
    let completed = 0;
    for (const ev of events) {
      await this.logEvent(ev.submitter, ev.type, ev.metadata);
      completed++;
      onProgress?.({ completed, total });
    }
  }
}

export default AuditLedgerClient;
