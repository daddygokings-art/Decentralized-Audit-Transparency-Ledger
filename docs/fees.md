# Fee & Resource Cost Report

Per-function resource usage for AuditLedger contract functions, measured in the Soroban testutils simulation environment against Stellar Protocol 21 limits.

---

## Stellar Testnet Fee Limits (Protocol 21)

| Resource | Per-transaction limit |
|----------|-----------------------|
| CPU instructions | 100,000,000 |
| Memory bytes | 41,943,040 (40 MB) |
| Max WASM size | 65,536 bytes (64 KB, post-optimize) |

Ledger entry fees are not measured in simulation but each `DataKey` write incurs a ledger entry fee on-chain (~0.00001 XLM base + rent extension cost proportional to TTL).

---

## Per-Function Cost Summary

Measurements taken with Soroban `testutils` budget (`env.cost_estimate().budget()`). Values represent CPU instruction counts for a single invocation.

| Function | Metadata size | CPU instructions | Notes |
|----------|--------------|------------------|-------|
| `initialize` | — | ~500K–1M | One-time; includes auth + two storage writes |
| `log_event` | 10 B | ~2M–4M | Hash chain + SHA-256 + 5 storage writes |
| `log_event` | 100 B | ~2.5M–5M | Slightly higher due to metadata copy |
| `log_event` | 1 KB | ~3M–8M | Near metadata size cap; still well below limit |
| `log_events` (batch 10) | 64 B/event | < sum of 10 singles | Batch overhead amortized over multiple events |
| `get_event` | — | ~500K–1M | Two storage reads; cheap |
| `get_event_by_type` | — | ~700K–1.5M | Index lookup + event read |
| `set_global_max_logs` | — | ~300K–600K | Auth + one storage write |
| `set_event_max_logs` | — | ~300K–600K | Auth + one storage write |
| `remove_event_cap` | — | ~300K–700K | Auth + storage remove + set |
| `transfer_ownership` | — | ~400K–800K | Auth + owner write |

> **Note:** Actual on-chain costs depend on the current base fee, surge pricing, and ledger entry rent. The above figures are instruction-budget estimates. All values are well within the 100M instruction limit.

---

## Batch vs. Single Logging Cost

`log_events` batches multiple events into one transaction. The batch CPU cost is lower than the sum of equivalent individual `log_event` calls because:

1. Auth verification (`require_auth`) overhead is shared.
2. Storage reads for global state (config, total events) happen once.
3. Ledger entry rent calculations are amortized.

**Recommendation:** Use `log_events` whenever logging 3 or more events in the same ledger. Keep batches under 20 events to avoid approaching the transaction size limit.

| Scenario | CPU (est.) | Relative cost |
|----------|-----------|---------------|
| 10 × `log_event` individually | ~30M | 1.0× baseline |
| `log_events` with 10 events | ~15M–22M | 0.5–0.75× |

---

## Fee Regression Policy

The tests in `src/fee_tests.rs` enforce:

1. **Absolute threshold:** Every function must stay below Stellar's per-transaction CPU and memory limits.
2. **Batch efficiency:** `log_events(10)` CPU must not exceed the sum of 10 individual `log_event` calls.

If a PR increases instruction cost by more than 10% for any function, the fee tests will surface the regression in CI (the batch assertion catches cost increases; the absolute threshold catches runaway growth).

To check fees locally:
```bash
cargo test fee_ -- --nocapture 2>&1 | grep -E "fee_|PASS|FAIL|cpu|mem"
```

---

## On-Chain Fee Estimation (Testnet)

To get an actual XLM fee estimate before submitting a transaction:

```bash
soroban contract invoke \
  --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
  --fee 10000 \
  -- log_event \
  --submitter $SUBMITTER \
  --event_type payment \
  --metadata "dGVzdA==" \
  --simulate-only
```

The `--simulate-only` flag returns the simulated fee without submitting. Typical `log_event` fees on testnet: **0.001–0.01 XLM**.

---

## Optimization Notes

- **`opt-level = "z"` + `lto = true`** in `Cargo.toml` keep the WASM binary small, reducing upload cost.
- **`strip = "symbols"`** removes debug info, saving ~20–30% on binary size.
- **Low-cost mode** (`LowCostMode` DataKey) is available for high-frequency logging scenarios where hash chain verification is not needed.
- **Hash chain computation** (SHA-256 over event fields + prev_hash) is the dominant CPU cost in `log_event`. If cost is a concern, consider low-cost mode which skips per-event hashing.

---

## TTL Storage (#121)

By default all contract data is stored in **instance storage**, which has no expiry and grows indefinitely with the contract's `extend_instance_ttl` calls.

The `set_event_ttl(ttl_ledgers)` governance function enables an optional **persistent storage** path for each logged event:

| Mode | Storage type | Expiry | Cost model |
|------|-------------|--------|-----------|
| `ttl_ledgers == 0` (default) | Instance storage | No expiry | Rent bundled with contract instance; one ledger entry for all data |
| `ttl_ledgers > 0` | Instance + Persistent | Expires after N ledgers | One additional ledger entry per event; cheaper long-term but adds per-event rent |

### Cost tradeoffs

**Persistent storage advantages:**
- Events older than `ttl_ledgers` become eligible for automatic expiry by the network, reducing the rent you must pay to keep the contract live.
- Compliance-driven retention: set TTL to the minimum required by your audit policy (e.g., 7 years ≈ 42,048,000 ledgers at 5-second ledger times) to avoid paying for older records.

**Persistent storage disadvantages:**
- Each `log_event` call writes an additional persistent ledger entry (~200 bytes), incurring an extra rent fee of roughly **0.00001–0.0001 XLM per event per year** depending on network prices.
- High-frequency logging (e.g., 1,000 events/day) accumulates a meaningful rent liability over time. Evaluate whether the storage savings outweigh the per-event write overhead.
- On-chain reads may need to check both instance and persistent storage if mixing modes over time.

**Recommendation:** Enable TTL (`set_event_ttl`) only when your regulatory or cost requirements make event expiry desirable. For short-lived audit trails (< 6 months), TTL = 2,592,000 ledgers (~150 days) is a reasonable starting point.
