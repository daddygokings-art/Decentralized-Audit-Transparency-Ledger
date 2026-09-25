import { describe, expect, it } from 'vitest';
import {
  DebugSession,
  buildBatchTrace,
  buildLogEventTrace,
  buildPanicTrace,
  buildReadWithWritebackTrace,
  eventLog,
  stringify,
} from '../src/index';
import type { TransactionTrace } from '../src/index';

function sessionAt(trace: TransactionTrace, index: number): DebugSession {
  const s = new DebugSession({ trace });
  s.goto(index);
  return s;
}

describe('stepping', () => {
  const trace = buildLogEventTrace();

  it('starts before the first step', () => {
    const s = new DebugSession({ trace });
    expect(s.stepIndex).toBe(-1);
    expect(s.currentStep()).toBeUndefined();
    expect(s.stack()).toEqual([]);
  });

  it('steps into the first step', () => {
    const s = new DebugSession({ trace });
    const stop = s.step('into');
    expect(stop.stepIndex).toBe(0);
    expect(stop.reason).toBe('step');
    expect(s.currentStep()?.function).toBe('log_event');
  });

  it('runs the whole callee when stepping over a call', () => {
    const s = new DebugSession({ trace });
    const call = trace.steps.findIndex((x) => x.op === 'call' && x.call?.target === 'load_config');
    s.goto(call);
    const helperSteps = trace.steps.filter((x) => x.function === 'load_config').map((x) => x.index);
    const stop = s.step('over');
    // Landed past every step of the callee, in the caller's frame.
    expect(helperSteps.every((i) => i < stop.stepIndex)).toBe(true);
    expect(s.currentStep()?.depth).toBe(0);
  });

  it('descends into the callee when stepping into a call', () => {
    const s = new DebugSession({ trace });
    const call = trace.steps.findIndex((x) => x.op === 'call' && x.call?.target === 'load_config');
    s.goto(call);
    const stop = s.step('into');
    expect(stop.stepIndex).toBe(call + 1);
    expect(s.currentStep()?.function).toBe('load_config');
  });

  it('steps out of a helper back to the caller', () => {
    const s = new DebugSession({ trace });
    const helperStep = trace.steps.findIndex((x) => x.function === 'load_config');
    s.runToNextBreakpoint(0);
    while (s.stepIndex < helperStep) s.step('into');
    const stop = s.step('out');
    expect(s.currentStep()?.function).toBe('log_event');
    expect(stop.stepIndex).toBeGreaterThan(helperStep);
  });

  it('never steps past the end of the trace', () => {
    const s = new DebugSession({ trace });
    s.runToNextBreakpoint(0);
    for (let i = 0; i < trace.steps.length + 5; i += 1) s.step('into');
    expect(s.stepIndex).toBe(trace.steps.length - 1);
  });

  it('reports the end of the trace', () => {
    const s = new DebugSession({ trace });
    const stop = s.runToNextBreakpoint(0);
    expect(stop.reason).toBe('end');
  });

  it('agrees between stepping to a position and seeking to it', () => {
    const walker = new DebugSession({ trace });
    for (let i = 0; i < trace.steps.length; i += 1) {
      if (i === 0) walker.goto(0);
      else walker.step('into');
      const seeker = sessionAt(trace, i);
      expect(seeker.currentStep()).toEqual(walker.currentStep());
      expect([...seeker.storage().entries()]).toEqual([...walker.storage().entries()]);
      expect(seeker.stack()).toEqual(walker.stack());
      expect(seeker.gasSpent()).toEqual(walker.gasSpent());
      expect(seeker.events()).toEqual(walker.events());
    }
  });

  it('clamps a seek past the end and reports before the start', () => {
    const s = new DebugSession({ trace });
    expect(s.goto(9_999).stepIndex).toBe(trace.steps.length - 1);
    expect(s.goto(-1).reason).toBe('entry');
    expect(s.goto(-1).stepIndex).toBe(-1);
  });
});

