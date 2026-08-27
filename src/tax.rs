//! Tax Compliance Data Structures and Enums
//!
//! Comprehensive data types for:
//! - VAT/GST determination
//! - Digital Services Tax
//! - Crypto asset reporting (CARF, DAC8)
//! - Transfer pricing
//! - Country-by-country reporting

use soroban_sdk::{contracttype, Symbol, Bytes, Vec, Address, BytesN};

/// VAT/GST Tax Jurisdiction
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TaxJurisdiction {
    EU = 0,
    UK = 1,
    US = 2,
    Canada = 3,
    Australia = 4,
    India = 5,
    Singapore = 6,
    HongKong = 7,
    Japan = 8,
    Switzerland = 9,
    UAE = 10,
}

/// Digital Service Category for DST
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DigitalServiceCategory {
    OnlineAdvertising = 0,
    OnlineMarketplace = 1,
    SocialMedia = 2,
    VideoStreaming = 3,
    MusicStreaming = 4,
    CloudServices = 5,
    DataServices = 6,
    OnlineSearch = 7,
}

/// Crypto Asset Type for CARF/DAC8 Reporting
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CryptoAssetType {
    Bitcoin = 0,
    Ethereum = 1,
    Stablecoin = 2,
    UtilityToken = 3,
    SecurityToken = 4,
    NFT = 5,
    OtherAltcoin = 6,
}

/// Transfer Pricing Method
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TransferPricingMethod {
    /// Comparable Uncontrolled Price
    CUP = 0,
    /// Cost Plus Method
    CostPlus = 1,
    /// Resale Price Method
    ResalePrice = 2,
    /// Profit Split Method
    ProfitSplit = 3,
    /// Transactional Net Margin Method (TNMM)
    TNMM = 4,
}

/// VAT/GST Supply Classification
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VATSupplyType {
    Goods = 0,
    Services = 1,
    Intangibles = 2,
    DigitalServices = 3,
    Construction = 4,
    Transportation = 5,
    Telecommunications = 6,
}

/// VAT/GST Exemption Reason
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VATExemptionReason {
    Financial = 0,
    Healthcare = 1,
    Education = 2,
    ExportedGoods = 3,
    IntraEUSupply = 4,
    CulturalActivities = 5,
    AgriculturalProduction = 6,
    None = 7,
}

/// VAT/GST Transaction Details
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VATTransaction {
    /// Transaction ID
    pub id: BytesN<32>,
    /// Supplier address
    pub supplier: Address,
    /// Customer address
    pub customer: Address,
    /// Supply type
    pub supply_type: VATSupplyType,
    /// Supply amount
    pub supply_amount: u64,
    /// Currency code (e.g., "EUR", "GBP")
    pub currency: Symbol,
    /// Transaction timestamp
    pub timestamp: u64,
    /// Place of supply jurisdiction
    pub place_of_supply: TaxJurisdiction,
    /// Customer jurisdiction
    pub customer_jurisdiction: TaxJurisdiction,
    /// Is B2B transaction
    pub is_b2b: bool,
    /// Is reverse charge applicable
    pub reverse_charge: bool,
    /// VAT exemption reason
    pub exemption_reason: VATExemptionReason,
    /// Applicable VAT rate (in basis points, e.g., 2000 = 20%)
    pub vat_rate: u32,
    /// VAT amount
    pub vat_amount: u64,
    /// Transaction description
    pub description: Bytes,
}

/// VAT/GST Determination Result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VATDetermination {
    /// Transaction reference
    pub transaction_id: BytesN<32>,
    /// Applicable VAT rate (basis points)
    pub vat_rate: u32,
    /// Is supply exempt
    pub is_exempt: bool,
    /// Exemption reason if applicable
    pub exemption_reason: Option<VATExemptionReason>,
    /// Is reverse charge applicable
    pub reverse_charge_applicable: bool,
    /// Place of supply
    pub place_of_supply: TaxJurisdiction,
    /// Calculated VAT amount
    pub vat_amount: u64,
    /// Determination timestamp
    pub determined_at: u64,
    /// Authority/rule source
    pub source: Bytes,
}

