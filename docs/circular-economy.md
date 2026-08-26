# Circular Economy Metrics

The `AuditLedger` contract provides a first-class circular economy tracking layer on top of its immutable event log. It lets manufacturers, recyclers, and supply-chain actors register physical assets as **material passports**, record what happens to those assets over their full lifecycle as **loop events**, and then query aggregated **circularity indicators** at any point in time.

All data is stored in Soroban instance storage and is verifiable by any network participant without a trusted intermediary.

---

## Concepts

### Material Passport

A material passport is the on-chain identity record for a physical asset — a product, component, or batch of raw material. It stores the asset's intrinsic properties at the time of manufacture/registration and accumulates mass-flow totals as loop events are recorded.

Each passport is identified by a 32-byte content-addressed ID derived from:

```
sha256(owner_address_bytes || asset_name || ledger_timestamp_le64)
```

This makes IDs collision-resistant and unpredictable without a trusted random oracle.

### Loop Events

A loop event records a single material-flow action. Each event captures:

- **Who** performed it (authenticated `actor` address)
- **What kind of action** (loop type)
- **How much material** was involved (`quantity_mg` in milligrams)
- **Where it went** (optional `target_material_id` for output linking)
- **Provenance metadata** (opaque bytes: batch ref, certificate, GPS, etc.)

Events are append-only and carry a sequential `seq` number within the material's history.

### Circularity Snapshot

A snapshot aggregates all registered material passports and loop events into a single set of circularity indicators at a given ledger sequence. Snapshots are stored on-chain indexed by a 0-based counter, giving an auditable time-series of circularity performance.

---

## Loop Event Types

| Symbol     | Discriminant | Description                                                 |
|------------|:------------:|-------------------------------------------------------------|
| `recycle`  | 0            | Material sent to a recycling process (downcycling included) |
| `reuse`    | 1            | Item used again in its current form, without transformation |
| `repair`   | 2            | Item repaired to extend its functional service life         |
| `remanuf`  | 3            | Product remanufactured to original specification            |
| `return`   | 4            | Item returned to manufacturer or supplier                   |
| `dispose`  | 5            | Material disposed — landfill, incineration, or waste export |

> Loop types `recycle`, `reuse`, `repair`, and `remanuf` contribute to the **circular mass** numerator. `dispose` contributes to the **linear mass** denominator. `return` is tracked as an event but does not directly contribute to any mass accumulator, since the eventual downstream fate is unknown at the time of return.

---

## Data Structures

### `MaterialPassport`

```rust
pub struct MaterialPassport {
    pub id: BytesN<32>,           // Content-addressed 32-byte ID
    pub name: Bytes,              // Human-readable asset name (≤128 bytes)
    pub category: Symbol,         // Material category (plastic, metal, glass, …)
    pub virgin_mass_mg: u64,      // Initial virgin mass in milligrams
    pub recyclability_bps: u32,   // Recyclability rating: 0–10000 (bps)
    pub owner: Address,           // Registering entity
    pub registered_at: u64,       // Ledger timestamp of registration
    // Accumulators updated by record_loop_event:
    pub total_recycled_mg: u64,
    pub total_reused_mg: u64,
    pub total_repaired_mg: u64,
    pub total_remanufactured_mg: u64,
    pub total_disposed_mg: u64,
    pub loop_event_count: u32,
}
```

**`recyclability_bps`** is a design-time estimate of the material's recyclability potential, expressed in basis points (100 bps = 1%). It is informational — actual flow is measured through loop events.

**Mass units** are milligrams throughout to avoid floating-point arithmetic and keep integer overflow safe up to ~9.2 × 10¹² tonnes.

### `LoopEvent`

```rust
pub struct LoopEvent {
    pub seq: u32,                           // 0-based position in material's history
    pub timestamp: u64,                     // Ledger timestamp
    pub loop_type: u32,                     // Discriminant (see table above)
    pub quantity_mg: u64,                   // Mass involved, in milligrams
    pub actor: Address,                     // Authenticated actor
    pub target_material_id: Option<BytesN<32>>, // Output material ID (optional)
    pub metadata: Bytes,                    // Opaque provenance data
}
```

### `CircularitySnapshot`

