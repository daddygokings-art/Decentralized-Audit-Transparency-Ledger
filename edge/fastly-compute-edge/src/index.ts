/**
 * Fastly Compute@Edge Event Router & Query Cacher (#521)
 *
 * Implements edge caching with surrogate keys, backend health checks,
 * and low-latency request termination on Fastly's edge POP network.
 */

// Fastly Compute JavaScript runtime types
declare const fastly: any;

export async function handleRequest(event: any): Promise<Response> {
  const req = event.request;
  const url = new URL(req.url);

  // Health check endpoint
  if (url.pathname === "/healthz") {
    return new Response(JSON.stringify({ status: "healthy", platform: "fastly-compute-edge" }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }

  // Handle Event Query Caching
  if (url.pathname.startsWith("/api/v1/events/query") && req.method === "GET") {
    const backendResponse = await fetch(req, {
      backend: "audit_api",
      cacheOverride: new (fastly as any).CacheOverride("override", {
        ttl: 60,
        swr: 300,
        surrogateKey: "audit-events-feed",
      }),
    });

    const response = new Response(backendResponse.body, backendResponse);
    response.headers.set("X-Fastly-Edge", "true");
    response.headers.set("Surrogate-Key", "audit-events-feed");
    return response;
  }

  // Proxy ingestion requests directly to backend
  if (url.pathname.startsWith("/api/v1/events/ingest") && req.method === "POST") {
    const ingestResponse = await fetch(req, {
      backend: "audit_api",
      cacheOverride: new (fastly as any).CacheOverride("pass"),
    });
    return ingestResponse;
  }

  return new Response(JSON.stringify({ error: "Not Found" }), {
    status: 404,
    headers: { "Content-Type": "application/json" },
  });
}
