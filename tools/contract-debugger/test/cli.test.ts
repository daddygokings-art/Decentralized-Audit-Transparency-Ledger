import { mkdtempSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest';
import { main } from '../src/cli';
import { buildInconsistentTrace, buildLogEventTrace, buildPanicTrace, serializeTrace } from '../src/index';

let dir: string;
let logEvent: string;
let panic: string;
let inconsistent: string;

let stdout: string;
let stderr: string;
let outSpy: { mockRestore: () => void };
let errSpy: { mockRestore: () => void };

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), 'audit-ledger-debugger-cli-'));
  logEvent = join(dir, 'log-event.json');
  panic = join(dir, 'panic.json');
  inconsistent = join(dir, 'inconsistent.json');
  writeFileSync(logEvent, serializeTrace(buildLogEventTrace()));
  writeFileSync(panic, serializeTrace(buildPanicTrace()));
  writeFileSync(inconsistent, serializeTrace(buildInconsistentTrace()));
  stdout = '';
  stderr = '';
  outSpy = vi.spyOn(process.stdout, 'write').mockImplementation((chunk: unknown) => {
    stdout += String(chunk);
    return true;
  });
  errSpy = vi.spyOn(process.stderr, 'write').mockImplementation((chunk: unknown) => {
    stderr += String(chunk);
    return true;
  });
  process.exitCode = undefined;
});

afterEach(() => {
  outSpy.mockRestore();
  errSpy.mockRestore();
  process.exitCode = undefined;
});

const run = (...argv: string[]): { stdout: string; stderr: string; code: number | string | undefined } => {
  stdout = '';
  stderr = '';
  main(argv);
  return { stdout, stderr, code: process.exitCode };
};

describe('cli help', () => {
  it('prints usage with no arguments', () => {
    const r = run();
    expect(r.stdout).toMatch(/audit-ledger-debug/);
    expect(r.stdout).toMatch(/USAGE/);
  });

  it('lists every command', () => {
    const r = run('help');
    for (const cmd of ['inspect', 'steps', 'storage', 'gas', 'stack', 'events', 'step', 'watch', 'replay', 'verify', 'normalize', 'scenario', 'dap', 'repl']) {
      expect(r.stdout).toContain(cmd);
    }
  });

  it('rejects an unknown command with exit 1', () => {
    const r = run('frobnicate');
    expect(r.stderr).toMatch(/unknown command/);
    expect(r.code).toBe(1);
  });

  it('reports a missing trace file', () => {
    const r = run('inspect', join(dir, 'nope.json'));
    expect(r.stderr).toMatch(/trace not found/);
    expect(r.code).toBe(1);
  });
});

describe('cli inspect', () => {
  it('summarises the contract, totals and step count', () => {
    const trace = buildLogEventTrace();
    const r = run('inspect', logEvent);
    expect(r.stdout).toContain('AuditLedger v0.1.0');
    expect(r.stdout).toContain(`${trace.steps.length} across ${trace.frames.length} frame(s)`);
    expect(r.stdout).toContain(trace.totals.instructions.toLocaleString());
    expect(r.stdout).toContain('accounting   consistent');
  });

  it('flags a gas mismatch', () => {
    const r = run('inspect', inconsistent);
    expect(r.stdout).toMatch(/MISMATCH/);
  });
});

describe('cli steps', () => {
  it('prints a timeline of every step', () => {
    const r = run('steps', logEvent);
    expect(r.stdout).toContain('function');
    expect(r.stdout).toContain('log_event');
    expect(r.stdout).toContain('append_event');
  });

  it('honours --from and --to', () => {
    const r = run('steps', logEvent, '--from', '0', '--to', '2');
    const lines = r.stdout.trim().split('\n');
    // header + rule + three steps
    expect(lines).toHaveLength(5);
  });

  it('filters to storage operations', () => {
    const r = run('steps', logEvent, '--storage');
    expect(r.stdout).toContain('storage.set');
    expect(r.stdout).not.toContain('event.publish');
  });

  it('filters to events', () => {
    const r = run('steps', logEvent, '--events');
    expect(r.stdout).toContain('event event_logged');
    expect(r.stdout).not.toContain('storage.set');
  });

  it('shows the panic with its error code', () => {
    const r = run('steps', panic);
    expect(r.stdout).toContain('panic GlobalMaxLogsReached');
  });
});

