/**
 * Bridge Event Verification API (#257)
 *
 * Exposes an HTTP API for verifying bridged events: submitting a
 * verification request, retrieving the stored proof, checking the
 * current verification status, and reading the verification history.
 */

import http from "http";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface EventProof {
  ledgerSeq: bigint;
  txHash: string;
  eventIndex: number;
  eventHash: string;
  signature: string;
}

export type VerificationStatusValue = "pending" | "verified" | "failed";

export interface VerificationHistoryEntry {
  status: VerificationStatusValue;
  timestamp: number;
  detail?: string;
}

export interface VerificationRecord {
  eventHash: string;
  status: VerificationStatusValue;
  proof: EventProof | null;
  history: VerificationHistoryEntry[];
  createdAt: number;
  updatedAt: number;
}

export type Verifier = (eventHash: string, proof: EventProof) => Promise<boolean> | boolean;

// ── Verification store ────────────────────────────────────────────────────────

export class VerificationStore {
  private records: Map<string, VerificationRecord> = new Map();

  private touch(eventHash: string): VerificationRecord {
    let record = this.records.get(eventHash);
    if (!record) {
      const now = Date.now();
      record = {
        eventHash,
        status: "pending",
        proof: null,
        history: [{ status: "pending", timestamp: now }],
        createdAt: now,
        updatedAt: now,
      };
      this.records.set(eventHash, record);
    }
    return record;
  }

  /** Records a verification request and runs the supplied verifier against it. */
  async submit(eventHash: string, proof: EventProof, verify: Verifier): Promise<VerificationRecord> {
    const record = this.touch(eventHash);
    record.proof = proof;

    let ok: boolean;
    let detail: string | undefined;
    try {
      ok = await verify(eventHash, proof);
    } catch (err) {
      ok = false;
      detail = err instanceof Error ? err.message : String(err);
    }

    const now = Date.now();
    record.status = ok ? "verified" : "failed";
    record.updatedAt = now;
    record.history.push({ status: record.status, timestamp: now, detail });

    return { ...record };
  }

  getStatus(eventHash: string): VerificationStatusValue | null {
    return this.records.get(eventHash)?.status ?? null;
  }

  getProof(eventHash: string): EventProof | null {
    return this.records.get(eventHash)?.proof ?? null;
  }

  getHistory(eventHash: string): VerificationHistoryEntry[] {
    return this.records.get(eventHash)?.history ?? [];
  }

  getRecord(eventHash: string): VerificationRecord | null {
    const record = this.records.get(eventHash);
    return record ? { ...record } : null;
  }

  size(): number {
    return this.records.size;
  }

  clear(): void {
    this.records.clear();
  }
}

// ── HTTP API ──────────────────────────────────────────────────────────────────

function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks).toString()));
    req.on("error", reject);
  });
}

function sendJson(res: http.ServerResponse, statusCode: number, body: unknown): void {
  res.writeHead(statusCode, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body));
}

/**
 * Handles a single verification API request. Routes:
 *   POST /verify                        — submit a verification request
 *   GET  /verify/:eventHash/status       — current verification status
 *   GET  /verify/:eventHash/proof        — stored proof for the event
 *   GET  /verify/:eventHash/history      — verification history
 */
export async function handleVerificationRequest(
  req: http.IncomingMessage,
  res: http.ServerResponse,
  store: VerificationStore,
  verify: Verifier
): Promise<void> {
  const url = new URL(req.url ?? "/", "http://localhost");
  const parts = url.pathname.split("/").filter(Boolean);

  if (req.method === "POST" && parts[0] === "verify" && parts.length === 1) {
    try {
      const body = JSON.parse(await readBody(req)) as { eventHash?: string; proof?: EventProof };
      if (!body.eventHash || !body.proof) {
        sendJson(res, 400, { error: "eventHash and proof are required" });
        return;
      }
      const record = await store.submit(body.eventHash, body.proof, verify);
      sendJson(res, 200, record);
    } catch (err) {
      sendJson(res, 400, { error: err instanceof Error ? err.message : String(err) });
    }
    return;
  }

  if (req.method === "GET" && parts[0] === "verify" && parts.length === 3) {
    const [, eventHash, resource] = parts;

    if (resource === "status") {
      const status = store.getStatus(eventHash);
      if (status === null) {
        sendJson(res, 404, { error: "unknown eventHash" });
        return;
      }
      sendJson(res, 200, { eventHash, status });
      return;
    }

    if (resource === "proof") {
      const proof = store.getProof(eventHash);
      if (proof === null) {
        sendJson(res, 404, { error: "no proof stored for eventHash" });
        return;
      }
      sendJson(res, 200, { eventHash, proof });
      return;
    }

    if (resource === "history") {
      const history = store.getHistory(eventHash);
      sendJson(res, 200, { eventHash, history });
      return;
    }
  }

  sendJson(res, 404, { error: "Not Found" });
}

export function startVerificationServer(port: number, store: VerificationStore, verify: Verifier): http.Server {
  const server = http.createServer((req, res) => {
    handleVerificationRequest(req, res, store, verify).catch((err) => {
      sendJson(res, 500, { error: err instanceof Error ? err.message : String(err) });
    });
  });

  server.listen(port, () => {
    console.log(`[verification] API listening on port ${port}`);
  });

  return server;
}
