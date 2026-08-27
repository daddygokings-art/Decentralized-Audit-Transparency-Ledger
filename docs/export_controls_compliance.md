# Export Controls & Sanctions Compliance Module

## Overview

Comprehensive export controls and sanctions compliance framework implementing OFAC (Office of Foreign Assets Control), EU sanctions, UN Security Council restrictions, and BIS (Bureau of Industry and Security) export control regulations. Includes denied party screening, end-use checks, license determination, re-export controls, controlled commodities tracking, and automated screening with real-time risk flagging.

## Regulatory Framework

### OFAC (Office of Foreign Assets Control)
- **SDN List** — Specially Designated Nationals and Blocked Persons
- **SSI List** — Sectoral Sanctions Identifications
- **Consolidated Sanctions List** — All OFAC-designated entities
- **Enforcement** — OFAC civil penalties up to $250K+ per violation, criminal up to $1M

### EU Sanctions
- **Consolidated List** — Combined EU sanctions and restrictive measures
- **Countries and Persons** — Individual and entity listings
- **Entities** — Designated organizations and institutions
- **Penalties** — Up to 10% of global turnover for corporate violations

### UN Security Council
- **UNSC Sanctions Lists** — 14 different UN sanctions regimes
- **Al-Qaeda/ISIS Lists** — Terrorist designation lists
- **DPRK, Iran, Syria** — Country-specific measures
- **Asset Freezes** — Comprehensive property freezing orders

### BIS (Bureau of Industry and Security)
- **EAR (Export Administration Regulations)** — Dual-use item controls
- **Commerce Control List (CCL)** — Classified commodities (encryption, semiconductors, advanced manufacturing)
- **License Requirements** — Determines when licenses needed
- **Country Groups** — Destination classifications (A, B, D, E)

### DDTC (Directorate of Defense Trade Controls)
- **ITAR (International Traffic in Arms Regulations)** — Defense articles and services
- **Munitions List** — Military items and components
- **Deemed Exports** — Transfer of controlled technology to foreign nationals

## Core Features

### 1. Denied Party Screening ✅
- OFAC/EU/UN list integration
- Name-based matching with confidence scoring
- Alternative name matching
- Address-based verification
- Automatic flagging on matches
- Multi-list cross-referencing

### 2. Export License Management ✅
- License issuance and tracking
- Validity period enforcement
- Commodity authorization lists
- Destination country restrictions
- End-user verification
- License status tracking (active/suspended/revoked/expired)

### 3. Controlled Commodity Registry ✅
- ECCN (Export Control Classification Number) tracking
- HS Code classification
- Technical data restriction flags
- Encryption level limits
- Deemed export identification
- Commodity-specific restrictions

### 4. End-Use Checks ✅
- Declared end-use verification
- Suspicious end-use detection (military, nuclear, weapons, missiles)
- Military-use classification
- Dual-use item flagging
- End-user legitimacy verification
- Result escalation for high-risk

### 5. Re-Export Controls ✅
- Re-export transaction recording
- Original license verification
- New destination screening
- Authorization requirement determination
- Re-export approval workflow
- Tracking of goods in transit

### 6. Automated Screening ✅
- Multi-factor screening (denied parties, commodities, destinations, end-use)
- Risk score calculation (0-100)
- Automatic flag generation
- Real-time blocking
- Escalation workflow
- Comprehensive audit trail

### 7. Country Classification ✅
- Country Group A — Allies (NATO, Japan, Korea)
- Country Group B — Advanced countries
- Country Group D — Other countries
- Country Group E — Embargo (Cuba, Iran, Syria, DPRK)
- Dynamic classification updates

## API Reference

### Denied Party Management

```rust
pub fn add_denied_party(
    env: Env,
    caller: Address,
    entity_name: Bytes,
    alt_names: Vec<Bytes>,
    address: Bytes,
    country: Bytes,
    authority: u32,
    reason: Bytes,
) -> BytesN<32>

pub fn screen_denied_party(
    env: Env,
    caller: Address,
    party: Address,
    party_name: Bytes,
) -> BytesN<32>

pub fn get_denied_party_match(env: Env, party: Address) -> DeniedPartyMatch
```

### Export License Management

```rust
pub fn issue_export_license(
    env: Env,
    caller: Address,
    exporter: Address,
    license_type: Bytes,
    items_authorized: Vec<Bytes>,
    destination_countries: Vec<Bytes>,
    end_use_statement: Bytes,
    authorized_end_user: Address,
    quantity_limit: u64,
    validity_days: u32,
) -> BytesN<32>

pub fn get_export_license(env: Env, license_id: BytesN<32>) -> ExportLicense

pub fn verify_license(
    env: Env,
    license_id: BytesN<32>,
    commodity: Bytes,
    destination: Bytes,
) -> bool
```

