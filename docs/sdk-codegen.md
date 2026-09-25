# SDK code generation

`tools/sdk-codegen` turns the contract's ABI into type-safe client bindings for
TypeScript, Python and Rust. The committed IDL at
[`abi/audit-ledger.json`](../../abi/audit-ledger.json) is the single source of
truth: when the contract changes, regenerate it and the three SDKs move
together.

Generated code is committed, not built at install time. Consumers get plain
source with no code-generation step and no runtime dependency on this tool.

## Layout

| Path | Contents |
| --- | --- |
| `abi/audit-ledger.json` | The contract IDL (source of truth) |
| `sdk/js/src/generated/` | TypeScript types, errors, events, client |
| `sdk/python/audit_ledger/generated/` | Python dataclasses, enums, errors, events, client |
| `sdk/rust/src/generated/` | Rust types, errors, events, client |
| `tools/sdk-codegen/` | The generator itself |

## Commands

All commands are CWD-independent: paths are resolved against the repository
root.

```bash
cd tools/sdk-codegen
npm install

npx ts-node src/cli.ts extract   # src/lib.rs        -> abi/audit-ledger.json
npx ts-node src/cli.ts generate  # abi/…             -> the three sdk/ trees
npx ts-node src/cli.ts check     # validate the IDL and confirm no drift
npx ts-node src/cli.ts verify    # the full CI gate (extract + generated drift)
npx ts-node src/cli.ts compat -b <baseline.json> -c <candidate.json>
```

`generate` accepts `-l typescript,python,rust` to limit the languages, and
`--check` to report drift without writing.

## Type mapping

Contract field names are preserved verbatim in all three languages, because they
map 1:1 onto the wire encoding and translating them would hide a mismatch. Only
*method* names are idiomatic per language: `log_event` becomes `logEvent` in
TypeScript and `log_event` in Python and Rust.

| Contract | TypeScript | Python | Rust |
| --- | --- | --- | --- |
| `Address` | `string` | `str` | `Address` |
| `Symbol`, `String`, `Bytes` | `string` | `str` | `Symbol` / `String` / `Bytes` |
| `u32`, `i32` | `number` | `int` | `u32` / `i32` |
| `u64`, `i64` | `string` | `int` | `u64` / `i64` |
| `Timepoint`, `Duration` | `string` | `int` | `Timepoint` / `Duration` |
| `BytesN<N>` | `string` | `str` | `BytesN<N>` |
| `Vec<T>` | `Array<T>` | `List[T]` | `Vec<T>` |
| `Option<T>` | `T \| null` | `Optional[T]` | `Option<T>` |
| `(A, B)` | `[A, B]` | `Tuple[A, B]` | `(A, B)` |
| `Map<K, V>` | `Record<K, V>` | `Dict[K, V]` | `Map<K, V>` |

`u64`/`i64` are 64-bit and exceed JavaScript's safe integer range, so they are
carried as decimal strings rather than silently losing precision.

Python identifiers that collide with a keyword are suffixed (`None` becomes
`None_`); the wire name is unchanged on the wire.

## Read-only versus state-changing

The IDL records two independent facts per function:

- **`mutating`** — the function may change state or emit an event. This is
  derived from the body, not the name, and propagates through delegation:
  `log_event` is a one-line wrapper around `log_event_with_hierarchy`, so a
  name prefix would mislabel it. Several `get_*` functions *are* mutating
  because they perform a periodic, conditional TTL-cleanup write.
- **`needsTransaction`** — whether the caller must submit a transaction. This
  is what the generated client uses, and it is `mutating && returns void`, or
  the naming convention when the function returns a value. A `get_*` that
  returns a value stays a read-only call: `simulateTransaction` runs the code
  without committing state, so there is nothing to submit.

The generated clients in this repository are transport-agnostic — they take an
`AuditLedgerTransport` — so the read/transaction decision is applied by the
transport you plug in, and `needsTransaction` is what the doc comments and the
call shape reflect.

## How extraction works

By default the generator parses `src/lib.rs` directly, so no Rust toolchain is
needed. It recovers the contract surface by scanning for the `#[contract]`
struct, the `#[contractimpl]` impl block, `#[contracttype]` types, the
`ContractError` enum, and `env.events().publish(...)` calls.

Event payloads are resolved by reconstructing the variable scope at each publish
site: function parameters, annotated `let` bindings, `for` loop patterns, tuple
destructuring, and unannotated initialisers (member access, container indexing,
`Self::helper(...)` return types, and `if`/`else` branches). When a payload
entry cannot be resolved, extraction records a warning and omits that event
rather than guessing at a type.

Anything unparseable becomes a warning on stdout and is excluded from the IDL,
so a silent type error in a generated SDK is not possible.

To use the toolchain's own output instead of the source parser:

```bash
soroban contract bindings json > bindings.json
npx ts-node src/cli.ts extract --bindings bindings.json
```

## Compatibility

`compat` diffs two IDLs and classifies the change:

| Severity | Examples | Required `spec_version` |
| --- | --- | --- |
| `breaking` | function removed, parameter added or retyped, return type changed, error renumbered | major |
| `minor` | function, type, or event added | minor |
| `patch` | documentation only | patch |

`compat` exits non-zero on a breaking change, so it can gate a release. Store
the previous IDL as an artefact to diff against:

```bash
npx ts-node src/cli.ts compat -b abi/audit-ledger.prev.json -c abi/audit-ledger.json
```

## In CI

[`.github/workflows/sdk-codegen.yml`](../../.github/workflows/sdk-codegen.yml)
runs on every change to `src/lib.rs`, the IDL, the generated trees, or the
generator. It type-checks and unit-tests the generator, runs `verify` (the IDL
and the generated code must be committed up to date), and then proves the
generated artefacts still build: TypeScript under `--strict`, Python via
`compileall` plus an import, and Rust via `rustfmt --check`.

To regenerate after a contract change:

```bash
cd tools/sdk-codegen
npx ts-node src/cli.ts extract
npx ts-node src/cli.ts generate
npx vitest run
```

## Testing

```bash
cd tools/sdk-codegen
npx vitest run
```

`test/fixtures/mini-contract.rs` is a miniature contract that exercises every
type shape the extractor must handle. `test/pipeline.test.ts` asserts against
the committed artefacts, so a stale IDL or a drifting generated file fails the
suite rather than reaching a consumer.
