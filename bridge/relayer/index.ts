/**
 * AuditLedger Cross-Chain Relayer (#79)
 *
 * Monitors AuditLedger events on Stellar, generates inclusion proofs,
 * and submits them to the EVM Verifier contract.
 */

import https from "https";
import http from "http";
import { createHash, createSign, createPrivateKey } from "crypto";

// ── Types ─────────────────────────────────────────────────────────────────────

interface AuditEvent {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  event_hash: string;
  ledger_seq: number;
  tx_hash: string;
}

interface EventProof {
  ledgerSeq: bigint;
  txHash: string;        // 0x-prefixed hex bytes32
  eventIndex: number;
  eventHash: string;     // 0x-prefixed hex bytes32
  signature: string;     // 0x-prefixed 65-byte ECDSA hex
}

interface HealthStatus {
  status: "ok" | "degraded";
  lastProcessedIndex: number;
  uptime: number;
  pollsWithoutEvents: number;
}

// ── Config ────────────────────────────────────────────────────────────────────

const STELLAR_RPC = process.env.STELLAR_RPC ?? "https://soroban-testnet.stellar.org";
const CONTRACT_ID = process.env.CONTRACT_ID ?? "";
const EVM_RPC = process.env.EVM_RPC ?? "http://localhost:8545";
const VERIFIER_ADDRESS = process.env.VERIFIER_ADDRESS ?? "";
const RELAY_PRIVATE_KEY_HEX = process.env.RELAY_PRIVATE_KEY ?? "";
const POLL_INTERVAL_MS = parseInt(process.env.POLL_INTERVAL ?? "5000", 10);
<<<<<<< feat/issues-145-146-147-137
const HEALTH_PORT = parseInt(process.env.HEALTH_PORT ?? "8080", 10);
const UNHEALTHY_POLL_THRESHOLD = 5; // Issue #145: unhealthy if no events in 5 poll cycles

// ── Health tracking state ─────────────────────────────────────────────────────

let relayerState = {
  startTime: Date.now(),
  lastProcessedIndex: 0,
  pollsWithoutEvents: 0,
};

function getHealthStatus(): HealthStatus {
  const uptime = Math.floor((Date.now() - relayerState.startTime) / 1000);
  const status = relayerState.pollsWithoutEvents >= UNHEALTHY_POLL_THRESHOLD ? "degraded" : "ok";
  return {
    status,
    lastProcessedIndex: relayerState.lastProcessedIndex,
    uptime,
    pollsWithoutEvents: relayerState.pollsWithoutEvents,
  };
=======
const PROOF_CACHE_MAX_SIZE = parseInt(process.env.PROOF_CACHE_MAX_SIZE ?? "1000", 10);
const PROOF_CACHE_TTL_MS = parseInt(process.env.PROOF_CACHE_TTL_MS ?? "3600000", 10); // 1 hour default

// ── LRU Proof Cache (Issue #142) ──────────────────────────────────────────────

interface CachedProof {
  proof: EventProof;
  timestamp: number;
}

class ProofCache {
  private cache: Map<string, CachedProof> = new Map();
  private maxSize: number;
  private ttlMs: number;

  constructor(maxSize: number, ttlMs: number) {
    this.maxSize = maxSize;
    this.ttlMs = ttlMs;
  }

  get(eventHash: string): EventProof | null {
    const cached = this.cache.get(eventHash);
    if (!cached) return null;

    // Check TTL
    if (Date.now() - cached.timestamp > this.ttlMs) {
      this.cache.delete(eventHash);
      return null;
    }

    // Move to end (most recently used)
    this.cache.delete(eventHash);
    this.cache.set(eventHash, cached);
    return cached.proof;
  }

  set(eventHash: string, proof: EventProof): void {
    // Remove if exists (to update LRU order)
    if (this.cache.has(eventHash)) {
      this.cache.delete(eventHash);
    }

    // Add to end (most recently used)
    this.cache.set(eventHash, { proof, timestamp: Date.now() });

    // Evict least recently used if over capacity
    if (this.cache.size > this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
  }

  clear(): void {
    this.cache.clear();
  }

  size(): number {
    return this.cache.size;
  }
>>>>>>> master
}

// ── HTTP helper ───────────────────────────────────────────────────────────────

function jsonRpc(url: string, body: object): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify(body);
    const parsed = new URL(url);
    const lib = parsed.protocol === "https:" ? https : http;
    const req = lib.request(
      {
        hostname: parsed.hostname,
        port: parsed.port || (parsed.protocol === "https:" ? 443 : 80),
        path: parsed.pathname,
        method: "POST",
        headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(payload) },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => {
          try {
            resolve(JSON.parse(Buffer.concat(chunks).toString()));
          } catch (e) {
            reject(e);
          }
        });
      }
    );
    req.on("error", reject);
    req.write(payload);
    req.end();
  });
}

