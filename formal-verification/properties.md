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

### P-21 — SupTech Feed Dispatcher Consistency Under Partial Failures

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/suptech_feeds.rs`, every dispatched regulatory telemetry batch is applied atomically: after any partial failure or mid-batch rollback, the dispatcher state (per-feed sequence numbers, pending queue, and committed cursor) is either fully advanced to the pre-batch state or fully advanced to the post-batch state — never a mixture. No feed may observe a committed cursor ahead of its own sequence number. |
| **CVL Rule** | `suptechFeedAtomicBatchApply`, `suptechFeedCursorNeverAheadOfSequence` |
| **K Claim** | `SUPTECH-1`, `SUPTECH-2` |
| **Status** | 🔲 Pending |

---

### P-22 — SupTech Feed Reordered / Duplicate Event Idempotence

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/suptech_feeds.rs`, duplicate submissions and out-of-order timestamps never corrupt feed state: a re-delivered event with an already-seen `(feed_id, sequence)` is rejected or ignored without mutating state, and events arriving out of timestamp order are either buffered or rejected but never cause the committed sequence to move backwards. |
| **CVL Rule** | `suptechFeedDuplicateRejected`, `suptechFeedSequenceMonotone` |
| **K Claim** | `SUPTECH-3`, `SUPTECH-4` |
| **Status** | 🔲 Pending |

---

### P-23 — SupTech Feed Merkle Proof and Hash-Chain Integrity After Recovery

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/suptech_feeds.rs`, after any simulated recovery (crash, restart, or replayed batch), every committed feed entry's Merkle proof verifies against the stored root and the hash chain linking consecutive entries remains intact (`entry[n].prev_hash == entry[n-1].entry_hash`). Recovery never produces a root that fails verification for a committed entry. |
| **CVL Rule** | `suptechFeedMerkleProofValid`, `suptechFeedHashChainIntactAfterRecovery` |
| **K Claim** | `SUPTECH-5`, `SUPTECH-6` |
| **Status** | 🔲 Pending |

---

### P-24 — SupTech Feed Concurrency Safety Under Multi-Actor Access

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/suptech_feeds.rs`, concurrent dispatches from multiple actors (RBAC-checked per `src/rbac.rs`) are serializable: the resulting committed state is equivalent to some sequential ordering of the accepted operations, and no interleaving loses an accepted event or double-applies a rejected one. |
| **CVL Rule** | `suptechFeedSerializableUnderConcurrency` |
| **K Claim** | `SUPTECH-7` |
| **Status** | 🔲 Pending |

---

### P-25 — CBDC Offline Voucher Double-Spend Reconciliation Consistency

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/cbdc_offline.rs`, the offline voucher reconciliation verifier never accepts two distinct vouchers that redeem the same underlying value: after reconciliation, the set of settled voucher identifiers is duplicate-free, and any voucher presented twice is flagged as a double-spend rather than settled twice. |
| **CVL Rule** | `cbdcOfflineNoDoubleSpend`, `cbdcOfflineSettledSetDuplicateFree` |
| **K Claim** | `CBDC-1`, `CBDC-2` |
| **Status** | 🔲 Pending |

---

### P-26 — CBDC Offline Voucher Reconciliation Atomicity Under Partial Failures

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/cbdc_offline.rs`, reconciliation of a batch of offline vouchers is atomic: a mid-batch rollback leaves the settled set, spent-value counters, and reconciliation cursor exactly as they were before the batch. Partial application of a batch is never observable. |
| **CVL Rule** | `cbdcOfflineReconcileAtomic`, `cbdcOfflineNoPartialBatchState` |
| **K Claim** | `CBDC-3`, `CBDC-4` |
| **Status** | 🔲 Pending |

---

