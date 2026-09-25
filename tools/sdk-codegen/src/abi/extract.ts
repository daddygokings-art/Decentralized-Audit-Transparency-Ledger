import * as crypto from 'crypto';
import * as fs from 'fs';
import { ContractIdl, IdlEnum, IdlError, IdlEvent, IdlField, IdlFunction, IdlInput, IdlStruct, IdlType, IdlTypeDef, isMutatingFunction, walkTypeRefs } from './types';

export interface ExtractResult {
  idl: ContractIdl;
  /** Non-fatal problems (unresolved event payload entries, unknown type text). */
  warnings: string[];
  /** SHA-256 of the analysed source, recorded in the IDL for drift detection. */
  sourceDigest: string;
}

export interface ExtractOptions {
  /** Contract source to analyse. Defaults to the repository's `src/lib.rs`. */
  source?: string;
  contractName?: string;
  crateName?: string;
  contractVersion?: string;
  sorobanSdkVersion?: string;
  specVersion?: string;
}

const DEFAULT_SOURCE = 'src/lib.rs';
const DEFAULT_CONTRACT = 'AuditLedger';
const CONTRACT_DECL_RE = /#\[contract\]\s*pub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)/;

/** Contract name declared by `#[contract] pub struct <Name>`, when present. */
export function detectContractName(src: string): string | undefined {
  const m = CONTRACT_DECL_RE.exec(src);
  return m ? m[1] : undefined;
}
const DEFAULT_CRATE = 'audit-ledger';
const DEFAULT_CONTRACT_VERSION = '0.1.0';
const DEFAULT_SOROBAN = '27.0.6';
const DEFAULT_SPEC_VERSION = '1.0.0';
const TOOL_VERSION = 'sdk-codegen 1.0.0';

// ── Source preprocessing ────────────────────────────────────────────────────

/** Replace comments with equivalent whitespace so byte offsets stay aligned. */
function blankComments(src: string): string {
  let out = '';
  let i = 0;
  const n = src.length;
  let inString: '"' | undefined;
  let inChar: "'" | undefined;
  let inLineComment = false;
  let inBlockComment = false;
  let blockDepth = 0;

  while (i < n) {
    const c = src[i];
    const next = src[i + 1];

    if (inLineComment) {
      if (c === '\n') {
        inLineComment = false;
        out += c;
      } else {
        out += ' ';
      }
      i += 1;
      continue;
    }
    if (inBlockComment) {
      if (c === '/' && next === '*') {
        blockDepth += 1;
        out += '  ';
        i += 2;
        continue;
      }
      if (c === '*' && next === '/') {
        blockDepth -= 1;
        out += '  ';
        i += 2;
        if (blockDepth === 0) inBlockComment = false;
        continue;
      }
      out += c === '\n' ? '\n' : ' ';
      i += 1;
      continue;
    }
    if (inString) {
      out += c;
      if (c === '\\') {
        out += next ?? '';
        i += 2;
        continue;
      }
      if (c === inString) inString = undefined;
      i += 1;
      continue;
    }
    if (inChar) {
      out += c;
      if (c === '\\') {
        out += next ?? '';
        i += 2;
        continue;
      }
      if (c === inChar) inChar = undefined;
      i += 1;
      continue;
    }
    if (c === '/' && next === '/') {
      inLineComment = true;
      out += '  ';
      i += 2;
      continue;
    }
    if (c === '/' && next === '*') {
      inBlockComment = true;
      blockDepth = 1;
      out += '  ';
      i += 2;
      continue;
    }
    if (c === '"') inString = '"';
    if (c === "'" && /[A-Za-z]/.test(next ?? '')) inChar = "'";
    out += c;
    i += 1;
  }
  return out;
}

/** Extract the `///` doc comment block immediately preceding `index`. */
function docBefore(src: string, index: number): string {
  const before = src.slice(0, index);
  const lines = before.split('\n');
  const collected: string[] = [];
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    const line = lines[i].trim();
    if (line.startsWith('///')) {
      collected.unshift(line.replace(/^\/\/\/?\s?/, '').trim());
    } else if (line === '') {
      if (collected.length > 0) break;
    } else if (line.startsWith('#[') || line.startsWith('pub ') || line.endsWith(',')) {
      // Attributes and the previous declaration — keep walking.
      continue;
    } else {
      break;
    }
  }
  return collected.join(' ').trim();
}

