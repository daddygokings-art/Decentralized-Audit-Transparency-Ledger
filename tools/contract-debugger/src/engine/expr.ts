/**
 * Watch-expression evaluator.
 *
 * Backs both `evaluate` in the debug adapter and `audit-ledger-debug watch`,
 * so a watch behaves the same in VS Code and on the command line.
 *
 * Supported grammar: literals (numbers, strings, `true`/`false`/`null` and
 * array literals), identifiers, member access, indexing, calls to a small
 * builtin set, unary `-`/`!`, and binary `+ - * / % == != === < <= > >= &&
 * ||`. `&&`/`||` short-circuit. Anything else is a parse error, reported with
 * the offset so a UI can point at the offending character.
 */

export type Value = unknown;

type Token =
  | { t: 'num'; v: number; i: number }
  | { t: 'str'; v: string; i: number }
  | { t: 'ident'; v: string; i: number }
  | { t: 'punct'; v: string; i: number }
  | { t: 'eof'; v: ''; i: number };

const PUNCT3 = ['==='];
const PUNCT2 = ['==', '!=', '<=', '>=', '&&', '||'];
const PUNCT1 = ['(', ')', '[', ']', '.', ',', '+', '-', '*', '/', '%', '<', '>', '!'];

export class ExpressionError extends Error {
  constructor(
    message: string,
    readonly offset: number,
  ) {
    super(message);
    this.name = 'ExpressionError';
  }
}

function tokenize(src: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === ' ' || c === '\t' || c === '\n' || c === '\r') {
      i += 1;
      continue;
    }
    if (c === '"' || c === "'") {
      const quote = c;
      let j = i + 1;
      let value = '';
      while (j < src.length && src[j] !== quote) {
        if (src[j] === '\\' && j + 1 < src.length) {
          const esc = src[j + 1];
          value += esc === 'n' ? '\n' : esc === 't' ? '\t' : esc;
          j += 2;
        } else {
          value += src[j];
          j += 1;
        }
      }
      if (j >= src.length) throw new ExpressionError('unterminated string', i);
      out.push({ t: 'str', v: value, i });
      i = j + 1;
      continue;
    }
    if (c >= '0' && c <= '9') {
      const match = /^\d+(\.\d+)?/.exec(src.slice(i))!;
      out.push({ t: 'num', v: Number(match[0]), i });
      i += match[0].length;
      continue;
    }
    if (/[A-Za-z_$]/.test(c)) {
      const match = /^[A-Za-z_$][A-Za-z0-9_$]*/.exec(src.slice(i))!;
      out.push({ t: 'ident', v: match[0], i });
      i += match[0].length;
      continue;
    }
    const three = src.slice(i, i + 3);
    const two = src.slice(i, i + 2);
    if (PUNCT3.includes(three)) {
      out.push({ t: 'punct', v: three, i });
      i += 3;
      continue;
    }
    if (PUNCT2.includes(two)) {
      out.push({ t: 'punct', v: two, i });
      i += 2;
      continue;
    }
    if (PUNCT1.includes(c)) {
      out.push({ t: 'punct', v: c, i });
      i += 1;
      continue;
    }
    throw new ExpressionError(`unexpected character ${JSON.stringify(c)}`, i);
  }
  out.push({ t: 'eof', v: '', i: src.length });
  return out;
}

type Node =
  | { k: 'lit'; v: Value }
  | { k: 'arr'; items: Node[] }
  | { k: 'var'; name: string }
  | { k: 'member'; obj: Node; name: string }
  | { k: 'index'; obj: Node; idx: Node }
  | { k: 'call'; callee: Node; args: Node[] }
  | { k: 'unary'; op: string; arg: Node }
  | { k: 'bin'; op: string; l: Node; r: Node };

/** Names the evaluator resolves against the caller-supplied scope. */
const SCOPE_MEMBERS = ['locals', 'storage', 'stack', 'gas', 'frame', 'args', 'step', 'events', 'contract', 'transaction'];

