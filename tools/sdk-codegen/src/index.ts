/**
 * Public entry point for the AuditLedger SDK code generator (#407).
 *
 * ```ts
 * import { extractIdlFromFile, generateTypeScript, checkCompatibility } from '@audit-ledger/sdk-codegen';
 *
 * const { idl } = extractIdlFromFile('src/lib.rs');
 * const files = generateTypeScript(idl);
 * const report = checkCompatibility(oldIdl, idl);
 * ```
 */

export * from './abi/types';
export * from './abi/extract';
export * from './abi/validate';
export * from './compat';
export * from './generators/common';
export * from './generators/typescript';
export * from './generators/python';
export * from './generators/rust';
export * from './pipeline';
