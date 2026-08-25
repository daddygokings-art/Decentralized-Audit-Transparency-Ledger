//! DeFi Protocol Auditing Contract Implementation
//!
//! Implements all DeFi auditing functions including TVL tracking, oracle verification,
//! liquidation monitoring, governance tracking, risk metrics, and audit report generation.

use crate::defi_auditing::*;
use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Contract implementation for DeFi auditing
pub struct DeFiAuditingContract;

#[contractimpl]
impl DeFiAuditingTrait for DeFiAuditingContract {
    // ==================== PROTOCOL MANAGEMENT ====================

    fn register_protocol(
        env: Env,
        protocol: Address,
        name: Symbol,
        protocol_type: ProtocolType,
        chain: Symbol,
        gov_token: Option<Address>,
    ) -> Result<(), DeFiAuditError> {
        // Check if already registered
        if storage_get_protocol(&env, &protocol).is_some() {
            return Ok(());
        }

        let registry = ProtocolRegistry {
            protocol: protocol.clone(),
            name,
            protocol_type,
            chain,
            gov_token,
            registered_at: env.ledger().timestamp(),
        };

        storage_set_protocol(&env, &protocol, &registry);

        // Update protocol list
        let mut protocol_list = storage_get_protocol_list(&env);
        protocol_list.push_back(protocol.clone());
        storage_set_protocol_list(&env, &protocol_list);

        // Update count
        let count = storage_get_protocol_count(&env);
        storage_set_protocol_count(&env, count + 1);

        Ok(())
    }

    fn get_protocol(env: Env, protocol: Address) -> Result<ProtocolRegistry, DeFiAuditError> {
        storage_get_protocol(&env, &protocol).ok_or(DeFiAuditError::ProtocolNotFound)
    }

    // ==================== TVL TRACKING ====================

    fn update_pool_tvl(
        env: Env,
        pool_id: BytesN<32>,
        protocol: Address,
        pool_name: Symbol,
        tvl_usd: u128,
        tvl_native: u128,
        lp_count: u32,
    ) -> Result<(), DeFiAuditError> {
        // Verify protocol exists
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let pool_tvl = PoolTVL {
            pool_id,
            protocol: protocol.clone(),
            pool_name,
            tvl_usd,
            tvl_native,
            lp_count,
            updated_at: env.ledger().timestamp(),
        };

        storage_set_pool_tvl(&env, &pool_id, &pool_tvl);

        // Update protocol pool list if new pool
        if !storage_pool_exists(&env, &protocol, &pool_id) {
            let mut pool_list = storage_get_protocol_pool_list(&env, &protocol);
            pool_list.push_back(pool_id);
            storage_set_protocol_pool_list(&env, &protocol, &pool_list);

            let count = storage_get_pool_count(&env, &protocol);
            storage_set_pool_count(&env, &protocol, count + 1);
        }

        // Update protocol TVL
        let current_tvl = storage_get_current_tvl(&env, &protocol).unwrap_or(0u128);
        let new_tvl = current_tvl.saturating_add(tvl_usd);
        storage_set_current_tvl(&env, &protocol, new_tvl);

        Ok(())
    }

    fn get_pool_tvl(env: Env, pool_id: BytesN<32>) -> Result<PoolTVL, DeFiAuditError> {
        storage_get_pool_tvl(&env, &pool_id).ok_or(DeFiAuditError::PoolNotFound)
    }

