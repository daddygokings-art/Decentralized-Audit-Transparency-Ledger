# CBDC Integration Guide

## Overview

This document describes the Central Bank Digital Currency (CBDC) integration framework for the Decentralized Audit & Transparency Ledger. The system supports four CBDC pilots:

- **Digital Euro (€)** — European Central Bank
- **Digital Dollar ($)** — U.S. Federal Reserve (experimental)
- **e-CNY (¥)** — People's Bank of China
- **Sand Dollar (BSD)** — Central Bank of The Bahamas

## Architecture

The CBDC integration consists of five core modules:

### 1. CBDC Types (`cbdc_types.rs`)

Defines the fundamental types and enumerations for CBDC operations.

#### Key Types

- **CBDCPilot** — Enumeration of supported CBDC pilots (EUR, USD, CNY, BSD)
- **CBDCTransaction** — Represents a cross-CBDC transaction with conversion details
- **InteropProtocol** — Interoperability standards (AtomicSwap, HubAndSpoke, ISO20022, CBPR)
- **PrivacyTier** — Privacy levels (Public, Pseudonymous, Private, RegulatoryConfidential)
- **OfflineStatus** — States for offline transactions (Pending, Reconciled, Failed, Disputed)
- **BatchSettlement** — Groups multiple offline transactions for batch processing
- **CBDCConfig** — Configuration for system limits and settings

#### Usage Example

```rust
use cbdc_types::*;

// Create a transaction between Digital Euro and Digital Dollar
let tx = CBDCTransaction {
    tx_id: BytesN::zero(),
    source_pilot: CBDCPilot::DigitalEuro as u8,
    dest_pilot: CBDCPilot::DigitalDollar as u8,
    from: sender_address,
    to: recipient_address,
    amount_source: 1_000_00, // 1000 EUR (2 decimal places)
    amount_dest: 1_120_00,   // 1120 USD (after conversion)
    exchange_rate: 1_120_000_000_000_000_000, // 1.12 (scaled by 1e18)
    timestamp: env.ledger().timestamp(),
    protocol: InteropProtocol::AtomicSwap as u8,
    privacy_tier: PrivacyTier::Pseudonymous as u8,
    offline_status: None,
    metadata: Bytes::new(&env),
};
```

### 2. CBDC Event Logging (`cbdc_logging.rs`)

Tracks all CBDC transactions in the audit ledger with comprehensive logging capabilities.

#### Key Components

- **CBDCEvent** — Represents a logged transaction with status and timestamp
- **CBDCEventConfig** — Configuration for event retention and limits
- **TransactionEventType** — Classification of events (CrossCBDCTransfer, BatchSettlement, etc.)
- **CBDCEventStats** — Aggregated statistics on transaction success rates and volumes
- **CBDCLogger** — Utilities for logging and validating transactions

#### Features

- Automatic timestamp recording
- Success/failure status tracking
- Transaction validation before logging
- Privacy-aware logging (respects privacy tier settings)
- Pilot-specific event aggregation

#### Usage Example

```rust
use cbdc_logging::*;

// Log a successful transaction
let event = CBDCLogger::log_transaction_success(
    &env,
    event_index,
    transaction,
    TransactionEventType::CrossCBDCTransfer,
);

// Validate transaction before logging
if let Err(e) = CBDCLogger::validate_transaction(&tx) {
    return Err(e.into());
}
```

### 3. Interoperability Layer (`cbdc_interop.rs`)

Implements cross-CBDC operations and settlement mechanisms.

#### Key Components

- **ExchangeRate** — Manages bid/ask prices with staleness detection
- **SettlementInstruction** — Represents settlement requests with status tracking
- **SettlementStatus** — State machine for settlement lifecycle
- **InteropManager** — Orchestrates settlement operations
- **SettlementPath** — Multi-hop conversion paths for complex transfers

#### Supported Protocols

1. **Atomic Swap** — Direct peer-to-peer transfers with cryptographic guarantees
2. **Hub-and-Spoke** — Transfers routed through neutral intermediary
3. **ISO 20022** — Standardized messaging format for financial transactions
4. **CBPR** — Cross-border payment rail with instant settlement

#### Key Operations

```rust
use cbdc_interop::*;

// Validate exchange rate
InteropManager::validate_exchange_rate(&rate, current_time, staleness_threshold)?;

// Convert amount between pilots
let dest_amount = InteropManager::convert_amount(source_amount, exchange_rate)?;

// Execute atomic swap
let swap_id = InteropManager::execute_atomic_swap(
    &env, &from_addr, &to_addr, source_amt, dest_amt, timeout_ledgers)?;

// Calculate settlement fee
let fee = InteropManager::compute_settlement_fee(amount, fee_bps)?;
```

