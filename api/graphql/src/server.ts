import http from "http";
import { ApolloServer } from "@apollo/server";
import { expressMiddleware } from "@apollo/server/express4";
import express from "express";
import { WebSocketServer } from "ws";
import { useServer } from "graphql-ws/lib/use/ws";
import { makeExecutableSchema } from "@graphql-tools/schema";
import { typeDefs } from "./schema";
import { resolvers } from "./resolvers";

const PORT = parseInt(process.env.PORT ?? "4000", 10);
const API_KEY = process.env.API_KEY ?? "dev-key";

const schema = makeExecutableSchema({ typeDefs, resolvers });

async function main() {
  const app = express();
  app.use(express.json());

  const httpServer = http.createServer(app);

  // WebSocket server for subscriptions
  const wsServer = new WebSocketServer({ server: httpServer, path: "/graphql" });
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
      context: async ({ req }) => ({
        apiKey: req.headers["x-api-key"] ?? req.headers["authorization"]?.replace("Bearer ", ""),
      }),
    })
  );

  // Health check
  app.get("/health", (_req, res) => res.json({ status: "ok" }));

  await new Promise<void>((resolve) => httpServer.listen(PORT, resolve));
  console.log(`🚀 GraphQL ready at http://localhost:${PORT}/graphql`);
  console.log(`🔌 Subscriptions via ws://localhost:${PORT}/graphql`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
