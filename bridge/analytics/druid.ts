/**
 * Apache Druid Real-Time Analytics Client & Ingestion Spec Generator
 *
 * Implements Apache Druid streaming supervisor specifications, native JSON queries,
 * hyperUnique submitter approximations, and visualization connectors.
 */

import http from "http";
import https from "https";
import { URL } from "url";

export interface DruidConfig {
  routerHost: string;
  routerPort: number;
  dataSource: string;
  secure?: boolean;
  timeoutMs?: number;
}

export interface DruidSupervisorSpec {
  type: string;
  dataSchema: {
    dataSource: string;
    timestampSpec: { column: string; format: string };
    dimensionsSpec: {
      dimensions: Array<string | { type: string; name: string }>;
    };
    metricsSpec: Array<{
      type: string;
      name: string;
      fieldName?: string;
    }>;
    granularitySpec: {
      type: string;
      segmentGranularity: string;
      queryGranularity: string;
      rollup: boolean;
    };
  };
  ioConfig: {
    type: string;
    topic?: string;
    consumerProperties?: Record<string, string>;
    inputFormat?: { type: string };
  };
  tuningConfig: {
    type: string;
    maxRowsInMemory: number;
    intermediatePersistPeriod: string;
    maxPendingSubmits: number;
  };
}

export class DruidClient {
  private config: DruidConfig;

  constructor(config: Partial<DruidConfig> = {}) {
    this.config = {
      routerHost: config.routerHost ?? process.env.DRUID_ROUTER_HOST ?? "localhost",
      routerPort: config.routerPort ?? parseInt(process.env.DRUID_ROUTER_PORT ?? "8888", 10),
      dataSource: config.dataSource ?? process.env.DRUID_DATASOURCE ?? "audit_events",
      secure: config.secure ?? process.env.DRUID_SECURE === "true",
      timeoutMs: config.timeoutMs ?? 5000,
    };
  }

  /**
   * Generate Apache Druid Real-Time Kafka / Streaming Supervisor Ingestion Specification
   */
  public generateSupervisorSpec(kafkaTopic: string = "audit-events-stream"): DruidSupervisorSpec {
    return {
      type: "kafka",
      dataSchema: {
        dataSource: this.config.dataSource,
        timestampSpec: {
          column: "timestamp",
          format: "iso",
        },
        dimensionsSpec: {
          dimensions: [
            "event_hash",
            "tx_hash",
            { type: "string", name: "event_type" },
            { type: "string", name: "category" },
            { type: "string", name: "submitter" },
            { type: "string", name: "status" },
            { type: "long", name: "ledger_seq" },
          ],
        },
        metricsSpec: [
          { type: "count", name: "events_count" },
          { type: "doubleSum", name: "total_gas_spent", fieldName: "gas_spent" },
          { type: "doubleMin", name: "min_latency_ms", fieldName: "latency_ms" },
          { type: "doubleMax", name: "max_latency_ms", fieldName: "latency_ms" },
          { type: "hyperUnique", name: "unique_submitters_hll", fieldName: "submitter" },
        ],
        granularitySpec: {
          type: "uniform",
          segmentGranularity: "DAY",
          queryGranularity: "SECOND",
          rollup: true,
        },
      },
      ioConfig: {
        type: "kafka",
        topic: kafkaTopic,
        consumerProperties: {
          "bootstrap.servers": process.env.KAFKA_BOOTSTRAP_SERVERS ?? "localhost:9092",
        },
        inputFormat: {
          type: "json",
        },
      },
      tuningConfig: {
        type: "kafka",
        maxRowsInMemory: 100000,
        intermediatePersistPeriod: "PT10M",
        maxPendingSubmits: 100,
      },
    };
  }

  /**
   * Execute a native Druid timeseries aggregation query
   */
  public async queryTimeseries(params: {
    intervals: string; // e.g. "2026-08-27T00:00:00Z/2026-08-28T00:00:00Z"
    granularity: string; // "minute" | "hour" | "day"
    filter?: any;
  }): Promise<any[]> {
    const query = {
      queryType: "timeseries",
      dataSource: this.config.dataSource,
      intervals: [params.intervals],
      granularity: params.granularity,
      aggregations: [
        { type: "longSum", name: "events", fieldName: "events_count" },
        { type: "doubleSum", name: "gas", fieldName: "total_gas_spent" },
        { type: "hyperUniqueCardinality", name: "unique_submitters", fieldName: "unique_submitters_hll" },
      ],
      filter: params.filter,
    };

    return this.postQuery(query);
  }

  /**
   * Execute a native Druid TopN query (e.g. top submitters by gas or count)
   */
  public async queryTopN(params: {
    dimension: string;
    metric: string;
    threshold: number;
    intervals: string;
  }): Promise<any[]> {
    const query = {
      queryType: "topN",
      dataSource: this.config.dataSource,
      dimension: params.dimension,
      threshold: params.threshold,
      metric: params.metric,
      intervals: [params.intervals],
      granularity: "all",
      aggregations: [
        { type: "longSum", name: "events", fieldName: "events_count" },
        { type: "doubleSum", name: "gas", fieldName: "total_gas_spent" },
      ],
    };

    return this.postQuery(query);
  }

  /**
   * Execute native SQL query on Druid SQL endpoint
   */
  public async querySql(sql: string): Promise<any[]> {
    return this.postJson("/druid/v2/sql", { query: sql });
  }

  private postQuery(queryObj: any): Promise<any> {
    return this.postJson("/druid/v2", queryObj);
  }

  private postJson(path: string, body: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const payload = JSON.stringify(body);
      const isHttps = this.config.secure;
      const reqModule = isHttps ? https : http;

      const req = reqModule.request(
        {
          host: this.config.routerHost,
          port: this.config.routerPort,
          path,
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "Content-Length": Buffer.byteLength(payload),
          },
          timeout: this.config.timeoutMs,
        },
        (res) => {
          let data = "";
          res.on("data", (chunk) => (data += chunk));
          res.on("end", () => {
            if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
              try {
                resolve(JSON.parse(data));
              } catch (e) {
                resolve(data);
              }
            } else {
              reject(new Error(`Druid query failed status ${res.statusCode}: ${data}`));
            }
          });
        }
      );

      req.on("error", (err) => reject(err));
      req.on("timeout", () => {
        req.destroy();
        reject(new Error("Druid query timed out"));
      });

      req.write(payload);
      req.end();
    });
  }
}
