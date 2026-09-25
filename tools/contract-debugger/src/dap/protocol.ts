/**
 * Minimal Debug Adapter Protocol codec.
 *
 * DAP is JSON-RPC 2.0 over a stream, each message framed by a `Content-Length`
 * header. Only framing and dispatch live here; the adapter logic is in
 * `server.ts`, which keeps the protocol layer testable without a socket.
 */

export interface DapMessage {
  seq: number;
  type: 'request' | 'response' | 'event';
  /** Set on requests. */
  command?: string;
  /** Set on events. */
  event?: string;
  /** Set on requests, echoed on the matching response. */
  request_seq?: number;
  command_?: string;
  success?: boolean;
  message?: string;
  body?: Record<string, unknown>;
  arguments?: Record<string, unknown>;
}

const HEADER = 'Content-Length: ';

/** Encode a message with its `Content-Length` header. Buffer, not a string. */
export function encodeMessage(message: DapMessage): Buffer {
  const body = JSON.stringify(message);
  return Buffer.from(`${HEADER}${Buffer.byteLength(body, 'utf8')}\r\n\r\n${body}`, 'utf8');
}

/**
 * Incremental framer.
 *
 * Feeds arbitrary byte chunks and emits whole messages. Headers are
 * case-insensitive per the spec, and a partial message is buffered until the
 * rest arrives — a debugger client writes headers and body in one write, but a
 * pipe can still split them.
 */
export class MessageDecoder {
  private buffer = Buffer.alloc(0);

  push(chunk: Buffer): DapMessage[] {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    const out: DapMessage[] = [];
    for (;;) {
      const headerEnd = this.buffer.indexOf('\r\n\r\n');
      if (headerEnd === -1) return out;
      const header = this.buffer.subarray(0, headerEnd).toString('utf8');
      const match = /content-length:\s*(\d+)/i.exec(header);
      if (!match) {
        // Unrecoverable framing error: drop the header and resynchronise.
        this.buffer = this.buffer.subarray(headerEnd + 4);
        continue;
      }
      const length = Number(match[1]);
      const start = headerEnd + 4;
      if (this.buffer.length < start + length) return out;
      const body = this.buffer.subarray(start, start + length).toString('utf8');
      this.buffer = this.buffer.subarray(start + length);
      try {
        out.push(JSON.parse(body) as DapMessage);
      } catch {
        // Skip unparseable payloads rather than killing the session.
      }
    }
  }

  get pending(): number {
    return this.buffer.length;
  }
}

export type DapHandler = (
  message: DapMessage,
) => Promise<DapMessage | DapMessage[] | null> | DapMessage | DapMessage[] | null;

let sequence = 0;

export function nextSeq(): number {
  sequence += 1;
  return sequence;
}

/** Reset the shared sequence counter. Test-only. */
export function resetSeq(): void {
  sequence = 0;
}

export function makeResponse(request: DapMessage, body?: Record<string, unknown>): DapMessage {
  return { seq: nextSeq(), type: 'response', request_seq: request.seq, command: request.command, success: true, body };
}

export function makeErrorResponse(
  request: DapMessage,
  message: string,
  body?: Record<string, unknown>,
): DapMessage {
  return {
    seq: nextSeq(),
    type: 'response',
    request_seq: request.seq,
    command: request.command,
    success: false,
    message,
    body,
  };
}

export function makeEvent(event: string, body?: Record<string, unknown>): DapMessage {
  return { seq: nextSeq(), type: 'event', event, body };
}

/** Every `request_seq` the server has answered. Test helper. */
export function collectResponses(messages: DapMessage[]): Map<number, DapMessage> {
  const out = new Map<number, DapMessage>();
  for (const m of messages) if (m.type === 'response' && typeof m.request_seq === 'number') out.set(m.request_seq, m);
  return out;
}
