import { PassThrough } from 'stream';
import { describe, expect, it } from 'vitest';
import {
  DapServer,
  MessageDecoder,
  buildLogEventTrace,
  buildPanicTrace,
  collectResponses,
  encodeMessage,
  nextSeq,
  resetSeq,
  serializeTrace,
} from '../src/index';
import type { DapMessage } from '../src/index';

function harness(initial?: DapMessage) {
  resetSeq();
  const input = new PassThrough();
  const output = new PassThrough();
  const chunks: Buffer[] = [];
  output.on('data', (c: Buffer) => chunks.push(c));
  const server = new DapServer({ input, output });
  const decoder = new MessageDecoder();
  const seen: DapMessage[] = [];
  const send = (m: DapMessage): void => {
    input.write(encodeMessage(m));
  };
  const request = (command: string, args?: Record<string, unknown>): number => {
    const seq = nextSeq();
    send({ seq, type: 'request', command, arguments: args });
    return seq;
  };
  const pump = (): void => {
    for (const chunk of chunks.splice(0)) seen.push(...decoder.push(chunk));
  };
  if (initial) send(initial);
  return { server, send, request, pump, seen, responses: () => collectResponses(seen), input, output };
}

const tracePath = (): string => {
  const path = `${process.env.TMPDIR ?? '/tmp'}/audit-ledger-dap-test.json`;
  require('fs').writeFileSync(path, serializeTrace(buildLogEventTrace()));
  return path;
};

describe('DAP framing', () => {
  it('round-trips a message with a Content-Length header', () => {
    const decoder = new MessageDecoder();
    const msg: DapMessage = { seq: 1, type: 'request', command: 'initialize' };
    const out = decoder.push(encodeMessage(msg));
    expect(out).toEqual([msg]);
  });

  it('reassembles a message split across chunks', () => {
    const decoder = new MessageDecoder();
    const buf = encodeMessage({ seq: 7, type: 'event', event: 'stopped', body: { reason: 'step' } });
    for (let i = 0; i < buf.length; i += 1) {
      decoder.push(buf.subarray(i, i + 1));
    }
    expect(decoder.pending).toBe(0);
  });

  it('handles several messages in one chunk', () => {
    const decoder = new MessageDecoder();
    const buf = Buffer.concat([encodeMessage({ seq: 1, type: 'request', command: 'a' }), encodeMessage({ seq: 2, type: 'request', command: 'b' })]);
    expect(decoder.push(buf).map((m) => m.seq)).toEqual([1, 2]);
  });

  it('accepts a lowercase header', () => {
    const decoder = new MessageDecoder();
    const body = JSON.stringify({ seq: 3, type: 'request', command: 'threads' });
    decoder.push(Buffer.from(`content-length: ${body.length}\r\n\r\n${body}`, 'utf8'));
    expect(decoder.push(Buffer.alloc(0))).toEqual([]);
  });

  it('skips an unparseable payload rather than dying', () => {
    const decoder = new MessageDecoder();
    const body = '{not json';
    const out = decoder.push(Buffer.from(`Content-Length: ${body.length}\r\n\r\n${body}`, 'utf8'));
    expect(out).toEqual([]);
  });
});

describe('DAP handshake', () => {
  it('answers initialize with the capabilities the debugger supports', () => {
    const h = harness();
    const seq = h.request('initialize');
    h.pump();
    const res = h.responses().get(seq)!;
    expect(res.success).toBe(true);
    expect(res.body?.supportsConfigurationDoneRequest).toBe(true);
    expect(res.body?.supportsFunctionBreakpoints).toBe(true);
    expect(res.body?.supportsEvaluateForHovers).toBe(true);
    expect(res.body?.supportsStepBack).toBe(true);
  });

  it('refuses a request before launch', () => {
    const h = harness();
    const seq = h.request('stackTrace');
    h.pump();
    const res = h.responses().get(seq)!;
    expect(res.success).toBe(false);
    expect(res.message).toMatch(/not launched/);
  });

  it('fails launch with no trace path', () => {
    const h = harness();
    const seq = h.request('launch', {});
    h.pump();
    expect(h.responses().get(seq)!.success).toBe(false);
  });

  it('launches with an explicit trace path', () => {
    const h = harness();
    const seq = h.request('launch', { tracePath: tracePath() });
    h.pump();
    expect(h.responses().get(seq)!.body?.started).toBe(true);
    expect(h.server.getSession()).not.toBeNull();
  });
});

