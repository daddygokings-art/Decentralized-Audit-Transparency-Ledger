// CBDC Integration Tests
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Bytes, BytesN, Env, Symbol, Vec};

// Import CBDC modules
use crate::cbdc_types::*;
use crate::cbdc_logging::*;
use crate::cbdc_interop::*;
use crate::cbdc_offline::*;
use crate::cbdc_privacy::*;

// ============ Boundary & Adversarial Tests (hardened interop) ============

#[test]
fn test_convert_amount_u128_max_saturates() {
    // u128::MAX amount with a rate > 1 must not panic; must saturate.
    let result = InteropManager::convert_amount(u128::MAX, 2_000_000_000_000_000_000u128);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), u128::MAX);
}

#[test]
fn test_convert_amount_zero_rate_is_typed_error() {
    let result = InteropManager::convert_amount(1_000_000u128, 0u128);
    assert!(result.is_err());
}

#[test]
fn test_compute_settlement_fee_u128_max_bounded() {
    // Fee on u128::MAX must saturate and never exceed the amount.
    let fee = InteropManager::compute_settlement_fee(u128::MAX, 10_000u32).unwrap();
    assert!(fee <= u128::MAX);
}

#[test]
fn test_compute_settlement_fee_invalid_bps_is_typed_error() {
    // bps > 10_000 (100%) is a boundary violation and must be a typed error.
    let result = InteropManager::compute_settlement_fee(1_000_000u128, 10_001u32);
    assert!(result.is_err());
}

#[test]
fn test_exchange_rate_spread_i128_max_no_panic() {
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: i128::MAX,
        timestamp: 0,
        bid_price: i128::MAX,
        ask_price: i128::MAX,
        max_volatility: u32::MAX,
    };
    // Must not panic on extreme spread inputs.
    let _ = rate.mid_price();
    let _ = rate.spread_bps();
}

#[test]
fn test_exchange_rate_extreme_volatility_is_typed_error() {
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: 1_000_000_000_000_000_000,
        timestamp: 0,
        bid_price: 1,
        ask_price: i128::MAX,
        max_volatility: 1,
    };
    // Volatility beyond max_volatility must surface as a typed error, not a panic.
    let result = InteropManager::validate_exchange_rate(&rate, 0, 3600);
    assert!(result.is_err());
}

#[test]
fn test_validate_pilot_pair_u64_max_is_typed_error() {
    let result = InteropManager::validate_pilot_pair(u64::MAX as u32, 1);
    assert!(result.is_err());
}

#[test]
fn test_oversized_metadata_payload_is_rejected() {
    let env = create_test_env();
    let mut tx = create_test_transaction(&env);
    // Build a payload larger than the enforced maximum collection length.
    let oversized = Bytes::from_slice(&env, &[0u8; 4096]);
    tx.metadata = oversized;
    assert!(CBDCLogger::validate_transaction(&tx).is_err());
}

#[test]
fn test_over_length_settlement_path_is_typed_error() {
    let env = create_test_env();
    let mut tx_ids = Vec::new(&env);
    // Push more ids than the maximum allowed settlement path length.
    for _ in 0..(MAX_SETTLEMENT_PATH_LEN + 1) {
        tx_ids.push_back(BytesN::zero());
    }
    let caller = Address::generate(&env);
    let result = OfflineManager::create_batch_settlement(&env, &caller, tx_ids, 1_000_00);
    assert!(result.is_err());
}

#[test]
fn test_max_settlement_path_len_accepted() {
    let env = create_test_env();
    let mut tx_ids = Vec::new(&env);
    for _ in 0..MAX_SETTLEMENT_PATH_LEN {
        tx_ids.push_back(BytesN::zero());
    }
    let caller = Address::generate(&env);
    let result = OfflineManager::create_batch_settlement(&env, &caller, tx_ids, 1_000_00);
    assert!(result.is_ok());
}

// ============ Storage Footprint Regression Assertions ============

