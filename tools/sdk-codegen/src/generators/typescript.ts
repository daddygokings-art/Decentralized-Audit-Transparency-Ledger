import { ContractIdl, IdlEnum, IdlStruct, IdlType, IdlTypeDef } from '../abi/types';
import { GeneratedFile, allTypes, banner, blockComment, fieldName, footer, joinPath, projectFunctions, renderType } from './common';

/** Directory (repo-relative) the TypeScript artefacts are written to. */
export const TS_OUTPUT_DIR = 'sdk/js/src/generated';

/** Map an IDL type to the TypeScript type expression used in generated code. */
export function tsType(type: IdlType): string {
  switch (type.kind) {
    case 'void':
      return 'void';
    case 'bool':
      return 'boolean';
    case 'u32':
    case 'u64':
    case 'i32':
    case 'i64':
    case 'timepoint':
    case 'duration':
      return 'number';
    case 'address':
    case 'symbol':
      return 'string';
    case 'string':
      return 'string';
    case 'bytes':
    case 'bytesn':
      return 'string';
    case 'option':
      return `${tsType(type.of)} | null`;
    case 'vec':
      return `Array<${tsType(type.of)}>`;
    case 'tuple':
      return `[${type.of.map(tsType).join(', ')}]`;
    case 'map':
      return `Record<${tsType(type.key)}, ${tsType(type.value)}>`;
    case 'type':
      return type.name;
    default:
      return 'unknown';
  }
}

function docLines(text: string | undefined, fallback: string): string[] {
  const body = text && text.trim().length > 0 ? text.trim() : fallback;
  return body.split('\n').flatMap((line) => {
    const wrapped = line.match(/.{1,96}(\s|$)/g);
    return (wrapped ?? [line]).map((w) => w.trimEnd());
  });
}

function renderStruct(def: IdlStruct): string {
  const lines: string[] = [];
  if (def.doc?.text) {
    lines.push(...docLines(def.doc.text, `Contract type \`${def.name}\`.`).map((l) => `/** ${l} */`));
  } else {
    lines.push(`/** Contract type \`${def.name}\` (${def.fields.length} field(s)). */`);
  }
  lines.push(`export interface ${def.name} {`);
  for (const f of def.fields) {
    if (f.doc?.text) lines.push(...docLines(f.doc.text, '').map((l) => `  /** ${l} */`));
    lines.push(`  ${fieldName(f.name)}: ${tsType(f.type)};`);
  }
  lines.push('}');
  return lines.join('\n');
}