// ── Health check HTTP server ──────────────────────────────────────────────────

function startHealthServer(): void {
  const server = http.createServer((req, res) => {
    if (req.url === "/healthz" && req.method === "GET") {
      const health = getHealthStatus();
      const statusCode = health.status === "ok" ? 200 : 503;
      res.writeHead(statusCode, { "Content-Type": "application/json" });
      res.end(JSON.stringify(health));
    } else {
      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "Not Found" }));
    }
  });
  
  server.listen(HEALTH_PORT, () => {
    console.log(`[relayer] health check server listening on port ${HEALTH_PORT}`);
  });
}

// ── Proof builder ─────────────────────────────────────────────────────────────

/**
 * Builds and signs an EventProof.
 * Signs: keccak256(abi.encodePacked(ledgerSeq, txHash, eventHash))
 */
function buildProof(event: AuditEvent, relayKey: Buffer): EventProof {
  const ledgerSeqBuf = Buffer.alloc(8);
  ledgerSeqBuf.writeBigUInt64BE(BigInt(event.ledger_seq ?? 0));

  const txHashBuf = Buffer.from((event.tx_hash ?? "0".repeat(64)).replace(/^0x/, ""), "hex");
  const eventHashBuf = Buffer.from(event.event_hash.replace(/^0x/, ""), "hex");

  // keccak256-like: use sha256 as a stand-in (replace with ethers.js keccak256 in production)
  const preimage = Buffer.concat([ledgerSeqBuf, txHashBuf, eventHashBuf]);
  const msgHash = createHash("sha256").update(preimage).digest();

  // ECDSA sign (secp256k1 via node crypto – requires openssl ecparam key)
  // In production, use ethers.js Wallet.signMessage for EVM compatibility
  const signer = createSign("SHA256");
  signer.update(preimage);
  const sig = signer.sign({ key: createPrivateKey({ key: relayKey, format: "der", type: "pkcs8" }), dsaEncoding: "ieee-p1363" });

  return {
    ledgerSeq: BigInt(event.ledger_seq ?? 0),
    txHash: "0x" + txHashBuf.toString("hex"),
    eventIndex: event.index,
    eventHash: "0x" + eventHashBuf.toString("hex"),
    signature: "0x" + sig.toString("hex"),
  };
}

// ── EVM submission ────────────────────────────────────────────────────────────

/**
 * Submits a proof to the EVM Verifier via eth_sendRawTransaction.
 * In production, use ethers.js to sign and submit the transaction.
 */
async function submitToEvm(proof: EventProof, eventData: Buffer): Promise<string> {
  // ABI-encode the call to verifyEvent(bytes,bytes)
  // This is a simplified placeholder — use ethers.js Interface.encodeFunctionData in production
  const proofHex = Buffer.concat([
    Buffer.alloc(8), // ledgerSeq placeholder
    Buffer.from(proof.txHash.slice(2), "hex"),
    Buffer.from(proof.eventHash.slice(2), "hex"),
    Buffer.from(proof.signature.slice(2), "hex"),
  ]).toString("hex");

  const callData = "0x" + "a1b2c3d4" + proofHex + eventData.toString("hex"); // selector placeholder

  const res = (await jsonRpc(EVM_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_call",
    params: [{ to: VERIFIER_ADDRESS, data: callData }, "latest"],
  })) as { result?: string };

  return res.result ?? "0x";
}

// ── Stellar polling ───────────────────────────────────────────────────────────

async function fetchLatestEvents(afterIndex: number): Promise<AuditEvent[]> {
  const res = (await jsonRpc(STELLAR_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "getEvents",
    params: [{ contractIds: [CONTRACT_ID], filters: [{ type: "contract" }], pagination: { after: String(afterIndex) } }],
  })) as { result?: { events?: unknown[] } };

  if (!res.result?.events) return [];

  return (res.result.events as unknown[]).map((e: unknown) => {
    const ev = e as Record<string, unknown>;
    return {
      index: Number(ev["id"] ?? 0),
      timestamp: Number(ev["ledgerClosedAt"] ?? 0),
      event_type: String(ev["topic"] ?? ""),
      submitter: String(ev["contractId"] ?? ""),
      metadata: JSON.stringify(ev["value"] ?? {}),
      event_hash: createHash("sha256").update(JSON.stringify(ev)).digest("hex"),
      ledger_seq: Number(ev["ledger"] ?? 0),
      tx_hash: String(ev["txHash"] ?? "0".repeat(64)),
    } as AuditEvent;
  });
}

