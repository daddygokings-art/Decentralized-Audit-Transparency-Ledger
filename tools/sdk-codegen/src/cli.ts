#!/usr/bin/env node

import * as fs from 'fs';
import * as path from 'path';
import { Command } from 'commander';
import { DEFAULT_CONTRACT_SOURCE, DEFAULT_IDL_PATH, REPO_ROOT, generatedFilesInSync, loadIdl, resolveRepoPath, runCompat, runExtract, syncGeneratedFiles } from './pipeline';
import type { Language } from './generators/common';

const VERSION = '1.0.0';

/** Display a repo-relative path so CLI output is CWD-independent. */
function display(target: string): string {
  return path.relative(process.cwd(), target) || target;
}

function parseLanguages(value: string): Language[] {
  const requested = value
    .split(',')
    .map((v) => v.trim().toLowerCase())
    .filter(Boolean);
  const aliases: Record<string, Language> = {
    ts: 'typescript',
    typescript: 'typescript',
    js: 'typescript',
    javascript: 'typescript',
    py: 'python',
    python: 'python',
    rs: 'rust',
    rust: 'rust',
  };
  const out: Language[] = [];
  for (const r of requested) {
    const lang = aliases[r];
    if (!lang) {
      throw new Error(`Unknown language "${r}". Expected one of: typescript, python, rust.`);
    }
    if (!out.includes(lang)) out.push(lang);
  }
  return out;
}

const program = new Command();

program
  .name('sdk-codegen')
  .description('AuditLedger SDK code generation from the Soroban contract ABI/IDL (#407)')
  .version(VERSION);

program
  .command('extract')
  .description('Regenerate the committed contract IDL from the contract source (or Soroban bindings)')
  .option('-s, --source <file>', 'contract Rust source file', DEFAULT_CONTRACT_SOURCE)
  .option('-o, --output <file>', 'where to write the IDL', DEFAULT_IDL_PATH)
  .option('-b, --bindings <file>', 'read a `soroban contract bindings json` payload instead of parsing Rust source')
  .option('--spec-version <ver>', 'explicit IDL spec_version (default: keep the previous value)')
  .option('--contract-version <ver>', 'contract version recorded in the IDL')
  .option('--soroban-sdk <ver>', 'soroban-sdk version recorded in the IDL')
  .option('--check', 'fail if the IDL on disk is not up to date', false)
  .action((opts) => {
    const source = opts.bindings ? resolveRepoPath(opts.bindings) : resolveRepoPath(opts.source);
    const output = resolveRepoPath(opts.output);
    if (!fs.existsSync(source)) {
      console.error(`Contract source not found: ${display(source)}`);
      process.exit(1);
    }
    const previousSpec = fs.existsSync(output)
      ? (JSON.parse(fs.readFileSync(output, 'utf8')) as { spec_version?: string }).spec_version
      : undefined;

    const result = runExtract({
      source,
      output,
      check: Boolean(opts.check),
      specVersion: opts.specVersion ?? previousSpec,
      contractVersion: opts.contractVersion,
      sorobanSdkVersion: opts.sorobanSdk,
      bindings: opts.bindings ? source : undefined,
    });

    for (const w of result.warnings) console.warn(`  warn: ${w}`);
    const { idl } = result;
    console.log(`Extracted IDL from ${display(source)}`);
    console.log(`  contract      : ${idl.contract.name} v${idl.contract.version}`);
    console.log(`  spec_version  : ${idl.spec_version}`);
    console.log(`  functions     : ${idl.functions.length}`);
    console.log(`  types         : ${idl.types.length}`);
    console.log(`  errors        : ${idl.errors.length}`);
    console.log(`  events        : ${idl.events.length}`);
    if (result.changed) {
      console.log(opts.check ? `  DRIFT: ${display(output)} is out of date` : `  wrote ${display(output)}`);
    } else {
      console.log(`  ${display(output)} already up to date`);
    }
    if (opts.check && result.changed) process.exit(1);
  });

