# Lifecycle Assessment (LCA)

The `AuditLedger` contract implements a full ISO 14040/14044-aligned lifecycle assessment layer, linking every LCA profile to the same immutable audit trail used for financial and operational events. For each product or functional unit, practitioners register a profile, submit per-phase environmental burdens, finalize to lock results, then optionally normalize and weight to produce a single score.

---

## Scope and Standards Alignment

| Standard | Coverage |
|---|---|
| ISO 14040 | Goal & scope definition, inventory analysis, impact assessment, interpretation |
| ISO 14044 | Requirements for LCA practitioners, data quality, uncertainty reporting |
| EN 15804 | Cradle-to-grave scope for construction products (phases A–D mapped to contract phases) |
| EF 3.1 | Impact categories (all 8 supported), normalization factors, weighting |
| CML 2016 | Alternative normalization reference set |
| ReCiPe H | Alternative weighting scheme |

---

## Lifecycle Phase Taxonomy

All seven phases cover the full **cradle-to-cradle** scope. Phases 0–5 give the standard **cradle-to-grave** scope.

| Discriminant | Symbol | Phase name | EN 15804 module |
|:---:|---|---|---|
| 0 | `raw_mat` | Raw material extraction & upstream processes | A1 |
| 1 | `mfg` | Manufacturing and processing | A3 |
| 2 | `transport` | Transport and distribution to the market | A4 |
| 3 | `use` | Use phase (energy, water, consumables) | B6 |
| 4 | `maint` | Maintenance and repair | B2, B3 |
| 5 | `eol` | End-of-life (disposal, incineration, landfill) | C3, C4 |
| 6 | `recycling` | Recycling and recovery (may carry negative/credit values) | D |

Phase 6 (`recycling`) credits may be negative integers, encoding avoided burdens from material recovery. The aggregator handles signed arithmetic throughout.

---

## Impact Categories

Eight mid-point impact categories, following CML/EF conventions. All values are stored as **fixed-point integers scaled by 10⁶** (micro-units) to avoid floating-point arithmetic on-chain.

To convert to SI units: `value_SI = stored_value / 1_000_000`

| Index | Abbreviation | SI Unit | Description |
|:---:|---|---|---|
| 0 | **GWP** | kg CO₂-eq | Global Warming Potential (100-year horizon) |
| 1 | **AP** | kg SO₂-eq | Acidification Potential |
| 2 | **EP** | kg PO₄³⁻-eq | Eutrophication Potential |
| 3 | **ODP** | kg CFC-11-eq | Ozone Depletion Potential |
| 4 | **POCP** | kg C₂H₄-eq | Photochemical Ozone Creation Potential |
| 5 | **ADP** | kg Sb-eq | Abiotic Depletion Potential (elements) |
| 6 | **WU** | m³ | Water Use |
| 7 | **LU** | m² · year | Land Use |

A `Vec<i64>` of exactly 8 values is passed to `record_phase_impact` and stored for each phase. Any values that remain zero are treated as "not measured" rather than "zero impact" — the distinction matters for interpretation but is not enforced on-chain.

---

## Data Structures

### `LcaProfile`

The on-chain header. Created by `register_lca_entry`; locked by `finalize_lca`.

```rust
pub struct LcaProfile {
    pub product_id: BytesN<32>,              // Content-addressed ID
    pub name: Bytes,                         // Product name
    pub functional_unit: Bytes,              // Reference flow description
    pub owner: Address,                      // Registering entity
    pub registered_at: u64,                  // Ledger timestamp
    pub finalized: bool,                     // True after finalize_lca
    pub phase_mask: u32,                     // Bitmask of submitted phases (bit i = phase i)
    pub material_passport_id: Option<BytesN<32>>, // Link to circular economy passport
}
```

### `LcaPhaseImpact`

Stored per `(product_id, phase_discriminant)`.

```rust
pub struct LcaPhaseImpact {
    pub phase: u32,                    // 0–6
    pub values: Vec<i64>,              // 8 micro-unit values
    pub submitter: Address,            // Actor who submitted this phase
    pub timestamp: u64,                // Ledger timestamp
    pub db_ref: Option<BytesN<32>>,    // Optional LCA database entry ID
    pub metadata: Bytes,               // Data source, methodology notes
}
```

### `LcaResult`

Aggregated result, populated by `finalize_lca` then enriched by `normalize_impacts` and `apply_weighting_scheme`.

