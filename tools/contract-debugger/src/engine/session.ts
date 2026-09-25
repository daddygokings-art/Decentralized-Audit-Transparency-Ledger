import { Step, StorageScope, TraceFrame, TransactionTrace, storageKeyId } from '../trace/schema';
import { ExpressionError, evaluate, stringify } from './expr';
import { StorageState, applyStep, deepEqual, storageAt } from './storage';
import { GasUsage, remainingGas } from './gas';
import { EventRecord, eventsUpTo } from './events';

export type StopReason = 'entry' | 'breakpoint' | 'step' | 'end' | 'error' | 'pause';

/** A breakpoint on a contract function, optionally narrowed to a line. */
export interface Breakpoint {
  id: number;
  functionName: string;
  line?: number;
  /** Only break on this operation kind, e.g. `storage.set`. */
  op?: string;
  hitCount: number;
  verified: boolean;
}

export type StepDirection = 'into' | 'over' | 'out' | 'continue' | 'pause';

export interface StoppedState {
  reason: StopReason;
  stepIndex: number;
  /** The step that just executed. */
  current: Step | undefined;
  description: string;
  text: string;
}

export interface StackFrame {
  id: number;
  /** DAP frame id; stable for the lifetime of the session. */
  name: string;
  line: number;
  column: number;
  source?: { name: string; path: string };
  stepIndex: number;
  depth: number;
  parentFrameId?: number;
}

export interface Variable {
  name: string;
  value: string;
  type: string;
  variablesReference: number;
  namedVariables?: number;
  indexedVariables?: number;
}

/** Scope groups shown in the DAP variables pane. */
export interface ScopeDescriptor {
  name: string;
  variablesReference: number;
  namedVariables: number;
  expensive: boolean;
}

const LOCAL_SCOPE = 1;
const ARG_SCOPE = 2;
const STORAGE_SCOPE = 3;
const STACK_SCOPE = 4;
const GAS_SCOPE = 5;

export interface SessionOptions {
  trace: TransactionTrace;
  /** Called for every line the debugger would print in a REPL. */
  output?: (line: string) => void;
  /** Starting position; defaults to before the first step (entry stop). */
  startAt?: number;
}

/**
 * Step-through debugger over a recorded trace.
 *
 * The session owns a cursor: an index into `trace.steps`, plus a break-at-step
 * -1 meaning "stopped before the first step". Everything a client can ask for —
 * stack, locals, storage, gas, events, watch expressions — is answered from the
 * trace at the cursor, never from a mirrored mutable copy of state.
 */
export class DebugSession {
  readonly trace: TransactionTrace;
  private cursor: number;
  private paused = false;
  private terminated = false;
  private breakpoints: Breakpoint[] = [];
  private nextBreakpointId = 1;
  private readonly output: (line: string) => void;
  private storageCache = new Map<number, StorageState>();
  private frameOrder: string[] = [];
  private frameMeta = new Map<string, { id: number; parentFrameId?: number }>();

  constructor(options: SessionOptions) {
    this.trace = options.trace;
    this.cursor = (options.startAt ?? -1) as number;
    this.output = options.output ?? (() => undefined);
    this.indexFrames();
  }

  private indexFrames(): void {
    this.frameOrder = [];
    this.frameMeta = new Map();
    const byId = new Map(this.trace.frames.map((f) => [f.id, f]));
    const roots = this.trace.frames.filter((f) => !f.parentId);
    const visit = (frame: TraceFrame, parentFrameId: number | undefined): void => {
      this.frameOrder.push(frame.id);
      this.frameMeta.set(frame.id, { id: this.frameOrder.length, parentFrameId });
      for (const child of this.trace.frames.filter((f) => f.parentId === frame.id && byId.has(f.id))) {
        visit(child, this.frameMeta.get(frame.id)!.id);
      }
    };
    for (const root of roots) visit(root, undefined);
    for (const f of this.trace.frames) if (!this.frameMeta.has(f.id)) visit(f, undefined);
  }

  get stepIndex(): number {
    return this.cursor;
  }

  get isPaused(): boolean {
    return this.paused;
  }

  get isTerminated(): boolean {
    return this.terminated;
  }

  /** Step at the cursor, or `undefined` before the first step. */
  currentStep(): Step | undefined {
    return this.trace.steps[this.cursor];
  }

