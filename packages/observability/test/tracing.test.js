const test = require('node:test');
const assert = require('node:assert');
const {
  OpenTelemetryTracer,
  TraceContextUtils,
  ErrorPrioritySampler,
  InMemoryTraceExporter,
  AuditAttributes,
} = require('../dist/tracing.js');

test('OpenTelemetry Tracing Standardization', async (t) => {
  await t.test('generates and propagates W3C traceparent headers', () => {
    const traceId = '4bf92f3577b34da6a3ce929d0e0e4736';
    const spanId = '00f067aa0ba902b7';
    const context = { traceId, spanId, traceFlags: 1 };

    const header = TraceContextUtils.serializeTraceparent(context);
    assert.strictEqual(header, `00-${traceId}-${spanId}-01`);

    const parsed = TraceContextUtils.parseTraceparent(header);
    assert.ok(parsed);
    assert.strictEqual(parsed.traceId, traceId);
    assert.strictEqual(parsed.spanId, spanId);
    assert.strictEqual(parsed.traceFlags, 1);
  });

  await t.test('creates spans with audit semantic attributes and parent correlation', () => {
    const exporter = new InMemoryTraceExporter();
    const tracer = new OpenTelemetryTracer({
      serviceName: 'audit-event-indexer',
      exporter,
    });

    const parentSpan = tracer.startSpan('IngestContractEvent', {
      attributes: {
        [AuditAttributes.CONTRACT_ID]: 'CA7Q...CONTRACT',
        [AuditAttributes.EVENT_TYPE]: 'AuditLogCreated',
      },
    });

    const parentContext = parentSpan.context();
    assert.strictEqual(parentContext.traceId.length, 32);
    assert.strictEqual(parentContext.spanId.length, 16);

    const childSpan = tracer.startSpan('VerifyProof', {
      parent: parentContext,
      attributes: {
        [AuditAttributes.TARGET_CHAIN]: 'ethereum-sepolia',
      },
    });

    assert.strictEqual(childSpan.context().traceId, parentContext.traceId);
    assert.notStrictEqual(childSpan.context().spanId, parentContext.spanId);

    childSpan.end();
    parentSpan.end();

    assert.strictEqual(exporter.exportedSpans.length, 2);
  });

  await t.test('withSpan automatically records exceptions and marks error status', async () => {
    const exporter = new InMemoryTraceExporter();
    const tracer = new OpenTelemetryTracer({
      serviceName: 'audit-relayer',
      exporter,
    });

    await assert.rejects(async () => {
      await tracer.withSpan('RelayBatch', async (span) => {
        span.setAttribute(AuditAttributes.CONTRACT_ID, 'TEST_CONTRACT');
        throw new Error('EVM submission failed with gas error');
      });
    }, /EVM submission failed/);

    assert.strictEqual(exporter.exportedSpans.length, 1);
    const recordedSpan = exporter.exportedSpans[0];
    assert.strictEqual(recordedSpan.getStatus().code, 'ERROR');
    assert.match(recordedSpan.getStatus().message, /gas error/);

    const events = recordedSpan.getEvents();
    assert.strictEqual(events.length, 1);
    assert.strictEqual(events[0].name, 'exception');
  });

  await t.test('ErrorPrioritySampler retains 100% of error traces', () => {
    const sampler = new ErrorPrioritySampler(0.0); // 0% sample rate for standard traffic
    const decision = sampler.shouldSample('123', 'test');
    assert.strictEqual(decision.isSampled, false);

    // When parent context is sampled (e.g. error propagated), retains trace
    const errorParentDecision = sampler.shouldSample('123', 'test', {}, { traceId: '1', spanId: '2', traceFlags: 1 });
    assert.strictEqual(errorParentDecision.isSampled, true);
  });
});