describe('breakpoints', () => {
  const trace = buildLogEventTrace();

  it('stops on a function breakpoint', () => {
    const s = new DebugSession({ trace });
    s.setBreakpoints([{ functionName: 'validate_metadata' }]);
    const stop = s.runToNextBreakpoint(0);
    expect(stop.reason).toBe('breakpoint');
    expect(s.currentStep()?.function).toBe('validate_metadata');
  });

  it('stops on a narrowed operation breakpoint', () => {
    const s = new DebugSession({ trace });
    s.setBreakpoints([{ functionName: 'append_event', op: 'storage.set' }]);
    const stop = s.runToNextBreakpoint(0);
    expect(s.currentStep()?.function).toBe('append_event');
    expect(s.currentStep()?.op).toBe('storage.set');
  });

  it('stops on the write of a specific key when narrowed by line', () => {
    const s = new DebugSession({ trace });
    const target = trace.steps.find((x) => x.storage?.key === 'EventOrder(42)')!;
    s.setBreakpoints([{ functionName: 'append_event', line: target.location!.line }]);
    const stop = s.runToNextBreakpoint(0);
    expect(s.currentStep()?.index).toBe(target.index);
  });

  it('verifies a breakpoint that can be hit and flags one that cannot', () => {
    const s = new DebugSession({ trace });
    const [good, bad] = s.setBreakpoints([{ functionName: 'append_event' }, { functionName: 'no_such_function' }]);
    expect(good.verified).toBe(true);
    expect(bad.verified).toBe(false);
  });

  it('counts hits and can be cleared', () => {
    const s = new DebugSession({ trace });
    s.setBreakpoints([{ functionName: 'append_event', op: 'storage.set' }]);
    const stop = s.runToNextBreakpoint(0);
    expect(s.getBreakpoints()[0].hitCount).toBe(1);
    s.setBreakpoints([]);
    expect(s.getBreakpoints()).toEqual([]);
    expect(s.runToNextBreakpoint(stop.stepIndex + 1).reason).toBe('end');
  });
});

describe('call stack', () => {
  const trace = buildLogEventTrace();

  it('is shallow at the transaction entry point', () => {
    const s = sessionAt(trace, 0);
    expect(s.stack().map((f) => f.name)).toEqual(['log_event']);
  });

  it('includes the caller chain inside a helper', () => {
    const s = sessionAt(trace, trace.steps.findIndex((x) => x.function === 'load_config'));
    expect(s.stack().map((f) => f.name)).toEqual(['log_event', 'load_config']);
  });

  it('is deepest inside the write burst', () => {
    const s = sessionAt(trace, trace.steps.findIndex((x) => x.storage?.key === 'EventOrder(42)'));
    expect(s.stack().map((f) => f.name)).toEqual(['log_event', 'append_event']);
  });

  it('numbers frames from 1 with parent links, innermost last', () => {
    const s = sessionAt(trace, trace.steps.findIndex((x) => x.function === 'load_config'));
    const stack = s.stack();
    expect(stack[0].id).toBe(1);
    expect(stack[0].parentFrameId).toBeUndefined();
    expect(stack[1].parentFrameId).toBe(1);
    expect(stack[1].depth).toBe(1);
    expect(stack[1].line).toBeGreaterThan(0);
  });

  it('returns to one frame after the helper returns', () => {
    const s = sessionAt(trace, trace.steps.findIndex((x) => x.function === 'ensure_not_paused'));
    expect(s.stack().map((f) => f.name)).toEqual(['log_event', 'ensure_not_paused']);
  });

  it('nests batch children under log_events', () => {
    const batch = buildBatchTrace();
    const s = sessionAt(batch, batch.steps.findIndex((x) => x.function === 'log_event' && x.storage?.op === 'write'));
    expect(s.stack().map((f) => f.name)).toEqual(['log_events', 'log_event']);
  });
});

