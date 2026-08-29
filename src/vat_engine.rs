//! VAT/GST Determination Engine
//!
//! Rules-based engine for:
//! - VAT/GST classification
//! - Rate determination
//! - Exemption application
//! - Reverse charge detection
//! - B2B/B2C rules

use soroban_sdk::{contracttype, Env, Symbol, Vec};
use crate::tax::{
    VATTransaction, VATDetermination, VATSupplyType, VATExemptionReason,
    TaxJurisdiction,
};

/// VAT Rate Rules by Jurisdiction
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VATRateRules {
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Standard VAT rate (basis points)
    pub standard_rate: u32,
    /// Reduced rate (basis points)
    pub reduced_rate: Option<u32>,
    /// Super-reduced rate (basis points)
    pub super_reduced_rate: Option<u32>,
    /// Zero rate for exports
    pub zero_rate_exports: bool,
    /// Reverse charge on imports
    pub reverse_charge_imports: bool,
}

/// VAT Exemption Rules
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VATExemptionRules {
    /// Jurisdiction
    pub jurisdiction: TaxJurisdiction,
    /// Exempt supply types
    pub exempt_types: Vec<VATSupplyType>,
    /// B2B exemptions
    pub b2b_exemptions: bool,
    /// Conditions for B2B exemption
    pub b2b_conditions: Symbol, // e.g., "intra_eu", "exported"
}

/// Helper for VAT/GST determination
pub struct VATDeterminationEngine;

impl VATDeterminationEngine {
    /// Determine VAT rate for a transaction
    pub fn determine_rate(
        env: &Env,
        transaction: &VATTransaction,
    ) -> u32 {
        // Apply rules based on jurisdiction and supply type
        match transaction.place_of_supply {
            TaxJurisdiction::EU => Self::get_eu_rate(transaction),
            TaxJurisdiction::UK => Self::get_uk_rate(transaction),
            TaxJurisdiction::US => Self::get_us_rate(transaction),
            TaxJurisdiction::Canada => Self::get_canada_rate(transaction),
            TaxJurisdiction::Australia => Self::get_au_rate(transaction),
            TaxJurisdiction::India => Self::get_india_rate(transaction),
            TaxJurisdiction::Singapore => Self::get_singapore_rate(transaction),
            TaxJurisdiction::HongKong => Self::get_hk_rate(transaction),
            TaxJurisdiction::Japan => Self::get_japan_rate(transaction),
            TaxJurisdiction::Switzerland => Self::get_ch_rate(transaction),
            TaxJurisdiction::UAE => Self::get_uae_rate(transaction),
        }
    }

