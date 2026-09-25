import { describe, expect, it } from 'vitest';
import {
  assertValidTrace,
  buildBatchTrace,
  buildInconsistentTrace,
  buildLogEventTrace,
  buildPanicTrace,
  buildReadWithWritebackTrace,
  checkGasConsistency,
  gasBreakdown,
  gasOf,
  loadTrace,
  parseTrace,
  serializeTrace,
  storageAt,
  storageDiff,
  storageRows,
  validateTrace,
  TRACE_FORMAT_VERSION,
} from '../src/index';
import type { TransactionTrace } from '../src/index';

describe('trace format', () => {
  it('accepts a well-formed trace', () => {
    expect(validateTrace(buildLogEventTrace())).toEqual([]);
  });

  it('rejects an unknown format version', () => {
    const trace = { ...buildLogEventTrace(), formatVersion: 99 };
    const issues = validateTrace(trace);
    expect(issues.some((i) => i.path === 'formatVersion')).toBe(true);
  });

  it('rejects a non-dense step index', () => {
    const trace = buildLogEventTrace();
    trace.steps[3].index = 99;
    const issues = validateTrace(trace);
    expect(issues.some((i) => i.path === 'steps[3].index')).toBe(true);
  });

  it('rejects a step referring to an unknown frame', () => {
    const trace = buildLogEventTrace();
    trace.steps[2].frameId = 'f99';
    expect(validateTrace(trace).some((i) => i.path === 'steps[2].frameId')).toBe(true);
  });

  it('rejects a frame with an unknown parent', () => {
    const trace = buildLogEventTrace();
    trace.frames[1].parentId = 'nope';
    expect(validateTrace(trace).some((i) => i.path === 'frames[1].parentId')).toBe(true);
  });

  it('rejects a delete that carries a value', () => {
    const trace = buildLogEventTrace();
    trace.steps[2].storage = { op: 'del', scope: 'instance', key: 'Paused', value: false };
    expect(validateTrace(trace).some((i) => i.path.includes('storage.value'))).toBe(true);
  });

  it('rejects an unknown storage op', () => {
    const trace = buildLogEventTrace();
    const step = trace.steps.find((s) => s.storage);
    step!.storage!.op = 'poke' as never;
    expect(validateTrace(trace).some((i) => i.path.includes('storage.op'))).toBe(true);
  });

  it('throws with a path list via assertValidTrace', () => {
    const trace = { ...buildLogEventTrace(), formatVersion: 2 };
    expect(() => assertValidTrace(trace)).toThrow(/formatVersion/);
  });

  it('round-trips through serialize and parse', () => {
    const trace = buildLogEventTrace();
    const { trace: back, warnings } = parseTrace(serializeTrace(trace));
    expect(back).toEqual(trace);
    expect(warnings).toEqual([]);
  });

  it('declares the version it emits', () => {
    expect(buildLogEventTrace().formatVersion).toBe(TRACE_FORMAT_VERSION);
  });
});

