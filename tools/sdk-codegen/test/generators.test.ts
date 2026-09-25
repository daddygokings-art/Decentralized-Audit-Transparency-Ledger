import { describe, expect, it } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { extractIdlFromSource } from '../src/abi/extract';
import { generatePython } from '../src/generators/python';
import { generateRust } from '../src/generators/rust';
import { generateTypeScript } from '../src/generators/typescript';
import { checkCompatibility, isVersionConsistent } from '../src/compat';
import { ContractIdl } from '../src/abi/types';

const FIXTURE = path.join(__dirname, 'fixtures', 'mini-contract.rs');
const idl: ContractIdl = extractIdlFromSource(fs.readFileSync(FIXTURE, 'utf8'), { source: 'mini-contract.rs' }).idl;

const filesOf = (fs_: Array<{ path: string; contents: string }>): Map<string, string> =>
  new Map(fs_.map((f) => [f.path, f.contents]));

describe('generators', () => {
  it('emits one file per artefact for each language', () => {
    expect(filesOf(generateTypeScript(idl)).size).toBe(5);
    expect(filesOf(generatePython(idl)).size).toBe(5);
    expect(filesOf(generateRust(idl)).size).toBe(5);
  });

  it('is deterministic', () => {
    expect(generateTypeScript(idl)).toEqual(generateTypeScript(idl));
    expect(generatePython(idl)).toEqual(generatePython(idl));
    expect(generateRust(idl)).toEqual(generateRust(idl));
  });

  it('stamps every file with a do-not-edit banner and the IDL fingerprint', () => {
    for (const files of [generateTypeScript(idl), generatePython(idl), generateRust(idl)]) {
      for (const file of files) {
        expect(file.contents).toContain('DO NOT EDIT');
        expect(file.contents).toContain('spec_version 1.0.0');
      }
    }
  });

  it('terminates every TypeScript block comment it opens', () => {
    for (const file of generateTypeScript(idl)) {
      const opens = (file.contents.match(/\/\*\*/g) ?? []).length;
      const closes = (file.contents.match(/\*\//g) ?? []).length;
      expect(closes).toBe(opens);
    }
  });

  it('keeps wire names verbatim in the invoke call', () => {
    const ts = filesOf(generateTypeScript(idl)).get('sdk/js/src/generated/client.ts')!;
    expect(ts).toMatch(/invoke<[^>]*>\('load', \[index\]\)/);
    const py = filesOf(generatePython(idl)).get('sdk/python/audit_ledger/generated/client.py')!;
    expect(py).toMatch(/invoke\("load", \[index\]\)/);
    const rs = filesOf(generateRust(idl)).get('sdk/rust/src/generated/client.rs')!;
    expect(rs).toMatch(/invoke\("load", args\)/);
  });

  it('keeps contract field names verbatim in Python payloads', () => {
    const types = filesOf(generatePython(idl)).get('sdk/python/audit_ledger/generated/types.py')!;
    expect(types).toContain('"label": self.label');
    expect(types).toContain('children: List[Inner]');
  });

  it('renames Python identifiers that collide with keywords', () => {
    const withNone: ContractIdl = {
      ...idl,
      types: [
        {
          kind: 'enum',
          name: 'Policy',
          variants: [
            { name: 'None', value: 0 },
            { name: 'class', value: 1 },
            { name: 'Strict', value: 2 },
          ],
        },
      ],
      functions: [],
      events: [],
    };
    const types = filesOf(generatePython(withNone)).get('sdk/python/audit_ledger/generated/types.py')!;
    expect(types).toContain('None_ = 0');
    expect(types).toContain('class_ = 1');
    expect(types).toContain('Strict = 2');
  });

  it('imports the contract types a generated client references', () => {
    const client = filesOf(generateTypeScript(idl)).get('sdk/js/src/generated/client.ts')!;
    expect(client).toMatch(/^import type \{ Mode, Outer \} from '\.\/types';$/m);
    const py = filesOf(generatePython(idl)).get('sdk/python/audit_ledger/generated/client.py')!;
    expect(py).toMatch(/^from \.types import Mode, Outer$/m);
  });

  it('generates Rust that is already rustfmt-clean', () => {
    const { spawnSync } = require('child_process');
    if (spawnSync('rustfmt', ['--version']).status !== 0) return; // optional toolchain
    const os = require('os');
    const staged = generateRust(idl);
    const dir = path.join(os.tmpdir(), `sdk-codegen-rf-${process.pid}-${Math.random().toString(36).slice(2)}`);
    fs.mkdirSync(dir, { recursive: true });
    const config = path.join(__dirname, '..', '..', '..', 'rustfmt.toml');
    try {
      for (const f of staged) fs.writeFileSync(path.join(dir, path.basename(f.path)), f.contents, 'utf8');
      const res = spawnSync(
        'rustfmt',
        ['--edition', '2021', ...(fs.existsSync(config) ? ['--config-path', path.dirname(config)] : []), path.join(dir, 'mod.rs')],
        { encoding: 'utf8' },
      );
      expect(res.status).toBe(0);
      for (const f of staged) {
        expect(fs.readFileSync(path.join(dir, path.basename(f.path)), 'utf8')).toBe(f.contents);
      }
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('checkCompatibility', () => {
  const base: ContractIdl = {
    ...idl,
    functions: [
      { name: 'alpha', inputs: [{ name: 'x', type: { kind: 'u32' } }], output: { kind: 'void' }, mutating: true },
      { name: 'beta', inputs: [], output: { kind: 'u32' }, mutating: false },
    ],
  };

  it('reports no findings for an identical IDL', () => {
    const report = checkCompatibility(base, base);
    expect(report.findings).toEqual([]);
    expect(report.severity).toBe('none');
  });

  it('flags a removed function as breaking', () => {
    const report = checkCompatibility(base, { ...base, functions: [base.functions[1]] });
    expect(report.severity).toBe('breaking');
    expect(report.findings.some((f) => f.path.includes('alpha'))).toBe(true);
  });

  it('flags a changed parameter type as breaking', () => {
    const report = checkCompatibility(base, {
      ...base,
      functions: [{ ...base.functions[0], inputs: [{ name: 'x', type: { kind: 'u64' } }] }, base.functions[1]],
    });
    expect(report.severity).toBe('breaking');
  });

  it('flags a renumbered error code as breaking', () => {
    const report = checkCompatibility(
      { ...base, errors: [{ name: 'Boom', code: 1 }] },
      { ...base, errors: [{ name: 'Boom', code: 2 }] },
    );
    expect(report.severity).toBe('breaking');
  });

  it('treats an added function as a non-breaking addition', () => {
    const report = checkCompatibility(base, {
      ...base,
      functions: [...base.functions, { name: 'gamma', inputs: [], output: { kind: 'void' }, mutating: true }],
    });
    expect(report.severity).toBe('minor');
  });

  it('requires the spec_version to be bumped for every detected change', () => {
    const breaking = { ...base, functions: [base.functions[1]] };
    expect(isVersionConsistent(checkCompatibility(base, breaking))).toBe(false);
    const bumped = { ...breaking, spec_version: '2.0.0' };
    expect(isVersionConsistent(checkCompatibility(base, bumped))).toBe(true);
  });
});
