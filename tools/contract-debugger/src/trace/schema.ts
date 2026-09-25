/**
 * Canonical transaction-trace format.
 *
 * One captured contract interaction becomes a {@link TransactionTrace}: the
 * contract and transaction it came from, the aggregate resource accounting, the
 * call frames, and a flat, ordered list of {@link Step}s. Flatness is the point:
 * a debugger needs one cursor it can advance, and every later view (call stack,
 * storage state, event log, gas breakdown) is a fold over the same sequence.
 *
 * The format is deliberately self-describing so a trace recorded on a testnet
 * node can be replayed anywhere without the original tooling.
 */

/** Current format version. Traces declaring anything else are rejected. */
export const TRACE_FORMAT_VERSION = 1;

/** Soroban storage tiers the contract uses. */
export type StorageScope = 'instance' | 'persistent';

/** What a step did to storage. */
export type StorageOp = 'read' | 'write' | 'del';

/** What produced a trace. Determines which fields are expected to be present. */
export type TraceSource = 'recorded' | 'rpc-simulation' | 'rpc-ledger' | 'fixture';

/**
 * How much detail a trace carries.
 *
 * `operation` is step-by-step execution: every storage access, event and call
 * is a step, and `totals` is reconstructed from those steps. `summary` is a
 * single-step view derived from RPC result metadata, where per-operation
 * detail does not exist; `totals` is then authoritative and the steps are not.
 */
export type TraceDetail = 'operation' | 'summary';

export interface TraceContract {
  /** Contract id (StrKey `C…`) or a placeholder such as `local`. */
  id: string;
  name: string;
  version: string;
  wasmHash?: string;
}

export interface TraceTransaction {
  hash?: string;
  source: TraceSource;
  /** Ledger the interaction was simulated against or included in. */
  ledger?: number;
  submittedAt?: string;
  sourceAccount?: string;
  operations?: number;
}

/** Resource accounting for the whole interaction. */
export interface TraceTotals {
  /** WASM instructions executed. */
  instructions: number;
  readBytes: number;
  writeBytes: number;
  events: number;
  storageReads: number;
  storageWrites: number;
  /** Fee in stroops. */
  fee: number;
  durationMs?: number;
}

/** A call frame. Frames form a tree through `parentId`. */
export interface TraceFrame {
  id: string;
  /** Contract function name, e.g. `log_event`. */
  function: string;
  parentId?: string;
  args?: Record<string, unknown>;
  returnValue?: unknown;
  /** Gas attributed to this frame, excluding callees. */
  gas?: {
    instructions: number;
    readBytes: number;
    writeBytes: number;
  };
  /** Populated when the frame ended in a panic. */
  error?: { name: string; code?: number };
}

export interface TraceLocation {
  file?: string;
  line?: number;
  column?: number;
}

/** Resource cost attributed to a single step. */
export interface StepGas {
  instructions: number;
  readBytes?: number;
  writeBytes?: number;
}

export interface StepStorage {
  op: StorageOp;
  scope: StorageScope;
  /**
   * Storage key in a stable textual form. Contract keys that are enum variants
   * with payloads use the Rust spelling, e.g. `EventData(9f2c…)`.
   */
  key: string;
  /** Value after the operation; absent for `del`, and for reads of absent keys. */
  value?: unknown;
  /** Value before the operation. Only written when it differs from `value`. */
  previous?: unknown;
}

/** One emitted contract event, already decoded. */
export interface TraceEvent {
  name: string;
  topics: unknown[];
  data: unknown[];
  /** Ledger sequence at emission time. */
  sequence?: number;
}

/** A cross-contract (or recursive) call. */
export interface StepCall {
  /** Callee name; same contract when unqualified. */
  target: string;
  args?: Record<string, unknown>;
}

export interface StepError {
  name: string;
  code?: number;
}

/**
 * One executed operation.
 *
 * `index` is dense and starts at 0, so a debugger position is always a valid
 * array offset and can be used directly in a UI.
 */
export interface Step {
  index: number;
  /** Frame this step executes in. */
  frameId: string;
  /** Depth of that frame, 0 for the transaction entry point. */
  depth: number;
  function: string;
  /**
   * Operation kind: `storage.get`, `storage.set`, `storage.del`,
   * `event.publish`, `call`, `return`, `panic`, or a contract-specific label.
   */
  op: string;
  location?: TraceLocation;
  gas: StepGas;
  storage?: StepStorage;
  event?: TraceEvent;
  call?: StepCall;
  error?: StepError;
  /** Frame-local variables visible after this step. */
  locals?: Record<string, unknown>;
  /** Human-readable note, e.g. why a branch was taken. */
  note?: string;
}

export interface TransactionTrace {
  formatVersion: number;
  contract: TraceContract;
  transaction: TraceTransaction;
  detail: TraceDetail;
  totals: TraceTotals;
  frames: TraceFrame[];
  steps: Step[];
}

/** Zeroed totals, used as the starting point when folding over steps. */
export function emptyTotals(): TraceTotals {
  return {
    instructions: 0,
    readBytes: 0,
    writeBytes: 0,
    events: 0,
    storageReads: 0,
    storageWrites: 0,
    fee: 0,
  };
}

/** A single storage entry as seen at some point in the trace. */
export interface StorageEntry {
  scope: StorageScope;
  key: string;
  value: unknown;
  /** Step index at which this value was last written. */
  lastWrittenAt: number;
}

/** Stable, human-readable rendering of a storage key. */
export function storageKeyId(scope: StorageScope, key: string): string {
  return `${scope}:${key}`;
}
