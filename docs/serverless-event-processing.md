# Contract Event Serverless Functions for Event Processing (#522)

This module provides cloud-agnostic serverless functions for event transformation, enrichment, filtering, and multi-cloud routing across:
- **AWS Lambda** (SQS triggers, EventBridge bus, DynamoDB Streams)
- **Google Cloud Functions (GCF Gen2)** (HTTP endpoints, Cloud Pub/Sub, Eventarc)
- **Azure Functions** (HTTP Triggers, Event Grid, Service Bus)
- **Knative Serving & Eventing** (CloudEvents v1.0 standard, scale-to-zero)

---

## Core Pipeline Architecture

```
Contract Event
      │
      ▼
┌──────────────────┐
│   EventFilter    │ ── (drop if unmatched) ──► DLQ / Discard
└─────────┬────────┘
          │ (pass)
          ▼
┌──────────────────┐
│   EventEnricher  │ ── (DID resolver, risk scoring, geo-context)
└─────────┬────────┘
          │
          ▼
┌──────────────────┐
│ EventTransformer │ ── (JSON / CloudEvents / Proto conversion)
└─────────┬────────┘
          │
          ▼
┌──────────────────┐
│   EventRouter    │ ──► AWS EventBridge / SQS
└──────────────────┘ ──► GCP PubSub
                     ──► Azure Service Bus
                     ──► Knative CloudEvents Broker
```

---

## Soroban On-Chain Coordination (`src/serverless_processing.rs`)

- `register_function`: On-chain registry of active serverless processors and their cryptographic endpoints.
- `record_processing_receipt`: Attests input/output hashes and execution timing for verifiable computation.
- `set_routing_rule`: Stores declarative routing rules and destination endpoints.
- `get_function`, `get_processing_receipt`, `get_routing_rule`: Query APIs for auditing receipts.
