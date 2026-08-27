// CBDC Integration Tests
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Bytes, BytesN, Env, Symbol, Vec};

// Import CBDC modules
use crate::cbdc_types::*;
use crate::cbdc_logging::*;
use crate::cbdc_interop::*;
use crate::cbdc_offline::*;
use crate::cbdc_privacy::*;

fn create_test_env() -> Env {
    Env::default()
}

fn create_test_transaction(env: &Env) -> CBDCTransaction {
    CBDCTransaction {
        tx_id: BytesN::zero(),
        source_pilot: CBDCPilot::DigitalEuro as u8,
        dest_pilot: CBDCPilot::DigitalDollar as u8,
        from: soroban_sdk::Address::generate(env),
        to: soroban_sdk::Address::generate(env),
        amount_source: 1_000_00,
        amount_dest: 1_120_00,
        exchange_rate: 1_120_000_000_000_000_000,
        timestamp: env.ledger().timestamp(),
        protocol: InteropProtocol::AtomicSwap as u8,
        privacy_tier: PrivacyTier::Public as u8,
        offline_status: None,
        metadata: Bytes::new(env),
    }
}

// ============ CBDC Types Tests ============

#[test]
fn test_cbdc_pilot_currency_codes() {
    assert_eq!(CBDCPilot::DigitalEuro.currency_code(), "EUR");
    assert_eq!(CBDCPilot::DigitalDollar.currency_code(), "USD");
    assert_eq!(CBDCPilot::eCNY.currency_code(), "CNY");
    assert_eq!(CBDCPilot::SandDollar.currency_code(), "BSD");
}

#[test]
fn test_cbdc_pilot_from_code() {
    assert_eq!(
        CBDCPilot::from_code("EUR"),
        Some(CBDCPilot::DigitalEuro)
    );
    assert_eq!(
        CBDCPilot::from_code("USD"),
        Some(CBDCPilot::DigitalDollar)
    );
    assert_eq!(CBDCPilot::from_code("CNY"), Some(CBDCPilot::eCNY));
    assert_eq!(CBDCPilot::from_code("BSD"), Some(CBDCPilot::SandDollar));
    assert_eq!(CBDCPilot::from_code("GBP"), None);
}

#[test]
fn test_privacy_tier_encryption_requirement() {
    assert!(!PrivacyTier::Public.requires_encryption());
    assert!(PrivacyTier::Pseudonymous.requires_encryption());
    assert!(PrivacyTier::Private.requires_encryption());
    assert!(PrivacyTier::RegulatoryConfidential.requires_encryption());
}

#[test]
fn test_offline_status_settlement() {
    assert!(!OfflineStatus::PendingReconciliation.is_settled());
    assert!(OfflineStatus::Reconciled.is_settled());
    assert!(!OfflineStatus::FailedReconciliation.is_settled());
    assert!(!OfflineStatus::Disputed.is_settled());
}

#[test]
fn test_cbdc_config_validation() {
    let config = CBDCConfig::default();
    assert!(config.is_valid_amount(100));
    assert!(config.is_valid_amount(1_000_000_00)); // Max is 1M
    assert!(!config.is_valid_amount(0));
    assert!(!config.is_valid_amount(2_000_000_00)); // Over max
}

// ============ CBDC Logging Tests ============

#[test]
fn test_cbdc_event_creation_success() {
    let env = create_test_env();
    let tx = create_test_transaction(&env);

    let event = CBDCEvent::transaction_success(
        &env,
        1,
        env.ledger().timestamp(),
        tx,
        Symbol::new(&env, "test_event"),
    );

    assert!(event.is_success());
    assert_eq!(event.event_index, 1);
    assert_eq!(event.error_message, None);
}

#[test]
fn test_cbdc_event_creation_failure() {
    let env = create_test_env();
    let tx = create_test_transaction(&env);
    let error = Bytes::from_slice(&env, b"Test error");

    let event = CBDCEvent::transaction_failed(
        &env,
        1,
        env.ledger().timestamp(),
        tx,
        Symbol::new(&env, "test_event"),
        error.clone(),
    );

    assert!(!event.is_success());
    assert_eq!(event.error_message, Some(error));
}

#[test]
fn test_cbdc_event_config_full() {
    let mut config = CBDCEventConfig::default();
    assert!(config.can_log_more());
    assert!(!config.is_full());

    config.event_count = config.max_events;
    assert!(!config.can_log_more());
    assert!(config.is_full());
}

#[test]
fn test_cbdc_event_config_increment() {
    let mut config = CBDCEventConfig::default();
    assert_eq!(config.event_count, 0);

    assert!(config.increment_count().is_ok());
    assert_eq!(config.event_count, 1);

    config.event_count = config.max_events;
    assert!(config.increment_count().is_err());
}

