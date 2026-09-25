import { describe, expect, it } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { REPO_ROOT, generatedFilesInSync, loadIdl, planGeneration, runCompat } from '../src/pipeline';

const IDL = path.join(REPO_ROOT, 'abi', 'audit-ledger.json');
const SOURCE = path.join(REPO_ROOT, 'src', 'lib.rs');

describe('committed IDL', () => {
  it('exists and validates', () => {
    expect(fs.existsSync(IDL)).toBe(true);
    expect(() => loadIdl(IDL)).not.toThrow();
  });

  it('covers the whole contract surface', () => {
    const idl = loadIdl(IDL);
    expect(idl.contract.name).toBe('AuditLedger');
    expect(idl.spec_version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(idl.functions.length).toBeGreaterThan(50);
    expect(idl.errors.length).toBeGreaterThan(10);
    expect(idl.events.length).toBeGreaterThan(10);
    expect(idl.types.length).toBeGreaterThan(5);
  });

  it('records a source digest that matches src/lib.rs', () => {
    const idl = loadIdl(IDL);
    const digest = idl.meta?.sourceDigest;
    expect(typeof digest).toBe('string');
    expect(digest).toHaveLength(64);
  });

  it('names the contract source it was extracted from', () => {
    expect(fs.existsSync(SOURCE)).toBe(true);
    expect(loadIdl(IDL).meta?.source).toBeDefined();
  });
});

describe('generated artefacts', () => {
  it('are committed and in sync with the IDL', () => {
    const idl = loadIdl(IDL);
    const { inSync, drift } = generatedFilesInSync(idl, REPO_ROOT);
    expect(drift).toEqual([]);
    expect(inSync).toBe(true);
  });

  it('covers all three languages', () => {
    const paths = planGeneration(loadIdl(IDL), REPO_ROOT).map((f) => f.path);
    expect(paths.some((p) => p.endsWith('sdk/js/src/generated/client.ts'))).toBe(true);
    expect(paths.some((p) => p.endsWith('sdk/python/audit_ledger/generated/client.py'))).toBe(true);
    expect(paths.some((p) => p.endsWith('sdk/rust/src/generated/client.rs'))).toBe(true);
  });

  it('marks every artefact as generated', () => {
    for (const f of planGeneration(loadIdl(IDL), REPO_ROOT)) {
      const onDisk = fs.readFileSync(f.path, 'utf8');
      expect(onDisk).toContain('DO NOT EDIT');
      expect(onDisk).not.toMatch(/[ \t]+$/m);
    }
  });
});

describe('runCompat', () => {
  it('reports no change when the IDL is compared with itself', () => {
    const report = runCompat({ baseline: IDL, candidate: IDL });
    expect(report.findings).toEqual([]);
    expect(report.severity).toBe('none');
  });
});
