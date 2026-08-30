# Contract Events Reference

Welcome to the comprehensive smart contract events documentation for the Decentralized Audit Transparency Ledger (AuditLedger).

## 1. Overview

AuditLedger smart contracts execute on Soroban (Stellar) and emit structured contract events via `env.events().publish(topics, payload)`. These events provide an immutable, verifiable audit trail for on-chain state transitions, regulatory compliance proofs, financial attestations, and administrative actions.

## 2. Event Taxonomy

Events emitted across AuditLedger are grouped into four primary categories:

```
Contract Events
├── 1. Core Ledger & Administrative Events (core-events.md)
│   ├── event_stored
│   ├── events_archived
│   ├── contract_paused / contract_unpaused
│   └── owner_added / owner_removed / proposal_*
├── 2. Compliance & Regulatory Events (compliance-events.md)
│   ├── anti_corruption
│   ├── export_controls
│   ├── trade_compliance
│   ├── data_retention
│   ├── esg_reporting
│   └── responsible_sourcing
├── 3. Financial & Asset Lifecycle Events (financial-events.md)
│   ├── rwa_asset
│   ├── rwa_valuation
│   ├── rwa_compliance
│   ├── stablecoin_reserves
│   └── cbdc_logging
└── 4. Governance & Identity Events (governance-events.md)
    ├── dao_governance
    ├── dao_treasury
    ├── dao_dispute_resolution
    ├── token_gating
    └── submitter_dids / reputation_system
```

## 3. Topic Structure & Schema Standards

Every contract event follows a standardized topic structure:

1. **Primary Topic Symbol**: Defines the event domain (e.g. `Symbol::new(&env, "anti_corruption")`).
2. **Action Symbol**: Specific operation or state mutation (e.g. `Symbol::new(&env, "incident_reported")`).
3. **Identifier / Context**: Address, asset ID, or index (e.g. caller `Address` or `BytesN<32>`).

### Event Payload Encoding
- **On-Chain**: Encoded as Soroban `Val` tuples or maps.
- **Off-Chain (REST / GraphQL / WebSocket)**: JSON-serialized payloads with ISO-8601 timestamps and hex-encoded hashes.

## 4. Querying and Subscribing

- **REST API**: `GET /v1/events?topic=anti_corruption` or `GET /v1/events/type/:type`
- **GraphQL API**: `subscription { eventEmitted(topic: "trade_compliance") { id topic payload timestamp } }`
- **WebSocket Feed**: `ws://api.auditledger.io/v1/events/stream`

---

*See individual category guides for payload schemas, examples, and field descriptions.*
