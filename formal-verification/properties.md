# AuditLedger Formally Verified Properties

This document catalogs every property formally specified and/or verified for the
`AuditLedger` Soroban smart contract. Each entry lists the property name,
category, a plain-English description, the corresponding CVL rule name, the
K-framework claim label, and the current verification status.

---

## Legend

| Field | Meaning |
|-------|---------|
| **Category** | `safety` — bad things never happen; `liveness` — good things eventually happen; `access-control` — privilege enforcement |
| **CVL Rule** | Name of the Certora rule in `audit_ledger.spec` |
| **K Claim** | Label of the K-framework claim in `audit_ledger-spec.k` |
| **Status** | ✅ Verified · 🔶 Partial (bounded) · ❌ Counterexample found · 🔲 Pending |

---

## Properties Table

### P-01 — Event Immutability

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Once an event is stored on-chain, its `index`, `timestamp`, `event_type`, `submitter`, `event_hash`, and `prev_hash` fields cannot be changed by any subsequent transaction. Events are append-only; there is no update or delete path. |
| **CVL Rule** | `eventImmutability` |
| **K Claim** | — (structural property enforced by write-once storage) |
| **Status** | ✅ Verified |

---

### P-02 — Total Events Counter Monotonically Increases

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | The `totalEvents` counter is a monotone non-decreasing integer. No operation may decrease it. Each successful `logEvent` increments it by exactly 1. |
| **CVL Rule** | `totalEventsMonotonicallyIncreases`, `logEventIncrementsByOne` |
| **K Claim** | `SPEC-2` |
| **Status** | ✅ Verified |

---

### P-03 — Global Cap Enforcement

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `totalEvents` never exceeds `globalMaxLogs`. `logEvent` reverts with `GlobalMaxLogsReached` (error 2) when `totalEvents >= globalMaxLogs`. |
| **CVL Rule** | `globalCapRespected`, `capEnforcedAfterLogEvent`, `logEventRevertsAtCap` |
| **K Claim** | `SPEC-3` |
| **Status** | ✅ Verified |

---

### P-04 — Per-Event-Type Cap Enforcement

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | When a per-event-type cap is configured via `setEventMaxLogs`, `logEvent` for that type reverts with `EventTypeMaxLogsReached` (error 3) when the type's count equals its cap. |
| **CVL Rule** | — (K rule LOG-5, CVL rule pending) |
| **K Claim** | K rules LOG-4, LOG-5 |
| **Status** | 🔶 Partial |

---

### P-05 — Owner-Only: setGlobalMaxLogs

| Field | Value |
|-------|-------|
| **Category** | access-control |
| **Description** | `setGlobalMaxLogs` succeeds only when `caller == owner`. Any other caller receives `CallerNotOwner` (error 1). |
| **CVL Rule** | `setGlobalMaxLogsOnlyOwner` |
| **K Claim** | `SPEC-5`, `SPEC-6` |
| **Status** | ✅ Verified |

---

### P-06 — Owner-Only: setEventMaxLogs

| Field | Value |
|-------|-------|
| **Category** | access-control |
| **Description** | `setEventMaxLogs` succeeds only when `caller == owner`. Any other caller receives `CallerNotOwner` (error 1). |
| **CVL Rule** | `setEventMaxLogsOnlyOwner` |
| **K Claim** | K rule GOV-5 |
| **Status** | ✅ Verified |

---

### P-07 — Owner-Only: removeEventCap

| Field | Value |
|-------|-------|
| **Category** | access-control |
| **Description** | `removeEventCap` succeeds only when `caller == owner`. Any other caller receives `CallerNotOwner` (error 1). |
| **CVL Rule** | `removeEventCapOnlyOwner` |
| **K Claim** | K rule GOV-8 |
| **Status** | ✅ Verified |

---

### P-08 — Owner-Only: transferOwnership

| Field | Value |
|-------|-------|
| **Category** | access-control |
| **Description** | `transferOwnership` succeeds only when `caller == owner`. Any other caller receives `CallerNotOwner` (error 1). |
| **CVL Rule** | `transferOwnershipOnlyOwner` |
| **K Claim** | K rule GOV-10 |
| **Status** | ✅ Verified |

---

### P-09 — Owner-Only: setPaused

| Field | Value |
|-------|-------|
| **Category** | access-control |
| **Description** | `setPaused` succeeds only when `caller == owner`. Any other caller receives `CallerNotOwner` (error 1). |
| **CVL Rule** | `setPausedOnlyOwner` |
| **K Claim** | K rule GOV-14 |
| **Status** | ✅ Verified |

---

