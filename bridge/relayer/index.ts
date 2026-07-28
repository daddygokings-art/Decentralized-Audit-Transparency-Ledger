/**
 * AuditLedger Cross-Chain Relayer
 *
 * Resolves merge conflicts between feat/issues-145-146-147-137 and master (#251).
 * Merges health check (#145), proof cache (#142), and deduplication features cleanly.
 *
 * Monitors AuditLedger events on Stellar, generates inclusion proofs,
 * and submits them to the EVM Verifier contract.
 */

import https from "https";
import http from "http";
import { createHash, createSign, createPrivateKey } from "crypto";
import { EventTransformer, StellarEvent, transformForEvm } from "./transform";
import { EventFilter, FilterConfig } from "./filter";
import { BatchProcessor, BatchProofEntry } from "./batch";
import { VerificationStore, Verifier as OnChainVerifier, startVerificationServer } from "./verification";
import { ErrorRecoveryManager, consoleNotifier } from "./recovery";

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

/**
 * Health status for the /healthz endpoint (#145).
 */
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
const HEALTH_PORT = parseInt(process.env.HEALTH_PORT ?? "8080", 10);

/** #145: Mark relayer degraded when no events seen for this many poll cycles. */
const UNHEALTHY_POLL_THRESHOLD = 5;

/** #142: Configurable proof LRU cache limits. */
const PROOF_CACHE_MAX_SIZE = parseInt(process.env.PROOF_CACHE_MAX_SIZE ?? "1000", 10);
const PROOF_CACHE_TTL_MS = parseInt(process.env.PROOF_CACHE_TTL_MS ?? "3600000", 10); // 1 hour

// Issue #255: event filter configuration (comma-separated lists, empty = allow all)
const FILTER_EVENT_TYPES_INCLUDE = (process.env.FILTER_EVENT_TYPES_INCLUDE ?? "").split(",").filter(Boolean);
const FILTER_SUBMITTERS_INCLUDE = (process.env.FILTER_SUBMITTERS_INCLUDE ?? "").split(",").filter(Boolean);
const FILTER_FROM_TIMESTAMP = process.env.FILTER_FROM_TIMESTAMP ? parseInt(process.env.FILTER_FROM_TIMESTAMP, 10) : undefined;
const FILTER_TO_TIMESTAMP = process.env.FILTER_TO_TIMESTAMP ? parseInt(process.env.FILTER_TO_TIMESTAMP, 10) : undefined;

// Issue #256: batch processing configuration
const BATCH_MAX_SIZE = parseInt(process.env.BATCH_MAX_SIZE ?? "25", 10);
const BATCH_MAX_WAIT_MS = parseInt(process.env.BATCH_MAX_WAIT_MS ?? "10000", 10);

// Issue #257: verification API configuration
const VERIFICATION_PORT = parseInt(process.env.VERIFICATION_PORT ?? "8081", 10);

// ── Health tracking state ─────────────────────────────────────────────────────

let relayerState = {
  startTime: Date.now(),
  lastProcessedIndex: 0,
  pollsWithoutEvents: 0,
};

/**
 * Returns the current health snapshot for the /healthz endpoint.
 * Merged from feat/issues-145-146-147-137 (#251).
 */
function getHealthStatus(): HealthStatus {
  const uptime = Math.floor((Date.now() - relayerState.startTime) / 1000);
  const status =
    relayerState.pollsWithoutEvents >= UNHEALTHY_POLL_THRESHOLD ? "degraded" : "ok";
  return {
    status,
    lastProcessedIndex: relayerState.lastProcessedIndex,
    uptime,
    pollsWithoutEvents: relayerState.pollsWithoutEvents,
  };
}

// ── LRU Proof Cache (#142) ────────────────────────────────────────────────────

interface CachedProof {
  proof: EventProof;
  timestamp: number;
}

