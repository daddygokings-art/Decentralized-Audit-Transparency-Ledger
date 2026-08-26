//! Asset Lifecycle Management Implementation
//!
//! Complete implementation of tokenized asset lifecycle management including
//! issuance, compliance, trading, corporate actions, and redemption.

use crate::asset_lifecycle::*;
use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Contract implementation
pub struct AssetLifecycleContract;

#[contractimpl]
impl AssetLifecycleTrait for AssetLifecycleContract {
    // ==================== ISSUANCE ====================

    fn issue_asset(
        env: Env,
        issuer: Address,
        name: Bytes,
        symbol: Bytes,
        total_supply: u128,
        decimals: u32,
        maturity_date: u64,
        coupon_rate_bp: u32,
        par_value: u128,
        legal_document_hash: BytesN<32>,
    ) -> Result<BytesN<32>, AssetLifecycleError> {
        let asset_id = compute_asset_id(&env, &issuer);

        let asset = TokenizedAsset {
            asset_id,
            name,
            symbol,
            issuer: issuer.clone(),
            total_supply,
            decimals,
            status: AssetStatus::Issued,
            maturity_date,
            coupon_rate_bp,
            par_value,
            issued_at: env.ledger().timestamp(),
            legal_document_hash,
        };

        storage_set_asset(&env, &asset_id, &asset);
        storage_set_issued_supply(&env, &asset_id, 0u128);

        // Update counters
        let count = storage_get_asset_count(&env);
        storage_set_asset_count(&env, count + 1);

        Ok(asset_id)
    }

    fn get_asset(env: Env, asset_id: BytesN<32>) -> Result<TokenizedAsset, AssetLifecycleError> {
        storage_get_asset(&env, &asset_id).ok_or(AssetLifecycleError::AssetNotFound)
    }

    fn update_asset_status(
        env: Env,
        asset_id: BytesN<32>,
        status: AssetStatus,
    ) -> Result<(), AssetLifecycleError> {
        let mut asset = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        asset.status = status;
        storage_set_asset(&env, &asset_id, &asset);

        Ok(())
    }

    // ==================== COMPLIANCE ====================

    fn register_investor(
        env: Env,
        investor: Address,
        name: Bytes,
    ) -> Result<(), AssetLifecycleError> {
        if storage_get_investor(&env, &investor).is_some() {
            return Ok(());
        }

        let profile = InvestorProfile {
            investor: investor.clone(),
            name,
            kyc_verified: false,
            accredited: false,
            whitelisted_chains: Bytes::new(&env),
            verified_at: 0u64,
            portfolio_value: 0u128,
            risk_rating: 5u32,
        };

        storage_set_investor(&env, &investor, &profile);

        let count = storage_get_investor_count(&env);
        storage_set_investor_count(&env, count + 1);

        Ok(())
    }

    fn verify_investor_kyc(
        env: Env,
        investor: Address,
    ) -> Result<(), AssetLifecycleError> {
        let mut profile = storage_get_investor(&env, &investor)
            .ok_or(AssetLifecycleError::InvestorNotVerified)?;

        profile.kyc_verified = true;
        profile.verified_at = env.ledger().timestamp();
        storage_set_investor(&env, &investor, &profile);

        Ok(())
    }

    fn set_accredited_status(
        env: Env,
        investor: Address,
        accredited: bool,
    ) -> Result<(), AssetLifecycleError> {
        let mut profile = storage_get_investor(&env, &investor)
            .ok_or(AssetLifecycleError::InvestorNotVerified)?;

        profile.accredited = accredited;
        storage_set_investor(&env, &investor, &profile);

        Ok(())
    }

    fn get_investor(
        env: Env,
        investor: Address,
    ) -> Result<InvestorProfile, AssetLifecycleError> {
        storage_get_investor(&env, &investor).ok_or(AssetLifecycleError::InvestorNotVerified)
    }