program
  .command('generate')
  .description('Generate the TypeScript, Python and Rust SDK bindings from the IDL')
  .option('-i, --idl <file>', 'contract IDL to read', DEFAULT_IDL_PATH)
  .option('-l, --languages <list>', 'comma-separated: typescript, python, rust')
  .option('--check', 'do not write; exit non-zero when artefacts are stale', false)
  .action((opts) => {
    const idl = loadIdl(resolveRepoPath(opts.idl));
    const languages = opts.languages ? parseLanguages(opts.languages) : (['typescript', 'python', 'rust'] as Language[]);
    const result = syncGeneratedFiles(idl, { outRoot: REPO_ROOT, languages, check: Boolean(opts.check) });
    const verb = opts.check ? 'would update' : 'wrote';
    for (const f of result.written) console.log(`  ${verb}: ${display(f)}`);
    for (const f of result.unchanged) console.log(`  up to date: ${display(f)}`);
    if (opts.check && result.written.length > 0) {
      console.error('\nGenerated SDK files are out of date. Run: sdk-codegen generate');
      process.exit(1);
    }
  });

program
  .command('check')
  .description('Validate the IDL and confirm every generated artefact matches it')
  .option('-i, --idl <file>', 'contract IDL to read', DEFAULT_IDL_PATH)
  .action((opts) => {
    const idl = loadIdl(resolveRepoPath(opts.idl));
    console.log(`IDL ${display(resolveRepoPath(opts.idl))} is valid (spec_version ${idl.spec_version})`);
    const { inSync, drift } = generatedFilesInSync(idl, REPO_ROOT);
    if (inSync) {
      console.log('All generated SDK artefacts are in sync.');
      return;
    }
    console.error('Generated SDK artefacts are out of sync:');
    for (const d of drift) console.error(`  ${display(d)}`);
    console.error('\nRun: sdk-codegen generate');
    process.exit(1);
  });

program
  .command('compat')
  .description('Check an IDL against a baseline for breaking contract-surface changes')
  .requiredOption('-b, --baseline <file>', 'baseline (previous) IDL')
  .requiredOption('-c, --candidate <file>', 'candidate (new) IDL')
  .option('--json', 'emit the report as JSON', false)
  .action((opts) => {
    const report = runCompat({ baseline: resolveRepoPath(opts.baseline), candidate: resolveRepoPath(opts.candidate) });
    if (opts.json) {
      console.log(JSON.stringify(report, null, 2));
    } else {
      console.log(`IDL compatibility: ${report.from} -> ${report.to}`);
      console.log(`  severity         : ${report.severity}`);
      console.log(`  expected version : ${report.expectedSpecVersion}`);
      for (const f of report.findings) console.log(`  [${f.severity}] ${f.path}: ${f.message}`);
      if (report.findings.length === 0) console.log('  no changes detected');
    }
    if (report.severity === 'breaking') process.exit(1);
  });

program
  .command('verify')
  .description('Run the full CI gate: extract drift, generated-file drift, and compatibility')
  .option('-i, --idl <file>', 'contract IDL', DEFAULT_IDL_PATH)
  .option('-s, --source <file>', 'contract source', DEFAULT_CONTRACT_SOURCE)
  .action((opts) => {
    let failed = false;

    const idlPath = resolveRepoPath(opts.idl);
    const sourcePath = resolveRepoPath(opts.source);

    // 1. The IDL must match the contract source.
    if (fs.existsSync(sourcePath)) {
      const extract = runExtract({
        source: sourcePath,
        output: idlPath,
        check: true,
        specVersion: (JSON.parse(fs.readFileSync(idlPath, 'utf8')) as { spec_version?: string }).spec_version,
      });
      if (extract.changed) {
        console.error(`FAIL: ${display(idlPath)} does not match ${display(sourcePath)}. Run: sdk-codegen extract`);
        failed = true;
      } else {
        console.log(`ok: ${display(idlPath)} matches ${display(sourcePath)}`);
      }
    } else {
      console.log(`skip: ${display(sourcePath)} not found`);
    }

    // 2. Generated artefacts must match the IDL.
    const idl = loadIdl(idlPath);
    const { inSync, drift } = generatedFilesInSync(idl, REPO_ROOT);
    if (inSync) {
      console.log('ok: generated SDK artefacts are in sync');
    } else {
      console.error('FAIL: generated SDK artefacts are out of sync:');
      for (const d of drift) console.error(`  ${display(d)}`);
      failed = true;
    }

    process.exit(failed ? 1 : 0);
  });

program.parse(process.argv);

export { program, VERSION, path };
