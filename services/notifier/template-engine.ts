/**
 * Configurable Notification Template Engine (#362, #361, #360)
 *
 * Handlebars/Mustache-style template engine for event notifications.
 * Supports {{variable}} placeholders, helpers, conditionals, and iteration.
 *
 * Syntax:
 *   {{variable}}              - Simple variable substitution
 *   {{variable | helper}}     - Pipe syntax (alias for {{helper variable}})
 *   {{#if variable}}..{{/if}} - Conditional rendering
 *   {{#unless variable}}..{{/unless}} - Inverse conditional
 *   {{#each variable}}..{{/each}} - Iteration over arrays/objects
 *   {{helper arg1 arg2}}      - Helper function invocation
 *   {{path.to.variable}}      - Dot-notation for nested access
 */

type HelperFn = (...args: string[]) => string;

interface Token {
  type: "text" | "variable" | "helper" | "if" | "unless" | "each" | "end" | "else";
  value: string;
  args?: string[];
  body?: Token[];
  elseBody?: Token[];
}

export interface TemplateVariableDefinition {
  name: string;
  description: string;
  example: string;
}

export interface TemplateEngineConfig {
  strictMode?: boolean;
  missingPlaceholder?: string;
  maxRecursion?: number;
}

interface TemplateContext {
  event: {
    index: number;
    timestamp: number;
    event_type: string;
    submitter: string;
    metadata: string;
  };
  chain: {
    id: string;
    network: string;
    name: string;
  };
  links: {
    explorer_url: string;
    transaction_url: string;
    event_url: string;
  };
  formatted: {
    date: string;
    time: string;
    datetime: string;
  };
  metadata_json: Record<string, unknown> | null;
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
  chain_id: string;
  chain_network: string;
  chain_name: string;
  explorer_url: string;
  transaction_url: string;
  event_url: string;
  date: string;
  time: string;
  datetime: string;
  [key: string]: unknown;
}

