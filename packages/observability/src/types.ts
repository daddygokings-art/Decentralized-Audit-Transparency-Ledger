/**
 * Observability Stack Standardization Types
 * Unified OpenTelemetry Tracing, Prometheus Metrics, and Loki Logging.
 */

export interface SpanContext {
  traceId: string;
  spanId: string;
  traceFlags: number; // 1 = sampled
  traceState?: string;
}

export type SpanStatusCode = 'UNSET' | 'OK' | 'ERROR';

export interface SpanStatus {
  code: SpanStatusCode;
  message?: string;
}

export interface SpanEvent {
  name: string;
  timestamp: number;
  attributes?: Record<string, any>;
}

export interface Span {
  name: string;
  context(): SpanContext;
  setAttribute(key: string, value: any): this;
  setAttributes(attributes: Record<string, any>): this;
  addEvent(name: string, attributes?: Record<string, any>): this;
  recordException(error: Error | string): this;
  setStatus(status: SpanStatus): this;
  end(endTime?: number): void;
  isRecording(): boolean;
  getDurationMs(): number;
  getAttributes(): Record<string, any>;
  getEvents(): SpanEvent[];
  getStatus(): SpanStatus;
}

export interface TraceExporter {
  export(spans: Span[]): Promise<void>;
  shutdown(): Promise<void>;
}

export interface Sampler {
  shouldSample(
    traceId: string,
    spanName: string,
    attributes?: Record<string, any>,
    parentContext?: SpanContext
  ): { isSampled: boolean; attributes?: Record<string, any> };
}

export interface TracerOptions {
  serviceName: string;
  serviceVersion?: string;
  environment?: string;
  sampler?: Sampler;
  exporter?: TraceExporter;
}

export interface Tracer {
  startSpan(name: string, options?: { parent?: SpanContext; attributes?: Record<string, any> }): Span;
  withSpan<T>(name: string, fn: (span: Span) => Promise<T>, options?: { parent?: SpanContext; attributes?: Record<string, any> }): Promise<T>;
  extractContext(carrier: Record<string, string | undefined>): SpanContext | null;
  injectContext(context: SpanContext, carrier: Record<string, string>): void;
}

export type LogLevel = 'debug' | 'info' | 'warn' | 'error' | 'fatal';

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  message: string;
  service: string;
  environment?: string;
  trace_id?: string;
  span_id?: string;
  contract_id?: string;
  event_type?: string;
  context?: Record<string, any>;
  error?: {
    name: string;
    message: string;
    stack?: string;
  };
}

export type MetricType = 'counter' | 'gauge' | 'histogram';

export interface MetricDefinition {
  name: string;
  help: string;
  type: MetricType;
  labelNames?: string[];
  buckets?: number[]; // For histograms
}
