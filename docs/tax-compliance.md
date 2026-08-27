# Tax Compliance System

## Overview

Comprehensive tax compliance system for:
- **VAT/GST** determination across 10 jurisdictions
- **Digital Services Tax** (DST) applicability and rates
- **Crypto Asset Reporting** (CARF/DAC8)
- **Transfer Pricing** arm's length analysis
- **Country-by-Country Reporting** (CbCR)
- **Audit Trail** and compliance documentation

## Components

### 1. VAT/GST Determination Engine

**Jurisdiction Coverage:**
- EU: 20% standard (0% intra-EU B2B goods)
- UK: 20%
- Canada: 5% GST
- Australia: 10% GST
- India: 5-18% (varies by supply type)
- Singapore: 8%
- Japan: 10%
- UAE: 5%
- Hong Kong: 0% (no VAT)
- US: State-based sales tax

**Features:**
- Jurisdiction-specific rates
- Exemption classification
- Reverse charge detection for B2B cross-border transactions
- Net/gross amount calculations
- Supply type classification (Goods, Services, Digital Services, etc.)

**Example:**
```rust
let determination = VATDeterminationEngine::determine_vat(env, &transaction);
// Returns: VAT rate, amount, reverse charge status, exemption reason
```

### 2. Digital Services Tax Engine

**Supported Jurisdictions:**
- EU (France, Italy, Spain, etc.): 3%
- UK: 2%
- India: 4%
- Australia: 3%

**In-Scope Services:**
- Online advertising
- Online marketplaces
- Social media platforms
- Video streaming services

**Applicability Criteria:**
- Annual global revenue threshold met (€750M-€1B+)
- Service provider meets residency requirements
- Recurring service revenues

**Example:**
```rust
let dst = DSTEngine::determine_dst(env, &transaction);
// Returns: Applicability, rate, jurisdictions, amount
```

### 3. Crypto Asset Reporting (CARF/DAC8)

**Asset Types:**
- Bitcoin
- Ethereum
- Stablecoins
- Utility tokens
- Security tokens
- NFTs
- Altcoins

**Reporting Requirements:**
- CARF (US: Common Reporting Standard)
- DAC8 (EU: Digital Asset Custodian Reporting)
- Annual transaction summaries
- Year-end holdings
- Cost basis and gains/losses

**Cost Basis Methods:**
- FIFO (First-in-first-out)
- Specific identification
- Average cost

**Example:**
```rust
let gains = CryptoReportingEngine::calculate_realized_gain(cost_basis, proceeds);
let report = CryptoReportingEngine::generate_carf_record(
    env, entity, transactions, holdings, year
);
```

### 4. Transfer Pricing Engine

**Supported Methods:**
1. **CUP** (Comparable Uncontrolled Price)
2. **Cost Plus** Method
3. **Resale Price** Method
4. **Profit Split** Method
5. **TNMM** (Transactional Net Margin Method)

**Defensibility Assessment:**
- Arm's length price validation
- Interquartile range analysis (±25%)
- Variance calculation and reporting
- Comparable transaction analysis

**Documentation Requirements:**
- Economic analysis
- Functional analysis
- Business purpose
- Regulatory support

**Example:**
```rust
let analysis = TransferPricingEngine::validate_price(
    transaction_price, 
    &comparable_prices
);
// Returns: Arm's length price, variance, defensibility status
```

### 5. Country-by-Country Reporting (CbCR)

**Data Aggregation by Jurisdiction:**
- Revenue (related + unrelated parties)
- Profit/loss
- Income tax paid
- Employee count
- Tangible assets
- Entity information

**Reporting Standards:**
- BEPS Action 13 (OECD)
- Base Erosion and Profit Shifting
- Transfer pricing alignment

**Filing Requirements:**
- Annual submission
- Jurisdiction-by-jurisdiction breakdown
- Effective tax rate disclosure
- Related party transaction summary

**Example:**
```rust
let report = CbCREngine::generate_cbcr(
    env,
    parent_entity,
    fiscal_year,
    jurisdictions
);
// Returns: Aggregated report with totals and ETR
```

## REST API Endpoints

### VAT/GST

```
POST /tax/vat-determination
Body: {
  supplierId, customerId, supplyType, amount, currency,
  placeOfSupply, customerJurisdiction, isB2B, exemptionReason
}
Returns: { vatRate, vatAmount, grossAmount, reverseChargeApplies, isExempt }
```

### Digital Services Tax

```
POST /tax/dst-determination
Body: {
  providerId, serviceCategory, revenue, currency, 
  userJurisdiction, annualRevenueThreshold, fiscalYearEnd
}
Returns: { isApplicable, dstRate, dstAmount, jurisdictions }
```

### Crypto Reporting

```
POST /tax/crypto-reporting
Body: {
  holderId, reportingYear, cryptoHoldings[], transactions[]
}
Returns: { totalGains, totalLosses, yearEndValue, status }
```

### Transfer Pricing

```
POST /tax/transfer-pricing-analysis
Body: {
  transferorId, transfereeId, amount, method, comparables[], fiscalYear
}
Returns: { armsLengthPrice, variance, defensible, adjustmentNeeded }
```

### Country-by-Country

```
POST /tax/cbcr-report
Body: {
  parentEntityId, fiscalYear, jurisdictions[]
}
Returns: { totals, jurisdictions, ETR, reportingStandard }
```

## Audit Trail and Documentation

### Tax Audit Events

Tracked events:
- VAT determination
- DST calculation
- Crypto transaction
- Transfer pricing analysis
- CbCR filing
- Exemption claims
- Compliance status changes