```rust
pub struct CircularitySnapshot {
    pub ledger_seq: u32,              // Ledger sequence at snapshot time
    pub timestamp: u64,               // Ledger timestamp
    pub total_materials: u32,         // Total registered passports
    pub total_virgin_mass_mg: u64,    // Sum of virgin_mass_mg across all passports
    pub total_circular_mass_mg: u64,  // recycle + reuse + repair + remanuf (mg)
    pub total_disposed_mass_mg: u64,  // dispose flows (mg)
    pub mci_bps: u32,                 // Material Circularity Indicator (0–10000 bps)
    pub recycling_rate_bps: u32,      // Recycled / total_flow (bps)
    pub reuse_rate_bps: u32,          // Reused / total_flow (bps)
    pub loop_closure_rate_bps: u32,   // Passports with ≥1 non-dispose event / total (bps)
    pub total_loop_events: u32,       // Total loop events across all materials
    pub snapshot_index: u32,          // 0-based snapshot ordinal
}
```

### `CircularityTotals`

Running aggregates persisted in instance storage — updated atomically on every `register_material_passport` and `record_loop_event` call to avoid O(N) scans.

```rust
pub struct CircularityTotals {
    pub total_materials: u32,
    pub total_virgin_mass_mg: u64,
    pub total_recycled_mg: u64,
    pub total_reused_mg: u64,
    pub total_repaired_mg: u64,
    pub total_remanufactured_mg: u64,
    pub total_disposed_mg: u64,
    pub total_loop_events: u32,
    pub materials_with_closed_loop: u32, // passports with ≥1 non-dispose loop event
}
```

---

## Circularity Indicator Formulas

All indicators are expressed in **basis points** (bps): `10000 bps = 100%`.

### Material Circularity Indicator (MCI)

The MCI is the primary headline indicator. It measures what fraction of tracked material flow is circular rather than linear.

```
circular_mass = total_recycled_mg + total_reused_mg
              + total_repaired_mg + total_remanufactured_mg

total_flow = circular_mass + total_disposed_mg

mci_bps = floor(circular_mass × 10000 / total_flow)   if total_flow > 0
        = 0                                             otherwise
```

**Range**: 0 (fully linear) → 10000 (fully circular)

**Example**: 3 tonnes recycled, 1 tonne disposed → MCI = 7500 bps (75%)

### Recycling Rate

```
recycling_rate_bps = floor(total_recycled_mg × 10000 / total_flow)
```

### Reuse Rate

```
reuse_rate_bps = floor(total_reused_mg × 10000 / total_flow)
```

### Loop Closure Rate

Measures the fraction of registered materials that have had at least one positive circular action (any loop event except `dispose`).

```
loop_closure_rate_bps = floor(materials_with_closed_loop × 10000 / total_materials)
```

A material "closes its loop" on the first non-dispose loop event recorded against it. Multiple non-dispose events on the same material do not increment this counter further.

---

## API Reference

### Write

#### `register_material_passport`

Register a new material passport. The caller becomes the passport owner.

```rust
fn register_material_passport(
    env: Env,
    caller: Address,       // Must authenticate
    name: Bytes,           // Asset name (≤128 bytes)
    category: Symbol,      // Material category
    virgin_mass_mg: u64,   // > 0
    recyclability_bps: u32 // 0–10000
) -> BytesN<32>            // Returns the new passport ID
```

**Error codes**:
- `ContractNotInitialized (9)` — Call `initialize` first.
- `InvalidFlowQuantity (37)` — `virgin_mass_mg == 0` or `recyclability_bps > 10000`.
- `MaterialPassportAlreadyExists (34)` — Duplicate ID (collision in sha256 preimage — astronomically rare).

**Soroban event emitted**: `topic=(circular, passport_reg)`, `data=<passport_id>`

#### `record_loop_event`

Append a loop event to a material's history. Updates the passport accumulators and global totals atomically.

```rust
fn record_loop_event(
    env: Env,
    caller: Address,                      // Must authenticate
    material_id: BytesN<32>,              // Registered passport ID
    loop_type: Symbol,                    // One of the six recognised types
    quantity_mg: u64,                     // > 0
    target_material_id: Option<BytesN<32>>, // Output material reference
    metadata: Bytes                       // Opaque provenance bytes
) -> u32                                  // Sequential seq within material history
```

