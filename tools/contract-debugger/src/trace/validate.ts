import { TRACE_FORMAT_VERSION, TransactionTrace } from './schema';

export interface TraceIssue {
  /** Dotted path to the offending value, e.g. `steps[4].gas.instructions`. */
  path: string;
  message: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function num(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function str(value: unknown): value is string {
  return typeof value === 'string';
}

/**
 * Check a trace for structural and internal consistency.
 *
 * Two classes of problem are reported. *Structural* issues mean the trace
 * cannot be replayed at all. *Consistency* issues mean it can be replayed but
 * the derived views would disagree with the recorded totals — for example a
 * per-step gas sum that does not match `totals.instructions`. The debugger
 * surfaces both rather than silently preferring one number over the other.
 */
export function validateTrace(input: unknown): TraceIssue[] {
  const issues: TraceIssue[] = [];
  const add = (path: string, message: string): void => {
    issues.push({ path, message });
  };

  if (!isRecord(input)) return [{ path: '$', message: 'trace must be an object' }];

  if (input.formatVersion !== TRACE_FORMAT_VERSION) {
    add('formatVersion', `expected ${TRACE_FORMAT_VERSION}, got ${JSON.stringify(input.formatVersion)}`);
  }

  if (!isRecord(input.contract)) {
    add('contract', 'missing contract descriptor');
  } else {
    for (const key of ['id', 'name', 'version'] as const) {
      if (!str(input.contract[key])) add(`contract.${key}`, 'must be a string');
    }
  }

  if (!isRecord(input.transaction)) {
    add('transaction', 'missing transaction descriptor');
  } else if (!str(input.transaction.source)) {
    add('transaction.source', 'must be a string');
  }

  if (input.detail !== 'operation' && input.detail !== 'summary') {
    add('detail', `must be "operation" or "summary", got ${JSON.stringify(input.detail)}`);
  }

  if (!isRecord(input.totals)) {
    add('totals', 'missing totals');
  } else {
    for (const key of ['instructions', 'readBytes', 'writeBytes', 'events', 'storageReads', 'storageWrites', 'fee'] as const) {
      if (!num(input.totals[key])) add(`totals.${key}`, 'must be a finite number');
    }
  }

  const frames = input.frames;
  if (!Array.isArray(frames)) {
    add('frames', 'must be an array');
  } else {
    const seen = new Set<string>();
    frames.forEach((f, i) => {
      if (!isRecord(f) || !str(f.id) || !str(f.function)) {
        add(`frames[${i}]`, 'must have string id and function');
        return;
      }
      if (seen.has(f.id)) add(`frames[${i}].id`, `duplicate frame id ${JSON.stringify(f.id)}`);
      seen.add(f.id);
    });
    frames.forEach((f, i) => {
      if (isRecord(f) && f.parentId !== undefined) {
        if (!str(f.parentId) || !seen.has(f.parentId)) {
          add(`frames[${i}].parentId`, `unknown parent frame ${JSON.stringify(f.parentId)}`);
        }
      }
    });
  }

  const steps = input.steps;
  if (!Array.isArray(steps)) {
    add('steps', 'must be an array');
    return issues;
  }

  const frameIds = new Set(Array.isArray(frames) ? frames.filter(isRecord).map((f) => String(f.id)) : []);
  steps.forEach((s, i) => {
    if (!isRecord(s)) {
      add(`steps[${i}]`, 'must be an object');
      return;
    }
    if (s.index !== i) add(`steps[${i}].index`, `expected dense index ${i}, got ${JSON.stringify(s.index)}`);
    if (!str(s.frameId)) add(`steps[${i}].frameId`, 'must be a string');
    else if (frameIds.size > 0 && !frameIds.has(s.frameId)) add(`steps[${i}].frameId`, `unknown frame ${JSON.stringify(s.frameId)}`);
    if (!str(s.op)) add(`steps[${i}].op`, 'must be a string');
    if (!isRecord(s.gas) || !num(s.gas.instructions)) add(`steps[${i}].gas.instructions`, 'must be a finite number');
    if (s.storage !== undefined) {
      if (!isRecord(s.storage)) add(`steps[${i}].storage`, 'must be an object');
      else {
        if (s.storage.op !== 'read' && s.storage.op !== 'write' && s.storage.op !== 'del') {
          add(`steps[${i}].storage.op`, `must be read|write|del, got ${JSON.stringify(s.storage.op)}`);
        }
        if (s.storage.scope !== 'instance' && s.storage.scope !== 'persistent') {
          add(`steps[${i}].storage.scope`, `must be instance|persistent, got ${JSON.stringify(s.storage.scope)}`);
        }
        if (!str(s.storage.key)) add(`steps[${i}].storage.key`, 'must be a string');
        if (s.storage.op === 'del' && s.storage.value !== undefined) {
          add(`steps[${i}].storage.value`, 'a delete must not carry a value');
        }
      }
    }
    if (s.event !== undefined && (!isRecord(s.event) || !str(s.event.name))) {
      add(`steps[${i}].event.name`, 'must be a string');
    }
    if (s.error !== undefined && (!isRecord(s.error) || !str(s.error.name))) {
      add(`steps[${i}].error.name`, 'must be a string');
    }
  });

  return issues;
}

/** Throwing variant of {@link validateTrace}. */
export function assertValidTrace(trace: unknown): asserts trace is TransactionTrace {
  const issues = validateTrace(trace);
  if (issues.length > 0) {
    const detail = issues.map((i) => `  ${i.path}: ${i.message}`).join('\n');
    throw new Error(`invalid transaction trace:\n${detail}`);
  }
}
