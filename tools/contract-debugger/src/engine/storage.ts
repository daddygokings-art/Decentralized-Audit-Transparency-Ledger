import { Step, StorageEntry, StorageScope, TransactionTrace, storageKeyId } from '../trace/schema';

export type StorageState = Map<string, StorageEntry>;

/**
 * Replay storage operations up to (and including) a step.
 *
 * The debugger never mutates a trace: the state at a given step is always
 * recomputed by folding from the start. Traces are short (thousands of steps),
 * and a pure fold removes an entire class of bug where stepping backwards leaves
 * stale state behind.
 */
export function storageAt(trace: TransactionTrace, stepIndex: number): StorageState {
  const state: StorageState = new Map();
  for (const step of trace.steps) {
    if (step.index > stepIndex) break;
    applyStep(state, step);
  }
  return state;
}

/**
 * Apply one storage step to the state, in place.
 *
 * A read is authoritative too: it returns the key's current value, so it seeds
 * the state just as a write does. Without this, a key the contract only ever
 * reads (`Paused`, `AllowlistMode`, `Config`) would be missing from the storage
 * view, and a key's first observation would be misreported as a creation
 * instead of a read.
 *
 * The consequence is that a key is reported as `created` at the step it was
 * *first observed*, not necessarily the step it was first written. State before
 * that first observation is genuinely unknown to a trace, and is shown as
 * absent rather than guessed at.
 */
export function applyStep(state: StorageState, step: Step): void {
  const s = step.storage;
  if (!s) return;
  const id = storageKeyId(s.scope, s.key);
  if (s.op === 'del') {
    state.delete(id);
    return;
  }
  if (s.op === 'read' && state.has(id)) {
    // Already known; keep the earlier observation index unless the value moved.
    const known = state.get(id)!;
    if (deepEqual(known.value, s.value)) return;
  }
  state.set(id, { scope: s.scope, key: s.key, value: s.value, lastWrittenAt: step.index });
}

export interface StorageDiff {
  key: string;
  scope: StorageScope;
  /**
   * `created` means first observed in this range (by a read or a write),
   * `updated` means the value changed, `deleted` means the key went away.
   */
  kind: 'created' | 'updated' | 'deleted';
  before: unknown;
  after: unknown;
  /** Step index at which the reported `after` value was established. */
  at: number;
}

/**
 * Changes between two points in the trace.
 *
 * `to < 0` means "up to the end". Every write carries the value it replaced in
 * `previous`, which is what makes a backwards diff exact even when a key is
 * written more than once.
 */
export function storageDiff(trace: TransactionTrace, from: number, to: number = trace.steps.length - 1): StorageDiff[] {
  const before = storageAt(trace, from);
  const after = storageAt(trace, to);
  const out: StorageDiff[] = [];
  const ids = new Set([...before.keys(), ...after.keys()]);

  for (const id of [...ids].sort()) {
    const b = before.get(id);
    const a = after.get(id);
    if (b === undefined && a !== undefined) {
      out.push({ key: a.key, scope: a.scope, kind: 'created', before: undefined, after: a.value, at: a.lastWrittenAt });
    } else if (b !== undefined && a === undefined) {
      out.push({ key: b.key, scope: b.scope, kind: 'deleted', before: b.value, after: undefined, at: lastWriteOf(trace, from, to, id) });
    } else if (b !== undefined && a !== undefined && !deepEqual(b.value, a.value)) {
      out.push({ key: a.key, scope: a.scope, kind: 'updated', before: b.value, after: a.value, at: a.lastWrittenAt });
    }
  }
  return out;
}

/** Last step in `(from, to]` that observed the given key. */
function lastWriteOf(trace: TransactionTrace, from: number, to: number, id: string): number {
  for (let i = to; i > from; i -= 1) {
    const s = trace.steps[i]?.storage;
    if (s && storageKeyId(s.scope, s.key) === id) return i;
  }
  return to;
}

/** Structural equality, sufficient for the decoded JSON values traces carry. */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return false;
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  if (typeof a === 'object' && typeof b === 'object') {
    const ao = a as Record<string, unknown>;
    const bo = b as Record<string, unknown>;
    const keys = new Set([...Object.keys(ao), ...Object.keys(bo)]);
    for (const k of keys) if (!deepEqual(ao[k], bo[k])) return false;
    return true;
  }
  return false;
}

/** Flatten storage state for display, sorted for stable output. */
export function storageRows(state: StorageState): Array<{ id: string; scope: StorageScope; key: string; value: unknown; at: number }> {
  return [...state.values()]
    .map((e) => ({ id: storageKeyId(e.scope, e.key), scope: e.scope, key: e.key, value: e.value, at: e.lastWrittenAt }))
    .sort((a, b) => a.id.localeCompare(b.id));
}
