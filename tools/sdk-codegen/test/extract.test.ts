import { describe, expect, it } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { extractIdlFromSource } from '../src/abi/extract';
import { assertValidIdl, validateIdl } from '../src/abi/validate';
import { IdlType } from '../src/abi/types';

const FIXTURE = path.join(__dirname, 'fixtures', 'mini-contract.rs');
const src = fs.readFileSync(FIXTURE, 'utf8');

const extract = () => extractIdlFromSource(src, { source: 'mini-contract.rs' });

function fieldType(typeName: string, field: string): IdlType {
  const def = extract().idl.types.find((t) => t.name === typeName && (t.kind === 'struct' || t.kind === 'enum'));
  if (!def || !('fields' in def) || !def.fields) throw new Error(`type ${typeName} not found`);
  const found = def.fields.find((f) => f.name === field);
  if (!found) throw new Error(`field ${typeName}.${field} not found`);
  return found.type;
}

describe('extractIdlFromSource', () => {
  it('reports no warnings for the fixture contract', () => {
    expect(extract().warnings).toEqual([]);
  });

  it('produces a structurally valid IDL', () => {
    const { idl } = extract();
    expect(() => assertValidIdl(idl)).not.toThrow();
    expect(validateIdl(idl)).toEqual([]);
  });

  it('extracts the contract surface', () => {
    const { idl } = extract();
    expect(idl.contract.name).toBe('Mini');
    expect(idl.functions.map((f) => f.name).sort()).toEqual(['batch', 'load', 'modes', 'store']);
    expect(idl.events.map((e) => e.name).sort()).toEqual(['stored']);
    expect(idl.types.map((t) => t.name).sort()).toEqual(['Inner', 'Mode', 'Outer']);
  });

  it('omits the host Env parameter and keeps return types', () => {
    const { idl } = extract();
    const store = idl.functions.find((f) => f.name === 'store')!;
    expect(store.inputs.map((i) => i.name)).toEqual(['record']);
    expect(store.output).toEqual({ kind: 'u32' });
    expect(store.mutating).toBe(true);
    expect(idl.functions.find((f) => f.name === 'load')!.mutating).toBe(false);
  });

  it('keeps tuple parameters nested inside a Vec', () => {
    const { idl } = extract();
    const batch = idl.functions.find((f) => f.name === 'batch')!;
    expect(batch.inputs[0].type).toEqual({
      kind: 'vec',
      of: { kind: 'tuple', of: [{ kind: 'address' }, { kind: 'symbol' }, { kind: 'bytes' }] },
    });
  });

  it('preserves nested struct, vec and option field types', () => {
    expect(fieldType('Outer', 'children')).toEqual({ kind: 'vec', of: { kind: 'type', name: 'Inner' } });
    expect(fieldType('Outer', 'lookup')).toEqual({ kind: 'option', of: { kind: 'address' } });
    expect(fieldType('Outer', 'tags')).toEqual({ kind: 'vec', of: { kind: 'symbol' } });
  });

  it('records payload-carrying enum variants with their fields', () => {
    const { idl } = extract();
    const mode = idl.types.find((t) => t.name === 'Mode');
    if (!mode || mode.kind !== 'enum') throw new Error('Mode enum missing');
    const on = mode.variants.find((v) => v.name === 'On')!;
    expect(on.fields).toEqual([{ name: 'field_0', type: { kind: 'u32' } }]);
    const named = mode.variants.find((v) => v.name === 'Named')!;
    expect(named.fields).toEqual([{ name: 'label', type: { kind: 'symbol' } }]);
    expect(mode.variants.find((v) => v.name === 'Off')!.fields).toBeUndefined();
  });

  it('resolves event payload types through locals and member access', () => {
    const { idl } = extract();
    const stored = idl.events.find((e) => e.name === 'stored')!;
    // `env.ledger().timestamp()` -> u64, `record.owner.clone()` -> address.
    expect(stored.data).toEqual([{ kind: 'u64' }, { kind: 'address' }]);
  });

  it('is deterministic across runs', () => {
    expect(JSON.stringify(extract().idl)).toBe(JSON.stringify(extract().idl));
  });
});
