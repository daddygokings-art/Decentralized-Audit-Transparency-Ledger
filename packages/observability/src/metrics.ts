import { MetricDefinition, MetricType } from './types';

export interface MetricLabelSet {
  [key: string]: string | number;
}

export class Counter {
  private values: Map<string, number> = new Map();

  constructor(public readonly def: MetricDefinition) {}

  private getLabelKey(labels: MetricLabelSet = {}): string {
    const sorted = Object.keys(labels)
      .sort()
      .map((k) => `${k}="${labels[k]}"`)
      .join(',');
    return sorted;
  }

  public inc(labels: MetricLabelSet = {}, value = 1): void {
    if (value < 0) throw new Error('Counter increment value must be non-negative');
    const key = this.getLabelKey(labels);
    const curr = this.values.get(key) || 0;
    this.values.set(key, curr + value);
  }

  public get(labels: MetricLabelSet = {}): number {
    const key = this.getLabelKey(labels);
    return this.values.get(key) || 0;
  }

  public getAll(): Array<{ labels: string; value: number }> {
    return Array.from(this.values.entries()).map(([labels, value]) => ({ labels, value }));
  }

  public reset(): void {
    this.values.clear();
  }
}

export class Gauge {
  private values: Map<string, number> = new Map();

  constructor(public readonly def: MetricDefinition) {}

  private getLabelKey(labels: MetricLabelSet = {}): string {
    const sorted = Object.keys(labels)
      .sort()
      .map((k) => `${k}="${labels[k]}"`)
      .join(',');
    return sorted;
  }

  public set(labelsOrValue: MetricLabelSet | number, value?: number): void {
    if (typeof labelsOrValue === 'number') {
      this.values.set('', labelsOrValue);
    } else {
      const key = this.getLabelKey(labelsOrValue);
      this.values.set(key, value !== undefined ? value : 0);
    }
  }

  public inc(labels: MetricLabelSet = {}, value = 1): void {
    const key = this.getLabelKey(labels);
    const curr = this.values.get(key) || 0;
    this.values.set(key, curr + value);
  }

  public dec(labels: MetricLabelSet = {}, value = 1): void {
    const key = this.getLabelKey(labels);
    const curr = this.values.get(key) || 0;
    this.values.set(key, curr - value);
  }

  public get(labels: MetricLabelSet = {}): number {
    const key = this.getLabelKey(labels);
    return this.values.get(key) || 0;
  }

  public getAll(): Array<{ labels: string; value: number }> {
    return Array.from(this.values.entries()).map(([labels, value]) => ({ labels, value }));
  }

  public reset(): void {
    this.values.clear();
  }
}

export interface HistogramBucketValues {
  counts: Map<number, number>; // bucket threshold -> cumulative count
  sum: number;
  count: number;
}

export class Histogram {
  private buckets: number[];
  private values: Map<string, HistogramBucketValues> = new Map();

  constructor(public readonly def: MetricDefinition) {
    this.buckets = (def.buckets || [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10]).sort(
      (a, b) => a - b
    );
  }

  private getLabelKey(labels: MetricLabelSet = {}): string {
    const sorted = Object.keys(labels)
      .sort()
      .map((k) => `${k}="${labels[k]}"`)
      .join(',');
    return sorted;
  }

  public observe(labelsOrValue: MetricLabelSet | number, val?: number): void {
    let labels: MetricLabelSet = {};
    let value: number;

    if (typeof labelsOrValue === 'number') {
      value = labelsOrValue;
    } else {
      labels = labelsOrValue;
      value = val !== undefined ? val : 0;
    }

    const key = this.getLabelKey(labels);
    let bucketVal = this.values.get(key);
    if (!bucketVal) {
      const counts = new Map<number, number>();
      for (const b of this.buckets) {
        counts.set(b, 0);
      }
      bucketVal = { counts, sum: 0, count: 0 };
      this.values.set(key, bucketVal);
    }

    bucketVal.sum += value;
    bucketVal.count += 1;

    for (const b of this.buckets) {
      if (value <= b) {
        bucketVal.counts.set(b, (bucketVal.counts.get(b) || 0) + 1);
      }
    }
  }

  public get(labels: MetricLabelSet = {}): HistogramBucketValues | undefined {
    const key = this.getLabelKey(labels);
    return this.values.get(key);
  }

  public getAll(): Array<{ labels: string; data: HistogramBucketValues }> {
    return Array.from(this.values.entries()).map(([labels, data]) => ({ labels, data }));
  }

  public getBuckets(): number[] {
    return [...this.buckets];
  }

  public reset(): void {
    this.values.clear();
  }
}

export class MetricsRegistry {
  private counters: Map<string, Counter> = new Map();
  private gauges: Map<string, Gauge> = new Map();
  private histograms: Map<string, Histogram> = new Map();

  public registerCounter(def: MetricDefinition): Counter {
    let c = this.counters.get(def.name);
    if (!c) {
      c = new Counter(def);
      this.counters.set(def.name, c);
    }
    return c;
  }

  public registerGauge(def: MetricDefinition): Gauge {
    let g = this.gauges.get(def.name);
    if (!g) {
      g = new Gauge(def);
      this.gauges.set(def.name, g);
    }
    return g;
  }

  public registerHistogram(def: MetricDefinition): Histogram {
    let h = this.histograms.get(def.name);
    if (!h) {
      h = new Histogram(def);
      this.histograms.set(def.name, h);
    }
    return h;
  }

  public getCounter(name: string): Counter | undefined {
    return this.counters.get(name);
  }

