#!/usr/bin/env node
/**
 * End-to-end check of the debug adapter, run by CI.
 *
 * Spawns the built CLI in `dap` mode and drives a real
 * initialize -> launch -> configurationDone -> continue -> stackTrace ->
 * disconnect exchange over stdio.  Each request waits for its own response, so
 * the check is deterministic rather than racing a timer.
 *
 * Usage: node test/dap-exchange.check.js
 * Requires: `npm run build` to have produced dist/cli.js.
 */

const { spawn } = require('child_process');
const { mkdtempSync } = require('fs');
const { tmpdir } = require('os');
const { join, resolve } = require('path');

const root = resolve(__dirname, '..');
const cli = join(root, 'dist', 'cli.js');
const TIMEOUT_MS = 20_000;

/** Client for one DAP session over the CLI's stdio. */
class DapClient {
  constructor() {
    this.proc = spawn(process.execPath, [cli, 'dap'], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.pending = new Map();
    this.events = [];
    this.stderr = '';
    this.buffer = Buffer.alloc(0);
    this.seq = 0;
    this.proc.stdout.on('data', (chunk) => this.consume(chunk));
    this.proc.stderr.on('data', (chunk) => {
      this.stderr += String(chunk);
    });
  }

  consume(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const end = this.buffer.indexOf('\r\n\r\n');
      if (end === -1) return;
      const header = this.buffer.subarray(0, end).toString();
      const match = /content-length:\s*(\d+)/i.exec(header);
      if (!match) {
        this.buffer = this.buffer.subarray(end + 4);
        continue;
      }
      const length = Number(match[1]);
      if (this.buffer.length < end + 4 + length) return;
      const message = JSON.parse(this.buffer.subarray(end + 4, end + 4 + length).toString());
      this.buffer = this.buffer.subarray(end + 4 + length);
      this.dispatch(message);
    }
  }

  dispatch(message) {
    if (message.type === 'event') {
      this.events.push(message.event);
      return;
    }
    const waiter = this.pending.get(message.request_seq);
    if (waiter) {
      this.pending.delete(message.request_seq);
      waiter(message);
    }
  }

  request(command, args) {
    this.seq += 1;
    const seq = this.seq;
    const body = JSON.stringify({ seq, type: 'request', command, arguments: args });
    return new Promise((resolvePromise, rejectPromise) => {
      const timer = setTimeout(() => {
        this.pending.delete(seq);
        rejectPromise(new Error(`timed out waiting for a response to ${command}`));
      }, TIMEOUT_MS);
      this.pending.set(seq, (message) => {
        clearTimeout(timer);
        resolvePromise(message);
      });
      this.proc.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
    });
  }

  kill() {
    this.proc.kill();
  }
}

function assertOk(response, command, problems) {
  if (!response || response.success !== true) {
    problems.push(`${command} failed: ${response ? response.message : 'no response'}`);
  }
  return response;
}

async function main() {
  const dir = mkdtempSync(join(tmpdir(), 'audit-ledger-dap-check-'));
  const trace = join(dir, 'log-event.json');
  const scenario = spawn(process.execPath, [cli, 'scenario', 'log-event', '-o', trace], { stdio: 'ignore' });
  await new Promise((res, rej) => {
    scenario.on('close', (code) => (code === 0 ? res() : rej(new Error(`scenario failed with ${code}`))));
  });

  const client = new DapClient();
  const problems = [];
  try {
    assertOk(await client.request('initialize'), 'initialize', problems);
    assertOk(await client.request('launch', { tracePath: trace, stopOnEntry: false }), 'launch', problems);
    assertOk(await client.request('configurationDone'), 'configurationDone', problems);
    assertOk(await client.request('continue'), 'continue', problems);

    const stack = assertOk(await client.request('stackTrace'), 'stackTrace', problems);
    const frames = stack && stack.body && stack.body.stackFrames;
    if (!Array.isArray(frames) || frames.length === 0) {
      problems.push('stackTrace returned no frames');
    } else {
      const named = frames.map((f) => f.name);
      if (!named.includes('log_event')) {
        problems.push(`stackTrace frames were ${JSON.stringify(named)}, expected to include log_event`);
      }
      for (const frame of frames) {
        if (!frame.source || !frame.source.path) {
          problems.push(`frame ${frame.name} has no source path, so VS Code cannot map it to a file`);
          break;
        }
      }
    }

    // A breakpoint that cannot be hit must be reported, not silently ignored.
    const bp = assertOk(
      await client.request('setFunctionBreakpoints', { breakpoints: [{ name: 'definitely_not_called' }] }),
      'setFunctionBreakpoints',
      problems,
    );
    const [first] = (bp && bp.body && bp.body.breakpoints) || [];
    if (!first || first.verified !== false || !first.message) {
      problems.push('an unhittable breakpoint should be reported as unverified with a reason');
    }

    assertOk(await client.request('evaluate', { expression: 'contract.name' }), 'evaluate', problems);
    assertOk(await client.request('disconnect'), 'disconnect', problems);

    if (!client.events.includes('terminated')) problems.push(`never signalled terminated (saw ${client.events.join(', ')})`);
  } finally {
    client.kill();
  }

  if (client.stderr.trim()) {
    problems.push(`adapter wrote to stderr: ${client.stderr.trim()}`);
  }
  if (problems.length) {
    console.error(`DAP exchange failed:\n  ${problems.join('\n  ')}`);
    process.exit(1);
  }
  console.log('DAP exchange ok');
}

main().catch((e) => {
  console.error(e instanceof Error ? e.message : String(e));
  process.exit(1);
});
