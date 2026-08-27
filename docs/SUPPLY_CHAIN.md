# Supply Chain Transparency Module

## Overview

The Supply Chain Transparency module provides a comprehensive, immutable ledger for tracking products through their entire lifecycle—from origin through certification, labor practices, and environmental impact. It enables brands to demonstrate integrity and consumers to verify product authenticity and ethical sourcing.

## Core Concepts

### Supply Chain Tracking

Products are tracked through multiple dimensions:

1. **Provenance** — Where products originate and their raw materials source
2. **Certifications** — ISO standards, organic, fair trade, and other third-party validations
3. **Labor Conditions** — Working conditions, wages, safety, and worker rights
4. **Environmental Impact** — Carbon footprint, water usage, waste, and renewable energy
5. **Chain of Custody** — Complete ownership and transfer history

### Verification Model

The system uses a **trust-but-verify** approach:
- Registered certifiers and auditors log data on-chain
- Data is cryptographically sealed with timestamps and signatures
- Consumers can independently verify the chain at any time
- No central authority required for verification

## Data Structures

### Brand

Represents a company or manufacturer:

```rust
pub struct Brand {
    pub brand_id: Symbol,           // Unique identifier (e.g., "ACME")
    pub name: Bytes,                // Display name
    pub owner: Address,             // Stellar address of brand owner
    pub verified: bool,             // Is brand verified by platform
    pub description: Bytes,         // Brand mission/description
    pub website: Bytes,             // Brand website URL
    pub support_contact: Bytes,     // Support contact information
}
```

### ProductSKU

Individual product tracking:

```rust
pub struct ProductSKU {
    pub sku: Bytes,                          // Product SKU/UPC
    pub brand_id: Symbol,                    // Associated brand
    pub product_name: Bytes,                 // Product name
    pub description: Bytes,                  // Product description
    pub provenance_id: BytesN<32>,           // Link to origin event
    pub certifications: Vec<BytesN<32>>,     // Links to certifications
    pub labor_reports: Vec<BytesN<32>>,      // Links to labor audits
    pub environmental_reports: Vec<BytesN<32>>, // Links to environmental reports
    pub created_date: u64,                   // When tracked
    pub last_updated: u64,                   // Last modification
}
```

### Provenance

Tracks product origin and custody:

```rust
pub struct Provenance {
    pub origin_location: Location,          // Factory/origin facility
    pub timestamp: u64,                     // When recorded
    pub raw_material_source: Bytes,         // Where materials come from
    pub producer_address: Address,          // Producer's Stellar address
    pub batch_id: Bytes,                    // Batch/lot number
    pub chain_of_custody: Vec<CustodyTransfer>, // All transfers
    pub is_verified: bool,                  // Verified by authority
}

pub struct CustodyTransfer {
    pub from_address: Address,              // Sender
    pub to_address: Address,                // Recipient
    pub timestamp: u64,                     // When transferred
    pub location: Location,                 // Transfer location
    pub transfer_notes: Bytes,              // Transfer details
}
```

### Certification

Third-party certifications (ISO, organic, fair trade, etc.):

```rust
pub struct Certification {
    pub cert_id: Bytes,                    // Unique cert ID
    pub cert_type: Symbol,                 // Type: ISO_9001, ORGANIC, etc.
    pub issuer: Address,                   // Certifying authority
    pub issued_date: u64,                  // When certified
    pub expiry_date: u64,                  // Expiration date
    pub scope: Bytes,                      // What is certified
    pub is_active: bool,                   // Current status
    pub verification_hash: BytesN<32>,     // Proof of certification
    pub audit_trail: Vec<AuditEntry>,      // Audit history
}
```

### Labor Conditions

Worker welfare and compliance audit:

```rust
pub struct LaborConditions {
    pub facility_id: Bytes,                 // Which facility
    pub report_date: u64,                   // When audited
    pub reporter: Address,                  // Auditing organization
    pub worker_count: u32,                  // Number of workers
    pub wage_compliance: bool,              // Wages meet minimums
    pub working_hours_compliance: bool,     // Hours within legal limits
    pub child_labor_free: bool,             // No child labor
    pub safety_standards_met: bool,         // Safety compliant
    pub freedom_of_association: bool,       // Union rights protected
    pub report_hash: BytesN<32>,            // Detailed report hash
    pub certifications: Vec<Bytes>,         // Associated certifications
}
```

