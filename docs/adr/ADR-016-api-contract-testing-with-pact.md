# ADR-016: Contract Event API Contract Testing with Pact

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-08-29 |
| **Deciders** | Backend, Frontend, and QA Engineering Teams |

---

## Context

AuditLedger's architecture features multiple API consumers (React/Next.js Web UI, JavaScript SDK, Python SDK, Cross-Chain Bridge Relayer) communicating with the REST API server (`api/rest`) and GraphQL API (`api/graphql`). 

Traditional end-to-end (E2E) integration tests across all consumers and services are brittle, slow, and fail to provide fast feedback when API schema modifications introduce breaking changes. We needed an independent, automated contract testing mechanism to ensure that consumer expectations and provider contracts remain compatible across releases without requiring shared live test environments.

---

## Decision

We adopt **Pact** (Consumer-Driven Contract Testing) for all API interactions across AuditLedger services:

1. **Consumer Contracts (`tests/contract-testing/consumers/`)**:
   - Consumers (`AuditLedgerWebUI`, `AuditLedgerSDK`, `BridgeRelayer`) write declarative test interactions specifying HTTP requests, queries, headers, and expected response bodies.
   - Executing consumer tests outputs versioned Pact contract files (`tests/contract-testing/pacts/*.json`).

2. **Provider State Management & Verification (`tests/contract-testing/provider/`)**:
   - The provider verification harness spins up the API provider server and configures provider state contexts (e.g. `events exist in the ledger`, `event with ID 0 exists`, `system is healthy`).
   - The verifier replays all interactions from consumer pacts against the running provider, validating status codes, response headers, and JSON body schemas.

3. **Pact Broker Integration & Can-I-Deploy Gating (`tests/contract-testing/scripts/`)**:
   - Generated pacts are published to a central Pact Broker (or validated locally in offline mode).
   - CI/CD deployment pipelines query `can-i-deploy` against the compatibility matrix to block incompatible releases before deployment to staging or production.

4. **CI/CD Workflow (`.github/workflows/pact-contract-testing.yml`)**:
   - Runs consumer contract tests, verifies provider adherence, and checks deployment safety on every PR affecting APIs or consumers.

---

## Consequences

### Positive
- **Independent Deployments**: Consumers and providers can release independently with mathematical confidence that interfaces match.
- **Fast Feedback**: Schema breakages are caught immediately at build time without needing full live environment orchestrations.
- **Living API Documentation**: Pact contract files serve as exact, tested specifications of how APIs are consumed.

### Negative
- Requires maintaining provider state fixtures as business logic and data structures evolve.
