/**
 * AuditLedger Prometheus Metrics Exporter
 *
 * Exposes contract metrics on :8000/metrics by polling the Soroban RPC.
 *
 * Environment variables:
 *   CONTRACT_ID   – Soroban contract address (required)
 *   RPC_URL       – Soroban RPC endpoint (default: https://soroban-testnet.stellar.org)
 *   NETWORK       – "testnet" | "mainnet" (default: testnet)
 *   SCRAPE_INTERVAL_MS – polling interval in ms (default: 15000)
 *   PORT          – HTTP port (default: 8000)
 */
"use strict";

const http = require("http");
const { SorobanRpc, Contract, Networks, xdr } = require("@stellar/stellar-sdk");
const client = require("prom-client");

const CONTRACT_ID = process.env.CONTRACT_ID || "";
const RPC_URL =
  process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const NETWORK = process.env.NETWORK || "testnet";
const SCRAPE_INTERVAL_MS = parseInt(process.env.SCRAPE_INTERVAL_MS || "15000", 10);
const PORT = parseInt(process.env.PORT || "8000", 10);

if (!CONTRACT_ID) {
  console.error("ERROR: CONTRACT_ID environment variable is required.");
  process.exit(1);
}

// ── Prometheus registry & metrics ──────────────────────────────────────────

const registry = new client.Registry();
client.collectDefaultMetrics({ register: registry });

const totalEvents = new client.Gauge({
  name: "audit_ledger_total_events",
  help: "Total number of events logged in the AuditLedger contract",
  registers: [registry],
});

const globalMaxLogs = new client.Gauge({
  name: "audit_ledger_global_max_logs",
  help: "Global maximum log cap configured on the contract",
  registers: [registry],
});

const storageUsagePct = new client.Gauge({
  name: "audit_ledger_storage_usage_percent",
  help: "Estimated storage usage as percentage of global_max_logs",
  registers: [registry],
});

const eventsByType = new client.Gauge({
  name: "audit_ledger_events_by_type",
  help: "Number of events per event type",
  labelNames: ["event_type"],
  registers: [registry],
});

const errorCount = new client.Counter({
  name: "audit_ledger_error_count",
  help: "Total number of failed contract invocations observed",
  registers: [registry],
});

const avgGasCost = new client.Gauge({
  name: "audit_ledger_avg_gas_cost",
  help: "Average fee (stroops) per log_event invocation (sampled)",
  registers: [registry],
});

// Issue 1: paused gauge with contract_id label
const pausedGauge = new client.Gauge({
  name: "audit_ledger_paused",
  help: "1 if the AuditLedger contract is paused, 0 if active",
  labelNames: ["contract_id"],
  registers: [registry],
});

// Issue 1: paused_since gauge (unix timestamp, 0 when not paused)
const pausedSinceGauge = new client.Gauge({
  name: "audit_ledger_paused_since",
  help: "Unix timestamp when the contract was paused (0 if not paused)",
  labelNames: ["contract_id"],
  registers: [registry],
});

// Issue 3: scrape error gauge — set to 1 after 10 consecutive failures
const scrapeErrorGauge = new client.Gauge({
  name: "audit_ledger_scrape_error",
  help: "1 if the exporter has hit 10+ consecutive RPC failures",
  labelNames: ["contract_id"],
  registers: [registry],
});

// ── Soroban RPC helpers ─────────────────────────────────────────────────────

const networkPassphrase =
  NETWORK === "mainnet" ? Networks.PUBLIC : Networks.TESTNET;

const server = new SorobanRpc.Server(RPC_URL, { allowHttp: RPC_URL.startsWith("http://") });

/**
 * Call a read-only contract function and return the raw ScVal result.
 * @param {string} method
 * @param {xdr.ScVal[]} args
 */
