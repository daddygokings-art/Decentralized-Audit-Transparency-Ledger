import { DapMessage, MessageDecoder, encodeMessage, makeErrorResponse, makeEvent, makeResponse, nextSeq } from './protocol';
import { DebugSession, StoppedState } from '../engine/session';
import { loadTrace, parseTrace } from '../trace/load';
import { checkGasConsistency } from '../engine/gas';
import { TRACE_FORMAT_VERSION, TransactionTrace } from '../trace/schema';

export interface DapServerOptions {
  input: NodeJS.ReadableStream;
  output: NodeJS.WritableStream;
  /** Trace path supplied by the client; falls back to this. */
  defaultTracePath?: string;
  /** Set false to answer without replaying a trace, for client tests. */
  autoValidate?: boolean;
}

interface LaunchArgs {
  trace?: string;
  tracePath?: string;
  stopOnEntry?: boolean;
  cwd?: string;
}

/** First function to execute each recorded source line, for line breakpoints. */
function lineOwnersOf(trace: TransactionTrace): Map<number, string> {
  const owners = new Map<number, string>();
  for (const s of trace.steps) {
    const line = s.location?.line;
    if (line !== undefined && !owners.has(line)) owners.set(line, s.function);
  }
  return owners;
}

/**
 * Debug Adapter Protocol server for transaction traces.
 *
 * A DAP adapter means VS Code (and any other DAP client) drives the debugger
 * with no custom extension code beyond registering the debug type, so
 * breakpoints, stepping, the call stack, the variables pane and watch
 * expressions all come from the client's standard UI.
 *
 * Trace "source lines" are the contract's own `src/lib.rs`; the adapter maps a
 * recorded step's `location` to a real line when the trace carries one, which
 * is what lets a user set a breakpoint by clicking in the editor.
 */
export class DapServer {
  private readonly decoder = new MessageDecoder();
  private session: DebugSession | null = null;
  private tracePath: string | undefined;
  private configurationDone = false;
  private stopOnEntry = true;
  private started = false;

  constructor(private readonly options: DapServerOptions) {
    this.tracePath = options.defaultTracePath;
    options.input.on('data', (chunk: Buffer) => this.onData(chunk));
    options.input.on('end', () => this.send({ seq: nextSeq(), type: 'event', event: 'terminated', body: {} }));
  }

  private send(message: DapMessage): void {
    this.options.output.write(encodeMessage(message));
  }

  private onData(chunk: Buffer): void {
    for (const message of this.decoder.push(chunk)) {
      void this.handle(message);
    }
  }