### Environmental Impact

Sustainability reporting:

```rust
pub struct EnvironmentalImpact {
    pub facility_id: Bytes,                 // Which facility
    pub report_period: (u64, u64),          // Start and end dates
    pub carbon_footprint: u32,              // kg CO2e
    pub water_usage: u32,                   // Liters used
    pub waste_generated: u32,               // kg of waste
    pub renewable_energy_percent: u32,      // % renewable
    pub emissions_reduction_percent: u32,   // Year-over-year improvement
    pub certifications: Vec<Bytes>,         // Environmental certs
    pub report_hash: BytesN<32>,            // Detailed report hash
}
```

## API Reference

### Brand Management

#### register_brand()

Register a new brand on the supply chain ledger.

```rust
pub fn register_brand(
    env: &Env,
    owner: Address,
    brand_id: Symbol,
    name: Bytes,
    description: Bytes,
    website: Bytes,
    support_contact: Bytes,
) -> ()
```

**Parameters:**
- `owner` — Brand owner's Stellar address (must authenticate)
- `brand_id` — Unique brand identifier (e.g., Symbol::new("ACME"))
- `name` — Brand display name
- `description` — Brand mission/description
- `website` — Brand website URL
- `support_contact` — Support contact information

**Example:**
```javascript
const owner = "GXXXXX...";
const brandId = "ACME";
const name = Buffer.from("ACME Corporation");
const description = Buffer.from("Quality product manufacturer");
const website = Buffer.from("https://acme.example.com");
const supportContact = Buffer.from("support@acme.example.com");

await contract.invoke({
  method: "register_brand",
  args: [owner, brandId, name, description, website, supportContact],
});
```

#### register_product_sku()

Track a new product SKU for supply chain transparency.

```rust
pub fn register_product_sku(
    env: &Env,
    brand_id: Symbol,
    sku: Bytes,
    product_name: Bytes,
    description: Bytes,
) -> ()
```

**Parameters:**
- `brand_id` — Associated brand ID
- `sku` — Product SKU/UPC code
- `product_name` — Product display name
- `description` — Product description

### Event Logging

#### log_provenance_event()

Record the origin and initial batch information for a product.

```rust
pub fn log_provenance_event(
    env: &Env,
    event_id: BytesN<32>,
    origin_location: Location,
    raw_material_source: Bytes,
    producer: Address,
    batch_id: Bytes,
) -> ()
```

**Parameters:**
- `event_id` — Unique event identifier (hash)
- `origin_location` — Factory/origin facility details
- `raw_material_source` — Source of raw materials
- `producer` — Producer's Stellar address (must authenticate)
- `batch_id` — Batch or lot number

#### log_custody_transfer()

Record a transfer of ownership/custody in the supply chain.

```rust
pub fn log_custody_transfer(
    env: &Env,
    event_id: BytesN<32>,
    from: Address,
    to: Address,
    location: Location,
    notes: Bytes,
) -> ()
```

**Parameters:**
- `event_id` — Associated provenance event ID
- `from` — Previous owner (must authenticate)
- `to` — New owner
- `location` — Transfer location
- `notes` — Transfer details (transport method, duration, etc.)

#### log_certification()

Record a third-party certification (ISO, organic, fair trade, etc.).

```rust
pub fn log_certification(
    env: &Env,
    cert_id: Bytes,
    cert_type: Symbol,
    issuer: Address,
    expiry_days: u64,
    scope: Bytes,
) -> ()
```

**Parameters:**
- `cert_id` — Unique certification ID
- `cert_type` — Type of certification (e.g., Symbol::new("ISO_9001"))
- `issuer` — Certifying authority (must authenticate)
- `expiry_days` — Days until expiration
- `scope` — What is certified (products, processes, etc.)

#### log_labor_conditions()

