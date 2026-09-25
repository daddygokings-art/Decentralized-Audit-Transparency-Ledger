import {
  StorageScope,
  Step,
  TraceContract,
  TraceEvent,
  TraceFrame,
  TraceTotals,
  TransactionTrace,
  emptyTotals,
} from '../trace/schema';

/**
 * Builder for operation-level traces.
 *
 * `totals` is always derived from the steps rather than typed in, which is the
 * point: it makes it impossible for a fixture to claim a gas total that its own
 * steps do not add up to, and it documents the invariant the recorder must hold
 * on real traces.
 */
export class TraceBuilder {
  private readonly frames: TraceFrame[] = [];
  private readonly steps: Step[] = [];
  private frameCounter = 0;

  constructor(
    private contract: TraceContract = {
      id: 'local',
      name: 'AuditLedger',
      version: '0.1.0',
    },
    private readonly options: {
      source?: TransactionTrace['transaction']['source'];
      hash?: string;
      fee?: number;
      ledger?: number;
    } = {},
  ) {}

  /** Declare a frame and return its id. Pass `parentId` to nest it. */
  frame(fn: string, opts: { parentId?: string; args?: Record<string, unknown> } = {}): string {
    const id = `f${this.frameCounter++}`;
    this.frames.push({ id, function: fn, parentId: opts.parentId, args: opts.args });
    return id;
  }

  /** Add a step and return it, so the caller can `step(...)` then adjust it. */
  step(step: Omit<Step, 'index' | 'depth'> & { depth?: number }): Step {
    const frame = this.frames.find((f) => f.id === step.frameId);
    if (!frame) throw new Error(`unknown frame ${step.frameId}`);
    const depth = step.depth ?? this.depthOf(frame.id);
    const full: Step = { ...step, index: this.steps.length, depth };
    this.steps.push(full);
    return full;
  }

  read(
    frameId: string,
    fn: string,
    scope: StorageScope,
    key: string,
    value: unknown,
    gas: number,
    extra: { line?: number; locals?: Record<string, unknown>; note?: string } = {},
  ): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'storage.get',
      gas: { instructions: gas, readBytes: 8 },
      storage: { op: 'read', scope, key, value },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
      locals: extra.locals,
      note: extra.note,
    });
  }

  write(
    frameId: string,
    fn: string,
    scope: StorageScope,
    key: string,
    value: unknown,
    gas: number,
    extra: { line?: number; previous?: unknown; locals?: Record<string, unknown>; note?: string } = {},
  ): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'storage.set',
      gas: { instructions: gas, writeBytes: 16 },
      storage: { op: 'write', scope, key, value, previous: extra.previous },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
      locals: extra.locals,
      note: extra.note,
    });
  }

  del(
    frameId: string,
    fn: string,
    scope: StorageScope,
    key: string,
    gas: number,
    extra: { line?: number; previous?: unknown } = {},
  ): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'storage.del',
      gas: { instructions: gas, writeBytes: 8 },
      storage: { op: 'del', scope, key, previous: extra.previous },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
    });
  }

  event(frameId: string, fn: string, event: TraceEvent, gas: number, extra: { line?: number } = {}): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'event.publish',
      gas: { instructions: gas, writeBytes: 64 },
      event,
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
    });
  }

  call(
    frameId: string,
    fn: string,
    target: string,
    args: Record<string, unknown>,
    gas: number,
    extra: { line?: number } = {},
  ): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'call',
      gas: { instructions: gas },
      call: { target, args },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
    });
  }

  plain(frameId: string, fn: string, op: string, gas: number, extra: { line?: number; locals?: Record<string, unknown>; note?: string } = {}): Step {
    return this.step({
      frameId,
      function: fn,
      op,
      gas: { instructions: gas },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
      locals: extra.locals,
      note: extra.note,
    });
  }

  panic(frameId: string, fn: string, name: string, code: number, gas: number, extra: { line?: number } = {}): Step {
    return this.step({
      frameId,
      function: fn,
      op: 'panic',
      gas: { instructions: gas },
      error: { name, code },
      location: { file: 'src/lib.rs', line: extra.line ?? 1 },
      note: `contract panicked with ${name} (#${code})`,
    });
  }

  /** Finish a frame with its own (callee-excluded) cost and return value. */
  closeFrame(frameId: string, returnValue: unknown): void {
    const frame = this.frames.find((f) => f.id === frameId);
    if (!frame) throw new Error(`unknown frame ${frameId}`);
    const own = this.steps
      .filter((s) => s.frameId === frameId)
      .reduce<{ instructions: number; readBytes: number; writeBytes: number }>(
        (acc, s) => ({
          instructions: acc.instructions + s.gas.instructions,
          readBytes: acc.readBytes + (s.gas.readBytes ?? 0),
          writeBytes: acc.writeBytes + (s.gas.writeBytes ?? 0),
        }),
        { instructions: 0, readBytes: 0, writeBytes: 0 },
      );
    frame.gas = own;
    frame.returnValue = returnValue;
  }

  failFrame(frameId: string, name: string, code: number): void {
    const frame = this.frames.find((f) => f.id === frameId);
    if (frame) frame.error = { name, code };
  }

  private depthOf(frameId: string): number {
    let depth = 0;
    let id: string | undefined = frameId;
    const byId = new Map(this.frames.map((f) => [f.id, f]));
    while (id) {
      const frame = byId.get(id);
      if (!frame?.parentId) break;
      depth += 1;
      id = frame.parentId;
    }
    return depth;
  }

  /** Materialise the trace, deriving `totals` from the recorded steps. */
  build(): TransactionTrace {
    const totals: TraceTotals = emptyTotals();
    for (const s of this.steps) {
      totals.instructions += s.gas.instructions;
      totals.readBytes += s.gas.readBytes ?? 0;
      totals.writeBytes += s.gas.writeBytes ?? 0;
      if (s.event) totals.events += 1;
      if (s.storage?.op === 'read') totals.storageReads += 1;
      if (s.storage?.op === 'write') totals.storageWrites += 1;
    }
    totals.fee = this.options.fee ?? 100_000;
    return {
      formatVersion: 1,
      contract: this.contract,
      transaction: {
        hash: this.options.hash,
        source: this.options.source ?? 'recorded',
        ledger: this.options.ledger ?? 1_000_000,
        sourceAccount: 'GDEBUGGERFIXTURE000000000000000000000000000000000000000000',
      },
      detail: 'operation',
      totals,
      frames: this.frames,
      steps: this.steps,
    };
  }
}

