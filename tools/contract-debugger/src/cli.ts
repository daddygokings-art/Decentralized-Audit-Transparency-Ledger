import { readFileSync } from 'fs';
import { DebugSession, Breakpoint } from './engine/session';
import { loadTrace, serializeTrace } from './trace/load';
import { checkGasConsistency, gasBreakdown } from './engine/gas';
import { eventBreakdown, eventLog } from './engine/events';
import { storageDiff, storageRows } from './engine/storage';
import { normalizeRpcResult, RpcResultEnvelope } from './trace/normalize';
import { TransactionTrace, StorageScope } from './trace/schema';
import { scenarioBuilders } from './fixtures/builder';
import { DapServer } from './dap/server';
import { VERSION } from './index';

interface Args {
  _: string[];
  [key: string]: string | boolean | string[] | undefined;
}

/** Positional argument, or undefined when the index is out of range. */
function positional(args: Args, i: number): string | undefined {
  const v = args._[i];
  return typeof v === 'string' ? v : undefined;
}

/** `-o` and friends, so the short forms in the usage text actually work. */
const SHORT_FLAGS: Record<string, string> = { o: 'o', h: 'help', v: 'version', t: 'trace', b: 'break', e: 'expr', a: 'at' };

function parseArgs(argv: string[]): Args {
  const out: Args = { _: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const short = /^-(o|h|v|t|b|e|a)$/.exec(a);
    if (short) {
      const key = SHORT_FLAGS[short[1]];
      if (argv[i + 1] !== undefined && !argv[i + 1].startsWith('-')) {
        out[key] = argv[i + 1];
        i += 1;
      } else {
        out[key] = true;
      }
      continue;
    }
    if (a.startsWith('--')) {
      const [key, inline] = a.slice(2).split('=', 2);
      if (inline !== undefined) {
        out[key] = inline;
      } else if (argv[i + 1] !== undefined && !argv[i + 1].startsWith('--')) {
        out[key] = argv[i + 1];
        i += 1;
      } else {
        out[key] = true;
      }
    } else {
      out._.push(a);
    }
  }
  return out;
}

type Option = string | boolean | string[] | undefined;

const num = (v: Option, fallback: number): number => {
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
};

const str = (v: Option, fallback = ''): string =>
  typeof v === 'string' ? v : fallback;

function flag(v: Option): boolean {
  return v === true || v === 'true';
}

const out = (line = ''): void => {
  process.stdout.write(`${line}\n`);
};

const err = (line: string): void => {
  process.stderr.write(`${line}\n`);
};

/** Render a value compactly for a table cell. */
function cell(v: unknown, width: number): string {
  const text = v === undefined ? '∅' : typeof v === 'string' ? v : JSON.stringify(v) ?? String(v);
  const one = text.replace(/\s+/g, ' ');
  return one.length > width ? `${one.slice(0, width - 1)}…` : one.padEnd(width);
}

function table(headers: string[], rows: string[][]): void {
  const widths = headers.map((h, i) => Math.max(h.length, ...rows.map((r) => (r[i] ?? '').length)));
  out(headers.map((h, i) => h.padEnd(widths[i])).join('  '));
  out(widths.map((w) => '─'.repeat(w)).join('  '));
  for (const r of rows) out(r.map((c, i) => c.padEnd(widths[i])).join('  '));
}

function pct(n: number): string {
  return `${(n * 100).toFixed(1)}%`;
}

function requireTrace(args: Args): TransactionTrace {
  const path = positional(args, 1);
  if (!path) throw new Error('expected a trace file as the first argument');
  const { trace, warnings } = loadTrace(path);
  for (const w of warnings) err(`warning: ${w}`);
  return trace;
}

function openSession(args: Args, trace: TransactionTrace): DebugSession {
  const session = new DebugSession({ trace, output: (l) => out(l) });
  const breaks = str(args.break);
  if (breaks) {
    const specs: Array<Omit<Breakpoint, 'id' | 'hitCount' | 'verified'>> = breaks.split(',').map((b) => {
      const [functionName, op] = b.split(':');
      return { functionName, op: op || undefined };
    });
    session.setBreakpoints(specs);
  }
  return session;
}

