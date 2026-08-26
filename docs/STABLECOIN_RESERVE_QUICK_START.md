# Stablecoin Reserve Auditing Quick Start Guide

## Building

```bash
# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test stablecoin_reserve

# Format code
cargo fmt

# Lint
cargo clippy
```

## Module Structure

The reserve auditing system is organized into three modules:

### 1. `stablecoin_reserves.rs`
Data structures and trait definitions:
- `AssetVerification` - Reserve asset records
- `Attestation` - Third-party attestations
- `TransparencyReport` - Periodic audit reports
- `RedemptionRequest` - Redemption testing
- `StressTest` - Stress test scenarios
- `ZkProofOfReserves` - ZK proof records
- `ReserveAuditingTrait` - API specification

### 2. `stablecoin_reserves_impl.rs`
Contract implementation with all API functions:
- Asset verification functions
- Attestation recording and verification
- Report generation and retrieval
- Redemption request and execution
- Stress test execution
- ZK proof verification

### 3. `stablecoin_reserves_tests.rs`
42 comprehensive unit tests

## Basic Workflow

### Step 1: Register Assets

```rust
let asset_id = ReserveAuditingContract::register_asset(
    env,
    AssetType::USDCash,
    1_000_000_000,  // Quantity in smallest units
    custody_address,
    proof_hash,
)?;
```

### Step 2: Record Attestations

```rust
let attestation_id = ReserveAuditingContract::record_attestation(
    env,
    attestor_address,
    asset_id,
    1_000_000_000,
    signature,
    public_key,
    expires_at,
)?;
```

### Step 3: Generate Report

```rust
let report_id = ReserveAuditingContract::generate_report(
    env,
    period_start,
    period_end,
    asset_breakdown_hash,
    attestations_hash,
    merkle_root,
)?;

let report = ReserveAuditingContract::get_latest_report(env)?;
println!("Total reserve: {}", report.total_reserve);
```

### Step 4: Test Redemptions

```rust
// Request
let redemption_id = ReserveAuditingContract::request_redemption(
    env,
    100_000_000,
    asset_id,
)?;

// Execute
ReserveAuditingContract::execute_redemption(env, redemption_id)?;
```

### Step 5: Execute Stress Tests

```rust
let test_id = ReserveAuditingContract::execute_stress_test(
    env,
    Bytes::from_slice(env, b"50% depletion scenario"),
    50,  // 50% depletion
    recovery_procedures_hash,
)?;
```

### Step 6: Verify ZK Proofs

```rust
// Range proof
let proof_id = ReserveAuditingContract::verify_zk_proof(
    env,
    ZkProofType::RangeProof,
    proof_data,
    commitment,
    expires_at,
)?;

// Merkle proof
let is_valid = ReserveAuditingContract::verify_merkle_proof(
    env,
    leaf_hash,
    merkle_root,
    proof_path,
)?;
```

## Query Functions

```rust
// Get statistics
let total = ReserveAuditingContract::total_reserve(env);
let assets = ReserveAuditingContract::asset_count(env);
let attestations = ReserveAuditingContract::attestation_count(env);
let reports = ReserveAuditingContract::report_count(env);
let redemptions = ReserveAuditingContract::redemption_count(env);
let stress_tests = ReserveAuditingContract::stress_test_count(env);
```

## Error Handling

All operations return `Result<T, ReserveError>`:

```rust
match ReserveAuditingContract::request_redemption(env, quantity, asset_id) {
    Ok(redemption_id) => println!("Redemption created: {}", redemption_id),
    Err(ReserveError::InsufficientReserve) => println!("Not enough reserves"),
    Err(ReserveError::AssetNotFound) => println!("Asset not found"),
    Err(e) => println!("Error: {:?}", e),
}
```

## Integration with Main Ledger

Reserve auditing events are logged via the main audit ledger:

```rust
// Register asset logs event with type "asset_verified"
// Event includes: asset_id, asset_type, quantity, custody_address

// Record attestation logs event with type "attestation_recorded"
// Event includes: attestation_id, attestor, asset_id, quantity

// Generate report logs event with type "report_generated"
// Event includes: report_id, period, total_reserve, asset_count
```

## Testing

Run specific test suites:

```bash
# Asset verification tests
cargo test stablecoin_reserve_tests::test_register_asset
cargo test stablecoin_reserve_tests::test_update_asset
cargo test stablecoin_reserve_tests::test_asset_count

# Attestation tests
cargo test stablecoin_reserve_tests::test_record_attestation
cargo test stablecoin_reserve_tests::test_verify_attestation

# Report tests
cargo test stablecoin_reserve_tests::test_generate_report
cargo test stablecoin_reserve_tests::test_get_latest_report

# Redemption tests
cargo test stablecoin_reserve_tests::test_request_redemption
cargo test stablecoin_reserve_tests::test_execute_redemption

# Stress test tests
cargo test stablecoin_reserve_tests::test_execute_stress_test

# ZK proof tests
cargo test stablecoin_reserve_tests::test_verify_zk_proof
cargo test stablecoin_reserve_tests::test_verify_merkle_proof

# Run all tests
cargo test stablecoin_reserve_tests
```

## Key Design Decisions

### 1. Content-Addressed IDs
All entity IDs are SHA256 hashes, enabling:
- Deterministic deduplication
- Efficient lookups
- Cryptographic verification

### 2. Persistent Storage
Uses Soroban's persistent storage with:
- Indexed lists for enumeration
- Counters for quick statistics
- Lazy initialization for cost efficiency

### 3. Modular Architecture
Three-file structure enables:
- Clear separation of concerns
- Easy testing and maintenance
- Future extensibility

### 4. Simplified Cryptography
Initial implementation includes:
- Placeholder signature verification (to be replaced)
- Placeholder ZK proof verification (to be replaced)
- Proper Merkle tree validation

### 5. Error Handling
Comprehensive error types for:
- Asset validation
- Attestation validity
- Proof verification
- Reserve sufficiency

## Next Steps

1. **Integrate Real Cryptography**
   - Implement ECDSA/EdDSA signature verification
   - Integrate bulletproofs or halo2 for ZK proofs
   - Use proper hash functions

2. **Add Cross-Contract Calls**
   - Enable custody contracts to verify reserves
   - Support oracle-based reserve updates

3. **Implement Governance**
   - Add owner-only configuration functions
   - Support pause/freeze mechanisms
   - Enable emergency procedures

4. **Build UI Dashboard**
   - Display asset composition
   - Show attestation status
   - Visualize reserve trends
   - Monitor redemption activity

5. **Deploy to Testnet**
   - Test with real Stellar addresses
   - Collect feedback from auditors
   - Optimize gas usage

## File Locations

| File | Purpose |
|------|---------|
| `src/stablecoin_reserves.rs` | Data structures and traits |
| `src/stablecoin_reserves_impl.rs` | Contract implementation |
| `src/stablecoin_reserves_tests.rs` | Unit tests (42 tests) |
| `docs/STABLECOIN_RESERVE_AUDITING.md` | Full documentation |
| `docs/STABLECOIN_RESERVE_QUICK_START.md` | This file |

## Support and Contributions

For questions, issues, or contributions:
1. Review the full documentation in `STABLECOIN_RESERVE_AUDITING.md`
2. Check test examples in `stablecoin_reserves_tests.rs`
3. Run tests: `cargo test stablecoin_reserve`
4. Open GitHub issues for bugs or features