```rust
pub struct LcaResult {
    pub totals: Vec<i64>,                    // Raw sums across all phases
    pub normalized: Vec<i64>,               // After normalize_impacts (or zeros)
    pub weighted: Vec<i64>,                 // After apply_weighting_scheme (or zeros)
    pub single_score: i64,                  // Sum of weighted (= 0 until weighting applied)
    pub norm_ref_name: Bytes,               // Name of applied normalization ref (or empty)
    pub weighting_scheme_name: Bytes,       // Name of applied weighting scheme (or empty)
    pub finalized_at: u64,                  // Ledger timestamp of finalize_lca call
}
```

### `LcaUncertainty`

Interval bounds per impact category, computed during `finalize_lca`.

```rust
pub struct LcaUncertainty {
    pub lo: Vec<i64>,          // Lower bound per category
    pub hi: Vec<i64>,          // Upper bound per category
    pub cv_bps: u32,           // Average coefficient of variation (bps) across phases
    pub computed_at: u64,      // Ledger timestamp
}
```

### `LcaNormRef`

Normalization reference set — per-person annual burden per impact category.

```rust
pub struct LcaNormRef {
    pub name: Bytes,           // Short name bytes
    pub refs: Vec<i64>,        // 8 reference values (micro-units); 0 = skip category
    pub owner: Address,        // Owner who registered this set
}
```

### `LcaWeightingScheme`

Category weights in basis points.

```rust
pub struct LcaWeightingScheme {
    pub name: Bytes,           // Short name bytes
    pub weights_bps: Vec<u32>, // 8 weights; partial weighting (sum < 10000) is allowed
    pub owner: Address,
}
```

### `LcaDbEntry`

On-chain anchor for an external LCA dataset.

```rust
pub struct LcaDbEntry {
    pub id: BytesN<32>,           // Content-addressed ID
    pub db_name: Bytes,           // Database name (e.g., "ecoinvent")
    pub version: Bytes,           // Dataset version (e.g., "3.10")
    pub activity: Bytes,          // Activity/process name
    pub geography: Bytes,         // Geography code (e.g., "GLO", "RER")
    pub provider: Address,        // Data provider
    pub registered_at: u64,
}
```

---

## Mathematical Model

### Aggregation (finalize_lca)

For each recorded phase p ∈ {0…6}, category c ∈ {0…7}:

```
totals[c] = Σ_p  phase_impacts[p].values[c]
```

### Uncertainty Propagation (Interval Arithmetic)

Each phase carries a coefficient of variation `cv_bps[p]` (0–10000 bps, where 10000 = 100%). The uncertainty interval for each category accumulates across phases:

```
delta[p][c]  = |phase_impacts[p].values[c]| × cv_bps[p] / 10_000

lo[c]        = Σ_p  ( phase_impacts[p].values[c] − delta[p][c] )
hi[c]        = Σ_p  ( phase_impacts[p].values[c] + delta[p][c] )
```

This is conservative (pessimistic) interval propagation: it does not assume phase uncertainties cancel out, so the bounds widen with each phase. For positive values `lo ≤ totals[c] ≤ hi`; for negative values (recycling credits) the sign is preserved.

The average `cv_bps` reported in `LcaUncertainty` is `Σ cv_bps[p] / count_of_submitted_phases`.

### Normalization

Given a normalization reference set R with values `refs[c]` (per-person annual burden, micro-units):

```
normalized[c] = floor( totals[c] × 1_000_000 / refs[c] )   if refs[c] ≠ 0
normalized[c] = totals[c]                                    if refs[c] = 0  (passthrough)
```

The `× 1_000_000` factor compensates for the existing micro-unit scale and keeps the result in a comparable range to a dimensionless person-equivalent score.

> **Example** (GWP): `totals[0]` = 1_320_000_000 µ-kg CO₂-eq (= 1320 kg CO₂-eq)
> CML 2016 ref: 8_760_000_000_000 µ-kg CO₂-eq (= 8.76 × 10⁶ kg CO₂-eq)
> `normalized[0]` = 1_320_000_000 × 1_000_000 / 8_760_000_000_000 ≈ **150** person-eq-µunits

### Weighting

Given a weighting scheme W with `weights_bps[c]`:

```
weighted[c]  = floor( normalized[c] × weights_bps[c] / 10_000 )

single_score = Σ_c weighted[c]
```

For an equal-weight scheme with 8 categories: `weights_bps[c] = 1250` for all c (sum = 10000).

---

## ID Derivation

### LCA Profile ID

```
product_id = sha256( owner_strkey_bytes || name_bytes || functional_unit_bytes || timestamp_le64 )
```

### LCA Database Entry ID