### Documentation Management

- Document tracking with content hashes
- Retention period management (6-10 years)
- Filing status tracking
- Authority reference linking
- Exemption certification

### Compliance Events

- Filing deadlines
- Audit status tracking
- Compliance requirements
- Outstanding liabilities
- Risk scoring

## Portal Features

### Dashboard
- Compliance status overview
- Deadline tracking
- Risk score assessment
- Outstanding liabilities

### VAT/GST Calculator
- Real-time rate determination
- Reverse charge detection
- Net/gross calculations
- Exemption checking

### DST Calculator
- Threshold verification
- Rate application
- Jurisdiction-specific rules

### Crypto Reporting Tool
- Transaction tracking
- Gain/loss calculation
- FIFO method support
- CARF/DAC8 report generation

### Transfer Pricing Analyzer
- Comparable price analysis
- Arm's length validation
- Defensibility assessment
- Documentation generation

### CbCR Generator
- Jurisdiction aggregation
- Revenue reconciliation
- Tax computation verification
- ETR analysis

## Data Structures

### VATTransaction
- Transaction ID and participants
- Supply type and amount
- Jurisdiction information
- B2B vs B2C classification
- Exemption claims

### DSTTransaction
- Service provider and category
- Revenue and thresholds
- User jurisdiction
- Fiscal year details

### CryptoTransaction
- Holder and counterparty
- Asset type and amount
- FMV in reporting currency
- Transaction type (buy, sell, stake)
- Holding period
- Cost basis

### TransferPricingDoc
- Related party transaction
- Transfer amount and description
- Methodology used
- Comparable transactions
- Economic analysis

### CbCRJurisdictionData
- Jurisdiction identifier
- Revenue (related/unrelated)
- Profit/loss and tax paid
- Employee count
- Tangible assets
- Entities operating

## Compliance Standards

### ISA 3000
- Assurance engagement standards
- Audit documentation
- Materiality assessment

### SOC2
- Service organization controls
- Trust service principles
- Control effectiveness

### CARF
- US crypto asset reporting
- Annual information return
- Form 8949 alignment

### DAC8
- EU digital asset custodian reporting
- Beneficial ownership identification
- Transaction documentation

### BEPS Action 13
- Transfer pricing documentation
- CbCR Master File
- Local File requirements

## Security & Audit

### Immutable Audit Trail
- All tax decisions logged
- Timestamp verification
- Actor identification
- Decision reasoning documentation

### Cryptographic Verification
- Transaction integrity proofs
- Hash chain validation
- Digital signatures

### Compliance Verification
- Automated rule validation
- Threshold checking
- Deadline tracking
- Exception reporting

## Testing

Comprehensive test suite with 35+ test cases:

**VAT/GST Tests:**
- Jurisdiction-specific rates (10 jurisdictions)
- Reverse charge detection
- B2B vs B2C rules
- Amount calculations

**DST Tests:**
- Service category classification
- Threshold verification
- Rate determination by jurisdiction

**Crypto Tests:**
- Gain/loss calculations
- FIFO method
- Reportability assessment

**Transfer Pricing Tests:**
- Price defensibility
- Variance analysis
- Method selection

**CbCR Tests:**
- Jurisdiction aggregation
- Revenue reconciliation
- ETR calculation

**Audit Trail Tests:**
- Event logging
- Documentation creation
- Exemption management

## Code Statistics

| Component | Lines | Files | Status |
|-----------|-------|-------|--------|
| Tax Data Structures | 468 | tax.rs | ✅ |
| VAT/GST Engine | 397 | vat_engine.rs | ✅ |
| Tax Engines (DST, Crypto, TP, CbCR) | 352 | tax_engines.rs | ✅ |
| Audit Trail | 419 | tax_audit_trail.rs | ✅ |
| REST API | 472 | api/rest/src/tax.ts | ✅ |
| Portal UI | 586 | ui/src/app/tax/page.tsx | ✅ |
| Tests | 509 | tax_tests.rs | ✅ |
| **Total** | **3,203** | **7** | **✅** |

## Deployment

### Prerequisites
- Rust with Soroban SDK
- Node.js 20+
- PostgreSQL for documentation storage

### Build
```bash
cargo build --target wasm32-unknown-unknown --release
cd api/rest && npm install && npm run build
cd ui && npm install && npm run build
```

### Run Services
```bash
docker compose up
# API: http://localhost:3002/tax
# Portal: http://localhost:3001/tax
```

## Future Enhancements

- [ ] Real-time tax rate updates from authorities
- [ ] Machine learning for tax optimization
- [ ] Advanced transfer pricing comparables database
- [ ] Automated compliance audit
- [ ] Multi-currency support with FX tracking
- [ ] Blockchain-based documentation proof
- [ ] RegTech partner integrations
- [ ] Advanced analytics and reporting
- [ ] Tax controversy management
- [ ] Audit defense automation

## References

- [OECD Transfer Pricing Guidelines](https://www.oecd.org/tax/transfer-pricing/)
- [BEPS Action 13: Transfer Pricing Documentation](https://www.oecd.org/ctp/beps-action-13-transfer-pricing-documentation-and-country-by-country-reporting.htm)
- [EU VAT Directive](https://ec.europa.eu/taxation_customs/business/vat_en)
- [Digital Services Tax Guidelines](https://ec.europa.eu/taxation_customs/business/vat_en)
- [CARF Reporting Requirements](https://www.irs.gov/pub/irs-drop/rr-21-04.pdf)
- [DAC8 Directive](https://ec.europa.eu/taxation_customs/business/vat_en)

