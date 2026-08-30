/**
 * Privacy-Preserving Analytics REST API Server
 *
 * Exposes endpoints for Differential Privacy queries, FL round management,
 * SMPC secret sharing, and Homomorphic Encryption operations.
 */

import http from "http";
import { URL } from "url";
import { DifferentialPrivacyEngine } from "./differential-privacy";
import { FederatedLearningCoordinator } from "./federated-learning";
import { SmpcEngine } from "./smpc";
import { HomomorphicEncryptionEngine } from "./homomorphic-encryption";

export interface PrivacyServices {
  dp: DifferentialPrivacyEngine;
  fl: FederatedLearningCoordinator;
  smpc: SmpcEngine;
  he: HomomorphicEncryptionEngine;
}

export function startPrivacyServer(
  services: PrivacyServices,
  port: number = 8088
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
      if (pathname === "/api/v1/privacy/health" && method === "GET") {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ status: "healthy", timestamp: Date.now() }));
        return;
      }

      // 1. DP Budget status
      if (pathname === "/api/v1/privacy/dp/budget" && method === "GET") {
        const budget = services.dp.getBudgetStatus();
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify(budget));
        return;
      }

      // 2. DP Query execution
      if (pathname === "/api/v1/privacy/dp/query" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const rawVal = json.rawValue ?? 100;
            const epsilon = json.epsilon ?? 0.5;
            const mech = json.mechanism || "laplace";

            const result = json.queryType === "sum"
              ? services.dp.querySum(rawVal, json.clipBound ?? 1000, epsilon, mech)
              : services.dp.queryCount(rawVal, epsilon, mech);

            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify(result));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // 3. FL Round creation
      if (pathname === "/api/v1/privacy/fl/rounds" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const round = services.fl.startRound({
              modelId: json.modelId || "audit-anomaly-detector",
              minParticipants: json.minParticipants || 2,
              initialWeights: json.initialWeights || [0.1, 0.2, 0.3],
            });
            res.writeHead(201, { "Content-Type": "application/json" });
            res.end(JSON.stringify(round));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // 4. SMPC Additive shares generation
      if (pathname === "/api/v1/privacy/smpc/split" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const secret = BigInt(json.secret || 12345);
            const parties = json.numParties || 3;
            const shares = services.smpc.generateAdditiveShares(secret, parties);
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ shares: shares.map((s) => s.toString()) }));
          } catch (e: any) {
            res.writeHead(400, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ error: e.message }));
          }
        });
        return;
      }

      // 5. Homomorphic addition
      if (pathname === "/api/v1/privacy/he/aggregate" && method === "POST") {
        let body = "";
        req.on("data", (chunk) => (body += chunk));
        req.on("end", () => {
          try {
            const json = JSON.parse(body || "{}");
            const keys = services.he.generateKeyPair();
            const c1 = services.he.encrypt(BigInt(json.value1 || 40), keys.publicKey);
            const c2 = services.he.encrypt(BigInt(json.value2 || 60), keys.publicKey);
            const cSum = services.he.addCiphertexts(c1, c2, keys.publicKey);
            const decrypted = services.he.decrypt(cSum, keys.publicKey, keys.privateKey);

            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(
              JSON.stringify({
                ciphertextSum: cSum.toString(),
                decryptedSum: decrypted.toString(),
              })
            );
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
