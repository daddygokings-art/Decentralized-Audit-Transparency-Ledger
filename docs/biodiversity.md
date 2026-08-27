# Biodiversity Impact Tracking

The `AuditLedger` contract provides an on-chain biodiversity accounting layer that links directly to supply-chain events. Any operational event — a cleared forest, a construction site, a farm parcel, a logistics hub — can be linked to a standardised biodiversity impact record. Offsets can be registered and retired to demonstrate compensation, and periodic nature-positive snapshots give an auditable time-series of the net biodiversity position.

All data is immutable once written, publicly verifiable, and cross-referenced with the existing audit event log by 32-byte event IDs.

---

## Standards Alignment

| Framework | Coverage |
|---|---|
| TNFD (Taskforce on Nature-related Financial Disclosures) | LEAP approach — Locate, Evaluate, Assess, Prepare |
| GBF Target 15 (Kunming-Montreal Global Biodiversity Framework) | Mandatory corporate nature disclosure |
| EU CSRD / ESRS E4 | Biodiversity and ecosystems reporting |
| IUCN Red List | Species threat categories (CR, EN, VU, NT, LC, DD) |
| IUCN STAR | Species Threat Abatement and Restoration metric (MSA proxy) |
| GLOBIO / PREDICTS | MSA (Mean Species Abundance) land-use pressure model |
| CICES v5.1 | Ecosystem service classification (provisioning, regulating, cultural, supporting) |
| TEEB / SEEA EA | Ecosystem accounting and monetary valuation |
| Voluntary Biodiversity Credits (VBC) | Credit issuance, transfer, and retirement |
| Biodiversity Net Gain (BNG, England) | Net gain unit registration and retirement |

---

## Core Concepts

### Mean Species Abundance (MSA)

MSA measures biodiversity integrity relative to an undisturbed reference state (MSA = 1.0 = pristine). Land-use change drives MSA down. The contract tracks biodiversity loss as **MSA·ha** (area-weighted MSA loss):

```
msa_loss_ha = area_ha × (MSA_before − MSA_after)
```

Values are stored as **MSA·ha micro-units** (× 10⁻⁶). To convert: `value / 1_000_000 = MSA·ha`.

### Nature-Positive Indicator

The nature-positive indicator measures whether total offset retirements compensate for total MSA losses:

```
nature_positive_bps = floor(total_retired_micro × 10_000 / total_msa_loss_micro)
                    = 10_000  when no losses are recorded (no impact → nature-positive by definition)
                    (capped at 10_000, i.e. 100%)
```

A value ≥ 10_000 bps = 100% signals a net-gain position. Values < 10_000 indicate an outstanding biodiversity debt.

### Net MSA Position

```
net_msa_micro = total_retired_micro − total_msa_loss_micro
```

Positive = net biodiversity gain. Negative = net debt.

---

## Land-Use Type Taxonomy

| Discriminant | Symbol | Description | Typical MSA |
|:---:|---|---|---|
| 0 | `crop` | Annual and permanent cropland | 0.30 |
| 1 | `pasture` | Managed grassland / livestock grazing | 0.50 |
| 2 | `forest` | Natural or semi-natural forest | 0.80–1.00 |
| 3 | `urban` | Urban / built-up area | 0.05 |
| 4 | `wetland` | Freshwater, coastal, and inland wetlands | 0.80–1.00 |
| 5 | `water` | Open water body | variable |
| 6 | `barren` | Rock, desert, polar (very low biodiversity) | 0.10 |
| 7 | `protected` | Formally protected area (IUCN PA categories I–VI) | 0.90–1.00 |

---

## Ecosystem Service Categories (CICES v5.1)

Values stored as **USD-cent micro-units** (× 10⁻⁶) per year. Negative values encode ecosystem gains (e.g., restoration).

| Index | Symbol | Class | Examples |
|:---:|---|---|---|
| 0 | `provision` | Provisioning | Food, water supply, raw materials, genetic resources |
| 1 | `regul` | Regulating | Climate regulation, flood control, water purification, pollination |
| 2 | `culture` | Cultural | Recreation, tourism, spiritual, educational |
| 3 | `support` | Supporting | Soil formation, nutrient cycling, habitat provision |

---

## Data Structures

### `BioImpact`

```rust
pub struct BioImpact {
    pub id: BytesN<32>,               // sha256(event_ref || actor_strkey || timestamp_le64)
    pub event_ref: BytesN<32>,        // Supply-chain audit event reference
    pub actor: Address,               // Entity recording this impact
    pub timestamp: u64,
    pub land_use_type: u32,           // 0–7 (see taxonomy above)
    pub area_m2_micro: u64,           // Affected area in m² × 10⁻⁶ (> 0)
    pub msa_loss_micro: u64,          // MSA·ha loss × 10⁻⁶
    pub eco_service_loss: Vec<i64>,   // 4 values (USD-cent × 10⁻⁶, per CICES order)
    pub location: Bytes,              // Geographic descriptor (e.g., "lat,lon")
    pub iucn_threat: Bytes,           // IUCN Red List category bytes (e.g., b"EN")
    pub metadata: Bytes,              // Opaque provenance
}
```

