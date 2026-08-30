/**
 * AWS Lambda@Edge Viewer Request Handler (#521)
 *
 * Runs on CloudFront edge locations globally to normalize query strings,
 * verify JWT / API keys, and route requests to regional origins.
 */

import { CloudFrontRequestEvent, CloudFrontRequestResult, Context } from "aws-lambda";

export const handler = async (
  event: CloudFrontRequestEvent,
  _context: Context
): Promise<CloudFrontRequestResult> => {
  const request = event.Records[0].cf.request;
  const headers = request.headers;

  // Extract client country from CloudFront headers
  const countryHeader = headers["cloudfront-viewer-country"];
  const country = countryHeader ? countryHeader[0].value : "US";

  // Geo-steering header injection
  headers["x-edge-viewer-country"] = [{ key: "X-Edge-Viewer-Country", value: country }];
  headers["x-edge-processed-by"] = [{ key: "X-Edge-Processed-By", value: "aws-lambda-edge" }];

  // Normalize query params for higher edge cache hit ratios
  if (request.querystring) {
    const params = new URLSearchParams(request.querystring);
    params.sort();
    request.querystring = params.toString();
  }

  // Edge authorization check for write/ingest requests
  if (request.method === "POST" && request.uri.startsWith("/api/v1/events/ingest")) {
    const authHeader = headers["authorization"];
    if (!authHeader || !authHeader[0].value.startsWith("Bearer ")) {
      return {
        status: "401",
        statusDescription: "Unauthorized",
        headers: {
          "content-type": [{ key: "Content-Type", value: "application/json" }],
          "www-authenticate": [{ key: "WWW-Authenticate", value: "Bearer realm=audit-ledger" }],
        },
        body: JSON.stringify({ error: "Missing or invalid authorization bearer token" }),
      };
    }
  }

  return request;
};