async function callContract(method, args = []) {
  const contract = new Contract(CONTRACT_ID);
  const op = contract.call(method, ...args);
  const tx = new (require("@stellar/stellar-sdk").TransactionBuilder)(
    // Simulate with a dummy source — read-only, no auth needed
    new (require("@stellar/stellar-sdk").Account)(
      "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
      "0"
    ),
    {
      fee: "100",
      networkPassphrase,
    }
  )
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Simulation error: ${sim.error}`);
  }
  return sim.result?.retval;
}

/**
 * Decode an ScVal u32 to a JS number.
 */
function scValToU32(val) {
  return val.u32();
}

// ── Retry state (Issue 3) ───────────────────────────────────────────────────

let consecutiveFailures = 0;
const MAX_FAILURES_BEFORE_ERROR_GAUGE = 10;
const BACKOFF_BASE_MS = 1000;
const BACKOFF_MAX_MS = 60000;

/**
 * Sleep for `ms` milliseconds.
 */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ── Scrape loop ─────────────────────────────────────────────────────────────

async function scrape() {
  try {
    // total_events
    const totalVal = await callContract("total_events");
    const total = scValToU32(totalVal);
    totalEvents.set(total);

    // global_max_logs
    try {
      const maxVal = await callContract("get_global_max_logs");
      const max = scValToU32(maxVal);
      globalMaxLogs.set(max);
      storageUsagePct.set(max > 0 ? (total / max) * 100 : 0);
    } catch {
      // Contract doesn't expose this endpoint; skip global max metrics
    }

    // events_by_type
    const knownTypes = (process.env.EVENT_TYPES || "")
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    for (const type of knownTypes) {
      try {
        const countVal = await callContract("event_count", [
          xdr.ScVal.scvSymbol(type),
        ]);
        eventsByType.set({ event_type: type }, scValToU32(countVal));
      } catch {
        // type not yet logged; ignore
      }
    }

    // Issue 1: paused status
    try {
      const pausedVal = await callContract("is_paused");
      const isPaused = pausedVal?.value() === true || pausedVal?.switch()?.name === "scvBool" && pausedVal.b() === true;
      pausedGauge.set({ contract_id: CONTRACT_ID }, isPaused ? 1 : 0);

      if (isPaused) {
        try {
          const sinceVal = await callContract("paused_since");
          // u64 comes back as a BigInt-like value
          const since = Number(sinceVal?.u64() ?? sinceVal?.i64() ?? 0);
          pausedSinceGauge.set({ contract_id: CONTRACT_ID }, since);
        } catch {
          pausedSinceGauge.set({ contract_id: CONTRACT_ID }, 0);
        }
      } else {
        pausedSinceGauge.set({ contract_id: CONTRACT_ID }, 0);
      }
    } catch {
      // is_paused not available; default to 0
      pausedGauge.set({ contract_id: CONTRACT_ID }, 0);
      pausedSinceGauge.set({ contract_id: CONTRACT_ID }, 0);
    }

    // Issue 3: reset failure counter on success
    consecutiveFailures = 0;
    scrapeErrorGauge.set({ contract_id: CONTRACT_ID }, 0);
  } catch (err) {
    console.error("Scrape error:", err.message);
    errorCount.inc();

    consecutiveFailures += 1;
    if (consecutiveFailures >= MAX_FAILURES_BEFORE_ERROR_GAUGE) {
      scrapeErrorGauge.set({ contract_id: CONTRACT_ID }, 1);
      console.error(`${consecutiveFailures} consecutive failures; audit_ledger_scrape_error set to 1`);
    }

    // Issue 3: exponential backoff retry (1s, 2s, 4s… capped at 60s)
    const backoffMs = Math.min(
      BACKOFF_BASE_MS * Math.pow(2, consecutiveFailures - 1),
      BACKOFF_MAX_MS
    );
    console.error(`Retrying in ${backoffMs}ms…`);
    await sleep(backoffMs);
    // Recurse once after backoff (single retry attempt per interval cycle)
    return scrape();
  }
}

// ── HTTP server ─────────────────────────────────────────────────────────────

const httpServer = http.createServer(async (req, res) => {
  if (req.url === "/metrics" && req.method === "GET") {
    try {
      res.setHeader("Content-Type", registry.contentType);
      res.end(await registry.metrics());
    } catch (err) {
      res.writeHead(500);
      res.end(err.message);
    }
  } else if (req.url === "/health" && req.method === "GET") {
    res.writeHead(200);
    res.end("ok");
  } else {
    res.writeHead(404);
    res.end("not found");
  }
});

httpServer.listen(PORT, () => {
  console.log(`Metrics exporter listening on :${PORT}/metrics`);
  console.log(`Contract: ${CONTRACT_ID}`);
  console.log(`RPC:      ${RPC_URL}`);
});

// Initial scrape then poll
scrape();
setInterval(scrape, SCRAPE_INTERVAL_MS);
