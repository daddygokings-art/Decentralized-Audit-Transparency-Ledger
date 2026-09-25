import { readFileSync, existsSync, statSync } from 'fs';
import { TransactionTrace } from './schema';
import { assertValidTrace, validateTrace } from './validate';

export interface LoadResult {
  trace: TransactionTrace;
  /** Non-fatal problems, e.g. totals that disagree with the step fold. */
  warnings: string[];
}

/**
 * Read a trace from disk.
 *
 * Accepts a single JSON object, a JSON array of traces (the first is returned),
 * or JSON Lines. `jsonl` is how the CLI's `record` subcommand streams traces out
 * of long-running sessions without holding them all in memory.
 */
export function loadTrace(path: string): LoadResult {
  if (!existsSync(path)) throw new Error(`trace not found: ${path}`);
  if (statSync(path).isDirectory()) throw new Error(`expected a trace file, got a directory: ${path}`);
  const raw = readFileSync(path, 'utf8');
  const text = raw.trim();
  if (text.length === 0) throw new Error(`trace file is empty: ${path}`);

  // Sniff the shape by parsing, not by pattern-matching the text: a
  // pretty-printed object, a compact object, an array and JSON Lines all start
  // with `{` or `[`, and guessing from indentation gets it wrong.
  const parsed = parseAny(text, path);

  assertValidTrace(parsed);
  return { trace: parsed, warnings: consistencyWarnings(parsed) };
}

export function parseTrace(text: string): LoadResult {
  const parsed = parseAny(text.trim(), '<inline>');
  assertValidTrace(parsed);
  return { trace: parsed, warnings: consistencyWarnings(parsed) };
}

/**
 * Parse a document that is either one trace, an array of traces, or JSON Lines.
 *
 * Falls back to JSON Lines only when whole-document parsing fails, and reports
 * both failures so a malformed trace names its file.
 */
function parseAny(text: string, origin: string): unknown {
  try {
    const whole = JSON.parse(text) as unknown;
    if (Array.isArray(whole)) {
      if (whole.length === 0) throw new Error(`trace array is empty: ${origin}`);
      return whole[0];
    }
    return whole;
  } catch (whole) {
    const lines = text.split('\n').filter((l) => l.trim().length > 0);
    if (lines.length < 2) {
      throw new Error(`cannot parse trace ${origin}: ${(whole as Error).message}`);
    }
    for (const [i, line] of lines.entries()) {
      try {
        return JSON.parse(line) as unknown;
      } catch (e) {
        throw new Error(`cannot parse trace ${origin}: not a single JSON document, and JSON Lines line ${i + 1} is invalid: ${(e as Error).message}`);
      }
    }
    throw new Error(`cannot parse trace ${origin}`);
  }
}

/** Human-readable summary of every validation issue, for `verify` output. */
export function consistencyWarnings(trace: TransactionTrace): string[] {
  return validateTrace(trace).map((i) => `${i.path}: ${i.message}`);
}

/** Canonical on-disk form, for reproducible diffs of generated traces. */
export function serializeTrace(trace: TransactionTrace): string {
  return `${JSON.stringify(trace, null, 2)}\n`;
}
