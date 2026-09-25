import { ContractIdl, IdlEnum, IdlFunction, IdlStruct, IdlType, IdlTypeDef, isMutatingFunction, typeSignature } from '../abi/types';

/** A single generated file: repo-relative path plus contents. */
export interface GeneratedFile {
  path: string;
  contents: string;
}

export type Language = 'typescript' | 'python' | 'rust';

/** Path separator used when writing generated trees, normalised per language. */
export function joinPath(base: string, ...parts: string[]): string {
  return [base.replace(/\/+$/, ''), ...parts].join('/');
}

/**
 * Contract field names are preserved verbatim (snake_case) in every generated
 * language: they map 1:1 onto the contract's XDR/JSON encoding, so a generated
 * client never needs a lossy name translation layer. Only *method* names are
 * idiomatically cased per language.
 */
export function fieldName(name: string): string {
  return name;
}

export function snakeToCamel(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_m, c: string) => c.toUpperCase());
}

export function camelToSnake(name: string): string {
  return name.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
}

export function pascal(name: string): string {
  const camel = snakeToCamel(name);
  return camel.charAt(0).toUpperCase() + camel.slice(1);
}

/** Reserved words that cannot be used as bare identifiers in the target language. */
const TS_RESERVED = new Set([
  'break', 'case', 'catch', 'class', 'const', 'continue', 'debugger', 'default', 'delete', 'do', 'else', 'export',
  'extends', 'finally', 'for', 'function', 'if', 'import', 'in', 'instanceof', 'new', 'return', 'super', 'switch',
  'this', 'throw', 'try', 'typeof', 'var', 'void', 'while', 'with', 'yield', 'let', 'static', 'enum', 'await',
  'implements', 'package', 'protected', 'interface', 'private', 'public',
]);

const PY_RESERVED = new Set([
  'False', 'None', 'True', 'and', 'as', 'assert', 'async', 'await', 'break', 'class', 'continue', 'def', 'del',
  'elif', 'else', 'except', 'finally', 'for', 'from', 'global', 'if', 'import', 'in', 'is', 'lambda', 'nonlocal',
  'not', 'or', 'pass', 'raise', 'return', 'try', 'while', 'with', 'yield', 'match', 'case', 'type',
]);

const RUST_RESERVED = new Set([
  'as', 'break', 'const', 'continue', 'crate', 'dyn', 'else', 'enum', 'extern', 'false', 'fn', 'for', 'if', 'impl',
  'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub', 'ref', 'return', 'self', 'Self', 'static', 'struct',
  'super', 'trait', 'true', 'type', 'unsafe', 'use', 'where', 'while', 'async', 'await', 'abstract', 'become',
  'box', 'do', 'final', 'macro', 'override', 'priv', 'typeof', 'unsized', 'virtual', 'yield', 'try',
]);

function safeIdent(name: string, reserved: Set<string>): string {
  return reserved.has(name) ? `${name}_` : name;
}

export const tsIdent = (name: string): string => safeIdent(name, TS_RESERVED);
export const pyIdent = (name: string): string => safeIdent(name, PY_RESERVED);
export const rustIdent = (name: string): string => safeIdent(name, RUST_RESERVED);

// ── Shared projections ──────────────────────────────────────────────────────

export interface FunctionArg {
  /** Contract-side name, used for the transport payload. */
  wireName: string;
  /** Per-language method parameter name. */
  paramName: string;
  type: IdlType;
}

export interface ProjectedFunction {
  /** Contract-side name, used for the transport call. */
  wireName: string;
  /** Per-language method name. */
  methodName: string;
  args: FunctionArg[];
  output: IdlType;
  /** The function may change state or emit events (derived from the contract body). */
  mutating: boolean;
  /** The caller must submit a transaction rather than a read-only call. */
  needsTransaction: boolean;
  doc?: string;
}