describe('storage inspection per step', () => {
  const trace = buildLogEventTrace();

  it('has nothing before the first storage access', () => {
    const s = sessionAt(trace, 0);
    expect(s.storage().size).toBe(0);
  });

  it('reveals a read-only key such as Paused', () => {
    const index = trace.steps.findIndex((x) => x.storage?.key === 'Paused');
    const s = sessionAt(trace, index);
    expect(s.storage().get('instance:Paused')?.value).toBe(false);
  });

  it('shows the pre-write value one step earlier and the new value after', () => {
    const write = trace.steps.findIndex((x) => x.storage?.key === 'RuntimeState' && x.storage.op === 'write');
    expect(sessionAt(trace, write - 1).storage().get('instance:RuntimeState')?.value).toEqual({
      total_events: 41,
      last_sequence: 41,
    });
    expect(sessionAt(trace, write).storage().get('instance:RuntimeState')?.value).toEqual({
      total_events: 42,
      last_sequence: 42,
    });
  });

  it('reports which key a step touched', () => {
    const index = trace.steps.findIndex((x) => x.storage?.key === 'SubmitterEventCount(GDEBUGGER…)');
    const s = sessionAt(trace, index);
    expect(s.storageTouchedBy(s.stepIndex)).toEqual(['persistent:SubmitterEventCount(GDEBUGGER…)']);
  });

  it('accumulates every write by the end of the call', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    const state = s.storage();
    expect(state.get('instance:RuntimeState')?.value).toEqual({ total_events: 42, last_sequence: 42 });
    expect(state.get('persistent:EventTypeIndices(access_granted)')?.value).toEqual([7, 19, 22, 42]);
    expect(state.get('persistent:SubmitterEventCount(GDEBUGGER…)')?.value).toBe(4);
  });

  it('filters by storage scope', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    const persistent = s.storage('persistent');
    expect([...persistent.values()].every((e) => e.scope === 'persistent')).toBe(true);
    expect(persistent.has('instance:Paused')).toBe(false);
  });

  it('shows the write-back a read-only get_event performs', () => {
    const t = buildReadWithWritebackTrace();
    const write = t.steps.findIndex((x) => x.storage?.op === 'write' && x.storage.key === 'RuntimeState');
    expect(sessionAt(t, write - 1).storage().get('instance:RuntimeState')?.value).toMatchObject({ last_sweep: 100 });
    expect(sessionAt(t, write).storage().get('instance:RuntimeState')?.value).toMatchObject({ last_sweep: 1_100 });
  });

  it('keeps per-position storage results stable across repeated queries', () => {
    const s = sessionAt(trace, 14);
    const first = [...s.storage().entries()];
    const second = [...s.storage().entries()];
    expect(first).toEqual(second);
  });
});

describe('event tracing', () => {
  it('logs the event with its step, function and depth', () => {
    const trace = buildLogEventTrace();
    const [event] = eventLog(trace);
    expect(event.name).toBe('event_logged');
    expect(event.topics).toEqual(['access_granted', 42]);
    expect(event.function).toBe('emit_event_logged');
    expect(event.ordinal).toBe(0);
    expect(event.step).toBe(trace.steps.findIndex((s) => s.event));
  });

  it('shows only the events emitted so far while stepping', () => {
    const trace = buildLogEventTrace();
    const at = trace.steps.findIndex((s) => s.event);
    expect(sessionAt(trace, at - 1).events()).toHaveLength(0);
    expect(sessionAt(trace, at).events()).toHaveLength(1);
    expect(sessionAt(trace, trace.steps.length - 1).events()).toHaveLength(1);
  });

  it('logs one event per batch item', () => {
    expect(eventLog(buildBatchTrace())).toHaveLength(3);
  });

  it('attributes each event to the emitting function', () => {
    const events = eventLog(buildBatchTrace());
    expect(events.every((e) => e.function === 'log_event')).toBe(true);
  });
});

