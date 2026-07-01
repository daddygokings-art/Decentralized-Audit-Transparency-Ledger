# Architecture Overview

This document explains how the Decentralized Audit & Transparency Ledger system fits together, from the Soroban contract to SDKs, APIs, the UI, off-chain services, and the cross-chain bridge.

## System Diagram

```mermaid
flowchart LR
  subgraph On-Chain
    C[AuditLedger Contract]
  end

  subgraph Off-Chain
    SDK[SDKs (JS/Python)]
    UI[UI / Viewer]
    REST[REST API (api/rest)]
    GraphQL[GraphQL API (api/graphql)]
    WS[WebSocket Event Stream (api/ws)]
    Metrics[Metrics Exporter (tools/metrics-exporter)]
    Notifier[Notifier Service (services/notifier)]
    Relayer[Bridge Relayer (bridge/relayer)]
    Prometheus[Prometheus]
    Grafana[Grafana]
    EVM[Verifier Contract (bridge/evm/Verifier.sol)]
  end

  SDK -->|Soroban RPC reads| C
  UI -->|Soroban RPC reads| C
  REST -->|API adapter| GraphQL
  GraphQL -->|Subscriptions| WS
  WS -->|Event stream| Notifier
  Metrics -->|Contract metrics| Prometheus
  Prometheus -->|Dashboards| Grafana
  C -->|Event logs + emitted events| Metrics
  C -->|Event logs + emitted events| REST
  C -->|Event logs + emitted events| GraphQL
  C -->|Event logs + emitted events| WS
  C -->|Event logs + inclusion proofs| Relayer
  Relayer -->|Verified proofs| EVM
```

> Note: The local Docker stack in `docker-compose.yml` starts `ui`, `rest`, `metrics-exporter`, `prometheus`, `grafana`, and `relayer`. The GraphQL API and WebSocket gateway are available as separate services in `api/graphql` and `api/ws`.

## Components

### 1. AuditLedger Contract

- Location: root crate (`Cargo.toml`, `src/`)
- Runs on the Stellar/Soroban network
- Exposes append-only event logging and read/query helpers
- Emits contract events and stores event metadata for on-chain auditability
- Supports governance functions such as caps, TTL, ownership transfers, and event limits

### 2. SDKs

- `sdk/js` — JavaScript/TypeScript client for AuditLedger
- `sdk/python` — Python SDK for AuditLedger

These SDKs provide a developer-friendly wrapper around Soroban contract calls and can be used by apps, scripts, and automation tooling.

### 3. REST API

- Location: `api/rest`
- Thin adapter exposing key query methods over HTTP
- Reuses GraphQL resolvers from `api/graphql` for event listing and stats
- Suitable for services or scripts that prefer plain REST access

### 4. GraphQL API

- Location: `api/graphql`
- Central API service for event queries and subscriptions
- Exposes a GraphQL schema with query and subscription support
- Can be used by client apps that need typed query semantics and live event streams

### 5. WebSocket Event Stream

- Location: `api/ws`
- Simple WebSocket gateway for pushing event notifications to subscribed clients
- Supports subscribing to specific event types or all events

### 6. UI

- Location: `ui`
- Next.js frontend for viewing AuditLedger events and statistics
- Reads contract state directly via Soroban RPC by default
- Uses the JS/TS client logic in `ui/src/lib/contract.ts`

### 7. Metrics Exporter

- Location: `tools/metrics-exporter`
- Polls the Soroban contract via RPC at regular intervals
- Exposes Prometheus-compatible metrics on `:8000/metrics`
- Helps operators monitor contract health, event rates, and storage usage

### 8. Notifier Service

- Location: `services/notifier`
- Connects to the WebSocket event stream and matches incoming events against rules
- Sends alerts via Slack, Telegram, Email, or Webhook
- Designed for compliance alerts, audit notifications, or operational alerts

### 9. Cross-Chain Bridge

- Relayer: `bridge/relayer`
- EVM verifier contract: `bridge/evm/Verifier.sol`

The bridge relayer polls Stellar events, builds inclusion proofs, and submits them to the EVM verifier contract. This enables independent verification of AuditLedger events on an EVM-compatible chain.

For bridge-specific architecture and proof details, see `bridge/docs/bridge-architecture.md`.

## Data Flow

### Event logged → indexed → displayed

1. An event is written to the `AuditLedger` contract on the Stellar network.
2. The contract stores the event in on-chain state and emits a Soroban event payload.
3. Off-chain services consume the event via one or more channels:
   - direct Soroban RPC reads from the UI or SDKs
   - GraphQL queries provided by `api/graphql`
   - REST queries provided by `api/rest`
   - event pushes from `api/ws`
4. The metrics exporter polls the contract to update Prometheus metrics.
5. The notifier service receives live events from the WebSocket stream and dispatches alerts.
6. The UI displays events and statistics using the SDK/contract client.
7. The bridge relayer consumes event data and generates proofs for the EVM verifier.

### Cross-chain verification path

- The relayer scans Stellar events and ledger metadata.
- It constructs a proof that an AuditLedger event was included in a given Stellar ledger.
- It submits that proof to `bridge/evm/Verifier.sol` on an EVM chain.
- The verifier contract validates the proof and marks the event as verified.

## Deployment Topology

### On-chain

- `AuditLedger` contract deployed to a Stellar/Soroban network
- Optionally, `bridge/evm/Verifier.sol` deployed to an EVM-compatible chain for cross-chain verification

### Off-chain services

- `ui` — Next.js frontend, usually served from `3001`
- `rest` — REST API service, usually served from `3002`
- `metrics-exporter` — Prometheus metrics endpoint, usually `8000`
- `prometheus` — metrics database, usually `9090`
- `grafana` — dashboard UI, usually `3000`
- `relayer` — bridge relayer health endpoint, usually `8080`

### Optional / separate services

- `api/graphql` — GraphQL endpoint, defaults to `4000`
- `api/ws` — WebSocket event stream, defaults to `4000`
- `services/notifier` — notification engine driven by the event stream
- `sdk/js` and `sdk/python` — client libraries used by applications and automation scripts

### Local Docker compose

The repository’s `docker-compose.yml` wires up a local development stack that includes:

- `relayer`
- `metrics-exporter`
- `prometheus`
- `grafana`
- `rest`
- `ui`

This stack is useful for seeing event metrics, dashboards, and the UI together with the contract.

## How to use this document

- Refer to `docs/architecture.md` when onboarding new contributors.
- Use `bridge/docs/bridge-architecture.md` for the bridge-specific trust model and proof flow.
- Use `docker-compose.yml` for local deployment of the main off-chain stack.
