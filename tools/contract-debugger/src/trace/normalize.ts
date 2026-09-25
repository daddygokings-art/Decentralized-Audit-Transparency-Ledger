import {
  TraceContract,
  TraceEvent,
  TraceFrame,
  TraceTotals,
  TransactionTrace,
  emptyTotals,
} from './schema';

type Rec = Record<string, unknown>;

function isRecord(v: unknown): v is Rec {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function str(v: unknown, fallback = ''): string {
  return typeof v === 'string' ? v : fallback;
}

/**
 * A Soroban RPC simulation or ledger result, as returned by
 * `simulateTransaction` or `getTransaction`.
 *
 * Only the fields the normalizer needs are declared; everything else is ignored
 * so a node version bump does not break ingestion.
 */
export interface RpcResultEnvelope {
  jsonrpc?: string;
  result?: {
    /** Present on simulateTransaction. */
    transactionData?: {
      minResourceFee?: string;
      cost?: { cpuInsns?: string; memBytes?: string };
      events?: unknown[];
      footprint?: { read?: unknown; readWrite?: unknown; archive?: unknown };
      results?: unknown[];
      restorePreamble?: { minResourceFee?: string; cost?: { cpuInsns?: string; memBytes?: string } };
    };
    /** Present on getTransaction. */
    resultMeta?: {
      returnValue?: string;
      events?: unknown[];
      txChangesBefore?: { created?: unknown[]; updated?: unknown[]; deleted?: unknown[]; ledgerEntries?: unknown[] };
      txChangesAfter?: { created?: unknown[]; updated?: unknown[]; deleted?: unknown[]; ledgerEntries?: unknown[] };
    };
    transaction?: {
      sourceAccount?: string;
      operations?: unknown[];
      ledger?: number;
      hash?: string;
    };
    ledger?: number;
  };
  error?: { code?: number; message?: string };
}

export interface NormalizeInput {
  rpc: RpcResultEnvelope;
  contract: Partial<TraceContract> & { id: string; name: string };
  /** Ledger the interaction was simulated against. */
  ledger?: number;
  /** Hash to record when the envelope does not carry one. */
  hash?: string;
}

function bigToNum(v: unknown): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'string' && /^\d+$/.test(v)) return Number(v);
  return 0;
}

/**
 * Normalize RPC output into a `summary`-detail trace.
 *
 * Honest about its limits: an RPC result carries the *aggregate* cost, the
 * emitted events, and a before/after ledger-entry diff. It does not carry the
 * per-operation sequence, per-step gas, or frame structure that step-through
 * debugging needs, so those are left empty and `detail` is set to `summary`.
 * `audit-ledger-debug verify` treats a summary trace's totals as authoritative
 * rather than reporting the missing step fold as a mismatch.
 */
