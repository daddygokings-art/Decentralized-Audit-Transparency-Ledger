import http from "http";
import { ApolloServer } from "@apollo/server";
import { expressMiddleware } from "@apollo/server/express4";
import express from "express";
import { WebSocketServer, WebSocket } from "ws";
import { useServer } from "graphql-ws/dist/use/ws";
import { makeExecutableSchema } from "@graphql-tools/schema";
import { GraphQLError } from "graphql";
import { typeDefs } from "./schema";
import { resolvers } from "./resolvers";
import { validateKey } from "../../rest/src/keys";
import type { Role } from "../../rest/src/keys";

const PORT = parseInt(process.env.PORT ?? "4000", 10);
const API_KEY = process.env.API_KEY ?? "dev-key";
const MAX_WS_CONNECTIONS = parseInt(process.env.MAX_WS_CONNECTIONS ?? "100", 10);

const schema = makeExecutableSchema({ typeDefs, resolvers });

const activeConnections = new Set<WebSocket>();

async function main() {
  const app = express();
  app.use(express.json({ limit: "1mb" }));
  app.use(graphqlValidation);

  const httpServer = http.createServer(app);

  const wsServer = new WebSocketServer({ server: httpServer, path: "/graphql" });

  wsServer.on("connection", (ws) => {
    if (activeConnections.size >= MAX_WS_CONNECTIONS) {
      ws.close(1013, "Too many connections");
      return;
    }
    activeConnections.add(ws);
    ws.on("close", () => activeConnections.delete(ws));
  });

  const cleanup = useServer({ schema }, wsServer);

  const apollo = new ApolloServer({
    schema,
    plugins: [
      {
        async serverWillStart() {
          return {
            async drainServer() {
              await cleanup.dispose();
            },
          };
        },
      },
    ],
  });

  await apollo.start();

  app.use(
    "/graphql",
    expressMiddleware(apollo, {
      context: async ({ req }) => {
        const apiKey = (req.headers["x-api-key"] ?? req.headers["authorization"]?.replace("Bearer ", "")) as string | undefined;
        let role: Role | undefined;
        if (apiKey) {
          const record = validateKey(apiKey);
          if (record) {
            role = record.role;
          }
        }
        return { apiKey, role };
      },
    })
  );

  // Health check endpoints (#268)
  const graphqlStartTime = Date.now();

  app.get("/healthz", (_req, res) => {
    res.json({
      status: "ok",
      service: "graphql",
      uptime: Math.floor((Date.now() - graphqlStartTime) / 1000),
      timestamp: new Date().toISOString(),
    });
  });

  app.get("/readyz", (_req, res) => {
    const checks: Record<string, { status: string; latencyMs?: number }> = {};

    const schemaCheckStart = Date.now();
    try {
      const op = { kind: "query" as const, name: { kind: "Name" as const, value: "__typename" } };
      checks.schema = { status: "ok", latencyMs: Date.now() - schemaCheckStart };
    } catch {
      checks.schema = { status: "failed", latencyMs: Date.now() - schemaCheckStart };
    }

    const allHealthy = Object.values(checks).every((c) => c.status === "ok");
    res.status(allHealthy ? 200 : 503).json({
      status: allHealthy ? "ready" : "not_ready",
      service: "graphql",
      checks,
      timestamp: new Date().toISOString(),
    });
  });

  app.get("/health", (_req, res) => {
    res.json({
      status: "ok",
      service: "graphql",
      uptime: Math.floor((Date.now() - graphqlStartTime) / 1000),
      timestamp: new Date().toISOString(),
    });
  });

  app.get("/metrics", (_req, res) => {
    const lines = [
      "# HELP graphql_uptime_seconds GraphQL service uptime",
      "# TYPE graphql_uptime_seconds gauge",
      `graphql_uptime_seconds ${Math.floor((Date.now() - graphqlStartTime) / 1000)}`,
    ];
    res.setHeader("Content-Type", "text/plain; version=0.0.4");
    res.send(lines.join("\n"));
  });

  await new Promise<void>((resolve) => httpServer.listen(PORT, resolve));
  console.log(`GraphQL ready at http://localhost:${PORT}/graphql`);
  console.log(`Subscriptions via ws://localhost:${PORT}/graphql`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
