import { ContractStatistics, Event, AuditLedgerError } from './types';

export type Transport = (method: string, params: any[]) => Promise<any>;

export interface RetryOptions {
  maxRetries?: number;
  baseDelayMs?: number;
}

export interface BatchProgress {
  completed: number;
  total: number;
}

export class AuditLedgerClient {
  transport: Transport;
  contractId?: string;
  maxRetries: number;
  baseDelayMs: number;

  constructor(transport: Transport, contractId?: string, retryOptions: RetryOptions = {}) {
    this.transport = transport;
    this.contractId = contractId;
    this.maxRetries = retryOptions.maxRetries ?? 3;
    this.baseDelayMs = retryOptions.baseDelayMs ?? 500;
  }

  static fromRpc(rpcUrl: string, contractId?: string, retryOptions: RetryOptions = {}) {
    const transport: Transport = async (method, params) => {
      try {
        const res = await fetch(rpcUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ method, params }),
        });
        if (!res.ok) throw new AuditLedgerError('Transport error', undefined, res.status);
        const json = await res.json();
        if (json.error) throw new AuditLedgerError(json.error.message, json.error.code, res.status);
        return json.result;
      } catch (err) {
        if (err instanceof AuditLedgerError) throw err;
        throw err;
      }
    };
    return new AuditLedgerClient(transport, contractId, retryOptions);
  }

  private async sleep(ms: number) {
    await new Promise((resolve) => setTimeout(resolve, ms));
  }

  private isRetryableError(err: unknown) {
    if (err instanceof AuditLedgerError) {
      return err.status === 429 || err.status === 503;
    }
    if (err instanceof TypeError) return true;
    if (typeof err === 'object' && err !== null) {
      const error = err as { name?: string; code?: string; status?: number };
      if (error.status === 429 || error.status === 503) return true;
      if (error.name === 'FetchError' || error.name === 'NetworkError') return true;
      if (error.code && ['ECONNRESET', 'ETIMEDOUT', 'EAI_AGAIN', 'ENOTFOUND'].includes(error.code)) return true;
    }
    return false;
  }

  private async callTransport<T>(method: string, params: any[]): Promise<T> {
    let attempt = 0;
    for (;;) {
      try {
        return await this.transport(method, params);
      } catch (err) {
        if (attempt >= this.maxRetries || !this.isRetryableError(err)) {
          throw err;
        }
        const delay = this.baseDelayMs * (2 ** attempt);
        attempt += 1;
        await this.sleep(delay);
      }
    }
  }

  async initialize(owner: string, globalMaxLogs: number, maxMetadataBytes: number = 4096) {
    return this.callTransport('initialize', [owner, globalMaxLogs, maxMetadataBytes]);
  }

  async logEvent(submitter: string, eventType: string, metadata: string) : Promise<string> {
    return this.callTransport('log_event', [submitter, eventType, metadata]);
  }

  async logEvents(events: { submitter: string; type: string; metadata: string }[]): Promise<number[]> {
    return this.callTransport('log_events', [events]);
  }

  async getEvent(id: string): Promise<Event> {
    return this.callTransport('get_event', [id]);
  }

  async totalEvents(): Promise<number> {
    return this.callTransport('total_events', []);
  }

  async eventCount(type: string): Promise<number> {
    return this.callTransport('event_count', [type]);
  }

  async getEventByType(type: string, index: number): Promise<Event> {
    return this.callTransport('get_event_by_type', [type, index]);
  }

  async getStatistics(): Promise<ContractStatistics> {
    return this.callTransport('get_statistics', []);
  }

  // Governance helpers (examples)
  async setGlobalMaxLogs(caller: string, newMax: number) {
    return this.callTransport('set_global_max_logs', [caller, newMax]);
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
