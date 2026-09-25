import { ContractIdl, IdlEnum, IdlStruct, IdlType } from '../abi/types';
import { GeneratedFile, banner, enums, footer, joinPath, projectFunctions, renderType, structs } from './common';

/** Directory (repo-relative) the Python artefacts are written to. */
export const PY_OUTPUT_DIR = 'sdk/python/audit_ledger/generated';

/** Map an IDL type to the Python type expression used in generated code. */
export function pyType(type: IdlType): string {
  switch (type.kind) {
    case 'void':
      return 'None';
    case 'bool':
      return 'bool';
    case 'u32':
    case 'u64':
    case 'i32':
    case 'i64':
    case 'timepoint':
    case 'duration':
      return 'int';
    case 'address':
    case 'string':
    case 'symbol':
    case 'bytes':
    case 'bytesn':
      return 'str';
    case 'option':
      return `Optional[${pyType(type.of)}]`;
    case 'vec':
      return `List[${pyType(type.of)}]`;
    case 'tuple':
      return `Tuple[${type.of.map(pyType).join(', ')}]`;
    case 'map':
      return `Dict[${pyType(type.key)}, ${pyType(type.value)}]`;
    case 'type':
      return type.name;
    default:
      return 'Any';
  }
}

/** Hard keywords and builtins that cannot be used as a Python identifier. */
const PY_RESERVED = new Set([
  'False', 'None', 'True', 'and', 'as', 'assert', 'async', 'await', 'break', 'class', 'continue',
  'def', 'del', 'elif', 'else', 'except', 'finally', 'for', 'from', 'global', 'if', 'import', 'in',
  'is', 'lambda', 'nonlocal', 'not', 'or', 'pass', 'raise', 'return', 'try', 'while', 'with', 'yield',
  'match', 'case', 'type', 'id', 'input', 'list', 'dict', 'set', 'tuple', 'object', 'bytes', 'str',
]);

/** Python-safe identifier for a contract name, preserving the wire name. */
function pyIdent(name: string): string {
  const safe = name.replace(/[^A-Za-z0-9_]/g, '_');
  const withLead = /^[0-9]/.test(safe) ? `_${safe}` : safe;
  return PY_RESERVED.has(withLead) ? `${withLead}_` : withLead;
}

function docstring(text: string | undefined, fallback: string, indent: string, extra: string[] = []): string[] {
  const body = text && text.trim().length > 0 ? text.trim() : fallback;
  const parts = [body];
  if (extra.length > 0) parts.push('', ...extra);
  const lines = parts.flatMap((part) => {
    if (part.length === 0) return [''];
    return (part.match(/.{1,88}(\s|$)/g) ?? [part]).map((l) => l.trimEnd());
  });
  if (lines.length === 1) return [`${indent}"""${lines[0]}"""`];
  // Blank lines inside a docstring stay blank rather than carrying indentation,
  // which would leave trailing whitespace in the generated file.
  const tail = lines.slice(1).map((l) => (l.length === 0 ? '' : `${indent}${l}`));
  return [`${indent}"""${lines[0]}`, ...tail, `${indent}"""`];
}

function renderStruct(def: IdlStruct): string {
  const lines: string[] = [];
  lines.push('@dataclass(frozen=True)');
  lines.push(`class ${def.name}:`);
  if (def.doc?.text) {
    lines.push(...docstring(def.doc.text, `Contract type ${def.name}.`, ''));
  } else {
    lines.push(`    """Contract type ${def.name} (${def.fields.length} field(s))."""`);
  }
  lines.push('');
  for (const f of def.fields) {
    if (f.doc?.text) {
      lines.push(`    # ${f.doc.text.split('\n').join(' ')}`);
    }
    lines.push(`    ${pyIdent(f.name)}: ${pyType(f.type)}`);
  }
  lines.push('');
  lines.push('    def to_wire(self) -> Dict[str, Any]:');
  lines.push('        """Return the field mapping expected by the RPC invoke layer."""');
  lines.push('        return {');
  for (const f of def.fields) {
    lines.push(`            "${f.name}": self.${pyIdent(f.name)},`);
  }
  lines.push('        }');
  lines.push('');
  lines.push('    @classmethod');
  lines.push(`    def from_wire(cls, payload: Mapping[str, Any]) -> "${def.name}":`);
  lines.push('        """Build the dataclass from an RPC payload keyed by contract field name."""');
  if (def.fields.length === 0) {
    lines.push('        return cls()');
  } else {
    lines.push('        return cls(');
    for (const f of def.fields) {
      lines.push(`            ${pyIdent(f.name)}=payload["${f.name}"],`);
    }
    lines.push('        )');
  }
  return lines.join('\n');
}