function resolvePath(obj: unknown, path: string): unknown {
  const parts = path.split(".");
  let current = obj;
  for (const part of parts) {
    if (current === null || current === undefined) return undefined;
    if (typeof current === "object" && part in (current as Record<string, unknown>)) {
      current = (current as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return current;
}

function coerceString(val: unknown): string {
  if (val === null || val === undefined) return "";
  if (typeof val === "string") return val;
  if (typeof val === "number" || typeof val === "boolean") return String(val);
  try {
    return JSON.stringify(val);
  } catch {
    return String(val);
  }
}

function truthy(val: unknown): boolean {
  if (val === null || val === undefined) return false;
  if (typeof val === "boolean") return val;
  if (typeof val === "number") return val !== 0;
  if (typeof val === "string") return val.length > 0;
  if (Array.isArray(val)) return val.length > 0;
  if (typeof val === "object") return Object.keys(val).length > 0;
  return true;
}

export class TemplateEngine {
  private helpers: Map<string, HelperFn> = new Map();
  private config: Required<TemplateEngineConfig>;

  static readonly BUILT_IN_HELPERS: Record<string, HelperFn> = {
    uppercase: (s: string) => s.toUpperCase(),
    lowercase: (s: string) => s.toLowerCase(),
    capitalize: (s: string) => s.charAt(0).toUpperCase() + s.slice(1),
    json: (s: string) => {
      try {
        return JSON.stringify(JSON.parse(s), null, 2);
      } catch {
        return s;
      }
    },
    truncate: (s: string, n?: string) => {
      const len = parseInt(n ?? "50", 10);
      if (s.length <= len) return s;
      return s.slice(0, len) + "...";
    },
    date: (s?: string) => {
      const ts = s ? parseInt(s, 10) : Date.now() / 1000;
      return new Date(ts * 1000).toLocaleDateString("en-US", {
        year: "numeric", month: "short", day: "numeric",
      });
    },
    time: (s?: string) => {
      const ts = s ? parseInt(s, 10) : Date.now() / 1000;
      return new Date(ts * 1000).toLocaleTimeString("en-US", {
        hour: "2-digit", minute: "2-digit",
      });
    },
    datetime: (s?: string) => {
      const ts = s ? parseInt(s, 10) : Date.now() / 1000;
      return new Date(ts * 1000).toLocaleString("en-US", {
        year: "numeric", month: "short", day: "numeric",
        hour: "2-digit", minute: "2-digit", second: "2-digit",
      });
    },
    iso_timestamp: (s?: string) => {
      const ts = s ? parseInt(s, 10) : Date.now() / 1000;
      return new Date(ts * 1000).toISOString();
    },
    default: (s: string, fallback?: string) => s || fallback || "",
  };

  static readonly AVAILABLE_VARIABLES: TemplateVariableDefinition[] = [
    { name: "index", description: "Global sequential index of the event", example: "{{index}}" },
    { name: "timestamp", description: "Unix timestamp of the event", example: "{{timestamp}}" },
    { name: "event_type", description: "Event type string (e.g. payment, compliance_alert)", example: "{{event_type}}" },
    { name: "submitter", description: "Address of the submitter", example: "{{submitter}}" },
    { name: "metadata", description: "Raw metadata string", example: "{{metadata}}" },
    { name: "chain.id", description: "Chain identifier", example: "{{chain.id}}" },
    { name: "chain.network", description: "Network name (testnet / mainnet)", example: "{{chain.network}}" },
    { name: "chain.name", description: "Human-readable chain name", example: "{{chain.name}}" },
    { name: "links.explorer_url", description: "Block explorer URL for the chain", example: "{{links.explorer_url}}" },
    { name: "links.transaction_url", description: "Transaction URL for the event", example: "{{links.transaction_url}}" },
    { name: "links.event_url", description: "Direct URL to the event details", example: "{{links.event_url}}" },
    { name: "formatted.date", description: "Formatted date (e.g. Jan 15, 2025)", example: "{{formatted.date}}" },
    { name: "formatted.time", description: "Formatted time (e.g. 02:30 PM)", example: "{{formatted.time}}" },
    { name: "formatted.datetime", description: "Formatted date and time", example: "{{formatted.datetime}}" },
    { name: "metadata_json", description: "Parsed JSON object from metadata field", example: "{{#each metadata_json}}{{@key}}: {{this}}{{/each}}" },
  ];

  constructor(config?: TemplateEngineConfig) {
    this.config = {
      strictMode: config?.strictMode ?? false,
      missingPlaceholder: config?.missingPlaceholder ?? "",
      maxRecursion: config?.maxRecursion ?? 10,
    };
    for (const [name, fn] of Object.entries(TemplateEngine.BUILT_IN_HELPERS)) {
      this.helpers.set(name, fn);
    }
  }

  registerHelper(name: string, fn: HelperFn): void {
    this.helpers.set(name, fn);
  }

  getHelpers(): Map<string, HelperFn> {
    return new Map(this.helpers);
  }

  getAvailableVariables(): TemplateVariableDefinition[] {
    return TemplateEngine.AVAILABLE_VARIABLES;
  }

  validate(template: string): { valid: boolean; errors: string[] } {
    const errors: string[] = [];
    let depth = 0;
    const stack: string[] = [];

    const tagRegex = /\{\{[#/]?[\s\S]*?\}\}/g;
    let match: RegExpExecArray | null;
    const re = new RegExp(tagRegex.source, "g");

    while ((match = re.exec(template)) !== null) {
      const tag = match[0];
      const inner = tag.slice(2, -2).trim();

      if (inner.startsWith("#if ")) {
        depth++;
        stack.push("if");
      } else if (inner.startsWith("#unless ")) {
        depth++;
        stack.push("unless");
      } else if (inner.startsWith("#each ")) {
        depth++;
        stack.push("each");
      } else if (inner === "/if" || inner === "/unless" || inner === "/each") {
        const expected = inner.slice(1);
        const last = stack.pop();
        if (last !== expected) {
          errors.push(`Unmatched closing tag: {{${inner}}}, expected {{/${last ?? "?"}}}`);
        }
        depth--;
      }
    }

    if (stack.length > 0) {
      errors.push(`Unclosed block tag(s): ${stack.map((s) => `{{#${s}}}...{{/${s}}}`).join(", ")}`);
    }
    if (depth !== 0) {
      errors.push("Block tag depth mismatch");
    }

    return { valid: errors.length === 0, errors };
  }

  render(template: string, context: TemplateContext): string {
    if (!template) return "";

    const tokens = this.tokenize(template);
    return this.evaluate(tokens, context, 0);
  }

  private tokenize(template: string): Token[] {
    const tokens: Token[] = [];
    const regex = /\{\{([#/]?[\s\S]*?)\}\}/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = regex.exec(template)) !== null) {
      if (match.index > lastIndex) {
        tokens.push({ type: "text", value: template.slice(lastIndex, match.index) });
      }

      const inner = match[1].trim();
      const fullTag = match[0];

      if (inner.startsWith("#if ")) {
        const expr = inner.slice(4).trim();
        tokens.push({
          type: "if",
          value: expr,
          body: [],
          elseBody: [],
        });
      } else if (inner.startsWith("#unless ")) {
        const expr = inner.slice(7).trim();
        tokens.push({
          type: "unless",
          value: expr,
          body: [],
          elseBody: [],
        });
      } else if (inner.startsWith("#each ")) {
        const expr = inner.slice(6).trim();
        tokens.push({
          type: "each",
          value: expr,
          body: [],
        });
      } else if (inner === "/if" || inner === "/unless" || inner === "/each") {
        tokens.push({ type: "end", value: inner.slice(1) });
      } else if (inner === "else") {
        tokens.push({ type: "else", value: "" });
      } else {
        const parts = inner.split(/\s+/);
        if (parts.length > 1 && this.helpers.has(parts[0])) {
          tokens.push({ type: "helper", value: parts[0], args: parts.slice(1) });
        } else if (parts.length >= 2 && parts[1] === "|" && parts.length >= 3 && this.helpers.has(parts[2])) {
          tokens.push({ type: "helper", value: parts[2], args: [parts[0]] });
        } else {
          tokens.push({ type: "variable", value: inner });
        }
      }

      lastIndex = match.index + fullTag.length;
    }

    if (lastIndex < template.length) {
      tokens.push({ type: "text", value: template.slice(lastIndex) });
    }

    return this.buildTree(tokens);
  }

  private buildTree(tokens: Token[]): Token[] {
    const root: Token[] = [];
    const stack: Token[] = [];
    let inElse = false;

    for (const token of tokens) {
      if (token.type === "if" || token.type === "unless" || token.type === "each") {
        token.body = [];
        inElse = false;
        if (stack.length > 0) {
          const parent = stack[stack.length - 1];
          if (inElse) {
            parent.elseBody?.push(token);
          } else {
            parent.body?.push(token);
          }
        } else {
          root.push(token);
        }
        stack.push(token);
      } else if (token.type === "else") {
        if (stack.length > 0) {
          const parent = stack[stack.length - 1];
          if (parent.type === "if" || parent.type === "unless") {
            parent.elseBody = [];
            inElse = true;
          }
        }
      } else if (token.type === "end") {
        if (stack.length > 0) {
          stack.pop();
        }
        inElse = false;
      } else {
        if (stack.length > 0) {
          const parent = stack[stack.length - 1];
          if (inElse) {
            parent.elseBody?.push(token);
          } else {
            parent.body?.push(token);
          }
        } else {
          root.push(token);
        }
      }
    }

    return root;
  }

  private evaluate(tokens: Token[], context: TemplateContext, depth: number): string {
    if (depth > this.config.maxRecursion) return "";

    let result = "";

    for (const token of tokens) {
      switch (token.type) {
        case "text":
          result += token.value;
          break;

        case "variable": {
          const val = resolvePath(context, token.value);
          result += val !== undefined ? coerceString(val) : this.config.missingPlaceholder;
          break;
        }

        case "helper": {
          const fn = this.helpers.get(token.value);
          if (fn) {
            const resolvedArgs = (token.args ?? []).map((arg) => {
              const resolved = resolvePath(context, arg);
              return resolved !== undefined ? coerceString(resolved) : arg;
            });
            result += fn(...resolvedArgs);
          } else {
            result += this.config.missingPlaceholder;
          }
          break;
        }

        case "if": {
          const val = resolvePath(context, token.value);
          const body = truthy(val) ? (token.body ?? []) : (token.elseBody ?? []);
          result += this.evaluate(body, context, depth + 1);
          break;
        }

        case "unless": {
          const val = resolvePath(context, token.value);
          const body = !truthy(val) ? (token.body ?? []) : (token.elseBody ?? []);
          result += this.evaluate(body, context, depth + 1);
          break;
        }

        case "each": {
          const val = resolvePath(context, token.value);
          if (Array.isArray(val)) {
            for (let i = 0; i < val.length; i++) {
              const item = val[i];
              const itemContext = this.createItemContext(context, token.value, item, i);
              itemContext["this"] = typeof item === "object" && item !== null ? item : item;
              result += this.evaluate(token.body ?? [], itemContext, depth + 1);
            }
          } else if (typeof val === "object" && val !== null) {
            for (const [key, item] of Object.entries(val)) {
              const itemContext = this.createItemContext(context, token.value, item as Record<string, unknown>, -1, key);
              itemContext["this"] = item;
              result += this.evaluate(token.body ?? [], itemContext, depth + 1);
            }
          }
          break;
        }
      }
    }

    return result;
  }

  private createItemContext(
    parentContext: TemplateContext,
    path: string,
    item: unknown,
    index: number,
    key?: string,
  ): TemplateContext {
    const parts = path.split(".");
    let newContext = JSON.parse(JSON.stringify(parentContext));
    let target = newContext as Record<string, unknown>;

    for (let i = 0; i < parts.length - 1; i++) {
      if (typeof target[parts[i]] === "object" && target[parts[i]] !== null) {
        target = target[parts[i]] as Record<string, unknown>;
      }
    }

    const lastPart = parts[parts.length - 1];

    if (Array.isArray(item)) {
      (target[lastPart] as unknown[]) = item;
    } else if (typeof item === "object" && item !== null) {
      (target[lastPart] as Record<string, unknown>) = item as Record<string, unknown>;
    } else {
      (target[lastPart] as unknown) = item;
    }

    (newContext as Record<string, unknown>)["@index"] = index >= 0 ? index : undefined;
    (newContext as Record<string, unknown>)["@key"] = key;
    (newContext as Record<string, unknown>)["@first"] = index === 0;
    (newContext as Record<string, unknown>)["@last"] = false;

    return newContext as TemplateContext;
  }
}

export function buildTemplateContext(event: {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
  metadata: string;
}, chainOverrides?: Partial<TemplateContext["chain"]>, linksOverrides?: Partial<TemplateContext["links"]>): TemplateContext {
  const ts = event.timestamp;
  const date = new Date(ts * 1000);
  const chainBaseUrl = chainOverrides?.id === "stellar" ? "https://stellar.expert/explorer" : "https://stellar.expert/explorer";

  let metadataJson: Record<string, unknown> | null = null;
  try {
    const parsed = JSON.parse(event.metadata);
    if (typeof parsed === "object" && parsed !== null) {
      metadataJson = parsed as Record<string, unknown>;
    }
  } catch {
    metadataJson = null;
  }

  const chain_id = chainOverrides?.id ?? "stellar";
  const chain_network = chainOverrides?.network ?? "testnet";
  const chain_name = chainOverrides?.name ?? "Stellar";
  const explorer_url = linksOverrides?.explorer_url ?? `${chainBaseUrl}/${chain_network}`;
  const transaction_url = linksOverrides?.transaction_url ?? "";
  const event_url = linksOverrides?.event_url ?? "";
  const formattedDate = date.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
  const formattedTime = date.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit" });
  const formattedDatetime = date.toLocaleString("en-US", {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
  });

  return {
    event: {
      index: event.index,
      timestamp: event.timestamp,
      event_type: event.event_type,
      submitter: event.submitter,
      metadata: event.metadata,
    },
    chain: { id: chain_id, network: chain_network, name: chain_name },
    links: { explorer_url, transaction_url, event_url },
    formatted: { date: formattedDate, time: formattedTime, datetime: formattedDatetime },
    metadata_json: metadataJson,
    index: event.index,
    timestamp: event.timestamp,
    event_type: event.event_type,
    submitter: event.submitter,
    metadata: event.metadata,
    chain_id,
    chain_network,
    chain_name,
    explorer_url,
    transaction_url,
    event_url,
    date: formattedDate,
    time: formattedTime,
    datetime: formattedDatetime,
  };
}
