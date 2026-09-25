import { ContractIdl, IdlFunction, IdlType, typeSignature } from './abi/types';

export type ChangeSeverity = 'breaking' | 'minor' | 'patch' | 'none';

export interface CompatibilityFinding {
  severity: ChangeSeverity;
  path: string;
  message: string;
  /** Suggested IDL version after applying this finding. */
  suggestion?: string;
}

export interface CompatibilityReport {
  from: string;
  to: string;
  /** Highest severity across all findings. */
  severity: ChangeSeverity;
  /** Version the new IDL should declare, given these findings. */
  expectedSpecVersion: string;
  findings: CompatibilityFinding[];
  compatible: boolean;
}

function parseSemver(v: string): [number, number, number] {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v);
  if (!m) throw new Error(`Not a semantic version: ${v}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function bump(v: string, severity: ChangeSeverity): string {
  if (severity === 'none') return v;
  const [major, minor, patch] = parseSemver(v);
  if (severity === 'breaking') return `${major + 1}.0.0`;
  if (severity === 'minor') return `${major}.${minor + 1}.0`;
  return `${major}.${minor}.${patch + 1}`;
}

function maxSeverity(a: ChangeSeverity, b: ChangeSeverity): ChangeSeverity {
  const order: ChangeSeverity[] = ['none', 'patch', 'minor', 'breaking'];
  return order.indexOf(a) >= order.indexOf(b) ? a : b;
}

/** Render an input list for diagnostics. */
function describeInputs(fn: IdlFunction): string {
  return fn.inputs.map((i) => `${i.name}: ${typeSignature(i.type)}`).join(', ');
}

function compareFunction(prev: IdlFunction, next: IdlFunction, out: CompatibilityFinding[]): void {
  const prevByName = new Map(prev.inputs.map((i) => [i.name, i]));
  const nextByName = new Map(next.inputs.map((i) => [i.name, i]));

  for (const [name, p] of prevByName) {
    const n = nextByName.get(name);
    if (!n) {
      out.push({
        severity: 'breaking',
        path: `functions.${prev.name}.inputs.${name}`,
        message: `input \`${name}\` was removed from \`${prev.name}()\` (was ${typeSignature(p.type)})`,
      });
      continue;
    }
    const ps = typeSignature(p.type);
    const ns = typeSignature(n.type);
    if (ps !== ns) {
      out.push({
        severity: 'breaking',
        path: `functions.${prev.name}.inputs.${name}`,
        message: `input \`${name}\` of \`${prev.name}()\` changed type: ${ps} -> ${ns}`,
      });
    }
  }

  for (const [name, n] of nextByName) {
    if (!prevByName.has(name)) {
      out.push({
        severity: 'breaking',
        path: `functions.${next.name}.inputs.${name}`,
        message: `input \`${name}\` was added to \`${next.name}()\` (${typeSignature(n.type)}); existing callers must pass it`,
      });
    }
  }

  // Positional order matters for the RPC layer, so compare it explicitly.
  const prevOrder = prev.inputs.map((i) => i.name).join(',');
  const nextOrder = next.inputs.map((i) => i.name).join(',');
  if (prevOrder === nextOrder) return;

  const prevSet = new Set(prevOrder.split(','));
  const reordered = nextOrder.split(',').filter((n) => prevSet.has(n));
  if (reordered.join(',') !== prevOrder) {
    out.push({
      severity: 'breaking',
      path: `functions.${prev.name}.inputs`,
      message: `parameter order of \`${prev.name}()\` changed: (${prevOrder}) -> (${nextOrder})`,
    });
  }
}

function compareTypeSignature(a: IdlType, b: IdlType, path: string, out: CompatibilityFinding[]): void {
  const as = typeSignature(a);
  const bs = typeSignature(b);
  if (as !== bs) {
    out.push({ severity: 'breaking', path, message: `type changed: ${as} -> ${bs}` });
  }
}