Record a labor conditions audit for a facility.

```rust
pub fn log_labor_conditions(
    env: &Env,
    facility_id: Bytes,
    worker_count: u32,
    wage_compliant: bool,
    hours_compliant: bool,
    child_labor_free: bool,
    safety_met: bool,
    freedom_of_association: bool,
    report_hash: BytesN<32>,
    reporter: Address,
) -> ()
```

**Parameters:**
- `facility_id` — Which facility was audited
- `worker_count` — Number of workers
- `wage_compliant` — Wages meet legal minimums
- `hours_compliant` — Working hours within legal limits
- `child_labor_free` — No child labor present
- `safety_met` — Safety standards met
- `freedom_of_association` — Union rights protected
- `report_hash` — SHA-256 of detailed report (stored off-chain)
- `reporter` — Auditing organization (must authenticate)

#### log_environmental_impact()

Record environmental impact data for a facility.

```rust
pub fn log_environmental_impact(
    env: &Env,
    facility_id: Bytes,
    report_period_start: u64,
    report_period_end: u64,
    carbon_footprint: u32,
    water_usage: u32,
    waste_generated: u32,
    renewable_energy_percent: u32,
    emissions_reduction: u32,
    report_hash: BytesN<32>,
    reporter: Address,
) -> ()
```

**Parameters:**
- `facility_id` — Which facility was audited
- `report_period_start` — Report period start timestamp
- `report_period_end` — Report period end timestamp
- `carbon_footprint` — Carbon footprint in kg CO2e
- `water_usage` — Water usage in liters
- `waste_generated` — Waste in kg
- `renewable_energy_percent` — Percentage of renewable energy (0-100)
- `emissions_reduction` — Year-over-year reduction percentage
- `report_hash` — SHA-256 of detailed environmental report
- `reporter` — Environmental auditor organization (must authenticate)

### Verification & Queries

#### verify_product_chain()

Verify a product's complete supply chain compliance.

```rust
pub fn verify_product_chain(
    env: &Env,
    brand_id: Symbol,
    sku: Bytes,
) -> SupplyChainVerification
```

**Returns:**
```rust
pub struct SupplyChainVerification {
    pub product_sku: Bytes,
    pub is_verified: bool,              // Overall verification result
    pub provenance_verified: bool,      // Origin verified
    pub certifications_valid: bool,     // All certs active
    pub labor_compliant: bool,          // Labor conditions acceptable
    pub environmental_standards_met: bool,
    pub verification_timestamp: u64,
    pub verification_score: u32,        // 0-100 compliance score
    pub issues: Vec<Bytes>,             // Any problems found
}
```

**Example:**
```javascript
const verification = await contract.call("verify_product_chain", 
  ["ACME", Buffer.from("SKU-12345")]);

console.log(`Product verified: ${verification.is_verified}`);
console.log(`Compliance score: ${verification.verification_score}`);
console.log(`Issues: ${verification.issues}`);
```

#### verify_certification()

Check if a specific certification is valid and current.

```rust
pub fn verify_certification(
    env: &Env,
    cert_id: Bytes,
) -> bool
```

**Parameters:**
- `cert_id` — Certification ID to verify

**Returns:** `true` if certification is active and not expired, `false` otherwise

#### get_product_timeline()

Get a consumer-friendly timeline of product events.

```rust
pub fn get_product_timeline(
    env: &Env,
    event_ids: Vec<BytesN<32>>,
) -> Vec<TimelineEntry>
```

**Returns:**
```rust
pub struct TimelineEntry {
    pub timestamp: u64,
    pub entry_type: Symbol,     // origin, custody, certification, etc.
    pub location: Option<Location>,
    pub description: Bytes,
    pub verified: bool,
    pub event_id: BytesN<32>,
}
```

#### get_brand_integrity_report()

Generate a brand integrity report showing overall compliance and transparency.

```rust
pub fn get_brand_integrity_report(
    env: &Env,
    brand_id: Symbol,
) -> BrandIntegrityReport
```