describe('cli storage', () => {
  it('shows the final state', () => {
    const r = run('storage', logEvent);
    expect(r.stdout).toContain('instance:RuntimeState');
    expect(r.stdout).toContain('total_events');
  });

  it('shows the state at a step', () => {
    const trace = buildLogEventTrace();
    const write = trace.steps.findIndex((s) => s.storage?.op === 'write' && s.storage.key === 'RuntimeState');
    const before = run('storage', logEvent, '--at', String(write - 1));
    const after = run('storage', logEvent, '--at', String(write));
    expect(before.stdout).toContain('41');
    expect(after.stdout).toContain('42');
  });

  it('filters by scope', () => {
    const r = run('storage', logEvent, '--scope', 'persistent');
    expect(r.stdout).toContain('persistent:EventOrder(42)');
    expect(r.stdout).not.toContain('instance:Paused');
  });

  it('diffs a range', () => {
    const trace = buildLogEventTrace();
    const write = trace.steps.findIndex((s) => s.storage?.op === 'write' && s.storage.key === 'RuntimeState');
    const read = trace.steps.findIndex((s) => s.storage?.op === 'read' && s.storage.key === 'RuntimeState');
    const r = run('storage', logEvent, '--diff', `${read}:${write}`);
    expect(r.stdout).toContain('updated');
    expect(r.stdout).toContain('RuntimeState');
  });
});

describe('cli gas', () => {
  it('breaks down by function', () => {
    const r = run('gas', logEvent);
    expect(r.stdout).toContain('function');
    expect(r.stdout).toMatch(/total [\d,]+ instructions/);
  });

  it('breaks down by operation', () => {
    const r = run('gas', logEvent, '--by', 'op');
    expect(r.stdout).toContain('storage.set');
    expect(r.stdout).toContain('event.publish');
  });

  it('reports a consistent trace as passing verify', () => {
    const r = run('gas', logEvent, '--verify');
    expect(r.stdout).toContain('all checks passed');
    expect(r.code).toBeUndefined();
  });

  it('exits non-zero when totals disagree with the steps', () => {
    const r = run('gas', inconsistent, '--verify');
    expect(r.stdout).toContain('MISMATCH');
    expect(r.code).toBe(1);
  });
});

describe('cli stack and events', () => {
  it('shows the call stack at a step', () => {
    const trace = buildLogEventTrace();
    const index = trace.steps.findIndex((s) => s.function === 'load_config');
    const r = run('stack', logEvent, '--at', String(index));
    // Innermost first, as a debugger's frame list shows it.
    expect(r.stdout).toContain('#0 load_config');
    expect(r.stdout).toContain('#1 log_event');
    expect(r.stdout.indexOf('#0 load_config')).toBeLessThan(r.stdout.indexOf('#1 log_event'));
  });

  it('lists the event log with a per-type summary', () => {
    const r = run('events', logEvent);
    expect(r.stdout).toContain('event_logged');
    expect(r.stdout).toContain('by event type');
  });

  it('limits events to those emitted up to a step', () => {
    const r = run('events', logEvent, '--at', '0');
    expect(r.stdout).toMatch(/no events/);
  });
});

describe('cli step', () => {
  it('reports the full state at a step', () => {
    const r = run('step', logEvent, '--to', '0');
    expect(r.stdout).toContain('stopped at step 0');
    expect(r.stdout).toContain('gas');
    // Nothing has been read yet, so there is no storage state to show.
    expect(r.stdout).not.toContain('storage state');

    const later = run('step', logEvent, '--to', '13');
    expect(later.stdout).toContain('storage state');
    expect(later.stdout).toContain('instance:RuntimeState');
  });

  it('emits machine-readable json', () => {
    const r = run('step', logEvent, '--to', '3', '--json');
    const payload = JSON.parse(r.stdout) as { stepIndex: number; stack: string[]; storageChanges: unknown[] };
    expect(payload.stepIndex).toBe(3);
    expect(payload.stack).toContain('log_event');
    expect(Array.isArray(payload.storageChanges)).toBe(true);
  });

  it('reports a panic stop', () => {
    const panicAt = buildPanicTrace().steps.findIndex((s) => s.error);
    const r = run('step', panic, '--to', String(panicAt));
    expect(r.stdout).toContain('panic');
    expect(r.stdout).toContain('GlobalMaxLogsReached');
    expect(r.stdout).toContain('#2004');
  });
});

describe('cli watch', () => {
  it('evaluates an expression', () => {
    const r = run('watch', logEvent, '--expr', 'contract.name');
    expect(r.stdout.trim()).toBe('"AuditLedger"');
  });

  it('exits non-zero on a bad expression', () => {
    const r = run('watch', logEvent, '--expr', '1 +');
    expect(r.stderr).toMatch(/error:/);
    expect(r.code).toBe(1);
  });

  it('requires an expression', () => {
    const r = run('watch', logEvent);
    expect(r.stderr).toMatch(/--expr/);
  });
});