### P-27 — CBDC Offline Voucher Reordered / Duplicate Submission Idempotence

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/cbdc_offline.rs`, duplicate voucher submissions and out-of-order timestamps are idempotent: re-submitting a voucher already present in the settled set does not change the settled set or spent-value counters, and out-of-order arrivals never cause a voucher to be settled before its dependencies are satisfied. |
| **CVL Rule** | `cbdcOfflineDuplicateIdempotent`, `cbdcOfflineOrderIndependentSettlement` |
| **K Claim** | `CBDC-5`, `CBDC-6` |
| **Status** | 🔲 Pending |

---

### P-28 — CBDC Offline Voucher Merkle Proof and Hash-Chain Integrity After Recovery

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/cbdc_offline.rs`, after any simulated recovery, every settled voucher's Merkle proof verifies against the stored reconciliation root and the hash chain over settled vouchers remains intact. Recovery never invalidates a previously verified proof for a settled voucher. |
| **CVL Rule** | `cbdcOfflineMerkleProofValid`, `cbdcOfflineHashChainIntactAfterRecovery` |
| **K Claim** | `CBDC-7`, `CBDC-8` |
| **Status** | 🔲 Pending |

---

### P-29 — CBDC Offline Voucher Concurrency Safety Under Multi-Actor Access

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For `src/cbdc_offline.rs`, concurrent voucher submissions from multiple actors (RBAC-checked per `src/rbac.rs`) are serializable with respect to the settled set and spent-value counters: the outcome is equivalent to some sequential ordering, and no interleaving permits a double-spend that a sequential execution would have rejected. |
| **CVL Rule** | `cbdcOfflineSerializableUnderConcurrency` |
| **K Claim** | `CBDC-9` |
| **Status** | 🔲 Pending |

---

## Supply Chain Custody Properties (`src/supply_chain.rs`)

The `SupplyChain` module tracks multi-tier custody handoffs for physical goods.
Each custody record is a `CustodyEvent` appended to a per-item hash chain, and
each batch of handoffs is committed with a Merkle root over the batch leaves.
The properties below define the state-machine invariants that the property-based
and concurrency test suites (`src/proptest.rs`, `src/concurrent_tests.rs`) must
uphold under duplicate submissions, out-of-order timestamps, mid-batch
rollbacks, and concurrent multi-actor access.

### SC-01 — Custody Chain Continuity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For every item, the custody chain is a single unbroken sequence. Each `CustodyEvent` at position `n > 0` has `prev_hash == events[n-1].event_hash`, and the genesis custody event has `prev_hash == [0x00; 32]`. No handoff may reference a predecessor that was never committed, and no committed event may be orphaned by a later rollback. |
| **CVL Rule** | `custodyChainContinuity` |
| **K Claim** | `SC-SPEC-1` |
| **Status** | 🔶 Partial (bounded depth; property tests cover 1,000+ sequences) |

---

### SC-02 — Custody Timestamp Monotonicity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Within a single item's custody chain, timestamps are strictly non-decreasing: for all `n > 0`, `events[n].timestamp >= events[n-1].timestamp`. Out-of-order submissions are rejected with `TimestampOutOfOrder` and never mutate stored state. Duplicate submissions of an already-committed handoff are idempotent and leave the chain, counters, and Merkle root unchanged. |
| **CVL Rule** | `custodyTimestampMonotone`, `duplicateHandoffIdempotent` |
| **K Claim** | `SC-SPEC-2` |
| **Status** | 🔶 Partial (bounded depth; property tests cover 1,000+ sequences) |

---

### SC-03 — Certification Validity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | A custody handoff is only accepted when the receiving actor holds a valid, unexpired certification for the item's tier. An expired or revoked certification causes the handoff to revert with `CertificationInvalid` and leaves the chain unchanged. Certification validity is checked against the RBAC/governance state in `src/rbac.rs` and cannot be bypassed by a concurrent writer. |
| **CVL Rule** | `certificationValidityEnforced` |
| **K Claim** | `SC-SPEC-3` |
| **Status** | 🔶 Partial (bounded depth; RBAC integration pending K rules) |

