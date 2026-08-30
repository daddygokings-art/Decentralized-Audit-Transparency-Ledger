# API Contract Testing Guide with Pact

This guide provides instructions for defining consumer contracts, implementing provider states, and validating APIs using Pact in AuditLedger.

## 1. Directory Structure

```
tests/contract-testing/
├── consumers/
│   ├── bridge-relayer-consumer.test.ts
│   ├── run-consumers.ts
│   ├── sdk-consumer.test.ts
│   └── web-ui-consumer.test.ts
├── pacts/
│   ├── AuditLedgerSDK-AuditLedgerRestAPI.json
│   ├── AuditLedgerWebUI-AuditLedgerRestAPI.json
│   └── BridgeRelayer-AuditLedgerRestAPI.json
├── provider/
│   ├── provider-server.ts
│   ├── provider-states.ts
│   └── verify-provider.ts
├── scripts/
│   ├── can-i-deploy.ts
│   ├── publish-pacts.ts
│   └── record-deployment.ts
├── package.json
├── pact.config.ts
└── tsconfig.json
```

## 2. Writing a Consumer Test

When adding a new API endpoint or request pattern to a consumer:
1. Define the HTTP method, path, query parameters, and expected response status and body schema.
2. Specify the required provider state (e.g. `event with ID 10 exists`).
3. Generate or update the pact file in `pacts/`.

## 3. Implementing Provider States

If your consumer test requires specific backend state:
1. Open `provider/provider-states.ts`.
2. Add a `case 'your state description':` block in `setupProviderState`.
3. Mutate the state context (e.g. inject test events or set mock database responses).

## 4. Running Contract Tests

```bash
cd tests/contract-testing

# Run consumer tests
npm run test:consumers

# Verify provider against contracts
npm run test:provider

# Check deployment readiness
npm run pact:can-i-deploy
```
