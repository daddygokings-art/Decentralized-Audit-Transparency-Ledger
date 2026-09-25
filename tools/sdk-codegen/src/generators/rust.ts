import { ContractIdl, IdlEnum, IdlStruct, IdlType } from '../abi/types';
import { REPO_ROOT } from '../pipeline';
import { GeneratedFile, banner, footer, joinPath, projectFunctions, renderType } from './common';
import { spawnSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

/** Directory (repo-relative) the Rust artefacts are written to. */
export const RUST_OUTPUT_DIR = 'sdk/rust/src/generated';

/** Map an IDL type to the Rust type expression used in generated code. */
export function rustType(type: IdlType): string {
  switch (type.kind) {
    case 'void':
      return '()';
    case 'bool':
      return 'bool';
    case 'u32':
      return 'u32';
    case 'u64':
      return 'u64';
    case 'i32':
      return 'i32';
    case 'i64':
      return 'i64';
    case 'address':
      return 'Address';
    case 'string':
      return 'String';
    case 'symbol':
      return 'Symbol';
    case 'bytes':
      return 'Bytes';
    case 'bytesn':
      return `BytesN<${type.n}>`;
    case 'timepoint':
      return 'Timepoint';
    case 'duration':
      return 'Duration';
    case 'option':
      return `Option<${rustType(type.of)}>`;
    case 'vec':
      return `Vec<${rustType(type.of)}>`;
    case 'tuple':
      return `(${type.of.map(rustType).join(', ')})`;
    case 'map':
      return `Map<${rustType(type.key)}, ${rustType(type.value)}>`;
    case 'type':
      return `super::${type.name}`;
    default:
      return '()';
  }
}

function docAttr(text: string | undefined, fallback: string): string {
  const body = text && text.trim().length > 0 ? text.trim() : fallback;
  return body
    .split('\n')
    .flatMap((line) => line.match(/.{1,96}(\s|$)/g) ?? [line])
    .map((l) => `#[doc = "${l.trim().replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"]`)
    .join('\n');
}

function renderStruct(def: IdlStruct): string {
  const lines: string[] = [];
  lines.push(docAttr(def.doc?.text, `Contract type \`${def.name}\`.`));
  lines.push('#[derive(Clone, Debug, PartialEq, Eq)]');
  lines.push(`pub struct ${def.name} {`);
  for (const f of def.fields) {
    if (f.doc?.text) lines.push(docAttr(f.doc.text, f.doc.text));
    lines.push(`    pub ${f.name}: ${rustType(f.type)},`);
  }
  lines.push('}');
  return lines.join('\n');
}

function renderEnum(def: IdlEnum): string {
  const hasPayload = def.variants.some((v) => v.fields && v.fields.length > 0);
  const lines: string[] = [];
  lines.push(docAttr(def.doc?.text, `Contract enum \`${def.name}\`.`));
  if (hasPayload) {
    // Payload variants keep their tuple shape; the discriminant is the
    // declaration order, which is how Soroban encodes them.
    lines.push('#[derive(Clone, Debug, PartialEq, Eq)]');
    lines.push(`pub enum ${def.name} {`);
    for (const v of def.variants) {
      if (v.doc?.text) lines.push(docAttr(v.doc.text, v.doc.text));
      const payload = v.fields && v.fields.length > 0 ? `(${v.fields.map((f) => rustType(f.type)).join(', ')})` : '';
      lines.push(`    /// Wire ordinal: ${v.value}`);
      lines.push(`    ${v.name}${payload},`);
    }
    lines.push('}');
    lines.push('');
    lines.push(`impl ${def.name} {`);
    lines.push('    /// Wire ordinal for this variant, by declaration order.');
    lines.push('    pub fn ordinal(&self) -> u32 {');
    lines.push('        match self {');
    for (const v of def.variants) {
      const payload = v.fields && v.fields.length > 0 ? `(${v.fields.map(() => '_').join(', ')})` : '';
      lines.push(`            ${def.name}::${v.name}${payload} => ${v.value},`);
    }
    lines.push('        }');
    lines.push('    }');
    lines.push('');
    lines.push('    /// Every variant name, in wire order.');
    lines.push("    pub const VARIANTS: &'static [&'static str] = &[");
    for (const v of def.variants) lines.push(`        "${v.name}",`);
    lines.push('    ];');
    lines.push('}');
    return lines.join('\n');
  }
  lines.push('#[derive(Clone, Copy, Debug, PartialEq, Eq)]');
  lines.push(`pub enum ${def.name} {`);
  for (const v of def.variants) {
    if (v.doc?.text) lines.push(docAttr(v.doc.text, v.doc.text));
    lines.push(`    /// Wire ordinal: ${v.value}`);
    lines.push(`    ${v.name} = ${v.value},`);
  }
  lines.push('}');
  lines.push('');
  lines.push(`impl ${def.name} {`);
  lines.push('    /// Decode a wire ordinal, or `None` when the ordinal is unknown.');
  lines.push(`    pub fn from_ordinal(value: u32) -> Option<Self> {`);
  lines.push('        match value {');
  for (const v of def.variants) {
    lines.push(`            ${v.value} => Some(${def.name}::${v.name}),`);
  }
  lines.push('            _ => None,');
  lines.push('        }');
  lines.push('    }');
  lines.push('');
  lines.push('    /// Wire ordinal for this variant.');
  lines.push('    pub fn ordinal(self) -> u32 {');
  lines.push('        self as u32');
  lines.push('    }');
  lines.push('}');
  return lines.join('\n');
}

function generateTypes(idl: ContractIdl): string {
  const parts: string[] = [
    banner('//'),
    '//',
    '// Contract data types. Field names are preserved verbatim from the IDL so a',
    '// value can be passed to the RPC layer without a translation step.',
    '',
    '#![allow(clippy::derive_partial_eq_without_eq)]',
    '',
    'use soroban_sdk::{Address, Bytes, BytesN, Duration, Map, String, Symbol, Timepoint, Vec};',
    '',
  ];
  for (const def of idl.types) {
    parts.push(def.kind === 'struct' ? renderStruct(def) : renderEnum(def), '');
  }
  parts.push(footer('//', idl));
  return `${parts.join('\n').trimEnd()}\n`;
}

function generateErrors(idl: ContractIdl): string {
  const lines: string[] = [
    banner('//'),
    '//',
    '// Contract error codes. Codes are stable wire values; never renumber them.',
    '',
    '/// Numeric contract error codes, ascending by wire value.',
    '#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
    '#[repr(u32)]',
    'pub enum ContractErrorCode {',
  ];
  for (const e of idl.errors) {
    lines.push(docAttr(e.doc?.text, `Contract error \`${e.name}\`.`));
    lines.push(`    ${e.name} = ${e.code},`);
  }
  lines.push('}');
  lines.push('');
  lines.push('impl ContractErrorCode {');
  lines.push('    /// Every error code known to this SDK, in wire order.');
  lines.push('    pub const ALL: [ContractErrorCode; ' + String(idl.errors.length) + '] = [');
  for (const e of idl.errors) {
    lines.push(`        ContractErrorCode::${e.name},`);
  }
  lines.push('    ];');
  lines.push('');
  lines.push('    /// Decode a wire code, or `None` when the code is unknown to this SDK.');
  lines.push('    pub fn from_code(code: u32) -> Option<Self> {');
  lines.push('        match code {');
  for (const e of idl.errors) {
    lines.push(`            ${e.code} => Some(ContractErrorCode::${e.name}),`);
  }
  lines.push('            _ => None,');
  lines.push('        }');
  lines.push('    }');
  lines.push('');
  lines.push('    /// Wire code for this error.');
  lines.push('    pub fn code(self) -> u32 {');
  lines.push('        self as u32');
  lines.push('    }');
  lines.push('');
  lines.push('    /// Canonical name, identical to the Rust variant name.');
  lines.push('    pub fn name(self) -> &\'static str {');
  lines.push('        match self {');
  for (const e of idl.errors) {
    lines.push(`            ContractErrorCode::${e.name} => "${e.name}",`);
  }
  lines.push('        }');
  lines.push('    }');
  lines.push('');
  lines.push('    /// Human-readable description including the wire code.');
  lines.push('    pub fn describe(self) -> String {');
  lines.push('        alloc::format!("{} ({})", self.name(), self.code())');
  lines.push('    }');
  lines.push('}');
  lines.push('');
  lines.push(footer('//', idl));
  return lines.join('\n');
}

function pascalEvent(name: string): string {
  return name
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

function generateEvents(idl: ContractIdl): string {
  const lines: string[] = [
    banner('//'),
    '//',
    '// Typed contract events. In Soroban the trailing topic is the event',
    '// discriminator; `topics` below lists the discriminator-relative topics.',
    '',
    '/// Event name constants, exactly as published on chain.',
    'pub mod event_name {',
  ];
  for (const e of idl.events) {
    lines.push(`    /// \`${e.name}\``);
    lines.push(`    pub const ${e.name.toUpperCase()}: &str = "${e.name}";`);
  }
  lines.push('}');
  lines.push('');
  lines.push('/// All event names known to this SDK, in alphabetical order.');
  lines.push(`pub const ALL_EVENT_NAMES: [&str; ${idl.events.length}] = [`);
  for (const e of idl.events) {
    lines.push(`    "${e.name}",`);
  }
  lines.push('];');
  lines.push('');
  for (const e of idl.events) {
    const cls = pascalEvent(e.name);
    lines.push('/// Decoded contract event payload.');
    lines.push('#[derive(Clone, Debug, PartialEq)]');
    lines.push(`pub struct ${cls}Event {`);
    e.topics.forEach((t, i) => lines.push(`    /// Topic ${i + 1}: ${renderType(t)}`));
    e.topics.forEach((_t, i) => lines.push(`    pub topic${i + 1}: ${rustType(e.topics[i])},`));
    e.data.forEach((t, i) => lines.push(`    /// Data ${i + 1}: ${renderType(t)}`));
    e.data.forEach((_t, i) => lines.push(`    pub data${i + 1}: ${rustType(e.data[i])},`));
    lines.push('}');
    lines.push('');
    lines.push(`impl ${cls}Event {`);
    lines.push('    /// The on-chain event name for this payload.');
    lines.push(`    pub const NAME: &'static str = "${e.name}";`);
    lines.push('}');
    lines.push('');
  }
  lines.push(footer('//', idl));
  return lines.join('\n');
}

function generateClient(idl: ContractIdl): string {
  const fns = projectFunctions(idl, 'snake_rust');
  const lines: string[] = [
    banner('//'),
    '//',
    `// Type-safe client for the ${idl.contract.name} Soroban contract.`,
    '//',
    '// The client is transport-agnostic: supply any implementation of',
    '// `ContractTransport`. Argument and return types come straight from the',
    '// contract IDL, so an unregenerated contract change fails to compile.',
    '',
    'use super::types::*;',
    '',
    '/// Transport contract: invokes a contract function and returns its result.',
    'pub trait ContractTransport {',
    '    /// Invoke `method` (the on-chain function name) with positional `args`.',
    '    fn invoke(&self, method: &str, args: Vec<Val>) -> Result<Val, TransportError>;',
    '}',
    '',
    '/// Opaque Soroban value passed across the transport boundary.',
    'pub type Val = soroban_sdk::Val;',
    '',
    '/// Error returned by a [`ContractTransport`] implementation.',
    '#[derive(Clone, Debug, PartialEq, Eq)]',
    'pub enum TransportError {',
    '    /// The transport could not reach the RPC endpoint.',
    '    Rpc(String),',
    '    /// The contract reverted; `code` is the on-chain error code.',
    '    Contract(u32),',
    '    /// The SDK and the deployed contract disagree about the surface.',
    '    Decode(String),',
    '}',
    '',
    `/// Generated client for ${idl.contract.name}.`,
    'pub struct GeneratedClient<T: ContractTransport> {',
    '    contract_id: Address,',
    '    transport: T,',
    '}',
    '',
    'impl<T: ContractTransport> GeneratedClient<T> {',
    '    /// Bind a transport to a contract address.',
    '    pub fn new(contract_id: Address, transport: T) -> Self {',
    '        Self { contract_id, transport }',
    '    }',
    '',
    '    /// The contract address this client invokes.',
    '    pub fn contract_id(&self) -> &Address {',
    '        &self.contract_id',
    '    }',
    '',
    '    /// All contract function names this client exposes.',
    `    pub const FUNCTION_NAMES: [&'static str; ${fns.length}] = [`,
  ];
  for (const fn of fns) {
    lines.push(`        "${fn.wireName}",`);
  }
  lines.push('    ];');
  lines.push('');
  for (const fn of fns) {
    const params = fn.args.map((a) => `${a.paramName}: ${rustType(a.type)}`).join(', ');
    const sig = fn.args.length > 0 ? `&self, ${params}` : '&self';
    const ret = fn.output.kind === 'void' ? 'Result<(), TransportError>' : `Result<${rustType(fn.output)}, TransportError>`;
    const label = fn.needsTransaction ? 'State-changing' : 'Read-only';
    lines.push(docAttr(fn.doc, `${label} contract function \`${fn.wireName}\`.`));
    if (fn.args.length > 0) {
      lines.push(...fn.args.map((a) => docAttr(`Wire name \`${a.wireName}\` (${renderType(a.type)}).`, `Parameter \`${a.paramName}\`.`)));
    }
    lines.push(`    pub fn ${fn.wireName}(${sig}) -> ${ret} {`);
    lines.push(`        let args: Vec<Val> = vec![${fn.args.map((a) => `Into::<Val>::into(${a.paramName})`).join(', ')}];`);
    lines.push(`        self.transport.invoke("${fn.wireName}", args)`);
    lines.push('    }');
    lines.push('');
  }
  lines.push('}');
  lines.push('');
  lines.push(footer('//', idl));
  return lines.join('\n');
}

function generateMod(idl: ContractIdl): string {
  return [
    banner('//'),
    '//',
    '// Generated contract binding. Include it with `mod generated;` and re-export',
    '// the pieces you need from your crate root.',
    '',
    'mod client;',
    'mod errors;',
    'mod events;',
    'mod types;',
    '',
    'pub use client::{ContractTransport, GeneratedClient, TransportError, Val};',
    'pub use errors::ContractErrorCode;',
    'pub use events::{event_name, ALL_EVENT_NAMES};',
    'pub use types::*;',
    '',
    footer('//', idl),
    '',
  ].join('\n');
}

/** Generate every Rust artefact for the given IDL. */
/**
 * Format a Rust module set with `rustfmt` so the output satisfies
 * `cargo fmt --check` in CI.
 *
 * The whole set is staged in a temporary directory first: `mod x;` statements
 * only resolve when the sibling modules are present, so formatting the files
 * one at a time would leave `mod.rs` unformatted. Formatting is best-effort —
 * when `rustfmt` is unavailable the unformatted source is still emitted, so
 * generation never depends on a Rust toolchain.
 */
function rustfmtAll(files: Array<{ name: string; contents: string }>): string[] {
  const tmpDir = path.join(os.tmpdir(), `sdk-codegen-${process.pid}-${Math.random().toString(36).slice(2)}`);
  try {
    fs.mkdirSync(tmpDir, { recursive: true });
    for (const f of files) fs.writeFileSync(path.join(tmpDir, f.name), f.contents, 'utf8');
    // Point rustfmt at the repository's own rustfmt.toml; the staging directory
    // lives outside the repository and would otherwise pick up rustfmt's
    // defaults, disagreeing with `cargo fmt --check`.
    const config = path.join(REPO_ROOT, 'rustfmt.toml');
    // rustfmt rejects a bare directory; hand it the module root so that
    // `mod x;` statements pull in the staged siblings.
    const args = ['--edition', '2021', ...(fs.existsSync(config) ? ['--config-path', REPO_ROOT] : []), path.join(tmpDir, 'mod.rs')];
    const res = spawnSync('rustfmt', args, { encoding: 'utf8' });
    if (res.status !== 0) return files.map((f) => f.contents);
    return files.map((f) => {
      const staged = path.join(tmpDir, f.name);
      if (!fs.existsSync(staged)) return f.contents;
      const formatted = fs.readFileSync(staged, 'utf8');
      return formatted.trim().length > 0 ? formatted : f.contents;
    });
  } catch {
    return files.map((f) => f.contents);
  } finally {
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // Best-effort cleanup of the staging directory.
    }
  }
}

export function generateRust(idl: ContractIdl, outDir: string = RUST_OUTPUT_DIR): GeneratedFile[] {
  const staged = [
    { name: 'types.rs', contents: generateTypes(idl) },
    { name: 'errors.rs', contents: generateErrors(idl) },
    { name: 'events.rs', contents: generateEvents(idl) },
    { name: 'client.rs', contents: generateClient(idl) },
    { name: 'mod.rs', contents: generateMod(idl) },
  ];
  const formatted = rustfmtAll(staged);
  return staged.map((f, i) => ({ path: joinPath(outDir, f.name), contents: formatted[i] }));
}
