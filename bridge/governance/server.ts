/**
 * Data Governance REST API Server
 *
 * Exposes endpoints for data catalog search, lineage graph queries,
 * quality scorecard retrieval, policy enforcement, and stewardship workflows.
 */

import http from "http";
import { URL } from "url";
import { DataCatalogManager } from "./catalog";
import { LineageTracker } from "./lineage";
import { DataQualityEngine } from "./quality";
import { PolicyEnforcementEngine } from "./policies";
import { StewardshipWorkflowManager } from "./stewardship";

export interface GovernanceServices {
  catalog: DataCatalogManager;
  lineage: LineageTracker;
  quality: DataQualityEngine;
  policies: PolicyEnforcementEngine;
  stewardship: StewardshipWorkflowManager;
}

export function startGovernanceServer(
  services: GovernanceServices,
  port: number = 8087
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
      if (pathname === "/api/v1/governance/health" && method === "GET") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", timestamp: Date.now() }));
        return;
      }

      // Catalog search
      if (pathname === "/api/v1/governance/catalog" && method === "GET") {
        const keyword = parsedUrl.searchParams.get("q") || undefined;
        const classification = (parsedUrl.searchParams.get("classification") as any) || undefined;
        const tag = (parsedUrl.searchParams.get("tag") as any) || undefined;

        const assets = services.catalog.searchAssets({ keyword, classification, tag });
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ assets, count: assets.length }));
        return;
      }

      // Lineage DAG
      if (pathname === "/api/v1/governance/lineage" && method === "GET") {
        const nodeId = parsedUrl.searchParams.get("nodeId");
        const graph = nodeId ? services.lineage.getUpstreamLineage(nodeId) : services.lineage.getFullGraph();
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ lineage: graph }));
        return;
      }

      // Quality scorecard
      if (pathname === "/api/v1/governance/quality" && method === "GET") {
        const assetId = parsedUrl.searchParams.get("assetId") || "asset-stellar-audit-events";
        const dummyEvents = [
          { event_hash: "0x11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff", ledger_seq: 123456, submitter: "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFTGOBMAOTQTVHXBMSYL5" },
          { event_hash: "0x99887766554433221100ffeeddccbbaa99887766554433221100ffeeddccbbaa", ledger_seq: 123457, submitter: "GACWTA5HYR7FUKVNYX4UGLYJ3K2XFNDQPMQJ62Q2H5F2YJ5I6T5N32Z4" },
        ];
        const scorecard = services.quality.evaluateQuality(assetId, dummyEvents);
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ scorecard }));
        return;
      }

      // Policy access evaluation & masking
      if (pathname === "/api/v1/governance/policies/enforce" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const role = json.role || "public";
            const allowed = services.policies.evaluateAccess(role, json.action || "read");
            const maskedData = services.policies.applyMasking(role, json.records || []);
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ allowed, data: maskedData }));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // Stewardship request creation
      if (pathname === "/api/v1/governance/stewardship/requests" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const request = services.stewardship.createRequest(json);
            res.writeHead(201, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ request }));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // Stewardship request review
      if (pathname === "/api/v1/governance/stewardship/review" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const reviewed = services.stewardship.reviewRequest(
              json.requestId,
              json.steward,
              json.approved,
              json.notes
            );
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ request: reviewed }));
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