#[test]
fn test_cbdc_logger_validate_transaction() {
    let env = create_test_env();
    let tx = create_test_transaction(&env);

    // Valid transaction should pass
    assert!(CBDCLogger::validate_transaction(&tx).is_ok());

    // Zero source amount should fail
    let mut bad_tx = tx.clone();
    bad_tx.amount_source = 0;
    assert!(CBDCLogger::validate_transaction(&bad_tx).is_err());

    // Zero destination amount should fail
    let mut bad_tx = tx.clone();
    bad_tx.amount_dest = 0;
    assert!(CBDCLogger::validate_transaction(&bad_tx).is_err());

    // Zero exchange rate should fail
    let mut bad_tx = tx.clone();
    bad_tx.exchange_rate = 0;
    assert!(CBDCLogger::validate_transaction(&bad_tx).is_err());

    // Same source and dest pilots should fail
    let mut bad_tx = tx.clone();
    bad_tx.dest_pilot = bad_tx.source_pilot;
    assert!(CBDCLogger::validate_transaction(&bad_tx).is_err());
}

#[test]
fn test_cbdc_event_stats_success_rate() {
    let env = create_test_env();
    let mut stats = CBDCEventStats::new(&env);

    stats.total_success = 80;
    stats.total_failed = 20;
    assert_eq!(stats.success_rate(), 80);

    stats.total_success = 100;
    stats.total_failed = 0;
    assert_eq!(stats.success_rate(), 100);

    stats.total_success = 0;
    stats.total_failed = 0;
    assert_eq!(stats.success_rate(), 100); // Default 100% for empty
}

// ============ CBDC Interoperability Tests ============

#[test]
fn test_exchange_rate_staleness() {
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: 1_000_000_000_000_000_000,
        timestamp: 1000,
        bid_price: 950_000_000_000_000_000,
        ask_price: 1_050_000_000_000_000_000,
        max_volatility: 500,
    };

    assert!(!rate.is_stale(2000, 3600)); // 1000 seconds old, threshold 1 hour
    assert!(rate.is_stale(10000, 3600)); // 9000 seconds old, exceeds threshold
}

#[test]
fn test_exchange_rate_mid_price() {
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: 1_000_000_000_000_000_000,
        timestamp: 1000,
        bid_price: 100_000_000_000_000_000u128,
        ask_price: 110_000_000_000_000_000u128,
        max_volatility: 500,
    };

    let mid = rate.mid_price();
    assert_eq!(mid, 105_000_000_000_000_000);
}

#[test]
fn test_amount_conversion() {
    let amount = 1_000_000_000_000_000_000u128; // 1e18
    let rate = 1_200_000_000_000_000_000u128; // 1.2x

    let result = InteropManager::convert_amount(amount, rate).unwrap();
    assert_eq!(result, 1_200_000_000_000_000_000); // 1.2e18
}

#[test]
fn test_settlement_fee_calculation() {
    let amount = 1_000_000u128;
    let fee_bps = 25; // 0.25%

    let fee = InteropManager::compute_settlement_fee(amount, fee_bps).unwrap();
    assert_eq!(fee, 250); // 0.25% of 1M
}

#[test]
fn test_settlement_status_transitions() {
    // Valid transitions
    assert!(InteropManager::is_valid_status_transition(
        SettlementStatus::Created,
        SettlementStatus::InitiatedSource
    ));

    assert!(InteropManager::is_valid_status_transition(
        SettlementStatus::ConfirmedSource,
        SettlementStatus::AwaitingDestination
    ));

    assert!(InteropManager::is_valid_status_transition(
        SettlementStatus::AwaitingDestination,
        SettlementStatus::Completed
    ));

    // Invalid transitions
    assert!(!InteropManager::is_valid_status_transition(
        SettlementStatus::Created,
        SettlementStatus::Completed
    ));

    // Failure is always allowed
    assert!(InteropManager::is_valid_status_transition(
        SettlementStatus::AwaitingDestination,
        SettlementStatus::Failed
    ));
}

#[test]
fn test_pilot_pair_validation() {
    // Valid pairs (different pilots)
    assert!(InteropManager::validate_pilot_pair(0, 1).is_ok());
    assert!(InteropManager::validate_pilot_pair(2, 3).is_ok());

    // Invalid: same pilot
    assert!(InteropManager::validate_pilot_pair(0, 0).is_err());

    // Invalid: out of range
    assert!(InteropManager::validate_pilot_pair(0, 5).is_err());
    assert!(InteropManager::validate_pilot_pair(10, 1).is_err());
}