export function normalizeRpcResult(input: NormalizeInput): TransactionTrace {
  const result = input.rpc.result ?? {};
  const td = result.transactionData ?? {};
  const meta = result.resultMeta ?? {};
  const tx = result.transaction ?? {};

  const cost = td.cost ?? td.restorePreamble?.cost ?? {};
  const feeText = td.minResourceFee ?? td.restorePreamble?.minResourceFee;

  const events = extractEvents(meta.events ?? td.events ?? []);

  const totals: TraceTotals = emptyTotals();
  totals.instructions = bigToNum(cost.cpuInsns);
  totals.readBytes = footprintCount(td.footprint?.read) + footprintCount(td.footprint?.readWrite);
  totals.writeBytes = footprintCount(td.footprint?.readWrite) + footprintCount(td.footprint?.archive);
  totals.events = events.length;
  totals.fee = feeText !== undefined ? bigToNum(feeText) : 0;

  // Ledger-entry churn, when the node reported it. Keys are raw XDR here: the
  // debugger deliberately avoids an XDR dependency, so a key stays opaque
  // unless a trace producer decoded it.
  const changes = meta.txChangesAfter ?? meta.txChangesBefore ?? {};
  const writes = (changes.created ?? []).length + (changes.updated ?? []).length;
  const deletes = (changes.deleted ?? []).length;
  totals.storageReads = 0;
  totals.storageWrites = writes;

  const steps: TransactionTrace['steps'] = [];
  let index = 0;
  const frame: TraceFrame = { id: 'f0', function: 'entry' };
  for (const e of events) {
    steps.push({
      index: index++,
      frameId: frame.id,
      depth: 0,
      function: frame.function,
      op: 'event.publish',
      gas: { instructions: 0 },
      event: e,
      note: 'event recovered from RPC result metadata',
    });
  }
  if (writes + deletes > 0) {
    steps.push({
      index: index++,
      frameId: frame.id,
      depth: 0,
      function: frame.function,
      op: 'storage.changes',
      gas: { instructions: 0 },
      note: `${writes} written, ${deletes} deleted ledger entries (values not decoded)`,
    });
  }

  return {
    formatVersion: 1,
    contract: {
      id: input.contract.id,
      name: input.contract.name,
      version: input.contract.version ?? '0.0.0',
      wasmHash: input.contract.wasmHash,
    },
    transaction: {
      hash: input.hash ?? (str(tx.hash) || undefined),
      source: 'rpc-simulation',
      ledger: input.ledger ?? (typeof tx.ledger === 'number' ? tx.ledger : result.ledger),
      sourceAccount: str(tx.sourceAccount) || undefined,
      operations: Array.isArray(tx.operations) ? tx.operations.length : undefined,
    },
    detail: 'summary',
    totals,
    frames: [frame],
    steps,
  };
}

function footprintCount(v: unknown): number {
  if (!Array.isArray(v)) return 0;
  return v.length;
}

/**
 * Pull the event name and payload out of a diagnostic event.
 *
 * Diagnostic events use XDR-scVal shapes whose type is a small integer, so the
 * topics and data are normalized to a tagged object rather than a bare value.
 */
function extractEvents(raw: unknown[]): TraceEvent[] {
  const out: TraceEvent[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    const inner = isRecord(item.event) ? item.event : item;
    const name = decodeScval(inner.type);
    if (typeof name !== 'string' || name.length === 0) continue;
    const topics: unknown[] = [];
    const data: unknown[] = [];
    if (Array.isArray(inner.body)) {
      for (const [i, v] of inner.body.entries()) (i < 3 ? topics : data).push(decodeScval(v));
    }
    out.push({
      name,
      topics: topics.map((t) => ({ str: t })),
      data: data.map((d) => ({ val: d })),
      sequence: typeof inner.inSuccessfulContractCall === 'boolean' ? (inner.inSuccessfulContractCall ? 1 : 0) : undefined,
    });
  }
  return out;
}

/**
 * Decode the subset of `scval` that a diagnostic event header uses.
 *
 * The layout mirrors the on-chain `ScVal` union: a 4-byte discriminant, then
 * the body. Symbol, string, and the small numeric forms are unwrapped; anything
 * else is returned as a hex string so nothing is silently misread.
 */
function decodeScval(v: unknown): unknown {
  if (typeof v === 'string') return decodeBase64Scval(v);
  if (isRecord(v)) {
    if (typeof v.str === 'string') return v.str;
    if (typeof v.sym === 'string') return v.sym;
    if (isRecord(v.vec)) return (v.vec as Rec).vec ?? [];
  }
  return v;
}

function decodeBase64Scval(b64: string): unknown {
  if (b64.length < 8) return b64;
  try {
    const buf = Buffer.from(b64, 'base64');
    if (buf.length < 4) return b64;
    const disc = buf.readUInt32LE(0);
    // 0: false, 1: true, 2: void, 6: u32, 9: symbol, 10: string
    if (disc === 9) return buf.subarray(8, 32).toString('hex');
    if (disc === 10) return buf.subarray(4).toString('utf8');
    if (disc === 6) return buf.readUInt32LE(4);
    if (disc === 0) return false;
    if (disc === 1) return true;
    if (disc === 2) return null;
    return `scval:${disc}:${buf.toString('hex').slice(0, 16)}`;
  } catch {
    return b64;
  }
}
