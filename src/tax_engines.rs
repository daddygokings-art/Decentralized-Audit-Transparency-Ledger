//! Digital Services Tax (DST) Engine
//! Crypto Asset Reporting (CARF/DAC8) Engine
//! Transfer Pricing Engine
//! Country-by-Country Reporting (CbCR) Engine

use soroban_sdk::{contracttype, Env, Vec, Address};
use crate::tax::*;

// ========== DIGITAL SERVICES TAX ENGINE ==========

pub struct DSTEngine;

impl DSTEngine {
    /// Check if DST applies to a transaction
    pub fn is_applicable(
        env: &Env,
        transaction: &DSTTransaction,
    ) -> bool {
        // DST applies to digital service companies with revenue > threshold
        // France: €750M global, 3% on in-scope revenue in France
        // UK: £500M global, 2% on UK DST revenue
        // EU countries have similar rules

        // Simplified check: threshold met and service in-scope
        transaction.threshold_met && Self::is_in_scope_service(transaction.service_category)
    }

    /// Check if service category is in-scope for DST
    fn is_in_scope_service(category: DigitalServiceCategory) -> bool {
        matches!(
            category,
            DigitalServiceCategory::OnlineAdvertising
                | DigitalServiceCategory::OnlineMarketplace
                | DigitalServiceCategory::SocialMedia
                | DigitalServiceCategory::VideoStreaming
        )
    }

    /// Determine DST rate and applicable jurisdictions
    pub fn determine_dst(
        env: &Env,
        transaction: &DSTTransaction,
    ) -> DSTDetermination {
        let is_applicable = Self::is_applicable(env, transaction);

        let (dst_rate, jurisdictions) = if is_applicable {
            match transaction.user_jurisdiction {
                TaxJurisdiction::EU => (300, {
                    let mut v = Vec::new(env);
                    v.push_back(transaction.user_jurisdiction);
                    v
                }), // 3%
                TaxJurisdiction::UK => (200, {
                    let mut v = Vec::new(env);
                    v.push_back(transaction.user_jurisdiction);
                    v
                }), // 2%
                TaxJurisdiction::India => (400, {
                    let mut v = Vec::new(env);
                    v.push_back(transaction.user_jurisdiction);
                    v
                }), // 4%
                TaxJurisdiction::Australia => (300, {
                    let mut v = Vec::new(env);
                    v.push_back(transaction.user_jurisdiction);
                    v
                }), // 3%
                _ => (0, Vec::new(env)),
            }
        } else {
            (0, Vec::new(env))
        };

        let dst_amount = if is_applicable && dst_rate > 0 {
            (transaction.revenue * dst_rate as u64) / 10000
        } else {
            0
        };

        DSTDetermination {
            transaction_id: transaction.id,
            is_applicable,
            dst_rate,
            jurisdictions,
            dst_amount,
            basis: soroban_sdk::Bytes::new(env)
                .try_extend_from_slice(b"DST_ENGINE_V1")
                .unwrap(),
            determined_at: env.ledger().timestamp(),
        }
    }
}

// ========== CRYPTO ASSET REPORTING ENGINE ==========

pub struct CryptoReportingEngine;

impl CryptoReportingEngine {
    /// Determine CARF/DAC8 reporting requirements
    pub fn is_reportable(
        holding: &CryptoHolding,
        transaction: &CryptoTransaction,
    ) -> bool {
        // CARF/DAC8 requires reporting of:
        // - Sales/dispositions
        // - Large holdings (varies by country)
        // - Cross-border transfers
        // - Staking rewards

        matches!(
            transaction.transaction_type.to_string(),
            s if s == "sell" || s == "transfer" || s == "stake_reward" || s == "large_holding"
        )
    }

    /// Calculate cost basis using FIFO method
    pub fn calculate_fifo_cost_basis(
        holdings: &Vec<CryptoHolding>,
        amount_sold: u64,
    ) -> (u64, u64) {
        let mut total_cost = 0u64;
        let mut processed = 0u64;

        for holding in holdings.iter() {
            if processed >= amount_sold {
                break;
            }

            let amount_to_process = if processed + holding.balance > amount_sold {
                amount_sold - processed
            } else {
                holding.balance
            };

            let proportion = (amount_to_process * 1_000_000) / holding.balance.max(1);
            let cost = (holding.cost_basis * proportion) / 1_000_000;
            total_cost = total_cost.saturating_add(cost);
            processed = processed.saturating_add(amount_to_process);
        }

        (total_cost, processed)
    }

    /// Calculate realized gain/loss
    pub fn calculate_realized_gain(
        cost_basis: u64,
        proceeds: u64,
    ) -> i64 {
        (proceeds as i64) - (cost_basis as i64)
    }

    /// Generate CARF reporting record
    pub fn generate_carf_record(
        env: &Env,
        reporting_entity: Address,
        transactions: Vec<CryptoTransaction>,
        holdings: Vec<CryptoHolding>,
        reporting_year: u32,
    ) -> CARFReportingRecord {
        let mut total_gains = 0u64;
        let mut total_losses = 0u64;

        for tx in transactions.iter() {
            if let Some(cost_basis) = tx.cost_basis {
                let gain = Self::calculate_realized_gain(cost_basis, tx.fair_market_value);
                if gain > 0 {
                    total_gains = total_gains.saturating_add(gain as u64);
                } else {
                    total_losses = total_losses.saturating_add((-gain) as u64);
                }
            }
        }

        CARFReportingRecord {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            reporting_entity,
            account_holder: None,
            reporting_year,
            transactions,
            year_end_holdings: holdings,
            total_realized_gains: total_gains,
            total_realized_losses: total_losses,
            filing_status: 0, // Draft
        }
    }
}