/** Member functions available on any value, so `x.len()` works without a builtin. */
const BUILTIN_METHODS: Record<string, (...args: Value[]) => Value> = {
  len: (v) => lengthOf(v),
  size: (v) => lengthOf(v),
  count: (v) => lengthOf(v),
  isEmpty: (v) => lengthOf(v) === 0,
  keys: (v) => (isPlainObject(v) ? Object.keys(v) : []),
  values: (v) => (isPlainObject(v) ? Object.values(v) : []),
  entries: (v) => (isPlainObject(v) ? Object.entries(v) : []),
  type: (v) => typeName(v),
  contains: (v, needle) => {
    if (typeof v === 'string') return v.includes(String(needle));
    if (Array.isArray(v)) return v.some((x) => deepEq(x, needle));
    return false;
  },
  has: (v, needle) => (isPlainObject(v) ? Object.prototype.hasOwnProperty.call(v, String(needle)) : false),
};

class Parser {
  private pos = 0;
  constructor(private readonly toks: Token[]) {}

  parse(): Node {
    const node = this.expr();
    const t = this.peek();
    if (t.t !== 'eof') throw new ExpressionError(`unexpected ${JSON.stringify(t.v)}`, t.i);
    return node;
  }

  private peek(): Token {
    return this.toks[this.pos];
  }

  private eat(v: string): boolean {
    const t = this.peek();
    if (t.t === 'punct' && t.v === v) {
      this.pos += 1;
      return true;
    }
    return false;
  }

  private expect(v: string): Token {
    const t = this.peek();
    if (t.t !== 'punct' || t.v !== v) throw new ExpressionError(`expected ${JSON.stringify(v)}`, t.i);
    this.pos += 1;
    return t;
  }

  private expr(): Node {
    let left = this.and();
    for (;;) {
      const t = this.peek();
      if (t.t !== 'punct') return left;
      if (t.v === '||') {
        this.pos += 1;
        left = { k: 'bin', op: '||', l: left, r: this.and() };
      } else if (t.v === '&&') {
        this.pos += 1;
        left = { k: 'bin', op: '&&', l: left, r: this.and() };
      } else {
        return left;
      }
    }
  }

  private and(): Node {
    let left = this.cmp();
    for (;;) {
      const t = this.peek();
      if (t.t !== 'punct' || !['==', '!=', '===', '<', '<=', '>', '>='].includes(t.v)) return left;
      this.pos += 1;
      left = { k: 'bin', op: t.v, l: left, r: this.cmp() };
    }
  }

  private cmp(): Node {
    let left = this.add();
    for (;;) {
      const t = this.peek();
      if (t.t !== 'punct' || !['+', '-'].includes(t.v)) return left;
      this.pos += 1;
      left = { k: 'bin', op: t.v, l: left, r: this.add() };
    }
  }

  private add(): Node {
    let left = this.mul();
    for (;;) {
      const t = this.peek();
      if (t.t !== 'punct' || !['*', '/', '%'].includes(t.v)) return left;
      this.pos += 1;
      left = { k: 'bin', op: t.v, l: left, r: this.mul() };
    }
  }

  private mul(): Node {
    let left = this.unary();
    for (;;) {
      const t = this.peek();
      if (t.t !== 'punct' || !['*', '/', '%'].includes(t.v)) return left;
      this.pos += 1;
      left = { k: 'bin', op: t.v, l: left, r: this.unary() };
    }
  }

  private unary(): Node {
    const t = this.peek();
    if (t.t === 'punct' && (t.v === '-' || t.v === '!')) {
      this.pos += 1;
      return { k: 'unary', op: t.v, arg: this.unary() };
    }
    return this.postfix();
  }

  private postfix(): Node {
    let node = this.primary();
    for (;;) {
      if (this.eat('.')) {
        const t = this.peek();
        if (t.t !== 'ident') throw new ExpressionError('expected property name after "."', t.i);
        this.pos += 1;
        if (this.eat('(')) {
          const args: Node[] = [];
          if (!this.eat(')')) {
            do {
              args.push(this.expr());
            } while (this.eat(','));
            this.expect(')');
          }
          node = { k: 'call', callee: { k: 'member', obj: node, name: t.v }, args };
        } else {
          node = { k: 'member', obj: node, name: t.v };
        }
        continue;
      }
      if (this.eat('[')) {
        const idx = this.expr();
        this.expect(']');
        node = { k: 'index', obj: node, idx };
        continue;
      }
      if (this.eat('(')) {
        const args: Node[] = [];
        if (!this.eat(')')) {
          do {
            args.push(this.expr());
          } while (this.eat(','));
          this.expect(')');
        }
        node = { k: 'call', callee: node, args };
        continue;
      }
      return node;
    }
  }