### `BioOffset`

```rust
pub struct BioOffset {
    pub id: BytesN<32>,                      // sha256(scheme || issuer_strkey || total_micro_le64 || timestamp_le64)
    pub scheme: Bytes,                       // Offset scheme name (e.g., b"vbc", b"bng")
    pub issuer: Address,
    pub total_micro: u64,                    // Total credits in MSA·ha × 10⁻⁶
    pub retired_micro: u64,                  // Retired (consumed) credits
    pub registered_at: u64,
    pub expires_at: u64,                     // 0 = no expiry
    pub eco_service_ref: Option<BytesN<32>>, // Optional ecosystem service site link
    pub metadata: Bytes,                     // Certificate reference, registry URL
}
```

### `EcoServiceRecord`

```rust
pub struct EcoServiceRecord {
    pub id: BytesN<32>,              // sha256(name || owner_strkey || area_le64 || timestamp_le64)
    pub name: Bytes,                 // Project/site name
    pub owner: Address,
    pub registered_at: u64,
    pub area_m2_micro: u64,          // Site area in m² × 10⁻⁶ (> 0)
    pub annual_values: Vec<i64>,     // 4 annual values in USD-cent × 10⁻⁶
    pub land_use_type: u32,
    pub metadata: Bytes,
}
```

### `SpeciesObservation`

```rust
pub struct SpeciesObservation {
    pub id: BytesN<32>,
    pub event_ref: BytesN<32>,       // Linked supply-chain event
    pub species_name: Bytes,         // Common name
    pub species_code: Bytes,         // Binomial or IUCN ID
    pub iucn_category: Bytes,        // Red List category (b"CR", b"EN", b"VU", etc.)
    pub count: u32,                  // Individual count (0 = presence-only)
    pub impact_direction: u32,       // 0=positive, 1=negative, 2=neutral
    pub observer: Address,
    pub timestamp: u64,
    pub metadata: Bytes,
}
```

### `BioTotals`

Running global aggregates — updated atomically on every write call:

```rust
pub struct BioTotals {
    pub total_impacts: u32,
    pub total_area_m2_micro: u64,
    pub total_msa_loss_micro: u64,
    pub total_eco_loss_micro: i64,    // signed (gains are negative)
    pub total_offset_micro: u64,
    pub total_retired_micro: u64,
    pub total_observations: u32,
    pub total_eco_records: u32,
}
```

### `BioSnapshot`

Point-in-time nature-positive accounting:

```rust
pub struct BioSnapshot {
    pub index: u32,
    pub ledger_seq: u32,
    pub timestamp: u64,
    pub total_impacts: u32,
    pub total_msa_loss_micro: u64,
    pub total_offset_micro: u64,
    pub total_retired_micro: u64,
    pub net_msa_micro: i64,            // retired − loss (signed)
    pub nature_positive_bps: u32,      // 0–10000 bps
    pub offset_coverage_bps: u32,      // same as nature_positive_bps (capped at 10000)
    pub total_eco_loss_micro: i64,
    pub total_observations: u32,
}
```

---

## API Reference

### Write

#### `record_bio_impact`

Record a biodiversity impact event linked to a supply-chain event.

```rust
fn record_bio_impact(
    env: Env,
    caller: Address,            // Must authenticate
    event_ref: BytesN<32>,      // Audit event reference
    land_use_type: Symbol,      // crop | pasture | forest | urban | wetland | water | barren | protected
    area_m2_micro: u64,         // > 0, m² × 10⁻⁶
    msa_loss_micro: u64,        // MSA·ha × 10⁻⁶
    eco_service_loss: Vec<i64>, // Exactly 4 values (negatives = gains)
    location: Bytes,
    iucn_threat: Bytes,
    metadata: Bytes,
) -> BytesN<32>                 // Returns impact ID
```

Soroban event: `topic=(bio, impact)`, `data=(id, land_use_type, area_m2_micro)`

**Error codes**: `InvalidLandUseType (49)`, `InvalidLandArea (51)`, `InvalidImpactCategory (41)`.

---

#### `register_bio_offset`

Register a biodiversity offset credit.

```rust
fn register_bio_offset(
    env: Env,
    caller: Address,
    scheme: Bytes,                       // e.g., b"vbc", b"bng", b"mitbank"
    total_micro: u64,                    // > 0, MSA·ha × 10⁻⁶
    expires_at: u64,                     // 0 = no expiry
    eco_service_ref: Option<BytesN<32>>, // Optional ecosystem service site
    metadata: Bytes,
) -> BytesN<32>                          // Returns offset ID
```