  /**
   * Move the cursor.
   *
   * Returns the resulting stop. `into` descends into a callee, `over` runs to
   * the next step at the same depth or shallower, `out` runs until the current
   * frame is left, and `continue` runs to the next breakpoint or the end.
   */
  step(direction: StepDirection): StoppedState {
    if (this.terminated) return this.stopState('end', 'session already finished');
    if (direction === 'pause') {
      this.paused = false;
      return this.stopState('pause', 'paused');
    }
    this.paused = false;

    if (direction === 'continue') {
      const hit = this.runToNextBreakpoint(this.cursor + 1);
      return hit;
    }

    const current = this.currentStep();
    if (direction === 'into') {
      return this.advance(this.cursor + 1, 'step', `stepped into ${this.describe(this.currentStep())}`);
    }
    if (current === undefined) {
      return this.advance(this.cursor + 1, 'step', 'stepped');
    }
    if (direction === 'over') {
      const depth = current.depth;
      let next = this.cursor + 1;
      while (next < this.trace.steps.length && this.trace.steps[next].depth > depth) next += 1;
      return this.advance(next, 'step', `stepped over ${current.function}`);
    }
    // out
    const depth = current.depth;
    let next = this.cursor + 1;
    while (next < this.trace.steps.length && this.trace.steps[next].depth >= depth) next += 1;
    return this.advance(next, 'step', `stepped out of ${current.function}`);
  }

  /**
   * Position the cursor at an exact step without executing anything.
   *
   * Replay is a pure fold, so seeking is O(1) and cannot disagree with having
   * stepped there normally — which is what makes the UI's jump-to-step and the
   * debugger's "step backwards" safe. `index` is clamped to the trace.
   */
  goto(index: number): StoppedState {
    if (index < 0 || this.trace.steps.length === 0) {
      this.cursor = -1;
      this.paused = true;
      return this.stopState('entry', 'before the first step', 'entry');
    }
    this.cursor = Math.min(index, this.trace.steps.length - 1);
    this.paused = true;
    const step = this.currentStep();
    if (step?.error) return this.stopState('error', step.error.name, step.note ?? `panic: ${step.error.name}`);
    return this.stopState('step', this.describe(step), this.describe(step));
  }

  /** Run until a breakpoint, an error, or the end of the trace. */
  runToNextBreakpoint(from: number): StoppedState {
    for (let i = Math.max(0, from); i < this.trace.steps.length; i += 1) {
      const step = this.trace.steps[i];
      if (step.error) {
        this.cursor = i;
        return this.stopState('error', `${step.error.name}`, step.note ?? `panic: ${step.error.name}`);
      }
      const bp = this.matchBreakpoint(step);
      if (bp) {
        bp.hitCount += 1;
        this.cursor = i;
        return this.stopState('breakpoint', `breakpoint ${bp.id} (${bp.functionName})`, this.describe(step));
      }
    }
    this.cursor = this.trace.steps.length - 1;
    return this.stopState('end', 'reached end of trace', `${this.trace.steps.length} steps`);
  }

  private advance(to: number, reason: StopReason, description: string): StoppedState {
    if (to >= this.trace.steps.length) {
      this.cursor = this.trace.steps.length - 1;
      this.paused = true;
      if (this.trace.steps.length === 0) return this.stopState('entry', 'empty trace', 'no steps recorded');
      return this.stopState('end', 'reached end of trace', `${this.trace.steps.length} steps`);
    }
    this.cursor = to;
    this.paused = true;
    const step = this.currentStep();
    if (step?.error) return this.stopState('error', step.error.name, step.note ?? `panic: ${step.error.name}`);
    return this.stopState(reason, description, this.describe(step));
  }

  private stopState(reason: StopReason, description: string, text: string = description): StoppedState {
    this.paused = true;
    return { reason, stepIndex: this.cursor, current: this.currentStep(), description, text };
  }

  private describe(step: Step | undefined): string {
    if (!step) return 'entry';
    if (step.storage) return `${step.storage.op} ${step.storage.scope}:${step.storage.key}`;
    if (step.event) return `event ${step.event.name}`;
    if (step.call) return `call ${step.call.target}`;
    if (step.error) return `panic ${step.error.name}`;
    return step.op;
  }

  /** Breakpoint that matches a step, if any. */
  private matchBreakpoint(step: Step): Breakpoint | undefined {
    return this.breakpoints.find(
      (bp) =>
        bp.functionName === step.function &&
        (bp.op === undefined || bp.op === step.op) &&
        (bp.line === undefined || bp.line === step.location?.line),
    );
  }

  setBreakpoints(request: Array<Omit<Breakpoint, 'id' | 'hitCount' | 'verified'>>): Breakpoint[] {
    this.breakpoints = request.map((r) => {
      const id = this.nextBreakpointId++;
      const verified = this.trace.steps.some((s) => s.function === r.functionName && (r.line === undefined || s.location?.line === r.line));
      return { id, hitCount: 0, verified, ...r };
    });
    return this.breakpoints;
  }

  getBreakpoints(): Breakpoint[] {
    return this.breakpoints;
  }

