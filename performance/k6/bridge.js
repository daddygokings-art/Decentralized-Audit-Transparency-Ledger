import http from 'k6/http';
import { check, sleep } from 'k6';

const relayerUrl = __ENV.RELAYER_URL || 'http://localhost:8080';
const metricsUrl = __ENV.METRICS_URL || 'http://localhost:8000';

function probe(url, tag) {
  const res = http.get(url, { tags: { endpoint: tag } });
  check(res, {
    [`${tag} status is 200`]: (r) => r.status === 200,
    [`${tag} has response`]: (r) => !!r.body && r.body.length > 0,
  });
  return res;
}

export function bridgeLoad() {
  probe(`${relayerUrl}/healthz`, 'relayer_health');
  probe(`${metricsUrl}/health`, 'metrics_health');
  probe(`${metricsUrl}/metrics`, 'metrics_export');
  sleep(0.4);
}

export const options = {
  thresholds: {
    http_req_failed: ['rate<0.01'],
    checks: ['rate>0.99'],
    http_req_duration: ['p(95)<1000'],
    'http_req_duration{endpoint:relayer_health}': ['p(95)<700'],
    'http_req_duration{endpoint:metrics_export}': ['p(95)<1200'],
  },
  scenarios: {
    bridge_smoke: {
      executor: 'shared-iterations',
      vus: 4,
      iterations: 20,
      maxDuration: '30s',
      exec: 'bridgeLoad',
    },
    bridge_load: {
      executor: 'constant-arrival-rate',
      rate: 8,
      timeUnit: '1s',
      duration: '2m',
      preAllocatedVUs: 10,
      maxVUs: 30,
      exec: 'bridgeLoad',
    },
  },
};