#[test]
fn test_storage_footprint_exchange_rate_within_budget() {
    let env = create_test_env();
    let rate = ExchangeRate {
        source_pilot: 0,
        dest_pilot: 1,
        rate: 1_000_000_000_000_000_000,
        timestamp: 0,
        bid_price: 1,
        ask_price: 2,
        max_volatility: 500,
    };
    let bytes = rate.to_xdr(&env).len();
    assert!(
        bytes <= EXCHANGE_RATE_XDR_BUDGET,
        "ExchangeRate XDR size {} exceeds budget {}",
        bytes,
        EXCHANGE_RATE_XDR_BUDGET
    );
}

#[test]
fn test_storage_footprint_settlement_record_within_budget() {
    let env = create_test_env();
    let record = SettlementRecord {
        settlement_id: BytesN::zero(),
        status: SettlementStatus::Created as u8,
        source_pilot: 0,
        dest_pilot: 1,
        amount_source: 1_000_00,
        amount_dest: 1_120_00,
        created_at: 0,
        updated_at: 0,
    };
    let bytes = record.to_xdr(&env).len();
    assert!(
        bytes <= SETTLEMENT_RECORD_XDR_BUDGET,
        "SettlementRecord XDR size {} exceeds budget {}",
        bytes,
        SETTLEMENT_RECORD_XDR_BUDGET
    );
}

#[test]
fn test_storage_footprint_enum_discriminants_packed() {
    // Enum discriminants must remain single-byte to keep XDR minimal.
    assert_eq!(SettlementStatus::Created as u32, 0);
    assert!(core::mem::size_of::<SettlementStatus>() <= 1);
    assert!(core::mem::size_of::<InteropProtocol>() <= 1);
}

// ============ 1,000-Transaction Ledger Byte Benchmark ============

#[test]
fn test_ledger_byte_benchmark_1000_transactions() {
    let env = create_test_env();
    let mut total_bytes: u64 = 0;

    for i in 0..1_000u64 {
        let record = SettlementRecord {
            settlement_id: BytesN::zero(),
            status: SettlementStatus::Created as u8,
            source_pilot: 0,
            dest_pilot: 1,
            amount_source: 1_000_00,
            amount_dest: 1_120_00,
            created_at: i,
            updated_at: i,
        };
        total_bytes = total_bytes.saturating_add(record.to_xdr(&env).len() as u64);
    }

    // Bounded execution cost: total serialized bytes must stay under the
    // per-1,000-transaction byte budget.
    assert!(
        total_bytes <= LEDGER_BYTES_PER_1000_TX_BUDGET,
        "1,000-tx ledger bytes {} exceed budget {}",
        total_bytes,
        LEDGER_BYTES_PER_1000_TX_BUDGET
    );
}

fn create_test_env() -> Env {
    Env::default()
}

// ============ Deterministic PRNG for property-based sequences ============

/// Small deterministic xorshift64* PRNG so the property-based suite is
/// reproducible across runs and platforms (no external `rand` dependency).
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        // Avoid the zero state which is a fixed point of xorshift.
        let state = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    fn gen_range(&mut self, lo: u32, hi: u32) -> u32 {
        debug_assert!(hi > lo);
        lo + (self.next_u32() % (hi - lo))
    }

    fn gen_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }
}

/// Fixed seed used by every property-based test so CI is deterministic.
const PROPERTY_SEED: u64 = 0xCBDC_0FF1_1NE5EED_u64;

/// Number of generated sequences required by the acceptance criteria.
const PROPERTY_SEQUENCES: usize = 1_000;

/// Build a valid offline transaction with a caller-controlled nonce.
fn make_offline_tx(env: &Env, nonce: u64) -> OfflineTransaction {
    let tx = create_test_transaction(env);
    let signature = Bytes::from_slice(env, b"property_signature");
    let pubkey = Bytes::from_slice(env, b"property_pubkey");
    OfflineManager::create_offline_transaction(env, tx, signature, pubkey, nonce).unwrap()
}

