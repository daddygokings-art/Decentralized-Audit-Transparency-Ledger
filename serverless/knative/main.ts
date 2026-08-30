/**
 * Knative CloudEvents Processor (#522)
 *
 * Implements a CloudEvents compliant HTTP server running on Knative Serving.
 */

import http from "http";
import { ServerlessEventPipeline } from "../core/pipeline";
import { ContractEvent } from "../core/types";

const pipeline = new ServerlessEventPipeline(
  { targetFormat: "cloudevents" },
  [],
  [{ destination: "knative-eventing" }]
);

const PORT = parseInt(process.env.PORT || "8080", 10);

const server = http.createServer(async (req, res) => {
  if (req.method === "POST" && req.url === "/") {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", async () => {
      try {
        const payload = JSON.parse(body);
        const event: ContractEvent = payload.data || payload;
        const result = await pipeline.processEvent(event);

        res.writeHead(result.success ? 200 : 500, { "Content-Type": "application/cloudevents+json" });
        res.end(JSON.stringify(result));
      } catch (err: any) {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: err.message }));
      }
    });
    return;
  }

  if (req.url === "/healthz") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "healthy", runtime: "knative" }));
    return;
  }

  res.writeHead(404);
  res.end();
});

server.listen(PORT, () => {
  console.log(`Knative CloudEvents Processor listening on port ${PORT}`);
});
