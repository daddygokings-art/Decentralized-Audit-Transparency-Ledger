import { OpenTelemetryTracer } from './tracing';
import { MetricsRegistry, createStandardObservabilityMetrics } from './metrics';
import { LokiLogger } from './logger';

export interface ObservabilityMiddlewareOptions {
  tracer: OpenTelemetryTracer;
  metrics: ReturnType<typeof createStandardObservabilityMetrics>;
  logger: LokiLogger;
}

export function createHttpObservabilityMiddleware(options: ObservabilityMiddlewareOptions) {
  const { tracer, metrics, logger } = options;

  return (req: any, res: any, next: any) => {
    const startTime = Date.now();
    const carrier: Record<string, string | undefined> = {};
    if (req.headers) {
      for (const [k, v] of Object.entries(req.headers)) {
        carrier[k.toLowerCase()] = Array.isArray(v) ? v[0] : (v as string | undefined);
      }
    }

    const parentContext = tracer.extractContext(carrier) || undefined;
    const span = tracer.startSpan(`HTTP ${req.method || 'GET'} ${req.path || req.url || '/'}`, {
      parent: parentContext,
      attributes: {
        'http.method': req.method || 'GET',
        'http.url': req.url || '/',
        'http.route': req.route?.path || req.path || req.url,
        'http.user_agent': req.headers ? req.headers['user-agent'] : undefined,
      },
    });

    const spanContext = span.context();

    if (res.setHeader) {
      res.setHeader('X-Trace-Id', spanContext.traceId);
      res.setHeader('X-Span-Id', spanContext.spanId);
    }

    const originalEnd = res.end;
    res.end = function (...args: any[]) {
      const durationMs = Date.now() - startTime;
      const statusCode = res.statusCode || 200;

      span.setAttribute('http.status_code', statusCode);
      if (statusCode >= 400) {
        span.setStatus({ code: 'ERROR', message: `HTTP ${statusCode}` });
        metrics.errorsTotal.inc({ component: 'http_api', error_type: `http_${statusCode}` });
      } else {
        span.setStatus({ code: 'OK' });
      }

      span.end();

      logger.info(`HTTP ${req.method} ${req.path || req.url} ${statusCode} (${durationMs}ms)`, {
        spanContext,
        context: {
          statusCode,
          durationMs,
          method: req.method,
          path: req.path || req.url,
        },
      });

      return originalEnd.apply(this, args);
    };

    if (typeof next === 'function') {
      next();
    }
  };
}
