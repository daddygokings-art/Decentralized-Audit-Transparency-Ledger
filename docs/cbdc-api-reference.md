# CBDC API Reference

## Module Exports

### cbdc_types.rs

```rust
pub enum CBDCPilot {
    DigitalEuro,
    DigitalDollar,
    eCNY,
    SandDollar,
}

pub enum InteropProtocol {
    AtomicSwap,
    HubAndSpoke,
    ISO20022,
    CBPR,
}

pub enum PrivacyTier {
    Public,
    Pseudonymous,
    Private,
    RegulatoryConfidential,
}

pub enum OfflineStatus {
    PendingReconciliation,
    Reconciled,
    FailedReconciliation,
    Disputed,
}

pub struct CBDCTransaction {
    pub tx_id: BytesN<32>,
    pub source_pilot: u8,
    pub dest_pilot: u8,
    pub from: Address,
    pub to: Address,
    pub amount_source: u128,
    pub amount_dest: u128,
    pub exchange_rate: u128,
    pub timestamp: u64,
    pub protocol: u8,
    pub privacy_tier: u8,
    pub offline_status: Option<u8>,
    pub metadata: Bytes,
}

pub struct CBDCConfig {
    pub max_tx_amount: u128,
    pub min_tx_amount: u128,
    pub exchange_rate_update_interval: u64,
    pub max_batch_size: u32,
    pub offline_mode_enabled: bool,
}
```

### cbdc_logging.rs

```rust
pub struct CBDCEvent {
    pub event_index: u32,
    pub timestamp: u64,
    pub transaction: CBDCTransaction,
    pub event_type: Symbol,
    pub status: Symbol,
    pub error_message: Option<Bytes>,
}

pub struct CBDCEventConfig {
    pub max_events: u32,
    pub event_count: u32,
    pub log_all_transactions: bool,
    pub retention_seconds: u64,
}

pub enum TransactionEventType {
    CrossCBDCTransfer,
    BatchSettlement,
    ExchangeRateUpdate,
    Reconciliation,
    DisputeReversal,
}

pub struct CBDCEventStats {
    pub total_success: u32,
    pub total_failed: u32,
    pub total_volume: u128,
    pub last_transaction_timestamp: u64,
    pub events_per_pilot: Vec<(u8, u32)>,
}

pub struct CBDCLogger;

impl CBDCLogger {
    pub fn log_transaction_success(
        env: &Env,
        event_index: u32,
        transaction: CBDCTransaction,
        event_type: TransactionEventType,
    ) -> CBDCEvent;

    pub fn log_transaction_failed(
        env: &Env,
        event_index: u32,
        transaction: CBDCTransaction,
        event_type: TransactionEventType,
        error_msg: &str,
    ) -> CBDCEvent;

    pub fn create_metadata(
        env: &Env,
        source: CBDCPilot,
        dest: CBDCPilot,
        protocol: InteropProtocol,
        privacy_tier: PrivacyTier,
    ) -> Bytes;

    pub fn validate_transaction(tx: &CBDCTransaction) -> Result<(), &'static str>;

    pub fn get_source_pilot(tx: &CBDCTransaction) -> Result<CBDCPilot, &'static str>;

    pub fn get_dest_pilot(tx: &CBDCTransaction) -> Result<CBDCPilot, &'static str>;
}
```

### cbdc_interop.rs

```rust
pub struct ExchangeRate {
    pub source_pilot: u8,
    pub dest_pilot: u8,
    pub rate: u128,
    pub timestamp: u64,
    pub bid_price: u128,
    pub ask_price: u128,
    pub max_volatility: u32,
}

pub struct SettlementInstruction {
    pub settlement_id: BytesN<32>,
    pub transaction: CBDCTransaction,
    pub protocol: u8,
    pub status: u8,
    pub timestamp: u64,
    pub confirmation_hash: Option<BytesN<32>>,
}

pub enum SettlementStatus {
    Created,
    InitiatedSource,
    ConfirmedSource,
    AwaitingDestination,
    Completed,
    Failed,
}

pub struct InteropManager;

impl InteropManager {
    pub fn validate_exchange_rate(
        rate: &ExchangeRate,
        current_time: u64,
        staleness_threshold: u64,
    ) -> Result<(), &'static str>;

    pub fn convert_amount(amount: u128, rate: u128) -> Result<u128, &'static str>;

    pub fn execute_atomic_swap(
        env: &Env,
        from: &Address,
        to: &Address,
        source_amount: u128,
        dest_amount: u128,
        timeout_ledgers: u32,
    ) -> Result<BytesN<32>, &'static str>;

    pub fn execute_hub_and_spoke(
        env: &Env,
        hub_address: &Address,
        source_amount: u128,
        dest_amount: u128,
    ) -> Result<BytesN<32>, &'static str>;

    pub fn validate_settlement_instruction(
        instruction: &SettlementInstruction,
    ) -> Result<(), &'static str>;

    pub fn protocol_version(protocol: InteropProtocol) -> &'static str;

    pub fn is_valid_status_transition(
        from: SettlementStatus,
        to: SettlementStatus,
    ) -> bool;

    pub fn compute_settlement_fee(amount: u128, fee_bps: u32) -> Result<u128, &'static str>;

    pub fn validate_pilot_pair(source: u8, dest: u8) -> Result<(), &'static str>;
}

pub struct SettlementPath {
    pub path: Vec<u8>,
    pub rates: Vec<u128>,
    pub total_cost_bps: u32,
}
```

