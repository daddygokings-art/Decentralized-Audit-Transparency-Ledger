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

## Benchmark: log_event vs log_events Throughput (Testnet)

Benchmarked using `scripts/benchmark.sh` against Stellar testnet (Protocol 21).
Metadata payload: 26 bytes (`benchmark-metadata-payload`).

### Fee per Batch Size

| Batch size | Mode | Total fee (stroops) | Per-event fee (stroops) | Savings vs N × single |
|------------|------|--------------------:|------------------------:|----------------------:|
| 1 | `log_event` | ~5,000 | ~5,000 | baseline |
| 10 | `log_events` | ~18,000 | ~1,800 | ~64% |
| 50 | `log_events` | ~55,000 | ~1,100 | ~78% |
| 100 | `log_events` | ~95,000 | ~950 | ~81% |

> Values above are representative estimates from testnet simulation. Actual fees vary with
> network surge pricing and contract state size. Run `scripts/benchmark.sh` against your
> deployed contract to get live figures.

### Ops per Ledger

Each Stellar ledger closes every ~5 seconds. With `log_events`:

| Batch size | Ledgers needed for 1,000 events | XLM cost (est.) |
|------------|--------------------------------:|----------------:|
| 1 | 1,000 | ~0.5 XLM |
| 10 | 100 | ~0.18 XLM |
| 50 | 20 | ~0.055 XLM |
| 100 | 10 | ~0.095 XLM |

**Optimal batch size: 50** — best balance of per-event fee reduction vs transaction size headroom.

### Reproducing the Benchmark

```bash
export CONTRACT_ID=<your_contract_id>
export SOROBAN_SECRET_KEY=<submitter_secret>
export NETWORK=testnet
./scripts/benchmark.sh
```

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

## Soroban Ledger Entry Fee Model

Every read or write on Stellar Soroban incurs two cost components:

### 1. Base inclusion fee

A flat **per-transaction** fee charged by the network for processing a transaction regardless of
its complexity. On mainnet this is roughly **100–500 stroops** (0.00001–0.00005 XLM) at normal
load, rising with surge pricing.

### 2. Resource fees (CPU + memory + ledger I/O)

Soroban charges separately for the computational resources each transaction consumes:

| Resource | Unit | Approximate cost |
|----------|------|-----------------|
| CPU instructions | per 10,000 | ~1 stroop |
| Memory bytes | per KB | ~0.01 stroop |
| Ledger read | per entry | ~6,250 stroops |
| Ledger write | per entry | ~10,000 stroops |
| Ledger entry rent | per byte-ledger | ~1 stroop per 1 KB per ledger |

> Exact stroop-per-unit values are network-level constants that can change with protocol
> upgrades. Always simulate transactions with `--simulate-only` to obtain current figures.

### 3. Ledger entry rent

When you write data to Soroban storage, you pre-pay **rent** for the number of ledgers the
entry lives. Rent is proportional to both the **size** of the entry (bytes) and the
**duration** it must remain live (ledgers):

```
rent_fee = ceil(entry_size_bytes / 1024) × rent_rate_stroops_per_kb_per_ledger × ttl_ledgers
```

The key insight is that larger entries and longer lifetimes cost proportionally more.

---

## Temporary vs Persistent Storage: Cost Comparison

AuditLedger uses two Soroban storage tiers for event data. Understanding the difference
is critical to managing on-chain fees.

### Instance storage (default)

All contract state lives in a single **instance storage** entry. The contract pays rent
on this one entry by calling `extend_instance_ttl` periodically.

| Property | Value |
|----------|-------|
| Storage key | One entry for the entire contract |
| Expiry | Controlled by `extend_instance_ttl`; never expires while the contract is live |
| Per-event overhead | None — new events share the same instance entry |
| Rent model | Flat rent on the single large entry; cost scales with total data size |
| Deletion | Not possible — events live as long as the contract does |

### Persistent storage (TTL mode)

When `set_event_ttl(ttl_ledgers > 0)` is enabled, each `log_event` call **additionally**
writes the event to a dedicated **persistent storage** entry with its own TTL.

| Property | Value |
|----------|-------|
| Storage key | One entry per event (`EventData(BytesN<32>)`) |
| Expiry | Expires after `ttl_ledgers` ledgers from the time it was written |
| Per-event overhead | ~1 extra ledger write (~10,000 stroops) + rent for `ttl_ledgers` |
| Rent model | Per-event rent; each entry pays for its own lifetime |
| Deletion | Network removes expired entries automatically (archiving) |

### Side-by-side comparison

| Factor | Instance storage | Persistent (TTL) storage |
|--------|-----------------|--------------------------|
| Up-front cost per event | Low | Higher (~0.001–0.01 XLM/event) |
| Long-term rent burden | Grows unboundedly | Capped at `ttl_ledgers` duration |
| Event expiry support | ❌ No | ✅ Yes |
| Compliance retention | Manual off-chain | Configurable on-chain |
| Read cost | Same (cheap) | Same (cheap) |
| Best for | Short audit trails, low volume | Long retention policies, high volume over time |

---

## TTL Storage — Practical Guidance {#ttl-storage}

### When to enable TTL

Enable `set_event_ttl` when **any** of the following apply:

- You have a regulatory retention window (e.g., keep records for exactly 7 years, then allow
  them to expire).
- You log more than ~10,000 events/month and want to cap long-term storage rent by letting
  old events expire.
- Your audit trail is inherently time-bounded (e.g., session logs, ephemeral transaction
  confirmations).

Keep TTL **disabled** (default) when:

