import { randomBytes } from 'crypto';
import {
  Span,
  SpanContext,
  SpanStatus,
  SpanEvent,
  Tracer,
  TracerOptions,
  Sampler,
  TraceExporter,
} from './types';

export const AuditAttributes = {
  CONTRACT_ID: 'audit.contract_id',
  EVENT_TYPE: 'audit.event_type',
  EVENT_HASH: 'audit.event_hash',
  LEDGER_SEQ: 'audit.ledger_seq',
  SUBMITTER: 'audit.submitter',
  VERIFICATION_STATUS: 'audit.verification_status',
  TARGET_CHAIN: 'bridge.target_chain',
  RELAY_TX_HASH: 'bridge.relay_tx_hash',
  DB_STATEMENT: 'db.statement',
  DB_MIGRATION_VERSION: 'db.migration.version',
  HTTP_METHOD: 'http.method',
  HTTP_STATUS_CODE: 'http.status_code',
  HTTP_ROUTE: 'http.route',
} as const;

export class TraceContextUtils {
  public static generateTraceId(): string {
    return randomBytes(16).toString('hex');
  }

  public static generateSpanId(): string {
    return randomBytes(8).toString('hex');
  }

  /**
   * Serializes a SpanContext to W3C traceparent header format:
   * version-traceid-spanid-traceflags (e.g. 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01)
   */
  public static serializeTraceparent(context: SpanContext): string {
    const flags = context.traceFlags.toString(16).padStart(2, '0');
    return `00-${context.traceId}-${context.spanId}-${flags}`;
  }

  /**
   * Parses W3C traceparent header format.
   */
  public static parseTraceparent(header: string): SpanContext | null {
    if (!header || typeof header !== 'string') return null;
    const parts = header.trim().split('-');
    if (parts.length < 4) return null;

    const [version, traceId, spanId, flagsStr] = parts;
    if (version !== '00') return null;
    if (traceId.length !== 32 || spanId.length !== 16) return null;

    const traceFlags = parseInt(flagsStr, 16) || 0;
    return {
      traceId,
      spanId,
      traceFlags,
    };
  }
}

export class ErrorPrioritySampler implements Sampler {
  constructor(private sampleRate = 1.0) {}

  public shouldSample(
    _traceId: string,
    _spanName: string,
    _attributes?: Record<string, any>,
    parentContext?: SpanContext
  ): { isSampled: boolean; attributes?: Record<string, any> } {
    if (parentContext && (parentContext.traceFlags & 1) === 1) {
      return { isSampled: true };
    }

    if (this.sampleRate >= 1.0) {
      return { isSampled: true };
    }

    const isSampled = Math.random() < this.sampleRate;
    return { isSampled };
  }
}

export class SpanImpl implements Span {
  private attributes: Record<string, any> = {};
  private events: SpanEvent[] = [];
  private status: SpanStatus = { code: 'UNSET' };
  private startTime: number;
  private endTime?: number;
  private durationMs = 0;

  constructor(
    public name: string,
    private spanContext: SpanContext,
    private parentSpanContext?: SpanContext,
    private onEnd?: (span: Span) => void
  ) {
    this.startTime = Date.now();
  }

  public context(): SpanContext {
    return this.spanContext;
  }

  public setAttribute(key: string, value: any): this {
    this.attributes[key] = value;
    return this;
  }

  public setAttributes(attrs: Record<string, any>): this {
    Object.assign(this.attributes, attrs);
    return this;
  }

  public addEvent(name: string, attributes?: Record<string, any>): this {
    this.events.push({
      name,
      timestamp: Date.now(),
      attributes,
    });
    return this;
  }

  public recordException(error: Error | string): this {
    const message = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? error.stack : undefined;
    const name = error instanceof Error ? error.name : 'Error';

    this.setStatus({ code: 'ERROR', message });
    this.addEvent('exception', {
      'exception.type': name,
      'exception.message': message,
      'exception.stacktrace': stack,
    });

    // Ensure error traces are flagged as sampled (Error Priority Sampling)
    this.spanContext.traceFlags |= 1;
    return this;
  }