// ============ CBDC Offline Tests ============

#[test]
fn test_offline_transaction_creation() {
    let env = create_test_env();
    let tx = create_test_transaction(&env);
    let signature = Bytes::from_slice(&env, b"test_signature");
    let pubkey = Bytes::from_slice(&env, b"test_pubkey");

    let offline_tx = OfflineManager::create_offline_transaction(
        &env, tx, signature.clone(), pubkey.clone(), 1,
    )
    .unwrap();

    assert_eq!(offline_tx.status, OfflineStatus::PendingReconciliation as u8);
    assert!(!offline_tx.is_settled());
    assert!(!offline_tx.is_failed());
}

#[test]
fn test_offline_transaction_validation() {
    let env = create_test_env();
    let tx = create_test_transaction(&env);
    let signature = Bytes::from_slice(&env, b"sig");
    let pubkey = Bytes::from_slice(&env, b"key");

    let offline_tx = OfflineManager::create_offline_transaction(
        &env, tx, signature, pubkey, 1,
    )
    .unwrap();

    assert!(OfflineManager::validate_offline_transaction(&offline_tx).is_ok());

    // Invalid: zero nonce
    let mut bad_tx = offline_tx.clone();
    bad_tx.nonce = 0;
    assert!(OfflineManager::validate_offline_transaction(&bad_tx).is_err());
}

#[test]
fn test_nonce_replay_detection() {
    // Valid: increasing nonce
    assert!(OfflineManager::verify_nonce(1, 2).is_ok());
    assert!(OfflineManager::verify_nonce(100, 101).is_ok());

    // Invalid: same nonce (replay)
    assert!(OfflineManager::verify_nonce(5, 5).is_err());

    // Invalid: decreasing nonce
    assert!(OfflineManager::verify_nonce(5, 3).is_err());
    assert!(OfflineManager::verify_nonce(100, 99).is_err());
}

#[test]
fn test_reconciliation_state_calculation() {
    let env = create_test_env();

    let statuses = Vec::from_array(
        &env,
        &[
            OfflineStatus::Reconciled as u8,
            OfflineStatus::Reconciled as u8,
            OfflineStatus::FailedReconciliation as u8,
            OfflineStatus::PendingReconciliation as u8,
        ],
    );

    let state = OfflineManager::compute_reconciliation_state(&statuses);
    assert_eq!(state.total_transactions, 4);
    assert_eq!(state.successful_count, 2);
    assert_eq!(state.failed_count, 1);
    assert_eq!(state.pending_count, 1);
    assert_eq!(state.disputed_count, 0);
    assert_eq!(state.reconciliation_rate(), 50); // 2/4 = 50%
}

#[test]
fn test_reconciliation_all_settled() {
    let mut state = ReconciliationState::new();
    state.total_transactions = 5;
    state.successful_count = 4;
    assert!(!state.all_settled());

    state.successful_count = 5;
    assert!(state.all_settled());
}

#[test]
fn test_reconciliation_any_failed() {
    let mut state = ReconciliationState::new();
    state.total_transactions = 5;
    state.successful_count = 5;
    assert!(!state.any_failed());

    state.failed_count = 1;
    assert!(state.any_failed());
}

// ============ CBDC Privacy Tests ============

#[test]
fn test_privacy_acl_access_check() {
    let env = create_test_env();
    let user = soroban_sdk::Address::generate(&env);
    let other = soroban_sdk::Address::generate(&env);

    let mut read_access = Vec::new(&env);
    read_access.push_back(user.clone());

    let acl = PrivacyACL {
        acl_id: BytesN::zero(),
        transaction_hash: BytesN::zero(),
        read_access,
        audit_access: Vec::new(&env),
        regulatory_access: Vec::new(&env),
        expires_at: 0,
    };

    assert!(acl.has_read_access(&user));
    assert!(!acl.has_read_access(&other));
}

#[test]
fn test_privacy_acl_expiration() {
    let env = create_test_env();
    let current_time = 1000u64;

    let acl = PrivacyACL {
        acl_id: BytesN::zero(),
        transaction_hash: BytesN::zero(),
        read_access: Vec::new(&env),
        audit_access: Vec::new(&env),
        regulatory_access: Vec::new(&env),
        expires_at: 2000, // Expires at timestamp 2000
    };

    assert!(!acl.is_expired(current_time)); // Before expiration
    assert!(acl.is_expired(3000)); // After expiration
}