---

### SC-04 — Merkle Root and Hash-Chain Integrity After Recovery

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | After any simulated recovery — including mid-batch rollbacks, replayed batches, and reordered event delivery — the recomputed Merkle root over the committed batch leaves equals the stored root, and every Merkle inclusion proof for a committed leaf verifies against that root. The per-item hash chain remains intact (see SC-01). A partially applied batch must never leave a stored root that does not match its leaves. |
| **CVL Rule** | `merkleRootMatchesLeaves`, `merkleProofVerifiesAfterRecovery` |
| **K Claim** | `SC-SPEC-4` |
| **Status** | 🔶 Partial (bounded depth; property tests cover 1,000+ sequences) |

---

## ML Feature Store Properties (`src/ml_feature_store.rs`)

These properties cover the on-chain ML feature vector lineage and hash-anchoring
module. They are exercised by the property-based suite in `src/proptest.rs`, the
multi-actor concurrency suite in `src/concurrent_tests.rs`, and the
fault-injection scenarios described below. Each property is stated as a state
machine invariant over the abstract state
`(features, anchors, merkle_roots, nonces, total_features)`.

### MFS-01 — Feature Registration Idempotence Under Duplicate Submission

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Submitting the same feature vector (identical `feature_id` and `feature_hash`) more than once must not create a second stored entry nor advance `total_features` a second time. The duplicate submission either reverts or is a no-op; in both cases the post-state equals the pre-state. |
| **Invariant** | `register(fv); register(fv) ⇒ state_after == state_after_first_register` |
| **Fault Scenario** | Duplicate submissions (same actor, repeated call; distinct actors, identical payload) |
| **CVL Rule** | `featureRegistrationIdempotent` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-02 — Monotone Feature Counter

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | `total_features` is a monotone non-decreasing counter. No operation, including rollback of a partially applied batch, may decrease it. Each accepted, non-duplicate registration increments it by exactly 1. |
| **Invariant** | `total_features' >= total_features ∧ (accepted ∧ ¬duplicate ⇒ total_features' == total_features + 1)` |
| **Fault Scenario** | Mid-batch rollback, duplicate submissions |
| **CVL Rule** | `totalFeaturesMonotone` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-03 — Out-of-Order Timestamp Rejection

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | A feature vector whose `timestamp` is strictly less than the timestamp of the most recently accepted vector for the same lineage must be rejected. Reordered events therefore cannot rewrite history; the accepted sequence of timestamps per lineage is non-decreasing. |
| **Invariant** | `∀ lineage: timestamps(lineage) is non-decreasing` |
| **Fault Scenario** | Out-of-order timestamps (shuffled event delivery, clock skew) |
| **CVL Rule** | `timestampMonotonePerLineage` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-04 — Merkle Root Determinism and Proof Integrity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For a fixed set of anchored feature hashes, the computed Merkle root is deterministic and independent of insertion order. Every generated Merkle proof verifies against the stored root, and any single-bit mutation of a leaf or of the root causes verification to fail. |
| **Invariant** | `root(perm(leaves)) == root(leaves) ∧ verify(proof(leaf), root) ∧ ¬verify(proof(leaf'), root) for leaf' ≠ leaf` |
| **Fault Scenario** | Merkle proof and hash-chain integrity after simulated recovery |
| **CVL Rule** | `merkleRootDeterministic`, `merkleProofSound` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-05 — Hash-Chain Linkage Across Anchors

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For every anchor at position `n > 0`, `anchors[n].prev_hash == anchors[n-1].anchor_hash`. The genesis anchor has `prev_hash == [0x00; 32]`. After simulated recovery (replay of the accepted prefix), the recomputed chain is byte-for-byte identical to the stored chain. |
| **Invariant** | `∀ n > 0: anchors[n].prev_hash == anchors[n-1].anchor_hash ∧ recovery(prefix) == stored_chain` |
| **Fault Scenario** | Mid-batch rollback, hash-chain integrity after simulated recovery |
| **CVL Rule** | `anchorHashChainIntegrity`, `recoveryPreservesChain` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-06 — Mid-Batch Rollback Atomicity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | If a batch registration fails partway through (e.g. one element violates MFS-03 or a cap), the entire batch is rolled back: no feature, anchor, Merkle root, or nonce from that batch is persisted. The post-state equals the pre-state of the batch. |
| **Invariant** | `batch_fails ⇒ state_after == state_before_batch` |
| **Fault Scenario** | Mid-batch rollbacks (injected failure at every position of the batch) |
| **CVL Rule** | `batchRollbackAtomic` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-07 — Nonce Strictly Monotone Per Submitter

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | For each submitter address, the stored nonce strictly increases with every accepted registration. Replay of a previously accepted submission under the same nonce is rejected. |
| **Invariant** | `∀ addr: nonces(addr) strictly increases per accepted registration` |
| **Fault Scenario** | Duplicate submissions, reordered events |
| **CVL Rule** | `featureNonceStrictlyMonotone` |
| **K Claim** | — (pending) |
| **Status** | 🔲 Pending |

