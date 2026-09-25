/**
 * AuditLedger contract ABI/IDL definition (#407).
 *
 * The IDL is the single source of truth for every generated SDK artefact. It is
 * produced from the contract itself — either by the Rust source extractor
 * (`sdk-codegen extract --source src/lib.rs`) or by the Soroban CLI bindings
 * (`soroban contract bindings json`) — and committed to `abi/audit-ledger.json`.
 *
 * Keeping the type model explicit (rather than a string union) means the
 * generators never have to re-parse type text, and the compatibility checker can
 * reason structurally about whether a change breaks SDK consumers.
 */

/** Discriminator for every IDL type node. */
export type IdlTypeKind =
  | 'void'
  | 'bool'
  | 'u32'
  | 'u64'
  | 'i32'
  | 'i64'
  | 'address'
  | 'string'
  | 'symbol'
  | 'bytes'
  | 'bytesn'
  | 'timepoint'
  | 'duration'
  | 'option'
  | 'vec'
  | 'tuple'
  | 'map'
  | 'type';

export type IdlType =
  | { kind: 'void' }
  | { kind: 'bool' }
  | { kind: 'u32' }
  | { kind: 'u64' }
  | { kind: 'i32' }
  | { kind: 'i64' }
  | { kind: 'address' }
  | { kind: 'string' }
  | { kind: 'symbol' }
  | { kind: 'bytes' }
  | { kind: 'bytesn'; n: number }
  | { kind: 'timepoint' }
  | { kind: 'duration' }
  | { kind: 'option'; of: IdlType }
  | { kind: 'vec'; of: IdlType }
  | { kind: 'tuple'; of: IdlType[] }
  | { kind: 'map'; key: IdlType; value: IdlType }
  | { kind: 'type'; name: string };

export interface IdlDoc {
  /** Line the declaration starts on in the contract source, for error messages. */
  line?: number;
  text: string;
}

export interface IdlField {
  name: string;
  type: IdlType;
  doc?: IdlDoc;
}

export interface IdlStruct {
  kind: 'struct';
  name: string;
  fields: IdlField[];
  doc?: IdlDoc;
}

export interface IdlEnumVariant {
  name: string;
  /** Ordinal on the wire; Soroban encodes unit variants by declaration order. */
  value: number;
  /**
   * Payload for tuple-style variants such as `ProposalAction::AddOwner(Address)`.
   * Absent for unit variants. Ordinals are still assigned by declaration order,
   * which is how Soroban discriminates the variants.
   */
  fields?: IdlField[];
  doc?: IdlDoc;
}

export interface IdlEnum {
  kind: 'enum';
  name: string;
  variants: IdlEnumVariant[];
  doc?: IdlDoc;
}

export type IdlTypeDef = IdlStruct | IdlEnum;

export interface IdlError {
  name: string;
  code: number;
  doc?: IdlDoc;
}

export interface IdlEvent {
  /** Contract event name, as published in the first topic slot. */
  name: string;
  /** Ordered topic list after the leading event-name symbol. */
  topics: IdlType[];
  /** Ordered data list. */
  data: IdlType[];
  doc?: IdlDoc;
}

export interface IdlInput {
  name: string;
  type: IdlType;
  doc?: IdlDoc;
}

export interface IdlFunction {
  name: string;
  inputs: IdlInput[];
  output: IdlType;
  /**
   * `true` when the function can mutate ledger state. Query functions are
   * `false`, which lets SDKs route them through read-only RPC calls.
   */
  mutating: boolean;
  doc?: IdlDoc;
}

export interface IdlContract {
  name: string;
  crate: string;
  /** Semver version of the contract surface described by this IDL. */
  version: string;
}

export interface IdlMeta {
  sorobanSdk: string;
  /** Repository-relative source of truth for the contract. */
  source: string;
  /** Tool that produced the file, e.g. `sdk-codegen 1.0.0`. */
  generatedBy: string;
  /** SHA-256 of the contract source the IDL was extracted from. */
  sourceDigest?: string;
}

export interface ContractIdl {
  /** `major.minor.patch`. Bumped by the compatibility checker rules. */
  spec_version: string;
  contract: IdlContract;
  meta: IdlMeta;
  types: IdlTypeDef[];
  errors: IdlError[];
  events: IdlEvent[];
  functions: IdlFunction[];
}

export const IDL_SCHEMA_VERSION = 1;

/** Contract errors/parameters that never mutate state and can be served from RPC reads. */
const QUERY_PREFIXES = ['get_', 'list_', 'search_', 'total_', 'event_count', 'is_', 'has_', 'verify_', 'snapshot_count', 'paused_since', 'find_', 'submitter_event_count'];

const MUTATION_PREFIXES = ['set_', 'log_', 'add_', 'remove_', 'update_', 'archive_', 'purge_', 'cleanup_', 'block_', 'unblock_', 'enable_', 'disable_', 'allow_', 'register_', 'unregister_', 'transfer_', 'initialize', 'pause', 'unpause', 'submit_', 'approve_', 'execute_', 'upgrade_', 'migrate_', 'create_', 'compact_', 'tag_', 'reset_'];

/** Classify a contract function as read-only or state-mutating. */
export function isMutatingFunction(name: string): boolean {
  if (MUTATION_PREFIXES.some((p) => name === p || name.startsWith(p))) return true;
  if (QUERY_PREFIXES.some((p) => name === p || name.startsWith(p))) return false;
  return true;
}

/** Stable structural fingerprint of a type node, used by the compat checker. */
export function typeSignature(type: IdlType): string {
  switch (type.kind) {
    case 'bytesn':
      return `bytesn<${type.n}>`;
    case 'option':
      return `option<${typeSignature(type.of)}>`;
    case 'vec':
      return `vec<${typeSignature(type.of)}>`;
    case 'tuple':
      return `tuple<${type.of.map(typeSignature).join(',')}>`;
    case 'map':
      return `map<${typeSignature(type.key)},${typeSignature(type.value)}>`;
    case 'type':
      return type.name;
    default:
      return type.kind;
  }
}

/** Walk every type node reachable from a type definition. */
export function walkTypeRefs(type: IdlType, visit: (name: string) => void): void {
  switch (type.kind) {
    case 'type':
      visit(type.name);
      return;
    case 'option':
    case 'vec':
      walkTypeRefs(type.of, visit);
      return;
    case 'tuple':
      type.of.forEach((t) => walkTypeRefs(t, visit));
      return;
    case 'map':
      walkTypeRefs(type.key, visit);
      walkTypeRefs(type.value, visit);
      return;
    default:
  }
}

/** Every user-defined type name referenced anywhere in the IDL. */
export function collectReferencedTypes(idl: ContractIdl): Set<string> {
  const refs = new Set<string>();
  const consider = (t: IdlType) => walkTypeRefs(t, (n) => refs.add(n));

  idl.types.forEach((def) => {
    if (def.kind === 'struct') def.fields.forEach((f) => consider(f.type));
    else def.variants.forEach((v) => v.fields?.forEach((f) => consider(f.type)));
  });
  idl.functions.forEach((fn) => {
    fn.inputs.forEach((i) => consider(i.type));
    consider(fn.output);
  });
  idl.events.forEach((e) => {
    e.topics.forEach(consider);
    e.data.forEach(consider);
  });
  return refs;
}

export function findTypeDef(idl: ContractIdl, name: string): IdlTypeDef | undefined {
  return idl.types.find((t) => t.name === name);
}

export function findFunction(idl: ContractIdl, name: string): IdlFunction | undefined {
  return idl.functions.find((f) => f.name === name);
}