### cbdc_offline.rs

```rust
pub struct OfflineTransaction {
    pub tx_hash: BytesN<32>,
    pub transaction: CBDCTransaction,
    pub signature: Bytes,
    pub signer_pubkey: Bytes,
    pub status: u8,
    pub created_at: u64,
    pub reconciled_at: Option<u64>,
    pub nonce: u32,
}

pub struct ReconciliationState {
    pub total_transactions: u32,
    pub successful_count: u32,
    pub failed_count: u32,
    pub pending_count: u32,
    pub disputed_count: u32,
}

pub struct OfflineManager;

impl OfflineManager {
    pub fn create_offline_transaction(
        env: &Env,
        transaction: CBDCTransaction,
        signature: Bytes,
        signer_pubkey: Bytes,
        nonce: u32,
    ) -> Result<OfflineTransaction, &'static str>;

    pub fn compute_offline_tx_hash(
        env: &Env,
        transaction: &CBDCTransaction,
        nonce: u32,
    ) -> BytesN<32>;

    pub fn validate_offline_transaction(tx: &OfflineTransaction) -> Result<(), &'static str>;

    pub fn reconcile_transaction(
        env: &Env,
        offline_tx: &mut OfflineTransaction,
    ) -> Result<(), &'static str>;

    pub fn fail_reconciliation(
        env: &Env,
        offline_tx: &mut OfflineTransaction,
    ) -> Result<(), &'static str>;

    pub fn create_batch_settlement(
        env: &Env,
        tx_ids: Vec<BytesN<32>>,
        total_amount: u128,
    ) -> Result<BatchSettlement, &'static str>;

    pub fn compute_batch_id(env: &Env, tx_ids: &Vec<BytesN<32>>) -> BytesN<32>;

    pub fn settle_batch(
        env: &Env,
        batch: &mut BatchSettlement,
    ) -> Result<(), &'static str>;

    pub fn verify_nonce(last_nonce: u32, current_nonce: u32) -> Result<(), &'static str>;

    pub fn compute_reconciliation_state(statuses: &Vec<u8>) -> ReconciliationState;

    pub fn verify_transaction_integrity(env: &Env, offline_tx: &OfflineTransaction) -> bool;
}

pub struct ReconciliationQueue {
    pub pending_tx_hashes: Vec<BytesN<32>>,
    pub max_queue_size: u32,
    pub last_flush_time: u64,
}
```

### cbdc_privacy.rs

```rust
pub struct MaskedTransaction {
    pub content_hash: BytesN<32>,
    pub privacy_tier: u8,
    pub encrypted_data: Option<Bytes>,
    pub authorized_decrypters: Vec<Address>,
    pub masked_at: u64,
    pub encryption_metadata: Bytes,
}

pub struct PrivacyACL {
    pub acl_id: BytesN<32>,
    pub transaction_hash: BytesN<32>,
    pub read_access: Vec<Address>,
    pub audit_access: Vec<Address>,
    pub regulatory_access: Vec<Address>,
    pub expires_at: u64,
}

pub struct PrivacyManager;

impl PrivacyManager {
    pub fn mask_transaction(
        env: &Env,
        transaction: &CBDCTransaction,
        privacy_tier: PrivacyTier,
        authorized_decrypters: Vec<Address>,
    ) -> Result<MaskedTransaction, &'static str>;

    pub fn compute_transaction_content_hash(
        env: &Env,
        transaction: &CBDCTransaction,
    ) -> BytesN<32>;

    pub fn create_privacy_acl(
        env: &Env,
        transaction_hash: BytesN<32>,
        read_access: Vec<Address>,
        audit_access: Vec<Address>,
        regulatory_access: Vec<Address>,
        expiration_secs: u64,
    ) -> Result<PrivacyACL, &'static str>;

    pub fn compute_acl_id(env: &Env, transaction_hash: &BytesN<32>) -> BytesN<32>;

    pub fn validate_privacy_configuration(
        tier: PrivacyTier,
        has_authorized_decrypters: bool,
    ) -> Result<(), &'static str>;

    pub fn check_access_permission(
        acl: &PrivacyACL,
        accessor: &Address,
        current_time: u64,
    ) -> Result<AccessLevel, &'static str>;
}

pub enum AccessLevel {
    None,
    AuditOnly,
    Read,
    RegulatoryFull,
}

pub struct PrivacyStats {
    pub public_count: u32,
    pub pseudonymous_count: u32,
    pub private_count: u32,
    pub regulatory_count: u32,
}
```