describe('gas per operation', () => {
  const trace = buildLogEventTrace();

  it('reports spent and remaining instructions', () => {
    const s = sessionAt(trace, 5);
    expect(s.gasSpent().instructions).toBe(trace.steps.slice(0, 6).reduce((n, x) => n + x.gas.instructions, 0));
    expect(s.gasSpent().instructions + s.gasRemaining().instructions).toBe(trace.totals.instructions);
  });

  it('reports byte traffic', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    expect(s.gasSpent().readBytes).toBe(trace.totals.readBytes);
    expect(s.gasSpent().writeBytes).toBe(trace.totals.writeBytes);
  });

  it('exposes gas as a watch scope', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    const r = s.tryWatch('gas.remaining');
    expect(r.ok && r.display).toBe('0');
  });
});

describe('error stops', () => {
  const trace = buildPanicTrace();

  it('stops on the panic with the contract error name', () => {
    const s = new DebugSession({ trace });
    const stop = s.runToNextBreakpoint(0);
    expect(stop.reason).toBe('error');
    expect(stop.description).toBe('GlobalMaxLogsReached');
  });

  it('still exposes state and a stack at the panic', () => {
    const s = new DebugSession({ trace });
    s.runToNextBreakpoint(0);
    expect(s.currentStep()?.error?.code).toBe(2004);
    expect(s.stack().length).toBeGreaterThan(0);
    expect(s.storage().get('instance:RuntimeState')?.value).toMatchObject({ total_events: 10_000 });
  });

  it('records the error on the frames that unwound', () => {
    const s = new DebugSession({ trace });
    s.runToNextBreakpoint(0);
    expect(trace.frames.every((f) => f.error?.name === 'GlobalMaxLogsReached')).toBe(true);
  });
});

describe('scopes and variables', () => {
  const trace = buildLogEventTrace();

  it('advertises the five scopes', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    expect(s.scopes().map((x) => x.name)).toEqual(['Locals', 'Arguments', 'Storage', 'Call Stack', 'Gas']);
  });

  it('lists frame locals', () => {
    const s = sessionAt(trace, 0);
    expect(s.variables(1).map((v) => v.name)).toContain('force');
  });

  it('lists the entry point arguments', () => {
    const s = sessionAt(trace, 0);
    const args = s.variables(2).map((v) => v.name);
    expect(args).toEqual(['submitter', 'event_type', 'metadata', 'category', 'sub_event_type', 'force']);
  });

  it('lists storage as scope:key with a value', () => {
    const s = sessionAt(trace, trace.steps.length - 1);
    const storage = s.variables(3);
    expect(storage.some((v) => v.name === 'instance:RuntimeState')).toBe(true);
    expect(storage.find((v) => v.name === 'instance:RuntimeState')?.value).toBe('{"total_events":42,"last_sequence":42}');
  });

  it('lists the call stack innermost first', () => {
    const s = sessionAt(trace, trace.steps.findIndex((x) => x.function === 'load_config'));
    const stack = s.variables(4);
    expect(stack[0].name).toContain('load_config');
    expect(stack[stack.length - 1].name).toContain('log_event');
  });

  it('lists gas counters', () => {
    const s = sessionAt(trace, 3);
    const names = s.variables(5).map((v) => v.name);
    expect(names).toEqual(['instructionsSpent', 'instructionsRemaining', 'readBytesSpent', 'writeBytesSpent']);
  });

  it('has no scopes before the session is stopped', () => {
    expect(new DebugSession({ trace }).scopes()).toEqual([]);
  });

  it('returns nothing for an unknown reference', () => {
    expect(sessionAt(trace, 1).variables(999_999)).toEqual([]);
  });
});