/** Position the cursor exactly; `-1` means "before the first step". */
function seekTo(session: DebugSession, at: number): void {
  session.goto(at);
}

const USAGE = `audit-ledger-debug ${VERSION} — transaction tracer and debugger for AuditLedger

USAGE
  audit-ledger-debug <command> <trace.json> [options]

COMMANDS
  inspect <trace>                     overview: contract, totals, step count
  steps <trace> [--from N] [--to N]   execution timeline (--storage --events --calls)
  storage <trace> [--at N]            storage state at a step
  storage <trace> --diff FROM:TO      storage changes between two steps
  gas <trace> [--by function|op|frame]  instruction breakdown
  gas <trace> --verify                cross-check recorded totals against the steps
  stack <trace> [--at N]              call stack at a step
  events <trace> [--at N]             event emission log
  step <trace> --to N [--json]        replay to a step, reporting the full state
  watch <trace> --expr EXPR [--at N]  evaluate a watch expression
  replay <trace> --break fn[:op]      run, stopping at breakpoints
  verify <trace>                      full accuracy report
  normalize <rpc.json> --contract-id C --contract-name N [-o out.json]
  scenario <name> [-o out.json]       write a built-in example trace
  dap [--trace path]                  run the Debug Adapter Protocol server
  repl <trace>                        interactive debugger prompt

GLOBAL OPTIONS
  -h, --help     show this help
  -v, --version  show the version
`;

function cmdInspect(args: Args): void {
  const trace = requireTrace(args);
  const t = trace.totals;
  out(`contract     ${trace.contract.name} v${trace.contract.version} (${trace.contract.id})`);
  out(`transaction  ${trace.transaction.hash ?? '(no hash)'} via ${trace.transaction.source}`);
  out(`detail       ${trace.detail}${trace.detail === 'summary' ? ' (no per-operation step data available)' : ''}`);
  out(`steps        ${trace.steps.length} across ${trace.frames.length} frame(s)`);
  out(`instructions ${t.instructions.toLocaleString()}`);
  out(`storage      ${t.storageReads} read(s), ${t.storageWrites} write(s)`);
  out(`io           ${t.readBytes} read byte(s), ${t.writeBytes} written byte(s)`);
  out(`events       ${t.events}`);
  out(`fee          ${t.fee} stroop(s)`);
  const gas = checkGasConsistency(trace);
  out(`accounting   ${gas.consistent ? 'consistent' : `MISMATCH (+${gas.delta} instructions vs totals)`}`);
}

function cmdSteps(args: Args): void {
  const trace = requireTrace(args);
  const from = num(args.from, 0);
  const to = num(args.to, trace.steps.length - 1);
  const wantStorage = flag(args.storage);
  const wantEvents = flag(args.events);
  const wantCalls = flag(args.calls);
  const filters: string[] = [];
  if (wantStorage) filters.push('storage');
  if (wantEvents) filters.push('events');
  if (wantCalls) filters.push('calls');

  const rows: string[][] = [];
  for (const s of trace.steps) {
    if (s.index < from || s.index > to) continue;
    const isStorage = Boolean(s.storage);
    const isEvent = Boolean(s.event);
    const isCall = s.op === 'call';
    if (filters.length > 0 && !((wantStorage && isStorage) || (wantEvents && isEvent) || (wantCalls && isCall))) continue;
    let detail = '';
    if (s.storage) detail = `${s.storage.op} ${s.storage.scope}:${s.storage.key}`;
    else if (s.event) detail = `event ${s.event.name}`;
    else if (s.call) detail = `call ${s.call.target}`;
    else if (s.error) detail = `panic ${s.error.name}`;
    rows.push([
      String(s.index),
      String(s.depth),
      s.function,
      s.op,
      cell(detail, 46),
      String(s.gas.instructions),
      s.location?.line !== undefined ? String(s.location.line) : '',
    ]);
  }
  table(['#', 'd', 'function', 'op', 'detail', 'gas', 'line'], rows);
}