describe('DAP breakpoints and stepping', () => {
  function launched(breakpoints?: Array<{ name: string; line?: number; condition?: string }>) {
    const h = harness();
    h.request('initialize');
    h.request('launch', { tracePath: tracePath() });
    h.pump();
    if (breakpoints) {
      h.request('setFunctionBreakpoints', { breakpoints });
      h.pump();
    }
    h.request('configurationDone');
    h.pump();
    return h;
  }

  it('verifies a breakpoint on a function the trace calls', () => {
    const h = launched([{ name: 'append_event' }]);
    const seq = nextSeq();
    h.send({ seq, type: 'request', command: 'setFunctionBreakpoints', arguments: { breakpoints: [{ name: 'append_event' }] } });
    h.pump();
    const res = h.responses().get(seq)!;
    expect((res.body?.breakpoints as Array<{ verified: boolean }>)[0].verified).toBe(true);
  });

  it('reports an unverified breakpoint with a reason', () => {
    const h = launched();
    const seq = nextSeq();
    h.send({ seq, type: 'request', command: 'setFunctionBreakpoints', arguments: { breakpoints: [{ name: 'nope' }] } });
    h.pump();
    const bp = (h.responses().get(seq)!.body?.breakpoints as Array<{ verified: boolean; message?: string }>)[0];
    expect(bp.verified).toBe(false);
    expect(bp.message).toMatch(/nope/);
  });

  it('maps a source-line breakpoint onto the function that owns the line', () => {
    const trace = buildLogEventTrace();
    const line = trace.steps.find((s) => s.function === 'append_event')!.location!.line!;
    const h = launched();
    const seq = nextSeq();
    h.send({ seq, type: 'request', command: 'setBreakpoints', arguments: { source: { path: 'src/lib.rs' }, breakpoints: [{ line }] } });
    h.pump();
    const bp = (h.responses().get(seq)!.body?.breakpoints as Array<{ verified: boolean }>)[0];
    expect(bp.verified).toBe(true);
  });

  it('emits a stopped event on continue and lands on the breakpoint', () => {
    const h = launched([{ name: 'validate_metadata' }]);
    const seq = h.request('continue');
    h.pump();
    expect(h.responses().get(seq)!.body?.allThreadsContinued).toBe(true);
    const stopped = h.seen.filter((m) => m.type === 'event' && m.event === 'stopped');
    expect(stopped.length).toBeGreaterThan(0);
    expect(h.server.getSession()!.currentStep()!.function).toBe('validate_metadata');
  });

  it('steps with next, stepIn and stepOut', () => {
    const h = launched();
    const atCall = buildLogEventTrace().steps.findIndex((s) => s.op === 'call' && s.call?.target === 'load_config');
    h.server.getSession()!.goto(atCall);
    h.request('next');
    h.pump();
    expect(h.server.getSession()!.currentStep()!.depth).toBe(0);

    h.server.getSession()!.goto(atCall);
    h.request('stepIn');
    h.pump();
    expect(h.server.getSession()!.currentStep()!.function).toBe('load_config');

    h.request('stepOut');
    h.pump();
    expect(h.server.getSession()!.currentStep()!.function).toBe('log_event');
  });

  it('terminates at the end of the trace', () => {
    const h = launched();
    h.request('continue');
    h.pump();
    expect(h.seen.some((m) => m.type === 'event' && m.event === 'terminated')).toBe(true);
  });

  it('stops with an exception reason on a panicking trace', () => {
    const path = `${process.env.TMPDIR ?? '/tmp'}/audit-ledger-dap-panic.json`;
    require('fs').writeFileSync(path, serializeTrace(buildPanicTrace()));
    const h = harness();
    h.request('launch', { tracePath: path });
    h.pump();
    h.request('configurationDone');
    h.request('continue');
    h.pump();
    const stopped = h.seen.find((m) => m.type === 'event' && m.event === 'stopped');
    expect(stopped?.body?.reason).toBe('exception');
    expect(stopped?.body?.description).toBe('GlobalMaxLogsReached');
  });

  it('answers an unknown request with an empty success', () => {
    const h = launched();
    const seq = h.request('gotoTargets');
    h.pump();
    const res = h.responses().get(seq)!;
    expect(res.success).toBe(true);
    expect(res.body).toEqual({});
  });
});

describe('DAP inspection', () => {
  function running() {
    const h = harness();
    h.request('launch', { tracePath: tracePath() });
    h.pump();
    h.request('configurationDone');
    h.server.getSession()!.goto(3);
    h.pump();
    return h;
  }

  it('reports one thread', () => {
    const h = running();
    const seq = h.request('threads');
    h.pump();
    expect(h.responses().get(seq)!.body?.threads).toEqual([{ id: 1, name: 'transaction' }]);
  });

  it('returns a stack with source paths', () => {
    const h = running();
    const seq = h.request('stackTrace');
    h.pump();
    const frames = h.responses().get(seq)!.body?.stackFrames as Array<{ name: string; source?: { path: string } }>;
    expect(frames.length).toBeGreaterThan(0);
    expect(frames[0].source?.path).toBe('src/lib.rs');
  });

  it('returns scopes and their variables', () => {
    const h = running();
    let seq = h.request('scopes');
    h.pump();
    const scopes = h.responses().get(seq)!.body?.scopes as Array<{ name: string; variablesReference: number }>;
    expect(scopes.map((s) => s.name)).toContain('Storage');

    seq = h.request('variables', { variablesReference: scopes[2].variablesReference });
    h.pump();
    const vars = h.responses().get(seq)!.body?.variables as Array<{ name: string; value: string }>;
    expect(vars.some((v) => v.name === 'instance:Paused')).toBe(true);
  });

  it('evaluates a watch expression', () => {
    const h = running();
    const seq = h.request('evaluate', { expression: 'contract.name' });
    h.pump();
    const res = h.responses().get(seq)!;
    expect(res.body?.result).toBe('"AuditLedger"');
  });

  it('fails an invalid watch expression with a reason', () => {
    const h = running();
    const seq = h.request('evaluate', { expression: '1 +' });
    h.pump();
    const res = h.responses().get(seq)!;
    expect(res.success).toBe(false);
    expect(res.message).toMatch(/offset/);
  });

  it('disconnects and terminates', () => {
    const h = running();
    const seq = h.request('disconnect');
    h.pump();
    expect(h.responses().get(seq)!.success).toBe(true);
    expect(h.seen.some((m) => m.type === 'event' && m.event === 'terminated')).toBe(true);
  });
});