/**
 * LRU proof cache with TTL eviction.
 * Merged and resolved from feat/issues-145-146-147-137 (#251).
 */
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

    if (Date.now() - cached.timestamp > this.ttlMs) {
      this.cache.delete(eventHash);
      return null;
    }

    // LRU: promote to end
    this.cache.delete(eventHash);
    this.cache.set(eventHash, cached);
    return cached.proof;
  }

  set(eventHash: string, proof: EventProof): void {
    if (this.cache.has(eventHash)) {
      this.cache.delete(eventHash);
    }

    this.cache.set(eventHash, { proof, timestamp: Date.now() });

    // Evict oldest entry when over capacity
    if (this.cache.size > this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      if (firstKey !== undefined) this.cache.delete(firstKey);
    }
  }

  clear(): void {
    this.cache.clear();
  }

  size(): number {
    return this.cache.size;
  }
}

// ── Event Deduplication (#251) ────────────────────────────────────────────────

/**
 * Tracks submitted event hashes to prevent duplicate EVM submissions.
 * Resolves the dedup conflict between feat/issues-145-146-147-137 and master (#251).
 *
 * Uses a bounded Set; once MAX_DEDUP_SIZE is reached, the oldest half is cleared
 * to avoid unbounded memory growth during long-running sessions.
 */
const MAX_DEDUP_SIZE = parseInt(process.env.MAX_DEDUP_SIZE ?? "10000", 10);
const submittedEvents: Set<string> = new Set();

/**
 * Returns true if this event hash has already been submitted to the EVM chain,
 * and registers it if not.
 */
function isDuplicate(eventHash: string): boolean {
  if (submittedEvents.has(eventHash)) return true;

  // Evict oldest half when the set grows too large
  if (submittedEvents.size >= MAX_DEDUP_SIZE) {
    const entries = Array.from(submittedEvents);
    const half = Math.floor(entries.length / 2);
    for (let i = 0; i < half; i++) {
      submittedEvents.delete(entries[i]);
    }
    console.warn(`[relayer] dedup set trimmed from ${entries.length} to ${submittedEvents.size} entries`);
  }

  submittedEvents.add(eventHash);
  return false;
}

// ── Rate limiters ─────────────────────────────────────────────────────────────

const stellarLimiter: TokenBucketRateLimiter = createStellarRateLimiter();
const evmLimiter: TokenBucketRateLimiter = createEvmRateLimiter();

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
        headers: {
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(payload),
        },
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

// ── Health check HTTP server (#145) ───────────────────────────────────────────

/**
 * Starts the /healthz HTTP endpoint.
 * Merged from feat/issues-145-146-147-137 — no duplicate function conflict (#251).
 */
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

function buildProof(event: AuditEvent, relayKey: Buffer): EventProof {
  const ledgerSeqBuf = Buffer.alloc(8);
  ledgerSeqBuf.writeBigUInt64BE(BigInt(event.ledger_seq ?? 0));

  const txHashBuf = Buffer.from(
    (event.tx_hash ?? "0".repeat(64)).replace(/^0x/, ""),
    "hex"
  );
  const eventHashBuf = Buffer.from(event.event_hash.replace(/^0x/, ""), "hex");

  const preimage = Buffer.concat([ledgerSeqBuf, txHashBuf, eventHashBuf]);

  const signer = createSign("SHA256");
  signer.update(preimage);
  const sig = signer.sign({
    key: createPrivateKey({ key: relayKey, format: "der", type: "pkcs8" }),
    dsaEncoding: "ieee-p1363",
  });

  return {
    ledgerSeq: BigInt(event.ledger_seq ?? 0),
    txHash: "0x" + txHashBuf.toString("hex"),
    eventIndex: event.index,
    eventHash: "0x" + eventHashBuf.toString("hex"),
    signature: "0x" + sig.toString("hex"),
  };
}

// ── EVM submission ────────────────────────────────────────────────────────────

async function submitToEvm(proof: EventProof, eventData: Buffer): Promise<string> {
  await evmLimiter.waitForToken();

  const proofHex = Buffer.concat([
    Buffer.alloc(8),
    Buffer.from(proof.txHash.slice(2), "hex"),
    Buffer.from(proof.eventHash.slice(2), "hex"),
    Buffer.from(proof.signature.slice(2), "hex"),
  ]).toString("hex");

  const callData = "0x" + "a1b2c3d4" + proofHex + eventData.toString("hex");

  const res = (await jsonRpc(EVM_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_call",
    params: [{ to: VERIFIER_ADDRESS, data: callData }, "latest"],
  })) as { result?: string };

  return res.result ?? "0x";
}