function cmdStorage(args: Args): void {
  const trace = requireTrace(args);
  if (typeof args.diff === 'string') {
    const [from, to] = args.diff.split(':').map((n) => Number(n));
    const diffs = storageDiff(trace, Number.isFinite(from) ? from : 0, Number.isFinite(to) ? to : trace.steps.length - 1);
    if (diffs.length === 0) {
      out('no storage changes in range');
      return;
    }
    table(
      ['step', 'change', 'key', 'before', 'after'],
      diffs.map((d) => [String(d.at), d.kind, `${d.scope}:${d.key}`, cell(d.before, 26), cell(d.after, 26)]),
    );
    return;
  }
  const at = num(args.at, trace.steps.length - 1);
  const session = new DebugSession({ trace });
  seekTo(session, at);
  const scope = str(args.scope) as StorageScope | '';
  const rows = storageRows(session.storage(scope || undefined)).map((r) => [
    r.id,
    cell(r.value, 54),
    String(r.at),
  ]);
  if (rows.length === 0) {
    out(`storage is empty at step ${at}`);
    return;
  }
  table(['key', 'value', 'written at'], rows);
}

function cmdGas(args: Args): void {
  const trace = requireTrace(args);
  if (flag(args.verify)) return void cmdVerify(args);
  const by = (str(args.by, 'function') as 'function' | 'op' | 'frame');
  const { slices, total } = gasBreakdown(trace, by);
  table(
    [by === 'frame' ? 'frame' : by, 'instructions', 'share', 'read B', 'write B'],
    slices.map((s) => [s.label, s.gas.instructions.toLocaleString(), pct(s.share), String(s.gas.readBytes), String(s.gas.writeBytes)]),
  );
  out('');
  out(`total ${total.toLocaleString()} instructions`);
}

function cmdStack(args: Args): void {
  const trace = requireTrace(args);
  const at = num(args.at, trace.steps.length - 1);
  const session = new DebugSession({ trace });
  seekTo(session, at);
  const stack = session.stack();
  if (stack.length === 0) {
    out(`not stopped at step ${at}`);
    return;
  }
  // Numbered innermost first, as debuggers and the DAP stack pane do. `depth`
  // is the call depth, which is the opposite end of the stack.
  const inner = [...stack].reverse();
  inner.forEach((f, i) => {
    out(`${i === 0 ? '▸' : ' '} #${i} ${f.name}  (depth ${f.depth}, ${f.source?.path ?? 'src/lib.rs'}:${f.line})`);
  });
}

function cmdEvents(args: Args): void {
  const trace = requireTrace(args);
  const at = args.at !== undefined ? num(args.at, trace.steps.length) : undefined;
  const all = eventLog(trace);
  const list = at === undefined ? all : all.filter((e) => e.step <= at);
  if (list.length === 0) {
    out('no events');
    return;
  }
  table(
    ['#', 'step', 'depth', 'event', 'topics', 'data'],
    list.map((e) => [String(e.ordinal), String(e.step), String(e.depth), e.name, cell(e.topics, 30), cell(e.data, 30)]),
  );
  if (at === undefined) {
    out('');
    out('by event type');
    table(
      ['event', 'count', 'emitters', 'gas'],
      eventBreakdown(trace).map((b) => [b.name, String(b.count), cell(b.emitters.join(', '), 40), b.gas.toLocaleString()]),
    );
  }
}

