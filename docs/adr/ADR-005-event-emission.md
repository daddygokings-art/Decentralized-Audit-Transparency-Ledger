# ADR-005: Event Emission Design for Off-Chain Consumers

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2024-01-01 |
| **Deciders** | Core team |

---

## Context

Soroban contracts can emit events that are visible in transaction metadata without being stored in contract state. Off-chain consumers (the metrics exporter, the UI, the WebSocket API, the bridge relayer) need to react to new events without polling contract storage.

The design must balance:
- **Information richness**: consumers need enough data to act without a secondary RPC call.
- **Cost**: emitting large payloads increases transaction fees.
- **Privacy**: some deployments may not want metadata visible in transaction history.

---

## Decision

Event emission is **configurable at runtime** via `set_event_emission_mode`. Four modes are available:

| Mode | Value | What is emitted |
|------|-------|-----------------|
| Full | `0` | `(event_type, submitter, index, metadata)` |
| Index-only | `1` | `(event_type, submitter, index)` — consumers fetch metadata via `get_event_metadata` |
| Hash-only | `2` | `(event_type, submitter, index, sha256(metadata))` |
| Silent | `3` | Nothing emitted |

The default mode is **Full (0)** to give integrators the richest experience out of the box.

### Event topic structure

All emitted events use a three-element topic tuple for consistent filtering:

```
topic: (Symbol("log_event"), event_type: Symbol, submitter: Address)
data:  depends on mode (see table above)
```

This allows consumers to subscribe with a single topic filter and differentiate by event type and submitter without parsing the data payload.

### Why not always emit full data?

Full emission stores metadata in transaction metadata on every Stellar validator. For sensitive financial data, operators may prefer index-only or hash-only mode and serve metadata through a permissioned off-chain layer. The configurable mode gives operators this choice without redeploying.

---

## Consequences

**Positive:**
- Off-chain consumers can subscribe to the Stellar event stream instead of polling `total_events`.
- Consistent topic structure enables filtered subscriptions per event type or submitter.
- Configurable mode allows privacy-conscious deployments to minimise on-chain data exposure.
- Hash-only mode allows consumers to verify metadata integrity without exposing the payload.

**Negative:**
- Index-only and hash-only modes require a secondary RPC call to fetch full event data; consumers must handle this gracefully.
- The four-mode configuration adds cognitive load for new integrators.
- Silent mode disables event-driven architectures entirely; documentation must warn operators clearly.

---

## How Consumers Should Process Events

1. **Subscribe** to the Stellar RPC event stream filtered by `CONTRACT_ID` and topic `log_event`.
2. **Parse** the topic to extract `event_type` and `submitter`.
3. **Extract** the sequential `index` from the data payload (all modes except Silent include it).
4. If in **index-only or hash-only** mode, call `get_event_metadata(id)` or `get_event(id)` using the ID from `get_event_by_order(index)` to retrieve full data.
5. **Verify** hash-chain integrity periodically using `verify_integrity` or `verify_integrity_range`.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|-------------|----------------|
| Always emit full data | Privacy risk for sensitive metadata; higher tx fees. |
| Never emit (storage-only) | Forces all consumers to poll; significantly higher RPC load. |
| Separate event contract | Doubles deployment complexity; state consistency between contracts is hard to guarantee. |
| Custom subscriber webhook | Off-chain concern; not suitable for an on-chain contract. |