**Returns:**
```rust
pub struct BrandIntegrityReport {
    pub brand_id: Symbol,
    pub report_date: u64,
    pub total_products_tracked: u32,
    pub avg_compliance_score: u32,      // 0-100
    pub certifications_count: u32,
    pub facilities_audited: u32,
    pub products_fully_traceable: u32,
    pub products_verified: u32,
    pub quality_issues: u32,
    pub compliance_trend: Symbol,       // improving, stable, declining
}
```

### Utility Functions

#### generate_qr_code_url()

Generate a QR code URL for product verification.

```rust
pub fn generate_qr_code_url(
    env: &Env,
    brand_id: Symbol,
    sku: Bytes,
    base_url: Bytes,
) -> Bytes
```

**Returns:** URL that can be encoded as QR code

#### generate_integrity_proof()

Generate a cryptographic proof of supply chain integrity.

```rust
pub fn generate_integrity_proof(
    env: &Env,
    event_ids: Vec<BytesN<32>>,
) -> BytesN<32>
```

**Returns:** SHA-256 hash of all event IDs, proving the chain is complete and unmodified

## Use Cases

### 1. Ethical Sourcing Verification

A consumer can verify that their coffee was:
- Sourced from a fair-trade certified farm
- Transported through verified custody chain
- Processed at a labor-compliant facility
- Packaged with minimal environmental impact

### 2. Quality Assurance

A retailer can verify:
- Product authenticity and batch numbers
- All required certifications are current
- Storage conditions tracked through custody chain
- No counterfeits in the supply chain

### 3. Compliance Reporting

A brand can demonstrate:
- Supplier audits and labor conditions
- Environmental impact metrics
- Third-party certifications
- Continuous improvement trends

### 4. Incident Response

In case of product recall or quality issue:
- Trace exact batch through entire supply chain
- Identify affected facilities and regions
- Contact all downstream holders
- Prove remediation actions

## Error Handling

The module defines specific error codes:

| Code | Error | Description |
|------|-------|-------------|
| 1001 | BrandNotRegistered | Brand does not exist |
| 1002 | SkuNotFound | Product SKU not found |
| 1003 | CertificationExpired | Certification is no longer valid |
| 1004 | InvalidLaborReport | Labor conditions data missing |
| 1005 | InvalidEnvironmentalData | Environmental report data missing |
| 1006 | VerificationFailed | Product chain verification failed |
| 1007 | IncompleteProvenance | Provenance trace incomplete |
| 1008 | UnverifiedCertification | Certification not verified |
| 1009 | UnauthorizedBrandAccess | Insufficient permissions |
| 1010 | InvalidChainOfCustody | Custody transfers invalid |

## Testing

The module includes 19 comprehensive test cases covering:

- Brand registration and product tracking
- Provenance event logging
- Custody transfer chains
- Certification management
- Labor conditions auditing
- Environmental impact tracking
- Product chain verification
- Consumer timeline generation
- Brand integrity reporting
- Full end-to-end scenarios

**Run tests:**
```bash
cargo test supply_chain
```

## Storage Optimization

The module uses persistent storage with keys organized by:

- **Brands**: `Brand(Symbol)` → Brand struct
- **Products**: `ProductSKU(Symbol, Bytes)` → ProductSKU struct
- **Events**: `ProvenanceEvent(BytesN<32>)` → Provenance data
- **Audits**: `LaborReport(BytesN<32>)`, `EnvironmentalReport(BytesN<32>)`
- **Indices**: Brand products, certifications, facility audits for fast lookups

## Security Considerations

1. **Authentication Required**: All logging functions require submitter authentication
2. **Immutability**: Events are append-only and cryptographically sealed
3. **Timestamp Validation**: Events must have recent timestamps
4. **Access Control**: Only brand owners can modify brand data
5. **Certification Expiry**: Automatic validation of active certifications
6. **Content Addressing**: Event IDs derived from content (prevents ID collision)

## Future Enhancements

- Batch verification for multiple products
- Automated compliance scoring algorithms
- Integration with external data sources (APIs)
- Blockchain bridge for cross-ledger verification
- Consumer feedback and ratings system
- Automated alerts for policy violations
- Advanced analytics and trend reporting