    fn get_protocol_tvl(env: Env, protocol: Address) -> Result<u128, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        Ok(storage_get_current_tvl(&env, &protocol).unwrap_or(0u128))
    }

    // ==================== ORACLE VERIFICATION ====================

    fn record_oracle_price(
        env: Env,
        oracle_id: BytesN<32>,
        asset: Address,
        price_usd: u128,
        source: Symbol,
        confidence_bp: u32,
        update_frequency: u64,
    ) -> Result<(), DeFiAuditError> {
        let now = env.ledger().timestamp();

        // Get previous price for history
        let price_history = storage_get_price_history(&env, &asset);
        let is_anomaly = if let Some(ref history) = price_history {
            is_price_anomaly(history.current_price, price_usd, 500u32) // 5% threshold
        } else {
            false
        };

        // Store oracle price
        let oracle_price = OraclePrice {
            oracle_id,
            asset: asset.clone(),
            price_usd,
            timestamp: now,
            source,
            confidence_bp,
            update_frequency,
        };

        storage_set_oracle_price(&env, &oracle_id, &oracle_price);

        // Update price history
        let new_history = PriceHistory {
            asset: asset.clone(),
            prev_price: price_history.as_ref().map(|h| h.current_price).unwrap_or(price_usd),
            current_price: price_usd,
            change_bp: if let Some(ref prev) = price_history {
                if prev.current_price > 0 {
                    ((price_usd as i128 - prev.current_price as i128) * 10000i128 / prev.current_price as i128) as i64
                } else {
                    0i64
                }
            } else {
                0i64
            },
            is_anomaly,
            timestamp: now,
        };

        storage_set_price_history(&env, &asset, &new_history);

        Ok(())
    }

    fn verify_price_anomaly(
        env: Env,
        asset: Address,
        current_price: u128,
        anomaly_threshold_bp: u32,
    ) -> Result<bool, DeFiAuditError> {
        let history = storage_get_price_history(&env, &asset)
            .ok_or(DeFiAuditError::InsufficientData)?;

        Ok(is_price_anomaly(history.current_price, current_price, anomaly_threshold_bp))
    }

    fn get_oracle_price(env: Env, oracle_id: BytesN<32>) -> Result<OraclePrice, DeFiAuditError> {
        storage_get_oracle_price(&env, &oracle_id).ok_or(DeFiAuditError::OraclePriceInvalid)
    }

    // ==================== LIQUIDATION MONITORING ====================

    fn record_liquidation(
        env: Env,
        protocol: Address,
        position: Address,
        liquidator: Address,
        collateral_asset: Address,
        debt_asset: Address,
        collateral_amount: u128,
        debt_amount: u128,
        liquidation_price: u128,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let event_id = compute_liquidation_id(&env, &protocol, &position);
        let now = env.ledger().timestamp();

        let liquidation = LiquidationEvent {
            event_id,
            protocol: protocol.clone(),
            position,
            liquidator,
            collateral_asset,
            debt_asset,
            collateral_amount,
            debt_amount,
            liquidation_price,
            timestamp: now,
        };

        storage_set_liquidation(&env, &event_id, &liquidation);

        // Update liquidation list
        let mut liq_list = storage_get_liquidation_list(&env, &protocol);
        liq_list.push_back(event_id);
        storage_set_liquidation_list(&env, &protocol, &liq_list);

        // Update count
        let count = storage_get_liquidation_count(&env, &protocol);
        storage_set_liquidation_count(&env, &protocol, count + 1);

        Ok(event_id)
    }

    fn add_at_risk_position(
        env: Env,
        protocol: Address,
        owner: Address,
        collateral_value: u128,
        debt_value: u128,
        health_factor: u128,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let liquidation_risk = if health_factor < 11000u128 {
            ((11000u128 - health_factor) * 100u128 / 11000u128).min(100u128) as u32
        } else {
            0u32
        };

        let position_id = compute_position_id(&env, &protocol, &owner);
        let now = env.ledger().timestamp();

        let position = AtRiskPosition {
            position_id,
            owner,
            protocol: protocol.clone(),
            collateral_value,
            debt_value,
            health_factor,
            liquidation_risk_percent: liquidation_risk,
            timestamp: now,
        };

        storage_set_at_risk_position(&env, &position_id, &position);

        // Update at-risk position list
        let mut risk_list = storage_get_at_risk_position_list(&env, &protocol);
        risk_list.push_back(position_id);
        storage_set_at_risk_position_list(&env, &protocol, &risk_list);

        // Update count
        let count = storage_get_at_risk_position_count(&env, &protocol);
        storage_set_at_risk_position_count(&env, &protocol, count + 1);

        Ok(position_id)
    }

    fn get_at_risk_position(
        env: Env,
        position_id: BytesN<32>,
    ) -> Result<AtRiskPosition, DeFiAuditError> {
        storage_get_at_risk_position(&env, &position_id)
            .ok_or(DeFiAuditError::InvalidParameter)
    }

    fn get_protocol_liquidations(env: Env, protocol: Address) -> Result<u32, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        Ok(storage_get_liquidation_count(&env, &protocol))
    }

    fn get_at_risk_positions(env: Env, protocol: Address) -> Result<u32, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        Ok(storage_get_at_risk_position_count(&env, &protocol))
    }

    // ==================== GOVERNANCE TRACKING ====================

    fn create_proposal(
        env: Env,
        protocol: Address,
        title: Bytes,
        description: Bytes,
        proposer: Address,
        start_time: u64,
        end_time: u64,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let proposal_id = compute_proposal_id(&env, &protocol);

        let proposal = GovernanceProposal {
            proposal_id,
            protocol: protocol.clone(),
            title,
            description,
            proposer,
            status: 1, // active
            votes_for: 0u128,
            votes_against: 0u128,
            votes_abstain: 0u128,
            start_time,
            end_time,
            execution_time: 0u64,
        };

        storage_set_proposal(&env, &proposal_id, &proposal);

        // Update proposal list
        let mut proposal_list = storage_get_proposal_list(&env, &protocol);
        proposal_list.push_back(proposal_id);
        storage_set_proposal_list(&env, &protocol, &proposal_list);

        // Update count
        let count = storage_get_proposal_count(&env, &protocol);
        storage_set_proposal_count(&env, &protocol, count + 1);

        Ok(proposal_id)
    }

    fn record_vote(
        env: Env,
        proposal_id: BytesN<32>,
        voter: Address,
        vote_direction: u32,
        voting_power: u128,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        if vote_direction > 2 {
            return Err(DeFiAuditError::InvalidParameter);
        }

        let mut proposal = storage_get_proposal(&env, &proposal_id)
            .ok_or(DeFiAuditError::GovernanceProposalNotFound)?;

        let vote_id = compute_vote_id(&env, &proposal_id, &voter);

        let voting_record = VotingRecord {
            vote_id,
            proposal_id,
            voter,
            vote_direction,
            voting_power,
            timestamp: env.ledger().timestamp(),
        };

        storage_set_voting_record(&env, &vote_id, &voting_record);

        // Update proposal votes
        match vote_direction {
            0 => proposal.votes_against = proposal.votes_against.saturating_add(voting_power),
            1 => proposal.votes_for = proposal.votes_for.saturating_add(voting_power),
            _ => proposal.votes_abstain = proposal.votes_abstain.saturating_add(voting_power),
        }

        storage_set_proposal(&env, &proposal_id, &proposal);

        Ok(vote_id)
    }

    fn update_proposal_status(
        env: Env,
        proposal_id: BytesN<32>,
        status: u32,
    ) -> Result<(), DeFiAuditError> {
        if status > 4 {
            return Err(DeFiAuditError::InvalidGovernanceState);
        }

        let mut proposal = storage_get_proposal(&env, &proposal_id)
            .ok_or(DeFiAuditError::GovernanceProposalNotFound)?;

        proposal.status = status;
        if status == 4 {
            // executed
            proposal.execution_time = env.ledger().timestamp();
        }

        storage_set_proposal(&env, &proposal_id, &proposal);

        Ok(())
    }

    fn get_proposal(env: Env, proposal_id: BytesN<32>) -> Result<GovernanceProposal, DeFiAuditError> {
        storage_get_proposal(&env, &proposal_id)
            .ok_or(DeFiAuditError::GovernanceProposalNotFound)
    }

    fn get_proposal_votes(env: Env, proposal_id: BytesN<32>) -> Result<(u128, u128, u128), DeFiAuditError> {
        let proposal = storage_get_proposal(&env, &proposal_id)
            .ok_or(DeFiAuditError::GovernanceProposalNotFound)?;

        Ok((proposal.votes_for, proposal.votes_against, proposal.votes_abstain))
    }

    // ==================== RISK METRICS ====================

    fn calculate_risk_metrics(
        env: Env,
        protocol: Address,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let tvl = storage_get_current_tvl(&env, &protocol).unwrap_or(0u128);
        let at_risk_count = storage_get_at_risk_position_count(&env, &protocol);
        let pool_count = storage_get_pool_count(&env, &protocol).max(1u32);

        // Concentration risk (simplified: assume top 3 pools have 60% of TVL)
        let concentration = 60u32; // placeholder

        // Average health factor (simplified)
        let avg_health = 15000u128; // placeholder

        // Liquidation risk: at-risk positions / total positions
        let liquidation_risk = (at_risk_count * 100 / pool_count.max(1u32)).min(100u32);

        // Price volatility (simplified)
        let volatility = 200u32; // 2% basis points

        // Overall health: 100 - (concentration * 0.2 + liquidation_risk * 0.5 + volatility * 0.3)
        let health_score = ((100u32
            .saturating_sub(concentration / 5)
            .saturating_sub(liquidation_risk / 2))
            .max(0u32))
        .min(100u32);

        let metrics_id = compute_metrics_id(&env, &protocol);

        let metrics = RiskMetrics {
            metrics_id,
            protocol: protocol.clone(),
            tvl_usd: tvl,
            concentration_risk: concentration,
            avg_health_factor: avg_health,
            liquidation_risk,
            price_volatility: volatility,
            protocol_health: health_score,
            updated_at: env.ledger().timestamp(),
        };

        storage_set_risk_metrics(&env, &metrics_id, &metrics);

        Ok(metrics_id)
    }

    fn get_risk_metrics(env: Env, metrics_id: BytesN<32>) -> Result<RiskMetrics, DeFiAuditError> {
        storage_get_risk_metrics(&env, &metrics_id).ok_or(DeFiAuditError::RiskCalculationError)
    }

    fn get_protocol_health_score(env: Env, protocol: Address) -> Result<u32, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        // Simplified: return a fixed score for now (in production, calculate from metrics)
        Ok(75u32)
    }

    // ==================== AUDIT REPORTS ====================

    fn generate_audit_report(
        env: Env,
        protocol: Address,
        period_start: u64,
        period_end: u64,
        findings_hash: BytesN<32>,
    ) -> Result<BytesN<32>, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;

        let report_id = compute_report_id(&env, &protocol);
        let tvl = storage_get_current_tvl(&env, &protocol).unwrap_or(0u128);
        let liquidation_count = storage_get_liquidation_count(&env, &protocol);

        let report = AuditReport {
            report_id,
            protocol: protocol.clone(),
            period_start,
            period_end,
            avg_tvl: tvl,
            peak_tvl: tvl,
            min_tvl: tvl,
            total_liquidations: liquidation_count,
            liquidation_value: 0u128,
            proposals_count: storage_get_proposal_count(&env, &protocol),
            avg_participation: 5000u32, // 50% placeholder
            health_score: 75u32,
            findings_hash,
            generated_at: env.ledger().timestamp(),
        };

        storage_set_audit_report(&env, &report_id, &report);

        // Update report list
        let mut report_list = storage_get_report_list(&env, &protocol);
        report_list.push_back(report_id);
        storage_set_report_list(&env, &protocol, &report_list);

        // Update count
        let count = storage_get_report_count(&env, &protocol);
        storage_set_report_count(&env, &protocol, count + 1);

        // Update last audit time
        storage_set_last_audit_time(&env, &protocol, env.ledger().timestamp());

        Ok(report_id)
    }

    fn get_audit_report(env: Env, report_id: BytesN<32>) -> Result<AuditReport, DeFiAuditError> {
        storage_get_audit_report(&env, &report_id).ok_or(DeFiAuditError::ReportGenerationFailed)
    }

    fn get_latest_audit_report(env: Env, protocol: Address) -> Result<AuditReport, DeFiAuditError> {
        let report_list = storage_get_report_list(&env, &protocol);
        if report_list.len() == 0 {
            return Err(DeFiAuditError::ReportGenerationFailed);
        }

        let latest_id = report_list.get(report_list.len() - 1).unwrap();
        storage_get_audit_report(&env, &latest_id).ok_or(DeFiAuditError::ReportGenerationFailed)
    }

    // ==================== QUERY FUNCTIONS ====================

    fn total_protocol_count(env: Env) -> u32 {
        storage_get_protocol_count(&env)
    }

    fn protocol_tvl(env: Env, protocol: Address) -> Result<u128, DeFiAuditError> {
        let _ = storage_get_protocol(&env, &protocol)
            .ok_or(DeFiAuditError::ProtocolNotFound)?;
        Ok(storage_get_current_tvl(&env, &protocol).unwrap_or(0u128))
    }

    fn protocol_pool_count(env: Env, protocol: Address) -> u32 {
        storage_get_pool_count(&env, &protocol)
    }

    fn protocol_liquidation_count(env: Env, protocol: Address) -> u32 {
        storage_get_liquidation_count(&env, &protocol)
    }

    fn protocol_at_risk_count(env: Env, protocol: Address) -> u32 {
        storage_get_at_risk_position_count(&env, &protocol)
    }

    fn protocol_proposal_count(env: Env, protocol: Address) -> u32 {
        storage_get_proposal_count(&env, &protocol)
    }

    fn protocol_report_count(env: Env, protocol: Address) -> u32 {
        storage_get_report_count(&env, &protocol)
    }
}