```
db_entry_id = sha256( db_name_bytes || version_bytes || activity_bytes || geography_bytes || timestamp_le64 )
```

All IDs are deterministic and content-addressed. Duplicate registration (same inputs at the same ledger timestamp) will produce the same ID and trigger `LcaProfileAlreadyExists (38)`.

---

## Storage Layout

All LCA state lives in **instance storage** under the following `DataKey` variants:

| DataKey | Value type | Description |
|---|---|---|
| `LcaProfile(BytesN<32>)` | `LcaProfile` | Profile header keyed by product ID |
| `LcaPhaseImpact(BytesN<32>, u32)` | `LcaPhaseImpact` | Per-phase impacts keyed by (product_id, phase) |
| `LcaResult(BytesN<32>)` | `LcaResult` | Aggregated result (finalized only) |
| `LcaNormRef(Symbol)` | `LcaNormRef` | Normalization reference set |
| `LcaWeightingScheme(Symbol)` | `LcaWeightingScheme` | Weighting scheme |
| `LcaUncertainty(BytesN<32>)` | `LcaUncertainty` *or* `Vec<i64>` | Intermediate cv_bps scratch (pre-finalize) → full bounds (post-finalize) |
| `LcaDbEntry(BytesN<32>)` | `LcaDbEntry` | External database reference |
| `LcaProfileCount` | `u32` | Total profiles registered |

---

## API Reference

### Write

#### `register_lca_entry`

Register a new LCA profile. The caller becomes the profile owner.

```rust
fn register_lca_entry(
    env: Env,
    caller: Address,                          // Must authenticate
    name: Bytes,                              // Product name
    functional_unit: Bytes,                   // Reference flow description
    material_passport_id: Option<BytesN<32>>, // Optional circular economy link
) -> BytesN<32>                               // Returns product_id
```

Soroban event: `topic=(lca, registered)`, `data=product_id`

**Error codes**: `LcaProfileAlreadyExists (38)`, `MaterialPassportNotFound (35)`.

---

#### `record_phase_impact`

Submit environmental impact data for one lifecycle phase. Callable multiple times (once per phase). Overwrites data for a phase that was previously submitted (last write wins per phase per product).

```rust
fn record_phase_impact(
    env: Env,
    caller: Address,             // Must be profile owner
    product_id: BytesN<32>,
    phase: Symbol,               // raw_mat | mfg | transport | use | maint | eol | recycling
    impacts: Vec<i64>,           // Exactly 8 values (micro-units); negatives allowed
    cv_bps: u32,                 // Uncertainty: 0 = none, 1000 = ±10%, 10000 = ±100%
    db_ref: Option<BytesN<32>>,  // Optional LCA database entry ID
    metadata: Bytes,             // Data source, methodology notes
)
```

Soroban event: `topic=(lca, phase_impact)`, `data=(product_id, phase, cv_bps)`

**Error codes**: `LcaProfileNotFound (39)`, `LcaAlreadyFinalized (42)`, `CallerNotOwner (1)`, `InvalidLcaPhase (40)`, `InvalidImpactCategory (41)`, `LcaDbEntryNotFound (46)`.

---

#### `finalize_lca`

Lock the profile, aggregate phase data, and compute uncertainty bounds.

```rust
fn finalize_lca(
    env: Env,
    caller: Address,      // Must be profile owner
    product_id: BytesN<32>,
) -> LcaResult
```

After this call:
- `LcaProfile.finalized = true`
- `LcaResult.totals` populated
- `LcaUncertainty.lo` / `.hi` computed via interval arithmetic
- No further `record_phase_impact` calls are accepted

Soroban event: `topic=(lca, finalized)`, `data=product_id`

**Error codes**: `LcaProfileNotFound (39)`, `LcaAlreadyFinalized (42)`, `CallerNotOwner (1)`.

---

#### `normalize_impacts`

Apply a normalization reference set to the aggregated totals.

```rust
fn normalize_impacts(
    env: Env,
    caller: Address,          // Must be profile owner
    product_id: BytesN<32>,
    norm_ref_name: Symbol,    // Registered reference set key
) -> LcaResult                // LcaResult with normalized field populated
```

Soroban event: `topic=(lca, normalized)`, `data=(product_id, norm_ref_name)`

**Error codes**: `LcaProfileNotFound (39)`, `LcaNotFinalized (43)`, `LcaNormRefNotFound (44)`, `CallerNotOwner (1)`.

---

#### `apply_weighting_scheme`

Weight the normalized values and compute a single score.