### P-10 — Paused Contract Prevents Event Logging

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | When `paused == true`, both `logEvent` and `logEvents` revert with `ContractPaused` (error 13). No events can be written to the ledger while paused. |
| **CVL Rule** | `pausedPreventsLogging`, `pausedPreventsBatchLogging` |
| **K Claim** | `SPEC-4`, K rule LOG-3 |
| **Status** | ✅ Verified |

---

### P-11 — Initialization Idempotence

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `initialize` may only succeed once. Any subsequent call reverts with `AlreadyInitialized` (error 30). The owner and cap are set exactly once at deployment. |
| **CVL Rule** | `initializeIdempotent`, `initializeSetsInitializedFlag`, `initializeSetsOwner` |
| **K Claim** | `SPEC-1`, K rule INI-2 |
| **Status** | ✅ Verified |

---

### P-12 — Zero-Address Owner Rejection

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `initialize` and `transferOwnership` both reject a zero/null address as the new owner, reverting with `NewOwnerIsZero` (error 6). |
| **CVL Rule** | `ownerIsNeverZero` (invariant) |
| **K Claim** | K rules INI-3, GOV-11 |
| **Status** | ✅ Verified |

---

### P-13 — Ownership Transfer Correctness

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | After a successful `transferOwnership(caller, newOwner)`, the stored owner is exactly `newOwner`. The previous owner has no residual privileges. |
| **CVL Rule** | `ownershipTransferCorrectness` |
| **K Claim** | `SPEC-7` |
| **Status** | ✅ Verified |

---

### P-14 — Hash Chain Integrity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For every event at position `n > 0`, `events[n].prev_hash == events[n-1].event_hash`. The genesis event (position 0) has `prev_hash == [0x00; 32]`. This creates a tamper-evident chain of all logged events. |
| **CVL Rule** | `hashChainIntegrity`, `genesisEventPrevHashIsZero` |
| **K Claim** | — (enforced by hash computation in LOG-1) |
| **Status** | 🔶 Partial (bounded depth) |

---

### P-15 — Nonce Strictly Monotone Per Submitter

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For each submitter address, the nonce stored at `SubmitterNonce(addr)` strictly increases with every accepted event. This prevents replay attacks where the same event data is submitted twice. |
| **CVL Rule** | `nonceStrictlyMonotonePerSubmitter` |
| **K Claim** | — (nonce tracking pending K integration) |
| **Status** | 🔶 Partial |

---

### P-16 — setGlobalMaxLogs Rejects Values Below Current Count

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `setGlobalMaxLogs(caller, newMax)` reverts with `MaxLogsBelowCurrentCount` (error 16) when `newMax < totalEvents`. The cap cannot be set so low as to make already-stored events retroactively invalid. |
| **CVL Rule** | — (pending) |
| **K Claim** | K rule GOV-3 |
| **Status** | 🔶 Partial |

---

### P-17 — Cap Removal Idempotence

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `removeEventCap` reverts with `CapNotSet` (error 7) when called on an event type that has no active cap. It succeeds exactly once per event type until a new cap is configured via `setEventMaxLogs`. |
| **CVL Rule** | — (pending) |
| **K Claim** | K rule GOV-6, GOV-7 |
| **Status** | 🔶 Partial |

---

### P-18 — Contract Uninitialized Prevents All Writes

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Before `initialize` is called, `logEvent`, `logEvents`, and all governance functions revert. |
| **CVL Rule** | `contractMustBeInitialized` (invariant) |
| **K Claim** | K rule LOG-2 |
| **Status** | ✅ Verified |

---

### P-19 — Same-Owner Transfer Rejected

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `transferOwnership(owner, owner)` reverts with `SameOwner` (error 15). |
| **CVL Rule** | — (pending) |
| **K Claim** | K rule GOV-12 |
| **Status** | 🔶 Partial |

---

### P-20 — Inverted Index Count Consistent with Stored Bytes

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For every `IndexKey`, `index_get_count(key) == len(packed_bytes) / 4`. The count never exceeds `INDEX_MAX_ENTRIES`. |
| **CVL Rule** | — (in-Rust test coverage; formal spec pending) |
| **K Claim** | — |
| **Status** | 🔲 Pending |

---

## Verification Coverage Summary

| Category | Total | Verified ✅ | Partial 🔶 | Pending 🔲 |
|----------|-------|------------|-----------|-----------|
| safety | 14 | 9 | 4 | 1 |
| access-control | 5 | 5 | 0 | 0 |
| liveness | 1 | 0 | 1 | 0 |
| **Total** | **20** | **14** | **5** | **1** |

---

## Adding New Properties

1. Write a plain-English description and add a row to the table above.
2. Add a CVL rule to `formal-verification/audit_ledger.spec`.
3. Add a K claim to `formal-verification/k-spec/audit_ledger-spec.k`.
4. Run verification (see `formal-verification/README.md`).
5. Update the **Status** field and coverage table.
