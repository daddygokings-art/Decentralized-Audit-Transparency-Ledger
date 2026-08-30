import http from 'k6/http';
import { check, sleep } from 'k6';

const baseUrl = __ENV.API_BASE_URL || 'http://localhost:3002';

function request(path, tags = {}) {
  const res = http.get(`${baseUrl}${path}`, { tags });
  check(res, {
    [`${path} status is 200`]: (r) => r.status === 200,
    [`${path} has body`]: (r) => !!r.body && r.body.length > 0,
  });
  return res;
}

export function apiLoad() {
  request('/healthz', { endpoint: 'healthz' });
  request('/readyz', { endpoint: 'readyz' });
  request('/metrics', { endpoint: 'metrics' });
  request('/v1/events?limit=20', { endpoint: 'events' });
  request('/v1/events/type/payment?limit=20', { endpoint: 'event_type' });
  sleep(0.2);
}

export const options = {
  thresholds: {
    http_req_failed: ['rate<0.01'],
    checks: ['rate>0.99'],
    http_req_duration: ['p(95)<500'],
    'http_req_duration{endpoint:healthz}': ['p(95)<200'],
    'http_req_duration{endpoint:events}': ['p(95)<800'],
    'http_req_duration{endpoint:event_type}': ['p(95)<800'],
  },
  scenarios: {
    api_smoke: {
      executor: 'shared-iterations',
      vus: 5,
      iterations: 25,
      maxDuration: '30s',
      exec: 'apiLoad',
    },
    api_load: {
      executor: 'constant-arrival-rate',
      rate: 20,
      timeUnit: '1s',
      duration: '2m',
      preAllocatedVUs: 20,
      maxVUs: 80,
      exec: 'apiLoad',
    },
  },
};