```rust
fn apply_weighting_scheme(
    env: Env,
    caller: Address,                // Must be profile owner
    product_id: BytesN<32>,
    weighting_scheme_name: Symbol,  // Registered scheme key
) -> LcaResult                      // LcaResult with weighted + single_score
```

Soroban event: `topic=(lca, weighted)`, `data=(product_id, weighting_scheme_name, single_score)`

**Error codes**: `LcaProfileNotFound (39)`, `LcaNotFinalized (43)`, `LcaWeightingSchemeNotFound (45)`, `CallerNotOwner (1)`.

---

#### `register_lca_db_entry`

Register an LCA database reference entry (any authenticated caller).

```rust
fn register_lca_db_entry(
    env: Env,
    caller: Address,
    db_name: Bytes,      // "ecoinvent", "gabi", "openlca", ...
    version: Bytes,      // "3.10", "2023.1", ...
    activity: Bytes,     // Process / activity name
    geography: Bytes,    // "GLO", "RER", "US", ...
) -> BytesN<32>          // Content-addressed entry ID
```

Soroban event: `topic=(lca, db_entry)`, `data=entry_id`

---

#### `register_norm_ref`

Register a normalization reference set. **Owner-only.**

```rust
fn register_norm_ref(
    env: Env,
    caller: Address,   // Must be contract owner
    name: Symbol,      // Short key, e.g. "cml2016", "ef31"
    refs: Vec<i64>,    // Exactly 8 values (micro-units); 0 = skip category
)
```

**Error codes**: `CallerNotOwner (1)`, `InvalidImpactCategory (41)`.

---

#### `register_weighting_scheme`

Register a weighting scheme. **Owner-only.**

```rust
fn register_weighting_scheme(
    env: Env,
    caller: Address,        // Must be contract owner
    name: Symbol,           // Short key, e.g. "ef31", "recipe_h"
    weights_bps: Vec<u32>,  // Exactly 8 weights in basis points
)
```

**Error codes**: `CallerNotOwner (1)`, `InvalidImpactCategory (41)`.

---

### Read

| Function | Returns | Notes |
|---|---|---|
| `get_lca_profile(product_id)` | `LcaProfile` | Profile header, finalized flag, phase_mask |
| `get_lca_result(product_id)` | `LcaResult` | Only available after `finalize_lca` |
| `get_lca_uncertainty(product_id)` | `LcaUncertainty` | Only available after `finalize_lca` |
| `compute_lca_summary(product_id)` | `LcaResult` | Alias for `get_lca_result` |
| `get_lca_db_entry(entry_id)` | `LcaDbEntry` | Retrieve a database reference |
| `lca_profile_count()` | `u32` | Total profiles ever registered |

---

## Error Reference

| Code | Name | Description |
|:---:|---|---|
| 38 | `LcaProfileAlreadyExists` | `register_lca_entry` called with a duplicate derived ID |
| 39 | `LcaProfileNotFound` | Product ID not registered |
| 40 | `InvalidLcaPhase` | Phase Symbol not one of the seven accepted values |
| 41 | `InvalidImpactCategory` | Impact/refs/weights vector is not exactly 8 elements |
| 42 | `LcaAlreadyFinalized` | `record_phase_impact` or `finalize_lca` called on a locked profile |
| 43 | `LcaNotFinalized` | `get_lca_result`, `get_lca_uncertainty`, `normalize_impacts`, or `apply_weighting_scheme` called before finalization |
| 44 | `LcaNormRefNotFound` | Named normalization reference set not registered |
| 45 | `LcaWeightingSchemeNotFound` | Named weighting scheme not registered |
| 46 | `LcaDbEntryNotFound` | Database entry ID not registered |

---

## End-to-End Workflow

```
register_lca_db_entry   ←  data providers register source datasets (optional)
        │
register_lca_entry      ←  practitioner registers product + functional unit
        │
record_phase_impact × N ←  submit impacts for each phase (0–6)
        │
finalize_lca            ←  aggregates totals + computes uncertainty intervals
        │
normalize_impacts       ←  (optional) divide by per-person annual burdens
        │
apply_weighting_scheme  ←  (optional) multiply by category weights → single score
        │
get_lca_result /        ←  read results (any caller, anytime after finalize)
compute_lca_summary
```

---

## Worked Example: PET Bottle (1000 units)

### Setup (owner)