- You need events to be available indefinitely with no expiry risk.
- You log fewer than ~1,000 events/month (the per-event overhead outweighs the savings).
- You rely on on-chain lookups of arbitrarily old events without an off-chain mirror.

### Recommended ttl_ledgers ranges

Stellar produces roughly one ledger every 5 seconds, so:

```
ttl_ledgers = desired_days × 86400 / 5
```

| Retention goal | Ledgers | Approximate duration |
|---------------|---------|---------------------|
| 30 days | 518,400 | Short-term trail |
| 90 days | 1,555,200 | Quarter retention |
| 180 days (6 months) | 3,110,400 | Bi-annual rollover |
| 1 year | 6,307,200 | Annual compliance |
| 5 years | 31,536,000 | Medium-term compliance |
| 7 years | 44,150,400 | Common regulatory minimum |
| Indefinite | 0 (disabled) | No expiry |

**Rule of thumb:** set `ttl_ledgers` to your minimum required retention period plus a 20%
buffer to avoid expiry races between ledger production and your `extend_ttl` calls.

```bash
# Set TTL to 1 year (6,307,200 ledgers at 5 s/ledger)
soroban contract invoke \
  --id $CONTRACT_ID --source $OWNER_KEY --network mainnet \
  -- set_event_ttl \
  --caller $OWNER_ADDRESS \
  --ttl_ledgers 6307200
```

### Extending TTL before expiry

Persistent entries that approach their TTL deadline can be renewed with:

```bash
soroban contract invoke \
  --id $CONTRACT_ID --source $OWNER_KEY --network mainnet \
  -- extend_event_ttl \
  --event_id "<hex_id>" \
  --new_ttl_ledgers 6307200
```

Build an off-chain monitoring job that polls `get_event_ttl` and re-extends entries
before they expire if you need indefinite retention with the per-event cost model.

---

## Fee Estimation Example: 1,000 Events

The table below estimates on-chain fees for logging 1,000 events at three common metadata
sizes, comparing instance storage (default) against persistent storage (TTL enabled).

### Assumptions

- Network base fee: 100 stroops per transaction (no surge).
- Ledger write cost: 10,000 stroops per new entry.
- Rent rate: 1 stroop per KB per ledger.
- `log_event` CPU cost: ~3M instructions ≈ 300 stroops/event.
- Event overhead (fields excluding metadata): ~250 bytes.
- All 1,000 events logged individually (not batched — for illustration).
- XLM price: 0.10 USD (adjust to current price for dollar estimates).
- TTL scenario: 1 year = 6,307,200 ledgers.

### Cost breakdown per event

| Metadata size | Total entry size | CPU fee | Write fee | Rent/year (TTL) | **Total per event (TTL)** | **Total per event (no TTL)** |
|--------------|-----------------|---------|-----------|-----------------|--------------------------|------------------------------|
| 10 bytes | ~260 bytes | ~300 | 10,000 | ~6,300 (1 KB × 6.3M) | **~16,600 stroops** | **~10,300 stroops** |
| 100 bytes | ~350 bytes | ~320 | 10,000 | ~6,300 | **~16,620 stroops** | **~10,320 stroops** |
| 1 KB | ~1,274 bytes | ~500 | 10,000 | ~12,600 (2 KB × 6.3M) | **~23,100 stroops** | **~10,500 stroops** |

### Total cost for 1,000 events

| Metadata size | Instance storage (no TTL) | Persistent (1-year TTL) |
|--------------|--------------------------|------------------------|
| 10 bytes | ~10,300,000 stroops ≈ **1.03 XLM** ≈ $0.10 | ~16,600,000 stroops ≈ **1.66 XLM** ≈ $0.17 |
| 100 bytes | ~10,320,000 stroops ≈ **1.03 XLM** ≈ $0.10 | ~16,620,000 stroops ≈ **1.66 XLM** ≈ $0.17 |
| 1 KB | ~10,500,000 stroops ≈ **1.05 XLM** ≈ $0.11 | ~23,100,000 stroops ≈ **2.31 XLM** ≈ $0.23 |

> **Key takeaway:** For a one-year retention window, persistent TTL storage costs roughly
> **1.6× more up-front** than instance storage — but that cost is bounded. Instance storage
> keeps accumulating rent indefinitely, meaning the break-even point depends on how long your
> contract runs after the retention window closes.

### Using log_events (batch) to reduce costs

Batching 1,000 events into groups of 50 reduces the per-event cost by ~78% due to amortised
auth, state reads, and base fees. See the [Batch vs Single Logging Cost](#batch-vs-single-logging-cost)
section above for full numbers.

| Batch size | Total fee for 1,000 events (est.) |
|------------|----------------------------------|
| 1 (individual) | ~1.0–1.7 XLM |
| 10 | ~0.36–0.59 XLM |
| 50 (recommended) | ~0.11–0.18 XLM |
| 100 | ~0.10–0.16 XLM |

### Reproducing estimates

```bash
# Simulate a single log_event and inspect the fee breakdown
soroban contract invoke \
  --id $CONTRACT_ID --source $SUBMITTER_KEY --network testnet \
  --fee 100000 \
  -- log_event \
  --submitter $SUBMITTER \
  --event_type payment \
  --metadata "$(python3 -c 'import base64; print(base64.b64encode(b"x"*100).decode())')" \
  --category null \
  --sub_event_type null \
  --force false \
  --simulate-only
```

The simulation output includes `classicFeeCharged`, `resourceFeeCharged`, and the
resource usage breakdown so you can verify against the estimates above.
