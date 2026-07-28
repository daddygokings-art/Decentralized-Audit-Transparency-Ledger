/**
 * #232 — SDK Event Validation
 *
 * Client-side validation before events are submitted to the contract.
 * Validates metadata size, event type format, submitter address format,
 * and supports custom validation rules.
 */

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

/**
 * A custom validation rule. Return a string error message if invalid,
 * or null/undefined if the event passes.
 */
export type ValidationRule = (event: EventInput) => string | null | undefined;

export interface EventInput {
  submitter: string;
  type: string;
  metadata: string;
}

export interface ValidatorOptions {
  /** Maximum metadata byte length (default: 4096) */
  maxMetadataBytes?: number;
  /** Allowed event type pattern (default: alphanumeric + underscore/hyphen, 1–64 chars) */
  eventTypePattern?: RegExp;
  /** Minimum event type length (default: 1) */
  minEventTypeLength?: number;
  /** Maximum event type length (default: 64) */
  maxEventTypeLength?: number;
  /** Submitter address pattern (default: Stellar public key G…) */
  submitterPattern?: RegExp;
  /** Additional custom rules */
  customRules?: ValidationRule[];
}

const STELLAR_ADDRESS_RE = /^G[A-Z2-7]{55}$/;
const DEFAULT_EVENT_TYPE_RE = /^[a-zA-Z0-9_\-]+$/;

export class EventValidator {
  private maxMetadataBytes: number;
  private eventTypePattern: RegExp;
  private minEventTypeLength: number;
  private maxEventTypeLength: number;
  private submitterPattern: RegExp;
  private customRules: ValidationRule[];

  constructor(options: ValidatorOptions = {}) {
    this.maxMetadataBytes = options.maxMetadataBytes ?? 4096;
    this.eventTypePattern = options.eventTypePattern ?? DEFAULT_EVENT_TYPE_RE;
    this.minEventTypeLength = options.minEventTypeLength ?? 1;
    this.maxEventTypeLength = options.maxEventTypeLength ?? 64;
    this.submitterPattern = options.submitterPattern ?? STELLAR_ADDRESS_RE;
    this.customRules = options.customRules ?? [];
  }

  /**
   * Add a custom validation rule at runtime.
   */
  addRule(rule: ValidationRule): void {
    this.customRules.push(rule);
  }

  /**
   * Validate a single event input.
   * Returns { valid, errors } — errors is an empty array when valid.
   */
  validate(event: EventInput): ValidationResult {
    const errors: string[] = [];

    // --- Metadata size validation ---
    const metadataByteLength = this.byteLength(event.metadata);
    if (metadataByteLength > this.maxMetadataBytes) {
      errors.push(
        `Metadata size ${metadataByteLength} bytes exceeds maximum of ${this.maxMetadataBytes} bytes`,
      );
    }

    // --- Event type validation ---
    if (typeof event.type !== 'string' || event.type.length === 0) {
      errors.push('Event type must be a non-empty string');
    } else {
      if (event.type.length < this.minEventTypeLength) {
        errors.push(
          `Event type length ${event.type.length} is below minimum of ${this.minEventTypeLength}`,
        );
      }
      if (event.type.length > this.maxEventTypeLength) {
        errors.push(
          `Event type length ${event.type.length} exceeds maximum of ${this.maxEventTypeLength}`,
        );
      }
      if (!this.eventTypePattern.test(event.type)) {
        errors.push(`Event type "${event.type}" does not match required pattern ${this.eventTypePattern}`);
      }
    }

    // --- Submitter address validation ---
    if (typeof event.submitter !== 'string' || event.submitter.length === 0) {
      errors.push('Submitter address must be a non-empty string');
    } else if (!this.submitterPattern.test(event.submitter)) {
      errors.push(`Submitter address "${event.submitter}" is not a valid Stellar public key (G… format)`);
    }

    // --- Custom rules ---
    for (const rule of this.customRules) {
      const result = rule(event);
      if (result) {
        errors.push(result);
      }
    }

    return { valid: errors.length === 0, errors };
  }

  /**
   * Validate multiple events, returns a result per event.
   */
  validateBatch(events: EventInput[]): ValidationResult[] {
    return events.map((e) => this.validate(e));
  }

  /**
   * Throws an AuditLedgerValidationError if the event is invalid.
   */
  validateOrThrow(event: EventInput): void {
    const result = this.validate(event);
    if (!result.valid) {
      throw new ValidationError(`Event validation failed: ${result.errors.join('; ')}`, result.errors);
    }
  }

  private byteLength(str: string): number {
    // UTF-8 byte length without TextEncoder dependency
    let len = 0;
    for (let i = 0; i < str.length; i++) {
      const code = str.charCodeAt(i);
      if (code < 0x80) len += 1;
      else if (code < 0x800) len += 2;
      else if (code < 0xd800 || code >= 0xe000) len += 3;
      else {
        // surrogate pair → 4 bytes
        i++;
        len += 4;
      }
    }
    return len;
  }
}

export class ValidationError extends Error {
  readonly errors: string[];

  constructor(message: string, errors: string[]) {
    super(message);
    this.name = 'ValidationError';
    this.errors = errors;
  }
}