  private async handle(message: DapMessage): Promise<void> {
    if (message.type !== 'request') return;
    const command = message.command ?? '';
    const reply = (body?: Record<string, unknown>): void => this.send(makeResponse(message, body));
    const fail = (msg: string): void => this.send(makeErrorResponse(message, msg));

    try {
      switch (command) {
        case 'initialize':
          reply({
            supportsConfigurationDoneRequest: true,
            supportsFunctionBreakpoints: true,
            supportsConditionalBreakpoints: false,
            supportsEvaluateForHovers: true,
            supportsSetVariable: false,
            supportsRestartRequest: false,
            supportsStepBack: true,
            supportsTerminateRequest: true,
            supportsDisassembleRequest: false,
            exceptionBreakpointFilters: [],
            // Stepping a recorded trace is reversible, so the client may offer
            // "step backwards" and replay from the start.
            supportTerminateDebuggee: true,
          });
          return;
        case 'launch':
        case 'attach': {
          const args = (message.arguments ?? {}) as LaunchArgs;
          if (this.options.defaultTracePath) this.tracePath = this.options.defaultTracePath;
          if (args.tracePath || args.trace) this.tracePath = args.tracePath ?? args.trace;
          this.stopOnEntry = args.stopOnEntry !== false;
          if (!this.tracePath) {
            fail('no trace supplied: pass --trace <path> or set "trace" in launch.json');
            return;
          }
          const loaded = loadTrace(this.tracePath);
          this.session = new DebugSession({ trace: loaded.trace, output: (l) => this.send(makeEvent('output', { category: 'console', output: `${l}\n` })) });
          for (const w of loaded.warnings) this.send(makeEvent('output', { category: 'stderr', output: `trace warning: ${w}\n` }));
          if (this.options.autoValidate !== false) {
            const gas = checkGasConsistency(loaded.trace);
            if (!gas.consistent) {
              this.send(
                makeEvent('output', {
                  category: 'stderr',
                  output: `trace gas mismatch: steps sum to ${gas.summed} instructions, totals record ${gas.recorded}\n`,
                }),
              );
            }
          }
          reply({ started: true });
          return;
        }
        case 'setFunctionBreakpoints': {
          const session = this.requireSession(fail);
          if (!session) return;
          const wanted = (message.arguments?.breakpoints ?? []) as Array<{ name: string; line?: number; condition?: string }>;
          const requested = wanted.map((b) => ({ functionName: b.name, line: b.line, op: b.condition }));
          const applied = session.setBreakpoints(requested);
          reply({
            breakpoints: applied.map((bp) => ({
              id: bp.id,
              verified: bp.verified,
              line: bp.line ?? 1,
              // Surface an unverified breakpoint in the UI rather than silently
              // never hitting it.
              message: bp.verified ? undefined : `no step in this trace calls ${bp.functionName}`,
            })),
          });
          return;
        }
        case 'setExceptionBreakpoints':
          reply({ breakpoints: [] });
          return;
        case 'setBreakpoints': {
          // Source-line breakpoints. A recorded trace is not a source file, so
          // the path is taken as a contract function name; an explicit
          // "breakpoint by line" maps to the function owning that line.
          const session = this.requireSession(fail);
          if (!session) return;
          const args = (message.arguments ?? {}) as { breakpoints?: Array<{ line?: number; condition?: string }> };
          // A source-line breakpoint on the contract: map it onto the
          // function that owns that line, so clicking in the editor works.
          const lineOwners = lineOwnersOf(session.trace);
          const created = (args.breakpoints ?? []).map((b) => {
            const fn = b.line !== undefined ? lineOwners.get(b.line) : undefined;
            const bp = session.addBreakpoint(
              { functionName: fn ?? '', line: b.line, op: b.condition },
              fn !== undefined,
            );
            return {
              id: bp.id,
              verified: bp.verified,
              line: b.line ?? 1,
              message: fn === undefined ? `no step in this trace runs line ${b.line}` : undefined,
            };
          });
          reply({ breakpoints: created });
          return;
        }
        case 'configurationDone': {
          this.configurationDone = true;
          reply();
          if (this.session && this.started && this.stopOnEntry && this.session.stepIndex < 0) {
            this.emitStopped(this.session.runToNextBreakpoint(0));
          }
          return;
        }
        case 'threads':
          reply({ threads: [{ id: 1, name: 'transaction' }] });
          return;
        case 'next':
        case 'stepIn':
        case 'stepOut':
        case 'continue': {
          const session = this.requireSession(fail);
          if (!session) return;
          this.started = true;
          const direction =
            command === 'next' ? 'over' : command === 'stepIn' ? 'into' : command === 'stepOut' ? 'out' : 'continue';
          const stop = session.step(direction);
          reply(command === 'continue' ? { allThreadsContinued: true } : undefined);
          this.emitStopped(stop);
          return;
        }
        case 'pause': {
          const session = this.requireSession(fail);
          if (!session) return;
          const stop = session.step('pause');
          reply();
          this.emitStopped(stop);
          return;
        }
        case 'stackTrace': {
          const session = this.requireSession(fail);
          if (!session) return;
          reply({ stackFrames: session.stack(), totalFrames: session.stack().length });
          return;
        }
        case 'scopes': {
          const session = this.requireSession(fail);
          if (!session) return;
          reply({ scopes: session.scopes() });
          return;
        }
        case 'variables': {
          const session = this.requireSession(fail);
          if (!session) return;
          const ref = Number(message.arguments?.variablesReference ?? 0);
          reply({ variables: session.variables(ref) });
          return;
        }
        case 'evaluate': {
          const session = this.requireSession(fail);
          if (!session) return;
          const expression = String(message.arguments?.expression ?? '');
          const result = session.tryWatch(expression);
          if (!result.ok) {
            fail(result.error);
            return;
          }
          reply({ result: result.display, type: result.type, variablesReference: 0 });
          return;
        }
        case 'source': {
          const args = (message.arguments ?? {}) as { source?: { path?: string } };
          reply({ source: { name: 'src/lib.rs', path: args.source?.path ?? 'src/lib.rs' } });
          return;
        }
        case 'disconnect':
        case 'terminate': {
          this.session?.terminate();
          reply();
          this.send({ seq: nextSeq(), type: 'event', event: 'terminated', body: {} });
          return;
        }
        default:
          // Unknown requests get a null-result success rather than an error, so
          // a client probing optional capabilities is not disconnected.
          this.send({ ...makeResponse(message), body: {} });
          return;
      }
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
    }
  }

  private requireSession(fail: (msg: string) => void): DebugSession | null {
    if (!this.session) {
      fail('debugger not launched: send "launch" with a trace path first');
      return null;
    }
    return this.session;
  }

  private emitStopped(stop: StoppedState): void {
    if (stop.reason === 'end') {
      this.send({ seq: nextSeq(), type: 'event', event: 'terminated', body: {} });
      return;
    }
    this.send(
      makeEvent('stopped', {
        reason: stop.reason === 'error' ? 'exception' : 'step',
        // `all` keeps the single synthetic thread's stack visible in clients
        // that filter by thread.
        threadId: 1,
        allThreadsStopped: true,
        preserveFocusHint: false,
        text: stop.text,
        description: stop.description,
      }),
    );
    if (stop.reason === 'error') {
      this.send(
        makeEvent('output', {
          category: 'stderr',
          output: `${stop.description}\n`,
        }),
      );
    }
  }

  /** Exposed for tests: the underlying session once launched. */
  getSession(): DebugSession | null {
    return this.session;
  }

  getConfigurationDone(): boolean {
    return this.configurationDone;
  }
}

/** Build a trace directly from JSON text, for in-process embedding. */
export function sessionFromText(text: string, output?: (line: string) => void): DebugSession {
  const { trace } = parseTrace(text);
  return new DebugSession({ trace, output });
}

export { TRACE_FORMAT_VERSION };