function cmdStep(args: Args): void {
  const trace = requireTrace(args);
  const to = num(args.to, trace.steps.length - 1);
  const session = new DebugSession({ trace });
  seekTo(session, to);
  const step = session.currentStep();
  const payload = {
    stepIndex: session.stepIndex,
    step,
    stopped: session.isPaused,
    locals: session.locals(),
    args: session.frameArgs(),
    stack: session.stack().map((f) => f.name),
    gasSpent: session.gasSpent(),
    gasRemaining: session.gasRemaining(),
    eventsEmitted: session.events().length,
    storageChanges: storageDiff(trace, session.stepIndex, session.stepIndex),
    storage: storageRows(session.storage()),
  };
  if (flag(args.json)) {
    out(JSON.stringify(payload, null, 2));
    return;
  }
  if (!step) {
    out(`trace has no step at index ${to}`);
    return;
  }
  out(`stopped at step ${step.index}  ${step.function} / ${step.op}   (depth ${step.depth})`);
  if (step.storage) out(`  storage      ${step.storage.op} ${step.storage.scope}:${step.storage.key} = ${JSON.stringify(step.storage.value ?? null)}`);
  if (step.event) out(`  event        ${step.event.name} topics=${JSON.stringify(step.event.topics)} data=${JSON.stringify(step.event.data)}`);
  if (step.error) out(`  panic        ${step.error.name} (#${step.error.code})`);
  if (step.note) out(`  note         ${step.note}`);
  out(`  stack        ${session.stack().map((f) => f.name).join(' → ')}`);
  out(`  gas          ${session.gasSpent().instructions.toLocaleString()} spent, ${session.gasRemaining().instructions.toLocaleString()} remaining`);
  out(`  events       ${session.events().length} emitted so far`);
  const locals = session.locals();
  if (Object.keys(locals).length > 0) {
    out('  locals');
    for (const [k, v] of Object.entries(locals)) out(`    ${k.padEnd(20)} ${JSON.stringify(v)}`);
  }
  const changes = payload.storageChanges;
  if (changes.length > 0) {
    out('  storage changes at this step');
    for (const c of changes) out(`    ${c.kind.padEnd(8)} ${c.scope}:${c.key} ${JSON.stringify(c.before ?? null)} → ${JSON.stringify(c.after ?? null)}`);
  }
  const rows = payload.storage;
  if (rows.length > 0) {
    out('  storage state');
    for (const r of rows) out(`    ${r.id.padEnd(48)} ${cell(r.value, 44)}`);
  }
}

function cmdWatch(args: Args): void {
  const trace = requireTrace(args);
  const expression = str(args.expr);
  if (!expression) throw new Error('expected --expr <expression>');
  const at = num(args.at, trace.steps.length - 1);
  const session = new DebugSession({ trace });
  seekTo(session, at);
  const result = session.tryWatch(expression);
  if (!result.ok) {
    err(`error: ${result.error}`);
    process.exitCode = 1;
    return;
  }
  out(result.display);
}

function cmdReplay(args: Args): void {
  const trace = requireTrace(args);
  const session = openSession(args, trace);
  const stop = session.runToNextBreakpoint(0);
  out(`${stop.description}  [step ${stop.stepIndex}] ${stop.text}`);
  const next = session.events();
  if (next.length > 0) out(`events so far: ${next.map((e) => e.name).join(', ')}`);
  const gas = checkGasConsistency(trace);
  if (!gas.consistent) {
    err(`accounting mismatch: steps sum to ${gas.summed}, totals record ${gas.recorded}`);
    process.exitCode = 1;
  }
}