// ── Main loop ─────────────────────────────────────────────────────────────────

async function run(): Promise<void> {
  relayerState.lastProcessedIndex = 0;
  const relayKey = RELAY_PRIVATE_KEY_HEX ? Buffer.from(RELAY_PRIVATE_KEY_HEX, "hex") : Buffer.alloc(32);
  const proofCache = new ProofCache(PROOF_CACHE_MAX_SIZE, PROOF_CACHE_TTL_MS);

  console.log(`[relayer] starting — Stellar RPC: ${STELLAR_RPC}`);
  console.log(`[relayer] EVM target: ${VERIFIER_ADDRESS} @ ${EVM_RPC}`);
<<<<<<< feat/issues-145-146-147-137
  
  // Start health check server (Issue #145)
  startHealthServer();
=======
  console.log(`[relayer] proof cache: max ${PROOF_CACHE_MAX_SIZE} entries, TTL ${PROOF_CACHE_TTL_MS}ms`);
>>>>>>> master

  while (true) {
    try {
      const events = await fetchLatestEvents(relayerState.lastProcessedIndex);

      if (events.length === 0) {
        // No events in this poll cycle
        relayerState.pollsWithoutEvents++;
      } else {
        // Reset counter when events are found
        relayerState.pollsWithoutEvents = 0;
      }

      for (const event of events) {
        console.log(`[relayer] processing event #${event.index} type=${event.event_type}`);
        const proof = buildProof(event, relayKey);
        const eventData = Buffer.from(JSON.stringify({ index: event.index, event_type: event.event_type, submitter: event.submitter, metadata: event.metadata }));
        const result = await submitToEvm(proof, eventData);
        console.log(`[relayer] submitted proof for event #${event.index} → EVM result: ${result}`);
        relayerState.lastProcessedIndex = Math.max(relayerState.lastProcessedIndex, event.index + 1);
      }
    } catch (err) {
      console.error("[relayer] poll error:", err);
    }

    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
}

if (require.main === module) {
  run().catch((err) => { console.error(err); process.exit(1); });
}

export { buildProof, fetchLatestEvents, EventProof, AuditEvent, HealthStatus };

// ── HTTP helper ───────────────────────────────────────────────────────────────

function jsonRpc(url: string, body: object): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify(body);
    const parsed = new URL(url);
    const lib = parsed.protocol === "https:" ? https : http;
    const req = lib.request(
      {
        hostname: parsed.hostname,
        port: parsed.port || (parsed.protocol === "https:" ? 443 : 80),
        path: parsed.pathname,
        method: "POST",
        headers: { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(payload) },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => {
          try {
            resolve(JSON.parse(Buffer.concat(chunks).toString()));
          } catch (e) {
            reject(e);
          }
        });
      }
    );
    req.on("error", reject);
    req.write(payload);
    req.end();
  });
}

// ── Proof builder ─────────────────────────────────────────────────────────────

/**
 * Builds and signs an EventProof.
 * Signs: keccak256(abi.encodePacked(ledgerSeq, txHash, eventHash))
 */
function buildProof(event: AuditEvent, relayKey: Buffer): EventProof {
  const ledgerSeqBuf = Buffer.alloc(8);
  ledgerSeqBuf.writeBigUInt64BE(BigInt(event.ledger_seq ?? 0));

  const txHashBuf = Buffer.from((event.tx_hash ?? "0".repeat(64)).replace(/^0x/, ""), "hex");
  const eventHashBuf = Buffer.from(event.event_hash.replace(/^0x/, ""), "hex");

  // keccak256-like: use sha256 as a stand-in (replace with ethers.js keccak256 in production)
  const preimage = Buffer.concat([ledgerSeqBuf, txHashBuf, eventHashBuf]);
  const msgHash = createHash("sha256").update(preimage).digest();

  // ECDSA sign (secp256k1 via node crypto – requires openssl ecparam key)
  // In production, use ethers.js Wallet.signMessage for EVM compatibility
  const signer = createSign("SHA256");
  signer.update(preimage);
  const sig = signer.sign({ key: createPrivateKey({ key: relayKey, format: "der", type: "pkcs8" }), dsaEncoding: "ieee-p1363" });

  return {
    ledgerSeq: BigInt(event.ledger_seq ?? 0),
    txHash: "0x" + txHashBuf.toString("hex"),
    eventIndex: event.index,
    eventHash: "0x" + eventHashBuf.toString("hex"),
    signature: "0x" + sig.toString("hex"),
  };
}