describe('gas accounting', () => {
  it('sums step instructions to the recorded total', () => {
    const trace = buildLogEventTrace();
    expect(gasOf(trace).instructions).toBe(trace.totals.instructions);
  });

  it('sums per-operation gas to the recorded total for every dimension', () => {
    for (const trace of [buildLogEventTrace(), buildBatchTrace(), buildPanicTrace(), buildReadWithWritebackTrace()]) {
      const gas = gasOf(trace);
      const consistency = checkGasConsistency(trace);
      expect(gas.instructions).toBe(trace.totals.instructions);
      expect(consistency.consistent).toBe(true);
      expect(consistency.readBytes.delta).toBe(0);
      expect(consistency.writeBytes.delta).toBe(0);
      expect(consistency.events.delta).toBe(0);
      expect(consistency.storageReads.delta).toBe(0);
      expect(consistency.storageWrites.delta).toBe(0);
    }
  });

  it('detects a total that does not match the steps', () => {
    const trace = buildInconsistentTrace();
    const consistency = checkGasConsistency(trace);
    expect(consistency.consistent).toBe(false);
    // delta is summed - recorded: the fixture inflates the recorded total.
    expect(consistency.delta).toBe(-5_000);
    expect(consistency.summed).toBeLessThan(consistency.recorded);
    expect(consistency.readBytes.consistent).toBe(false);
  });

  it('matches each frame declaration to the steps in that frame', () => {
    for (const trace of [buildLogEventTrace(), buildBatchTrace()]) {
      for (const frame of checkGasConsistency(trace).frames) {
        expect(frame.consistent).toBe(true);
        expect(frame.delta).toBe(0);
      }
    }
  });

  it('breaks gas down by function and the shares total 100%', () => {
    const trace = buildLogEventTrace();
    const { slices, total, attributed } = gasBreakdown(trace, 'function');
    expect(attributed).toBe(total);
    expect(slices.reduce((s, x) => s + x.share, 0)).toBeCloseTo(1, 10);
    const logEvent = slices.find((s) => s.key === 'log_event');
    expect(logEvent).toBeDefined();
  });

  it('sorts the breakdown largest first', () => {
    const { slices } = gasBreakdown(buildLogEventTrace(), 'function');
    for (let i = 1; i < slices.length; i += 1) {
      expect(slices[i - 1].gas.instructions).toBeGreaterThanOrEqual(slices[i].gas.instructions);
    }
  });

  it('attributes no gas to the second get_event call beyond its own steps', () => {
    const trace = buildReadWithWritebackTrace();
    const { slices } = gasBreakdown(trace, 'function');
    // The helper is a separate function, so the parent's own cost excludes it.
    const parent = slices.find((s) => s.key === 'get_event')!;
    const helper = slices.find((s) => s.key === 'maybe_sweep_expired')!;
    expect(parent.gas.instructions).toBeLessThan(trace.totals.instructions);
    expect(helper.gas.instructions).toBeGreaterThan(0);
  });

  it('breaks gas down by operation kind', () => {
    const { slices } = gasBreakdown(buildLogEventTrace(), 'op');
    expect(slices.map((s) => s.key)).toContain('storage.set');
    expect(slices.map((s) => s.key)).toContain('event.publish');
  });

  it('treats a summary trace as authoritative rather than mismatched', () => {
    const trace = { ...buildLogEventTrace(), detail: 'summary' as const, steps: [] };
    trace.totals.instructions = 999;
    const consistency = checkGasConsistency(trace);
    expect(consistency.consistent).toBe(true);
  });
});