const ID_HEX = 'a1'.repeat(32);

/**
 * A realistic single `log_event` interaction.
 *
 * Mirrors the contract's real access pattern: config and pause checks, allowlist
 * check, global and per-type limit checks, metadata validation, then the write
 * burst in `_append_event` and the `event_logged` publication. The delegation
 * through `Self::` helpers is what makes the call stack non-trivial.
 */
export function buildLogEventTrace(): TransactionTrace {
  const b = new TraceBuilder({ id: 'local', name: 'AuditLedger', version: '0.1.0', wasmHash: '00'.repeat(32) }, { hash: 'fixture-log-event' });

  const root = b.frame('log_event', {
    args: {
      submitter: 'GDEBUGGERFIXTURE000000000000000000000000000000000000000000',
      event_type: 'access_granted',
      metadata: 'base64:AQIDBAU=',
      category: 'auth',
      sub_event_type: 'none',
      force: false,
    },
  });
  const cfg = b.frame('load_config', { parentId: root });
  const pause = b.frame('ensure_not_paused', { parentId: root });
  const allow = b.frame('check_submitter_allowed', { parentId: root });
  const limit = b.frame('check_global_limit', { parentId: root });
  const meta = b.frame('validate_metadata', { parentId: root });
  const append = b.frame('append_event', { parentId: root });
  const emit = b.frame('emit_event_logged', { parentId: root });

  b.plain(root, 'log_event', 'entry', 40, { line: 412, locals: { force: false } });
  b.call(root, 'log_event', 'load_config', {}, 30, { line: 418 });

  b.read(cfg, 'load_config', 'instance', 'Config', { global_max_logs: 10_000, max_metadata_bytes: 1024 }, 90, { line: 208 });
  b.closeFrame(cfg, { global_max_logs: 10_000, max_metadata_bytes: 1024 });

  b.read(pause, 'ensure_not_paused', 'instance', 'Paused', false, 60, { line: 96 });
  b.plain(pause, 'ensure_not_paused', 'branch', 20, { line: 98, note: 'paused == false, continuing' });
  b.closeFrame(pause, undefined);

  b.read(allow, 'check_submitter_allowed', 'instance', 'AllowlistMode', false, 55, { line: 240 });
  b.plain(allow, 'check_submitter_allowed', 'branch', 15, { line: 242, note: 'allowlist disabled, skipping blocklist lookup' });
  b.closeFrame(allow, true);

  b.read(limit, 'check_global_limit', 'instance', 'RuntimeState', { total_events: 41, last_sequence: 41 }, 75, { line: 300 });
  b.read(limit, 'check_global_limit', 'persistent', 'EventTypeIndices(access_granted)', [7, 19, 22], 70, { line: 312 });
  b.plain(limit, 'check_global_limit', 'branch', 25, { line: 318, note: '43 < 10000' });
  b.closeFrame(limit, true);

  b.read(meta, 'validate_metadata', 'instance', 'MetadataSchema(access_granted)', { pattern: null, max_keys: 16 }, 65, { line: 352 });
  b.plain(meta, 'validate_metadata', 'call', 120, { line: 360, locals: { metadata_len: 5 } });
  b.closeFrame(meta, true);

  b.call(root, 'log_event', 'append_event', { id: `0x${ID_HEX}` }, 35, { line: 428 });
  b.read(append, 'append_event', 'instance', 'RuntimeState', { total_events: 41, last_sequence: 41 }, 80, { line: 520 });
  b.write(append, 'append_event', 'persistent', `EventData(0x${ID_HEX})`, { submitter: 'GDEBUGGER…', event_type: 'access_granted', sequence: 42 }, 320, {
    line: 528,
    previous: undefined,
  });
  b.write(append, 'append_event', 'persistent', 'EventOrder(42)', `0x${ID_HEX}`, 180, { line: 536 });
  b.write(append, 'append_event', 'persistent', 'EventTypeIndices(access_granted)', [7, 19, 22, 42], 210, {
    line: 544,
    previous: [7, 19, 22],
  });
  b.write(append, 'append_event', 'persistent', 'SubmitterEventIndices(GDEBUGGER…)', [7, 19, 22, 42], 205, { line: 552 });
  b.write(append, 'append_event', 'persistent', 'SubmitterEventCount(GDEBUGGER…)', 4, 150, { line: 560, previous: 3 });
  b.write(append, 'append_event', 'instance', 'RuntimeState', { total_events: 42, last_sequence: 42 }, 240, {
    line: 568,
    previous: { total_events: 41, last_sequence: 41 },
  });
  b.closeFrame(append, `0x${ID_HEX}`);

  b.call(root, 'log_event', 'emit_event_logged', {}, 30, { line: 434 });
  b.event(
    emit,
    'emit_event_logged',
    {
      name: 'event_logged',
      topics: ['access_granted', 42],
      data: [`0x${ID_HEX}`],
      sequence: 42,
    },
    410,
    { line: 604 },
  );
  b.closeFrame(emit, undefined);

  b.plain(root, 'log_event', 'return', 45, { line: 440, locals: { event_id: `0x${ID_HEX}` } });
  b.closeFrame(root, `0x${ID_HEX}`);

  return b.build();
}