```bash
# Register CML 2016 normalization reference (GWP ref = 8.76 × 10^6 kg CO2-eq → 8_760_000_000_000_000 µ-units)
soroban contract invoke -- register_norm_ref \
  --caller <OWNER> --name cml2016 \
  --refs "[8760000000000000,5400000000000,700000000000,63000,12000000000,15000000000,110000000000000,700000000000000]"

# Register equal weighting scheme (8 × 1250 bps = 10000)
soroban contract invoke -- register_weighting_scheme \
  --caller <OWNER> --name equal8 \
  --weights_bps "[1250,1250,1250,1250,1250,1250,1250,1250]"
```

### Register data source

```bash
soroban contract invoke -- register_lca_db_entry \
  --caller <PROVIDER> \
  --db_name "ecoinvent" --version "3.10" \
  --activity "polyethylene terephthalate production, granulate, GLO" \
  --geography "GLO"
# → db_entry_id = <DB_ID>
```

### Register LCA profile

```bash
soroban contract invoke -- register_lca_entry \
  --caller <PRACTITIONER> \
  --name "PET Bottle 500ml" \
  --functional_unit "1000 units, filled and delivered" \
  --material_passport_id null
# → product_id = <PROD_ID>
```

### Submit phase data

```bash
# Raw material: 900 kg CO2-eq = 900_000_000 µ-units GWP; ±5% cv
soroban contract invoke -- record_phase_impact \
  --caller <PRACTITIONER> --product_id <PROD_ID> \
  --phase raw_mat \
  --impacts "[900000000,0,0,0,0,0,0,0]" \
  --cv_bps 500 --db_ref <DB_ID> --metadata ""

# Manufacturing: 400 kg CO2-eq
soroban contract invoke -- record_phase_impact \
  --caller <PRACTITIONER> --product_id <PROD_ID> \
  --phase mfg --impacts "[400000000,0,0,0,0,0,0,0]" --cv_bps 500 ...

# Transport: 80 kg; Use: 10 kg; EoL: 50 kg; Recycling: −120 kg (credit)
```

### Finalize

```bash
soroban contract invoke -- finalize_lca \
  --caller <PRACTITIONER> --product_id <PROD_ID>
# Returns LcaResult.totals[0] = 1_320_000_000 (1320 kg CO2-eq)
# LcaUncertainty.lo[0] = 1_254_000_000 / hi[0] = 1_386_000_000
```

### Normalize and weight

```bash
soroban contract invoke -- normalize_impacts \
  --caller <PRACTITIONER> --product_id <PROD_ID> --norm_ref_name cml2016

soroban contract invoke -- apply_weighting_scheme \
  --caller <PRACTITIONER> --product_id <PROD_ID> --weighting_scheme_name equal8
# → single_score (dimensionless weighted person-equivalent)
```

---

## Integration with Circular Economy Module

LCA profiles can be linked to [Material Passports](circular-economy.md) via `material_passport_id`. This enables cross-module queries:

```
get_lca_profile(product_id)          → profile.material_passport_id
get_material_passport(passport_id)   → passport.total_recycled_mg, loop_event_count, ...
get_circularity_totals()             → MCI, recycling_rate_bps
```

A product with both a material passport and an LCA profile provides the most complete sustainability picture: the LCA quantifies absolute environmental impacts across all categories; the circular economy module tracks actual material flow performance and loop closure.

---

## Known Limitations and Design Choices

**Fixed-point arithmetic.** All values use `i64` with a 10⁻⁶ scale factor. Maximum representable value: ≈ 9.2 × 10¹² SI units (e.g., 9.2 × 10¹² kg CO₂-eq). Values exceeding this saturate. For aggregate studies across many products, consider normalizing before storing.

**Interval arithmetic (not Monte Carlo).** The uncertainty model uses conservative interval propagation — the widest possible bounds — rather than a probabilistic Monte Carlo simulation. This is deterministic and on-chain feasible. Probabilistic bounds would require random number generation (not available in Soroban without an oracle) and are better computed off-chain.

**One result per profile.** `finalize_lca` is irreversible. If data corrections are needed, register a new profile (timestamp will differ, yielding a new ID). The old profile's result remains on-chain for audit purposes.

**System boundary.** The contract does not enforce system boundary completeness. A profile may be finalized with zero phases recorded. This is intentional — partial LCAs (e.g., cradle-to-gate, excluding use and EoL) are valid ISO 14044 studies. The `phase_mask` field signals which phases are present.

**Weighting optionality.** Weighting produces a single score by collapsing all categories into one number, which is scientifically controversial. ISO 14044 requires weighting to be clearly stated and separable from the inventory/characterization results. The contract preserves all intermediate steps (`totals`, `normalized`, `weighted`) so consumers can stop at any stage.
