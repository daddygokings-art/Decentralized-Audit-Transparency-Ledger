# Contract debugger and transaction tracer

`tools/contract-debugger` replays a recorded contract interaction step by step:
call stack, storage at each step, emitted events, per-operation gas, and watch
expressions. It ships a Debug Adapter Protocol server, so VS Code drives it with
its standard debugging UI, and a CLI for the same views in a terminal.

- [Quick start](#quick-start)
- [The trace format](#the-trace-format)
- [Where traces come from](#where-traces-come-from)
- [CLI](#cli)
- [VS Code](#vs-code)
- [Debug adapter protocol](#debug-adapter-protocol)
- [Watch expressions](#watch-expressions)
- [Accuracy](#accuracy)
- [Design notes](#design-notes)

## Quick start

```console
$ cd tools/contract-debugger
$ npm ci
$ npm test

# Write a worked example trace.
$ npx ts-node src/cli.ts scenario log-event -o /tmp/log-event.json

# Look at it.
$ npx ts-node src/cli.ts inspect /tmp/log-event.json
$ npx ts-node src/cli.ts steps  /tmp/log-event.json --storage
$ npx ts-node src/cli.ts gas    /tmp/log-event.json --by op
$ npx ts-node src/cli.ts verify /tmp/log-event.json
```

To use it against a real contract, point `--trace` at a recorded trace, or at an
RPC result you have normalized:

```console
$ stellar contract invoke ... --send 2>&1 | jq . > /tmp/result.json
$ npx ts-node src/cli.ts normalize /tmp/result.json \
    --contract-id C… --contract-name AuditLedger -o /tmp/trace.json
$ npx ts-node src/cli.ts step /tmp/trace.json --to 12
```

## The trace format

A trace is a single JSON document, versioned by `formatVersion`. It is
deliberately self-describing: a trace recorded against a testnet node replays
anywhere, with no dependency on the tooling that produced it.

```jsonc
{
  "formatVersion": 1,
  "contract":  { "id": "local", "name": "AuditLedger", "version": "0.1.0" },
  "transaction": { "hash": "…", "source": "recorded", "ledger": 1000000 },

  // The resource accounting the recorder claims for the whole interaction.
  "totals": { "instructions": 4123, "readBytes": 64, "writeBytes": 176,
              "events": 1, "storageReads": 7, "storageWrites": 6, "fee": 100000 },

  "frames": [
    { "id": "f0", "function": "log_event", "args": { "event_type": "access_granted" },
      "gas": { "instructions": 305, "readBytes": 8, "writeBytes": 16 } },
    { "id": "f1", "function": "append_event", "parentId": "f0",
      "returnValue": "0xa1…", "gas": { "instructions": 1305, "readBytes": 8, "writeBytes": 128 } }
  ],

  // The flat, ordered execution. `index` is dense, so a debugger position is
  // always a valid array offset.
  "steps": [
    { "index": 0, "frameId": "f0", "depth": 0, "function": "log_event",
      "op": "call", "gas": { "instructions": 30 },
      "call": { "target": "append_event" }, "location": { "file": "src/lib.rs", "line": 428 } },

    { "index": 1, "frameId": "f1", "depth": 1, "function": "append_event",
      "op": "storage.set", "gas": { "instructions": 320, "writeBytes": 16 },
      "storage": { "op": "write", "scope": "persistent", "key": "EventData(0xa1…)",
                   "value": { "sequence": 42 }, "previous": null },
      "location": { "file": "src/lib.rs", "line": 528 }, "locals": { "id": "0xa1…" } }
  ]
}
```

### Design rules

**Steps are flat, everything else is a fold.** The debugger advances one cursor
into `steps`, and the call stack, storage state, event log and gas breakdown are
all derived from the same sequence. There is no second copy of the truth that
could disagree with the first.

**A read is as authoritative as a write.** A read returns the key's value, so it
seeds storage state just as a write does. Without this, every key the contract
only ever reads — `Paused`, `AllowlistMode`, `Config` — would be missing from the
storage view, and a key's first appearance would be misreported as a creation.
The consequence is that `created` in a diff means *first observed in this range*,
by a read or a write; state before that first observation is genuinely unknown to
a trace, and is reported as absent rather than guessed at.

**Storage keys are `scope:key`.** A Soroban key is only meaningful together with
its tier, so keys are namespaced by `instance` or `persistent` everywhere: in
diffs, in the variables pane, and in the CLI. The key itself uses the contract's
own Rust spelling, e.g. `EventData(0xa1…)` or `EventTypeIndices(access_granted)`.

**Frame cost excludes callees.** A frame's declared `gas` covers only the steps
recorded in that frame. A delegating function such as `log_event` is not charged
for the work `append_event` did, so a per-frame breakdown never double counts.

**`detail` says how much the trace knows.** `operation` means step-by-step
execution was recorded and `totals` is reconstructible. `summary` means the
trace came from RPC metadata, which carries aggregate cost and events but no
per-operation sequence; there `totals` is authoritative and there is nothing to
step through. `verify` says so rather than reporting a missing fold as a
mismatch.

## Where traces come from

Two producers, and the difference matters when you are reading a view.

### Recorded traces (`detail: "operation"`)

Full-fidelity: storage reads and writes, event emissions, calls and returns, the
frame tree, and per-step instruction counts. This is what step-through
debugging, storage inspection, event tracing and per-operation gas are built on.

The host already carries the data for this. `soroban_env_host` exposes
`TraceEvent` and `TraceState` through a trace hook, and `TraceState` holds
exactly what this format needs — `cpu_insns`, `mem_bytes`, instance and ledger
storage sizes, and the event count — while `TraceEvent::EnvCall`/`EnvRet` supply
each call with its arguments and `PushCtx`/`PopCtx` supply the frame stack. A
recorder is therefore mostly a matter of transcribing those into this schema.

One constraint is worth stating plainly: in `soroban-env-host` 27.0.1,
`Host::set_trace_hook` is `pub(crate)`. An out-of-tree crate cannot install a
trace hook today. Producing an operation-level trace for a *hosted* contract
therefore requires either a recorder compiled against the host's internals, or an
instrumented harness that drives the contract through `Env::test` and records
each host call itself. Until one of those exists, the `scenario` traces below are
the reference the debugger is tested against, and `normalize` is what you can
feed it from a live node today.

### Normalized RPC results (`detail: "summary"`)

`stellar contract simulate` / `getTransaction` output can be converted with
`normalize`. Be clear about what that gives you: the aggregate instruction count,
the fee, the footprint, the ledger-entry churn, and the emitted events. It does
**not** give you the operation sequence, per-step gas, or the frame tree, because
RPC does not return them. Inspect, verify, and the event log all work; stepping
and per-operation gas do not, and the CLI says so when it writes the trace.

## CLI

```
audit-ledger-debug <command> <trace.json> [options]
```

| Command | What it does |
| --- | --- |
| `inspect <trace>` | Contract, transaction, totals, step count, accounting status |
| `steps <trace>` | Execution timeline; `--storage` / `--events` / `--calls` to filter |
| `storage <trace>` | Storage state; `--at N` for a step, `--diff FROM:TO` for a range, `--scope` to filter |
| `gas <trace>` | Instruction breakdown; `--by function\|op\|frame` |
| `gas <trace> --verify` | Cross-check recorded totals against the steps |
| `stack <trace>` | Call stack at a step, innermost frame first |
| `events <trace>` | Emission log, plus a per-event-type summary |
| `step <trace> --to N` | Replay to a step and print the full state; `--json` for scripting |
| `watch <trace> --expr EXPR` | Evaluate a watch expression |
| `replay <trace> --break fn[:op]` | Run until a breakpoint |
| `verify <trace>` | Full accuracy report |
| `normalize <rpc.json>` | Build a summary trace from RPC output |
| `scenario <name>` | Write a worked example trace |
| `dap` | Run the debug adapter on stdio |
| `repl <trace>` | Interactive debugger prompt |

Frames are numbered innermost first, matching the debug adapter and the VS Code
frame list; `depth` in a trace is the opposite end of the stack and is reported
alongside.

The REPL:

```
(dbg) br append_event:storage.set
breakpoint on append_event op=storage.set
(dbg) c
[14] write persistent:EventData(0xa1a1a1a1…)
(dbg) p storage.RuntimeState.total_events
41
(dbg) stack
  #0 append_event
  #1 log_event
(dbg) loc
  (no locals recorded at this step)
(dbg) gas
  spent 1100 instructions, 56B read, 16B write
(dbg) quit
bye
```

`stack` and `gas` show the two views a contract developer reaches for most: which
helper is running, and what it has cost so far.

### Built-in scenarios

`scenario <name>` writes a worked trace. They exist so the debugger's behaviour
is pinned to something inspectable, and they double as examples of the format.

| Name | What it exercises |
| --- | --- |
| `log-event` | A single `log_event`: seven delegated helpers, thirteen storage operations (7 reads, 6 writes), one event, a real call stack |
| `batch` | `log_events` with three children, showing frames nesting and one write burst per item |
| `panic` | `GlobalMaxLogsReached`, for error stops and unwinding frames |
| `read-writeback` | `get_event`, a value-returning call that still writes a periodic TTL cleanup |
| `inconsistent` | Deliberately wrong totals, for proving `verify` catches a bad trace |

## VS Code

`tools/contract-debugger/vscode` is a thin extension that registers the
`audit-ledger-trace` debug type and launches the adapter. Because the debugger
speaks DAP, breakpoints, stepping, the call stack, the variables pane and watch
expressions all come from VS Code itself rather than a custom UI.

`.vscode/launch.json`:

```json
{
  "type": "audit-ledger-trace",
  "request": "launch",
  "name": "Debug transaction trace",
  "trace": "${workspaceFolder}/traces/latest.json",
  "stopOnEntry": true
}
```

Press F5, or use **AuditLedger: Open Transaction Trace** to pick a file.
**AuditLedger: Verify Transaction Trace** runs the accounting cross-check on the
active file and shows the report.

Each step's `location` points at the contract's own `src/lib.rs`, so clicking a
step in the call stack, or setting a line breakpoint, lands on the line that
produced it. A breakpoint on a function the trace never calls comes back
`verified: false` with a reason, rather than silently never hitting.

Because a trace is immutable, stepping backwards is a seek rather than a rewind,
and the adapter advertises `supportsStepBack`.

## Debug adapter protocol

JSON-RPC 2.0 over stdio, `Content-Length` framed, on `audit-ledger-debug dap`.

| Request | Behaviour |
| --- | --- |
| `initialize` | Advertises configurationDone, function breakpoints, hover evaluation and step-back |
| `launch` / `attach` | `trace` or `tracePath`; a `summary` trace loads but cannot be stepped |
| `setFunctionBreakpoints` | Break on a contract function, optionally narrowed by `op` or line |
| `setBreakpoints` | Line breakpoints, mapped onto the function that owns that line |
| `configurationDone` | Ends setup; the run starts |
| `next` / `stepIn` / `stepOut` / `continue` / `pause` | Stepping; `over` runs the callee, `into` descends into it, `out` returns to the caller |
| `stackTrace` | Frames innermost first, each with its `src/lib.rs` location |
| `scopes` | `Locals`, `Arguments`, `Storage`, `Call Stack`, `Gas` |
| `variables` | Expands a scope reference; storage entries are `scope:key` |
| `evaluate` | Watch expressions and hover |
| `disconnect` / `terminate` | Ends the session |

Events: `stopped` (with `step`, `breakpoint` or `exception`), `terminated`,
`output`.

A panicking trace stops with `reason: "exception"` and the `ContractError` name
as the description, so a `ContractError` panic is as visible as a Rust panic.

## Watch expressions

The same evaluator backs the CLI and the adapter, so a watch behaves the same in
both places.

```
event_type                          a frame local
submitter                           an argument
storage.RuntimeState.total_events   storage, by dotted key
storage["EventOrder(42)"]           storage, by quoted key
storageByScope.instance.Paused      storage, by scope
stack                               the call stack
events.contains("event_logged")     the event log so far
gas.remaining                       instructions left
contract.name / transaction.source  metadata
2 + 3 * 4                           arithmetic, with precedence
list.len()  obj.keys()  n.type()    builtins
false && missing.thing              short-circuits, so this is false
```

Literals include numbers, strings, `true`/`false`/`null` and array literals.
Member access on a missing value is `undefined` rather than an error, so a watch
keeps working while a value is absent. Genuine mistakes — a syntax error, an
unknown method — are reported, with the character offset for a syntax error so a
UI can point at it.

## Accuracy

The debugger's claim is that its numbers are derived rather than asserted, so
`verify` and the tests both check the derivation.

**`totals` must equal the fold.** Instructions, read and written bytes, events,
storage reads and writes are each summed from the steps and compared against what
the trace records. Any disagreement is a non-zero exit.

**Each frame's declared cost must equal its steps.** A frame is charged only for
the steps recorded in it, so a delegating function is not double charged for its
callees.

**Storage state is a pure fold.** `storageAt(step)` recomputes from the start
every time, so the state at a step cannot depend on how the debugger got there.
The tests assert this directly: for every step, a session that stepped there and
one that seeked there report identical state, stack, gas and events.

**A bad trace is reported, not absorbed.** The `inconsistent` scenario inflates
its recorded totals by 5,000 instructions; CI asserts `verify` fails on it and
passes on the others. A cross-check that never fails is not a check.

The generated views are then pinned to hand-computed expectations — the
`log-event` trace's final state, the per-function gas shares summing to 1, the
call stack at each phase, and the exact step each kind of breakpoint lands on.

## Design notes

**Why DAP rather than a custom protocol.** The debugger is a viewer over a
recorded sequence, not a live process: it cannot attach to a running contract and
it does not need to. DAP already specifies the request vocabulary for exactly
this — stepping, breakpoints, scopes, watch expressions, call stacks — and every
editor already speaks it. Building a custom protocol would have meant building a
custom UI to match, for no gain.

**Why the trace is immutable and replay is a fold.** An alternative design
mutates state as you step. That makes a debugger that cannot be stepped
backwards, and whose state depends on the path taken to reach it. Recomputing from
the start makes seeking free, makes step-backwards correct, and removes an entire
class of state-desync bug. Traces are thousands of steps, so the fold is cheap.

**Why `verify` treats a `summary` trace differently.** An operation-level trace
is held to the sum, because the sum is reconstructible and a disagreement means
the recorder is wrong. A summary trace has no step sequence to fold, so its
totals are authoritative; reporting the absence of a fold as a mismatch would
train people to ignore the check.

**Known limitations.**

- Operation-level capture needs host trace-hook access that `soroban-env-host`
  27.0.1 does not expose to an out-of-tree crate. See
  [Where traces come from](#where-traces-come-from).
- `normalize` does not decode XDR, so ledger-entry keys stay opaque base64. This
  is deliberate: the debugger takes no Stellar SDK dependency, and a key it cannot
  decode is better left visibly raw than mis-decoded.
- Locals come from the recorder. A trace that does not record them shows an empty
  `Locals` scope rather than guessing.
- Stepping is over recorded steps, not instructions. A debugger that stopped
  between every WASM instruction would be unusable and is not what a contract
  developer needs.