function renderEnum(def: IdlEnum): string {
  const hasPayload = def.variants.some((v) => v.fields && v.fields.length > 0);
  const lines: string[] = [];

  if (!hasPayload) {
    lines.push(`class ${def.name}(int, Enum):`);
    if (def.doc?.text) {
      lines.push(...docstring(def.doc.text, `Contract enum ${def.name}.`, ''));
    } else {
      lines.push(`    """Contract enum ${def.name} (${def.variants.length} unit variant(s))."""`);
    }
    lines.push('');
    for (const v of def.variants) lines.push(`    ${pyIdent(v.name)} = ${v.value}`);
    lines.push('');
    lines.push('    @classmethod');
    lines.push(`    def from_wire(cls, value: int) -> "${def.name}":`);
    lines.push('        """Decode a wire ordinal, raising `ValueError` on unknown values."""');
    lines.push('        try:');
    lines.push('            return cls(value)');
    lines.push('        except ValueError as exc:  # pragma: no cover - defensive');
    lines.push(`            raise ValueError(f"Unknown ${def.name} ordinal: {value}") from exc`);
    return lines.join('\n');
  }

  // Payload-carrying enum: a tagged value keeps the wire ordinal next to the
  // payload, mirroring the discriminated union used by the TS/Rust bindings.
  lines.push('@dataclass(frozen=True)');
  lines.push(`class ${def.name}:`);
  if (def.doc?.text) {
    lines.push(...docstring(def.doc.text, `Contract enum ${def.name}.`, ''));
  } else {
    lines.push(`    """Contract enum ${def.name} (${def.variants.length} payload variant(s))."""`);
  }
  lines.push('');
  lines.push('    variant: str');
  lines.push('    ordinal: int');
  lines.push('    value: Optional[Tuple[Any, ...]] = None');
  lines.push('');
  for (const v of def.variants) {
    if (v.fields && v.fields.length > 0) {
      const params = v.fields.map((f) => `${pyIdent(f.name)}: ${pyType(f.type)}`).join(', ');
      lines.push(`    @staticmethod`);
      lines.push(`    def ${pyIdent(v.name)}(${params}) -> "${def.name}":`);
      lines.push(`        return ${def.name}("${v.name}", ${v.value}, (${v.fields.map((f) => pyIdent(f.name)).join(', ')},))`);
    } else {
      lines.push('    @staticmethod');
      lines.push(`    def ${pyIdent(v.name)}() -> "${def.name}":`);
      lines.push(`        return ${def.name}("${v.name}", ${v.value}, None)`);
    }
    lines.push('');
  }
  lines.push('    @classmethod');
  lines.push(`    def variants(cls) -> Tuple[str, ...]:`);
  lines.push('        """Return every variant name, in wire order."""');
  lines.push('        return (');
  for (const v of def.variants) lines.push(`            "${v.name}",`);
  lines.push('        )');
  return lines.join('\n');
}

function generateTypes(idl: ContractIdl): string {
  const parts: string[] = [
    banner('#'),
    '"""',
    '',
    'Contract data types for the AuditLedger Soroban contract.',
    '',
    'Field names are preserved verbatim from the contract IDL so a value can be',
    'handed straight to the RPC layer without translation.',
    '"""',
    '',
    'from dataclasses import dataclass',
    'from enum import Enum',
    'from typing import Any, Dict, List, Mapping, Optional, Tuple',
    '',
    '__all__ = [',
  ];
  for (const def of idl.types) {
    parts.push(`    "${def.name}",`);
  }
  parts.push(']', '');
  for (const def of idl.types) {
    parts.push(def.kind === 'struct' ? renderStruct(def) : renderEnum(def), '');
  }
  parts.push(footer('#', idl));
  return `${parts.join('\n').trimEnd()}\n`;
}