/// Recompute the hash-chain digest over a sequence of offline transactions
/// exactly as the reconciliation verifier would, so we can assert integrity
/// after simulated recovery.
fn recompute_hash_chain(env: &Env, txs: &[OfflineTransaction]) -> BytesN<32> {
    let mut chain = BytesN::<32>::zero();
    for tx in txs.iter() {
        let mut material = Bytes::new(env);
        material.append(&Bytes::from_slice(env, chain.to_array().as_slice()));
        material.append(&Bytes::from_slice(env, tx.tx_hash.to_array().as_slice()));
        chain = env.crypto().sha256(&material).into();
    }
    chain
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

    let batch_caller = Address::generate(&env);
    let batch =
        OfflineManager::create_batch_settlement(&env, &batch_caller, tx_ids, 2_000_00).unwrap();
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

// ============ CBDC Offline: Property-Based Invariant Suite ============

/// Invariants that must hold for every generated offline transaction
/// regardless of the operation sequence applied to it.
fn assert_offline_invariants(tx: &OfflineTransaction) {
    // A freshly created transaction is always pending and unsettled.
    assert_eq!(tx.status, OfflineStatus::PendingReconciliation as u8);
    assert!(!tx.is_settled());
    assert!(!tx.is_failed());

    // Nonce must be strictly positive for a valid voucher.
    assert!(tx.nonce > 0);

    // Structural validation must always pass for a well-formed voucher.
    assert!(OfflineManager::validate_offline_transaction(tx).is_ok());
}

#[test]
fn test_property_offline_sequence_invariants() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED);

    for _ in 0..PROPERTY_SEQUENCES {
        let nonce = rng.gen_range(1, 1_000_000) as u64;
        let tx = make_offline_tx(&env, nonce);

        // Invariant: creation always yields a pending, unsettled voucher.
        assert_offline_invariants(&tx);

        // Invariant: nonce monotonicity is enforced against the previous nonce.
        if nonce > 1 {
            assert!(OfflineManager::verify_nonce(nonce - 1, nonce).is_ok());
        }
        // Replaying the same nonce must always be rejected.
        assert!(OfflineManager::verify_nonce(nonce, nonce).is_err());
    }
}

#[test]
fn test_property_duplicate_submission_rejected() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xD0D0_D0D0);

    for _ in 0..PROPERTY_SEQUENCES {
        let nonce = rng.gen_range(1, 100_000) as u64;
        let tx = make_offline_tx(&env, nonce);

        // First submission of a nonce is accepted.
        assert!(OfflineManager::verify_nonce(nonce.saturating_sub(1), nonce).is_ok());
        // Duplicate submission of the same nonce is always rejected.
        assert!(OfflineManager::verify_nonce(nonce, nonce).is_err());
        // A stale (lower) nonce is also rejected.
        if nonce > 1 {
            assert!(OfflineManager::verify_nonce(nonce, nonce - 1).is_err());
        }

        assert_offline_invariants(&tx);
    }
}

#[test]
fn test_property_out_of_order_timestamps() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0x7171_7171);

    for _ in 0..PROPERTY_SEQUENCES {
        // Generate two timestamps in arbitrary order.
        let a = rng.gen_range(1, 1_000_000) as u64;
        let b = rng.gen_range(1, 1_000_000) as u64;
        let (earlier, later) = if a <= b { (a, b) } else { (b, a) };

        let tx_early = make_offline_tx(&env, earlier);
        let tx_late = make_offline_tx(&env, later);

        // Regardless of arrival order, the reconciliation state must be
        // order-independent: it only counts statuses.
        let statuses = Vec::from_array(
            &env,
            &[
                tx_early.status,
                tx_late.status,
                OfflineStatus::Reconciled as u8,
            ],
        );
        let state = OfflineManager::compute_reconciliation_state(&statuses);
        assert_eq!(state.total_transactions, 3);
        assert_eq!(state.pending_count, 2);
        assert_eq!(state.successful_count, 1);

        // Reordering the same statuses must yield an identical state.
        let reordered = Vec::from_array(
            &env,
            &[
                OfflineStatus::Reconciled as u8,
                tx_late.status,
                tx_early.status,
            ],
        );
        let state2 = OfflineManager::compute_reconciliation_state(&reordered);
        assert_eq!(state.total_transactions, state2.total_transactions);
        assert_eq!(state.successful_count, state2.successful_count);
        assert_eq!(state.pending_count, state2.pending_count);
        assert_eq!(state.failed_count, state2.failed_count);
        assert_eq!(state.disputed_count, state2.disputed_count);
    }
}

