import { LogLevel, LogEntry, SpanContext } from './types';

export interface LoggerOptions {
  serviceName: string;
  environment?: string;
  minLevel?: LogLevel;
  sink?: (entry: LogEntry, jsonStr: string) => void;
}

const LEVEL_SEVERITY: Record<LogLevel, number> = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40,
  fatal: 50,
};

const SENSITIVE_KEYS = new Set([
  'password',
  'secret',
  'privatekey',
  'private_key',
  'token',
  'apikey',
  'api_key',
  'authorization',
  'auth',
]);

export class LokiLogger {
  private serviceName: string;
  private environment: string;
  private minLevel: LogLevel;
  private sink?: (entry: LogEntry, jsonStr: string) => void;
  private contextData: Record<string, any> = {};

  constructor(options: LoggerOptions) {
    this.serviceName = options.serviceName;
    this.environment = options.environment || process.env.NODE_ENV || 'development';
    this.minLevel = options.minLevel || (process.env.LOG_LEVEL as LogLevel) || 'info';
    this.sink = options.sink;
  }

  public setContext(context: Record<string, any>): this {
    Object.assign(this.contextData, context);
    return this;
  }

  private redact(obj: any): any {
    if (obj === null || obj === undefined) return obj;
    if (typeof obj !== 'object') return obj;

    if (Array.isArray(obj)) {
      return obj.map((item) => this.redact(item));
    }

    const cleaned: Record<string, any> = {};
    for (const [k, v] of Object.entries(obj)) {
      if (SENSITIVE_KEYS.has(k.toLowerCase())) {
        cleaned[k] = '[REDACTED]';
      } else if (typeof v === 'object' && v !== null) {
        cleaned[k] = this.redact(v);
      } else {
        cleaned[k] = v;
      }
    }
    return cleaned;
  }

  private shouldLog(level: LogLevel): boolean {
    return LEVEL_SEVERITY[level] >= LEVEL_SEVERITY[this.minLevel];
  }

  public log(
    level: LogLevel,
    message: string,
    meta?: {
      context?: Record<string, any>;
      spanContext?: SpanContext;
      error?: Error | string;
      contract_id?: string;
      event_type?: string;
    }
  ): LogEntry | null {
    if (!this.shouldLog(level)) return null;

    const mergedContext = this.redact({
      ...this.contextData,
      ...(meta?.context || {}),
    });

    let errorObj: LogEntry['error'];
    if (meta?.error) {
      if (meta.error instanceof Error) {
        errorObj = {
          name: meta.error.name,
          message: meta.error.message,
          stack: meta.error.stack,
        };
      } else {
        errorObj = {
          name: 'Error',
          message: String(meta.error),
        };
      }
    }

    const entry: LogEntry = {
      timestamp: new Date().toISOString(),
      level,
      message,
      service: this.serviceName,
      environment: this.environment,
      trace_id: meta?.spanContext?.traceId,
      span_id: meta?.spanContext?.spanId,
      contract_id: meta?.contract_id,
      event_type: meta?.event_type,
      context: Object.keys(mergedContext).length > 0 ? mergedContext : undefined,
      error: errorObj,
    };

    const jsonStr = JSON.stringify(entry);

    if (this.sink) {
      this.sink(entry, jsonStr);
    } else {
      if (level === 'error' || level === 'fatal') {
        process.stderr.write(jsonStr + '\n');
      } else {
        process.stdout.write(jsonStr + '\n');
      }
    }

    return entry;
  }

  public debug(message: string, meta?: Parameters<LokiLogger['log']>[2]): LogEntry | null {
    return this.log('debug', message, meta);
  }

  public info(message: string, meta?: Parameters<LokiLogger['log']>[2]): LogEntry | null {
    return this.log('info', message, meta);
  }

  public warn(message: string, meta?: Parameters<LokiLogger['log']>[2]): LogEntry | null {
    return this.log('warn', message, meta);
  }

  public error(message: string, meta?: Parameters<LokiLogger['log']>[2]): LogEntry | null {
    return this.log('error', message, meta);
  }

  public fatal(message: string, meta?: Parameters<LokiLogger['log']>[2]): LogEntry | null {
    return this.log('fatal', message, meta);
  }

  /**
   * Formats an array of log entries into a Loki push payload.
   */
  public static formatLokiPushPayload(entries: LogEntry[]): object {
    const streamsMap: Map<string, Array<[string, string]>> = new Map();

    for (const entry of entries) {
      const labels = {
        service: entry.service,
        level: entry.level,
        environment: entry.environment || 'production',
        ...(entry.contract_id ? { contract_id: entry.contract_id } : {}),
        ...(entry.event_type ? { event_type: entry.event_type } : {}),
      };

      const labelKey = Object.entries(labels)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([k, v]) => `${k}="${v}"`)
        .join(',');

      const nanoTime = String(new Date(entry.timestamp).getTime() * 1000000);
      const logLine = JSON.stringify(entry);

      if (!streamsMap.has(labelKey)) {
        streamsMap.set(labelKey, []);
      }
      streamsMap.get(labelKey)!.push([nanoTime, logLine]);
    }

    const streams = Array.from(streamsMap.entries()).map(([labelKey, values]) => {
      const streamLabels: Record<string, string> = {};
      labelKey.split(',').forEach((pair) => {
        const [k, v] = pair.split('=');
        if (k && v) streamLabels[k] = v.replace(/"/g, '');
      });
      return { stream: streamLabels, values };
    });

    return { streams };
  }
}