function cmdVerify(args: Args): void {
  const trace = requireTrace(args);
  const gas = checkGasConsistency(trace);
  const rows: Array<[string, string, string, string, string]> = [
    ['instructions', String(gas.summed), String(gas.recorded), String(gas.delta), ok(gas.consistent)],
    ['readBytes', String(gas.readBytes.summed), String(gas.readBytes.recorded), String(gas.readBytes.delta), ok(gas.readBytes.consistent)],
    ['writeBytes', String(gas.writeBytes.summed), String(gas.writeBytes.recorded), String(gas.writeBytes.delta), ok(gas.writeBytes.consistent)],
    ['events', String(gas.events.summed), String(gas.events.recorded), String(gas.events.delta), ok(gas.events.consistent)],
    ['storageReads', String(gas.storageReads.summed), String(gas.storageReads.recorded), String(gas.storageReads.delta), ok(gas.storageReads.consistent)],
    ['storageWrites', String(gas.storageWrites.summed), String(gas.storageWrites.recorded), String(gas.storageWrites.delta), ok(gas.storageWrites.consistent)],
  ];
  out(`totals cross-check (${trace.detail} detail)`);
  table(['dimension', 'from steps', 'recorded', 'delta', 'status'], rows);
  out('');
  out('per-frame declared vs observed');
  table(
    ['frame', 'function', 'declared', 'observed', 'delta', 'status'],
    gas.frames.map((f) => [
      f.frameId,
      trace.frames.find((x) => x.id === f.frameId)?.function ?? '?',
      f.declared === undefined ? '∅' : String(f.declared),
      String(f.observed),
      String(f.delta),
      ok(f.consistent),
    ]),
  );
  const bad = rows.filter((r) => r[4] !== 'ok').length + gas.frames.filter((f) => !f.consistent).length;
  out('');
  out(bad === 0 ? 'all checks passed' : `${bad} check(s) failed`);
  if (bad > 0) process.exitCode = 1;
}

const ok = (consistent: boolean): string => (consistent ? 'ok' : 'MISMATCH');

function cmdNormalize(args: Args): void {
  const path = positional(args, 1);
  if (!path) throw new Error('expected an RPC result JSON file');
  const rpc = JSON.parse(readFileSync(path, 'utf8')) as RpcResultEnvelope;
  if (rpc.error) throw new Error(`RPC error: ${rpc.error.message ?? 'unknown'}`);
  const trace = normalizeRpcResult({
    rpc,
    contract: { id: str(args['contract-id'], 'local'), name: str(args['contract-name'], 'AuditLedger'), version: str(args['contract-version'], '0.1.0') },
    ledger: args.ledger ? num(args.ledger, 0) : undefined,
  });
  const text = serializeTrace(trace);
  const dest = str(args.o);
  if (dest) {
    require('fs').writeFileSync(dest, text);
    out(`wrote ${dest} (${trace.steps.length} step(s), ${trace.detail} detail)`);
    if (trace.detail === 'summary') {
      out('note: RPC results carry aggregate cost only; step-through needs a recorded operation-level trace');
    }
  } else {
    process.stdout.write(text);
  }
}

function cmdScenario(args: Args): void {
  const name = positional(args, 1);
  const build = name ? scenarioBuilders[name] : undefined;
  if (!build) {
    err(`unknown scenario ${JSON.stringify(name)}; available: ${Object.keys(scenarioBuilders).join(', ')}`);
    process.exitCode = 1;
    return;
  }
  const text = serializeTrace(build());
  const dest = str(args.o);
  if (dest) {
    require('fs').writeFileSync(dest, text);
    out(`wrote ${dest}`);
  } else {
    process.stdout.write(text);
  }
}