---

### MFS-08 — Concurrent Access Linearizability

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Under concurrent access by multiple actors, the observed final state is equivalent to some sequential ordering of the accepted operations. No interleaving may produce a state that violates MFS-01 through MFS-07. |
| **Invariant** | `∃ sequential order σ: concurrent_execution ≡ σ` |
| **Fault Scenario** | Multi-actor concurrent access (`src/concurrent_tests.rs`) |
| **CVL Rule** | — (in-Rust concurrency suite; formal spec pending) |
| **K Claim** | — |
| **Status** | 🔲 Pending |

---

## VAT Determination Engine Properties (`src/vat_engine.rs`)

`VATDeterminationEngine` is stateless; the test suite uses a test-only journal
model to check deterministic acceptance, batch atomicity, and recovery around
the public determination API. The abstract state is
`(records, transaction_ids, last_timestamp, hash_chain, merkle_root)`.
The journal rules below describe the test harness and do not imply that the
production engine persists submissions.

### VAT-01 — Bounded and Deterministic Determinations

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Every accepted transaction satisfies `supply_amount <= MAX_SUPPLY_AMOUNT` and `vat_rate <= MAX_VAT_RATE_BPS`. The jurisdiction, supply type, exemption, and reverse-charge rules produce a deterministic rate and amount; the amount is zero for exemptions and applicable reverse charges, and every calculated rate is at most 10,000 basis points. |
| **Invariant** | `accepted(t) ⇒ validate_transaction(t) ∧ result.vat_rate <= 10_000 ∧ (result.is_exempt ∨ result.reverse_charge_applicable ⇒ result.vat_amount == 0)` |
| **Fault Scenario** | Boundary amounts/rates and all jurisdiction/supply/exemption rule combinations |
| **Status** | 🔶 Covered by deterministic boundary matrix and property sequences |

---

### VAT-02 — Duplicate and Timestamp Rejection

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | In the test journal, a transaction ID is committed at most once and accepted timestamps are non-decreasing. Duplicate IDs and strictly older timestamps are rejected without changing the state. |
| **Invariant** | `duplicate(id) ∨ timestamp < last_timestamp ⇒ state_after == state_before` |
| **Fault Scenario** | Duplicate submissions and out-of-order timestamps |
| **Status** | 🔶 Covered by generated sequences and regression tests |

---

### VAT-03 — Mid-Batch Rollback Atomicity

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | An injected failure at any batch position restores the exact pre-batch journal state, including accepted IDs, determinations, timestamps, hash links, and Merkle leaves. |
| **Invariant** | `batch_fails_at(k) ⇒ state_after == state_before` for every `0 <= k < batch.len()` |
| **Fault Scenario** | Mid-batch failures at every position |
| **Status** | 🔶 Covered by deterministic fault injection |

