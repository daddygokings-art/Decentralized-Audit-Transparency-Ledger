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

// ── Scrape loop ─────────────────────────────────────────────────────────────

async function scrape() {
  try {
    // total_events
    const totalVal = await callContract("total_events");
    const total = scValToU32(totalVal);
    totalEvents.set(total);

    // global_max_logs – read from ledger storage directly or via a helper
    // We approximate by fetching from contract storage key if not exposed via API.
    // For now, track what we can derive.
    try {
      // Try to call a hypothetical get_global_max (not in contract) –
      // gracefully skip if unavailable.
      const maxVal = await callContract("get_global_max_logs");
      const max = scValToU32(maxVal);
      globalMaxLogs.set(max);
      storageUsagePct.set(max > 0 ? (total / max) * 100 : 0);
    } catch {
      // Contract doesn't expose this endpoint; skip global max metrics
    }

    // events_by_type – requires knowing all types; scrape is best-effort
    // In a production setup, maintain a list of known types in config or
    // discover them from ledger state via get_ledger_entries.
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
  } catch (err) {
    console.error("Scrape error:", err.message);
    errorCount.inc();
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