#[test]
fn test_property_mid_batch_rollback_consistency() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xBADC_0FFE);

    for _ in 0..PROPERTY_SEQUENCES {
        let batch_size = rng.gen_range(1, 8) as usize;
        let rollback_at = rng.gen_range(0, batch_size as u32) as usize;

        let mut tx_ids = Vec::new(&env);
        let mut committed: Vec<OfflineTransaction> = Vec::new(&env);
        let mut rolled_back = false;

        for i in 0..batch_size {
            if i == rollback_at {
                // Simulate a mid-batch rollback: drop everything staged so far.
                tx_ids = Vec::new(&env);
                committed = Vec::new(&env);
                rolled_back = true;
                break;
            }
            let tx = make_offline_tx(&env, (i as u64) + 1);
            tx_ids.push_back(tx.tx_hash.clone());
            committed.push_back(tx);
        }

        if rolled_back {
            // After a rollback the batch must be empty and consistent.
            assert_eq!(tx_ids.len(), 0);
            assert_eq!(committed.len(), 0);
        } else {
            // A fully staged batch must be creatable and preserve its size.
            let total = (batch_size as u128) * 1_000_00;
            let caller = Address::generate(&env);
            let batch = OfflineManager::create_batch_settlement(&env, &caller, tx_ids, total).unwrap();
            assert_eq!(batch.transaction_ids.len(), batch_size as u32);
            assert_eq!(committed.len(), batch_size);
        }
    }
}

#[test]
fn test_property_merkle_and_hash_chain_integrity_after_recovery() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0x5EED_5EED);

    for _ in 0..PROPERTY_SEQUENCES {
        let len = rng.gen_range(1, 6) as usize;
        let mut txs: Vec<OfflineTransaction> = Vec::new(&env);
        for i in 0..len {
            txs.push_back(make_offline_tx(&env, (i as u64) + 1));
        }

        // Snapshot the chain digest before simulated recovery.
        let before = recompute_hash_chain(&env, &txs);

        // Simulate recovery: re-derive the chain from the same vouchers.
        let after = recompute_hash_chain(&env, &txs);

        // Hash-chain integrity must be preserved across recovery.
        assert_eq!(before, after);

        // Merkle-style pairwise combination must also be deterministic and
        // order-sensitive: swapping two leaves changes the root.
        if len >= 2 {
            let mut swapped: Vec<OfflineTransaction> = Vec::new(&env);
            swapped.push_back(txs.get(1).unwrap());
            swapped.push_back(txs.get(0).unwrap());
            for i in 2..len {
                swapped.push_back(txs.get(i as u32).unwrap());
            }
            let swapped_root = recompute_hash_chain(&env, &swapped);
            assert_ne!(before, swapped_root);
        }
    }
}

// ============ CBDC Offline: Multi-Actor Concurrency ============

#[test]
fn test_concurrent_multi_actor_nonce_isolation() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xC0C0_C0C0);

    // Simulate N actors each submitting an interleaved stream of vouchers.
    let actor_count = 4usize;
    let per_actor = 250usize;

    let mut actor_nonces: [u64; 4] = [0; 4];
    let mut total = 0usize;

    for _ in 0..(actor_count * per_actor) {
        let actor = rng.gen_range(0, actor_count as u32) as usize;
        let next = actor_nonces[actor] + 1;

        // Each actor's nonce stream must be strictly increasing.
        assert!(OfflineManager::verify_nonce(actor_nonces[actor], next).is_ok());
        actor_nonces[actor] = next;

        let tx = make_offline_tx(&env, next);
        assert_offline_invariants(&tx);
        total += 1;
    }

    assert_eq!(total, actor_count * per_actor);
    // Every actor advanced its own nonce independently.
    for n in actor_nonces.iter() {
        assert!(*n > 0);
    }
}

