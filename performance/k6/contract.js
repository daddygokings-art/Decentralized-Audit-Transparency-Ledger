import http from 'k6/http';
import { check, sleep } from 'k6';

const rpcUrl = __ENV.CONTRACT_RPC_URL || 'https://soroban-testnet.stellar.org';
const payload = JSON.stringify({
  jsonrpc: '2.0',
  id: 1,
  method: 'getHealth',
});

export function contractLoad() {
  const res = http.post(rpcUrl, payload, {
    headers: {
      'Content-Type': 'application/json',
    },
    tags: { endpoint: 'rpc_health' },
  });

  check(res, {
    'rpc responds with HTTP 200': (r) => r.status === 200,
    'rpc returns JSON-RPC envelope': (r) => {
      const body = r.json();
      return !!body && (body.result !== undefined || body.error !== undefined);
    },
  });
  sleep(0.5);
}

export const options = {
  thresholds: {
    http_req_failed: ['rate<0.02'],
    checks: ['rate>0.98'],
    http_req_duration: ['p(95)<1500'],
    'http_req_duration{endpoint:rpc_health}': ['p(95)<1500'],
  },
  scenarios: {
    contract_smoke: {
      executor: 'shared-iterations',
      vus: 3,
      iterations: 10,
      maxDuration: '20s',
      exec: 'contractLoad',
    },
    contract_load: {
      executor: 'constant-arrival-rate',
      rate: 5,
      timeUnit: '1s',
      duration: '2m',
      preAllocatedVUs: 5,
      maxVUs: 20,
      exec: 'contractLoad',
    },
  },
};