function renderEnum(def: IdlEnum): string {
  const lines: string[] = [];
  const hasPayload = def.variants.some((v) => v.fields && v.fields.length > 0);
  if (def.doc?.text) {
    lines.push(...docLines(def.doc.text, `Contract enum \`${def.name}\`.`).map((l) => `/** ${l} */`));
  } else {
    lines.push(`/** Contract enum \`${def.name}\` (${def.variants.length} ${hasPayload ? 'payload' : 'unit'} variant(s)). */`);
  }

  if (!hasPayload) {
    const union = def.variants.map((v) => `'${v.name}'`).join(' | ');
    lines.push(`export type ${def.name} = ${union};`);
    lines.push('');
    lines.push(`/** Wire ordinals for \`${def.name}\`, as encoded by the contract. */`);
    lines.push(`export const ${def.name}Ordinal: Record<${def.name}, number> = {`);
    for (const v of def.variants) lines.push(`  ${v.name}: ${v.value},`);
    lines.push('};');
    lines.push('');
    lines.push(`/** Every \`${def.name}\` value, in wire order. */`);
    lines.push(`export const ${def.name}Values: readonly ${def.name}[] = [`);
    for (const v of def.variants) lines.push(`  '${v.name}',`);
    lines.push('];');
    lines.push('');
    lines.push(`/** Decode a wire ordinal into a \`${def.name}\`; throws on unknown ordinals. */`);
    lines.push(`export function ${def.name}FromOrdinal(value: number): ${def.name} {`);
    lines.push(`  const found = ${def.name}Values.find((candidate) => ${def.name}Ordinal[candidate] === value);`);
    lines.push('  if (found === undefined) {');
    lines.push(`    throw new Error(\`Unknown ${def.name} ordinal: \${value}\`);`);
    lines.push('  }');
    lines.push('  return found;');
    lines.push('}');
    return lines.join('\n');
  }

  // Payload-carrying enum: a discriminated union keeps the wire ordinal next to
  // the payload, so `switch (action.variant)` stays exhaustively checkable.
  lines.push(`export type ${def.name} =`);
  def.variants.forEach((v, i) => {
    const tail = i === def.variants.length - 1 ? ';' : '';
    if (v.fields && v.fields.length > 0) {
      const fields = v.fields.map((f) => `${f.name}: ${tsType(f.type)}`).join('; ');
      lines.push(`  | { variant: '${v.name}'; ordinal: ${v.value}; value: { ${fields} } }${tail}`);
    } else {
      lines.push(`  | { variant: '${v.name}'; ordinal: ${v.value} }${tail}`);
    }
  });
  lines.push('');
  lines.push(`/** Variant names carried by \`${def.name}\`, in wire order. */`);
  lines.push(`export const ${def.name}Variants: readonly ${def.name}['variant'][] = [`);
  for (const v of def.variants) lines.push(`  '${v.name}',`);
  lines.push(`];`);
  lines.push('');
  lines.push(`/** Constructors for every \`${def.name}\` variant. */`);
  lines.push(`export const ${def.name} = {`);
  for (const v of def.variants) {
    if (v.fields && v.fields.length > 0) {
      const params = v.fields.map((f) => `${f.name}: ${tsType(f.type)}`).join(', ');
      lines.push(`  ${v.name}(${params}) {`);
      lines.push(`    return { variant: '${v.name}', ordinal: ${v.value}, value: { ${v.fields.map((f) => f.name).join(', ')} } };`);
      lines.push('  },');
    } else {
      lines.push(`  ${v.name}() {`);
      lines.push(`    return { variant: '${v.name}', ordinal: ${v.value} };`);
      lines.push('  },');
    }
  }
  lines.push('} as const;');
  return lines.join('\n');
}

function generateTypes(idl: ContractIdl): string {
  const parts: string[] = [
    banner('/**'),
    ' *',
    ' * Contract data types. Field names are preserved verbatim from the IDL so',
    ' * that a value can be handed to the RPC layer without a translation step.',
    ' */',
    '',
  ];
  for (const def of allTypes(idl)) {
    parts.push(def.kind === 'struct' ? renderStruct(def) : renderEnum(def), '');
  }
  parts.push(footer('/**', idl));
  return `${parts.join('\n').trimEnd()}\n`;
}

function generateErrors(idl: ContractIdl): string {
  const lines: string[] = [
    banner('/**'),
    ' *',
    ' * Contract error codes. Codes are stable wire values; never renumber them.',
    ' */',
    '',
    'export const ContractErrorCode = {',
  ];
  for (const e of idl.errors) {
    lines.push(`  ${e.name}: ${e.code},`);
  }
  lines.push('} as const;');
  lines.push('');
  lines.push('export type ContractErrorName = keyof typeof ContractErrorCode;');
  lines.push('');
  lines.push('/** Numeric code for a contract error, or `undefined` when unknown. */');
  lines.push('export function contractErrorCode(name: string): number | undefined {');
  lines.push('  return (ContractErrorCode as Record<string, number>)[name];');
  lines.push('}');
  lines.push('');
  lines.push('/** Canonical error name for a numeric code, or `undefined` when unknown. */');
  lines.push('export function contractErrorName(code: number): ContractErrorName | undefined {');
  lines.push('  return (Object.keys(ContractErrorCode) as ContractErrorName[]).find(');
  lines.push('    (name) => ContractErrorCode[name] === code,');
  lines.push('  );');
  lines.push('}');
  lines.push('');
  lines.push('/** Human-readable message for a contract error code. */');
  lines.push('export function describeContractError(code: number): string {');
  lines.push('  const name = contractErrorName(code);');
  lines.push('  return name === undefined ? `Unknown contract error code ${code}` : `${name} (${code})`;');
  lines.push('}');
  lines.push('');
  lines.push('/** All contract error names, ascending by wire code. */');
  lines.push('export const CONTRACT_ERROR_NAMES: readonly ContractErrorName[] = [');
  for (const e of idl.errors) {
    lines.push(`  '${e.name}',`);
  }
  lines.push('];');
  lines.push('');
  lines.push(footer('/**', idl));
  return lines.join('\n');
}

