/**
 * Client wrapper for the AuditLedger Soroban contract.
 *
 * All reads are performed via simulateTransaction (no auth / fees needed).
 */
import {
  SorobanRpc,
  Contract,
  Networks,
  TransactionBuilder,
  Account,
  xdr,
  nativeToScVal,
  scValToNative,
} from "@stellar/stellar-sdk";
import type { AuditEvent, ContractStats, SearchFilters } from "@/types";

const CONTRACT_ID =
  process.env.NEXT_PUBLIC_CONTRACT_ID ??
  process.env.CONTRACT_ID ??
  "";

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ??
  process.env.RPC_URL ??
  "https://soroban-testnet.stellar.org";

const NETWORK =
  process.env.NEXT_PUBLIC_NETWORK ?? process.env.NETWORK ?? "testnet";

const networkPassphrase =
  NETWORK === "mainnet" ? Networks.PUBLIC : Networks.TESTNET;

// Dummy fee-only source for read-only simulations
const DUMMY_SOURCE = new Account(
  "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
  "0"
);

const server = new SorobanRpc.Server(RPC_URL, {
  allowHttp: RPC_URL.startsWith("http://"),
});

async function callContract(method: string, args: xdr.ScVal[] = []) {
  const contract = new Contract(CONTRACT_ID);
  const op = contract.call(method, ...args);
  const tx = new TransactionBuilder(DUMMY_SOURCE, {
    fee: "100",
    networkPassphrase,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    throw new Error(`Contract error: ${sim.error}`);
  }
  return sim.result?.retval;
}

/** Decode a contract Event ScVal to our AuditEvent interface. */
function decodeEvent(scval: xdr.ScVal): AuditEvent {
  const map = scValToNative(scval) as Record<string, unknown>;
  const toHex = (v: unknown) =>
    Buffer.from(v as Uint8Array).toString("hex");

  return {
    id: "", // filled in by callers who know the id
    index: Number(map.index),
    timestamp: Number(map.timestamp),
    event_type: String(map.event_type),
    submitter: String(map.submitter),
    metadata: toHex(map.metadata),
    event_hash: toHex(map.event_hash),
    prev_hash: toHex(map.prev_hash),
  };
}

export async function fetchTotalEvents(): Promise<number> {
  const val = await callContract("total_events");
  return val ? Number(scValToNative(val)) : 0;
}

export async function fetchEventByOrder(order: number): Promise<AuditEvent> {
  const val = await callContract("get_event_by_order", [
    nativeToScVal(order, { type: "u32" }),
  ]);
  const evt = decodeEvent(val!);
  evt.id = evt.event_hash; // use event_hash as display ID
  return evt;
}

export async function fetchEventById(id: string): Promise<AuditEvent> {
  const idBytes = Buffer.from(id, "hex");
  const val = await callContract("get_event", [
    xdr.ScVal.scvBytes(idBytes),
  ]);
  const evt = decodeEvent(val!);
  evt.id = id;
  return evt;
}

export async function fetchEventCount(eventType: string): Promise<number> {
  const val = await callContract("event_count", [
    xdr.ScVal.scvSymbol(eventType),
  ]);
  return val ? Number(scValToNative(val)) : 0;
}

/** Fetch a page of events (by sequential order). */
export async function fetchEventPage(
  page: number,
  pageSize: number
): Promise<AuditEvent[]> {
  const start = page * pageSize;
  const total = await fetchTotalEvents();
  const end = Math.min(start + pageSize, total);
  const events: AuditEvent[] = [];
  for (let i = start; i < end; i++) {
    events.push(await fetchEventByOrder(i));
  }
  return events;
}

/** Build summary stats. */
export async function fetchStats(
  knownTypes: string[]
): Promise<ContractStats> {
  const totalEvents = await fetchTotalEvents();
  const eventsByType: Record<string, number> = {};
  for (const t of knownTypes) {
    try {
      eventsByType[t] = await fetchEventCount(t);
    } catch {
      eventsByType[t] = 0;
    }
  }
  return { totalEvents, globalMaxLogs: 0, eventsByType };
}

/** Minimal search — filters fetched events client-side (suitable for small sets). */
export async function searchEvents(
  filters: SearchFilters,
  maxScan = 200
): Promise<AuditEvent[]> {
  const total = await fetchTotalEvents();
  const limit = Math.min(total, maxScan);
  const results: AuditEvent[] = [];
  for (let i = total - 1; i >= Math.max(0, total - limit); i--) {
    const evt = await fetchEventByOrder(i);
    if (filters.event_type && evt.event_type !== filters.event_type) continue;
    if (filters.submitter && !evt.submitter.includes(filters.submitter)) continue;
    if (
      filters.metadata &&
      !evt.metadata.includes(filters.metadata.toLowerCase())
    )
      continue;
    if (
      filters.dateFrom &&
      evt.timestamp < Math.floor(new Date(filters.dateFrom).getTime() / 1000)
    )
      continue;
    if (
      filters.dateTo &&
      evt.timestamp > Math.floor(new Date(filters.dateTo).getTime() / 1000)
    )
      continue;
    results.push(evt);
  }
  return results;
}