Soroban event: `topic=(bio, offset_reg)`, `data=(id, total_micro)`

**Error codes**: `InvalidOffsetQuantity (52)`, `BioImpactNotFound (47)` (if `eco_service_ref` points to unregistered record).

---

#### `retire_bio_offset`

Apply offset credits to compensate for impacts. Irreversible, cumulative.

```rust
fn retire_bio_offset(
    env: Env,
    caller: Address,
    offset_id: BytesN<32>,
    quantity_micro: u64,      // > 0; ≤ remaining balance
) -> u64                      // Returns remaining available balance
```

Soroban event: `topic=(bio, offset_ret)`, `data=(offset_id, quantity_micro, remaining)`

**Error codes**: `BioOffsetNotFound (48)`, `InvalidOffsetQuantity (52)`, `OffsetAlreadyRetired (53)`, `OffsetRetirementExceedsBalance (54)`.

---

#### `register_eco_service_record`

Register an ecosystem service valuation for a project site.

```rust
fn register_eco_service_record(
    env: Env,
    caller: Address,
    name: Bytes,
    area_m2_micro: u64,         // > 0
    land_use_type: Symbol,
    annual_values: Vec<i64>,    // Exactly 4 values (USD-cent × 10⁻⁶)
    metadata: Bytes,
) -> BytesN<32>                 // Returns site/record ID
```

Soroban event: `topic=(bio, eco_reg)`, `data=id`

**Error codes**: `InvalidLandUseType (49)`, `InvalidLandArea (51)`, `InvalidImpactCategory (41)`.

---

#### `record_species_observation`

Record a species sighting or survey result linked to a supply-chain event.

```rust
fn record_species_observation(
    env: Env,
    caller: Address,
    event_ref: BytesN<32>,
    species_name: Bytes,
    species_code: Bytes,         // IUCN taxonomic ID or binomial name
    iucn_category: Bytes,        // b"CR" | b"EN" | b"VU" | b"NT" | b"LC" | b"DD"
    count: u32,                  // 0 = presence-only
    impact_direction: u32,       // 0=positive | 1=negative | 2=neutral
    metadata: Bytes,
) -> BytesN<32>                  // Returns observation ID
```

Soroban event: `topic=(bio, species_obs)`, `data=(id, impact_direction)`

---

#### `compute_nature_positive_score`

Aggregate current totals into a nature-positive snapshot.

```rust
fn compute_nature_positive_score(env: Env) -> BioSnapshot
```

Callable by any party. Persists the snapshot on-chain and increments the snapshot counter.

Soroban event: `topic=(bio, snapshot)`, `data=(index, nature_positive_bps, net_msa_micro)`

---

### Read

| Function | Returns | Description |
|---|---|---|
| `get_bio_impact(id)` | `BioImpact` | Impact record by ID |
| `get_bio_offset(id)` | `BioOffset` | Offset record by ID |
| `get_eco_service_record(id)` | `EcoServiceRecord` | Ecosystem service site |
| `get_species_observation(id)` | `SpeciesObservation` | Species observation by ID |
| `get_bio_snapshot(index)` | `BioSnapshot` | Snapshot by 0-based ordinal |
| `bio_snapshot_count()` | `u32` | Total snapshots taken |
| `get_bio_totals()` | `BioTotals` | Live running aggregates |

---

## Error Reference

| Code | Name | Description |
|:---:|---|---|
| 47 | `BioImpactNotFound` | No impact record (or ecosystem service record) for the given ID |
| 48 | `BioOffsetNotFound` | No offset record for the given ID |
| 49 | `InvalidLandUseType` | Land-use Symbol not one of the eight recognised values |
| 50 | `InvalidEcoServiceCat` | Ecosystem service Symbol not one of the four recognised values |
| 51 | `InvalidLandArea` | `area_m2_micro == 0` |
| 52 | `InvalidOffsetQuantity` | `total_micro == 0` or `quantity_micro == 0` |
| 53 | `OffsetAlreadyRetired` | Offset fully consumed; no credits remain |
| 54 | `OffsetRetirementExceedsBalance` | Retirement quantity > remaining balance |
| 55 | `SpeciesObservationNotFound` | No observation for the given ID |

---

## Storage Layout

All biodiversity state lives in **instance storage**:

| DataKey | Value type | Description |
|---|---|---|
| `BioImpact(BytesN<32>)` | `BioImpact` | Impact records keyed by ID |
| `BioOffset(BytesN<32>)` | `BioOffset` | Offset records keyed by ID |
| `EcoServiceRecord(BytesN<32>)` | `EcoServiceRecord` | Site valuations keyed by ID |
| `SpeciesObservation(BytesN<32>)` | `SpeciesObservation` | Observations keyed by ID |
| `BioTotals` | `BioTotals` | Live running aggregates |
| `BioSnapshot(u32)` | `BioSnapshot` | Snapshots keyed by 0-based ordinal |
| `BioSnapshotCount` | `u32` | Total snapshots taken |