### 4. Offline Capability (`cbdc_offline.rs`)

Enables transaction signing and reconciliation without connectivity.

#### Key Components

- **OfflineTransaction** — Cryptographically signed transaction before reconciliation
- **OfflineManager** — Handles offline transaction lifecycle
- **ReconciliationState** — Tracks batch reconciliation progress
- **ReconciliationQueue** — Queues transactions for batch processing

#### Offline Flow

1. **Create offline transaction** — User signs transaction with private key
2. **Queue for reconciliation** — Add to batch when network available
3. **Reconcile on-chain** — Submit batch to ledger for settlement
4. **Track status** — Monitor reconciliation progress and conflicts

#### Replay Protection

The offline module implements nonce-based replay detection:

```rust
// Nonces must strictly increase per submitter
OfflineManager::verify_nonce(last_nonce, current_nonce)?;
```

#### Usage Example

```rust
use cbdc_offline::*;

// Create offline transaction
let offline_tx = OfflineManager::create_offline_transaction(
    &env,
    transaction,
    signature,
    signer_pubkey,
    nonce,
)?;

// Validate before reconciliation
OfflineManager::validate_offline_transaction(&offline_tx)?;

// Reconcile to on-chain
let mut tx = offline_tx.clone();
OfflineManager::reconcile_transaction(&env, &mut tx)?;
```

### 5. Privacy Enforcement (`cbdc_privacy.rs`)

Implements multi-tier privacy controls for transaction confidentiality.

#### Privacy Tiers

| Tier | Visibility | Use Case |
|------|-----------|----------|
| **Public** | Full transparency | Regulatory settlements, public transfers |
| **Pseudonymous** | Addresses visible, amounts encrypted | Standard transfers |
| **Private** | Full encryption except ID/timestamp | Sensitive corporate transfers |
| **RegulatoryConfidential** | Central bank access only | Suspicious activity reports, investigations |

#### Key Components

- **MaskedTransaction** — Transaction with encrypted sensitive data
- **PrivacyACL** — Access control list for decryption
- **PrivacyManager** — Encryption/decryption coordination
- **AccessLevel** — Granular permission levels
- **PrivacyStats** — Privacy usage statistics

#### Access Levels

- **None** — No access
- **AuditOnly** — Can verify transaction occurred (not full details)
- **Read** — Full transaction details visible
- **RegulatoryFull** — Unencrypted data for central bank regulators

#### Usage Example

```rust
use cbdc_privacy::*;

// Mask transaction with privacy tier
let masked = PrivacyManager::mask_transaction(
    &env,
    &transaction,
    PrivacyTier::Pseudonymous,
    authorized_decrypters,
)?;

// Create access control list
let acl = PrivacyManager::create_privacy_acl(
    &env,
    transaction_hash,
    read_access_list,
    audit_access_list,
    regulatory_access_list,
    expiration_secs,
)?;

// Check access permission
let access_level = PrivacyManager::check_access_permission(&acl, &user_address, current_time)?;
```

## Integration Patterns

### Pattern 1: Simple Cross-CBDC Transfer

```rust
// 1. Create transaction
let tx = CBDCTransaction {
    source_pilot: CBDCPilot::DigitalEuro as u8,
    dest_pilot: CBDCPilot::DigitalDollar as u8,
    amount_source: 1000_00,
    // ... other fields
};

// 2. Validate transaction
CBDCLogger::validate_transaction(&tx)?;

// 3. Get exchange rate
let rate = get_exchange_rate(CBDCPilot::DigitalEuro, CBDCPilot::DigitalDollar)?;

// 4. Validate rate freshness
InteropManager::validate_exchange_rate(&rate, env.ledger().timestamp(), 3600)?;

// 5. Convert amount
let dest_amount = InteropManager::convert_amount(tx.amount_source, rate.rate)?;

// 6. Log transaction
let event = CBDCLogger::log_transaction_success(&env, index, tx, event_type);
```

### Pattern 2: Offline Transaction with Batch Settlement

```rust
// 1. User creates and signs offline
let offline_tx = OfflineManager::create_offline_transaction(
    &env, tx, signature, pubkey, nonce)?;

// 2. Add to reconciliation queue
queue.add_transaction(offline_tx.tx_hash)?;

// 3. When batch ready, create settlement
let batch = OfflineManager::create_batch_settlement(&env, tx_ids, total_amount)?;

// 4. Settle on-chain
OfflineManager::settle_batch(&env, &mut batch)?;

// 5. Log settlement event
let event = CBDCLogger::log_transaction_success(
    &env, index, tx, TransactionEventType::BatchSettlement);
```