/** `log_events` batch of three, with a second submitters' index updated. */
export function buildBatchTrace(): TransactionTrace {
  const b = new TraceBuilder({ id: 'local', name: 'AuditLedger', version: '0.1.0' }, { hash: 'fixture-batch' });
  const root = b.frame('log_events', { args: { events: ['<3 items>'] } });
  b.plain(root, 'log_events', 'entry', 55, { line: 452 });
  b.read(root, 'log_events', 'instance', 'RuntimeState', { total_events: 41, last_sequence: 41 }, 80, { line: 460 });

  const ids = ['b1'.repeat(32), 'b2'.repeat(32), 'b3'.repeat(32)];
  ids.forEach((id, i) => {
    const child = b.frame('log_event', { parentId: root, args: { event_type: ['access_granted', 'data_export', 'privilege_change'][i] } });
    b.read(child, 'log_event', 'instance', 'Paused', false, 60, { line: 96 });
    b.write(child, 'log_event', 'persistent', `EventData(0x${id})`, { sequence: 42 + i }, 320, { line: 528 });
    b.write(child, 'log_event', 'persistent', `EventOrder(${42 + i})`, `0x${id}`, 180, { line: 536 });
    b.event(child, 'log_event', { name: 'event_logged', topics: [['access_granted', 'data_export', 'privilege_change'][i], 42 + i], data: [`0x${id}`], sequence: 42 + i }, 410, { line: 604 });
    b.closeFrame(child, `0x${id}`);
  });

  b.write(root, 'log_events', 'instance', 'RuntimeState', { total_events: 44, last_sequence: 44 }, 240, {
    line: 470,
    previous: { total_events: 41, last_sequence: 41 },
  });
  b.plain(root, 'log_events', 'return', 50, { line: 476 });
  b.closeFrame(root, ['0xb1'.repeat(32), '0xb2'.repeat(32), '0xb3'.repeat(32)]);
  return b.build();
}

