import { describe, expect, it } from 'vitest';
import { ExpressionError, evaluate, scopeMembers, stringify, typeName } from '../src/index';

const scope = {
  locals: { n: 10, name: 'audit', flag: false, list: [1, 2, 3], obj: { deep: { leaf: 42 } } },
  storage: { RuntimeState: { total: 5 }, 'EventOrder(1)': 'abc' },
  storageByScope: { instance: { Paused: false }, persistent: {} },
  stack: ['log_events', 'log_event'],
  gas: { instructions: 100, remaining: 0 },
  contract: { name: 'AuditLedger' },
  step: 7,
  args: { submitter: 'GABC' },
};

const run = (expr: string): unknown => evaluate(expr, scope);

describe('expression evaluation', () => {
  it('evaluates literals', () => {
    expect(run('1')).toBe(1);
    expect(run('1.5')).toBe(1.5);
    expect(run('"hi"')).toBe('hi');
    expect(run("'hi'")).toBe('hi');
    expect(run('true')).toBe(true);
    expect(run('false')).toBe(false);
    expect(run('null')).toBeNull();
  });

  it('prefers a local over a scope member of the same name', () => {
    expect(run('name')).toBe('audit');
  });

  it('falls back to arguments then undefined', () => {
    expect(run('submitter')).toBe('GABC');
    expect(run('missing')).toBeUndefined();
  });

  it('does member access', () => {
    expect(run('obj.deep.leaf')).toBe(42);
    expect(run('storage.RuntimeState.total')).toBe(5);
    expect(run('contract.name')).toBe('AuditLedger');
  });

  it('does indexed access on objects and arrays', () => {
    expect(run('list[0]')).toBe(1);
    expect(run('list[2]')).toBe(3);
    expect(run('storage["EventOrder(1)"]')).toBe('abc');
    expect(run('locals.obj["deep"].leaf')).toBe(42);
  });

  it('reads array length both ways', () => {
    expect(run('list.length')).toBe(3);
    expect(run('list.len()')).toBe(3);
    expect(run('"abc".length')).toBe(3);
  });

  it('does arithmetic with correct precedence', () => {
    expect(run('1 + 2 * 3')).toBe(7);
    expect(run('(1 + 2) * 3')).toBe(9);
    expect(run('7 % 4')).toBe(3);
    expect(run('7 / 2')).toBe(3.5);
    expect(run('-n')).toBe(-10);
  });

  it('concatenates with + when either side is a string', () => {
    expect(run('"a" + 1')).toBe('a1');
    expect(run('name + "!"')).toBe('audit!');
  });

  it('compares with < <= > >=', () => {
    expect(run('1 < 2')).toBe(true);
    expect(run('2 <= 2')).toBe(true);
    expect(run('3 > 4')).toBe(false);
    expect(run('3 >= 3')).toBe(true);
  });

  it('compares by value for == and by identity for ===', () => {
    expect(run('list == [1,2,3]')).toBe(true);
    expect(run('locals.obj == locals.obj')).toBe(true);
    expect(run('storage == storage')).toBe(true);
  });

  it('short-circuits && and ||', () => {
    expect(run('false && missing.x')).toBe(false);
    expect(run('true || missing.x')).toBe(true);
    expect(run('true && name')).toBe('audit');
    expect(run('false || name')).toBe('audit');
  });

  it('supports ! on truthy and falsy values', () => {
    expect(run('!flag')).toBe(true);
    expect(run('!n')).toBe(false);
    expect(run('!0')).toBe(true);
  });

  it('exposes builtin methods', () => {
    expect(run('list.contains(2)')).toBe(true);
    expect(run('name.contains("udi")')).toBe(true);
    expect(run('locals.obj.keys().len()')).toBe(1);
    expect(run('locals.obj.has("deep")')).toBe(true);
    expect(run('n.type()')).toBe('number');
    expect(run('list.isEmpty()')).toBe(false);
    expect(run('storage.values().len()')).toBe(2);
  });

  it('is undefined rather than throwing on a missing member', () => {
    expect(run('missing.deeper.deepest')).toBeUndefined();
    expect(run('locals.obj.missing')).toBeUndefined();
    expect(run('list[99]')).toBeUndefined();
    expect(run('list["x"]')).toBeUndefined();
  });

  it('reports syntax errors with an offset', () => {
    expect(() => run('1 +')).toThrowError(ExpressionError);
    expect(() => run('(1')).toThrowError(/expected/);
    expect(() => run('1 2')).toThrowError(/unexpected/);
    expect(() => run('"unterminated')).toThrowError(/unterminated/);
    expect(() => run('a # b')).toThrowError(/unexpected character/);
    expect(() => run('.foo')).toThrowError(/expected/);
  });

  it('reports an offset inside the expression', () => {
    try {
      run('1 + "unterminated');
      expect.unreachable('expected a parse error');
    } catch (e) {
      expect(e).toBeInstanceOf(ExpressionError);
      expect((e as ExpressionError).offset).toBe(4);
    }
  });

  it('rejects an unknown method', () => {
    expect(() => run('list.nope()')).toThrowError(/unknown method/);
  });

  it('rejects a bare call, since there are no global functions', () => {
    expect(() => run('foo()')).toThrowError(/only method calls/);
  });

  it('guards against pathological member-chain nesting', () => {
    // A chain long enough to blow the evaluator's recursion budget is refused
    // rather than crashing the adapter.
    const chain = `locals${'.x'.repeat(200)}`;
    expect(() => run(chain)).toThrowError(/nested too deeply/);
  });

  it('allows a legitimately deep chain', () => {
    expect(run('obj.deep.leaf')).toBe(42);
  });
});

describe('value rendering', () => {
  it('stringifies each JSON shape', () => {
    expect(stringify(1)).toBe('1');
    expect(stringify('a')).toBe('"a"');
    expect(stringify(null)).toBe('null');
    expect(stringify(undefined)).toBe('undefined');
    expect(stringify([1, 'a'])).toBe('[1,"a"]');
    expect(stringify({ a: [1] })).toBe('{"a":[1]}');
    expect(stringify(10n)).toBe('10n');
  });

  it('names types', () => {
    expect(typeName(null)).toBe('null');
    expect(typeName(undefined)).toBe('undefined');
    expect(typeName([1])).toBe('array');
    expect(typeName({})).toBe('object');
    expect(typeName(1)).toBe('number');
    expect(typeName('a')).toBe('string');
  });

  it('advertises the scope names a client should offer', () => {
    expect(scopeMembers()).toEqual(
      expect.arrayContaining(['locals', 'storage', 'stack', 'gas', 'frame', 'args', 'step', 'events', 'contract', 'transaction']),
    );
  });
});