// ========== TRANSFER PRICING ENGINE ==========

pub struct TransferPricingEngine;

impl TransferPricingEngine {
    /// Validate transfer price using arm's length principle
    pub fn validate_price(
        transaction_price: u64,
        comparable_prices: &Vec<u64>,
    ) -> TransferPricingAnalysis {
        let avg_comparable = if !comparable_prices.is_empty() {
            comparable_prices.iter().sum::<u64>() / comparable_prices.len() as u64
        } else {
            transaction_price
        };

        let variance = (transaction_price as i64) - (avg_comparable as i64);
        let variance_percentage = if avg_comparable > 0 {
            ((variance.abs() as u64 * 10000) / avg_comparable) as u32
        } else {
            0
        };

        // Defensible if within interquartile range (±25%)
        let defensible = variance_percentage <= 2500;

        TransferPricingAnalysis {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            doc_id: soroban_sdk::BytesN::from_array([0u8; 32]),
            arms_length_price: avg_comparable,
            transfer_price: transaction_price,
            variance,
            variance_percentage,
            defensible,
            adjustment_recommendations: soroban_sdk::Bytes::new(&Env::default()),
            analysis_date: 0,
        }
    }

    /// Determine appropriate transfer pricing method
    pub fn select_method(
        supply_type: &str,
        transaction_type: &str,
    ) -> TransferPricingMethod {
        if supply_type.contains("goods") {
            TransferPricingMethod::CUP
        } else if supply_type.contains("service") {
            TransferPricingMethod::CostPlus
        } else if supply_type.contains("intangible") {
            TransferPricingMethod::ProfitSplit
        } else {
            TransferPricingMethod::TNMM
        }
    }
}

// ========== COUNTRY-BY-COUNTRY REPORTING ENGINE ==========

pub struct CbCREngine;

impl CbCREngine {
    /// Aggregate data by jurisdiction
    pub fn aggregate_jurisdictions(
        env: &Env,
        transactions: &Vec<VATTransaction>,
    ) -> Vec<CbCRJurisdictionData> {
        // In production, would aggregate actual transaction data
        // For now, return structure showing how aggregation would work

        let mut jurisdictions = Vec::new(env);

        jurisdictions.push_back(CbCRJurisdictionData {
            jurisdiction: TaxJurisdiction::EU,
            revenue_unrelated: 1_000_000,
            revenue_related: 500_000,
            total_revenue: 1_500_000,
            profit_loss: 300_000,
            income_tax_paid: 75_000,
            employee_count: 50,
            tangible_assets: 2_000_000,
            entities: {
                let mut v = Vec::new(env);
                v.push_back(soroban_sdk::Bytes::new(env).try_extend_from_slice(b"EU Subsidiary").unwrap());
                v
            },
        });

        jurisdictions
    }

    /// Generate CbCR
    pub fn generate_cbcr(
        env: &Env,
        reporting_entity: Address,
        fiscal_year: u32,
        jurisdictions: Vec<CbCRJurisdictionData>,
    ) -> CbCReport {
        let mut total_revenue = 0u64;
        let mut total_profit = 0i64;
        let mut total_tax = 0u64;

        for jd in jurisdictions.iter() {
            total_revenue = total_revenue.saturating_add(jd.total_revenue);
            total_profit = total_profit.saturating_add(jd.profit_loss);
            total_tax = total_tax.saturating_add(jd.income_tax_paid);
        }

        CbCReport {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            reporting_entity,
            fiscal_year,
            jurisdictions,
            total_revenue,
            total_profit,
            total_tax_paid: total_tax,
            generated_date: env.ledger().timestamp(),
            reporting_standard: soroban_sdk::Bytes::new(env)
                .try_extend_from_slice(b"BEPS_Action13")
                .unwrap(),
            filing_status: 0, // Draft
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dst_in_scope_services() {
        assert!(DSTEngine::is_in_scope_service(
            DigitalServiceCategory::OnlineAdvertising
        ));
        assert!(DSTEngine::is_in_scope_service(
            DigitalServiceCategory::SocialMedia
        ));
        assert!(!DSTEngine::is_in_scope_service(DigitalServiceCategory::CloudServices));
    }

    #[test]
    fn test_crypto_gain_calculation() {
        let gain = CryptoReportingEngine::calculate_realized_gain(10_000, 15_000);
        assert_eq!(gain, 5_000);
    }

    #[test]
    fn test_crypto_loss_calculation() {
        let loss = CryptoReportingEngine::calculate_realized_gain(15_000, 10_000);
        assert_eq!(loss, -5_000);
    }

    #[test]
    fn test_transfer_pricing_defensible() {
        let comparable = vec![100_000, 105_000, 95_000];
        let analysis = TransferPricingEngine::validate_price(100_000, &comparable);
        assert!(analysis.defensible);
    }

    #[test]
    fn test_transfer_pricing_not_defensible() {
        let comparable = vec![100_000, 105_000, 95_000];
        let analysis = TransferPricingEngine::validate_price(150_000, &comparable);
        assert!(!analysis.defensible);
    }
}
