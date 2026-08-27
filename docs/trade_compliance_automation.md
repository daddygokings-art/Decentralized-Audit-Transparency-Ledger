# Trade Compliance Automation Module

## Overview

Comprehensive trade compliance automation framework implementing HS (Harmonized System) classification, origin determination, FTA qualification, customs valuation, license management, customs broker integration, and AEO (Authorized Economic Operator) certification support for streamlined cross-border trade operations.

## Standards & Frameworks

### HS Code (Harmonized System)
- 6-12 digit commodity classification
- Duty rate assignment
- Product category classification
- Unit of measure standardization

### Rules of Origin (ROO)
- Originating vs. non-originating goods
- Value content calculations
- Regional cumulation rules
- Substantial transformation tests

### Free Trade Agreements (FTAs)
- USMCA (US-Mexico-Canada)
- CPTPP (Comprehensive and Progressive TPP)
- RCEP (Regional Comprehensive Economic Partnership)
- EU Free Trade Agreements
- Preference margin calculations

### Customs Valuation
- Transaction value method
- Identical goods method
- Similar goods method
- Deductive value method
- Computed value method
- Fallback methods

### AEO Certification (WCO Standard)
- C-TPAT equivalent programs
- Security level tiers (basic, standard, enhanced)
- Compliance record tracking
- Audit history maintenance

## Core Features

### 1. HS Code Classification ✅
- Register and retrieve HS codes
- Duty rate assignment
- Product category management
- Unit of measure tracking
- Classification notes

### 2. Origin Determination ✅
- Originating/non-originating classification
- Value content calculation
- Regional cumulation support
- Origin history tracking

### 3. FTA Qualification ✅
- Multi-FTA support (USMCA, CPTPP, RCEP, EU)
- Preference margin calculation
- Rules of Origin verification
- Certificate of Origin requirement tracking

### 4. Customs Valuation ✅
- 5 valuation methods (transaction, identical, similar, deductive, computed)
- Adjustment tracking (freight, insurance, packing)
- Dutiable value calculation
- Method documentation

### 5. License Management ✅
- Import/export license issuance
- Product category authorization
- Country authorization
- Validity period tracking
- Status management

### 6. Customs Broker Integration ✅
- Broker registration and licensing
- Country authorization matrix
- License validity tracking
- AEO certification status
- Active/inactive status management

### 7. AEO Certification ✅
- WCO C-TPAT equivalent
- Security level tiers
- Compliance record tracking
- Audit history maintenance
- Certification status tracking

### 8. Certificate of Origin ✅
- CoO issuance
- Exporter/importer tracking
- FTA linkage
- Certification number assignment
- Timestamp and signature verification

### 9. Duty Calculation ✅
- Base duty rate application
- FTA preferential rate calculation
- Duty savings determination
- Alternative duty calculation
- Compliance audit trail

## API Reference

### HS Code Management
```rust
pub fn register_hs_code(hs_code, description, category, unit, duty_rate, note) -> code_id
pub fn get_hs_code(code_id) -> HSCodeClassification
```

### Origin Determination
```rust
pub fn determine_origin(shipment_id, product, hs_code, country, origin_type, value_content) -> origin_id
pub fn get_origin(origin_id) -> OriginDetermination
```

### FTA Qualification
```rust
pub fn qualify_for_fta(shipment_id, fta_name, exporter_country, importer_country, hs_code, qualifies, roo_satisfied) -> fta_id
pub fn get_fta_qualification(fta_id) -> FTAQualification
```

### Customs Valuation
```rust
pub fn valuate_for_customs(shipment_id, invoice_price, currency, method, adjustments) -> valuation_id
pub fn get_valuation(valuation_id) -> CustomsValuation
```

### License Management
```rust
pub fn issue_trade_license(license_number, holder, categories, countries, validity_days) -> license_id
pub fn get_license(license_id) -> TradeLicense
```

### Customs Broker
```rust
pub fn register_customs_broker(broker_address, name, license_number, countries, validity_days) -> broker_id
pub fn get_broker(broker_id) -> CustomsBroker
```

### AEO Certification
```rust
pub fn certify_aeo(entity, name, cert_type, security_level, validity_days) -> aeo_id
pub fn get_aeo_certification(aeo_id) -> AEOCertification
```

### Certificate of Origin
```rust
pub fn issue_certificate_of_origin(shipment_id, importer, hs_code, country, fta_name, cert_number) -> coo_id
pub fn get_certificate_of_origin(coo_id) -> CertificateOfOrigin
```

### Duty Calculation
```rust
pub fn calculate_duty(shipment_id, dutiable_value, base_rate, fta_rate) -> duty_id
pub fn get_duty_calculation(duty_id) -> DutyCalculation
```

### Statistics
```rust
pub fn get_trade_compliance_stats() -> (hs_codes, licenses, brokers, aeos, trades)
```

## Data Structures

### HSCodeClassification
- hs_code: e.g., "8471.30.00"
- product_description
- product_category
- unit_of_measure
- base_duty_rate
- classification_notes

### OriginDetermination
- shipment_id
- country_of_origin (ISO 3166-1 alpha-2)
- origin_type (0=fully_originating, 1=cumulation, 2=non_originating)
- value_content (% regional value)