// ── EVM submission ────────────────────────────────────────────────────────────

/**
 * Submits a proof to the EVM Verifier via eth_sendRawTransaction.
 * In production, use ethers.js to sign and submit the transaction.
 */
async function submitToEvm(proof: EventProof, eventData: Buffer): Promise<string> {
  // ABI-encode the call to verifyEvent(bytes,bytes)
  // This is a simplified placeholder — use ethers.js Interface.encodeFunctionData in production
  const proofHex = Buffer.concat([
    Buffer.alloc(8), // ledgerSeq placeholder
    Buffer.from(proof.txHash.slice(2), "hex"),
    Buffer.from(proof.eventHash.slice(2), "hex"),
    Buffer.from(proof.signature.slice(2), "hex"),
  ]).toString("hex");

  const callData = "0x" + "a1b2c3d4" + proofHex + eventData.toString("hex"); // selector placeholder

  const res = (await jsonRpc(EVM_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_call",
    params: [{ to: VERIFIER_ADDRESS, data: callData }, "latest"],
  })) as { result?: string };

  return res.result ?? "0x";
}

// ── Stellar polling ───────────────────────────────────────────────────────────

async function fetchLatestEvents(afterIndex: number): Promise<AuditEvent[]> {
  const res = (await jsonRpc(STELLAR_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "getEvents",
    params: [{ contractIds: [CONTRACT_ID], filters: [{ type: "contract" }], pagination: { after: String(afterIndex) } }],
  })) as { result?: { events?: unknown[] } };

  if (!res.result?.events) return [];

  return (res.result.events as unknown[]).map((e: unknown) => {
    const ev = e as Record<string, unknown>;
    return {
      index: Number(ev["id"] ?? 0),
      timestamp: Number(ev["ledgerClosedAt"] ?? 0),
      event_type: String(ev["topic"] ?? ""),
      submitter: String(ev["contractId"] ?? ""),
      metadata: JSON.stringify(ev["value"] ?? {}),
      event_hash: createHash("sha256").update(JSON.stringify(ev)).digest("hex"),
      ledger_seq: Number(ev["ledger"] ?? 0),
      tx_hash: String(ev["txHash"] ?? "0".repeat(64)),
    } as AuditEvent;
  });
}

// ── Main loop ─────────────────────────────────────────────────────────────────

async function run(): Promise<void> {
  relayerState.lastProcessedIndex = 0;
  const relayKey = RELAY_PRIVATE_KEY_HEX ? Buffer.from(RELAY_PRIVATE_KEY_HEX, "hex") : Buffer.alloc(32);

  console.log(`[relayer] starting — Stellar RPC: ${STELLAR_RPC}`);
  console.log(`[relayer] EVM target: ${VERIFIER_ADDRESS} @ ${EVM_RPC}`);
  
  // Start health check server (Issue #145)
  startHealthServer();

  while (true) {
    try {
      const events = await fetchLatestEvents(relayerState.lastProcessedIndex);

      if (events.length === 0) {
        // No events in this poll cycle
        relayerState.pollsWithoutEvents++;
      } else {
        // Reset counter when events are found
        relayerState.pollsWithoutEvents = 0;
      }

      for (const event of events) {
        console.log(`[relayer] processing event #${event.index} type=${event.event_type}`);
        
        // Issue #142: Check proof cache before rebuilding
        let proof = proofCache.get(event.event_hash);
        if (proof) {
          console.log(`[relayer] proof cache hit for event #${event.index}`);
        } else {
          console.log(`[relayer] proof cache miss for event #${event.index}, building proof`);
          proof = buildProof(event, relayKey);
          proofCache.set(event.event_hash, proof);
        }
        
        const eventData = Buffer.from(JSON.stringify({ index: event.index, event_type: event.event_type, submitter: event.submitter, metadata: event.metadata }));
        const result = await submitToEvm(proof, eventData);
        console.log(`[relayer] submitted proof for event #${event.index} → EVM result: ${result}`);
        relayerState.lastProcessedIndex = Math.max(relayerState.lastProcessedIndex, event.index + 1);
      }
    } catch (err) {
      console.error("[relayer] poll error:", err);
    }

    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
}

if (require.main === module) {
  run().catch((err) => { console.error(err); process.exit(1); });
}

<<<<<<< feat/issues-145-146-147-137
export { buildProof, fetchLatestEvents, EventProof, AuditEvent, HealthStatus };
=======
export { buildProof, fetchLatestEvents, EventProof, AuditEvent, ProofCache };
>>>>>>> master