#[test]
fn test_privacy_tier_requirements() {
    // Public shouldn't need encryption
    assert!(!PrivacyTier::Public.requires_encryption());

    // All others need encryption
    assert!(PrivacyTier::Pseudonymous.requires_encryption());
    assert!(PrivacyTier::Private.requires_encryption());
    assert!(PrivacyTier::RegulatoryConfidential.requires_encryption());
}

#[test]
fn test_access_level_permissions() {
    assert!(!AccessLevel::None.can_read_full_details());
    assert!(!AccessLevel::AuditOnly.can_read_full_details());
    assert!(AccessLevel::Read.can_read_full_details());
    assert!(AccessLevel::RegulatoryFull.can_read_full_details());

    assert!(!AccessLevel::None.can_audit());
    assert!(AccessLevel::AuditOnly.can_audit());
    assert!(AccessLevel::Read.can_audit());
    assert!(AccessLevel::RegulatoryFull.can_audit());
}

#[test]
fn test_privacy_stats_aggregation() {
    let env = create_test_env();
    let mut stats = PrivacyStats::new();

    assert_eq!(stats.total_count(), 0);

    stats.record_transaction(PrivacyTier::Public);
    stats.record_transaction(PrivacyTier::Public);
    stats.record_transaction(PrivacyTier::Private);

    assert_eq!(stats.total_count(), 3);
    assert_eq!(stats.public_count, 2);
    assert_eq!(stats.private_count, 1);
    assert_eq!(stats.privacy_ratio(), 33); // 1/3 encrypted
}

// ============ Integration Tests ============

#[test]
fn test_cross_cbdc_transfer_workflow() {
    let env = create_test_env();

    // 1. Create transaction
    let tx = create_test_transaction(&env);

    // 2. Validate
    assert!(CBDCLogger::validate_transaction(&tx).is_ok());

    // 3. Create exchange rate
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: 1_120_000_000_000_000_000,
        timestamp: env.ledger().timestamp(),
        bid_price: 1_100_000_000_000_000_000,
        ask_price: 1_140_000_000_000_000_000,
        max_volatility: 500,
    };

    // 4. Validate rate
    assert!(
        InteropManager::validate_exchange_rate(&rate, env.ledger().timestamp(), 3600).is_ok()
    );

    // 5. Convert amount
    let converted = InteropManager::convert_amount(1_000_00, rate.rate).unwrap();
    assert!(converted > 0);

    // 6. Log transaction
    let event = CBDCLogger::log_transaction_success(
        &env,
        1,
        tx,
        TransactionEventType::CrossCBDCTransfer,
    );

    assert!(event.is_success());
}

#[test]
fn test_offline_batch_workflow() {
    let env = create_test_env();

    // 1. Create offline transactions
    let tx1 = create_test_transaction(&env);
    let tx2 = create_test_transaction(&env);

    let sig = Bytes::from_slice(&env, b"sig");
    let pubkey = Bytes::from_slice(&env, b"key");

    let offline_tx1 = OfflineManager::create_offline_transaction(&env, tx1, sig.clone(), pubkey.clone(), 1)
        .unwrap();
    let offline_tx2 = OfflineManager::create_offline_transaction(&env, tx2, sig, pubkey, 2)
        .unwrap();

    // 2. Validate both
    assert!(OfflineManager::validate_offline_transaction(&offline_tx1).is_ok());
    assert!(OfflineManager::validate_offline_transaction(&offline_tx2).is_ok());

    // 3. Create batch
    let mut tx_ids = Vec::new(&env);
    tx_ids.push_back(offline_tx1.tx_hash);
    tx_ids.push_back(offline_tx2.tx_hash);

    let batch =
        OfflineManager::create_batch_settlement(&env, tx_ids, 2_000_00).unwrap();
    assert_eq!(batch.transaction_ids.len(), 2);
}

#[test]
fn test_privacy_masking_workflow() {
    let env = create_test_env();

    // 1. Create transaction
    let tx = create_test_transaction(&env);

    // 2. Mask with privacy tier
    let user = soroban_sdk::Address::generate(&env);
    let mut authorized = Vec::new(&env);
    authorized.push_back(user.clone());

    let masked = PrivacyManager::mask_transaction(
        &env,
        &tx,
        PrivacyTier::Private,
        authorized,
    )
    .unwrap();

    assert_eq!(masked.privacy_tier, PrivacyTier::Private as u8);
    assert!(masked.is_encrypted());

    // 3. Create ACL
    let acl = PrivacyManager::create_privacy_acl(
        &env,
        masked.content_hash,
        Vec::new(&env),
        Vec::new(&env),
        Vec::new(&env),
        0,
    )
    .unwrap();

    assert_eq!(acl.transaction_hash, masked.content_hash);
}