## Common Operations

### Create and Log a CBDC Transaction

```rust
// 1. Build transaction
let tx = CBDCTransaction {
    tx_id: compute_tx_id(&env),
    source_pilot: CBDCPilot::DigitalEuro as u8,
    dest_pilot: CBDCPilot::DigitalDollar as u8,
    from: sender.clone(),
    to: recipient.clone(),
    amount_source: 1000_00,
    amount_dest: 1120_00,
    exchange_rate: 1_120_000_000_000_000_000,
    timestamp: env.ledger().timestamp(),
    protocol: InteropProtocol::AtomicSwap as u8,
    privacy_tier: PrivacyTier::Pseudonymous as u8,
    offline_status: None,
    metadata: Bytes::new(&env),
};

// 2. Validate
CBDCLogger::validate_transaction(&tx)?;

// 3. Log
let event = CBDCLogger::log_transaction_success(
    &env,
    event_index,
    tx.clone(),
    TransactionEventType::CrossCBDCTransfer,
);
```

### Validate Exchange Rate and Convert Amount

```rust
let rate = get_fresh_rate(source_pilot, dest_pilot)?;

// Check staleness (< 1 hour)
InteropManager::validate_exchange_rate(&rate, current_time, 3600)?;

// Convert
let dest_amount = InteropManager::convert_amount(source_amount, rate.rate)?;
```

### Create and Reconcile Offline Transaction

```rust
// Create
let offline_tx = OfflineManager::create_offline_transaction(
    &env,
    transaction,
    signature,
    pubkey,
    nonce,
)?;

// Validate
OfflineManager::validate_offline_transaction(&offline_tx)?;

// Reconcile
let mut tx = offline_tx.clone();
OfflineManager::reconcile_transaction(&env, &mut tx)?;
```

### Apply Privacy Masking and Access Control

```rust
// Mask transaction
let masked = PrivacyManager::mask_transaction(
    &env,
    &transaction,
    PrivacyTier::Private,
    authorized_decrypters,
)?;

// Create ACL (1 hour expiration)
let acl = PrivacyManager::create_privacy_acl(
    &env,
    tx_hash,
    read_access,
    audit_access,
    regulatory_access,
    3600,
)?;

// Check access
let access_level = PrivacyManager::check_access_permission(
    &acl,
    &user_address,
    current_time,
)?;
```

## Error Codes

| Code | Message | Solution |
|------|---------|----------|
| `Exchange rate is stale` | Rate > 1 hour old | Fetch fresh rate |
| `Nonce replay detected` | Nonce not increasing | Use new nonce |
| `Transaction amount cannot be zero` | Invalid amount | Validate inputs |
| `Access control list has expired` | ACL TTL exceeded | Create new ACL |
| `Batch is full` | Queue exceeded max | Flush and retry |
| `Batch cannot be empty` | Empty transaction list | Add transactions |
| `Source and destination pilots cannot be the same` | Invalid pilot pair | Use different pilots |
| `Signature cannot be empty` | Missing signature | Provide signature |
| `Encrypted transactions must have authorized decrypters` | Privacy config invalid | Add decrypters |

## Constants

- **Exchange Rate Scale**: 1e18 (precision: 18 decimal places)
- **Default Batch Size**: 1000 transactions
- **Default Event Limit**: 100,000 CBDC events
- **Staleness Threshold**: 3600 seconds (1 hour)
- **Max Volatility**: Configurable per rate (default: 500 bps = 5%)

## Testing Functions

All modules include `#[cfg(test)]` test cases covering:
- Type conversions and validations
- Edge cases (zero amounts, overflow, underflow)
- Status transitions
- ACL expiration
- Nonce replay detection
- Privacy tier enforcement

Run tests with:
```bash
cargo test cbdc_ -- --nocapture
```