function compareContract(oldIdl: ContractIdl, newIdl: ContractIdl): CompatibilityFinding[] {
  const out: CompatibilityFinding[] = [];

  if (oldIdl.contract.name !== newIdl.contract.name) {
    out.push({
      severity: 'breaking',
      path: 'contract.name',
      message: `contract name changed: ${oldIdl.contract.name} -> ${newIdl.contract.name}`,
    });
  }

  // ── Functions ────────────────────────────────────────────────────────────
  const oldFns = new Map(oldIdl.functions.map((f) => [f.name, f]));
  const newFns = new Map(newIdl.functions.map((f) => [f.name, f]));

  for (const [name, f] of oldFns) {
    const n = newFns.get(name);
    if (!n) {
      out.push({ severity: 'breaking', path: `functions.${name}`, message: `function \`${name}()\` was removed` });
      continue;
    }
    if (f.mutating !== n.mutating) {
      out.push({
        severity: 'breaking',
        path: `functions.${name}.mutating`,
        message: `\`${name}()\` changed from ${f.mutating ? 'mutating' : 'read-only'} to ${n.mutating ? 'mutating' : 'read-only'}`,
      });
    }
    compareFunction(f, n, out);
    compareTypeSignature(f.output, n.output, `functions.${name}.output`, out);
    void describeInputs;
  }
  for (const name of newFns.keys()) {
    if (!oldFns.has(name)) {
      out.push({ severity: 'minor', path: `functions.${name}`, message: `function \`${name}()\` was added` });
    }
  }

  // ── Types ────────────────────────────────────────────────────────────────
  const oldTypes = new Map(oldIdl.types.map((t) => [t.name, t]));
  const newTypes = new Map(newIdl.types.map((t) => [t.name, t]));

  for (const [name, t] of oldTypes) {
    const n = newTypes.get(name);
    if (!n) {
      out.push({ severity: 'breaking', path: `types.${name}`, message: `type \`${name}\` was removed` });
      continue;
    }
    if (t.kind !== n.kind) {
      out.push({
        severity: 'breaking',
        path: `types.${name}`,
        message: `type \`${name}\` changed kind: ${t.kind} -> ${n.kind}`,
      });
      continue;
    }
    if (t.kind === 'struct' && n.kind === 'struct') {
      const oldFields = new Map(t.fields.map((f) => [f.name, f]));
      const newFields = new Map(n.fields.map((f) => [f.name, f]));
      for (const [fname, f] of oldFields) {
        const nf = newFields.get(fname);
        if (!nf) {
          out.push({
            severity: 'breaking',
            path: `types.${name}.${fname}`,
            message: `field \`${name}.${fname}\` was removed`,
          });
          continue;
        }
        compareTypeSignature(f.type, nf.type, `types.${name}.${fname}`, out);
      }
      for (const fname of newFields.keys()) {
        if (!oldFields.has(fname)) {
          out.push({
            severity: 'breaking',
            path: `types.${name}.${fname}`,
            message: `field \`${name}.${fname}\` was added; generated struct literals must set it`,
          });
        }
      }
    }
    if (t.kind === 'enum' && n.kind === 'enum') {
      const oldVariants = new Map(t.variants.map((v) => [v.name, v.value]));
      const newVariants = new Map(n.variants.map((v) => [v.name, v.value]));
      for (const [vname, ordinal] of oldVariants) {
        if (!newVariants.has(vname)) {
          out.push({
            severity: 'breaking',
            path: `types.${name}.${vname}`,
            message: `variant \`${name}::${vname}\` was removed`,
          });
          continue;
        }
        if (newVariants.get(vname) !== ordinal) {
          out.push({
            severity: 'breaking',
            path: `types.${name}.${vname}`,
            message: `variant \`${name}::${vname}\` was renumbered: ${ordinal} -> ${newVariants.get(vname)}`,
          });
        }
      }
      for (const vname of newVariants.keys()) {
        if (!oldVariants.has(vname)) {
          out.push({
            severity: 'minor',
            path: `types.${name}.${vname}`,
            message: `variant \`${name}::${vname}\` was added`,
          });
        }
      }
    }
  }
  for (const name of newTypes.keys()) {
    if (!oldTypes.has(name)) {
      out.push({ severity: 'minor', path: `types.${name}`, message: `type \`${name}\` was added` });
    }
  }

  // ── Errors ───────────────────────────────────────────────────────────────
  const oldErrors = new Map(oldIdl.errors.map((e) => [e.name, e.code]));
  const newErrors = new Map(newIdl.errors.map((e) => [e.name, e.code]));
  for (const [name, code] of oldErrors) {
    if (!newErrors.has(name)) {
      out.push({ severity: 'breaking', path: `errors.${name}`, message: `error \`${name}\` was removed` });
    } else if (newErrors.get(name) !== code) {
      out.push({
        severity: 'breaking',
        path: `errors.${name}`,
        message: `error \`${name}\` was renumbered: ${code} -> ${newErrors.get(name)}`,
      });
    }
  }
  for (const name of newErrors.keys()) {
    if (!oldErrors.has(name)) {
      out.push({ severity: 'minor', path: `errors.${name}`, message: `error \`${name}\` was added` });
    }
  }

  // ── Events ───────────────────────────────────────────────────────────────
  const oldEvents = new Map(oldIdl.events.map((e) => [e.name, e]));
  const newEvents = new Map(newIdl.events.map((e) => [e.name, e]));
  for (const [name, e] of oldEvents) {
    const n = newEvents.get(name);
    if (!n) {
      out.push({ severity: 'breaking', path: `events.${name}`, message: `event \`${name}\` was removed` });
      continue;
    }
    if (e.topics.length !== n.topics.length) {
      out.push({
        severity: 'breaking',
        path: `events.${name}.topics`,
        message: `event \`${name}\` topic count changed: ${e.topics.length} -> ${n.topics.length}`,
      });
    } else {
      e.topics.forEach((t, i) => compareTypeSignature(t, n.topics[i], `events.${name}.topics[${i}]`, out));
    }
    if (e.data.length !== n.data.length) {
      out.push({
        severity: 'breaking',
        path: `events.${name}.data`,
        message: `event \`${name}\` data count changed: ${e.data.length} -> ${n.data.length}`,
      });
    } else {
      e.data.forEach((t, i) => compareTypeSignature(t, n.data[i], `events.${name}.data[${i}]`, out));
    }
  }
  for (const name of newEvents.keys()) {
    if (!oldEvents.has(name)) {
      out.push({ severity: 'minor', path: `events.${name}`, message: `event \`${name}\` was added` });
    }
  }

  return out;
}

/**
 * Compare two IDLs and derive the version the new one must declare.
 *
 * Semantics follow the SDK versioning policy in `docs/sdk-codegen.md`:
 * breaking -> major, additive -> minor, documentation-only -> patch.
 */
export function checkCompatibility(oldIdl: ContractIdl, newIdl: ContractIdl): CompatibilityReport {
  const findings = compareContract(oldIdl, newIdl);
  const severity = findings.reduce<ChangeSeverity>((acc, f) => maxSeverity(acc, f.severity), 'none');
  const expectedSpecVersion = bump(oldIdl.spec_version, severity);
  return {
    from: oldIdl.spec_version,
    to: newIdl.spec_version,
    severity,
    expectedSpecVersion,
    findings,
    compatible: severity !== 'breaking',
  };
}

/** True when the declared `spec_version` is consistent with the observed change set. */
export function isVersionConsistent(report: CompatibilityReport): boolean {
  if (report.findings.length === 0) return true;
  return report.to === report.expectedSpecVersion;
}