/** A failing `log_event` that panics on the global cap: shows error stops. */
export function buildPanicTrace(): TransactionTrace {
  const b = new TraceBuilder({ id: 'local', name: 'AuditLedger', version: '0.1.0' }, { hash: 'fixture-panic' });
  const root = b.frame('log_event', { args: { event_type: 'access_granted', force: false } });
  const limit = b.frame('check_global_limit', { parentId: root });
  b.plain(root, 'log_event', 'entry', 40, { line: 412 });
  b.read(limit, 'check_global_limit', 'instance', 'RuntimeState', { total_events: 10_000, last_sequence: 10_000 }, 75, { line: 300 });
  b.plain(limit, 'check_global_limit', 'branch', 25, { line: 316, note: 'total_events == global_max_logs' });
  b.panic(limit, 'check_global_limit', 'GlobalMaxLogsReached', 2004, 60, { line: 316 });
  b.failFrame(limit, 'GlobalMaxLogsReached', 2004);
  b.closeFrame(limit, undefined);
  b.panic(root, 'log_event', 'GlobalMaxLogsReached', 2004, 30, { line: 420 });
  b.failFrame(root, 'GlobalMaxLogsReached', 2004);
  b.closeFrame(root, undefined);
  return b.build();
}

/** `get_event`, which writes a periodic TTL cleanup: shows a value-returning
 *  mutating call, the case the SDK's `needsTransaction` exists for. */
export function buildReadWithWritebackTrace(): TransactionTrace {
  const b = new TraceBuilder({ id: 'local', name: 'AuditLedger', version: '0.1.0' }, { hash: 'fixture-get-event' });
  const root = b.frame('get_event', { args: { id: `0x${ID_HEX}` } });
  const sweep = b.frame('maybe_sweep_expired', { parentId: root });
  b.read(root, 'get_event', 'persistent', `EventData(0x${ID_HEX})`, { submitter: 'GDEBUGGER…', event_type: 'access_granted', sequence: 42 }, 90, { line: 620 });
  b.call(root, 'get_event', 'maybe_sweep_expired', {}, 30, { line: 624 });
  b.read(sweep, 'maybe_sweep_expired', 'instance', 'RuntimeState', { total_events: 42, last_sweep: 100 }, 70, { line: 640 });
  b.plain(sweep, 'maybe_sweep_expired', 'branch', 20, { line: 644, note: 'now > last_sweep + SWEEP_INTERVAL, sweeping' });
  b.write(sweep, 'maybe_sweep_expired', 'instance', 'RuntimeState', { total_events: 42, last_sweep: 1_100 }, 130, {
    line: 648,
    previous: { total_events: 42, last_sweep: 100 },
  });
  b.closeFrame(sweep, 1);
  b.plain(root, 'get_event', 'return', 40, { line: 630 });
  b.closeFrame(root, { submitter: 'GDEBUGGER…', event_type: 'access_granted', sequence: 42 });
  return b.build();
}

/** A trace whose recorded totals deliberately disagree with its steps. */
export function buildInconsistentTrace(): TransactionTrace {
  const trace = buildLogEventTrace();
  trace.totals.instructions += 5_000;
  trace.totals.readBytes += 3;
  return trace;
}

export const scenarioBuilders: Record<string, () => TransactionTrace> = {
  'log-event': buildLogEventTrace,
  batch: buildBatchTrace,
  panic: buildPanicTrace,
  'read-writeback': buildReadWithWritebackTrace,
  inconsistent: buildInconsistentTrace,
};