function generateErrors(idl: ContractIdl): string {
  const lines: string[] = [
    banner('#'),
    '"""',
    '',
    'Contract error codes. Codes are stable wire values; never renumber them.',
    '"""',
    '',
    'from enum import IntEnum',
    'from typing import Dict, Optional',
    '',
    '__all__ = ["ContractErrorCode", "contract_error_name", "describe_contract_error", "CONTRACT_ERROR_NAMES"]',
    '',
    '',
    'class ContractErrorCode(IntEnum):',
    '    """Numeric contract error codes, ascending by wire value."""',
    '',
  ];
  for (const e of idl.errors) {
    lines.push(`    ${e.name} = ${e.code}`);
  }
  lines.push('');
  lines.push('');
  lines.push('CONTRACT_ERROR_NAMES: Dict[int, str] = {');
  for (const e of idl.errors) {
    lines.push(`    ${e.code}: "${e.name}",`);
  }
  lines.push('}');
  lines.push('');
  lines.push('');
  lines.push('def contract_error_name(code: int) -> Optional[str]:');
  lines.push('    """Return the canonical error name for a wire code, or `None`."""');
  lines.push('    return CONTRACT_ERROR_NAMES.get(int(code))');
  lines.push('');
  lines.push('');
  lines.push('def describe_contract_error(code: int) -> str:');
  lines.push('    """Return a human-readable description of a wire error code."""');
  lines.push('    name = contract_error_name(code)');
  lines.push('    if name is None:');
  lines.push('        return f"Unknown contract error code {code}"');
  lines.push('    return f"{name} ({code})"');
  lines.push('');
  lines.push(footer('#', idl));
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
    banner('#'),
    '"""',
    '',
    'Typed contract events. In Soroban the trailing topic is the event',
    'discriminator; `topics` below lists the discriminator-relative topics.',
    '"""',
    '',
    'from dataclasses import dataclass, field',
    'from typing import Any, Dict, List, Optional, Sequence, Tuple',
    // Event payloads may reference contract enums/structs by name.
    ...(() => {
      const payloads = idl.events
        .map((e) => [...e.topics, ...e.data].map((t) => pyType(t)).join(' '))
        .join(' ');
      const used = [...structs(idl), ...enums(idl)]
        .map((t) => t.name)
        .filter((n) => new RegExp(`\\b${n}\\b`).test(payloads))
        .sort();
      return used.length > 0 ? [`from .types import ${used.join(', ')}`] : [];
    })(),
    '',
    '__all__ = [',
    '    "ContractEvent",',
    '    "CONTRACT_EVENT_NAMES",',
    '    "EVENT_PAYLOAD_SHAPES",',
  ];
  for (const e of idl.events) {
    lines.push(`    "${pascalEvent(e.name)}Event",`);
  }
  lines.push(']', '');
  lines.push('CONTRACT_EVENT_NAMES: Tuple[str, ...] = (');
  for (const e of idl.events) {
    lines.push(`    "${e.name}",`);
  }
  lines.push(')');
  lines.push('');
  lines.push('');
  lines.push('@dataclass(frozen=True)');
  lines.push('class ContractEvent:');
  lines.push('    """A decoded contract event, before payload narrowing."""');
  lines.push('');
  lines.push('    name: str');
  lines.push('    contract_id: str');
  lines.push('    ledger: int');
  lines.push('    topics: Sequence[str] = field(default_factory=tuple)');
  lines.push('    data: Sequence[Any] = field(default_factory=tuple)');
  lines.push('');
  lines.push('');
  for (const e of idl.events) {
    const cls = pascalEvent(e.name);
    lines.push('@dataclass(frozen=True)');
    lines.push(`class ${cls}Event:`);
    lines.push(`    """Decoded \`${e.name}\` contract event."""`);
    lines.push('');
    lines.push('    name: str = "' + e.name + '"');
    lines.push('    contract_id: str = ""');
    lines.push('    ledger: int = 0');
    e.topics.forEach((t, i) => lines.push(`    topic${i + 1}: ${pyType(t)} = ${pythonDefault(t)}`));
    e.data.forEach((t, i) => lines.push(`    data${i + 1}: ${pyType(t)} = ${pythonDefault(t)}`));
    lines.push('');
    lines.push('');
  }
  lines.push('#: Event name -> ordered (topic types, data types) as encoded on chain.');
  lines.push('EVENT_PAYLOAD_SHAPES: Dict[str, Tuple[List[str], List[str]]] = {');
  for (const e of idl.events) {
    const topics = e.topics.map((t) => `"${renderType(t)}"`).join(', ');
    const data = e.data.map((t) => `"${renderType(t)}"`).join(', ');
    lines.push(`    "${e.name}": ([${topics}], [${data}]),`);
  }
  lines.push('}');
  lines.push('');
  lines.push(footer('#', idl));
  return lines.join('\n');
}

function pythonDefault(type: IdlType): string {
  switch (type.kind) {
    case 'bool':
      return 'False';
    case 'u32':
    case 'u64':
    case 'i32':
    case 'i64':
    case 'timepoint':
    case 'duration':
      return '0';
    case 'option':
      return 'None';
    case 'vec':
      return 'None';
    case 'map':
      return 'None';
    case 'tuple':
      return 'None';
    default:
      return '""';
  }
}

