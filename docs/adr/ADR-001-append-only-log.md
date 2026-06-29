# ADR-001: Append-Only Log Structure

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2024-01-01 |
| **Deciders** | Core team |

---

## Context

AuditLedger must provide a tamper-evident record of financial and operational events. The central design question was whether events should be **mutable** (updateable after the fact) or **append-only** (immutable once written).

Mutable storage allows corrections but introduces risk: any party with write access could silently alter historical records, undermining the audit guarantee. Append-only storage is inherently more conservative and aligns with the core value proposition of a public audit trail.

---

## Decision

All events are stored in an **append-only log**. Once an event is written to the ledger, its core fields (`index`, `timestamp`, `event_type`, `submitter`, `metadata`, `event_hash`, `prev_hash`) are never overwritten or deleted.

Events are keyed by a content-addressed SHA-256 ID:

```
id = sha256(contract_id || submitter || event_type_bytes || metadata || timestamp || index)
```

A sequential `EventOrder(u32) → BytesN<32>` mapping provides ordered access by insertion position.

---

## Consequences

**Positive:**
- Immutability is enforced at the storage layer — no governance action can retroactively alter events.
- Content-addressed IDs make tampering detectable: changing any field changes the ID.
- Simple mental model for integrators: event `N` always refers to the same event.

**Negative:**
- Incorrect events cannot be deleted; they can only be superseded by a correcting follow-up event.
- Storage grows monotonically; archiving and purge mechanisms (`archive_events`, `purge_archived_events`) are required to manage state size.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|-------------|----------------|
| Mutable key-value store | Would allow silent history tampering; breaks the audit guarantee. |
| Event sourcing with soft deletes | Adds complexity without a meaningful security benefit over append-only. |
| IPFS / off-chain storage with on-chain hash | Increases integration complexity and introduces liveness dependency on off-chain storage. |