/// Digital Services Tax Transaction
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DSTTransaction {
    /// Transaction ID
    pub id: BytesN<32>,
    /// Service provider
    pub provider: Address,
    /// Service type
    pub service_category: DigitalServiceCategory,
    /// Service revenue
    pub revenue: u64,
    /// Currency
    pub currency: Symbol,
    /// User jurisdiction
    pub user_jurisdiction: TaxJurisdiction,
    /// Fiscal year end
    pub fiscal_year_end: u64,
    /// Annual revenue threshold met
    pub threshold_met: bool,
    /// Is recurring service
    pub recurring_service: bool,
    /// Transaction timestamp
    pub timestamp: u64,
}

/// DST Determination Result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DSTDetermination {
    /// Transaction reference
    pub transaction_id: BytesN<32>,
    /// Is DST applicable
    pub is_applicable: bool,
    /// DST rate (basis points, e.g., 300 = 3%)
    pub dst_rate: u32,
    /// Applicable jurisdictions
    pub jurisdictions: Vec<TaxJurisdiction>,
    /// DST amount (for this jurisdiction)
    pub dst_amount: u64,
    /// Determination basis
    pub basis: Bytes,
    /// Determined at timestamp
    pub determined_at: u64,
}

/// Crypto Transaction for CARF/DAC8 Reporting
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CryptoTransaction {
    /// Transaction ID (hash or blockchain tx ID)
    pub id: BytesN<32>,
    /// Account holder
    pub account_holder: Address,
    /// Counterparty address (if known)
    pub counterparty: Option<Address>,
    /// Asset type
    pub asset_type: CryptoAssetType,
    /// Amount of asset
    pub amount: u64,
    /// Fair market value in reporting currency
    pub fair_market_value: u64,
    /// Reporting currency code
    pub currency: Symbol,
    /// Transaction type (buy, sell, transfer, stake, etc.)
    pub transaction_type: Symbol,
    /// Date of transaction
    pub transaction_date: u64,
    /// Holder jurisdiction
    pub holder_jurisdiction: TaxJurisdiction,
    /// Cost basis if purchase
    pub cost_basis: Option<u64>,
    /// Holding period in days
    pub holding_period_days: u32,
    /// Acquisition date
    pub acquisition_date: u64,
}

/// Crypto Asset Holdings
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CryptoHolding {
    /// Holder address
    pub holder: Address,
    /// Asset type
    pub asset_type: CryptoAssetType,
    /// Current balance
    pub balance: u64,
    /// Fair market value
    pub fair_market_value: u64,
    /// Holder jurisdiction
    pub holder_jurisdiction: TaxJurisdiction,
    /// Acquisition date of current batch (if FIFO)
    pub acquisition_date: u64,
    /// Cost basis of current batch
    pub cost_basis: u64,
    /// Date of valuation
    pub valuation_date: u64,
}

/// CARF/DAC8 Reporting Record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CARFReportingRecord {
    /// Record ID
    pub id: BytesN<32>,
    /// Reporting entity
    pub reporting_entity: Address,
    /// Account holder (if different from reporting entity)
    pub account_holder: Option<Address>,
    /// Reporting year
    pub reporting_year: u32,
    /// Transactions during year
    pub transactions: Vec<CryptoTransaction>,
    /// Holdings at year-end
    pub year_end_holdings: Vec<CryptoHolding>,
    /// Total gains realized
    pub total_realized_gains: u64,
    /// Total losses realized
    pub total_realized_losses: u64,
    /// Filing status
    pub filing_status: u32, // 0=draft, 1=submitted, 2=confirmed
}

/// Transfer Pricing Documentation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferPricingDoc {
    /// Documentation ID
    pub id: BytesN<32>,
    /// Related party transaction ID
    pub transaction_id: BytesN<32>,
    /// Transferor
    pub transferor: Address,
    /// Transferee
    pub transferee: Address,
    /// Goods/services transferred
    pub transfer_description: Bytes,
    /// Transfer amount
    pub transfer_amount: u64,
    /// Currency
    pub currency: Symbol,
    /// Methodology used
    pub method: TransferPricingMethod,
    /// Comparable transactions analyzed
    pub comparables_count: u32,
    /// Economic analysis provided
    pub economic_analysis: Bytes,
    /// Year of transfer
    pub fiscal_year: u32,
    /// Documentaton date
    pub documentation_date: u64,
}