function generateClient(idl: ContractIdl): string {
  const fns = projectFunctions(idl, 'snake');
  const lines: string[] = [
    banner('#'),
    '"""',
    '',
    `Type-safe client for the ${idl.contract.name} Soroban contract.`,
    '',
    'The client is transport-agnostic: supply any object implementing',
    '`AuditLedgerTransport` (Stellar RPC, a local `Env` in tests, or a mock).',
    'Argument and return types are derived from the contract IDL, so a contract',
    'change that is not regenerated here fails at import time rather than at',
    'runtime.',
    '"""',
    '',
    'from typing import Any, Dict, List, Optional, Protocol, Sequence, Tuple, Union, runtime_checkable',
    // Contract types referenced in method signatures are imported explicitly.
    ...(() => {
      const sigs = fns
        .map((f) => `${f.args.map((a) => pyType(a.type)).join(' ')} ${pyType(f.output)}`)
        .join(' ');
      const used = [...structs(idl), ...enums(idl)]
        .map((t) => t.name)
        .filter((n) => new RegExp(`\\b${n}\\b`).test(sigs))
        .sort();
      return used.length > 0 ? ['', `from .types import ${used.join(', ')}`] : [];
    })(),
    '',
    '__all__ = ["AuditLedgerTransport", "GeneratedAuditLedgerClient"]',
    '',
    '',
    '@runtime_checkable',
    'class AuditLedgerTransport(Protocol):',
    '    """Invokes a contract function and returns its decoded result."""',
    '',
    '    def invoke(self, method: str, args: Sequence[Any]) -> Any:',
    '        """Invoke `method` with positional `args` and return the result."""',
    '        ...',
    '',
    '',
    `class Generated${idl.contract.name}Client:`,
    `    """Generated client binding every public function of ${idl.contract.name}."""`,
    '',
    '    #: Contract function names exposed by this client.',
    '    FUNCTION_NAMES: Sequence[str] = (',
  ];
  for (const fn of fns) {
    lines.push(`        "${fn.wireName}",`);
  }
  lines.push('    )');
  lines.push('');
  lines.push('    def __init__(self, contract_id: str, transport: AuditLedgerTransport) -> None:');
  lines.push('        self.contract_id = contract_id');
  lines.push('        self._transport = transport');
  lines.push('');

  for (const fn of fns) {
    const params = fn.args.map((a) => `${pyIdent(a.paramName)}: ${pyType(a.type)}`).join(', ');
    const sig = fn.args.length > 0 ? `self, ${params}` : 'self';
    const ret = fn.output.kind === 'void' ? 'None' : pyType(fn.output);
    const label = fn.needsTransaction ? 'State-changing' : 'Read-only';
    const doc = fn.doc && fn.doc.trim().length > 0 ? fn.doc.trim() : `${label} contract function \`${fn.wireName}\`.`;
    lines.push(`    def ${fn.wireName}(${sig}) -> ${ret}:`);
    const extra: string[] = [];
    if (fn.args.length > 0) {
      extra.push('Args:');
      for (const a of fn.args) {
        extra.push(`    ${a.paramName}: wire name \`${a.wireName}\` (${renderType(a.type)}).`);
      }
    }
    if (fn.output.kind !== 'void') {
      extra.push('Returns:', `    ${renderType(fn.output)}.`);
    }
    lines.push(...docstring(doc, '', '        ', extra));
    lines.push('');
    lines.push(`        return self._transport.invoke("${fn.wireName}", [${fn.args.map((a) => pyIdent(a.paramName)).join(', ')}])`);
    lines.push('');
  }

  lines.push('    def as_dict(self) -> Union[dict, List[Any]]:');
  lines.push('        """Return a small client summary, useful in debuggers and logs."""');
  lines.push('        return {"contract_id": self.contract_id, "functions": len(self.FUNCTION_NAMES)}');
  lines.push('');
  lines.push(footer('#', idl));
  return lines.join('\n');
}

function generateInit(idl: ContractIdl): string {
  return [
    banner('#'),
    '"""',
    '',
    'Generated AuditLedger SDK surface (types, errors, events, client).',
    '',
    'Import the concrete client and models from `audit_ledger` itself; this',
    'package is the machine-generated contract binding underneath it.',
    '"""',
    '',
    'from .client import AuditLedgerTransport, Generated' + idl.contract.name + 'Client',
    'from .errors import (',
    '    CONTRACT_ERROR_NAMES,',
    '    ContractErrorCode,',
    '    contract_error_name,',
    '    describe_contract_error,',
    ')',
    'from .events import EVENT_PAYLOAD_SHAPES, CONTRACT_EVENT_NAMES, ContractEvent',
    'from .types import *  # noqa: F401,F403  (contract types)',
    '',
    footer('#', idl),
    '',
  ].join('\n');
}

/** Generate every Python artefact for the given IDL. */
export function generatePython(idl: ContractIdl, outDir: string = PY_OUTPUT_DIR): GeneratedFile[] {
  void enums;
  void structs;
  return [
    { path: joinPath(outDir, 'types.py'), contents: generateTypes(idl) },
    { path: joinPath(outDir, 'errors.py'), contents: generateErrors(idl) },
    { path: joinPath(outDir, 'events.py'), contents: generateEvents(idl) },
    { path: joinPath(outDir, 'client.py'), contents: generateClient(idl) },
    { path: joinPath(outDir, '__init__.py'), contents: generateInit(idl) },
  ];
}
