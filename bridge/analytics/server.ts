/**
 * Real-Time Analytics HTTP Server
 *
 * Exposes sub-second query endpoints, real-time telemetry metrics,
 * rollup reports, and Grafana / visualization tool integrations.
 */

import http from "http";
import { URL } from "url";
import { RealtimeAnalyticsEngine } from "./realtime-engine";

export function startAnalyticsServer(
  engine: RealtimeAnalyticsEngine,
  port: number = 8085
): http.Server {
  const server = http.createServer(async (req, res) => {
    const parsedUrl = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    const pathname = parsedUrl.pathname;
    const method = req.method?.toUpperCase();

    // Enable CORS for visualization dashboards (Grafana, Superset, Tableau)
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

    if (method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    try {
      // 1. Health endpoint
      if (pathname === "/api/v1/analytics/health" && method === "GET") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", timestamp: Date.now() }));
        return;
      }

      // 2. Real-time telemetry snapshot (sub-second queries)
      if (pathname === "/api/v1/analytics/realtime/summary" && method === "GET") {
        const snapshot = engine.getRealtimeSnapshot();
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify(snapshot));
        return;
      }

      // 3. Rollup aggregations
      if (pathname === "/api/v1/analytics/rollup" && method === "GET") {
        const fromParam = parsedUrl.searchParams.get("from");
        const toParam = parsedUrl.searchParams.get("to");
        const granularity = (parsedUrl.searchParams.get("granularity") as "hour" | "day") || "hour";

        const fromTime = fromParam ? new Date(fromParam) : new Date(Date.now() - 86400000);
        const toTime = toParam ? new Date(toParam) : new Date();

        const rollups = await engine.getRollups(fromTime, toTime, granularity);
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ rollups, count: rollups.length }));
        return;
      }

      // 4. Sub-second SQL Query endpoint
      if (pathname === "/api/v1/analytics/query" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", async () => {
          try {
            const json = JSON.parse(body || "{}");
            const sql = json.query || json.sql;
            if (!sql) {
              res.writeHead(400, { "Content-Type": "application/json" });
              res.end(JSON.stringify({ error: "Missing query or sql field in request body" }));
              return;
            }

            const result = await engine.executeQuery(sql);
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify(result));
          } catch (e: any) {
            res.writeHead(500, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // 5. Grafana SimpleJson / Infinity datasource endpoint
      if (pathname === "/api/v1/analytics/visualization/grafana" && method === "POST") {
        const snapshot = engine.getRealtimeSnapshot();
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify([
          { target: "current_tps", datapoints: [[snapshot.currentTps, Date.now()]] },
          { target: "avg_latency_ms", datapoints: [[snapshot.averageLatencyMs, Date.now()]] },
          { target: "p95_latency_ms", datapoints: [[snapshot.p95LatencyMs, Date.now()]] },
          { target: "p99_latency_ms", datapoints: [[snapshot.p99LatencyMs, Date.now()]] },
        ]));
        return;
      }

      // Fallthrough: 404
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