function generateEvents(idl: ContractIdl): string {
  const lines: string[] = [
    banner('/**'),
    ' *',
    ' * Typed contract events. In Soroban the trailing topic is the event',
    ' * discriminator; `topics` below lists the discriminator-relative topics.',
    ' */',
    '',
    // Event payload types may reference contract enums/structs by name.
    ...(() => {
      const payloads = idl.events
        .map((e) => [...e.topics, ...e.data].map((t) => tsType(t)).join(' '))
        .join(' ');
      const used = allTypes(idl)
        .filter((t) => t.kind === 'struct' || t.kind === 'enum')
        .map((t) => t.name)
        .filter((n) => new RegExp(`\\b${n}\\b`).test(payloads))
        .sort();
      return used.length > 0 ? ['', `import type { ${used.join(', ')} } from './types';`] : [];
    })(),
    'export const ContractEventName = {',
  ];
  for (const e of idl.events) {
    lines.push(`  ${pascalEvent(e.name)}: '${e.name}',`);
  }
  lines.push('} as const;');
  lines.push('');
  lines.push('export type ContractEventNameLiteral = (typeof ContractEventName)[keyof typeof ContractEventName];');
  lines.push('');
  lines.push('export const CONTRACT_EVENT_NAMES: readonly ContractEventNameLiteral[] = [');
  for (const e of idl.events) {
    lines.push(`  '${e.name}',`);
  }
  lines.push('];');
  lines.push('');
  for (const e of idl.events) {
    const cls = pascalEvent(e.name);
    lines.push('/**');
    lines.push(` * \`${e.name}\``);
    e.topics.forEach((t, i) => lines.push(` * @topic ${i + 1} ${renderType(t)}`));
    e.data.forEach((t, i) => lines.push(` * @data  ${i + 1} ${renderType(t)}`));
    lines.push(' */');
    lines.push(`export interface ${cls}Event {`);
    lines.push(`  name: '${e.name}';`);
    e.topics.forEach((t, i) => lines.push(`  topic${i + 1}: ${tsType(t)};`));
    e.data.forEach((t, i) => lines.push(`  data${i + 1}: ${tsType(t)};`));
    lines.push('}');
    lines.push('');
  }
  lines.push('/** A decoded contract event whose payload has not been narrowed to a specific shape. */');
  lines.push('export interface DecodedContractEvent {');
  lines.push('  name: string;');
  lines.push('  contractId: string;');
  lines.push('  ledger: number;');
  lines.push('  topics: string[];');
  lines.push('  data: unknown[];');
  lines.push('}');
  lines.push('');
  lines.push(footer('/**', idl));
  return lines.join('\n');
}