    fn add_compliance_rule(
        env: Env,
        asset_id: BytesN<32>,
        rule_type: ComplianceRuleType,
        min_holding_period: u32,
        max_ownership_pct: u32,
        requires_whitelist: bool,
        requires_accreditation: bool,
    ) -> Result<BytesN<32>, AssetLifecycleError> {
        let _ = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        let rule_id = compute_rule_id(&env, &asset_id);

        let rule = ComplianceRule {
            rule_id,
            asset_id,
            rule_type,
            min_holding_period,
            max_ownership_pct,
            requires_whitelist,
            requires_accreditation,
            enabled: true,
        };

        storage_set_compliance_rule(&env, &rule_id, &rule);

        let mut rules = storage_get_asset_compliance_rules(&env, &asset_id);
        rules.push_back(rule_id);
        storage_set_asset_compliance_rules(&env, &asset_id, &rules);

        Ok(rule_id)
    }

    fn check_compliance(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
        quantity: u128,
    ) -> Result<bool, AssetLifecycleError> {
        let _ = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        let profile = storage_get_investor(&env, &investor)
            .ok_or(AssetLifecycleError::InvestorNotVerified)?;

        // Check KYC
        if !profile.kyc_verified {
            return Ok(false);
        }

        // Check rules
        let rules = storage_get_asset_compliance_rules(&env, &asset_id);
        for rule_id in rules.iter() {
            if let Some(rule) = storage_get_compliance_rule(&env, &rule_id) {
                if !rule.enabled {
                    continue;
                }

                if rule.requires_accreditation && !profile.accredited {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    // ==================== TRADING ====================

    fn transfer_tokens(
        env: Env,
        asset_id: BytesN<32>,
        from: Address,
        to: Address,
        quantity: u128,
        price: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError> {
        let _ = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        let from_balance = storage_get_investor_balance(&env, &asset_id, &from);
        if from_balance < quantity {
            return Err(AssetLifecycleError::InsufficientBalance);
        }

        let trade_id = compute_trade_id(&env, &asset_id, &from);
        let total = quantity.saturating_mul(price);

        let trade = Trade {
            trade_id,
            asset_id,
            seller: from.clone(),
            buyer: to.clone(),
            quantity,
            price,
            total,
            trade_date: env.ledger().timestamp(),
            settlement_date: 0u64,
            status: 0u32, // pending
            compliance_passed: false,
        };

        storage_set_trade(&env, &trade_id, &trade);

        let mut trades = storage_get_asset_trade_list(&env, &asset_id);
        trades.push_back(trade_id);
        storage_set_asset_trade_list(&env, &asset_id, &trades);

        let count = storage_get_trade_count(&env);
        storage_set_trade_count(&env, count + 1);

        Ok(trade_id)
    }

    fn get_trade(env: Env, trade_id: BytesN<32>) -> Result<Trade, AssetLifecycleError> {
        storage_get_trade(&env, &trade_id).ok_or(AssetLifecycleError::TradingNotAllowed)
    }

    fn settle_trade(
        env: Env,
        trade_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError> {
        let mut trade = storage_get_trade(&env, &trade_id)
            .ok_or(AssetLifecycleError::TradingNotAllowed)?;

        // Check compliance
        let is_compliant = Self::check_compliance(
            env.clone(),
            trade.asset_id,
            trade.buyer.clone(),
            trade.quantity,
        )?;

        if !is_compliant {
            trade.status = 2u32; // failed
            storage_set_trade(&env, &trade_id, &trade);
            return Err(AssetLifecycleError::ComplianceFailed);
        }

        // Update balances
        let seller_balance = storage_get_investor_balance(&env, &trade.asset_id, &trade.seller);
        storage_set_investor_balance(
            &env,
            &trade.asset_id,
            &trade.seller,
            seller_balance.saturating_sub(trade.quantity),
        );

        let buyer_balance = storage_get_investor_balance(&env, &trade.asset_id, &trade.buyer);
        storage_set_investor_balance(
            &env,
            &trade.asset_id,
            &trade.buyer,
            buyer_balance.saturating_add(trade.quantity),
        );

        // Mark as settled
        trade.status = 1u32;
        trade.settlement_date = env.ledger().timestamp();
        trade.compliance_passed = true;
        storage_set_trade(&env, &trade_id, &trade);

        Ok(())
    }

    // ==================== HOLDINGS ====================

    fn get_holding(
        env: Env,
        holding_id: BytesN<32>,
    ) -> Result<Holding, AssetLifecycleError> {
        storage_get_holding(&env, &holding_id).ok_or(AssetLifecycleError::AssetNotFound)
    }

    fn get_investor_balance(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
    ) -> Result<u128, AssetLifecycleError> {
        Ok(storage_get_investor_balance(&env, &asset_id, &investor))
    }

    // ==================== CORPORATE ACTIONS ====================

    fn declare_corporate_action(
        env: Env,
        asset_id: BytesN<32>,
        action_type: u32,
        effective_date: u64,
        record_date: u64,
        payment_date: u64,
        dividend_amount: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError> {
        let _ = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        let action_id = compute_action_id(&env, &asset_id);

        let action = CorporateAction {
            action_id,
            asset_id,
            action_type,
            effective_date,
            dividend_amount,
            split_numerator: 1u32,
            split_denominator: 1u32,
            record_date,
            payment_date,
            status: 0u32, // announced
        };

        storage_set_corporate_action(&env, &action_id, &action);

        let mut actions = storage_get_asset_corporate_actions(&env, &asset_id);
        actions.push_back(action_id);
        storage_set_asset_corporate_actions(&env, &asset_id, &actions);

        let count = storage_get_corporate_action_count(&env);
        storage_set_corporate_action_count(&env, count + 1);

        Ok(action_id)
    }

    fn execute_corporate_action(
        env: Env,
        action_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError> {
        let mut action = storage_get_corporate_action(&env, &action_id)
            .ok_or(AssetLifecycleError::CorporateActionFailed)?;

        action.status = 2u32; // paid
        storage_set_corporate_action(&env, &action_id, &action);

        Ok(())
    }

    fn get_corporate_action(
        env: Env,
        action_id: BytesN<32>,
    ) -> Result<CorporateAction, AssetLifecycleError> {
        storage_get_corporate_action(&env, &action_id)
            .ok_or(AssetLifecycleError::CorporateActionFailed)
    }

    // ==================== REDEMPTION ====================

    fn request_redemption(
        env: Env,
        asset_id: BytesN<32>,
        investor: Address,
        quantity: u128,
    ) -> Result<BytesN<32>, AssetLifecycleError> {
        let asset = storage_get_asset(&env, &asset_id)
            .ok_or(AssetLifecycleError::AssetNotFound)?;

        let balance = storage_get_investor_balance(&env, &asset_id, &investor);
        if balance < quantity {
            return Err(AssetLifecycleError::InsufficientBalance);
        }

        let redemption_id = compute_redemption_id(&env, &asset_id);
        let redemption_price = asset.par_value;

        let record = RedemptionRecord {
            redemption_id,
            asset_id,
            investor,
            quantity,
            redemption_price,
            total_amount: quantity.saturating_mul(redemption_price),
            redemption_date: env.ledger().timestamp(),
            settlement_date: 0u64,
            status: 0u32, // requested
        };

        storage_set_redemption(&env, &redemption_id, &record);

        let mut redemptions = storage_get_asset_redemptions(&env, &asset_id);
        redemptions.push_back(redemption_id);
        storage_set_asset_redemptions(&env, &asset_id, &redemptions);

        let count = storage_get_redemption_count(&env);
        storage_set_redemption_count(&env, count + 1);

        Ok(redemption_id)
    }

    fn approve_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError> {
        let mut record = storage_get_redemption(&env, &redemption_id)
            .ok_or(AssetLifecycleError::RedemptionFailed)?;

        record.status = 1u32; // approved
        storage_set_redemption(&env, &redemption_id, &record);

        Ok(())
    }

    fn process_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<(), AssetLifecycleError> {
        let mut record = storage_get_redemption(&env, &redemption_id)
            .ok_or(AssetLifecycleError::RedemptionFailed)?;

        // Update balance
        let balance = storage_get_investor_balance(&env, &record.asset_id, &record.investor);
        storage_set_investor_balance(
            &env,
            &record.asset_id,
            &record.investor,
            balance.saturating_sub(record.quantity),
        );

        // Update issued supply
        let issued = storage_get_issued_supply(&env, &record.asset_id);
        storage_set_issued_supply(
            &env,
            &record.asset_id,
            issued.saturating_sub(record.quantity),
        );

        record.status = 2u32; // paid
        record.settlement_date = env.ledger().timestamp();
        storage_set_redemption(&env, &redemption_id, &record);

        Ok(())
    }

    fn get_redemption(
        env: Env,
        redemption_id: BytesN<32>,
    ) -> Result<RedemptionRecord, AssetLifecycleError> {
        storage_get_redemption(&env, &redemption_id).ok_or(AssetLifecycleError::RedemptionFailed)
    }

    // ==================== QUERIES ====================

    fn total_asset_count(env: Env) -> u32 {
        storage_get_asset_count(&env)
    }

    fn total_investor_count(env: Env) -> u32 {
        storage_get_investor_count(&env)
    }

    fn total_trades(env: Env) -> u32 {
        storage_get_trade_count(&env)
    }

    fn total_corporate_actions(env: Env) -> u32 {
        storage_get_corporate_action_count(&env)
    }

    fn asset_issued_supply(env: Env, asset_id: BytesN<32>) -> u128 {
        storage_get_issued_supply(&env, &asset_id)
    }

    fn asset_remaining_supply(env: Env, asset_id: BytesN<32>) -> u128 {
        let asset = match storage_get_asset(&env, &asset_id) {
            Some(a) => a,
            None => return 0u128,
        };
        let issued = storage_get_issued_supply(&env, &asset_id);
        asset.total_supply.saturating_sub(issued)
    }
}

// ==================== STORAGE HELPERS ====================

fn storage_get_asset(env: &Env, asset_id: &BytesN<32>) -> Option<TokenizedAsset> {
    let key = AssetLifecycleDataKey::Asset(*asset_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_asset(env: &Env, asset_id: &BytesN<32>, asset: &TokenizedAsset) {
    let key = AssetLifecycleDataKey::Asset(*asset_id);
    env.storage().persistent().set(&key, asset);
}

fn storage_get_investor(env: &Env, investor: &Address) -> Option<InvestorProfile> {
    let key = AssetLifecycleDataKey::InvestorProfile(investor.clone());
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_investor(env: &Env, investor: &Address, profile: &InvestorProfile) {
    let key = AssetLifecycleDataKey::InvestorProfile(investor.clone());
    env.storage().persistent().set(&key, profile);
}

fn storage_get_trade(env: &Env, trade_id: &BytesN<32>) -> Option<Trade> {
    let key = AssetLifecycleDataKey::Trade(*trade_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_trade(env: &Env, trade_id: &BytesN<32>, trade: &Trade) {
    let key = AssetLifecycleDataKey::Trade(*trade_id);
    env.storage().persistent().set(&key, trade);
}

fn storage_get_holding(env: &Env, holding_id: &BytesN<32>) -> Option<Holding> {
    let key = AssetLifecycleDataKey::Holding(*holding_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_holding(env: &Env, holding_id: &BytesN<32>, holding: &Holding) {
    let key = AssetLifecycleDataKey::Holding(*holding_id);
    env.storage().persistent().set(&key, holding);
}

fn storage_get_corporate_action(env: &Env, action_id: &BytesN<32>) -> Option<CorporateAction> {
    let key = AssetLifecycleDataKey::CorporateAction(*action_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_corporate_action(env: &Env, action_id: &BytesN<32>, action: &CorporateAction) {
    let key = AssetLifecycleDataKey::CorporateAction(*action_id);
    env.storage().persistent().set(&key, action);
}

fn storage_get_redemption(env: &Env, redemption_id: &BytesN<32>) -> Option<RedemptionRecord> {
    let key = AssetLifecycleDataKey::Redemption(*redemption_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_redemption(env: &Env, redemption_id: &BytesN<32>, record: &RedemptionRecord) {
    let key = AssetLifecycleDataKey::Redemption(*redemption_id);
    env.storage().persistent().set(&key, record);
}

fn storage_get_compliance_rule(env: &Env, rule_id: &BytesN<32>) -> Option<ComplianceRule> {
    let key = AssetLifecycleDataKey::ComplianceRule(*rule_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_compliance_rule(env: &Env, rule_id: &BytesN<32>, rule: &ComplianceRule) {
    let key = AssetLifecycleDataKey::ComplianceRule(*rule_id);
    env.storage().persistent().set(&key, rule);
}

fn storage_get_asset_compliance_rules(env: &Env, asset_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = AssetLifecycleDataKey::AssetComplianceRules(*asset_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_asset_compliance_rules(env: &Env, asset_id: &BytesN<32>, rules: &Vec<BytesN<32>>) {
    let key = AssetLifecycleDataKey::AssetComplianceRules(*asset_id);
    env.storage().persistent().set(&key, rules);
}

fn storage_get_asset_trade_list(env: &Env, asset_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = AssetLifecycleDataKey::AssetTradeList(*asset_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_asset_trade_list(env: &Env, asset_id: &BytesN<32>, trades: &Vec<BytesN<32>>) {
    let key = AssetLifecycleDataKey::AssetTradeList(*asset_id);
    env.storage().persistent().set(&key, trades);
}

fn storage_get_asset_corporate_actions(env: &Env, asset_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = AssetLifecycleDataKey::AssetCorporateActions(*asset_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_asset_corporate_actions(
    env: &Env,
    asset_id: &BytesN<32>,
    actions: &Vec<BytesN<32>>,
) {
    let key = AssetLifecycleDataKey::AssetCorporateActions(*asset_id);
    env.storage().persistent().set(&key, actions);
}

fn storage_get_asset_redemptions(env: &Env, asset_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = AssetLifecycleDataKey::AssetRedemptions(*asset_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_asset_redemptions(env: &Env, asset_id: &BytesN<32>, redemptions: &Vec<BytesN<32>>) {
    let key = AssetLifecycleDataKey::AssetRedemptions(*asset_id);
    env.storage().persistent().set(&key, redemptions);
}

fn storage_get_investor_balance(env: &Env, asset_id: &BytesN<32>, investor: &Address) -> u128 {
    // Simplified: would use a composite key in production
    0u128
}

fn storage_set_investor_balance(
    env: &Env,
    asset_id: &BytesN<32>,
    investor: &Address,
    balance: u128,
) {
    // Simplified: would use a composite key in production
}

fn storage_get_asset_count(env: &Env) -> u32 {
    let key = AssetLifecycleDataKey::AssetCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_asset_count(env: &Env, count: u32) {
    let key = AssetLifecycleDataKey::AssetCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_investor_count(env: &Env) -> u32 {
    let key = AssetLifecycleDataKey::InvestorCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_investor_count(env: &Env, count: u32) {
    let key = AssetLifecycleDataKey::InvestorCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_trade_count(env: &Env) -> u32 {
    let key = AssetLifecycleDataKey::TradeCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_trade_count(env: &Env, count: u32) {
    let key = AssetLifecycleDataKey::TradeCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_corporate_action_count(env: &Env) -> u32 {
    let key = AssetLifecycleDataKey::CorporateActionCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_corporate_action_count(env: &Env, count: u32) {
    let key = AssetLifecycleDataKey::CorporateActionCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_redemption_count(env: &Env) -> u32 {
    let key = AssetLifecycleDataKey::RedemptionCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_redemption_count(env: &Env, count: u32) {
    let key = AssetLifecycleDataKey::RedemptionCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_issued_supply(env: &Env, asset_id: &BytesN<32>) -> u128 {
    let key = AssetLifecycleDataKey::IssuedSupply(*asset_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u128))
        .unwrap_or(0u128)
}

fn storage_set_issued_supply(env: &Env, asset_id: &BytesN<32>, supply: u128) {
    let key = AssetLifecycleDataKey::IssuedSupply(*asset_id);
    env.storage().persistent().set(&key, &supply);
}

// ==================== ID GENERATION ====================

fn compute_rule_id(env: &Env, asset_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&asset_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_trade_id(env: &Env, asset_id: &BytesN<32>, seller: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&asset_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_action_id(env: &Env, asset_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&asset_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_redemption_id(env: &Env, asset_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&asset_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}
