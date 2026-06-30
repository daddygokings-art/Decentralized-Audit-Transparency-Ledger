# ADR-002: Global vs. Per-Event-Type Logging Limits

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2024-01-01 |
| **Deciders** | Core team |

---

## Context

Unbounded on-chain storage is a denial-of-service vector: a single actor can spam the ledger with millions of events, bloating contract state and raising storage fees for everyone. The contract needs a mechanism to cap the total number of events and, optionally, the volume of any single event type.

Two axes of control were considered:
1. A **global cap** on the total number of events across all types.
2. **Per-event-type caps** that limit how many events of a specific type can be logged.

---

## Decision

Both dimensions are implemented:

- **Global cap** (`global_max_logs` in `Config`): a hard upper bound on total events. Required at initialization. Can be raised by the owner.
- **Per-event-type caps** (`EventCapSet(Symbol)` + `EventMaxLogs(Symbol)`): optional caps per event type. They are **opt-in** — a type without a cap is limited only by the global cap.
- **Cap removal** (`remove_event_cap`): the owner can remove a per-type cap, returning that type to global-only enforcement. This is tracked in `EventCapRemoved(Symbol)` to prevent re-adding a cap that has been intentionally cleared.

### Interaction

An event is accepted only if **both** conditions pass:

```
total_events < global_max_logs
AND (no per-type cap OR type_count < type_cap)
```

---

## Consequences

**Positive:**
- Prevents state bloat from any single event type without requiring per-type configuration upfront.
- Gives operators fine-grained control over high-volume types (e.g., capping `payment` at 500k while leaving `audit` uncapped).
- Caps are opt-in so integrators can start without configuring every type.

**Negative:**
- Two cap checks on every `log_event` call adds a small amount of storage I/O.
- Cap removal is one-way per session (re-adding requires going through `set_event_max_logs` again); this is intentional to prevent accidental re-capping.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|-------------|----------------|
| Global cap only | Cannot isolate a runaway event type without pausing the whole contract. |
| Per-type cap only | No backstop for total state size if many types are created. |
| No caps | Unacceptable DoS risk in a public contract. |
| Fee-based rate limiting | Fees are controlled by the Stellar network, not the contract; insufficient on their own. |
