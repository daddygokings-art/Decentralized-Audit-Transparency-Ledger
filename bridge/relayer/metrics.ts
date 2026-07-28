/**
 * Bridge Metrics Exporter (#254)
 *
 * Exposes Prometheus-compatible metrics for the bridge monitoring dashboard:
 *   - Bridge status (relayer health, last index, uptime)
 *   - Event processing metrics (processed, submitted, skipped)
 *   - Error rate tracking (by category)
 *   - Latency histograms (Stellar poll, EVM submission, end-to-end)
 *   - Replay queue stats
 *
 * Mount this exporter in the relayer process or run it as a sidecar.
 */

import http from "http";

// ── Metric primitives ─────────────────────────────────────────────────────────

type Labels = Record<string, string>;

function labelsToString(labels: Labels): string {
  const pairs = Object.entries(labels)
    .map(([k, v]) => `${k}="${v.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`)
    .join(",");
  return pairs.length > 0 ? `{${pairs}}` : "";
}

class Counter {
  private values: Map<string, number> = new Map();

  constructor(public readonly name: string, public readonly help: string) {}

  inc(labels: Labels = {}, amount = 1): void {
    const key = JSON.stringify(labels);
    this.values.set(key, (this.values.get(key) ?? 0) + amount);
  }

  render(): string {
    const lines: string[] = [
      `# HELP ${this.name} ${this.help}`,
      `# TYPE ${this.name} counter`,
    ];
    for (const [labelJson, value] of this.values) {
      const labels: Labels = JSON.parse(labelJson);
      lines.push(`${this.name}${labelsToString(labels)} ${value}`);
    }
    return lines.join("\n");
  }
}

class Gauge {
  private values: Map<string, number> = new Map();

  constructor(public readonly name: string, public readonly help: string) {}

  set(value: number, labels: Labels = {}): void {
    this.values.set(JSON.stringify(labels), value);
  }

  inc(labels: Labels = {}, amount = 1): void {
    const key = JSON.stringify(labels);
    this.values.set(key, (this.values.get(key) ?? 0) + amount);
  }

  render(): string {
    const lines: string[] = [
      `# HELP ${this.name} ${this.help}`,
      `# TYPE ${this.name} gauge`,
    ];
    for (const [labelJson, value] of this.values) {
      const labels: Labels = JSON.parse(labelJson);
      lines.push(`${this.name}${labelsToString(labels)} ${value}`);
    }
    return lines.join("\n");
  }
}

/**
 * Simple histogram — tracks observations in configurable buckets.
 */
class Histogram {
  private buckets: number[];
  private bucketCounts: Map<string, number[]> = new Map();
  private sums: Map<string, number> = new Map();
  private counts: Map<string, number> = new Map();

  constructor(
    public readonly name: string,
    public readonly help: string,
    buckets: number[] = [5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000]
  ) {
    this.buckets = [...buckets].sort((a, b) => a - b);
  }

  observe(value: number, labels: Labels = {}): void {
    const key = JSON.stringify(labels);

    if (!this.bucketCounts.has(key)) {
      this.bucketCounts.set(key, new Array(this.buckets.length).fill(0));
      this.sums.set(key, 0);
      this.counts.set(key, 0);
    }

    const counts = this.bucketCounts.get(key)!;
    for (let i = 0; i < this.buckets.length; i++) {
      if (value <= this.buckets[i]) counts[i]++;
    }

    this.sums.set(key, (this.sums.get(key) ?? 0) + value);
    this.counts.set(key, (this.counts.get(key) ?? 0) + 1);
  }

  render(): string {
    const lines: string[] = [
      `# HELP ${this.name} ${this.help}`,
      `# TYPE ${this.name} histogram`,
    ];

    for (const [labelJson, counts] of this.bucketCounts) {
      const labels: Labels = JSON.parse(labelJson);

      for (let i = 0; i < this.buckets.length; i++) {
        const bucketLabels = { ...labels, le: String(this.buckets[i]) };
        lines.push(`${this.name}_bucket${labelsToString(bucketLabels)} ${counts[i]}`);
      }
      const infLabels = { ...labels, le: "+Inf" };
      lines.push(`${this.name}_bucket${labelsToString(infLabels)} ${this.counts.get(labelJson) ?? 0}`);
      lines.push(`${this.name}_sum${labelsToString(labels)} ${this.sums.get(labelJson) ?? 0}`);
      lines.push(`${this.name}_count${labelsToString(labels)} ${this.counts.get(labelJson) ?? 0}`);
    }

    return lines.join("\n");
  }
}

// ── Bridge metric definitions ─────────────────────────────────────────────────

/** Bridge status metrics (#254 — Bridge Status) */
export const bridgeRelayerStatus = new Gauge(
  "bridge_relayer_status",
  "Bridge relayer health: 0=ok, 1=degraded, 2=down"
);

export const bridgeRelayerLastIndex = new Gauge(
  "bridge_relayer_last_processed_index",
  "Most recent Stellar event index successfully relayed to EVM"
);

export const bridgeRelayerUptime = new Gauge(
  "bridge_relayer_uptime_seconds",
  "Seconds since the bridge relayer process started"
);

export const bridgePollsWithoutEvents = new Gauge(
  "bridge_relayer_polls_without_events",
  "Consecutive Stellar poll cycles with no new events"
);