/** Index of the `}` matching the `{` at `open`. */
function matchBrace(src: string, open: number): number {
  let depth = 0;
  for (let i = open; i < src.length; i += 1) {
    if (src[i] === '{') depth += 1;
    else if (src[i] === '}') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/** Index of the `)` matching the `(` at `open`. */
function matchParen(src: string, open: number): number {
  let depth = 0;
  for (let i = open; i < src.length; i += 1) {
    if (src[i] === '(') depth += 1;
    else if (src[i] === ')') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/** Split a comma-separated list at nesting depth zero. */
function splitTopLevel(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = '';
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === '<' || c === '(' || c === '[') depth += 1;
    else if (c === '>' || c === ')' || c === ']') depth -= 1;
    if (c === ',' && depth === 0) {
      parts.push(current.trim());
      current = '';
      continue;
    }
    current += c;
  }
  if (current.trim().length > 0) parts.push(current.trim());
  return parts;
}

// ── Type mapping ────────────────────────────────────────────────────────────

const SCALARS: Record<string, IdlType['kind']> = {
  bool: 'bool',
  u32: 'u32',
  u64: 'u64',
  i32: 'i32',
  i64: 'i64',
  u8: 'u32',
  u16: 'u32',
  i8: 'i32',
  i16: 'i32',
  Address: 'address',
  String: 'string',
  str: 'string',
  Symbol: 'symbol',
  Bytes: 'bytes',
  Timepoint: 'timepoint',
  Duration: 'duration',
  Env: 'void',
};

export class UnknownTypeError extends Error {}

/** Translate a Rust type expression into an IDL type node. */
export function parseRustType(text: string): IdlType {
  const t = text.trim().replace(/\s+/g, ' ');
  if (t.length === 0) throw new UnknownTypeError('empty type');

  // Strip reference sugar: `&T`, `&mut T`.
  const inner = t.replace(/^&\s*(mut\s+)?/, '').trim();
  if (inner === '()' || inner === '') return { kind: 'void' };

  const bytesn = /^BytesN<\s*(\d+)\s*>$/.exec(inner);
  if (bytesn) return { kind: 'bytesn', n: Number(bytesn[1]) };

  const bytesnShort = /^BytesN<\s*([A-Z_][A-Za-z0-9_]*)\s*>$/.exec(inner);
  if (bytesnShort) {
    const named = NAMED_BYTESN[bytesnShort[1]];
    if (named) return { kind: 'bytesn', n: named };
    throw new UnknownTypeError(`BytesN length is a non-literal: ${inner}`);
  }

  for (const [prefix, kind] of [
    ['Option<', 'option'],
    ['Vec<', 'vec'],
  ] as const) {
    if (inner.startsWith(prefix) && inner.endsWith('>')) {
      const of = parseRustType(inner.slice(prefix.length, -1));
      return { kind, of } as IdlType;
    }
  }

  if (inner.startsWith('BTreeMap<') && inner.endsWith('>')) {
    const parts = splitTopLevel(inner.slice('BTreeMap<'.length, -1));
    if (parts.length === 2) {
      return { kind: 'map', key: parseRustType(parts[0]), value: parseRustType(parts[1]) };
    }
  }
  if (inner.startsWith('HashMap<') && inner.endsWith('>')) {
    const parts = splitTopLevel(inner.slice('HashMap<'.length, -1));
    if (parts.length === 2) {
      return { kind: 'map', key: parseRustType(parts[0]), value: parseRustType(parts[1]) };
    }
  }

  if (inner.startsWith('(') && inner.endsWith(')') && matchParen(inner, 0) === inner.length - 1) {
    const members = splitTopLevel(inner.slice(1, -1));
    return { kind: 'tuple', of: members.map((m) => parseRustType(m)) };
  }

  if (inner.startsWith('[') && inner.endsWith(']')) {
    return { kind: 'vec', of: parseRustType(inner.slice(1, -1).replace(/;\s*\d+\s*$/, '')) };
  }

  if (/^[A-Z][A-Za-z0-9_]*$/.test(inner)) {
    const scalar = SCALARS[inner];
    if (scalar) return { kind: scalar } as IdlType;
    return { kind: 'type', name: inner };
  }

  const scalar = SCALARS[t];
  if (scalar) return { kind: scalar } as IdlType;
  throw new UnknownTypeError(`unsupported Rust type: ${text}`);
}

const NAMED_BYTESN: Record<string, number> = {
  Hash: 32,
  U32: 4,
  U64: 8,
  SIGNATURE: 64,
  ADDRESS: 32,
  PUBLIC_KEY: 32,
};

// ── Declaration parsing ─────────────────────────────────────────────────────

const ATTR = String.raw`#\[[^\]]*\]`;
const DECL_RE = new RegExp(
  `${ATTR}\\s*pub\\s+(struct|enum)\\s+([A-Za-z_][A-Za-z0-9_]*)\\s*\\{`,
  'g',
);

function lineOf(src: string, index: number): number {
  return src.slice(0, index).split('\n').length;
}

function parseContractTypes(src: string): { types: IdlTypeDef[]; warnings: string[] } {
  const types: IdlTypeDef[] = [];
  const warnings: string[] = [];
  DECL_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = DECL_RE.exec(src)) !== null) {
    const kind = m[1] as 'struct' | 'enum';
    const name = m[2];
    const open = src.indexOf('{', m.index + m[0].length - 1);
    const close = matchBrace(src, open);
    if (close < 0) {
      warnings.push(`unbalanced braces for ${name}; skipped`);
      continue;
    }
    const body = src.slice(open + 1, close);
    const line = lineOf(src, m.index);
    const docText = docBefore(src, m.index);
    const doc = docText.length > 0 ? { line, text: docText } : undefined;

    if (kind === 'struct') {
      const fields: IdlField[] = [];
      for (const raw of splitTopLevel(body)) {
        const entry = raw.trim();
        if (entry.length === 0) continue;
        const fm = /(?:pub\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(entry);
        if (!fm) {
          warnings.push(`could not parse field of ${name}: ${entry.slice(0, 60)}`);
          continue;
        }
        try {
          fields.push({ name: fm[1], type: parseRustType(fm[2]) });
        } catch (err) {
          warnings.push(`field ${name}.${fm[1]}: ${(err as Error).message}`);
        }
      }
      const def: IdlStruct = { kind: 'struct', name, fields, ...(doc ? { doc } : {}) };
      types.push(def);
    } else {
      // Enums carrying payloads (storage keys such as `DataKey`) are internal
      // plumbing rather than part of the contract's value surface, so they are
      // skipped once instead of warned about variant by variant.
      const entries = splitTopLevel(body).map((e) => e.trim()).filter((e) => e.length > 0);
      const variants: IdlEnum['variants'] = [];
      let next = 0;
      for (const entry of entries) {
        // Unit variant: `Pause` or `Pause = 3`.
        const unit = /^([A-Za-z_][A-Za-z0-9_]*)\s*(?:=\s*(\d+))?$/.exec(entry);
        if (unit) {
          const value = unit[2] === undefined ? next : Number(unit[2]);
          next = value + 1;
          variants.push({ name: unit[1], value });
          continue;
        }
        // Tuple variant: `AddOwner(Address)`.
        const tuple = /^([A-Za-z_][A-Za-z0-9_]*)\s*(?:=\s*(\d+))?\s*\(([\s\S]*)\)$/.exec(entry);
        if (tuple) {
          const value = tuple[2] === undefined ? next : Number(tuple[2]);
          next = value + 1;
          const fields: IdlField[] = splitTopLevel(tuple[3]).map((member, i) => {
            const name = `field_${i}`;
            try {
              return { name, type: parseRustType(member) };
            } catch (err) {
              warnings.push(`variant ${name}.${tuple[1]}.${i}: ${(err as Error).message}`);
              return { name, type: { kind: 'bytes' } };
            }
          });
          variants.push({ name: tuple[1], value, fields });
          continue;
        }
        // Struct variant: `Named { label: Symbol, count: u32 }`.
        const structVariant = /^([A-Za-z_][A-Za-z0-9_]*)\s*(?:=\s*(\d+))?\s*\{([\s\S]*)\}$/.exec(entry);
        if (structVariant) {
          const value = structVariant[2] === undefined ? next : Number(structVariant[2]);
          next = value + 1;
          const fields: IdlField[] = [];
          for (const member of splitTopLevel(structVariant[3])) {
            const decl = member.trim();
            if (decl.length === 0) continue;
            const fm = /^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(decl);
            if (!fm) {
              warnings.push(`could not parse field of ${name}.${structVariant[1]}: ${decl.slice(0, 40)}`);
              continue;
            }
            try {
              fields.push({ name: fm[1], type: parseRustType(fm[2]) });
            } catch (err) {
              warnings.push(`field ${structVariant[1]}.${fm[1]}: ${(err as Error).message}`);
              fields.push({ name: fm[1], type: { kind: 'bytes' } });
            }
          }
          variants.push({ name: structVariant[1], value, fields });
          continue;
        }
        warnings.push(`could not parse variant of ${name}: ${entry.slice(0, 60)}`);
      }
      const def: IdlEnum = { kind: 'enum', name, variants, ...(doc ? { doc } : {}) };
      types.push(def);
    }
  }
  return { types, warnings };
}

const ERROR_RE = /pub\s+enum\s+ContractError\s*\{/;

/** Parse `ContractError` variants and their numeric wire codes. */
export function parseContractErrors(src: string): { errors: IdlError[]; warnings: string[] } {
  const errors: IdlError[] = [];
  const warnings: string[] = [];
  const m = ERROR_RE.exec(src);
  // A contract without a `ContractError` enum simply has no error surface.
  if (!m) return { errors, warnings: [] };
  const open = src.indexOf('{', m.index);
  const close = matchBrace(src, open);
  if (close < 0) return { errors, warnings: ['unbalanced ContractError braces'] };
  const body = src.slice(open + 1, close);

  for (const raw of splitTopLevel(body)) {
    const entry = raw.trim();
    if (entry.length === 0) continue;
    const vm = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*$/.exec(entry);
    if (!vm) {
      warnings.push(`could not parse ContractError variant: ${entry.slice(0, 60)}`);
      continue;
    }
    errors.push({ name: vm[1], code: Number(vm[2]) });
  }
  errors.sort((a, b) => a.code - b.code);
  return { errors, warnings };
}

const ANY_FN_RE = /\n[ \t]*(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\(/g;

/**
 * Return type of every function in the file, keyed by name.
 *
 * Private helpers are excluded from the contract's public function list but
 * are still callable from function bodies, so expression inference needs them.
 */
export function parseHelperReturnTypes(src: string): Map<string, IdlType> {
  const returns = new Map<string, IdlType>();
  let m: RegExpExecArray | null;
  ANY_FN_RE.lastIndex = 0;
  while ((m = ANY_FN_RE.exec(src)) !== null) {
    const parenOpen = src.indexOf('(', m.index + m[0].length - 1);
    const parenClose = matchParen(src, parenOpen);
    if (parenClose < 0) continue;
    const retMatch = /^\s*->\s*([^{]+?)\s*\{/.exec(src.slice(parenClose + 1, parenClose + 400));
    if (!retMatch) continue;
    try {
      returns.set(m[1], parseRustType(retMatch[1]));
    } catch {
      // Unsupported helper return type — the caller falls back to its own inference.
    }
  }
  return returns;
}

const FN_RE = /\n    pub fn ([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\(/g;

/** Parse the `#[contractimpl] impl <Contract>` block into function descriptors. */
export function parseContractFunctions(src: string, contractName: string): { functions: IdlFunction[]; warnings: string[] } {
  const functions: IdlFunction[] = [];
  const warnings: string[] = [];

  const implRe = new RegExp(`#\\[contractimpl\\]\\s*impl\\s+${contractName}\\s*\\{`);
  const im = implRe.exec(src);
  if (!im) return { functions, warnings: [`no #[contractimpl] impl ${contractName} block found`] };
  const open = src.indexOf('{', im.index);
  const close = matchBrace(src, open);
  if (close < 0) return { functions, warnings: [`unbalanced impl ${contractName} braces`] };
  const body = src.slice(open + 1, close);

  FN_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  const scans: Array<{ name: string; scan: BodyScan }> = [];
  while ((m = FN_RE.exec(body)) !== null) {
    const name = m[1];
    scans.push({ name, scan: scanBody(body, m.index + m[0].length) });
    const line = lineOf(src, open + m.index);
    const docText = docBefore(src, open + m.index);
    const doc = docText.length > 0 ? { line, text: docText } : undefined;

    const parenOpen = body.indexOf('(', m.index + m[0].length - 1);
    const parenClose = matchParen(body, parenOpen);
    if (parenClose < 0) {
      warnings.push(`could not read parameter list of ${name}`);
      continue;
    }
    const args = splitTopLevel(body.slice(parenOpen + 1, parenClose));

    const after = body.slice(parenClose + 1, parenClose + 400);
    const retMatch = /^\s*->\s*([^{]+?)\s*\{/.exec(after);
    let output: IdlType = { kind: 'void' };
    if (retMatch) {
      try {
        output = parseRustType(retMatch[1]);
      } catch (err) {
        warnings.push(`return type of ${name}: ${(err as Error).message}`);
        output = { kind: 'bytes' };
      }
    }

    const inputs: IdlInput[] = [];
    for (const [i, arg] of args.entries()) {
      const entry = arg.trim();
      if (entry.length === 0) continue;
      // `env: Env` is the host handle, not a contract parameter.
      if (/^env\s*:\s*Env$/.test(entry)) continue;
      const am = /^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(entry);
      if (!am) {
        warnings.push(`could not parse parameter ${i} of ${name}: ${entry.slice(0, 60)}`);
        continue;
      }
      try {
        inputs.push({ name: am[1], type: parseRustType(am[2]) });
      } catch (err) {
        warnings.push(`parameter ${name}.${am[1]}: ${(err as Error).message}`);
      }
    }

    functions.push({
      name,
      inputs,
      output,
      mutating: isMutatingFunction(name),
      ...(doc ? { doc } : {}),
    });
  }

  // The body is authoritative when readable, and propagates through delegation.
  // The naming heuristic only decides functions whose source could not be read.
  const mutating = resolveMutating(scans);
  for (const fn of functions) {
    if (mutating.has(fn.name)) fn.mutating = mutating.get(fn.name) as boolean;
  }
  return { functions, warnings };
}

/**
 * True when a function body performs a storage write, extends a persistent
 * collection, or emits an event.
 *
 * The `get*`/`query*` naming convention in {@link isMutatingFunction} is a
 * convention, not a guarantee: a function such as `load` is read-only despite
 * its name, and `set_metadata` is mutating despite not matching any prefix.
 * Reading the body removes the guesswork for the overwhelming majority of
 * functions while keeping the naming heuristic as a fallback.
 */
const STATE_MUTATOR = /\.\s*(?:set|set_typed|remove|extend|push_back|push_front|pop_front|pop_back|append|insert|replace|publish|retain)\s*\(/;

interface BodyScan {
  /** Body text, or undefined when the source could not be read end to end. */
  text?: string;
}

/** Locate a function body, returning undefined when braces do not balance. */
function scanBody(body: string, from: number): BodyScan {
  const braceOpen = body.indexOf('{', from);
  if (braceOpen < 0) return {};
  const braceClose = matchBrace(body, braceOpen);
  if (braceClose < 0) return {};
  return { text: body.slice(braceOpen, braceClose) };
}

/**
 * Resolve `mutating` for every contract function.
 *
 * A function is mutating when its own body writes state or emits an event, or
 * when it delegates to a function that does. `log_event` is a one-line wrapper
 * around `log_event_with_hierarchy`, so name prefixes and direct body scans
 * both mislabel it; following `Self::other(` calls to a fixpoint does not.
 *
 * Note that a getter may still be mutating: several `get_*` functions perform
 * a periodic, conditional TTL-cleanup write. The flag means "may change state
 * or emit events", which is what decides whether a transaction must be
 * submitted rather than a read-only call.
 */
function resolveMutating(entries: Array<{ name: string; scan: BodyScan }>): Map<string, boolean> {
  const selfWrites = new Map<string, boolean>();
  const delegates = new Map<string, string[]>();

  for (const { name, scan } of entries) {
    if (scan.text === undefined) continue;
    selfWrites.set(name, STATE_MUTATOR.test(scan.text));
    const calls = new Set<string>();
    const callRe = /\bSelf::([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;
    let m: RegExpExecArray | null;
    while ((m = callRe.exec(scan.text)) !== null) calls.add(m[1]);
    delegates.set(name, [...calls]);
  }

  const resolved = new Map<string, boolean>();
  for (const { name } of entries) resolved.set(name, selfWrites.get(name) ?? false);

  // Widening to a fixpoint; the bound guards against a pathological cycle.
  for (let pass = 0; pass < entries.length + 1; pass += 1) {
    let changed = false;
    for (const { name, scan } of entries) {
      if (scan.text === undefined) continue;
      for (const callee of delegates.get(name) ?? []) {
        if (callee === name || !resolved.has(callee)) continue;
        if (resolved.get(callee) && !resolved.get(name)) {
          resolved.set(name, true);
          changed = true;
          break;
        }
      }
    }
    if (!changed) break;
  }
  return resolved;
}

// ── Event parsing ───────────────────────────────────────────────────────────

const PUBLISH_RE = /env\s*\.\s*events\s*\(\s*\)\s*\.\s*publish\s*\(/g;
const SYMBOL_LIT = /(?:Symbol::(?:new|short)\s*\(\s*&?env\s*,\s*"([^"]+)"\s*\)|symbol_short!\s*\(\s*"([^"]+)"\s*\))/g;

interface LocalScope {
  types: Map<string, IdlType>;
}

/** Known integer helpers whose result keeps the receiver's type. */
const SAME_TYPE_INT_METHODS = [
  'saturating_add', 'saturating_sub', 'saturating_mul', 'wrapping_add', 'wrapping_sub',
];

const DEFAULT_METHODS: Record<string, IdlType> = {
  checked_add: { kind: 'option', of: { kind: 'u32' } },
  checked_sub: { kind: 'option', of: { kind: 'u32' } },
  unwrap_or: { kind: 'u32' },
};

/** Element type produced by reading one item out of a container expression. */
function elementTypeOf(container: IdlType | undefined): IdlType | undefined {
  if (!container) return undefined;
  if (container.kind === 'vec') return container.of;
  if (container.kind === 'option') return elementTypeOf(container.of);
  return undefined;
}

/**
 * Resolve the type of an unannotated `let` initialiser.
 *
 * Only shapes whose type is unambiguous are handled; anything else is left
 * unresolved and surfaced as an extraction warning rather than guessed at.
 */
function inferLetInit(
  init: string,
  locals: Map<string, IdlType>,
  structFields: Map<string, Map<string, IdlType>>,
  fnReturns: Map<string, IdlType>,
): IdlType | undefined {
  let e = init.trim();
  while (e.endsWith(';')) e = e.slice(0, -1).trim();
  for (const call of PASSTHROUGH_CALLS) {
    if (e.endsWith(`.${call}`)) e = e.slice(0, -(call.length + 1)).trim();
  }
  if (e.length === 0) return undefined;

  if (e === 'true' || e === 'false') return { kind: 'bool' };
  if (/^-?\d+u?64$/.test(e)) return { kind: 'u64' };
  if (/^-?\d+$/.test(e)) return { kind: 'u32' };

  if (e === 'env.ledger().timestamp()' || e === 'env.ledger().sequence()') return { kind: 'u64' };

  const selfCall = /^Self::([A-Za-z_][A-Za-z0-9_]*)\s*\(/.exec(e);
  if (selfCall) return fnReturns.get(selfCall[1]);

  const defaultOf = /^([A-Z][A-Za-z0-9_]*)::default\s*\(\s*\)$/.exec(e);
  if (defaultOf) {
    try {
      return parseRustType(defaultOf[1]);
    } catch {
      return undefined;
    }
  }

  // `x.get(..)`, `x.iter()`, `x[i]` — one element out of a known container.
  const oneOf = /^([A-Za-z_][A-Za-z0-9_]*)\s*(?:\.\s*get\s*\(|\.\s*iter\s*\(\s*\)\s*\.?\s*next\s*\(\s*\)\s*\.?\s*unwrap\s*\(\s*\)|\[\s*\d+\s*\])/.exec(e);
  if (oneOf) return elementTypeOf(locals.get(oneOf[1]));

  // `let x = if cond { a } else { b };` — the branch tail expressions decide.
  const ifElse = /^if\b[\s\S]*?\{\s*([\s\S]*?)\s*\}\s*else\s*\{\s*([\s\S]*?)\s*\}$/.exec(e);
  if (ifElse) {
    for (const tail of [ifElse[1], ifElse[2]]) {
      const resolved = inferLetInit(tail, locals, structFields, fnReturns);
      if (resolved) return resolved;
    }
    return undefined;
  }

  // Plain identifier or struct-field read.
  return inferExprType(e, locals, structFields);
}

/** Element type of a `for` loop binding, given the iterated expression. */
function forLoopBinding(
  pattern: string,
  iterated: string,
  locals: Map<string, IdlType>,
): Array<[string, IdlType]> {
  const names = splitTopLevel(pattern).map((n) => n.trim()).filter((n) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(n));
  if (names.length === 0) return [];

  let containerExpr = iterated.trim();
  containerExpr = containerExpr.replace(/\.\s*(iter|into_iter)\s*\(\s*\)\s*$/, '');
  containerExpr = containerExpr.replace(/\.\s*enumerate\s*\(\s*\)\s*$/, '');
  containerExpr = containerExpr.replace(/\.\s*(rev|cloned|into_values)\s*\(\s*\)\s*$/, '');
  // `for x in a..b` is a numeric range; `for (a, b) in ..` a numeric pattern.
  if (/^-?\d+\s*\.\.=?/.test(containerExpr)) {
    return names.map((n) => [n, { kind: 'u32' }] as [string, IdlType]);
  }
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(containerExpr)) return [];

  const container = locals.get(containerExpr);
  if (!container) return [];
  const element = elementTypeOf(container);
  if (!element) return [];

  if (names.length === 1) return [[names[0], element]];
  if (element.kind === 'tuple' && element.of.length === names.length) {
    return names.map((n, i) => [n, element.of[i]] as [string, IdlType]);
  }
  return [];
}

/**
 * Build the variable -> type map visible at a point inside the contract.
 *
 * Sources, in decreasing precedence: annotated `let` bindings, `for` loop
 * patterns, tuple-destructuring `let`s, unannotated `let` initialisers, and
 * function parameters. Declarations are processed in source order so a later
 * binding can reference an earlier one.
 */
function collectLocals(
  prefix: string,
  structFields: Map<string, Map<string, IdlType>>,
  fnReturns: Map<string, IdlType>,
): LocalScope {
  const types = new Map<string, IdlType>();
  const set = (name: string, type: IdlType | undefined): void => {
    if (name.length > 0 && type) types.set(name, type);
  };

  // Function parameters, declared before any body statements.
  const fnRe = /\b(?:pub\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*(?:<[^>]*>)?\s*\(/g;
  let m: RegExpExecArray | null;
  while ((m = fnRe.exec(prefix)) !== null) {
    const parenOpen = prefix.indexOf('(', m.index + m[0].length - 1);
    const parenClose = matchParen(prefix, parenOpen);
    if (parenClose < 0) continue;
    for (const arg of splitTopLevel(prefix.slice(parenOpen + 1, parenClose))) {
      const entry = arg.trim();
      if (entry.length === 0) continue;
      const am = /^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/.exec(entry);
      if (!am) continue;
      try {
        set(am[1], parseRustType(am[2]));
      } catch {
        // Unsupported parameter type — resolved by the fallback chain below.
      }
    }
  }

  const annotate = (name: string, type: IdlType): void => {
    types.set(name, type);
  };

  // Annotated bindings: `let name: Type = ...` / `let mut name: Type = ...`.
  const annotatedRe = /\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^=;]+)=/g;
  while ((m = annotatedRe.exec(prefix)) !== null) {
    try {
      annotate(m[1], parseRustType(m[2]));
    } catch {
      // Unsupported annotation — resolved by the fallback chain below.
    }
  }

  // `for <pattern> in <expr> {`
  const forRe = /\bfor\s+([^{(]*?)\s+in\s+([^{]+?)\s*\{/g;
  while ((m = forRe.exec(prefix)) !== null) {
    const pattern = m[1].trim().replace(/^\(|\)$/g, '');
    for (const [name, type] of forLoopBinding(pattern, m[2], types)) set(name, type);
  }

  // Destructuring bindings: `let (a, b) = <expr>;`
  const destructureRe = /\blet\s+\(([^)]*)\)\s*=\s*([^;]+);/g;
  while ((m = destructureRe.exec(prefix)) !== null) {
    const element = inferLetInit(m[2], types, structFields, fnReturns);
    if (!element) continue;
    const names = splitTopLevel(m[1]).map((n) => n.trim()).filter(Boolean);
    if (element.kind === 'tuple' && element.of.length === names.length) {
      names.forEach((n, i) => set(n, element.of[i]));
    } else if (names.length === 1) {
      set(names[0], element);
    }
  }

  // Unannotated bindings: `let name = <expr>;` — resolved in source order.
  const plainRe = /\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([\s\S]+?);/g;
  while ((m = plainRe.exec(prefix)) !== null) {
    if (types.has(m[1])) continue;
    set(m[1], inferLetInit(m[2], types, structFields, fnReturns));
  }

  return { types };
}

/** Type-agnostic no-op method calls that can be stripped from a payload expression. */
const PASSTHROUGH_CALLS = ['clone()', 'unwrap()', 'unwrap_or_default()'];

/** Infer the type of a value expression appearing in an event payload. */
function inferExprType(
  expr: string,
  locals: Map<string, IdlType>,
  structFields: Map<string, Map<string, IdlType>>,
): IdlType | undefined {
  let e = expr.trim();

  for (const call of PASSTHROUGH_CALLS) {
    if (e.endsWith(`.${call}`)) e = e.slice(0, -(call.length + 1));
  }

  if (e === 'env.ledger().timestamp()') return { kind: 'u64' };
  if (e === 'env.ledger().sequence()') return { kind: 'u64' };
  if (e === 'true' || e === 'false') return { kind: 'bool' };

  const literal = /^-?\d+(u32|u64|usize|i32|i64)?$/.exec(e);
  if (literal) {
    if (literal[1] === 'i32' || literal[1] === 'i64') return { kind: 'i32' };
    return { kind: 'u32' };
  }

  const asCast = /^(.*?)\s+as\s+(u8|u16|u32|u64|i32|i64|usize)$/.exec(e);
  if (asCast) {
    const target = asCast[2];
    const base = inferExprType(asCast[1], locals, structFields);
    // A cast of a wider integer narrows it; keep the cast result regardless.
    if (target.startsWith('u') || target === 'usize') {
      return base && base.kind === 'u64' ? { kind: 'u64' } : { kind: 'u32' };
    }
    return { kind: 'i32' };
  }

  const mapCast = /^(.*?)\.map\s*\(\s*\|\s*[A-Za-z_][A-Za-z0-9_]*\s*\|\s*(.*?)\s*\)$/.exec(e);
  if (mapCast) {
    const inner = inferExprType(mapCast[1], locals, structFields);
    const mapped = inferExprType(mapCast[2], locals, structFields) ?? { kind: 'u32' };
    return inner ? { kind: 'option', of: mapped } : undefined;
  }

  const lenCast = /^(.*?)\.len\s*\(\s*\)\s+as\s+u32$/.exec(e);
  if (lenCast) {
    return inferExprType(lenCast[1], locals, structFields) ? { kind: 'u32' } : undefined;
  }

  const methodCall = /^([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\s*\(/.exec(e);
  if (methodCall) {
    if (SAME_TYPE_INT_METHODS.includes(methodCall[2])) return locals.get(methodCall[1]);
    return DEFAULT_METHODS[methodCall[2]];
  }

  const field = /^([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)$/.exec(e);
  if (field) {
    const structType = locals.get(field[1]);
    if (structType && structType.kind === 'type') {
      const f = structFields.get(structType.name)?.get(field[2]);
      if (f) return f;
    }
    return undefined;
  }

  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(e)) {
    return locals.get(e);
  }
  return undefined;
}

function parseEventPayload(
  src: string,
  contractTypes: IdlTypeDef[],
  fnReturns: Map<string, IdlType>,
): { events: IdlEvent[]; warnings: string[] } {
  const warnings: string[] = [];
  const events: IdlEvent[] = [];
  const seen = new Map<string, number>();

  const structFields = new Map<string, Map<string, IdlType>>();
  for (const def of contractTypes) {
    if (def.kind === 'struct') {
      const map = new Map<string, IdlType>();
      def.fields.forEach((f) => map.set(f.name, f.type));
      structFields.set(def.name, map);
    }
  }

  PUBLISH_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = PUBLISH_RE.exec(src)) !== null) {
    const callOpen = src.indexOf('(', m.index + m[0].length - 1);
    const callClose = matchParen(src, callOpen);
    if (callClose < 0) {
      warnings.push(`unbalanced events().publish() at offset ${m.index}`);
      continue;
    }
    const args = splitTopLevel(src.slice(callOpen + 1, callClose));
    if (args.length < 2) {
      warnings.push('events().publish() with fewer than 2 arguments; skipped');
      continue;
    }

    const topicArg = args[0].trim();
    if (!topicArg.startsWith('(')) {
      warnings.push('events().publish() topics must be a tuple literal; skipped');
      continue;
    }
    const topicSymbols: string[] = [];
    SYMBOL_LIT.lastIndex = 0;
    let sm: RegExpExecArray | null;
    while ((sm = SYMBOL_LIT.exec(topicArg)) !== null) topicSymbols.push(sm[1] ?? sm[2]);

    if (topicSymbols.length === 0) {
      warnings.push('events().publish() without a symbol topic; skipped');
      continue;
    }
    // Soroban convention: the trailing topic symbol is the event discriminator.
    const name = topicSymbols[topicSymbols.length - 1];
    const topics = topicSymbols.slice(0, -1).map<IdlType>(() => ({ kind: 'symbol' }));

    const dataArg = args[1].trim();
    const dataExprs = dataArg.startsWith('(') && matchParen(dataArg, 0) === dataArg.length - 1
      ? splitTopLevel(dataArg.slice(1, -1))
      : [dataArg];

    // Scope = enclosing `impl` body up to this call, so parameter and `let`
    // bindings from the current function are visible.
    const locals = collectLocals(src.slice(0, m.index), structFields, fnReturns);

    const data: IdlType[] = [];
    for (const expr of dataExprs) {
      const t = inferExprType(expr, locals.types, structFields);
      if (t) data.push(t);
      else warnings.push(`event "${name}": unresolved payload expression \`${expr.slice(0, 60)}\``);
    }

    const previous = seen.get(name);
    if (previous !== undefined) {
      events[previous] = { ...events[previous], doc: events[previous].doc };
      continue;
    }
    seen.set(name, events.length);
    events.push({ name, topics, data });
  }

  events.sort((a, b) => a.name.localeCompare(b.name));
  return { events, warnings };
}

// ── Public entry point ──────────────────────────────────────────────────────

/**
 * Extract a `ContractIdl` from Rust contract source.
 *
 * Types are narrowed to the transitive closure of everything reachable from the
 * contract functions and events, which keeps generated SDKs free of internal
 * `DataKey`-style plumbing that is not part of the public surface.
 */
export function extractIdlFromSource(sourceText: string, options: ExtractOptions = {}): ExtractResult {
  const src = blankComments(sourceText);
  const warnings: string[] = [];
  const contractName = options.contractName ?? detectContractName(src) ?? DEFAULT_CONTRACT;

  const { types: allTypes, warnings: typeWarnings } = parseContractTypes(src);
  warnings.push(...typeWarnings);
  const { errors, warnings: errorWarnings } = parseContractErrors(src);
  warnings.push(...errorWarnings);
  const { functions, warnings: fnWarnings } = parseContractFunctions(src, contractName);
  warnings.push(...fnWarnings);
  const fnReturns = parseHelperReturnTypes(src);
  for (const f of functions) fnReturns.set(f.name, f.output);
  const { events, warnings: eventWarnings } = parseEventPayload(src, allTypes, fnReturns);
  warnings.push(...eventWarnings);

  // Roots: types named by a function signature or event payload.
  const roots = new Set<string>();
  const note = (t: IdlType): void => {
    if (t.kind === 'type') roots.add(t.name);
    else if (t.kind === 'option' || t.kind === 'vec') note(t.of);
    else if (t.kind === 'tuple') t.of.forEach(note);
    else if (t.kind === 'map') {
      note(t.key);
      note(t.value);
    }
  };
  functions.forEach((fn) => {
    fn.inputs.forEach((i) => note(i.type));
    note(fn.output);
  });
  events.forEach((e) => {
    e.topics.forEach(note);
    e.data.forEach(note);
  });

  const byName = new Map(allTypes.map((t) => [t.name, t]));
  const reachable = new Set<string>([...roots].filter((n) => byName.has(n)));
  const queue = [...reachable];
  while (queue.length > 0) {
    const def = byName.get(queue.shift() as string);
    if (!def) continue;
    const fields = def.kind === 'struct' ? def.fields : def.variants.flatMap((v) => v.fields ?? []);
    for (const f of fields) {
      walkTypeRefs(f.type, (name) => {
        if (byName.has(name) && !reachable.has(name)) {
          reachable.add(name);
          queue.push(name);
        }
      });
    }
  }

  const types = allTypes.filter((t) => reachable.has(t.name));

  const idl: ContractIdl = {
    spec_version: options.specVersion ?? DEFAULT_SPEC_VERSION,
    contract: {
      name: contractName,
      crate: options.crateName ?? DEFAULT_CRATE,
      version: options.contractVersion ?? DEFAULT_CONTRACT_VERSION,
    },
    meta: {
      sorobanSdk: options.sorobanSdkVersion ?? DEFAULT_SOROBAN,
      source: options.source ?? DEFAULT_SOURCE,
      generatedBy: TOOL_VERSION,
      sourceDigest: crypto.createHash('sha256').update(sourceText).digest('hex'),
    },
    types,
    errors,
    events,
    functions,
  };

  return { idl, warnings, sourceDigest: idl.meta.sourceDigest as string };
}

/** Read the contract source and extract its IDL. */
export function extractIdlFromFile(filePath: string, options: ExtractOptions = {}): ExtractResult {
  const sourceText = fs.readFileSync(filePath, 'utf8');
  return extractIdlFromSource(sourceText, { ...options, source: options.source ?? filePath });
}

/**
 * Convert Soroban CLI bindings output (`soroban contract bindings json`) into a
 * `ContractIdl`. Used when the IDL is produced from a compiled WASM artifact
 * rather than from source.
 */
export function idlFromSorobanBindings(bindings: unknown, options: ExtractOptions = {}): ExtractResult {
  const warnings: string[] = [];
  if (typeof bindings !== 'object' || bindings === null) {
    throw new Error('Soroban bindings payload must be an object');
  }
  const root = bindings as Record<string, unknown>;
  const rawTypes = Array.isArray(root.types) ? (root.types as Array<Record<string, unknown>>) : [];
  const types: IdlTypeDef[] = [];

  const typeIndex = new Map<string, Record<string, unknown>>();

  const normalizeType = (raw: unknown, path: string): IdlType => {
    if (typeof raw === 'string') {
      try {
        return parseRustType(raw);
      } catch {
        return { kind: 'type', name: raw };
      }
    }
    if (typeof raw !== 'object' || raw === null) {
      warnings.push(`${path}: unsupported type encoding`);
      return { kind: 'bytes' };
    }
    const node = raw as Record<string, unknown>;
    const kind = String(node.kind ?? node.type ?? '');
    switch (kind) {
      case 'bytesN':
      case 'bytesn':
        return { kind: 'bytesn', n: Number(node.size ?? node.n ?? 32) };
      case 'option':
        return { kind: 'option', of: normalizeType(node.value ?? node.of, `${path}.value`) };
      case 'vec':
        return { kind: 'vec', of: normalizeType(node.value ?? node.of, `${path}.value`) };
      case 'map':
        return {
          kind: 'map',
          key: normalizeType(node.key, `${path}.key`),
          value: normalizeType(node.value, `${path}.value`),
        };
      case 'tuple':
        return {
          kind: 'tuple',
          of: (Array.isArray(node.value) ? node.value : [node.value]).map((v, i) => normalizeType(v, `${path}.value[${i}]`)),
        };
      default: {
        const name = String(node.name ?? kind);
        const lowered = name.toLowerCase();
        if (['bool', 'u32', 'u64', 'i32', 'i64', 'address', 'string', 'symbol', 'bytes', 'timepoint', 'duration', 'void'].includes(lowered)) {
          return { kind: lowered } as IdlType;
        }
        return { kind: 'type', name };
      }
    }
  };

  for (const t of rawTypes) {
    if (typeof t.name === 'string') typeIndex.set(t.name, t);
  }

  for (const t of rawTypes) {
    const name = String(t.name ?? '');
    if (name.length === 0) continue;
    if (t.kind === 'struct' || (t.kind === undefined && Array.isArray(t.fields))) {
      const fields: IdlField[] = (Array.isArray(t.fields) ? t.fields : []).map((f, i) => {
        const field = f as Record<string, unknown>;
        return { name: String(field.name ?? `field_${i}`), type: normalizeType(field.type, `types.${name}.${i}`) };
      });
      const def: IdlStruct = { kind: 'struct', name, fields };
      types.push(def);
      continue;
    }
    if (t.kind === 'enum' || Array.isArray(t.cases) || Array.isArray(t.variants)) {
      const raw = (t.cases ?? t.variants) as Array<Record<string, unknown>> | undefined;
      const def: IdlEnum = {
        kind: 'enum',
        name,
        variants: (raw ?? []).map((v, i) => ({
          name: String(v.name ?? `Variant${i}`),
          value: typeof v.value === 'number' ? v.value : i,
        })),
      };
      types.push(def);
      continue;
    }
    // `type` style entries: `{ name, type: { ... } }` refer to a type alias.
    if (t.type !== undefined) {
      types.push({ kind: 'enum', name, variants: [] });
      warnings.push(`type alias "${name}" modelled as an empty enum; add explicit variants if needed`);
    }
  }

  const rawErrors = Array.isArray(root.errors) ? (root.errors as Array<Record<string, unknown>>) : [];
  const errors: IdlError[] = rawErrors.map((e, i) => {
    const discriminant = e.discriminant ?? e.name;
    return {
      name: String(discriminant ?? `Error${i}`),
      code: Number(e.value ?? i + 1),
    };
  });

  const rawFunctions = Array.isArray(root.functions) ? (root.functions as Array<Record<string, unknown>>) : [];
  const functions: IdlFunction[] = rawFunctions.map((f) => {
    const name = String(f.name ?? '');
    const inputs = ((f.inputs ?? []) as Array<Record<string, unknown>>).map((inp, i) => ({
      name: String(inp.name ?? `arg_${i}`),
      type: normalizeType(inp.type, `functions.${name}.inputs[${i}]`),
    }));
    return {
      name,
      inputs,
      output: f.output === undefined ? { kind: 'void' } : normalizeType(f.output, `functions.${name}.output`),
      mutating: typeof f.mutating === 'boolean' ? f.mutating : isMutatingFunction(name),
    };
  });

  const rawEvents = Array.isArray(root.events) ? (root.events as Array<Record<string, unknown>>) : [];
  const events: IdlEvent[] = rawEvents.map((e) => ({
    name: String(e.name ?? ''),
    topics: ((e.topics ?? []) as unknown[]).map((t, i) => normalizeType(t, `events.${String(e.name)}.topics[${i}]`)),
    data: ((e.data ?? []) as unknown[]).map((d, i) => normalizeType(d, `events.${String(e.name)}.data[${i}]`)),
  }));

  // Soroban bindings name the contract at the top level; honour it.
  const declaredName = typeof root.name === 'string' ? root.name : undefined;
  const idl: ContractIdl = {
    spec_version: options.specVersion ?? DEFAULT_SPEC_VERSION,
    contract: {
      name: options.contractName ?? declaredName ?? DEFAULT_CONTRACT,
      crate: options.crateName ?? DEFAULT_CRATE,
      version: options.contractVersion ?? DEFAULT_CONTRACT_VERSION,
    },
    meta: {
      sorobanSdk: options.sorobanSdkVersion ?? DEFAULT_SOROBAN,
      source: options.source ?? DEFAULT_SOURCE,
      generatedBy: TOOL_VERSION,
    },
    types,
    errors,
    events,
    functions,
  };

  return { idl, warnings, sourceDigest: '' };
}

/** Canonical serialisation: stable key order + 2-space indent + trailing newline. */
export function serializeIdl(idl: ContractIdl): string {
  return `${JSON.stringify(idl, null, 2)}\n`;
}
