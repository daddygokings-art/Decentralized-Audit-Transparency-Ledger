import { TraceEvent, TransactionTrace } from '../trace/schema';

export interface EventRecord extends TraceEvent {
  /** Step that emitted the event. */
  step: number;
  /** Contract function the event was emitted from. */
  function: string;
  /** Call-stack depth of the emitting frame. */
  depth: number;
  /** Zero-based ordinal within the transaction. */
  ordinal: number;
}

/** Every event in emission order. */
export function eventLog(trace: TransactionTrace): EventRecord[] {
  const out: EventRecord[] = [];
  for (const step of trace.steps) {
    if (!step.event) continue;
    out.push({
      ...step.event,
      step: step.index,
      function: step.function,
      depth: step.depth,
      ordinal: out.length,
    });
  }
  return out;
}

/** Events emitted at or before a step. */
export function eventsUpTo(trace: TransactionTrace, stepIndex: number): EventRecord[] {
  return eventLog(trace).filter((e) => e.step <= stepIndex);
}

export interface EventBreakdown {
  name: string;
  count: number;
  /** Distinct contract functions that emitted this event. */
  emitters: string[];
  gas: number;
}

/** Event frequency and the gas spent producing each. */
export function eventBreakdown(trace: TransactionTrace): EventBreakdown[] {
  const byName = new Map<string, EventBreakdown>();
  for (const step of trace.steps) {
    if (!step.event) continue;
    let row = byName.get(step.event.name);
    if (!row) {
      row = { name: step.event.name, count: 0, emitters: [], gas: 0 };
      byName.set(step.event.name, row);
    }
    row.count += 1;
    row.gas += step.gas.instructions;
    if (!row.emitters.includes(step.function)) row.emitters.push(step.function);
  }
  return [...byName.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}