  public setStatus(status: SpanStatus): this {
    this.status = status;
    return this;
  }

  public end(endTime?: number): void {
    if (this.endTime !== undefined) return; // already ended
    this.endTime = endTime || Date.now();
    this.durationMs = Math.max(0, this.endTime - this.startTime);
    if (this.onEnd) {
      this.onEnd(this);
    }
  }

  public isRecording(): boolean {
    return this.endTime === undefined;
  }

  public getDurationMs(): number {
    return this.durationMs || (Date.now() - this.startTime);
  }

  public getAttributes(): Record<string, any> {
    return { ...this.attributes };
  }

  public getEvents(): SpanEvent[] {
    return [...this.events];
  }

  public getStatus(): SpanStatus {
    return { ...this.status };
  }
}

export class InMemoryTraceExporter implements TraceExporter {
  public exportedSpans: Span[] = [];

  public async export(spans: Span[]): Promise<void> {
    this.exportedSpans.push(...spans);
  }

  public async shutdown(): Promise<void> {
    this.exportedSpans = [];
  }

  public clear(): void {
    this.exportedSpans = [];
  }
}

export class OpenTelemetryTracer implements Tracer {
  private serviceName: string;
  private environment: string;
  private sampler: Sampler;
  private exporter?: TraceExporter;
  private currentContext: SpanContext | null = null;

  constructor(options: TracerOptions) {
    this.serviceName = options.serviceName;
    this.environment = options.environment || process.env.NODE_ENV || 'development';
    this.sampler = options.sampler || new ErrorPrioritySampler(1.0);
    this.exporter = options.exporter;
  }

  public startSpan(
    name: string,
    options: { parent?: SpanContext; attributes?: Record<string, any> } = {}
  ): Span {
    const parentContext = options.parent || this.currentContext || undefined;
    const traceId = parentContext ? parentContext.traceId : TraceContextUtils.generateTraceId();
    const spanId = TraceContextUtils.generateSpanId();

    const sampleDecision = this.sampler.shouldSample(traceId, name, options.attributes, parentContext);
    const traceFlags = sampleDecision.isSampled ? 1 : 0;

    const spanContext: SpanContext = {
      traceId,
      spanId,
      traceFlags,
    };

    const span = new SpanImpl(name, spanContext, parentContext, (endedSpan) => {
      if ((spanContext.traceFlags & 1) === 1 && this.exporter) {
        this.exporter.export([endedSpan]).catch(() => {});
      }
    });

    span.setAttribute('service.name', this.serviceName);
    span.setAttribute('deployment.environment', this.environment);

    if (options.attributes) {
      span.setAttributes(options.attributes);
    }

    return span;
  }

  public async withSpan<T>(
    name: string,
    fn: (span: Span) => Promise<T>,
    options: { parent?: SpanContext; attributes?: Record<string, any> } = {}
  ): Promise<T> {
    const span = this.startSpan(name, options);
    const prevContext = this.currentContext;
    this.currentContext = span.context();

    try {
      const result = await fn(span);
      if (span.getStatus().code === 'UNSET') {
        span.setStatus({ code: 'OK' });
      }
      return result;
    } catch (err) {
      span.recordException(err as Error);
      throw err;
    } finally {
      span.end();
      this.currentContext = prevContext;
    }
  }

  public extractContext(carrier: Record<string, string | undefined>): SpanContext | null {
    if (!carrier) return null;
    const header = carrier['traceparent'] || carrier['Traceparent'];
    if (!header) return null;
    return TraceContextUtils.parseTraceparent(header);
  }

  public injectContext(context: SpanContext, carrier: Record<string, string>): void {
    if (!context || !carrier) return;
    carrier['traceparent'] = TraceContextUtils.serializeTraceparent(context);
  }
}