// Issue #257: read-only check against the Verifier contract's verified-events mapping
const verifyOnEvm: OnChainVerifier = async (eventHash: string): Promise<boolean> => {
  const hashHex = eventHash.replace(/^0x/, "").padStart(64, "0");
  const callData = "0x" + "9e5faafc" + hashHex; // isVerified(bytes32) selector placeholder

  const res = (await jsonRpc(EVM_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_call",
    params: [{ to: VERIFIER_ADDRESS, data: callData }, "latest"],
  })) as { result?: string };

  return res.result === "0x" + "0".repeat(63) + "1";
};

// ── Stellar polling ───────────────────────────────────────────────────────────

async function fetchLatestEvents(afterIndex: number): Promise<AuditEvent[]> {
  const res = (await jsonRpc(STELLAR_RPC, {
    jsonrpc: "2.0",
    id: 1,
    method: "getEvents",
    params: [
      {
        contractIds: [CONTRACT_ID],
        filters: [{ type: "contract" }],
        pagination: { after: String(afterIndex) },
      },
    ],
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

const MAX_EVM_RETRIES = 3;

async function run(): Promise<void> {
  relayerState.lastProcessedIndex = 0;
  const relayKey =
    RELAY_PRIVATE_KEY_HEX
      ? Buffer.from(RELAY_PRIVATE_KEY_HEX, "hex")
      : Buffer.alloc(32);
  const proofCache = new ProofCache(PROOF_CACHE_MAX_SIZE, PROOF_CACHE_TTL_MS);
  const transformer = new EventTransformer({
    chainId: "evm-mainnet",
    sourceChain: "stellar",
  });

  // Issue #255: selective bridging via event type / submitter / time range filters
  const filterConfig: FilterConfig = {
    eventType: FILTER_EVENT_TYPES_INCLUDE.length > 0 ? { include: FILTER_EVENT_TYPES_INCLUDE } : undefined,
    submitter: FILTER_SUBMITTERS_INCLUDE.length > 0 ? { include: FILTER_SUBMITTERS_INCLUDE } : undefined,
    timeRange:
      FILTER_FROM_TIMESTAMP !== undefined || FILTER_TO_TIMESTAMP !== undefined
        ? { fromTimestamp: FILTER_FROM_TIMESTAMP, toTimestamp: FILTER_TO_TIMESTAMP }
        : undefined,
  };
  const eventFilter = new EventFilter(filterConfig);

  // Issue #256: batch collection, proof generation, submission, and stats
  const batchProcessor = new BatchProcessor({ maxBatchSize: BATCH_MAX_SIZE, maxWaitMs: BATCH_MAX_WAIT_MS });

  // Issue #257: verification API backing store
  const verificationStore = new VerificationStore();

  // Issue #258: error classification, retry/backoff, dead letter queue, notifications
  const errorRecoveryManager = new ErrorRecoveryManager<AuditEvent>({
    strategies: { contract: { maxRetries: MAX_EVM_RETRIES, backoffMs: () => 2_000 } },
  });
  errorRecoveryManager.notifications.subscribe(consoleNotifier);

  console.log(`[relayer] starting — Stellar RPC: ${STELLAR_RPC}`);
  console.log(`[relayer] EVM target: ${VERIFIER_ADDRESS} @ ${EVM_RPC}`);
  console.log(`[relayer] proof cache: max ${PROOF_CACHE_MAX_SIZE} entries, TTL ${PROOF_CACHE_TTL_MS}ms`);
  console.log(`[relayer] batch config: max ${BATCH_MAX_SIZE} events, max wait ${BATCH_MAX_WAIT_MS}ms`);

  // Start health check server (#145)
  startHealthServer();

  // Issue #257: start the bridge event verification API
  startVerificationServer(VERIFICATION_PORT, verificationStore, verifyOnEvm);

  const buildProofCached = (evt: AuditEvent) => {
    let proof = proofCache.get(evt.event_hash);
    if (proof) {
      console.log(`[relayer] proof cache hit for event #${evt.index}`);
    } else {
      console.log(`[relayer] proof cache miss for event #${evt.index}, building proof`);
      proof = buildProof(evt, relayKey);
      proofCache.set(evt.event_hash, proof);
    }
    return proof;
  };

  const submitBatchEntry = async (entry: BatchProofEntry): Promise<string> => {
    const event = entry.event as AuditEvent;

    // Issue #259: Transform event for cross-chain compatibility
    const txResult = transformer.transformEvent(event);
    if (!txResult.success) {
      throw new Error(`transformation failed: ${txResult.errors.join(", ")}`);
    }

    const evmEvent = txResult.data!;
    const eventData = Buffer.from(JSON.stringify({
      index: evmEvent.index,
      eventType: evmEvent.eventType,
      category: evmEvent.category,
      submitter: evmEvent.submitter,
      metadata: evmEvent.metadata,
      chainId: evmEvent.chainId,
      sourceChain: evmEvent.sourceChain,
    }));

    try {
      const result = await submitToEvm(entry.proof!, eventData);
      console.log(`[relayer] submitted proof for event #${event.index} → EVM result: ${result}`);
      errorRecoveryManager.resolved(event.event_hash);
      // Issue #257: record the submission against the verification API
      await verificationStore.submit(event.event_hash, entry.proof!, verifyOnEvm);
      return result;
    } catch (err) {
      const decision = errorRecoveryManager.handle(event.event_hash, event, err);
      if (decision.shouldRetry) {
        console.warn(`[relayer] event #${event.index} failed, retrying (attempt ${decision.attempt}, delay ${decision.delayMs}ms)`);
      } else {
        console.error(`[relayer] event #${event.index} exhausted retries, moved to dead letter queue`);
      }
      throw err;
    }
  };

  while (true) {
    try {
      await stellarLimiter.waitForToken();
      const rawEvents = await fetchLatestEvents(relayerState.lastProcessedIndex);

      if (rawEvents.length === 0) {
        relayerState.pollsWithoutEvents++;
      } else {
        relayerState.pollsWithoutEvents = 0;
      }

      // Issue #255: apply event filters before any further processing
      const { passed: events, rejected } = eventFilter.apply(rawEvents);
      if (rejected.length > 0) {
        console.log(`[relayer] filtered out ${rejected.length} event(s): ${rejected.map((r) => r.reason).join("; ")}`);
      }

      for (const event of events) {
        console.log(
          `[relayer] processing event #${event.index} type=${event.event_type}`
        );

        // Deduplication check (#251) — skip events already submitted
        if (isDuplicate(event.event_hash)) {
          console.log(
            `[relayer] duplicate event #${event.index} (hash ${event.event_hash}) — skipping`
          );
          relayerState.lastProcessedIndex = Math.max(
            relayerState.lastProcessedIndex,
            event.index + 1
          );
          continue;
        }

        const txResult = transformer.transformEvent(event);
        if (!txResult.success) {
          console.error(
            `[relayer] transformation failed for event #${event.index}:`,
            txResult.errors
          );
          continue;
        }
        if (txResult.warnings.length > 0) {
          console.warn(
            `[relayer] transformation warnings for event #${event.index}:`,
            txResult.warnings
          );
        }

        // Issue #256: collect into the current batch instead of submitting immediately
        batchProcessor.collect(event);
        relayerState.lastProcessedIndex = Math.max(relayerState.lastProcessedIndex, event.index + 1);
      }

      if (batchProcessor.isReady()) {
        const batchResult = await batchProcessor.processBatch(buildProofCached, submitBatchEntry);
        console.log(
          `[relayer] batch #${batchResult.batchId} complete — submitted=${batchResult.submitted} failed=${batchResult.failed}`
        );
        console.log(`[relayer] batch stats: ${JSON.stringify(batchProcessor.getStatistics())}`);
        if (errorRecoveryManager.deadLetterQueue.size() > 0) {
          console.warn(`[relayer] dead letter queue size: ${errorRecoveryManager.deadLetterQueue.size()}`);
        }
      }
    } catch (err) {
      console.error("[relayer] poll error:", err);
    }

    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
}

if (require.main === module) {
  run().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

export {
  buildProof,
  fetchLatestEvents,
  EventProof,
  AuditEvent,
  HealthStatus,
  ProofCache,
  EventTransformer,
  verifyOnEvm,
};