### Pattern 3: Privacy-Preserving Transfer

```rust
// 1. Create transaction
let tx = CBDCTransaction { /* ... */ };

// 2. Apply privacy masking
let masked = PrivacyManager::mask_transaction(
    &env, &tx, PrivacyTier::Private, authorized_decrypters)?;

// 3. Create access control
let acl = PrivacyManager::create_privacy_acl(
    &env, tx_hash, read_list, audit_list, reg_list, expiry)?;

// 4. Grant access to regulators
// (Users can later decrypt with their keys)

// 5. Log masked transaction
// (Audit trail shows transaction occurred, not amounts)
```

## Testing

Each module includes comprehensive unit tests. Run tests with:

```bash
# Test specific module
cargo test cbdc_types::tests

# Test all CBDC modules
cargo test cbdc_

# Run with logging
RUST_LOG=debug cargo test cbdc_types --nocapture
```

### Test Coverage

- **cbdc_types**: Pilot conversions, config validation, transaction hashing
- **cbdc_logging**: Event creation, statistics, validation
- **cbdc_interop**: Exchange rates, settlement status transitions, fee calculations
- **cbdc_offline**: Nonce replay detection, batch reconciliation, integrity verification
- **cbdc_privacy**: ACL expiration, access level checks, privacy statistics

## Configuration

### CBDC System Configuration

```rust
let config = CBDCConfig {
    max_tx_amount: 1_000_000_00,        // 1M units max
    min_tx_amount: 1_00,                // 0.01 units min
    exchange_rate_update_interval: 3600, // 1 hour
    max_batch_size: 1000,               // Offline batch limit
    offline_mode_enabled: true,         // Allow offline signing
};
```

### Event Logging Configuration

```rust
let event_config = CBDCEventConfig {
    max_events: 100_000,           // Keep 100k events
    log_all_transactions: true,    // Log every transaction
    retention_seconds: 0,          // Keep forever (or set limit)
    event_count: 0,               // Current count
};
```

## Security Considerations

1. **Exchange Rate Staleness** — Always validate rates are fresh (< 1 hour)
2. **Nonce Replay Detection** — Enforce monotonically increasing nonces
3. **Privacy Tier Enforcement** — Respect privacy tier in all operations
4. **Batch Atomicity** — All-or-nothing semantics for batch settlements
5. **Access Control** — Always check ACL before decryption

## Error Handling

Common errors and recovery:

| Error | Cause | Recovery |
|-------|-------|----------|
| `Exchange rate is stale` | Rate not updated within threshold | Fetch fresh rate from oracle |
| `Nonce replay detected` | Nonce not strictly increasing | Use new nonce value |
| `Transaction amount cannot be zero` | Invalid input | Validate before creating transaction |
| `Access control list has expired` | ACL TTL exceeded | Create new ACL with fresh expiration |
| `Batch is full` | Queue exceeded max size | Flush pending batch and create new |

## Performance Characteristics

- **Transaction creation**: O(1)
- **Exchange rate validation**: O(1)
- **Batch settlement**: O(n) where n = batch size
- **ACL checking**: O(m) where m = number of access entries
- **Privacy masking**: O(1) (hash-based)

## Future Enhancements

1. **Multi-hop routing** — Automatic best-path discovery across pilots
2. **Liquidity pools** — Reduce settlement times via AMM-style pools
3. **Layer 2 scaling** — State channels for high-frequency settlements
4. **Cross-chain attestation** — Proof anchoring to external blockchains
5. **ML-based fraud detection** — Anomaly detection on transaction patterns

## Deployment Checklist

- [ ] Review exchange rate sources and update intervals
- [ ] Configure privacy tier defaults for your use case
- [ ] Set up offline reconciliation queue size
- [ ] Define regulatory access groups (if needed)
- [ ] Enable/disable specific interoperability protocols
- [ ] Set max transaction amounts per pilot pair
- [ ] Configure event retention policies
- [ ] Test batch settlement under load
- [ ] Verify nonce management across offline signers
- [ ] Audit privacy ACL configurations

## Contact & Support

For questions about CBDC integration:
- Review the module documentation inline in each file
- Check the comprehensive test cases for usage examples
- Refer to the unit test output for validation behavior

## License

MIT
