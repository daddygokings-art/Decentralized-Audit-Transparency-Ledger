/**
 * #237 — SDK Logging & Debugging
 *
 * Provides configurable log levels, structured request/response logging,
 * error context enrichment, and performance (timing) logging.
 */

export enum LogLevel {
  NONE = 0,
  ERROR = 1,
  WARN = 2,
  INFO = 3,
  DEBUG = 4,
}

export interface LogEntry {
  level: LogLevel;
  message: string;
  timestamp: number;
  context?: Record<string, unknown>;
}

export type LogHandler = (entry: LogEntry) => void;

export interface LoggerOptions {
  level?: LogLevel;
  handler?: LogHandler;
  /** Prefix attached to every log message */
  prefix?: string;
}

const DEFAULT_HANDLER: LogHandler = (entry) => {
  const ts = new Date(entry.timestamp).toISOString();
  const lvl = LogLevel[entry.level];
  const ctx = entry.context ? ` ${JSON.stringify(entry.context)}` : '';
  const msg = `[audit-ledger-sdk][${ts}][${lvl}] ${entry.message}${ctx}`;
  if (entry.level === LogLevel.ERROR) {
    console.error(msg);
  } else if (entry.level === LogLevel.WARN) {
    console.warn(msg);
  } else {
    console.log(msg);
  }
};

export class Logger {
  private level: LogLevel;
  private handler: LogHandler;
  private prefix: string;

  constructor(options: LoggerOptions = {}) {
    this.level = options.level ?? LogLevel.WARN;
    this.handler = options.handler ?? DEFAULT_HANDLER;
    this.prefix = options.prefix ?? '';
  }

  setLevel(level: LogLevel): void {
    this.level = level;
  }

  getLevel(): LogLevel {
    return this.level;
  }

  setHandler(handler: LogHandler): void {
    this.handler = handler;
  }

  private emit(level: LogLevel, message: string, context?: Record<string, unknown>): void {
    if (level > this.level) return;
    this.handler({
      level,
      message: this.prefix ? `[${this.prefix}] ${message}` : message,
      timestamp: Date.now(),
      context,
    });
  }

  error(message: string, context?: Record<string, unknown>): void {
    this.emit(LogLevel.ERROR, message, context);
  }

  warn(message: string, context?: Record<string, unknown>): void {
    this.emit(LogLevel.WARN, message, context);
  }

  info(message: string, context?: Record<string, unknown>): void {
    this.emit(LogLevel.INFO, message, context);
  }

  debug(message: string, context?: Record<string, unknown>): void {
    this.emit(LogLevel.DEBUG, message, context);
  }

  /**
   * Log a transport request before it is sent.
   */
  logRequest(method: string, params: unknown[]): void {
    this.debug('Request', { method, params });
  }

  /**
   * Log a transport response after it is received.
   */
  logResponse(method: string, result: unknown, durationMs: number): void {
    this.debug('Response', { method, result, durationMs });
  }

  /**
   * Log an error with full context (method, attempt, error details).
   */
  logError(method: string, err: unknown, context?: Record<string, unknown>): void {
    const errCtx: Record<string, unknown> = {
      method,
      error: err instanceof Error ? { name: err.name, message: err.message } : String(err),
      ...context,
    };
    this.error('Transport error', errCtx);
  }

  /**
   * Log a performance measurement.
   */
  logPerformance(operation: string, durationMs: number, context?: Record<string, unknown>): void {
    this.info('Performance', { operation, durationMs, ...context });
  }
}

/** Shared default logger instance (users can replace via AuditLedgerClient options) */
export const defaultLogger = new Logger({ level: LogLevel.WARN });