  public getGauge(name: string): Gauge | undefined {
    return this.gauges.get(name);
  }

  public getHistogram(name: string): Histogram | undefined {
    return this.histograms.get(name);
  }

  public resetAll(): void {
    for (const c of this.counters.values()) c.reset();
    for (const g of this.gauges.values()) g.reset();
    for (const h of this.histograms.values()) h.reset();
  }

  /**
   * Serializes all metrics to Prometheus plaintext exposition format.
   */
  public toPrometheusFormat(): string {
    const lines: string[] = [];

    // Counters
    for (const counter of this.counters.values()) {
      lines.push(`# HELP ${counter.def.name} ${counter.def.help}`);
      lines.push(`# TYPE ${counter.def.name} counter`);
      const entries = counter.getAll();
      if (entries.length === 0) {
        lines.push(`${counter.def.name} 0`);
      } else {
        for (const e of entries) {
          const lbls = e.labels ? `{${e.labels}}` : '';
          lines.push(`${counter.def.name}${lbls} ${e.value}`);
        }
      }
    }

    // Gauges
    for (const gauge of this.gauges.values()) {
      lines.push(`# HELP ${gauge.def.name} ${gauge.def.help}`);
      lines.push(`# TYPE ${gauge.def.name} gauge`);
      const entries = gauge.getAll();
      if (entries.length === 0) {
        lines.push(`${gauge.def.name} 0`);
      } else {
        for (const e of entries) {
          const lbls = e.labels ? `{${e.labels}}` : '';
          lines.push(`${gauge.def.name}${lbls} ${e.value}`);
        }
      }
    }

    // Histograms
    for (const hist of this.histograms.values()) {
      lines.push(`# HELP ${hist.def.name} ${hist.def.help}`);
      lines.push(`# TYPE ${hist.def.name} histogram`);
      const entries = hist.getAll();
      for (const e of entries) {
        const baseLabels = e.labels;
        let cumulative = 0;
        for (const bucket of hist.getBuckets()) {
          cumulative += e.data.counts.get(bucket) || 0;
          const bucketLabels = baseLabels ? `${baseLabels},le="${bucket}"` : `le="${bucket}"`;
          lines.push(`${hist.def.name}_bucket{${bucketLabels}} ${cumulative}`);
        }
        const infLabels = baseLabels ? `${baseLabels},le="+Inf"` : `le="+Inf"`;
        lines.push(`${hist.def.name}_bucket{${infLabels}} ${e.data.count}`);
        const sumLabels = baseLabels ? `{${baseLabels}}` : '';
        lines.push(`${hist.def.name}_sum${sumLabels} ${e.data.sum}`);
        lines.push(`${hist.def.name}_count${sumLabels} ${e.data.count}`);
      }
    }

    return lines.join('\n') + '\n';
  }
}

/**
 * Standard Contract Event RED & Subsystem Metrics
 */
export function createStandardObservabilityMetrics(registry: MetricsRegistry = new MetricsRegistry()) {
  return {
    // RED Metrics - Ingestion
    eventIngestionTotal: registry.registerCounter({
      name: 'audit_event_ingestion_total',
      help: 'Total contract events ingested into the platform',
      type: 'counter',
      labelNames: ['contract_id', 'event_type', 'status'],
    }),
    eventIngestionDuration: registry.registerHistogram({
      name: 'audit_event_ingestion_duration_seconds',
      help: 'Duration of contract event ingestion cycle in seconds',
      type: 'histogram',
      labelNames: ['event_type'],
      buckets: [0.01, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10],
    }),
    errorsTotal: registry.registerCounter({
      name: 'audit_errors_total',
      help: 'Total system and ingestion errors by component',
      type: 'counter',
      labelNames: ['component', 'error_type'],
    }),

    // Subsystem Metrics - Bridge & Verification
    eventVerificationTotal: registry.registerCounter({
      name: 'audit_event_verification_total',
      help: 'Total cross-chain event proof verifications',
      type: 'counter',
      labelNames: ['chain', 'status'],
    }),
    eventVerificationDuration: registry.registerHistogram({
      name: 'audit_event_verification_duration_seconds',
      help: 'Latency of cross-chain proof verification',
      type: 'histogram',
      labelNames: ['chain'],
      buckets: [0.05, 0.1, 0.5, 1, 2, 5, 15, 30],
    }),
    bridgeRelayedEventsTotal: registry.registerCounter({
      name: 'audit_bridge_relayed_events_total',
      help: 'Total events successfully relayed across chains',
      type: 'counter',
      labelNames: ['target_chain', 'status'],
    }),

    // Database & Pipeline Health Gauges
    dbQueryDuration: registry.registerHistogram({
      name: 'audit_db_query_duration_seconds',
      help: 'Database query duration in seconds',
      type: 'histogram',
      labelNames: ['operation', 'table'],
      buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1, 5],
    }),
    activeSubmittersGauge: registry.registerGauge({
      name: 'audit_active_submitters_gauge',
      help: 'Current active unique event submitters',
      type: 'gauge',
      labelNames: ['window'],
    }),
    deadLetterQueueSize: registry.registerGauge({
      name: 'audit_dead_letter_queue_size',
      help: 'Number of failed events currently in dead letter queue',
      type: 'gauge',
      labelNames: ['service'],
    }),
    ingestionLagLedgers: registry.registerGauge({
      name: 'audit_ingestion_lag_ledgers',
      help: 'Lag in ledgers between on-chain tip and last indexed event',
      type: 'gauge',
      labelNames: ['contract_id'],
    }),
    registry,
  };
}