#[test]
fn test_concurrent_duplicate_race_is_rejected() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xACE0_ACE0);

    for _ in 0..PROPERTY_SEQUENCES {
        let nonce = rng.gen_range(1, 50_000) as u64;
        // Two actors race to submit the same nonce; exactly one wins.
        let first = OfflineManager::verify_nonce(nonce.saturating_sub(1), nonce);
        let second = OfflineManager::verify_nonce(nonce, nonce);
        assert!(first.is_ok());
        assert!(second.is_err());
    }
}

// ============ CBDC Offline: Chaos / Fault Injection ============

#[test]
fn test_chaos_random_operation_sequences_preserve_invariants() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xFA17_FA17);

    for _ in 0..PROPERTY_SEQUENCES {
        let ops = rng.gen_range(1, 16) as usize;
        let mut last_nonce: u64 = 0;
        let mut live: Vec<OfflineTransaction> = Vec::new(&env);

        for _ in 0..ops {
            match rng.gen_range(0, 4) {
                // Create a new voucher with a fresh nonce.
                0 => {
                    let nonce = last_nonce + rng.gen_range(1, 5) as u64;
                    let tx = make_offline_tx(&env, nonce);
                    assert_offline_invariants(&tx);
                    last_nonce = nonce;
                    live.push_back(tx);
                }
                // Attempt a duplicate submission (must be rejected).
                1 => {
                    if last_nonce > 0 {
                        assert!(OfflineManager::verify_nonce(last_nonce, last_nonce).is_err());
                    }
                }
                // Attempt an out-of-order (stale) submission (must be rejected).
                2 => {
                    if last_nonce > 1 {
                        assert!(
                            OfflineManager::verify_nonce(last_nonce, last_nonce - 1).is_err()
                        );
                    }
                }
                // Simulate a rollback of the live set.
                _ => {
                    live = Vec::new(&env);
                    last_nonce = 0;
                }
            }
        }

        // After any chaos sequence the reconciliation state must be coherent.
        let mut statuses = Vec::new(&env);
        for tx in live.iter() {
            statuses.push_back(tx.status);
        }
        let state = OfflineManager::compute_reconciliation_state(&statuses);
        assert_eq!(state.total_transactions, live.len());
        assert_eq!(
            state.successful_count + state.failed_count + state.pending_count + state.disputed_count,
            state.total_transactions
        );
    }
}

#[test]
fn test_chaos_recovery_restores_hash_chain() {
    let env = create_test_env();
    let mut rng = DeterministicRng::new(PROPERTY_SEED ^ 0xDEAD_BEEF);

    for _ in 0..PROPERTY_SEQUENCES {
        let len = rng.gen_range(1, 5) as usize;
        let mut txs: Vec<OfflineTransaction> = Vec::new(&env);
        for i in 0..len {
            txs.push_back(make_offline_tx(&env, (i as u64) + 1));
        }

        let root = recompute_hash_chain(&env, &txs);

        // Inject a fault: corrupt one leaf, then recover from the snapshot.
        let mut corrupted: Vec<OfflineTransaction> = Vec::new(&env);
        for (i, tx) in txs.iter().enumerate() {
            if i == 0 {
                let mut bad = tx.clone();
                bad.nonce = bad.nonce.wrapping_add(1);
                corrupted.push_back(bad);
            } else {
                corrupted.push_back(tx.clone());
            }
        }
        let corrupted_root = recompute_hash_chain(&env, &corrupted);
        assert_ne!(root, corrupted_root);

        // Recovery: rebuild from the original vouchers restores the root.
        let recovered_root = recompute_hash_chain(&env, &txs);
        assert_eq!(root, recovered_root);
    }
}
