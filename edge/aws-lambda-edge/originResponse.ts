/**
 * AWS Lambda@Edge Origin Response Handler (#521)
 *
 * Injects caching headers, compression indicators, and security headers at CloudFront edge.
 */

import { CloudFrontResponseEvent, CloudFrontResponseResult, Context } from "aws-lambda";

export const handler = async (
  event: CloudFrontResponseEvent,
  _context: Context
): Promise<CloudFrontResponseResult> => {
  const response = event.Records[0].cf.response;
  const request = event.Records[0].cf.request;

  // Add Edge Caching headers if not present
  if (!response.headers["cache-control"]) {
    if (request.uri.startsWith("/api/v1/events/query")) {
      response.headers["cache-control"] = [
        {
          key: "Cache-Control",
          value: "public, max-age=60, s-maxage=300, stale-while-revalidate=600",
        },
      ];
    } else {
      response.headers["cache-control"] = [
        { key: "Cache-Control", value: "no-store, no-cache, must-revalidate" },
      ];
    }
  }

  // Security headers at edge
  response.headers["strict-transport-security"] = [
    { key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains; preload" },
  ];
  response.headers["x-content-type-options"] = [
    { key: "X-Content-Type-Options", value: "nosniff" },
  ];
  response.headers["x-frame-options"] = [
    { key: "X-Frame-Options", value: "DENY" },
  ];
  response.headers["x-edge-provider"] = [
    { key: "X-Edge-Provider", value: "AWS-Lambda-At-Edge" },
  ];

  return response;
};
