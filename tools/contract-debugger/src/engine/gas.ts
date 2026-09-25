import { Step, TraceTotals, TransactionTrace, emptyTotals } from '../trace/schema';

export interface GasUsage {
  instructions: number;
  readBytes: number;
  writeBytes: number;
}

/** Accumulate a step's cost into a running total. */
export function accumulate(totals: TraceTotals, step: Step): TraceTotals {
  totals.instructions += step.gas.instructions;
  totals.readBytes += step.gas.readBytes ?? 0;
  totals.writeBytes += step.gas.writeBytes ?? 0;
  if (step.storage?.op === 'read') totals.storageReads += 1;
  if (step.storage?.op === 'write') totals.storageWrites += 1;
  if (step.event) totals.events += 1;
  return totals;
}

/** Total cost of every step in the trace. */
export function gasOf(trace: TransactionTrace): TraceTotals {
  const totals = emptyTotals();
  for (const step of trace.steps) accumulate(totals, step);
  totals.fee = trace.totals.fee;
  return totals;
}

export interface GasSlice {
  key: string;
  label: string;
  gas: GasUsage;
  /** Share of total instructions, 0..1. */
  share: number;
}

/**
 * Break the instruction budget down by contract function, operation kind, or
 * call frame.
 *
 * "Own" cost for a frame excludes its callees, so a recursive or delegating
 * contract function is not double counted when the breakdown is per frame.
 */
export function gasBreakdown(
  trace: TransactionTrace,
  dimension: 'function' | 'op' | 'frame',
): { slices: GasSlice[]; total: number; attributed: number } {
  const own = new Map<string, GasUsage>();
  const label = new Map<string, string>();
  const calleeFrames = new Set<string>();
  for (const f of trace.frames) if (f.parentId) calleeFrames.add(f.id);

  for (const step of trace.steps) {
    const key =
      dimension === 'function' ? step.function : dimension === 'op' ? step.op : step.frameId;
    label.set(key, dimension === 'frame' ? (trace.frames.find((f) => f.id === step.frameId)?.function ?? step.frameId) : key);
    const bucket = own.get(key) ?? { instructions: 0, readBytes: 0, writeBytes: 0 };
    bucket.instructions += step.gas.instructions;
    bucket.readBytes += step.gas.readBytes ?? 0;
    bucket.writeBytes += step.gas.writeBytes ?? 0;
    own.set(key, bucket);
  }

  const total = trace.steps.reduce((sum, s) => sum + s.gas.instructions, 0);
  const slices = [...own.entries()]
    .map(([key, gas]) => ({
      key,
      label: label.get(key) ?? key,
      gas,
      share: total === 0 ? 0 : gas.instructions / total,
      // Kept for callers that want to hide helper frames.
      isCallee: dimension === 'frame' ? calleeFrames.has(key) : false,
    }))
    .sort((a, b) => b.gas.instructions - a.gas.instructions || a.key.localeCompare(b.key));

  const attributed = slices.reduce((sum, s) => sum + s.gas.instructions, 0);
  return { slices, total, attributed };
}

export interface GasConsistency {
  /** Per-step instructions actually observed. */
  summed: number;
  /** `totals.instructions` as recorded. */
  recorded: number;
  /** `summed - recorded`. */
  delta: number;
  consistent: boolean;
  /** The same check for the other resource dimensions. */
  readBytes: { summed: number; recorded: number; delta: number; consistent: boolean };
  writeBytes: { summed: number; recorded: number; delta: number; consistent: boolean };
  events: { summed: number; recorded: number; delta: number; consistent: boolean };
  storageReads: { summed: number; recorded: number; delta: number; consistent: boolean };
  storageWrites: { summed: number; recorded: number; delta: number; consistent: boolean };
  /** Per-frame declared cost versus the steps recorded in that frame. */
  frames: Array<{ frameId: string; declared?: number; observed: number; delta: number; consistent: boolean }>;
}

/**
 * Cross-check the recorded totals against a fold over the steps.
 *
 * This is the accuracy check behind `audit-ledger-debug verify` and the
 * `gas` command. A summary-level trace has no operation detail to fold, so its
 * totals are authoritative and reported as consistent with an empty fold; only
 * `operation` traces are held to the sum.
 */
export function checkGasConsistency(trace: TransactionTrace): GasConsistency {
  const observed = gasOf(trace);
  const isOperation = trace.detail === 'operation';
  const compare = (summed: number, recorded: number) => ({
    summed,
    recorded,
    delta: summed - recorded,
    consistent: !isOperation || summed === recorded,
  });

  const frames = trace.frames.map((f) => {
    const frameSteps = trace.steps.filter((s) => s.frameId === f.id);
    const own = frameSteps.reduce((sum, s) => sum + s.gas.instructions, 0);
    const declared = f.gas?.instructions;
    return {
      frameId: f.id,
      declared,
      observed: own,
      delta: declared === undefined ? 0 : own - declared,
      consistent: declared === undefined || own === declared,
    };
  });

  return {
    summed: observed.instructions,
    recorded: trace.totals.instructions,
    delta: observed.instructions - trace.totals.instructions,
    consistent: !isOperation || observed.instructions === trace.totals.instructions,
    readBytes: compare(observed.readBytes, trace.totals.readBytes),
    writeBytes: compare(observed.writeBytes, trace.totals.writeBytes),
    events: compare(observed.events, trace.totals.events),
    storageReads: compare(observed.storageReads, trace.totals.storageReads),
    storageWrites: compare(observed.storageWrites, trace.totals.storageWrites),
    frames,
  };
}

/** Cost attributed to a single step and everything after it, inclusive. */
export function remainingGas(trace: TransactionTrace, stepIndex: number): GasUsage {
  return trace.steps
    .slice(stepIndex)
    .reduce<GasUsage>(
      (acc, s) => ({
        instructions: acc.instructions + s.gas.instructions,
        readBytes: acc.readBytes + (s.gas.readBytes ?? 0),
        writeBytes: acc.writeBytes + (s.gas.writeBytes ?? 0),
      }),
      { instructions: 0, readBytes: 0, writeBytes: 0 },
    );
}
