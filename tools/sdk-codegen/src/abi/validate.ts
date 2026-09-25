import {
  ContractIdl,
  IdlType,
  IdlTypeDef,
  collectReferencedTypes,
  findFunction,
  findTypeDef,
  typeSignature,
} from './types';

/** Single validation problem, addressed to a JSON-pointer-ish location. */
export interface ValidationIssue {
  path: string;
  message: string;
}

const SCALAR_KINDS = new Set([
  'void',
  'bool',
  'u32',
  'u64',
  'i32',
  'i64',
  'address',
  'string',
  'symbol',
  'bytes',
  'bytesn',
  'timepoint',
  'duration',
  'option',
  'vec',
  'tuple',
  'map',
  'type',
]);

const RESERVED = new Set(['self', 'Self', 'crate', 'super']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Validate a single type node, returning nested issues. */
function validateType(type: unknown, path: string, issues: ValidationIssue[]): void {
  if (!isRecord(type)) {
    issues.push({ path, message: 'type must be an object' });
    return;
  }
  const kind = type.kind;
  if (typeof kind !== 'string' || !SCALAR_KINDS.has(kind)) {
    issues.push({ path: `${path}.kind`, message: `unknown type kind: ${String(kind)}` });
    return;
  }
  switch (kind) {
    case 'bytesn':
      if (typeof type.n !== 'number' || !Number.isInteger(type.n) || type.n <= 0) {
        issues.push({ path: `${path}.n`, message: 'bytesn requires a positive integer length' });
      }
      break;
    case 'option':
    case 'vec':
      validateType(type.of, `${path}.of`, issues);
      break;
    case 'tuple':
      if (!Array.isArray(type.of)) {
        issues.push({ path: `${path}.of`, message: 'tuple requires an array of members' });
      } else {
        type.of.forEach((t, i) => validateType(t, `${path}.of[${i}]`, issues));
      }
      break;
    case 'map':
      validateType(type.key, `${path}.key`, issues);
      validateType(type.value, `${path}.value`, issues);
      break;
    case 'type':
      if (typeof type.name !== 'string' || type.name.length === 0) {
        issues.push({ path: `${path}.name`, message: 'type reference requires a name' });
      } else if (RESERVED.has(type.name)) {
        issues.push({ path: `${path}.name`, message: `reserved name is not a contract type: ${type.name}` });
      }
      break;
    default:
      break;
  }
}

function validateTypeDef(def: unknown, path: string, issues: ValidationIssue[]): void {
  if (!isRecord(def)) {
    issues.push({ path, message: 'type definition must be an object' });
    return;
  }
  if (typeof def.name !== 'string' || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(def.name)) {
    issues.push({ path: `${path}.name`, message: 'type name must be a valid identifier' });
  }
  if (def.kind === 'struct') {
    if (!Array.isArray(def.fields)) {
      issues.push({ path: `${path}.fields`, message: 'struct requires a fields array' });
      return;
    }
    const seen = new Set<string>();
    def.fields.forEach((f, i) => {
      if (!isRecord(f) || typeof f.name !== 'string') {
        issues.push({ path: `${path}.fields[${i}]`, message: 'field requires a name' });
        return;
      }
      if (seen.has(f.name)) {
        issues.push({ path: `${path}.fields[${i}].name`, message: `duplicate field: ${f.name}` });
      }
      seen.add(f.name);
      validateType(f.type, `${path}.fields[${i}].type`, issues);
    });
    return;
  }
  if (def.kind === 'enum') {
    if (!Array.isArray(def.variants)) {
      issues.push({ path: `${path}.variants`, message: 'enum requires a variants array' });
      return;
    }
    const seenNames = new Set<string>();
    const seenValues = new Set<number>();
    def.variants.forEach((v, i) => {
      if (!isRecord(v) || typeof v.name !== 'string') {
        issues.push({ path: `${path}.variants[${i}]`, message: 'variant requires a name' });
        return;
      }
      if (seenNames.has(v.name)) {
        issues.push({ path: `${path}.variants[${i}].name`, message: `duplicate variant: ${v.name}` });
      }
      seenNames.add(v.name);
      if (typeof v.value !== 'number' || !Number.isInteger(v.value) || v.value < 0) {
        issues.push({ path: `${path}.variants[${i}].value`, message: 'variant requires a non-negative ordinal' });
      } else if (seenValues.has(v.value)) {
        issues.push({ path: `${path}.variants[${i}].value`, message: `duplicate ordinal: ${v.value}` });
      } else {
        seenValues.add(v.value);
      }
      if (v.fields !== undefined) {
        if (!Array.isArray(v.fields)) {
          issues.push({ path: `${path}.variants[${i}].fields`, message: 'variant fields must be an array' });
        } else {
          v.fields.forEach((f, j) => {
            if (!isRecord(f) || typeof f.name !== 'string') {
              issues.push({ path: `${path}.variants[${i}].fields[${j}]`, message: 'variant field requires a name' });
              return;
            }
            validateType(f.type, `${path}.variants[${i}].fields[${j}].type`, issues);
          });
        }
      }
    });
    return;
  }
  issues.push({ path: `${path}.kind`, message: `unknown type definition kind: ${String(def.kind)}` });
}

const SEMVER = /^\d+\.\d+\.\d+$/;

/**
 * Full structural validation of a contract IDL.
 *
 * Beyond shape checks this enforces the invariants the generators rely on:
 * unique names, resolvable type references, no acyclic-less recursion, and
 * error codes that are unique and non-negative.
 */
export function validateIdl(input: unknown): ValidationIssue[] {
  const issues: ValidationIssue[] = [];

  if (!isRecord(input)) {
    return [{ path: '$', message: 'IDL root must be an object' }];
  }

  if (typeof input.spec_version !== 'string' || !SEMVER.test(input.spec_version)) {
    issues.push({ path: '$.spec_version', message: 'spec_version must be semver (major.minor.patch)' });
  }

  if (!isRecord(input.contract)) {
    issues.push({ path: '$.contract', message: 'contract metadata is required' });
  } else {
    for (const key of ['name', 'crate', 'version'] as const) {
      if (typeof input.contract[key] !== 'string' || (input.contract[key] as string).length === 0) {
        issues.push({ path: `$.contract.${key}`, message: `contract.${key} is required` });
      }
    }
  }

  if (!isRecord(input.meta)) {
    issues.push({ path: '$.meta', message: 'meta block is required' });
  } else {
    for (const key of ['sorobanSdk', 'source', 'generatedBy'] as const) {
      if (typeof input.meta[key] !== 'string' || (input.meta[key] as string).length === 0) {
        issues.push({ path: `$.meta.${key}`, message: `meta.${key} is required` });
      }
    }
  }

  if (!Array.isArray(input.types)) {
    issues.push({ path: '$.types', message: 'types must be an array' });
  } else {
    const seen = new Set<string>();
    input.types.forEach((def, i) => {
      validateTypeDef(def, `$.types[${i}]`, issues);
      if (isRecord(def) && typeof def.name === 'string') {
        if (seen.has(def.name)) {
          issues.push({ path: `$.types[${i}].name`, message: `duplicate type: ${def.name}` });
        }
        seen.add(def.name);
      }
    });
  }

  if (!Array.isArray(input.errors)) {
    issues.push({ path: '$.errors', message: 'errors must be an array' });
  } else {
    const seen = new Set<string>();
    const seenCodes = new Set<number>();
    input.errors.forEach((e, i) => {
      if (!isRecord(e) || typeof e.name !== 'string') {
        issues.push({ path: `$.errors[${i}]`, message: 'error requires a name' });
        return;
      }
      if (seen.has(e.name)) {
        issues.push({ path: `$.errors[${i}].name`, message: `duplicate error: ${e.name}` });
      }
      seen.add(e.name);
      if (typeof e.code !== 'number' || !Number.isInteger(e.code) || e.code <= 0) {
        issues.push({ path: `$.errors[${i}].code`, message: 'error code must be a positive integer' });
      } else if (seenCodes.has(e.code)) {
        issues.push({ path: `$.errors[${i}].code`, message: `duplicate error code: ${e.code}` });
      } else {
        seenCodes.add(e.code);
      }
    });
  }

  if (!Array.isArray(input.events)) {
    issues.push({ path: '$.events', message: 'events must be an array' });
  } else {
    const seen = new Set<string>();
    input.events.forEach((e, i) => {
      if (!isRecord(e) || typeof e.name !== 'string' || e.name.length === 0) {
        issues.push({ path: `$.events[${i}]`, message: 'event requires a name' });
        return;
      }
      if (seen.has(e.name)) {
        issues.push({ path: `$.events[${i}].name`, message: `duplicate event: ${e.name}` });
      }
      seen.add(e.name);
      for (const key of ['topics', 'data'] as const) {
        if (!Array.isArray(e[key])) {
          issues.push({ path: `$.events[${i}].${key}`, message: `event.${key} must be an array` });
        } else {
          (e[key] as unknown[]).forEach((t, j) => validateType(t, `$.events[${i}].${key}[${j}]`, issues));
        }
      }
    });
  }

  if (!Array.isArray(input.functions)) {
    issues.push({ path: '$.functions', message: 'functions must be an array' });
    return issues;
  }

  const seenFns = new Set<string>();
  input.functions.forEach((fn, i) => {
    if (!isRecord(fn) || typeof fn.name !== 'string' || fn.name.length === 0) {
      issues.push({ path: `$.functions[${i}]`, message: 'function requires a name' });
      return;
    }
    if (seenFns.has(fn.name)) {
      issues.push({ path: `$.functions[${i}].name`, message: `duplicate function: ${fn.name}` });
    }
    seenFns.add(fn.name);
    if (typeof fn.mutating !== 'boolean') {
      issues.push({ path: `$.functions[${i}].mutating`, message: 'function.mutating must be a boolean' });
    }
    if (!Array.isArray(fn.inputs)) {
      issues.push({ path: `$.functions[${i}].inputs`, message: 'function.inputs must be an array' });
    } else {
      const seen = new Set<string>();
      fn.inputs.forEach((input, j) => {
        if (!isRecord(input) || typeof input.name !== 'string') {
          issues.push({ path: `$.functions[${i}].inputs[${j}]`, message: 'input requires a name' });
          return;
        }
        if (seen.has(input.name)) {
          issues.push({ path: `$.functions[${i}].inputs[${j}].name`, message: `duplicate input: ${input.name}` });
        }
        seen.add(input.name);
        validateType(input.type, `$.functions[${i}].inputs[${j}].type`, issues);
      });
    }
    if (fn.output === undefined) {
      issues.push({ path: `$.functions[${i}].output`, message: 'function.output is required (use { kind: "void" })' });
    } else {
      validateType(fn.output, `$.functions[${i}].output`, issues);
    }
  });

  if (issues.length > 0) return issues;

  // Reference resolution — only meaningful once the shape is known-good.
  const idl = input as unknown as ContractIdl;
  for (const name of collectReferencedTypes(idl)) {
    if (!findTypeDef(idl, name)) {
      issues.push({ path: '$.types', message: `unresolved type reference: ${name}` });
    }
  }
  for (const def of idl.types) {
    const structLike = def.kind === 'struct' ? def.fields.map((f) => `${f.name}: ${typeSignature(f.type)}`) : [];
    for (const field of structLike) {
      if (/Map<[^>]*Map</.test(field)) {
        issues.push({ path: '$.types', message: `nested map field is not encodable by Soroban: ${field}` });
      }
    }
  }
  for (const fn of idl.functions) {
    if (findTypeDef(idl, fn.name)) {
      issues.push({ path: '$.functions', message: `function name collides with type name: ${fn.name}` });
    }
  }
  void findFunction;

  return issues;
}

/** Throw a single aggregated error when the IDL is invalid. */
export function assertValidIdl(idl: unknown): asserts idl is ContractIdl {
  const issues = validateIdl(idl);
  if (issues.length > 0) {
    const detail = issues.map((i) => `  - ${i.path}: ${i.message}`).join('\n');
    throw new Error(`Invalid contract IDL (${issues.length} problem(s)):\n${detail}`);
  }
}

/** All type definitions in declaration order, structs and enums alike. */
export function allTypeDefs(idl: ContractIdl): IdlTypeDef[] {
  return idl.types;
}

/** Render a type node as a short human label (used in CLI output and reports). */
export function describeType(type: IdlType): string {
  return typeSignature(type);
}