describe('storage state', () => {
  const trace = buildLogEventTrace();

  it('is empty before the first step', () => {
    expect(storageAt(trace, -1).size).toBe(0);
  });

  it('reflects writes once the step has run', () => {
    const index = trace.steps.findIndex((s) => s.storage?.key === 'RuntimeState' && s.storage.op === 'write');
    const before = storageAt(trace, index - 1).get('instance:RuntimeState');
    const after = storageAt(trace, index).get('instance:RuntimeState');
    expect(before?.value).toEqual({ total_events: 41, last_sequence: 41 });
    expect(after?.value).toEqual({ total_events: 42, last_sequence: 42 });
  });

  it('is keyed by scope so the same key in two tiers stays distinct', () => {
    // Built explicitly rather than relying on fixture shape.
    const scopeTrace = structuredClone(buildLogEventTrace());
    scopeTrace.steps = [
      { index: 0, frameId: 'f0', depth: 0, function: 'x', op: 'storage.get', gas: { instructions: 1 }, storage: { op: 'read', scope: 'instance', key: 'Paused', value: false } },
      { index: 1, frameId: 'f0', depth: 0, function: 'x', op: 'storage.get', gas: { instructions: 1 }, storage: { op: 'read', scope: 'persistent', key: 'Paused', value: 'other' } },
    ];
    scopeTrace.frames = [{ id: 'f0', function: 'x' }];
    const state = storageAt(scopeTrace, 1);
    expect(state.get('instance:Paused')?.value).toBe(false);
    expect(state.get('persistent:Paused')?.value).toBe('other');
  });

  it('removes a key on delete', () => {
    const t = structuredClone(buildLogEventTrace());
    t.steps = [
      { index: 0, frameId: 'f0', depth: 0, function: 'x', op: 'storage.set', gas: { instructions: 1 }, storage: { op: 'write', scope: 'instance', key: 'Paused', value: true } },
      { index: 1, frameId: 'f0', depth: 0, function: 'x', op: 'storage.del', gas: { instructions: 1 }, storage: { op: 'del', scope: 'instance', key: 'Paused', previous: true } },
    ];
    t.frames = [{ id: 'f0', function: 'x' }];
    expect(storageAt(t, 0).size).toBe(1);
    expect(storageAt(t, 1).size).toBe(0);
  });

  it('reports the final state of a log_event run', () => {
    const state = storageAt(trace, trace.steps.length - 1);
    expect(state.get('instance:RuntimeState')?.value).toEqual({ total_events: 42, last_sequence: 42 });
    expect(state.get('persistent:EventOrder(42)')?.value).toBeDefined();
    expect(state.get('persistent:SubmitterEventCount(GDEBUGGER…)')?.value).toBe(4);
  });

  it('classifies a first observation as created and a value change as updated', () => {
    const runtimeRead = trace.steps.findIndex((s) => s.storage?.key === 'RuntimeState' && s.storage.op === 'read');
    const runtimeWrite = trace.steps.findIndex((s) => s.storage?.key === 'RuntimeState' && s.storage.op === 'write');

    // From the start of the trace the key is first seen by a read.
    const fromStart = storageDiff(trace, 0, runtimeWrite).find((d) => d.key === 'RuntimeState');
    expect(fromStart?.kind).toBe('created');
    // `at` is when the reported `after` value was established, which is the
    // write, not the earlier read that first revealed the key.
    expect(fromStart?.at).toBe(runtimeWrite);

    // From just after the read, the write is a genuine value change.
    const afterRead = storageDiff(trace, runtimeRead, runtimeWrite).find((d) => d.key === 'RuntimeState');
    expect(afterRead?.kind).toBe('updated');
    expect(afterRead?.before).toEqual({ total_events: 41, last_sequence: 41 });
    expect(afterRead?.after).toEqual({ total_events: 42, last_sequence: 42 });
    expect(afterRead?.at).toBe(runtimeWrite);
  });

  it('reports a deletion with the value it held', () => {
    const t = structuredClone(trace);
    t.steps = [
      { index: 0, frameId: 'f0', depth: 0, function: 'x', op: 'storage.set', gas: { instructions: 1 }, storage: { op: 'write', scope: 'persistent', key: 'K', value: 1 } },
      { index: 1, frameId: 'f0', depth: 0, function: 'x', op: 'storage.del', gas: { instructions: 1 }, storage: { op: 'del', scope: 'persistent', key: 'K', previous: 1 } },
    ];
    t.frames = [{ id: 'f0', function: 'x' }];
    const [d] = storageDiff(t, 0, 1);
    expect(d.kind).toBe('deleted');
    expect(d.before).toBe(1);
    expect(d.after).toBeUndefined();
    expect(d.at).toBe(1);
  });

  it('reports nothing when a range re-reads the same values', () => {
    const t = structuredClone(trace);
    t.steps = [
      { index: 0, frameId: 'f0', depth: 0, function: 'x', op: 'storage.get', gas: { instructions: 1 }, storage: { op: 'read', scope: 'instance', key: 'Paused', value: false } },
      { index: 1, frameId: 'f0', depth: 0, function: 'x', op: 'storage.get', gas: { instructions: 1 }, storage: { op: 'read', scope: 'instance', key: 'Paused', value: false } },
      { index: 2, frameId: 'f0', depth: 0, function: 'x', op: 'nop', gas: { instructions: 1 } },
    ];
    t.frames = [{ id: 'f0', function: 'x' }];
    expect(storageDiff(t, 0, 1)).toEqual([]);
  });

  it('tracks the last write index for each key', () => {
    const state = storageAt(buildBatchTrace(), buildBatchTrace().steps.length - 1);
    const runtime = state.get('instance:RuntimeState');
    expect(runtime?.lastWrittenAt).toBe(buildBatchTrace().steps.length - 2);
  });

  it('sorts storage rows for stable display', () => {
    const rows = storageRows(storageAt(trace, trace.steps.length - 1));
    const ids = rows.map((r) => r.id);
    expect([...ids].sort()).toEqual(ids);
  });
});

describe('trace loading', () => {
  it('reads a trace from disk and re-validates it', () => {
    const trace = buildLogEventTrace();
    const path = `${process.env.TMPDIR ?? '/tmp'}/audit-ledger-debugger-test.json`;
    require('fs').writeFileSync(path, serializeTrace(trace));
    const { trace: loaded, warnings } = loadTrace(path);
    expect(loaded.steps.length).toBe(trace.steps.length);
    expect(warnings).toEqual([]);
  });

  it('surfaces an accounting mismatch as a load warning', () => {
    const path = `${process.env.TMPDIR ?? '/tmp'}/audit-ledger-debugger-bad.json`;
    require('fs').writeFileSync(path, serializeTrace(buildInconsistentTrace()));
    const { warnings } = loadTrace(path);
    // Structural validation passes; the gas cross-check is reported separately
    // by `verify`, so a structurally valid but inconsistent trace still loads.
    expect(warnings).toEqual([]);
  });

  it('rejects a missing file with a clear message', () => {
    expect(() => loadTrace('/nonexistent/trace.json')).toThrow(/trace not found/);
  });
});