export const bridgeProofCacheSize = new Gauge(
  "bridge_proof_cache_size",
  "Current number of entries in the LRU proof cache"
);

export const bridgeDedupSetSize = new Gauge(
  "bridge_dedup_set_size",
  "Current size of the event deduplication set"
);

/** Event processing metrics (#254 — Event Processing) */
export const bridgeEventsProcessed = new Counter(
  "bridge_events_processed_total",
  "Total Stellar events processed by the relayer"
);

export const bridgeEventsSubmitted = new Counter(
  "bridge_events_submitted_total",
  "Total EVM proof submissions attempted"
);

export const bridgeEventsSkipped = new Counter(
  "bridge_events_skipped_total",
  "Total events skipped due to deduplication"
);

export const bridgeEvmSubmissions = new Counter(
  "bridge_evm_submissions_total",
  "EVM submission outcomes by result label (success|failure|retry)"
);

/** Error rate tracking (#254 — Error Rate Tracking) */
export const bridgeErrors = new Counter(
  "bridge_errors_total",
  "Bridge errors by category (stellar_rpc|evm_submission|proof_build|transform)"
);

export const bridgeLastErrorTimestamp = new Gauge(
  "bridge_last_error_timestamp",
  "Unix timestamp of the most recent error by category"
);

/** Latency histograms (#254 — Latency Monitoring) */
export const bridgeStellarPollDuration = new Histogram(
  "bridge_stellar_poll_duration_ms",
  "Latency of Stellar RPC getEvents calls in milliseconds",
  [5, 10, 25, 50, 100, 250, 500, 1000, 2000, 5000]
);

export const bridgeEvmSubmissionDuration = new Histogram(
  "bridge_evm_submission_duration_ms",
  "Latency of EVM proof submission calls in milliseconds",
  [10, 25, 50, 100, 250, 500, 1000, 2000, 5000, 10000]
);

export const bridgeE2ELatency = new Histogram(
  "bridge_e2e_latency_ms",
  "End-to-end latency from Stellar event observed to EVM verification confirmed, in ms",
  [100, 250, 500, 1000, 2500, 5000, 10000, 30000, 60000]
);

/** Replay queue metrics (#253 + #254) */
export const bridgeReplayQueueDepth = new Gauge(
  "bridge_replay_queue_depth",
  "Number of events currently waiting in the replay queue"
);

export const bridgeReplaySucceeded = new Gauge(
  "bridge_replay_total_succeeded",
  "Total replay attempts that succeeded"
);

export const bridgeReplayFailed = new Gauge(
  "bridge_replay_total_failed",
  "Total events that permanently failed after max retries"
);

// ── Registry ──────────────────────────────────────────────────────────────────

const ALL_METRICS: Array<{ render(): string }> = [
  bridgeRelayerStatus,
  bridgeRelayerLastIndex,
  bridgeRelayerUptime,
  bridgePollsWithoutEvents,
  bridgeProofCacheSize,
  bridgeDedupSetSize,
  bridgeEventsProcessed,
  bridgeEventsSubmitted,
  bridgeEventsSkipped,
  bridgeEvmSubmissions,
  bridgeErrors,
  bridgeLastErrorTimestamp,
  bridgeStellarPollDuration,
  bridgeEvmSubmissionDuration,
  bridgeE2ELatency,
  bridgeReplayQueueDepth,
  bridgeReplaySucceeded,
  bridgeReplayFailed,
];

function renderAllMetrics(): string {
  return ALL_METRICS.map((m) => m.render()).join("\n\n") + "\n";
}

// ── HTTP server ───────────────────────────────────────────────────────────────

const METRICS_PORT = parseInt(process.env.BRIDGE_METRICS_PORT ?? "9101", 10);

/**
 * Starts the /metrics HTTP endpoint for Prometheus scraping.
 * Configure Prometheus to scrape this endpoint every 15–30 seconds.
 */
export function startBridgeMetricsServer(): void {
  const server = http.createServer((req, res) => {
    if (req.url === "/metrics" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "text/plain; version=0.0.4; charset=utf-8" });
      res.end(renderAllMetrics());
    } else if (req.url === "/healthz" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "ok" }));
    } else {
      res.writeHead(404, { "Content-Type": "text/plain" });
      res.end("Not Found");
    }
  });

  server.listen(METRICS_PORT, () => {
    console.log(`[bridge-metrics] Prometheus exporter listening on port ${METRICS_PORT} at /metrics`);
  });
}

// ── Convenience helpers ───────────────────────────────────────────────────────

/**
 * Records a bridge error and updates the last-error timestamp gauge.
 * @param category  Error category: stellar_rpc | evm_submission | proof_build | transform
 * @param message   Short error description (truncated to 128 chars for label safety).
 */
export function recordBridgeError(category: string, message: string): void {
  bridgeErrors.inc({ category });
  bridgeLastErrorTimestamp.set(Math.floor(Date.now() / 1000), {
    category,
    message: message.slice(0, 128),
  });
}

/**
 * Syncs replay queue stats into Prometheus gauges.
 * Call this after every processOnce() invocation.
 */
export function syncReplayStats(stats: {
  queueDepth: number;
  totalSucceeded: number;
  totalFailed: number;
}): void {
  bridgeReplayQueueDepth.set(stats.queueDepth);
  bridgeReplaySucceeded.set(stats.totalSucceeded);
  bridgeReplayFailed.set(stats.totalFailed);
}

export { renderAllMetrics };
