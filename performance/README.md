# Performance Testing with k6

This directory contains targeted k6 load tests covering the repository's most important runtime paths:

- API endpoints served by the REST service
- Contract JSON-RPC health checks against the Soroban RPC endpoint
- Bridge and metrics health probes for relayer/observer components

## Scripts

- `performance/k6/api.js` — REST API smoke and sustained load checks
- `performance/k6/contract.js` — Soroban RPC health and contract-facing access checks
- `performance/k6/bridge.js` — relayer and metrics exporter checks

## Running locally

```bash
# API checks
docker run --rm -i \
  -e API_BASE_URL=http://host.docker.internal:3002 \
  -v "$PWD:/src" -w /src \
  grafana/k6:latest run /src/performance/k6/api.js

# Contract checks
docker run --rm -i \
  -e CONTRACT_RPC_URL=https://soroban-testnet.stellar.org \
  -v "$PWD:/src" -w /src \
  grafana/k6:latest run /src/performance/k6/contract.js

# Bridge checks
docker run --rm -i \
  -e RELAYER_URL=http://host.docker.internal:8080 \
  -e METRICS_URL=http://host.docker.internal:8000 \
  -v "$PWD:/src" -w /src \
  grafana/k6:latest run /src/performance/k6/bridge.js
```

## Thresholds

The scenarios enforce conservative latency and failure-rate thresholds for CI so regressions are visible before release.
