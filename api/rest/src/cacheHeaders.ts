import type { Response } from "express";

export function setPublicCacheHeaders(res: Response): void {
  res.setHeader("Cache-Control", "public, max-age=300");
  res.removeHeader("Pragma");
  res.removeHeader("Expires");
}