// ==================== STORAGE HELPERS ====================

fn storage_get_protocol(env: &Env, protocol: &Address) -> Option<ProtocolRegistry> {
    let key = DeFiAuditDataKey::Protocol(protocol.clone());
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_protocol(env: &Env, protocol: &Address, registry: &ProtocolRegistry) {
    let key = DeFiAuditDataKey::Protocol(protocol.clone());
    env.storage().persistent().set(&key, registry);
}

fn storage_get_protocol_list(env: &Env) -> Vec<Address> {
    let key = DeFiAuditDataKey::ProtocolList;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_protocol_list(env: &Env, list: &Vec<Address>) {
    let key = DeFiAuditDataKey::ProtocolList;
    env.storage().persistent().set(&key, list);
}

fn storage_get_protocol_count(env: &Env) -> u32 {
    let key = DeFiAuditDataKey::ProtocolCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_protocol_count(env: &Env, count: u32) {
    let key = DeFiAuditDataKey::ProtocolCount;
    env.storage().persistent().set(&key, &count);
}

// Pool TVL storage helpers
fn storage_get_pool_tvl(env: &Env, pool_id: &BytesN<32>) -> Option<PoolTVL> {
    let key = DeFiAuditDataKey::PoolTVL(*pool_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_pool_tvl(env: &Env, pool_id: &BytesN<32>, pool: &PoolTVL) {
    let key = DeFiAuditDataKey::PoolTVL(*pool_id);
    env.storage().persistent().set(&key, pool);
}

fn storage_get_protocol_pool_list(env: &Env, protocol: &Address) -> Vec<BytesN<32>> {
    let key = DeFiAuditDataKey::ProtocolPoolList(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_protocol_pool_list(env: &Env, protocol: &Address, list: &Vec<BytesN<32>>) {
    let key = DeFiAuditDataKey::ProtocolPoolList(protocol.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_pool_count(env: &Env, protocol: &Address) -> u32 {
    let key = DeFiAuditDataKey::PoolCount(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_pool_count(env: &Env, protocol: &Address, count: u32) {
    let key = DeFiAuditDataKey::PoolCount(protocol.clone());
    env.storage().persistent().set(&key, &count);
}

fn storage_pool_exists(env: &Env, protocol: &Address, pool_id: &BytesN<32>) -> bool {
    let pool_list = storage_get_protocol_pool_list(env, protocol);
    for p in pool_list.iter() {
        if p == *pool_id {
            return true;
        }
    }
    false
}

// Oracle price storage helpers
fn storage_get_oracle_price(env: &Env, oracle_id: &BytesN<32>) -> Option<OraclePrice> {
    let key = DeFiAuditDataKey::OraclePrice(*oracle_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_oracle_price(env: &Env, oracle_id: &BytesN<32>, price: &OraclePrice) {
    let key = DeFiAuditDataKey::OraclePrice(*oracle_id);
    env.storage().persistent().set(&key, price);
}

fn storage_get_price_history(env: &Env, asset: &Address) -> Option<PriceHistory> {
    let key = DeFiAuditDataKey::PriceHistory(asset.clone());
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_price_history(env: &Env, asset: &Address, history: &PriceHistory) {
    let key = DeFiAuditDataKey::PriceHistory(asset.clone());
    env.storage().persistent().set(&key, history);
}

// Liquidation storage helpers
fn storage_get_liquidation(env: &Env, event_id: &BytesN<32>) -> Option<LiquidationEvent> {
    let key = DeFiAuditDataKey::Liquidation(*event_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_liquidation(env: &Env, event_id: &BytesN<32>, event: &LiquidationEvent) {
    let key = DeFiAuditDataKey::Liquidation(*event_id);
    env.storage().persistent().set(&key, event);
}

fn storage_get_liquidation_list(env: &Env, protocol: &Address) -> Vec<BytesN<32>> {
    let key = DeFiAuditDataKey::LiquidationList(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_liquidation_list(env: &Env, protocol: &Address, list: &Vec<BytesN<32>>) {
    let key = DeFiAuditDataKey::LiquidationList(protocol.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_liquidation_count(env: &Env, protocol: &Address) -> u32 {
    let key = DeFiAuditDataKey::LiquidationCount(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_liquidation_count(env: &Env, protocol: &Address, count: u32) {
    let key = DeFiAuditDataKey::LiquidationCount(protocol.clone());
    env.storage().persistent().set(&key, &count);
}

// At-risk position storage helpers
fn storage_get_at_risk_position(env: &Env, position_id: &BytesN<32>) -> Option<AtRiskPosition> {
    let key = DeFiAuditDataKey::AtRiskPosition(*position_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_at_risk_position(env: &Env, position_id: &BytesN<32>, position: &AtRiskPosition) {
    let key = DeFiAuditDataKey::AtRiskPosition(*position_id);
    env.storage().persistent().set(&key, position);
}

fn storage_get_at_risk_position_list(env: &Env, protocol: &Address) -> Vec<BytesN<32>> {
    let key = DeFiAuditDataKey::AtRiskPositionList(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_at_risk_position_list(env: &Env, protocol: &Address, list: &Vec<BytesN<32>>) {
    let key = DeFiAuditDataKey::AtRiskPositionList(protocol.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_at_risk_position_count(env: &Env, protocol: &Address) -> u32 {
    let key = DeFiAuditDataKey::AtRiskPositionCount(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_at_risk_position_count(env: &Env, protocol: &Address, count: u32) {
    let key = DeFiAuditDataKey::AtRiskPositionCount(protocol.clone());
    env.storage().persistent().set(&key, &count);
}

// Governance storage helpers
fn storage_get_proposal(env: &Env, proposal_id: &BytesN<32>) -> Option<GovernanceProposal> {
    let key = DeFiAuditDataKey::GovernanceProposal(*proposal_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_proposal(env: &Env, proposal_id: &BytesN<32>, proposal: &GovernanceProposal) {
    let key = DeFiAuditDataKey::GovernanceProposal(*proposal_id);
    env.storage().persistent().set(&key, proposal);
}

fn storage_get_proposal_list(env: &Env, protocol: &Address) -> Vec<BytesN<32>> {
    let key = DeFiAuditDataKey::ProposalList(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_proposal_list(env: &Env, protocol: &Address, list: &Vec<BytesN<32>>) {
    let key = DeFiAuditDataKey::ProposalList(protocol.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_proposal_count(env: &Env, protocol: &Address) -> u32 {
    let key = DeFiAuditDataKey::ProposalCount(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_proposal_count(env: &Env, protocol: &Address, count: u32) {
    let key = DeFiAuditDataKey::ProposalCount(protocol.clone());
    env.storage().persistent().set(&key, &count);
}

fn storage_set_voting_record(env: &Env, vote_id: &BytesN<32>, record: &VotingRecord) {
    let key = DeFiAuditDataKey::VotingRecord(*vote_id);
    env.storage().persistent().set(&key, record);
}

// Risk metrics storage helpers
fn storage_get_risk_metrics(env: &Env, metrics_id: &BytesN<32>) -> Option<RiskMetrics> {
    let key = DeFiAuditDataKey::RiskMetrics(*metrics_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_risk_metrics(env: &Env, metrics_id: &BytesN<32>, metrics: &RiskMetrics) {
    let key = DeFiAuditDataKey::RiskMetrics(*metrics_id);
    env.storage().persistent().set(&key, metrics);
}

// Audit report storage helpers
fn storage_get_audit_report(env: &Env, report_id: &BytesN<32>) -> Option<AuditReport> {
    let key = DeFiAuditDataKey::AuditReport(*report_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_audit_report(env: &Env, report_id: &BytesN<32>, report: &AuditReport) {
    let key = DeFiAuditDataKey::AuditReport(*report_id);
    env.storage().persistent().set(&key, report);
}

fn storage_get_report_list(env: &Env, protocol: &Address) -> Vec<BytesN<32>> {
    let key = DeFiAuditDataKey::ReportList(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_report_list(env: &Env, protocol: &Address, list: &Vec<BytesN<32>>) {
    let key = DeFiAuditDataKey::ReportList(protocol.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_report_count(env: &Env, protocol: &Address) -> u32 {
    let key = DeFiAuditDataKey::ReportCount(protocol.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_report_count(env: &Env, protocol: &Address, count: u32) {
    let key = DeFiAuditDataKey::ReportCount(protocol.clone());
    env.storage().persistent().set(&key, &count);
}

fn storage_set_last_audit_time(env: &Env, protocol: &Address, timestamp: u64) {
    let key = DeFiAuditDataKey::LastAuditTime(protocol.clone());
    env.storage().persistent().set(&key, &timestamp);
}

fn storage_get_current_tvl(env: &Env, protocol: &Address) -> Option<u128> {
    let key = DeFiAuditDataKey::CurrentTVL(protocol.clone());
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_current_tvl(env: &Env, protocol: &Address, tvl: u128) {
    let key = DeFiAuditDataKey::CurrentTVL(protocol.clone());
    env.storage().persistent().set(&key, &tvl);
}

// ==================== ID GENERATION ====================

fn compute_liquidation_id(env: &Env, protocol: &Address, position: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    let addr_bytes = protocol.to_string().as_bytes();
    if addr_bytes.len() <= 16 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_position_id(env: &Env, protocol: &Address, owner: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    let addr_bytes = owner.to_string().as_bytes();
    if addr_bytes.len() <= 32 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_proposal_id(env: &Env, protocol: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let addr_bytes = protocol.to_string().as_bytes();
    let mut data = [0u8; 40];
    if addr_bytes.len() <= 32 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_vote_id(env: &Env, proposal_id: &BytesN<32>, voter: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&proposal_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_metrics_id(env: &Env, protocol: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let addr_bytes = protocol.to_string().as_bytes();
    let mut data = [0u8; 40];
    if addr_bytes.len() <= 32 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_report_id(env: &Env, protocol: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let addr_bytes = protocol.to_string().as_bytes();
    let mut data = [0u8; 40];
    if addr_bytes.len() <= 32 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}