  private primary(): Node {
    const t = this.peek();
    if (t.t === 'num' || t.t === 'str') {
      this.pos += 1;
      return { k: 'lit', v: t.v };
    }
    if (t.t === 'ident') {
      this.pos += 1;
      // `true`/`false`/`null` are literals, not scope lookups, so
      // `false && x` yields false rather than undefined.
      if (t.v === 'true') return { k: 'lit', v: true };
      if (t.v === 'false') return { k: 'lit', v: false };
      if (t.v === 'null') return { k: 'lit', v: null };
      return { k: 'var', name: t.v };
    }
    if (t.t === 'punct' && t.v === '(') {
      this.pos += 1;
      const node = this.expr();
      this.expect(')');
      return node;
    }
    if (t.t === 'punct' && t.v === '[') {
      this.pos += 1;
      const items: Node[] = [];
      if (!this.eat(']')) {
        do {
          items.push(this.expr());
        } while (this.eat(','));
        this.expect(']');
      }
      return { k: 'arr', items };
    }
    throw new ExpressionError(`unexpected ${JSON.stringify(t.v || 'end of expression')}`, t.i);
  }
}

function isPlainObject(v: Value): v is Record<string, Value> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function lengthOf(v: Value): number {
  if (typeof v === 'string' || Array.isArray(v)) return v.length;
  if (isPlainObject(v)) return Object.keys(v).length;
  if (v === null || v === undefined) return 0;
  return NaN;
}

export function typeName(v: Value): string {
  if (v === null) return 'null';
  if (v === undefined) return 'undefined';
  if (Array.isArray(v)) return 'array';
  if (typeof v === 'object') return 'object';
  return typeof v;
}

/** Short, single-line rendering for debugger watch output. */
export function stringify(v: Value): string {
  if (typeof v === 'string') return JSON.stringify(v);
  if (v === undefined) return 'undefined';
  if (v === null) return 'null';
  if (typeof v === 'bigint') return `${v.toString()}n`;
  if (typeof v === 'object') {
    try {
      return JSON.stringify(v) ?? 'undefined';
    } catch {
      return String(v);
    }
  }
  return String(v);
}

function deepEq(a: Value, b: Value): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return false;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => deepEq(v, b[i]));
  if (isPlainObject(a) && isPlainObject(b)) {
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    return ka.length === kb.length && ka.every((k) => deepEq(a[k], b[k]));
  }
  return false;
}

/** Render for concatenation: strings raw, everything else JSON. */
function rawFor(v: Value): string {
  return typeof v === 'string' ? v : stringify(v);
}

function truthy(v: Value): boolean {
  if (Array.isArray(v)) return v.length > 0;
  if (typeof v === 'string') return v.length > 0;
  if (isPlainObject(v)) return Object.keys(v).length > 0;
  return Boolean(v);
}

function toNumber(v: Value): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'bigint') return Number(v);
  if (typeof v === 'boolean') return v ? 1 : 0;
  if (typeof v === 'string' && v.trim() !== '' && Number.isFinite(Number(v))) return Number(v);
  return NaN;
}

const MAX_DEPTH = 32;

/**
 * Evaluate a watch expression.
 *
 * Resolution order for a bare identifier: locals, then the injected scope
 * members, then the scope itself. Locals win so a watch on `event_type` is not
 * shadowed by a debugger-provided name.
 */
export function evaluate(expression: string, scope: Record<string, Value>): Value {
  const ast = new Parser(tokenize(expression)).parse();
  return evalNode(ast, scope, 0);
}