---

### VAT-04 — Hash-Chain and Merkle Integrity After Recovery

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Each committed journal hash links the prior hash and current determination leaf. A recovery replay reproduces the same accepted IDs and hash chain; all Merkle proofs verify against the recomputed root, and a mutated leaf fails verification. |
| **Invariant** | `chain[n] == H(0x02 || chain[n-1] || leaf[n]) ∧ replay(records) == state ∧ verify(proof(leaf[n]), root)` |
| **Fault Scenario** | Replay after rollback, reordered submissions, and tampered Merkle leaves |
| **Status** | 🔶 Covered by generated sequences and recovery checks |

---

### VAT-05 — Multi-Actor Serializable Interleaving

| Field | Value |
|-------|-------|
| **Category** | safety |
| **Description** | Interleaved submissions from multiple actors are equivalent to their accepted sequential order. Rejected duplicates do not change state, and recovery preserves the hash chain and Merkle commitments. |
| **Invariant** | `interleaved(records) == sequential(accepted(records))` |
| **Fault Scenario** | Concurrent multi-actor access (`src/concurrent_tests.rs`) |
| **Status** | 🔶 Covered by deterministic interleaving test |

---

## Fault-Injection Scenarios

The following fault-injection scenarios are simulated by the test suite and must
leave the applicable MFS and VAT invariants intact:

| ID | Scenario | Injection Point | Expected Outcome |
|----|----------|-----------------|------------------|
| FI-01 | Duplicate submission (same actor) | Repeated `register` with identical payload | Rejected or no-op; state unchanged (MFS-01) |
| FI-02 | Duplicate submission (distinct actors) | Two actors submit identical payload | Exactly one accepted; counter advances once (MFS-01, MFS-07) |
| FI-03 | Out-of-order timestamps | Shuffled delivery of a valid sequence | Reordered events rejected; accepted timestamps non-decreasing (MFS-03) |
| FI-04 | Mid-batch rollback | Failure injected at position `k` of an `n`-element batch, for all `k` | Full rollback; post-state == pre-state (MFS-06) |
| FI-05 | Recovery replay | Replay accepted prefix after simulated crash | Recomputed Merkle root and hash chain identical to stored (MFS-04, MFS-05) |
| FI-06 | Leaf tampering | Single-bit mutation of an anchored leaf | Merkle proof verification fails (MFS-04) |
| FI-07 | Concurrent interleaving | Randomized interleaving of multi-actor operations | Final state linearizable; no invariant violation (MFS-08) |
| FI-08 | VAT duplicate or reordered submission | Repeated ID or older transaction timestamp | Rejected without changing journal state (VAT-02) |
| FI-09 | VAT batch rollback | Injected failure at each batch position | Full journal state, chain, and Merkle leaves restored (VAT-03) |
| FI-10 | VAT recovery and tampering | Replay accepted VAT records, then mutate a leaf | Chain and root reproduced; mutated leaf proof rejected (VAT-04) |
| FI-11 | VAT multi-actor interleaving | Rotate submissions among multiple actors | Accepted state remains serializable and recoverable (VAT-05) |

---

## Verification Coverage Summary

| Category | Total | Verified ✅ | Partial 🔶 | Pending 🔲 |
|----------|-------|------------|-----------|-----------|
| safety | 32 | 9 | 13 | 10 |
| access-control | 5 | 5 | 0 | 0 |
| liveness | 1 | 0 | 1 | 0 |
| ml-feature-store | 8 | 0 | 0 | 8 |
| **Total** | **46** | **14** | **14** | **18** |

---

## Adding New Properties

1. Write a plain-English description and add a row to the table above.
2. Add a CVL rule to `formal-verification/audit_ledger.spec`.
3. Add a K claim to `formal-verification/k-spec/audit_ledger-spec.k`.
4. Run verification (see `formal-verification/README.md`).
5. Update the **Status** field and coverage table.
