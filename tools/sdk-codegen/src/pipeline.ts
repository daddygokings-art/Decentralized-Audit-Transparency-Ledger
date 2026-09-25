import * as fs from 'fs';
import * as path from 'path';
import { ContractIdl } from './abi/types';
import { extractIdlFromFile, extractIdlFromSource, idlFromSorobanBindings, serializeIdl } from './abi/extract';
import { assertValidIdl, validateIdl } from './abi/validate';
import { generatePython } from './generators/python';
import { generateRust } from './generators/rust';
import { generateTypeScript } from './generators/typescript';
import { GeneratedFile, Language } from './generators/common';
import { CompatibilityReport, checkCompatibility, isVersionConsistent } from './compat';

export const DEFAULT_IDL_PATH = 'abi/audit-ledger.json';
export const DEFAULT_CONTRACT_SOURCE = 'src/lib.rs';

/** Repository root, derived from this file's location so the CLI is CWD-independent. */
export const REPO_ROOT = path.resolve(__dirname, '..', '..', '..');

/** Resolve a repo-relative path, leaving absolute paths untouched. */
export function resolveRepoPath(target: string): string {
  return path.isAbsolute(target) ? target : path.join(REPO_ROOT, target);
}

export interface WriteResult {
  written: string[];
  unchanged: string[];
  removed: string[];
}

/** Read + validate an IDL from disk. */
export function loadIdl(idlPath: string): ContractIdl {
  const raw = fs.readFileSync(idlPath, 'utf8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new Error(`${idlPath} is not valid JSON: ${(err as Error).message}`);
  }
  assertValidIdl(parsed);
  return parsed;
}

/** All artefacts a full generation run produces, without touching the filesystem. */
export function planGeneration(idl: ContractIdl, outRoot = '.', languages: Language[] = ['typescript', 'python', 'rust']): GeneratedFile[] {
  const files: GeneratedFile[] = [];
  if (languages.includes('typescript')) files.push(...generateTypeScript(idl));
  if (languages.includes('python')) files.push(...generatePython(idl));
  if (languages.includes('rust')) files.push(...generateRust(idl));
  return files.map((f) => ({ ...f, path: path.posix.join(outRoot.replace(/\\/g, '/').replace(/\/+$/, ''), f.path) }));
}

export interface SyncOptions {
  outRoot?: string;
  languages?: Language[];
  /** When true nothing is written; the returned result reports the drift instead. */
  check?: boolean;
  /** Write `--- header ---` provenance comments into the IDL? No-op placeholder for symmetry. */
  languagesFilter?: Language[];
}

/**
 * Write generated artefacts, reporting which files changed.
 *
 * Files are only rewritten when their contents differ, which keeps mtimes (and
 * therefore downstream build caches) stable across no-op regenerations.
 */
export function syncGeneratedFiles(idl: ContractIdl, options: SyncOptions = {}): WriteResult {
  const files = planGeneration(idl, options.outRoot ?? '.', options.languages ?? ['typescript', 'python', 'rust']);
  const written: string[] = [];
  const unchanged: string[] = [];

  for (const file of files) {
    const exists = fs.existsSync(file.path);
    if (exists && fs.readFileSync(file.path, 'utf8') === file.contents) {
      unchanged.push(file.path);
      continue;
    }
    if (!options.check) {
      fs.mkdirSync(path.dirname(file.path), { recursive: true });
      fs.writeFileSync(file.path, file.contents, 'utf8');
    }
    written.push(file.path);
  }

  return { written, unchanged, removed: [] };
}

export interface ExtractOptionsCli {
  source: string;
  output: string;
  check: boolean;
  specVersion?: string;
  contractVersion?: string;
  sorobanSdkVersion?: string;
  bindings?: string;
}

/** Regenerate the committed IDL from the contract, and report drift. */
export function runExtract(options: ExtractOptionsCli): { changed: boolean; idl: ContractIdl; warnings: string[]; contents: string } {
  const result = options.bindings
    ? idlFromSorobanBindings(JSON.parse(fs.readFileSync(options.bindings, 'utf8')), {
        source: options.source,
        specVersion: options.specVersion,
        contractVersion: options.contractVersion,
        sorobanSdkVersion: options.sorobanSdkVersion,
      })
    : extractIdlFromFile(options.source, {
        source: options.source,
        specVersion: options.specVersion,
        contractVersion: options.contractVersion,
        sorobanSdkVersion: options.sorobanSdkVersion,
      });

  assertValidIdl(result.idl);
  const contents = serializeIdl(result.idl);
  const previous = fs.existsSync(options.output) ? fs.readFileSync(options.output, 'utf8') : null;
  const changed = previous !== contents;

  if (changed && !options.check) {
    fs.mkdirSync(path.dirname(options.output), { recursive: true });
    fs.writeFileSync(options.output, contents, 'utf8');
  }
  return { changed, idl: result.idl, warnings: result.warnings, contents };
}

export interface CompatOptions {
  baseline: string;
  candidate: string;
}

/** Run the compatibility check between two IDL files. */
export function runCompat(options: CompatOptions): CompatibilityReport {
  const baseline = loadIdl(options.baseline);
  const candidate = loadIdl(options.candidate);
  return checkCompatibility(baseline, candidate);
}

/** True when every artefact on disk already matches what would be generated. */
export function generatedFilesInSync(idl: ContractIdl, outRoot = '.'): { inSync: boolean; drift: string[] } {
  const files = planGeneration(idl, outRoot);
  const drift: string[] = [];
  for (const file of files) {
    if (!fs.existsSync(file.path)) {
      drift.push(`missing: ${file.path}`);
      continue;
    }
    if (fs.readFileSync(file.path, 'utf8') !== file.contents) {
      drift.push(`stale:   ${file.path}`);
    }
  }
  return { inSync: drift.length === 0, drift };
}

export { checkCompatibility, isVersionConsistent, extractIdlFromSource, validateIdl };
export type { CompatibilityReport, GeneratedFile, Language };