### Controlled Commodity Registry

```rust
pub fn register_commodity(
    env: Env,
    caller: Address,
    name: Bytes,
    eccn: Bytes,
    control_type: Bytes,
    regulated_by: u32,
    restricted_countries: Vec<Bytes>,
    license_requirement: u32,
    technical_data_restricted: bool,
    encryption_level: u32,
    is_deemed_export: bool,
) -> BytesN<32>

pub fn get_commodity(env: Env, commodity_id: BytesN<32>) -> ControlledCommodity
```

### End-Use Checks

```rust
pub fn check_end_use(
    env: Env,
    caller: Address,
    commodity: Bytes,
    declared_end_use: Bytes,
    end_user: Address,
    destination_country: Bytes,
) -> BytesN<32>

pub fn get_end_use_check(env: Env, check_id: BytesN<32>) -> EndUseCheck
```

### Re-Export Controls

```rust
pub fn record_re_export(
    env: Env,
    caller: Address,
    re_exporter: Address,
    original_exporter: Address,
    commodity: Bytes,
    original_destination: Bytes,
    new_destination: Bytes,
    original_license: BytesN<32>,
) -> BytesN<32>

pub fn approve_re_export(env: Env, caller: Address, re_export_id: BytesN<32>)

pub fn get_re_export(env: Env, re_export_id: BytesN<32>) -> ReExportRecord
```

### Automated Screening

```rust
pub fn screen_export(
    env: Env,
    caller: Address,
    exporter: Address,
    commodity: Bytes,
    destination: Bytes,
    end_use: Bytes,
    end_user: Address,
) -> BytesN<32>

pub fn get_screening_result(env: Env, screening_id: BytesN<32>) -> ScreeningResult
```

### Country Classification

```rust
pub fn set_country_group(
    env: Env,
    caller: Address,
    country_code: Bytes,
    group: u32,
)

pub fn get_country_group(env: Env, country_code: Bytes) -> u32
```

### Statistics

```rust
pub fn get_export_controls_stats(env: Env) -> (u32, u32, u32, u32, u32)
// Returns: (denied_parties, screenings, blocked, licenses, commodities)
```

## Data Structures

### DeniedPartyListEntry
```rust
pub struct DeniedPartyListEntry {
    pub id: BytesN<32>,
    pub entity_name: Bytes,
    pub alt_names: Vec<Bytes>,
    pub address: Bytes,
    pub country: Bytes,
    pub authority: u32,
    pub reason: Bytes,
    pub effective_date: u64,
    pub entry_hash: BytesN<32>,
}
```

### ExportLicense
```rust
pub struct ExportLicense {
    pub id: BytesN<32>,
    pub exporter: Address,
    pub license_type: Bytes,
    pub issued_date: u64,
    pub expiration_date: u64,
    pub items_authorized: Vec<Bytes>,
    pub destination_countries: Vec<Bytes>,
    pub end_use_statement: Bytes,
    pub authorized_end_user: Address,
    pub quantity_limit: u64,
    pub status: u32,
    pub license_hash: BytesN<32>,
}
```

### ControlledCommodity
```rust
pub struct ControlledCommodity {
    pub id: BytesN<32>,
    pub name: Bytes,
    pub eccn: Bytes,
    pub control_type: Bytes,
    pub regulated_by: u32,
    pub restricted_countries: Vec<Bytes>,
    pub license_requirement: u32,
    pub technical_data_restricted: bool,
    pub encryption_level: u32,
    pub is_deemed_export: bool,
    pub updated_at: u64,
    pub commodity_hash: BytesN<32>,
}
```

### EndUseCheck
```rust
pub struct EndUseCheck {
    pub id: BytesN<32>,
    pub commodity: Bytes,
    pub declared_end_use: Bytes,
    pub end_user: Address,
    pub destination_country: Bytes,
    pub checked_at: u64,
    pub result: u32,
    pub risk_flags: Vec<Bytes>,
    pub suspicious_patterns: Vec<Bytes>,
    pub check_hash: BytesN<32>,
}
```

