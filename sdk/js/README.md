# AuditLedger JS/TS SDK (minimal)

This folder contains a minimal TypeScript SDK for AuditLedger. It exposes an `AuditLedgerClient` class that communicates via a pluggable transport.

Usage example:

```ts
import { AuditLedgerClient } from '@auditledger/sdk';

const client = AuditLedgerClient.fromRpc('http://localhost:8080/rpc', 'CONTRACT_ID');
await client.initialize('OWNER_ADDR', 1000);
```

The SDK supports `watchEvents` via WebSocket, and `submitBatch` with progress callbacks.