  /**
   * Register one more breakpoint alongside the existing ones.
   *
   * Used for source-line breakpoints, which are mapped to the function owning
   * the line before they reach the session.
   */
  addBreakpoint(spec: Omit<Breakpoint, 'id' | 'hitCount' | 'verified'>, verified = true): Breakpoint {
    const bp: Breakpoint = {
      id: this.nextBreakpointId++,
      hitCount: 0,
      verified,
      ...spec,
    };
    this.breakpoints.push(bp);
    return bp;
  }

  /**
   * Storage state at the cursor, memoised per position.
   *
   * Caching is safe because a trace is immutable; the cache is keyed by step
   * index and dropped wholesale if a caller hands in a different trace.
   */
  storage(scope?: StorageScope): StorageState {
    const cached = this.storageCache.get(this.cursor);
    const state = cached ?? storageAt(this.trace, this.cursor);
    if (!cached) this.storageCache.set(this.cursor, state);
    if (!scope) return state;
    const out: StorageState = new Map();
    for (const [k, v] of state) if (v.scope === scope) out.set(k, v);
    return out;
  }

  /** Storage state produced by a single step, for step-diff highlighting. */
  storageTouchedBy(stepIndex: number): string[] {
    const step = this.trace.steps[stepIndex];
    if (!step?.storage) return [];
    return [storageKeyId(step.storage.scope, step.storage.key)];
  }

  /** Apply the step at the cursor, for clients tracking a live overlay. */
  applyCurrentStep(): void {
    const step = this.currentStep();
    if (!step) return;
    const state = new Map(this.storage());
    applyStep(state, step);
    this.storageCache.set(this.cursor, state);
  }

  events(): EventRecord[] {
    return eventsUpTo(this.trace, this.cursor);
  }

  gasRemaining(): GasUsage {
    return remainingGas(this.trace, this.cursor + 1);
  }

  gasSpent(): GasUsage {
    return this.trace.steps
      .slice(0, this.cursor + 1)
      .reduce<GasUsage>(
        (acc, s) => ({
          instructions: acc.instructions + s.gas.instructions,
          readBytes: acc.readBytes + (s.gas.readBytes ?? 0),
          writeBytes: acc.writeBytes + (s.gas.writeBytes ?? 0),
        }),
        { instructions: 0, readBytes: 0, writeBytes: 0 },
      );
  }

  /** The call stack as it stood at the cursor, innermost first. */
  stack(): StackFrame[] {
    if (!this.paused) return [];
    const step = this.currentStep();
    if (!step) return [];
    const chain: string[] = [];
    let id: string | undefined = step.frameId;
    const byId = new Map(this.trace.frames.map((f) => [f.id, f]));
    while (id) {
      chain.unshift(id);
      id = byId.get(id)?.parentId;
    }
    return chain.map((frameId, i) => {
      const frame = byId.get(frameId);
      const meta = this.frameMeta.get(frameId);
      return {
        id: meta?.id ?? i + 1,
        name: frame?.function ?? frameId,
        line: step.location?.line ?? 1,
        column: step.location?.column ?? 1,
        source: step.location?.file ? { name: step.location.file, path: step.location.file } : undefined,
        stepIndex: this.cursor,
        depth: i,
        parentFrameId: meta?.parentFrameId,
      };
    });
  }

  /** Frame-local variables after the step at the cursor. */
  locals(): Record<string, unknown> {
    return { ...(this.currentStep()?.locals ?? {}) };
  }

  /** Arguments of the frame the cursor is in. */
  frameArgs(): Record<string, unknown> {
    return { ...(this.trace.frames.find((f) => f.id === this.currentStep()?.frameId)?.args ?? {}) };
  }

  scopes(): ScopeDescriptor[] {
    if (!this.paused) return [];
    const locals = this.locals();
    const args = this.frameArgs();
    const storage = this.storage();
    return [
      { name: 'Locals', variablesReference: LOCAL_SCOPE, namedVariables: Object.keys(locals).length, expensive: false },
      { name: 'Arguments', variablesReference: ARG_SCOPE, namedVariables: Object.keys(args).length, expensive: false },
      { name: 'Storage', variablesReference: STORAGE_SCOPE, namedVariables: storage.size, expensive: true },
      { name: 'Call Stack', variablesReference: STACK_SCOPE, namedVariables: this.stack().length, expensive: false },
      { name: 'Gas', variablesReference: GAS_SCOPE, namedVariables: 3, expensive: false },
    ];
  }