### FTAQualification
- shipment_id
- fta_name (USMCA, CPTPP, RCEP, EU)
- exporter_country
- importer_country
- qualifies (bool)
- preference_margin (duty savings)
- certificate_of_origin_required (bool)
- roo_satisfied (bool)

### CustomsValuation
- shipment_id
- invoice_price
- currency
- valuation_method (1-5)
- adjustments (freight, insurance, etc.)
- dutiable_value

### TradeLicense
- license_number
- holder (Address)
- product_categories
- countries_authorized
- issued_date
- expiration_date
- status (0=active, 1=suspended, 2=revoked)

### CustomsBroker
- broker_address
- broker_name
- license_number
- countries_authorized
- license_issued
- license_expiration
- aeo_certified (bool)
- is_active (bool)

### AEOCertification
- entity (Address)
- entity_name
- certification_type (C-TPAT, EORI, AEO_F)
- security_level (1=basic, 2=standard, 3=enhanced)
- certified_date
- expiration_date
- compliance_record
- audit_history
- status (0=active, 1=suspended, 2=revoked)

### CertificateOfOrigin
- shipment_id
- exporter (Address)
- importer (Address)
- product_hs_code
- country_of_origin
- fta_name
- issued_date
- certification_number

### DutyCalculation
- shipment_id
- dutiable_value
- base_duty_rate
- calculated_duty
- fta_duty_rate
- fta_duty
- duty_savings

## Error Codes

| Code | Error | Scenario |
|------|-------|----------|
| 4000 | HSCodeNotFound | HS code not found or invalid |
| 4001 | OriginDeterminationFailed | Origin determination failed |
| 4002 | FTAQualificationFailed | Product doesn't qualify for FTA |
| 4003 | CustomsValuationError | Valuation method invalid |
| 4004 | LicenseRequired | License required but not present |
| 4005 | BrokerNotAuthorized | Broker not authorized |
| 4006 | AEOCertificationInvalid | AEO cert expired or invalid |
| 4007 | CertificateOfOriginRequired | CoO required |
| 4008 | RulesOfOriginNotSatisfied | ROO not met |
| 4009 | PreferenceMarginInsufficient | Preference margin insufficient |
| 4010 | TariffClassificationDisputed | Classification disputed |
| 4011 | DutyCalculationError | Duty calculation error |
| 4012 | BrokerLicenseExpired | Broker license expired |

## Usage Examples

### Example 1: Register HS Code
```rust
TradeCompliance::register_hs_code(
    env, owner,
    b"8471.30.00",
    b"Portable automatic data processing machines",
    b"Electronics",
    b"UNIT",
    500,  // 5% duty
    b"Computers",
);
```

### Example 2: Determine Origin
```rust
TradeCompliance::determine_origin(
    env, trader,
    shipment_id,
    b"Computers",
    b"8471.30.00",
    b"US",
    0,   // fully originating
    100, // 100% value content
);
```

### Example 3: Qualify for FTA
```rust
TradeCompliance::qualify_for_fta(
    env, trader,
    shipment_id,
    b"USMCA",
    b"US",
    b"MX",
    b"6204.62.00",
    true,  // qualifies
    true,  // ROO satisfied
);
```

### Example 4: Calculate Duty with FTA
```rust
TradeCompliance::calculate_duty(
    env, trader,
    shipment_id,
    10000u64,  // $100.00
    500,       // 5% base duty
    200,       // 2% FTA rate
);
// Result: $5.00 base duty vs $2.00 FTA duty = $3.00 savings
```

## Integration with Audit Ledger

All trade compliance events logged to main Audit Ledger:

```rust
AuditLedger::log_event(env, actor, Symbol::new(&env, "hs_code_registered"), data);
AuditLedger::log_event(env, actor, Symbol::new(&env, "fta_qualified"), data);
AuditLedger::log_event(env, actor, Symbol::new(&env, "duty_calculated"), data);
```

## Performance

### Storage Efficiency
- HS Code: ~256 bytes
- Origin determination: ~384 bytes
- FTA qualification: ~512 bytes
- Customs valuation: ~320 bytes
- Trade license: ~448 bytes
- Customs broker: ~512 bytes
- AEO certification: ~640 bytes
- Certificate of Origin: ~512 bytes
- Duty calculation: ~384 bytes

### Computational Complexity
- Most operations: O(1)
- Statistics gathering: O(1) counter reads

## Best Practices

1. **HS Classification** — Verify classification against official HS nomenclature
2. **Origin Verification** — Document supply chain for originating goods
3. **FTA Documentation** — Maintain CoO and supporting documents
4. **Valuation Method** — Use transaction value when available
5. **Broker Licensing** — Verify broker licenses and AEO status
6. **Record Retention** — Maintain 3-7 year record retention per jurisdiction
7. **Audit Trail** — Track all classification and duty decisions
8. **Compliance Updates** — Monitor HS and FTA changes

## Future Enhancements

- [ ] Real-time HS nomenclature updates
- [ ] Machine learning for classification
- [ ] Automated ROO validation
- [ ] Multi-currency exchange rate integration
- [ ] Broker performance analytics
- [ ] Predictive duty calculations
- [ ] Blockchain-verified CoO
- [ ] Digital ledger integration with customs agencies
