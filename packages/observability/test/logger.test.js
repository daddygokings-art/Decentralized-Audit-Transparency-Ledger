const test = require('node:test');
const assert = require('node:assert');
const { LokiLogger } = require('../dist/logger.js');

test('Loki Structured Logging Standardization', async (t) => {
  await t.test('formats structured JSON logs with trace correlation', () => {
    const logs = [];
    const logger = new LokiLogger({
      serviceName: 'audit-api',
      environment: 'production',
      minLevel: 'debug',
      sink: (entry) => logs.push(entry),
    });

    logger.info('Contract event processed', {
      contract_id: 'CONTRACT_ABC',
      event_type: 'AuditSubmitted',
      spanContext: {
        traceId: 'trace-123456',
        spanId: 'span-789',
        traceFlags: 1,
      },
      context: {
        ledger_seq: 104523,
      },
    });

    assert.strictEqual(logs.length, 1);
    const log = logs[0];
    assert.strictEqual(log.level, 'info');
    assert.strictEqual(log.service, 'audit-api');
    assert.strictEqual(log.trace_id, 'trace-123456');
    assert.strictEqual(log.span_id, 'span-789');
    assert.strictEqual(log.contract_id, 'CONTRACT_ABC');
    assert.strictEqual(log.event_type, 'AuditSubmitted');
    assert.strictEqual(log.context.ledger_seq, 104523);
  });

  await t.test('redacts sensitive keys in context payloads', () => {
    const logs = [];
    const logger = new LokiLogger({
      serviceName: 'relayer',
      sink: (entry) => logs.push(entry),
    });

    logger.warn('Relayer config loaded', {
      context: {
        host: 'https://rpc.stellar.org',
        privateKey: '0x123456789abcdef',
        api_key: 'supersecret',
      },
    });

    const log = logs[0];
    assert.strictEqual(log.context.host, 'https://rpc.stellar.org');
    assert.strictEqual(log.context.privateKey, '[REDACTED]');
    assert.strictEqual(log.context.api_key, '[REDACTED]');
  });

  await t.test('formats Loki push payload streams correctly', () => {
    const entries = [
      {
        timestamp: '2026-08-29T00:00:00.000Z',
        level: 'info',
        message: 'Event verified',
        service: 'bridge-verifier',
        environment: 'production',
        contract_id: 'CTR1',
      },
    ];

    const payload = LokiLogger.formatLokiPushPayload(entries);
    assert.ok(payload.streams);
    assert.strictEqual(payload.streams.length, 1);
    assert.strictEqual(payload.streams[0].stream.service, 'bridge-verifier');
    assert.strictEqual(payload.streams[0].stream.level, 'info');
    assert.strictEqual(payload.streams[0].values.length, 1);
  });
});
