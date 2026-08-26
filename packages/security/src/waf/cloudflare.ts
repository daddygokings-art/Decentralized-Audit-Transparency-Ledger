import type { Request } from "express";
import { createHmac, timingSafeEqual } from "crypto";

/**
 * Snapshot of Cloudflare's published IPv4/IPv6 ranges
 * (https://www.cloudflare.com/ips/). Operators should refresh this
 * periodically (e.g. a daily job calling `updateCloudflareRanges`) rather
 * than relying solely on the baked-in snapshot, since Cloudflare adds
 * ranges occasionally.
 */
let cloudflareIpv4Ranges = [
  "173.245.48.0/20",
  "103.21.244.0/22",
  "103.22.200.0/22",
  "103.31.4.0/22",
  "141.101.64.0/18",
  "108.162.192.0/18",
  "190.93.240.0/20",
  "188.114.96.0/20",
  "197.234.240.0/22",
  "198.41.128.0/17",
  "162.158.0.0/15",
  "104.16.0.0/13",
  "104.24.0.0/14",
  "172.64.0.0/13",
  "131.0.72.0/22",
];

export function updateCloudflareRanges(ranges: string[]): void {
  cloudflareIpv4Ranges = ranges;
}

export function getCloudflareRanges(): string[] {
  return [...cloudflareIpv4Ranges];
}

function ipToInt(ip: string): number | null {
  const parts = ip.split(".").map(Number);
  if (parts.length !== 4 || parts.some((p) => Number.isNaN(p) || p < 0 || p > 255)) return null;
  return ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0;
}

function cidrContains(cidr: string, ip: string): boolean {
  const [range, bitsStr] = cidr.split("/");
  const bits = parseInt(bitsStr, 10);
  const ipInt = ipToInt(ip);
  const rangeInt = ipToInt(range);
  if (ipInt === null || rangeInt === null) return false;
  const mask = bits === 0 ? 0 : (0xffffffff << (32 - bits)) >>> 0;
  return (ipInt & mask) === (rangeInt & mask);
}

/** Verifies the immediate TCP peer is a real Cloudflare edge node — this
 * must be checked against `req.socket.remoteAddress`, never a
 * client-suppliable header, or it's trivially spoofable. Only once this
 * passes should `CF-Connecting-IP` be trusted as the true client IP. */
export function isFromCloudflare(remoteAddress: string | undefined): boolean {
  if (!remoteAddress) return false;
  const addr = remoteAddress.replace("::ffff:", "");
  return cloudflareIpv4Ranges.some((cidr) => cidrContains(cidr, addr));
}

/** Resolves the true client IP, trusting `CF-Connecting-IP` only when the
 * request actually arrived from a Cloudflare edge IP. */
export function resolveClientIp(req: Request): string {
  const remoteAddress = req.socket.remoteAddress;
  if (isFromCloudflare(remoteAddress)) {
    const cfIp = req.headers["cf-connecting-ip"] as string | undefined;
    if (cfIp) return cfIp;
  }
  return remoteAddress ?? "unknown";
}

/** Optional defense-in-depth: verify an HMAC signature a Cloudflare Worker
 * (running in front of origin) attaches to forwarded requests, so origin
 * can reject anything that didn't pass through the expected Worker even if
 * an attacker learns/spoofs an edge IP via another Cloudflare zone. */
export function verifyWorkerSignature(req: Request, sharedSecret: string, headerName = "x-cf-worker-signature"): boolean {
  const provided = req.headers[headerName] as string | undefined;
  if (!provided) return false;
  const payload = `${req.method}:${req.originalUrl}:${(req.headers["cf-ray"] as string) ?? ""}`;
  const expected = createHmac("sha256", sharedSecret).update(payload).digest("hex");
  const a = Buffer.from(provided);
  const b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}

export interface CloudflareSignal {
  trusted: boolean;
  clientIp: string;
  rayId?: string;
  country?: string;
  threatScore?: number;
}

/** Cloudflare adds `cf-ipcountry` and, when the Bot Management /
 * Threat Score product is enabled, `cf-threat-score` (0-100, higher =
 * riskier) at the edge before proxying to origin. */
export function readCloudflareSignal(req: Request): CloudflareSignal {
  const trusted = isFromCloudflare(req.socket.remoteAddress);
  return {
    trusted,
    clientIp: resolveClientIp(req),
    rayId: req.headers["cf-ray"] as string | undefined,
    country: req.headers["cf-ipcountry"] as string | undefined,
    threatScore: trusted && req.headers["cf-threat-score"] ? Number(req.headers["cf-threat-score"]) : undefined,
  };
}