---

## Nature-Positive Reporting Workflow

```
Supply-chain event occurs
        │
record_bio_impact ──────────────────────── links event_ref → BioImpact stored
        │                                   BioTotals.msa_loss incremented
        │
        ├── register_eco_service_record ─── characterise restoration site → EcoServiceRecord
        │         │
        │   register_bio_offset ─────────── issue credits backed by site → BioOffset
        │         │
        │   retire_bio_offset ───────────── consume credits to offset loss → retired_micro++
        │
record_species_observation ─────────────── optional: link species survey to event
        │
compute_nature_positive_score ──────────── aggregate → BioSnapshot (time-series point)
        │
get_bio_snapshot(0..count-1) ───────────── retrieve time-series for reporting
```

---

## End-to-End CLI Walkthrough

### 1. Record a land-clearing impact

```bash
soroban contract invoke -- record_bio_impact \
  --caller <SUPPLIER> \
  --event_ref <AUDIT_EVENT_ID> \
  --land_use_type forest \
  --area_m2_micro 5000000000 \    # 5 km²
  --msa_loss_micro 1200000 \      # 1.2 MSA·ha
  --eco_service_loss "[0,5000000,0,2000000]" \   # regulating + supporting
  --location "-3.1,52.4" \
  --iucn_threat "EN" \
  --metadata "clearing-permit-2026-007"
# → impact_id
```

### 2. Register an offset site

```bash
soroban contract invoke -- register_eco_service_record \
  --caller <NGO> \
  --name "Borneo Rainforest Restoration" \
  --area_m2_micro 10000000000 \   # 10 km²
  --land_use_type protected \
  --annual_values "[0,8000000,500000,3000000]" \
  --metadata "REDD+-cert-BOR-2026"
# → site_id
```

### 3. Issue biodiversity offset credits

```bash
soroban contract invoke -- register_bio_offset \
  --caller <NGO> \
  --scheme "vbc" \
  --total_micro 2000000 \        # 2.0 MSA·ha
  --expires_at 0 \
  --eco_service_ref <site_id> \
  --metadata "VBC-registry-BOR-001"
# → offset_id
```

### 4. Retire credits to compensate the impact

```bash
soroban contract invoke -- retire_bio_offset \
  --caller <SUPPLIER> \
  --offset_id <offset_id> \
  --quantity_micro 1200000        # exactly matches the 1.2 MSA·ha loss
# → remaining_balance = 800000
```

### 5. Take a nature-positive snapshot

```bash
soroban contract invoke -- compute_nature_positive_score
# Returns BioSnapshot:
# {
#   nature_positive_bps: 10000,    # 100% — fully offset
#   net_msa_micro:       0,
#   total_msa_loss_micro: 1200000,
#   total_retired_micro:  1200000
# }
```

---

## Integration with LCA and Circular Economy Modules

Biodiversity records can be cross-linked with the other sustainability modules:

- **LCA integration**: Use `event_ref` to point to the same supply-chain event that drove an LCA phase impact. The LCA `LU` (Land Use) category (index 7) and `msa_loss_micro` in `BioImpact` are complementary measures of the same physical land pressure.

- **Circular economy integration**: Material passports (`register_material_passport`) and biodiversity impacts both reference supply-chain events. A product with both modules populated gives a complete picture: material flow efficiency (circularity) + ecological footprint (biodiversity) + life-cycle environmental burden (LCA).

---

## Design Notes

**MSA·ha micro-units.** Floating-point arithmetic is unavailable in Soroban. All biodiversity quantities use integer micro-units (× 10⁻⁶) for sufficient resolution. 1 MSA·ha = 1_000_000 micro-units. Maximum representable value: ~9.2 × 10¹² MSA·ha — sufficient for any realistic supply-chain study.

**Signed ecosystem service values.** `eco_service_loss` and `annual_values` use `i64` to allow negative numbers encoding gains (e.g., from restoration or rewilding activities). The global totals accumulator `total_eco_loss_micro` is also signed.

**Offset coverage cap.** The nature-positive indicator is capped at 10_000 bps (100%). Over-offsetting (more retired than lost) increases `net_msa_micro` but does not artificially inflate the indicator beyond 100%.

**Who can retire offsets.** Any authenticated caller may retire an offset. This reflects real-world practice where a buyer acquires and retires credits issued by a third-party project developer. There is no ownership restriction on retirement — the offset ID itself is the bearer instrument.

**Snapshot immutability.** Each call to `compute_nature_positive_score` creates a new snapshot. Old snapshots are never overwritten. This gives regulators and auditors an append-only time-series of the organization's nature-positive journey.