function cmdRepl(args: Args): void {
  const trace = requireTrace(args);
  const session = openSession(args, trace);
  out(`audit-ledger-debug ${VERSION} — ${trace.contract.name} v${trace.contract.version}, ${trace.steps.length} steps`);
  out('commands: n/next, s/stepIn, o/stepOut, c/continue, p/print EXPR, stack, storage [N], gas, events, br <fn>[:<op>], rmbr, loc, help, quit');

  const readline = require('readline') as typeof import('readline');
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout, prompt: '(dbg) ' });
  rl.prompt();
  rl.on('line', (line: string) => {
    const [cmd, ...rest] = line.trim().split(/\s+/);
    const arg = rest.join(' ');
    try {
      switch (cmd) {
        case '':
          break;
        case 'n':
        case 'next':
        case 's':
        case 'in':
        case 'o':
        case 'out': {
          const direction = cmd === 'n' || cmd === 'next' ? 'over' : cmd === 's' || cmd === 'in' ? 'into' : 'out';
          const stop = session.step(direction);
          out(`[${stop.stepIndex}] ${stop.text}`);
          if (stop.reason === 'end') rl.close();
          break;
        }
        case 'c':
        case 'continue': {
          const stop = session.step('continue');
          out(`[${stop.stepIndex}] ${stop.text}`);
          if (stop.reason === 'end') rl.close();
          break;
        }
        case 'p':
        case 'print': {
          const r = session.tryWatch(arg);
          out(r.ok ? r.display : `error: ${r.error}`);
          break;
        }
        case 'stack': {
          [...session.stack()].reverse().forEach((f, i) => out(`  #${i} ${f.name}`));
          break;
        }
        case 'storage': {
          seekTo(session, arg ? Number(arg) : session.stepIndex);
          for (const r of storageRows(session.storage())) out(`  ${r.id.padEnd(46)} ${cell(r.value, 40)}`);
          break;
        }
        case 'gas': {
          const g = session.gasSpent();
          out(`  spent ${g.instructions} instructions, ${g.readBytes}B read, ${g.writeBytes}B write`);
          out(`  remaining ${session.gasRemaining().instructions}`);
          break;
        }
        case 'events': {
          for (const e of session.events()) out(`  [${e.step}] ${e.name}`);
          break;
        }
        case 'br': {
          const [fn, op] = arg.split(':');
          session.setBreakpoints([{ functionName: fn, op: op || undefined }]);
          out(`breakpoint on ${fn}${op ? ` op=${op}` : ''}`);
          break;
        }
        case 'rmbr':
          session.setBreakpoints([]);
          out('breakpoints cleared');
          break;
        case 'loc': {
          const l = session.locals();
          if (Object.keys(l).length === 0) out('  (no locals recorded at this step)');
          for (const [k, v] of Object.entries(l)) out(`  ${k.padEnd(20)} ${JSON.stringify(v)}`);
          break;
        }
        case 'help':
          out('  n next | s in | o out | c continue | p EXPR | stack | storage N | gas | events | br fn[:op] | rmbr | loc | quit');
          break;
        case 'q':
        case 'quit':
        case 'exit':
          rl.close();
          return;
        default:
          out(`unknown command ${JSON.stringify(cmd)}; try help`);
      }
    } catch (e) {
      out(`error: ${e instanceof Error ? e.message : String(e)}`);
    }
    rl.prompt();
  });
  rl.on('close', () => {
    session.terminate();
    out('bye');
  });
}

function cmdDap(args: Args): void {
  const server = new DapServer({
    input: process.stdin,
    output: process.stdout,
    defaultTracePath: str(args.trace) || undefined,
  });
  // Keep the process alive; stdio drives the lifetime.
  void server;
}

const COMMANDS: Record<string, (args: Args) => void> = {
  inspect: cmdInspect,
  steps: cmdSteps,
  storage: cmdStorage,
  gas: cmdGas,
  stack: cmdStack,
  events: cmdEvents,
  step: cmdStep,
  watch: cmdWatch,
  replay: cmdReplay,
  verify: cmdVerify,
  normalize: cmdNormalize,
  scenario: cmdScenario,
  repl: cmdRepl,
  dap: cmdDap,
};

export function main(argv: string[] = process.argv.slice(2)): void {
  const args = parseArgs(argv);
  if (flag(args.help) || args._.length === 0 || args._[0] === 'help') {
    out(USAGE);
    return;
  }
  if (flag(args.version) || args._[0] === 'version') {
    out(VERSION);
    return;
  }
  const name = positional(args, 0);
  const command = name ? COMMANDS[name] : undefined;
  if (!command) {
    err(`unknown command ${JSON.stringify(name ?? '')}\n`);
    err(USAGE);
    process.exitCode = 1;
    return;
  }
  try {
    command(args);
  } catch (e) {
    err(`error: ${e instanceof Error ? e.message : String(e)}`);
    process.exitCode = 1;
  }
}

if (require.main === module) main();