describe('watch expressions', () => {
  const trace = buildLogEventTrace();
  const s = sessionAt(trace, trace.steps.length - 1);

  it('reads a local', () => {
    const r = s.tryWatch('event_id');
    expect(r.ok).toBe(true);
    expect(r.display).toMatch(/^"0x/);
  });

  it('reads an argument', () => {
    expect(s.tryWatch('event_type').display).toBe('"access_granted"');
  });

  it('reads storage by key', () => {
    expect(s.tryWatch('storage.RuntimeState.total_events').display).toBe('42');
  });

  it('reads a persistent key with a quoted name', () => {
    expect(s.tryWatch('storage["SubmitterEventCount(GDEBUGGER…)"]').display).toBe('4');
  });

  it('reads a scoped storage path', () => {
    expect(s.tryWatch('storageByScope.persistent["EventOrder(42)"].length').display).toBe('66');
  });

  it('reads contract and transaction metadata', () => {
    expect(s.tryWatch('contract.name').display).toBe('"AuditLedger"');
    expect(s.tryWatch('transaction.source').display).toBe('"recorded"');
  });

  it('reads the call stack and the emitted event names', () => {
    expect(s.tryWatch('stack.len()').display).toBe('1');
    expect(s.tryWatch('events.contains("event_logged")').display).toBe('true');
  });

  it('evaluates arithmetic and comparison', () => {
    expect(s.tryWatch('gas.instructions > 0').display).toBe('true');
    expect(s.tryWatch('2 + 3 * 4').display).toBe('14');
  });

  it('short-circuits logical operators', () => {
    expect(s.tryWatch('false && missing.thing').display).toBe('false');
    expect(s.tryWatch('true || missing.thing').display).toBe('true');
    expect(s.tryWatch('null == null').display).toBe('true');
  });

  it('treats true, false and null as literals rather than scope lookups', () => {
    expect(s.tryWatch('true').display).toBe('true');
    expect(s.tryWatch('null').display).toBe('null');
    expect(s.tryWatch('!true').display).toBe('false');
  });

  it('evaluates to undefined for an unknown name', () => {
    expect(s.tryWatch('nope').display).toBe('undefined');
  });

  it('reports a syntax error with an offset', () => {
    const r = s.tryWatch('1 +');
    expect(r.ok).toBe(false);
    expect(r.ok === false && r.error).toMatch(/offset/);
  });

  it('reports an unknown method', () => {
    const r = s.tryWatch('storage.RuntimeState.frobnicate()');
    expect(r.ok === false && r.error).toMatch(/unknown method/);
  });

  it('does not throw on a missing member of a missing object', () => {
    expect(s.tryWatch('missing.deep.deeper').display).toBe('undefined');
  });
});

describe('session lifecycle', () => {
  it('stops answering after terminate', () => {
    const s = new DebugSession({ trace: buildLogEventTrace() });
    s.runToNextBreakpoint(0);
    s.terminate();
    expect(s.isTerminated).toBe(true);
    expect(s.step('into').reason).toBe('end');
  });

  it('forwards output lines', () => {
    const lines: string[] = [];
    const s = new DebugSession({ trace: buildLogEventTrace(), output: (l) => lines.push(l) });
    s.print('hello');
    expect(lines).toEqual(['hello']);
  });

  it('handles a trace with no steps', () => {
    const empty: TransactionTrace = { ...buildLogEventTrace(), steps: [], frames: [] };
    const s = new DebugSession({ trace: empty });
    const stop = s.runToNextBreakpoint(0);
    expect(stop.reason).toBe('end');
    expect(s.storage().size).toBe(0);
  });
});

describe('stringify', () => {
  it('renders values the way the debugger shows them', () => {
    expect(stringify('a')).toBe('"a"');
    expect(stringify(1)).toBe('1');
    expect(stringify(true)).toBe('true');
    expect(stringify(null)).toBe('null');
    expect(stringify(undefined)).toBe('undefined');
    expect(stringify([1, 2])).toBe('[1,2]');
    expect(stringify({ a: 1 })).toBe('{"a":1}');
  });
});
