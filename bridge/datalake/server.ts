/**
 * Data Lake REST API Server
 *
 * Exposes endpoints for Time Travel queries, ACID commit logs,
 * schema evolution, and query federation.
 */

import http from "http";
import { URL } from "url";
import { IcebergTableManager } from "./iceberg";
import { DeltaLogManager } from "./deltalake";
import { TimeTravelEngine } from "./timetravel";
import { SchemaEvolutionManager } from "./schema-evolution";
import { QueryEngineAdapter } from "./query-engine";

export interface DataLakeServices {
  iceberg: IcebergTableManager;
  delta: DeltaLogManager;
  timetravel: TimeTravelEngine;
  schemaManager: SchemaEvolutionManager;
  queryAdapter: QueryEngineAdapter;
}

export function startDataLakeServer(
  services: DataLakeServices,
  port: number = 8086
): http.Server {
  const server = http.createServer(async (req, res) => {
    const parsedUrl = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    const pathname = parsedUrl.pathname;
    const method = req.method?.toUpperCase();

    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

    if (method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    try {
      // Health check
      if (pathname === "/api/v1/datalake/health" && method === "GET") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", timestamp: Date.now() }));
        return;
      }

      // List snapshots (Iceberg & Delta)
      if (pathname === "/api/v1/datalake/snapshots" && method === "GET") {
        const format = parsedUrl.searchParams.get("format") || "iceberg";
        if (format === "iceberg") {
          const meta = services.iceberg.getTableMetadata();
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ snapshots: meta.snapshots, currentSnapshotId: meta.currentSnapshotId }));
        } else {
          const commits = services.delta.getAllCommits();
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ commits, currentVersion: services.delta.getCurrentVersion() }));
        }
        return;
      }

      // Time travel query resolver
      if (pathname === "/api/v1/datalake/timetravel" && method === "GET") {
        const format = (parsedUrl.searchParams.get("format") as "iceberg" | "delta") || "delta";
        const asOfVersion = parsedUrl.searchParams.get("asOfVersion") || undefined;
        const asOfTimestampStr = parsedUrl.searchParams.get("asOfTimestamp");
        const asOfTimestamp = asOfTimestampStr ? parseInt(asOfTimestampStr, 10) : undefined;

        const resolved = services.timetravel.resolveSnapshot({
          tableFormat: format,
          asOfVersion,
          asOfTimestamp,
        });

        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ query: "time_travel_resolved", result: resolved }));
        return;
      }

      // Schema history
      if (pathname === "/api/v1/datalake/schema/history" && method === "GET") {
        const history = services.schemaManager.getSchemaHistory();
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ schemas: history, activeVersion: services.schemaManager.getLatestSchema().version }));
        return;
      }

      // Schema evolution endpoint
      if (pathname === "/api/v1/datalake/schema/evolve" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const newSchema = services.schemaManager.evolveSchema(json.columns || []);
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ status: "evolved", schema: newSchema }));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "Endpoint not found" }));
    } catch (err: any) {
      res.writeHead(500, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: err.message }));
    }
  });

  server.listen(port);
  return server;
}