**Error codes**:
- `MaterialPassportNotFound (35)` — `material_id` not registered.
- `InvalidLoopEventType (36)` — `loop_type` not one of the six recognised Symbols.
- `InvalidFlowQuantity (37)` — `quantity_mg == 0`.

**Soroban event emitted**: `topic=(circular, loop_event)`, `data=(material_id, loop_type, quantity_mg)`

### Read

#### `get_material_passport`

```rust
fn get_material_passport(env: Env, material_id: BytesN<32>) -> MaterialPassport
```

Returns the full passport including accumulated totals.

**Error codes**: `MaterialPassportNotFound (35)`

#### `get_material_loop`

```rust
fn get_material_loop(env: Env, material_id: BytesN<32>) -> Vec<LoopEvent>
```

Returns the ordered list of all loop events recorded for a material.

**Error codes**: `MaterialPassportNotFound (35)`

#### `compute_circularity_score`

```rust
fn compute_circularity_score(env: Env) -> CircularitySnapshot
```

Reads the current `CircularityTotals`, computes all circularity indicators, persists the snapshot on-chain, increments the snapshot counter, and returns the snapshot.

No authentication required — this is a public read+write call (anyone can trigger a snapshot).

**Soroban event emitted**: `topic=(circular, snapshot)`, `data=(snapshot_index, mci_bps)`

#### `get_circularity_snapshot`

```rust
fn get_circularity_snapshot(env: Env, index: u32) -> CircularitySnapshot
```

Retrieve a stored snapshot by 0-based index.

**Error codes**: `SnapshotNotFound (30)`

#### `circularity_snapshot_count`

```rust
fn circularity_snapshot_count(env: Env) -> u32
```

Total number of circularity snapshots stored (one per `compute_circularity_score` call).

#### `get_circularity_totals`

```rust
fn get_circularity_totals(env: Env) -> CircularityTotals
```

Return the live running totals without creating a snapshot. Useful for lightweight monitoring.

---

## Error Reference

| Code | Name                          | Description                                               |
|------|-------------------------------|-----------------------------------------------------------|
| 34   | `MaterialPassportAlreadyExists` | Passport with this ID already registered               |
| 35   | `MaterialPassportNotFound`    | No passport registered with the given ID                  |
| 36   | `InvalidLoopEventType`        | `loop_type` symbol not recognised                         |
| 37   | `InvalidFlowQuantity`         | `quantity_mg == 0` or `virgin_mass_mg == 0`, or `recyclability_bps > 10000` |

---

## Storage Layout

All circular economy state is stored in **instance storage** under the following `DataKey` variants:

| DataKey                          | Value type            | Description                                          |
|----------------------------------|-----------------------|------------------------------------------------------|
| `MaterialPassport(BytesN<32>)`   | `MaterialPassport`    | Full passport keyed by material ID                   |
| `MaterialLoopEvents(BytesN<32>)` | `Vec<LoopEvent>`      | Ordered loop history for a material                  |
| `CircularitySnapshot(u32)`       | `CircularitySnapshot` | Snapshot keyed by 0-based ordinal                    |
| `CircularitySnapshotCount`       | `u32`                 | Total snapshots taken                                |
| `CircularityTotals`              | `CircularityTotals`   | Live running aggregate — updated on every write call |
| `AllMaterialIds`                 | reserved              | Reserved for future bulk enumeration                 |

Using instance storage keeps all circular economy data under the contract's TTL and avoids the additional ledger entries of persistent storage. For long-lived deployments, consider archiving old passport data off-chain and using persistent storage for passports with explicit TTL management (see `docs/fees.md#ttl-storage`).

---

## Walkthrough: End-to-End Example

The following Soroban CLI invocations illustrate a complete material lifecycle.

### 1. Register a material passport

```bash
soroban contract invoke --id <CONTRACT_ID> --source <MANUFACTURER_KEY> --network testnet -- \
  register_material_passport \
  --caller <MANUFACTURER_ADDR> \
  --name "5050504554426f74746c65" \  # hex("PETBottle")
  --category "plastic" \
  --virgin_mass_mg 500000 \          # 500 g
  --recyclability_bps 9000           # 90%
```