export function projectFunctions(idl: ContractIdl, style: 'camel' | 'snake' | 'snake_rust'): ProjectedFunction[] {
  return idl.functions.map((fn: IdlFunction) => ({
    wireName: fn.name,
    methodName: style === 'camel' ? snakeToCamel(fn.name) : fn.name,
    args: fn.inputs.map((i) => ({
      wireName: i.name,
      paramName: style === 'camel' ? tsIdent(snakeToCamel(i.name)) : i.name,
      type: i.type,
    })),
    output: fn.output,
    mutating: fn.mutating,
    // Two independent signals decide this:
    //  - the naming convention says the function exists to change state
    //    (`log_*`, `set_*`, ...) rather than to report it (`get_*`, `list_*`);
    //  - the function returns nothing, so a read-only call would discard the
    //    only thing the caller is after.
    // A `get_*` that returns a value stays a read-only call even when its body
    // performs a lazy, incidental write: `simulateTransaction` runs the code
    // without committing state, so there is nothing to submit.
    needsTransaction: isMutatingFunction(fn.name) || (fn.mutating && fn.output.kind === 'void'),
    doc: fn.doc?.text,
  }));
}

export function structs(idl: ContractIdl): IdlStruct[] {
  return idl.types.filter((t): t is IdlStruct => t.kind === 'struct');
}

export function enums(idl: ContractIdl): IdlEnum[] {
  return idl.types.filter((t): t is IdlEnum => t.kind === 'enum');
}

export function allTypes(idl: ContractIdl): IdlTypeDef[] {
  return idl.types;
}

/** Strip a trailing `/**` from a comment prefix, leaving the indentation. */
function BLOCK_INDENT(prefix: string): string {
  return prefix.replace(/\/\*\*$/, '');
}

/**
 * Wrap `text` in a block comment, one line per source line.
 *
 * A `/**` prefix produces a self-terminated block comment; any other prefix is
 * treated as a line-comment marker and left for the caller to close.
 */
export function blockComment(prefix: string, text: string | undefined, fallback: string): string {
  const body = (text && text.trim().length > 0 ? text.trim() : fallback).split('\n');
  const bodyPrefix = prefix.endsWith('/**') ? `${BLOCK_INDENT(prefix)} * ` : `${prefix} `;
  const lines = [prefix, ...body.map((l) => (l.length > 0 ? `${bodyPrefix}${l}` : bodyPrefix.trimEnd()))];
  return prefix.endsWith('/**') ? [...lines, `${BLOCK_INDENT(prefix)} */`].join('\n') : lines.join('\n');
}

/** Human-readable one-line rendering of a type, used in generated doc comments. */
export function renderType(type: IdlType): string {
  return typeSignature(type);
}

const BANNER_LINES = [
  'GENERATED FILE — DO NOT EDIT.',
  'Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).',
  'Source of truth: abi/audit-ledger.json',
];

/**
 * Standard `do not edit` banner shared by every generated artefact.
 *
 * Left open: callers continue the block with their own ` * ` lines and close it.
 */
export function banner(prefix: string): string {
  if (prefix.endsWith('/**')) {
    return [prefix, ...BANNER_LINES.map((l) => `${BLOCK_INDENT(prefix)} * ${l}`)].join('\n');
  }
  return BANNER_LINES.map((l) => `${prefix} ${l}`).join('\n');
}

/** Self-terminated footer stamped with the exact IDL the file came from. */
export function footer(prefix: string, idl: ContractIdl): string {
  const lines = [
    `contract: ${idl.contract.name} v${idl.contract.version}`,
    `idl: spec_version ${idl.spec_version} (${idl.functions.length} functions, ${idl.types.length} types, ${idl.errors.length} errors, ${idl.events.length} events)`,
  ];
  if (prefix.endsWith('/**')) {
    return [prefix, ...lines.map((l) => `${BLOCK_INDENT(prefix)} * ${l}`), `${BLOCK_INDENT(prefix)} */`].join('\n');
  }
  return lines.map((l) => `${prefix} ${l}`).join('\n');
}