    /// Get EU VAT rate
    fn get_eu_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => {
                if transaction.is_b2b {
                    0 // Intra-EU supply is zero-rated
                } else {
                    2000 // Standard 20%
                }
            }
            VATSupplyType::Services => 2000,
            VATSupplyType::DigitalServices => {
                if transaction.customer_jurisdiction == TaxJurisdiction::EU {
                    2000 // 20% standard rate
                } else {
                    0 // 0% outside EU
                }
            }
            VATSupplyType::Intangibles => 2000,
            VATSupplyType::Construction => 2000,
            VATSupplyType::Transportation => {
                if transaction.is_b2b {
                    0 // B2B transport often exempt
                } else {
                    2000
                }
            }
            VATSupplyType::Telecommunications => 2000,
        }
    }

    /// Get UK VAT rate
    fn get_uk_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 2000,
            VATSupplyType::Services => 2000,
            VATSupplyType::DigitalServices => 2000,
            VATSupplyType::Intangibles => 2000,
            VATSupplyType::Construction => 2000,
            VATSupplyType::Transportation => 2000,
            VATSupplyType::Telecommunications => 2000,
        }
    }

    /// Get US sales tax rate (simplified - varies by state)
    fn get_us_rate(transaction: &VATTransaction) -> u32 {
        // US uses sales tax, not VAT - simplified to 0% for demo
        // In reality, would vary by state: 5-10% range
        0
    }

    /// Get Canadian GST/HST
    fn get_canada_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 500, // 5% GST
            VATSupplyType::Services => 500,
            VATSupplyType::DigitalServices => 500,
            VATSupplyType::Intangibles => 500,
            VATSupplyType::Construction => 500,
            VATSupplyType::Transportation => 500,
            VATSupplyType::Telecommunications => 500,
        }
    }

    /// Get Australian GST rate
    fn get_au_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 1000, // 10% GST
            VATSupplyType::Services => 1000,
            VATSupplyType::DigitalServices => 1000,
            VATSupplyType::Intangibles => 1000,
            VATSupplyType::Construction => 1000,
            VATSupplyType::Transportation => 1000,
            VATSupplyType::Telecommunications => 1000,
        }
    }

    /// Get India GST rate (simplified)
    fn get_india_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 1800, // Can be 0%, 5%, 12%, 18%
            VATSupplyType::Services => 1800,
            VATSupplyType::DigitalServices => 1800,
            VATSupplyType::Intangibles => 1800,
            VATSupplyType::Construction => 1200,
            VATSupplyType::Transportation => 500,
            VATSupplyType::Telecommunications => 1800,
        }
    }

    /// Get Singapore GST rate
    fn get_singapore_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 800, // 8% GST
            VATSupplyType::Services => 800,
            VATSupplyType::DigitalServices => 800,
            VATSupplyType::Intangibles => 800,
            VATSupplyType::Construction => 800,
            VATSupplyType::Transportation => 800,
            VATSupplyType::Telecommunications => 800,
        }
    }

    /// Get Hong Kong - no VAT/GST
    fn get_hk_rate(_transaction: &VATTransaction) -> u32 {
        0
    }

    /// Get Japan consumption tax rate
    fn get_japan_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 1000, // 10% consumption tax
            VATSupplyType::Services => 1000,
            VATSupplyType::DigitalServices => 1000,
            VATSupplyType::Intangibles => 1000,
            VATSupplyType::Construction => 1000,
            VATSupplyType::Transportation => 1000,
            VATSupplyType::Telecommunications => 1000,
        }
    }

    /// Get UAE VAT rate (introduced 2018)
    fn get_uae_rate(transaction: &VATTransaction) -> u32 {
        match transaction.supply_type {
            VATSupplyType::Goods => 500, // 5% VAT
            VATSupplyType::Services => 500,
            VATSupplyType::DigitalServices => 500,
            VATSupplyType::Intangibles => 500,
            VATSupplyType::Construction => 500,
            VATSupplyType::Transportation => 500,
            VATSupplyType::Telecommunications => 500,
        }
    }

    /// Check if supply is exempt
    pub fn is_exempt(transaction: &VATTransaction) -> bool {
        // Financial services exempt in most jurisdictions
        // Healthcare exempt
        // Education exempt
        // Certain cultural activities exempt

        match transaction.exemption_reason {
            VATExemptionReason::None => false,
            VATExemptionReason::Financial => true,
            VATExemptionReason::Healthcare => true,
            VATExemptionReason::Education => true,
            VATExemptionReason::ExportedGoods => transaction.place_of_supply != transaction.customer_jurisdiction,
            VATExemptionReason::IntraEUSupply => {
                transaction.place_of_supply == TaxJurisdiction::EU &&
                transaction.customer_jurisdiction == TaxJurisdiction::EU
            }
            VATExemptionReason::CulturalActivities => true,
            VATExemptionReason::AgriculturalProduction => true,
        }
    }

    /// Check if reverse charge applies
    pub fn should_apply_reverse_charge(transaction: &VATTransaction) -> bool {
        // Reverse charge applies in B2B situations
        // When supplier is in different jurisdiction
        // For specific supply types (construction, services, intangibles)

        if !transaction.is_b2b {
            return false;
        }

        if transaction.place_of_supply == transaction.customer_jurisdiction {
            return false;
        }

        // Reverse charge applies for these types
        matches!(
            transaction.supply_type,
            VATSupplyType::Construction | VATSupplyType::Services | VATSupplyType::DigitalServices | VATSupplyType::Intangibles
        )
    }

    /// Determine VAT for a transaction
    pub fn determine_vat(
        env: &Env,
        transaction: &VATTransaction,
    ) -> VATDetermination {
        let is_exempt = Self::is_exempt(transaction);
        let reverse_charge = Self::should_apply_reverse_charge(transaction);

        let vat_rate = if is_exempt || reverse_charge {
            0
        } else {
            Self::determine_rate(env, transaction)
        };

        let vat_amount = if is_exempt || reverse_charge {
            0
        } else {
            (transaction.supply_amount * vat_rate as u64) / 10000
        };

        VATDetermination {
            transaction_id: transaction.id,
            vat_rate,
            is_exempt,
            exemption_reason: if is_exempt {
                Some(transaction.exemption_reason)
            } else {
                None
            },
            reverse_charge_applicable: reverse_charge,
            place_of_supply: transaction.place_of_supply,
            vat_amount,
            determined_at: env.ledger().timestamp(),
            source: soroban_sdk::Bytes::new(env).try_extend_from_slice(b"VAT_ENGINE_V1").unwrap(),
        }
    }

    /// Calculate net and gross amounts
    pub fn calculate_amounts(
        supply_amount: u64,
        vat_rate: u32,
        is_inclusive: bool,
    ) -> (u64, u64, u64) {
        if is_inclusive {
            // supply_amount is gross (inclusive of VAT)
            let vat_amount = (supply_amount * vat_rate as u64) / (10000 + vat_rate as u64);
            let net_amount = supply_amount - vat_amount;
            (net_amount, vat_amount, supply_amount)
        } else {
            // supply_amount is net (exclusive of VAT)
            let vat_amount = (supply_amount * vat_rate as u64) / 10000;
            (supply_amount, vat_amount, supply_amount + vat_amount)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vat_rate_rules_struct() {
        let rules = VATRateRules {
            jurisdiction: TaxJurisdiction::EU,
            standard_rate: 2000,
            reduced_rate: Some(1000),
            super_reduced_rate: Some(500),
            zero_rate_exports: true,
            reverse_charge_imports: true,
        };

        assert_eq!(rules.standard_rate, 2000);
        assert_eq!(rules.reduced_rate, Some(1000));
    }

    #[test]
    fn test_eu_goods_b2b_zero_rated() {
        // Intra-EU B2B goods supply should be zero-rated
        let rate = VATDeterminationEngine::get_eu_rate(&VATTransaction {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            supplier: soroban_sdk::Address::random(&Env::default()),
            customer: soroban_sdk::Address::random(&Env::default()),
            supply_type: VATSupplyType::Goods,
            supply_amount: 100000,
            currency: Symbol::new(&Env::default(), "EUR"),
            timestamp: 0,
            place_of_supply: TaxJurisdiction::EU,
            customer_jurisdiction: TaxJurisdiction::EU,
            is_b2b: true,
            reverse_charge: false,
            exemption_reason: VATExemptionReason::None,
            vat_rate: 0,
            vat_amount: 0,
            description: soroban_sdk::Bytes::new(&Env::default()),
        });

        assert_eq!(rate, 0);
    }

    #[test]
    fn test_reverse_charge_b2b_cross_border() {
        let mut transaction = VATTransaction {
            id: soroban_sdk::BytesN::from_array([0u8; 32]),
            supplier: soroban_sdk::Address::random(&Env::default()),
            customer: soroban_sdk::Address::random(&Env::default()),
            supply_type: VATSupplyType::Services,
            supply_amount: 100000,
            currency: Symbol::new(&Env::default(), "EUR"),
            timestamp: 0,
            place_of_supply: TaxJurisdiction::EU,
            customer_jurisdiction: TaxJurisdiction::UK,
            is_b2b: true,
            reverse_charge: false,
            exemption_reason: VATExemptionReason::None,
            vat_rate: 0,
            vat_amount: 0,
            description: soroban_sdk::Bytes::new(&Env::default()),
        };

        assert!(VATDeterminationEngine::should_apply_reverse_charge(&transaction));
    }

    #[test]
    fn test_calculate_amounts_exclusive() {
        let (net, vat, gross) = VATDeterminationEngine::calculate_amounts(
            100000, // €100
            2000,   // 20%
            false,  // exclusive
        );

        assert_eq!(net, 100000);
        assert_eq!(vat, 20000);
        assert_eq!(gross, 120000);
    }

    #[test]
    fn test_calculate_amounts_inclusive() {
        let (net, vat, gross) = VATDeterminationEngine::calculate_amounts(
            120000, // €120 (inclusive)
            2000,   // 20%
            true,   // inclusive
        );

        assert_eq!(net, 100000);
        assert_eq!(vat, 20000);
        assert_eq!(gross, 120000);
    }
}
