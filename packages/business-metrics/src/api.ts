import { BusinessMetricsAggregator } from './aggregator';
import { ExecutiveReportGenerator } from './reporting';

export function createBusinessMetricsApiRouter(aggregator: BusinessMetricsAggregator) {
  return (req: any, res: any) => {
    const path = req.path || req.url || '';
    const summary = aggregator.generateExecutiveSummary();

    if (path.endsWith('/overview')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary));
    }

    if (path.endsWith('/submitters')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary.submitters));
    }

    if (path.endsWith('/growth')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary.growth));
    }

    if (path.endsWith('/governance')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary.governance));
    }

    if (path.endsWith('/bridge')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary.bridge));
    }

    if (path.endsWith('/api-adoption')) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify(summary.apiAdoption));
    }

    if (path.endsWith('/report')) {
      const markdown = ExecutiveReportGenerator.generateMarkdownReport(summary);
      res.writeHead(200, { 'Content-Type': 'text/markdown' });
      return res.end(markdown);
    }

    if (path.endsWith('/metrics')) {
      const metrics = ExecutiveReportGenerator.toPrometheusMetrics(summary);
      res.writeHead(200, { 'Content-Type': 'text/plain; version=0.0.4' });
      return res.end(metrics);
    }

    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Endpoint not found' }));
  };
}