/// Transfer Pricing Analysis Result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferPricingAnalysis {
    /// Analysis ID
    pub id: BytesN<32>,
    /// Reference to documentation
    pub doc_id: BytesN<32>,
    /// Arm's length price
    pub arms_length_price: u64,
    /// Transfer price charged
    pub transfer_price: u64,
    /// Price variance
    pub variance: i64, // negative = underpriced, positive = overpriced
    /// Variance percentage (basis points)
    pub variance_percentage: u32,
    /// Is transfer price defensible
    pub defensible: bool,
    /// Adjustment recommendations
    pub adjustment_recommendations: Bytes,
    /// Analysis date
    pub analysis_date: u64,
}

/// Country-by-Country Reporting Data
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CbCRJurisdictionData {
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Revenue from unrelated parties
    pub revenue_unrelated: u64,
    /// Revenue from related parties
    pub revenue_related: u64,
    /// Total revenue
    pub total_revenue: u64,
    /// Profit or loss
    pub profit_loss: i64,
    /// Income tax paid
    pub income_tax_paid: u64,
    /// Number of employees
    pub employee_count: u32,
    /// Tangible assets (excluding cash)
    pub tangible_assets: u64,
    /// Entity names operating in jurisdiction
    pub entities: Vec<Bytes>,
}

/// Country-by-Country Report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CbCReport {
    /// Report ID
    pub id: BytesN<32>,
    /// Reporting entity (parent)
    pub reporting_entity: Address,
    /// Fiscal year
    pub fiscal_year: u32,
    /// Data for each jurisdiction
    pub jurisdictions: Vec<CbCRJurisdictionData>,
    /// Total consolidated revenue
    pub total_revenue: u64,
    /// Total consolidated profit
    pub total_profit: i64,
    /// Total tax paid
    pub total_tax_paid: u64,
    /// Report generated date
    pub generated_date: u64,
    /// Reporting standard (BEPS, Model Rules, etc.)
    pub reporting_standard: Bytes,
    /// Filing status
    pub filing_status: u32, // 0=draft, 1=submitted, 2=confirmed
}

/// Tax Audit Event
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxAuditEvent {
    /// Event ID
    pub id: BytesN<32>,
    /// Related transaction/calculation ID
    pub reference_id: BytesN<32>,
    /// Event type (e.g., "vat_determination", "dst_calculation")
    pub event_type: Symbol,
    /// Actor (tax officer, system, regulator)
    pub actor: Address,
    /// Timestamp
    pub timestamp: u64,
    /// Action taken
    pub action: Symbol,
    /// Details/notes
    pub details: Bytes,
    /// Supporting documentation
    pub supporting_docs: Vec<BytesN<32>>,
}

/// Tax Compliance Status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxComplianceStatus {
    /// Entity address
    pub entity: Address,
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Last VAT return filing date
    pub last_vat_filing: u64,
    /// Next VAT return due date
    pub next_vat_due: u64,
    /// Last CbCR filing date
    pub last_cbcr_filing: u64,
    /// Next CbCR due date
    pub next_cbcr_due: u64,
    /// Outstanding tax liabilities
    pub outstanding_liabilities: u64,
    /// Compliance risk score (0-100)
    pub compliance_risk_score: u32,
    /// Last audit date
    pub last_audit_date: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tax_jurisdiction_ordering() {
        assert!(TaxJurisdiction::EU < TaxJurisdiction::UK);
        assert!(TaxJurisdiction::UK < TaxJurisdiction::US);
    }

    #[test]
    fn test_digital_service_category_ordering() {
        assert!(DigitalServiceCategory::OnlineAdvertising < DigitalServiceCategory::OnlineMarketplace);
    }

    #[test]
    fn test_crypto_asset_type_ordering() {
        assert!(CryptoAssetType::Bitcoin < CryptoAssetType::Ethereum);
    }

    #[test]
    fn test_transfer_pricing_method_ordering() {
        assert!(TransferPricingMethod::CUP < TransferPricingMethod::CostPlus);
    }

    #[test]
    fn test_vat_exemption_reason_ordering() {
        assert!(VATExemptionReason::Financial < VATExemptionReason::Healthcare);
    }
}