describe('cli replay', () => {
  it('runs to the end of a trace with no breakpoints', () => {
    const r = run('replay', logEvent);
    expect(r.stdout).toMatch(/reached end of trace|steps/);
    expect(r.stdout).toContain('event_logged');
  });

  it('stops at a function breakpoint', () => {
    const r = run('replay', logEvent, '--break', 'append_event');
    expect(r.stdout).toMatch(/breakpoint \d+ \(append_event\)/);
  });

  it('stops at a narrowed operation breakpoint', () => {
    const r = run('replay', logEvent, '--break', 'append_event:storage.set');
    expect(r.stdout).toMatch(/append_event/);
  });

  it('exits non-zero when accounting is inconsistent', () => {
    const r = run('replay', inconsistent);
    expect(r.stderr).toMatch(/accounting mismatch/);
    expect(r.code).toBe(1);
  });
});

describe('cli verify', () => {
  it('reports per-frame and per-dimension checks', () => {
    const r = run('verify', logEvent);
    expect(r.stdout).toContain('totals cross-check');
    expect(r.stdout).toContain('per-frame declared vs observed');
    expect(r.stdout).toContain('all checks passed');
  });

  it('lists the mismatching dimension on a bad trace', () => {
    const r = run('verify', inconsistent);
    expect(r.stdout).toContain('MISMATCH');
    expect(r.stdout).toMatch(/check\(s\) failed/);
  });
});

describe('cli scenario', () => {
  it('writes each built-in scenario', () => {
    for (const name of ['log-event', 'batch', 'panic', 'read-writeback']) {
      const dest = join(dir, `${name}.json`);
      const r = run('scenario', name, '-o', dest);
      expect(r.stdout).toContain('wrote');
      const r2 = run('inspect', dest);
      expect(r2.stdout).toContain('consistent');
    }
  });

  it('rejects an unknown scenario', () => {
    const r = run('scenario', 'nope');
    expect(r.stderr).toMatch(/unknown scenario/);
    expect(r.code).toBe(1);
  });
});

describe('cli normalize', () => {
  const rpc = {
    jsonrpc: '2.0',
    id: 1,
    result: {
      ledger: 1_234_567,
      transactionData: {
        minResourceFee: '150000',
        cost: { cpuInsns: '884210', memBytes: '65536' },
        footprint: { read: [{ x: 1 }], readWrite: [{ y: 1 }, { z: 1 }] },
      },
      resultMeta: {
        events: [
          {
            type: 'AAAAAQ==',
            body: ['ZGVjb2RlZA==', 'dgAAAAM=', 'AAAAAQ=='],
            inSuccessfulContractCall: true,
          },
        ],
      },
    },
  };

  it('normalizes RPC output into a summary trace', () => {
    const input = join(dir, 'rpc.json');
    writeFileSync(input, JSON.stringify(rpc));
    const r = run('normalize', input, '--contract-id', 'CABC', '--contract-name', 'AuditLedger');
    const trace = JSON.parse(r.stdout) as { detail: string; totals: { instructions: number; fee: number; events: number }; steps: unknown[] };
    expect(trace.detail).toBe('summary');
    expect(trace.totals.instructions).toBe(884_210);
    expect(trace.totals.fee).toBe(150_000);
    expect(trace.totals.events).toBe(1);
    expect(trace.steps.length).toBeGreaterThan(0);
  });

  it('warns that summary traces cannot be stepped through', () => {
    const input = join(dir, 'rpc.json');
    const dest = join(dir, 'normalized.json');
    writeFileSync(input, JSON.stringify(rpc));
    const r = run('normalize', input, '--contract-id', 'CABC', '--contract-name', 'AuditLedger', '-o', dest);
    expect(r.stdout).toMatch(/step-through needs a recorded operation-level trace/);
  });

  it('reports an RPC error', () => {
    const input = join(dir, 'rpc-error.json');
    writeFileSync(input, JSON.stringify({ error: { code: -32602, message: 'bad request' } }));
    const r = run('normalize', input, '--contract-id', 'CABC', '--contract-name', 'X');
    expect(r.stderr).toMatch(/RPC error: bad request/);
    expect(r.code).toBe(1);
  });

  it('defaults the contract identity when none is given', () => {
    const input = join(dir, 'rpc.json');
    writeFileSync(input, JSON.stringify(rpc));
    const r = run('normalize', input);
    const trace = JSON.parse(r.stdout) as { contract: { id: string; name: string } };
    expect(trace.contract).toEqual({ id: 'local', name: 'AuditLedger', version: '0.1.0' });
  });
});