function pascalEvent(name: string): string {
  return name
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

function generateClient(idl: ContractIdl): string {
  const fns = projectFunctions(idl, 'camel');
  const lines: string[] = [
    banner('/**'),
    ' *',
    ' * Type-safe client for the AuditLedger Soroban contract.',
    ' *',
    ' * The client is transport-agnostic: supply any implementation of',
    ' * `AuditLedgerTransport` (Stellar RPC, a local `Env` in tests, or a mock).',
    ' * Argument and return types are derived from the contract IDL, so a contract',
    ' * change that is not regenerated here fails to compile rather than failing at',
    ' * runtime.',
    ' */',
    '',
    'import type * as generated from \'./types\';',
    // The client references struct/enum types by bare name, so import exactly
    // the ones that appear in its own signatures.
    ...(() => {
      const sigs = fns
        .map((f) => `${f.args.map((a) => tsType(a.type)).join(' ')} ${tsType(f.output)}`)
        .join(' ');
      const used = allTypes(idl)
        .filter((t) => t.kind === 'struct' || t.kind === 'enum')
        .map((t) => t.name)
        .filter((n) => new RegExp(`\\b${n}\\b`).test(sigs))
        .sort();
      return used.length > 0 ? [`import type { ${used.join(', ')} } from './types';`] : [];
    })(),
    '',
    '/** Transport contract: invokes a contract function and resolves its return value. */',
    'export interface AuditLedgerTransport {',
    '  /**',
    '   * @param method Contract function name exactly as declared on-chain.',
    '   * @param args Positional arguments, already converted to wire form.',
    '   */',
    '  invoke<T = unknown>(method: string, args: unknown[]): Promise<T>;',
    '}',
    '',
    `/** Options accepted by {@link Generated${idl.contract.name}Client}. */`,
    'export interface GeneratedClientOptions {',
    '  /** Contract id (StrKey `C…`) or contract address to invoke. */',
    '  contractId: string;',
    '  transport: AuditLedgerTransport;',
    '}',
    '',
    `/** Generated, type-safe client for the ${idl.contract.name} contract. */`,
    `export class Generated${idl.contract.name}Client {`,
    '  readonly contractId: string;',
    '  private readonly transport: AuditLedgerTransport;',
    '',
    '  constructor(options: GeneratedClientOptions) {',
    '    this.contractId = options.contractId;',
    '    this.transport = options.transport;',
    '  }',
    '',
  ];

  for (const fn of fns) {
    const params = fn.args.map((a) => `${a.paramName}: ${tsType(a.type)}`).join(', ');
    const ret = fn.output.kind === 'void' ? 'Promise<void>' : `Promise<${tsType(fn.output)}>`;
    const callArgs = fn.args.map((a) => a.paramName).join(', ');
    const label = fn.needsTransaction ? 'State-changing' : 'Read-only';
    const doc: string[] = [];
    if (fn.doc && fn.doc.trim().length > 0) {
      doc.push(...fn.doc.trim().split('\n'));
    } else {
      doc.push(`${label} contract function \`${fn.wireName}\`.`);
    }
    if (fn.args.length > 0) {
      doc.push(...fn.args.map((a) => `@param ${a.paramName} wire name \`${a.wireName}\` (${renderType(a.type)})`));
    }
    if (fn.output.kind !== 'void') doc.push(`@returns ${renderType(fn.output)}`);
    lines.push(blockComment('  /**', doc.join('\n'), `${label} contract function \`${fn.wireName}\`.`));
    lines.push(`  async ${fn.methodName}(${params}): ${ret} {`);
    if (fn.output.kind === 'void') {
      lines.push(`    await this.transport.invoke('${fn.wireName}', [${callArgs}]);`);
    } else {
      lines.push(`    return this.transport.invoke<${tsType(fn.output)}>('${fn.wireName}', [${callArgs}]);`);
    }
    lines.push('  }');
    lines.push('');
  }

  lines.push('  /** All function names this client exposes, for tooling and validation. */');
  lines.push('  static readonly functionNames: readonly string[] = [');
  for (const fn of fns) {
    lines.push(`    '${fn.wireName}',`);
  }
  lines.push('  ];');
  lines.push('}');
  lines.push('');
  lines.push('export type { generated };');
  lines.push(footer('/**', idl));
  return lines.join('\n');
}

function generateIndex(idl: ContractIdl): string {
  return [
    banner('/**'),
    ' *',
    ' * Public surface of the generated AuditLedger SDK. Hand-written SDK modules',
    ' * live one directory up and may re-export from here.',
    ' */',
    '',
    'export * from \'./types\';',
    'export * from \'./errors\';',
    'export * from \'./events\';',
    'export * from \'./client\';',
    '',
    footer('/**', idl),
    '',
  ].join('\n');
}

/** Generate every TypeScript artefact for the given IDL. */
export function generateTypeScript(idl: ContractIdl, outDir: string = TS_OUTPUT_DIR): GeneratedFile[] {
  return [
    { path: joinPath(outDir, 'types.ts'), contents: generateTypes(idl) },
    { path: joinPath(outDir, 'errors.ts'), contents: generateErrors(idl) },
    { path: joinPath(outDir, 'events.ts'), contents: generateEvents(idl) },
    { path: joinPath(outDir, 'client.ts'), contents: generateClient(idl) },
    { path: joinPath(outDir, 'index.ts'), contents: generateIndex(idl) },
  ];
}

/** Exposed for tests: the `IdlTypeDef` discriminator the TypeScript renderer handles. */
export function tsSupportedDefKinds(): Array<IdlTypeDef['kind']> {
  return ['struct', 'enum'];
}