  /** Expand one of the scope references from {@link scopes}. */
  variables(reference: number): Variable[] {
    switch (reference) {
      case LOCAL_SCOPE:
        return toVariables(this.locals());
      case ARG_SCOPE:
        return toVariables(this.frameArgs());
      case STORAGE_SCOPE:
        return [...this.storage().values()]
          .sort((a, b) => storageKeyId(a.scope, a.key).localeCompare(storageKeyId(b.scope, b.key)))
          .map((e) => ({
            name: `${e.scope}:${e.key}`,
            value: stringify(e.value),
            type: e.value === undefined ? 'none' : Array.isArray(e.value) ? 'array' : typeof e.value,
            variablesReference: isExpandable(e.value) ? 400_000 + this.cursor * 1000 + (e.lastWrittenAt % 1000) : 0,
            namedVariables: isPlainRecord(e.value) ? Object.keys(e.value).length : undefined,
          }));
      case STACK_SCOPE:
        return this.stack()
          .slice()
          .reverse()
          .map((f) => ({
            name: `#${f.depth} ${f.name}`,
            value: f.depth === 0 ? '<top>' : '<caller>',
            type: 'frame',
            variablesReference: 0,
          }));
      case GAS_SCOPE: {
        const spent = this.gasSpent();
        const left = this.gasRemaining();
        return [
          { name: 'instructionsSpent', value: String(spent.instructions), type: 'number', variablesReference: 0 },
          { name: 'instructionsRemaining', value: String(left.instructions), type: 'number', variablesReference: 0 },
          { name: 'readBytesSpent', value: String(spent.readBytes), type: 'number', variablesReference: 0 },
          { name: 'writeBytesSpent', value: String(spent.writeBytes), type: 'number', variablesReference: 0 },
        ];
      }
      default: {
        const entry = this.storageEntryForReference(reference);
        if (!entry) return [];
        return toVariables(entry);
      }
    }
  }

  private storageEntryForReference(reference: number): Record<string, unknown> | null {
    const offset = reference - 400_000 - this.cursor * 1000;
    if (offset < 0 || offset > 999) return null;
    const write = this.trace.steps.find((s) => s.index === offset && s.storage?.op === 'write');
    const keyId = write ? storageKeyId(write.storage!.scope, write.storage!.key) : '';
    const entry = this.storage().get(keyId);
    return isPlainRecord(entry?.value) ? (entry.value as Record<string, unknown>) : null;
  }

  /** Scope visible to watch expressions at the cursor. */
  evaluationScope(): Record<string, unknown> {
    // Flat, dotted access: `storage.RuntimeState`, `storage["EventData(…)"]`.
    // Scoped lookup stays available through `storageEntries` when a key
    // appears in both tiers.
    const storageView: Record<string, unknown> = Object.create(null);
    for (const e of this.storage().values()) storageView[e.key] = e.value;
    const byScope: Record<string, Record<string, unknown>> = { instance: {}, persistent: {} };
    for (const e of this.storage().values()) byScope[e.scope][e.key] = e.value;
    return {
      locals: this.locals(),
      args: this.frameArgs(),
      storage: storageView,
      storageByScope: byScope,
      stack: this.stack().map((f) => f.name),
      frame: this.currentStep()?.function,
      step: this.cursor,
      gas: { ...this.gasSpent(), remaining: this.gasRemaining().instructions },
      events: this.events().map((e) => e.name),
      contract: this.trace.contract,
      transaction: this.trace.transaction,
    };
  }

  /**
   * Evaluate a watch expression.
   *
   * Returns the value plus a rendering, because DAP clients need the raw value
   * for the variables pane and the CLI needs the string form.
   */
  watch(expression: string): { value: unknown; display: string; type: string } {
    const value = evaluate(expression, this.evaluationScope());
    return { value, display: stringify(value), type: value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value };
  }

  /** Evaluate, converting syntax and lookup failures into a message. */
  tryWatch(expression: string): { ok: true; display: string; type: string } | { ok: false; error: string } {
    try {
      const r = this.watch(expression);
      return { ok: true, display: r.display, type: r.type };
    } catch (e) {
      if (e instanceof ExpressionError) return { ok: false, error: `${e.message} (at offset ${e.offset})` };
      return { ok: false, error: e instanceof Error ? e.message : String(e) };
    }
  }

  /** True when two values in scope differ, for conditional watch display. */
  static differs(a: unknown, b: unknown): boolean {
    return !deepEqual(a, b);
  }

  terminate(): void {
    this.terminated = true;
    this.paused = false;
  }

  print(line: string): void {
    this.output(line);
  }
}

function isPlainRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function isExpandable(v: unknown): boolean {
  return isPlainRecord(v) || Array.isArray(v);
}

function toVariables(record: Record<string, unknown>): Variable[] {
  return Object.entries(record).map(([name, value]) => ({
    name,
    value: stringify(value),
    type: value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value,
    // Locals and arguments are leaves; the value is already rendered whole.
    // Only storage entries get an expandable reference, since their values can
    // be large enough to be worth collapsing.
    variablesReference: 0,
    namedVariables: isPlainRecord(value) ? Object.keys(value).length : undefined,
    indexedVariables: Array.isArray(value) ? value.length : undefined,
  }));
}