Returns: `<PASSPORT_ID>` (32-byte hex)

### 2. Record a reuse event

```bash
soroban contract invoke --id <CONTRACT_ID> --source <DISTRIBUTOR_KEY> --network testnet -- \
  record_loop_event \
  --caller <DISTRIBUTOR_ADDR> \
  --material_id <PASSPORT_ID> \
  --loop_type "reuse" \
  --quantity_mg 500000 \
  --target_material_id null \
  --metadata "72656669766564" # hex("refived")
```

### 3. Record a recycle event with output linkage

```bash
soroban contract invoke --id <CONTRACT_ID> --source <RECYCLER_KEY> --network testnet -- \
  record_loop_event \
  --caller <RECYCLER_ADDR> \
  --material_id <PASSPORT_ID> \
  --loop_type "recycle" \
  --quantity_mg 450000 \             # 450 g sent to recycling
  --target_material_id <OUTPUT_PASSPORT_ID> \
  --metadata "62617463682d32303236" # hex("batch-2026")
```

### 4. Take a circularity snapshot

```bash
soroban contract invoke --id <CONTRACT_ID> --source <ANY_KEY> --network testnet -- \
  compute_circularity_score
```

Returns a `CircularitySnapshot` — e.g.:

```json
{
  "ledger_seq": 1042357,
  "total_materials": 1,
  "total_virgin_mass_mg": 500000,
  "total_circular_mass_mg": 950000,
  "total_disposed_mass_mg": 0,
  "mci_bps": 10000,
  "recycling_rate_bps": 4736,
  "reuse_rate_bps": 5263,
  "loop_closure_rate_bps": 10000,
  "total_loop_events": 2,
  "snapshot_index": 0
}
```

### 5. Read the passport state

```bash
soroban contract invoke --id <CONTRACT_ID> --network testnet -- \
  get_material_passport --material_id <PASSPORT_ID>
```

---

## Integration Patterns

### Off-chain Monitoring

The metrics exporter (`tools/metrics-exporter`) can poll `get_circularity_totals` and expose the values as Prometheus gauges for Grafana dashboards:

```
audit_ledger_mci_bps              — Current MCI in basis points
audit_ledger_recycling_rate_bps   — Recycling rate
audit_ledger_reuse_rate_bps       — Reuse rate
audit_ledger_loop_closure_rate_bps — Loop closure rate
audit_ledger_total_materials      — Total registered passports
audit_ledger_total_loop_events    — Total loop events
```

### Snapshot Time-Series

Call `compute_circularity_score` once per reporting period (e.g., weekly). Read all stored snapshots via `get_circularity_snapshot(0..count-1)` to build a time-series. Each snapshot carries `ledger_seq` and `timestamp` for temporal alignment.

### Product-Level Audit

For any product under scrutiny, retrieve its passport with `get_material_passport` and its full event history with `get_material_loop`. Both are directly verifiable on-chain — no intermediary database required.

### Supply Chain Cross-Linking

When a recycler converts one material into another, record the loop event on the source material with `target_material_id` pointing to the output passport. This creates a directed graph of material transformations, fully traceable on-chain.

---

## Design Decisions

**Why milligrams?** Soroban smart contracts use integer arithmetic only. Milligrams provide sub-gram precision for lightweight goods while keeping totals within `u64` limits (max ~9.2 million tonnes per accumulator — sufficient for any realistic supply chain deployment).

**Why basis points?** Percentages expressed as floats would require custom fixed-point arithmetic. Basis points are integers, unambiguous, and familiar in financial contexts (the existing contract already uses them for caps and rates).

**Why instance storage?** Circular economy state is queried together. Storing everything under instance storage means a single instance entry read warms all state for the call. For very large numbers of passports, future work could shard passports into persistent storage keyed by material ID.

**Why are `CircularityTotals` persisted separately?** Computing the MCI over N passports naively requires iterating all N entries. Maintaining running totals in a single key reduces every snapshot computation to O(1) reads regardless of how many passports exist.

**Why is `AllMaterialIds` reserved but not written?** Enumerating all material IDs would require a growing list, which can become expensive. The design intentionally defers enumeration to off-chain indexers that subscribe to the `(circular, passport_reg)` event stream and maintain their own ID lists.
