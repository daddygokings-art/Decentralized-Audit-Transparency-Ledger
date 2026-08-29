# @audit-ledger/contract-testing

Consumer-Driven API Contract Testing with Pact for the Decentralized Audit Transparency Ledger.

## Overview

This test suite implements Consumer-Driven Contract (CDC) testing using **Pact**, ensuring that consumers (Web UI, SDKs, Bridge Relayers) and providers (REST API, GraphQL API) can evolve independently without breaking API compatibility.

## Components

1. **Consumers Tested**:
   - `AuditLedgerWebUI`: Frontend portal consuming event streaming, pagination, individual event lookup, and metrics endpoints.
   - `AuditLedgerSDK`: JS & Python client SDK consuming event queries by topic/type and readiness probes.
   - `BridgeRelayer`: Cross-chain relayer consuming health, tamper-evidence verification, and metrics endpoints.

2. **Provider Verification**:
   - Provider state management (`tests/contract-testing/provider/provider-states.ts`).
   - In-process HTTP provider server (`tests/contract-testing/provider/provider-server.ts`).
   - Automated verification runner (`tests/contract-testing/provider/verify-provider.ts`).

3. **Pact Broker & CI Integration**:
   - Automatic contract publication (`scripts/publish-pacts.ts`).
   - Pre-deployment gating via `can-i-deploy` (`scripts/can-i-deploy.ts`).
   - Environment deployment recording (`scripts/record-deployment.ts`).

## Running Contract Tests

```bash
cd tests/contract-testing

# 1. Run consumer contract tests (generates/validates pacts in pacts/)
npm run test:consumers

# 2. Run provider contract verification against pacts
npm run test:provider

# 3. Publish pacts to Pact Broker
npm run pact:publish

# 4. Check if safe to deploy
npm run pact:can-i-deploy
```