### ScreeningResult
```rust
pub struct ScreeningResult {
    pub id: BytesN<32>,
    pub party: Address,
    pub commodity: Bytes,
    pub destination: Bytes,
    pub screened_at: u64,
    pub result: u32,
    pub matches_found: u32,
    pub risk_score: u32,
    pub license_needed: bool,
    pub end_use_check_required: bool,
    pub screening_hash: BytesN<32>,
}
```

## Error Codes

| Code | Error | Scenario |
|------|-------|----------|
| 3000 | DeniedPartyDetected | Party matches OFAC/EU/UN list |
| 3001 | EndUseCheckFailed | End-use check failed |
| 3002 | LicenseRequired | License required but not present |
| 3003 | InvalidLicense | License expired or invalid |
| 3004 | ReExportProhibited | Re-export restricted |
| 3005 | RestrictedDestination | Destination restricted for commodity |
| 3006 | ControlledCommodity | Item is controlled |
| 3007 | SanctionedEndUse | Military/nuclear/weapons use detected |
| 3008 | MultipleListMatches | Party on multiple lists |
| 3009 | TransactionBlocked | Screening blocked transaction |
| 3010 | ScreeningDatabaseUninitialized | Database not ready |
| 3011 | UnknownExportClass | Classification unknown |
| 3012 | DeemedExportProhibited | Deemed export prohibited |
| 3013 | CountryGroupRestricted | Country group restrictions |
| 3014 | EncryptionLevelExceeded | Encryption exceeds limits |

## Usage Examples

### Example 1: Add Denied Party

```rust
ExportControls::add_denied_party(
    env,
    owner,
    b"Sanctioned Entity Ltd",
    vec![b"SEL Inc"],
    b"Tehran, Iran",
    b"IR",
    1,  // OFAC
    b"IRGC-affiliated entity",
);
```

### Example 2: Issue Export License

```rust
let license_id = ExportControls::issue_export_license(
    env,
    owner,
    exporter,
    b"LICENSE-001",
    vec![b"Encryption software"],
    vec![b"GB", b"DE"],
    b"Commercial use only",
    end_user,
    1000u64,
    90, // 90 days
);
```

### Example 3: Register Controlled Commodity

```rust
ExportControls::register_commodity(
    env,
    owner,
    b"Advanced Semiconductor",
    b"3A001",
    b"Advanced Computing",
    4,  // BIS
    vec![b"IR", b"KP"],
    1,  // License required
    true,
    256, // 256-bit encryption
    false,
);
```

### Example 4: Perform End-Use Check

```rust
ExportControls::check_end_use(
    env,
    exporter,
    b"Item X",
    b"Commercial manufacturing",
    end_user,
    b"DE",
);
```

### Example 5: Automated Export Screening

```rust
let screening_id = ExportControls::screen_export(
    env,
    exporter,
    exporter,
    b"Dual-use component",
    b"GB",
    b"Commercial use",
    end_user,
);

let result = ExportControls::get_screening_result(env, screening_id);
```

## Integration with Audit Ledger

All export control activities can be logged to the main Audit Ledger:

```rust
// Log screening
AuditLedger::log_event(env, exporter, Symbol::new(&env, "export_screening"), data);

// Log license issuance
AuditLedger::log_event(env, owner, Symbol::new(&env, "export_license_issued"), data);

// Log denied party detection
AuditLedger::log_event(env, screener, Symbol::new(&env, "denied_party_detected"), data);
```

## Best Practices

1. **Regular Updates** — Update OFAC/EU/UN lists quarterly
2. **Training** — Annual export control training for all export-involved employees
3. **Classification** — Proper ECCN classification before export
4. **Documentation** — Maintain complete export documentation
5. **Verification** — Verify end-users and end-use before approval
6. **Record Keeping** — Maintain 7-year export records
7. **Automation** — Use automated screening for all transactions
8. **Escalation** — Clear process for high-risk items

## Performance

### Storage Efficiency
- Denied party entry: ~384 bytes
- Export license: ~640 bytes
- Controlled commodity: ~512 bytes
- End-use check: ~480 bytes
- Re-export record: ~512 bytes
- Screening result: ~576 bytes

### Computational Complexity
- Most operations: O(1)
- Screening: O(log n) for denied party lookups
- Statistics gathering: O(1) counter reads

## Future Enhancements

- [ ] Real-time OFAC/EU/UN list updates
- [ ] Machine learning for risk scoring
- [ ] Behavioral analysis
- [ ] Encryption strength enforcement
- [ ] Autonomous sanctions integration
- [ ] API sandbox for testing
- [ ] Batch screening support
- [ ] Historical trend analysis