function evalNode(node: Node, scope: Record<string, Value>, depth: number): Value {
  if (depth > MAX_DEPTH) throw new ExpressionError('expression nested too deeply', 0);
  switch (node.k) {
    case 'lit':
      return node.v;
    case 'arr':
      return node.items.map((item) => evalNode(item, scope, depth + 1));
    case 'var': {
      if (Object.prototype.hasOwnProperty.call(scope, node.name)) return scope[node.name];
      if (SCOPE_MEMBERS.includes(node.name) && Object.prototype.hasOwnProperty.call(scope, node.name)) return scope[node.name];
      const locals = scope.locals;
      if (isPlainObject(locals) && Object.prototype.hasOwnProperty.call(locals, node.name)) return locals[node.name];
      const args = scope.args;
      if (isPlainObject(args) && Object.prototype.hasOwnProperty.call(args, node.name)) return args[node.name];
      return undefined;
    }
    case 'member': {
      const obj = evalNode(node.obj, scope, depth + 1);
      if (obj === null || obj === undefined) return undefined;
      if (isPlainObject(obj)) return Object.prototype.hasOwnProperty.call(obj, node.name) ? obj[node.name] : undefined;
      if (Array.isArray(obj)) {
        if (node.name === 'length') return obj.length;
        return undefined;
      }
      if (typeof obj === 'string') {
        if (node.name === 'length') return obj.length;
        return undefined;
      }
      return undefined;
    }
    case 'index': {
      const obj = evalNode(node.obj, scope, depth + 1);
      const idx = evalNode(node.idx, scope, depth + 1);
      if (obj === null || obj === undefined) return undefined;
      if (Array.isArray(obj) || typeof obj === 'string') {
        const n = toNumber(idx);
        return Number.isInteger(n) ? (obj as Value[])[n] : undefined;
      }
      if (isPlainObject(obj)) return Object.prototype.hasOwnProperty.call(obj, String(idx)) ? obj[String(idx)] : undefined;
      return undefined;
    }
    case 'call': {
      if (node.callee.k === 'member') {
        const recv = evalNode(node.callee.obj, scope, depth + 1);
        const fn = BUILTIN_METHODS[node.callee.name];
        if (!fn) {
          throw new ExpressionError(`unknown method ${JSON.stringify(node.callee.name)}`, 0);
        }
        return fn(recv, ...node.args.map((a) => evalNode(a, scope, depth + 1)));
      }
      throw new ExpressionError('only method calls are supported', 0);
    }
    case 'unary': {
      const v = evalNode(node.arg, scope, depth + 1);
      if (node.op === '-') return -toNumber(v);
      return !truthy(v);
    }
    case 'bin': {
      if (node.op === '&&') {
        const l = evalNode(node.l, scope, depth + 1);
        return truthy(l) ? evalNode(node.r, scope, depth + 1) : l;
      }
      if (node.op === '||') {
        const l = evalNode(node.l, scope, depth + 1);
        return truthy(l) ? l : evalNode(node.r, scope, depth + 1);
      }
      const l = evalNode(node.l, scope, depth + 1);
      const r = evalNode(node.r, scope, depth + 1);
      switch (node.op) {
        case '+':
          // Concatenation when either side is a string. A string operand is
          // used raw rather than re-quoted, so `"a" + 1` is `a1` and
          // `name + "!"` is `audit!`; a non-string operand is rendered.
          if (typeof l === 'string' || typeof r === 'string') return rawFor(l) + rawFor(r);
          return toNumber(l) + toNumber(r);
        case '-':
          return toNumber(l) - toNumber(r);
        case '*':
          return toNumber(l) * toNumber(r);
        case '/':
          return toNumber(l) / toNumber(r);
        case '%':
          return toNumber(l) % toNumber(r);
        case '==':
          return deepEq(l, r);
        case '===':
          return l === r;
        case '!=':
          return !deepEq(l, r);
        case '<':
          return toNumber(l) < toNumber(r);
        case '<=':
          return toNumber(l) <= toNumber(r);
        case '>':
          return toNumber(l) > toNumber(r);
        case '>=':
          return toNumber(l) >= toNumber(r);
        default:
          throw new ExpressionError(`unknown operator ${JSON.stringify(node.op)}`, 0);
      }
    }
    default: {
      const exhaustive: never = node;
      throw new ExpressionError(`unsupported node ${JSON.stringify(exhaustive)}`, 0);
    }
  }
}

/** Names advertised to clients so editor completion matches what resolves. */
export function scopeMembers(): string[] {
  return [...SCOPE_MEMBERS, 'Object.keys', 'Object.values', 'Object.entries'];
}
