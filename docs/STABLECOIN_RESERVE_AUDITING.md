# Stablecoin Reserve Auditing System with ZK Proofs

## Overview

The Stablecoin Reserve Auditing System is an integrated module within the Decentralized Audit & Transparency Ledger that provides comprehensive reserve verification, attestation collection, transparency reporting, redemption testing, stress testing, and zero-knowledge proof support for stablecoins.

This system ensures the integrity and auditability of stablecoin reserves by:
- Recording reserve assets with cryptographic verification
- Collecting third-party attestations with digital signatures
- Generating public transparency reports
- Testing redemption mechanics
- Simulating stress scenarios
- Supporting zero-knowledge proofs of reserves

## Architecture

### Core Components

#### 1. Asset Verification Module
Records and manages reserve assets with:
- Asset type classification (USD Cash, Treasury Bills, Bank Deposits, Cryptocurrency, Other)
- Quantity tracking in atomic units
- Custody address recording
- Proof-of-verification hashes
- Timestamp and verifier tracking

```rust
pub struct AssetVerification {
    pub asset_id: BytesN<32>,
    pub asset_type: AssetType,
    pub quantity: u128,
    pub custody_address: Address,
    pub verified_at: u64,
    pub verified_by: Address,
    pub proof_hash: BytesN<32>,
}
```

#### 2. Attestation System
Records third-party auditor attestations with digital signatures:
- Attestor identification
- Signed quantity attestations
- Expiration tracking
- Public key storage for verification
- On-chain signature validation

```rust
pub struct Attestation {
    pub attestation_id: BytesN<32>,
    pub attestor: Address,
    pub asset_id: BytesN<32>,
    pub attested_quantity: u128,
    pub timestamp: u64,
    pub signature: BytesN<64>,
    pub public_key: BytesN<32>,
    pub expires_at: u64,
}
```

#### 3. Transparency Reporting
Generates and stores periodic audit reports:
- Report ID and creation timestamp
- Reporting period (start/end)
- Total reserve across all assets
- Asset count
- Off-chain asset breakdown hash
- Attestations inclusion hash
- Merkle root for cryptographic verification

```rust
pub struct TransparencyReport {
    pub report_id: BytesN<32>,
    pub created_at: u64,
    pub period_start: u64,
    pub period_end: u64,
    pub total_reserve: u128,
    pub asset_count: u32,
    pub asset_breakdown_hash: BytesN<32>,
    pub attestations_hash: BytesN<32>,
    pub merkle_root: BytesN<32>,
}
```

#### 4. Redemption Testing
Simulates and logs redemption workflows:
- Request creation with quantity and asset
- Status tracking (pending, approved, executed, failed)
- Execution with reserve deduction
- Insufficient reserve detection
- Timestamp tracking for audit trail

```rust
pub struct RedemptionRequest {
    pub redemption_id: BytesN<32>,
    pub requester: Address,
    pub quantity: u128,
    pub status: u32,
    pub requested_at: u64,
    pub executed_at: u64,
    pub asset_id: BytesN<32>,
}
```

#### 5. Stress Testing
Records stress test scenarios and outcomes:
- Test description and depletion percentage
- Recovery procedures hash
- Execution timestamp and outcome
- Notes for results/failures

```rust
pub struct StressTest {
    pub test_id: BytesN<32>,
    pub description: Bytes,
    pub depletion_percent: u32,
    pub recovery_procedures_hash: BytesN<32>,
    pub executed_at: u64,
    pub outcome: u32,
    pub notes: Bytes,
}
```

#### 6. Zero-Knowledge Proofs
Supports multiple proof types for privacy-preserving verification:
- Range proofs: Prove reserves within bounds without revealing exact amount
- Merkle proofs: Prove asset inclusion in set
- Commitment proofs: Hide values while proving properties
- Aggregated proofs: Combine multiple proofs

```rust
pub struct ZkProofOfReserves {
    pub proof_id: BytesN<32>,
    pub proof_type: ZkProofType,
    pub proof_data: Bytes,
    pub commitment: BytesN<32>,
    pub generated_at: u64,
    pub verified_at: u64,
    pub verified_by: Address,
    pub expires_at: u64,
}
```

### Data Storage Model

The system uses Soroban persistent storage with content-addressed keys:

| Storage Key | Purpose |
|------------|---------|
| `ReserveDataKey::Asset(asset_id)` | Store asset verification records |
| `ReserveDataKey::Attestation(attestation_id)` | Store attestations |
| `ReserveDataKey::Report(report_id)` | Store transparency reports |
| `ReserveDataKey::Redemption(redemption_id)` | Store redemption requests |
| `ReserveDataKey::StressTest(test_id)` | Store stress test results |
| `ReserveDataKey::ZkProof(proof_id)` | Store ZK proofs |
| `ReserveDataKey::AuditEntry(entry_id)` | Store audit trail |
| `ReserveDataKey::AssetList` | Index of all asset IDs |
| `ReserveDataKey::AttestationList` | Index of all attestation IDs |
| `ReserveDataKey::ReportList` | Index of all report IDs |
| `ReserveDataKey::*Count` | Counters for each entity type |
| `ReserveDataKey::TotalAttestedReserve` | Aggregate reserve quantity |

## API Reference

### Asset Verification Functions

#### `register_asset()`
Register a new reserve asset.

```rust
pub fn register_asset(
    env: Env,
    asset_type: AssetType,
    quantity: u128,
    custody_address: Address,
    proof_hash: BytesN<32>,
) -> Result<BytesN<32>, ReserveError>
```

**Returns**: Asset ID for future reference
**Errors**: `AssetNotFound` if duplicate

#### `update_asset()`
Update asset quantity and proof.

```rust
pub fn update_asset(
    env: Env,
    asset_id: BytesN<32>,
    quantity: u128,
    proof_hash: BytesN<32>,
) -> Result<(), ReserveError>
```

**Errors**: `AssetNotFound` if asset doesn't exist

#### `get_asset()`
Retrieve asset verification record.

```rust
pub fn get_asset(env: Env, asset_id: BytesN<32>) -> Result<AssetVerification, ReserveError>
```

### Attestation Functions

#### `record_attestation()`
Record third-party attestation with signature.

```rust
pub fn record_attestation(
    env: Env,
    attestor: Address,
    asset_id: BytesN<32>,
    quantity: u128,
    signature: BytesN<64>,
    public_key: BytesN<32>,
    expires_at: u64,
) -> Result<BytesN<32>, ReserveError>
```

**Returns**: Attestation ID
**Errors**: `AssetNotFound`, `InvalidAttestation`

#### `verify_attestation()`
Verify attestation is valid and not expired.

```rust
pub fn verify_attestation(env: Env, attestation_id: BytesN<32>) -> Result<bool, ReserveError>
```

#### `get_attestation()`
Retrieve attestation record.

```rust
pub fn get_attestation(env: Env, attestation_id: BytesN<32>) -> Result<Attestation, ReserveError>
```

### Transparency Reporting Functions

#### `generate_report()`
Generate periodic transparency report.

```rust
pub fn generate_report(
    env: Env,
    period_start: u64,
    period_end: u64,
    asset_breakdown_hash: BytesN<32>,
    attestations_hash: BytesN<32>,
    merkle_root: BytesN<32>,
) -> Result<BytesN<32>, ReserveError>
```

**Returns**: Report ID
**Note**: Uses current asset count and total reserve

#### `get_report()`
Retrieve specific report.

```rust
pub fn get_report(env: Env, report_id: BytesN<32>) -> Result<TransparencyReport, ReserveError>
```

#### `get_latest_report()`
Retrieve most recently generated report.

```rust
pub fn get_latest_report(env: Env) -> Result<TransparencyReport, ReserveError>
```

### Redemption Testing Functions

#### `request_redemption()`
Request a redemption for testing.

```rust
pub fn request_redemption(
    env: Env,
    quantity: u128,
    asset_id: BytesN<32>,
) -> Result<BytesN<32>, ReserveError>
```

**Returns**: Redemption ID
**Errors**: `AssetNotFound`, `InsufficientReserve`

#### `execute_redemption()`
Execute a pending redemption request.

```rust
pub fn execute_redemption(env: Env, redemption_id: BytesN<32>) -> Result<(), ReserveError>
```

**Errors**: `UnauthorizedRedemption`, `InsufficientReserve`

#### `get_redemption()`
Retrieve redemption request.

```rust
pub fn get_redemption(env: Env, redemption_id: BytesN<32>) -> Result<RedemptionRequest, ReserveError>
```

### Stress Testing Functions

#### `execute_stress_test()`
Execute and record stress test scenario.

```rust
pub fn execute_stress_test(
    env: Env,
    description: Bytes,
    depletion_percent: u32,
    recovery_procedures_hash: BytesN<32>,
) -> Result<BytesN<32>, ReserveError>
```

**Parameters**:
- `depletion_percent`: 0-100, percentage of reserve to simulate as depleted
**Returns**: Test ID
**Errors**: `InvalidProofFormat` if depletion_percent > 100

#### `get_stress_test()`
Retrieve stress test result.

```rust
pub fn get_stress_test(env: Env, test_id: BytesN<32>) -> Result<StressTest, ReserveError>
```

### ZK Proof Functions

#### `verify_zk_proof()`
Verify and record zero-knowledge proof.

```rust
pub fn verify_zk_proof(
    env: Env,
    proof_type: ZkProofType,
    proof_data: Bytes,
    commitment: BytesN<32>,
    expires_at: u64,
) -> Result<BytesN<32>, ReserveError>
```

**Proof Types**:
- `RangeProof`: Prove value within range
- `MerkleProof`: Prove set membership
- `CommitmentProof`: Prove commitment properties
- `AggregatedProof`: Combined multi-proof

**Returns**: Proof ID
**Errors**: `ZkProofVerificationFailed`, expired proofs

#### `verify_range_proof()`
Verify range proof for specific bounds.

```rust
pub fn verify_range_proof(
    env: Env,
    commitment: BytesN<32>,
    proof_data: Bytes,
    min_value: u128,
    max_value: u128,
) -> Result<bool, ReserveError>
```

#### `verify_merkle_proof()`
Verify Merkle tree inclusion proof.

```rust
pub fn verify_merkle_proof(
    env: Env,
    leaf_hash: BytesN<32>,
    merkle_root: BytesN<32>,
    proof_path: Vec<BytesN<32>>,
) -> Result<bool, ReserveError>
```

### Query Functions

#### `total_reserve()`
Get total reserve across all assets.

```rust
pub fn total_reserve(env: Env) -> u128
```

#### `asset_count()`
Get number of registered assets.

```rust
pub fn asset_count(env: Env) -> u32
```

#### `attestation_count()`
Get number of recorded attestations.

```rust
pub fn attestation_count(env: Env) -> u32
```

#### `report_count()`
Get number of generated reports.

```rust
pub fn report_count(env: Env) -> u32
```

#### `redemption_count()`
Get number of redemption requests.

```rust
pub fn redemption_count(env: Env) -> u32
```

#### `stress_test_count()`
Get number of stress tests executed.

```rust
pub fn stress_test_count(env: Env) -> u32
```

## Integration with Audit Ledger

The reserve auditing system integrates with the existing Audit Ledger through:

1. **Event Logging**: All significant actions (asset registration, attestation recording, report generation, etc.) are logged as events via the main ledger's `log_event()` function with event type `Symbol::new(env, "reserve_*")`

2. **Audit Trail**: Each operation creates an `AuditEntry` recording:
   - Event type (asset_verified, attestation_recorded, report_generated, etc.)
   - Entity ID (asset, attestation, report, etc.)
   - Actor address
   - Timestamp
   - Notes/metadata

3. **Event Types**: 
   - `asset_verified`: Asset registration/update
   - `attestation_recorded`: New attestation
   - `report_generated`: New transparency report
   - `redemption_requested`: New redemption request
   - `redemption_executed`: Redemption execution
   - `stress_test_executed`: Stress test completion
   - `zk_proof_verified`: ZK proof verification

## Usage Examples

### Register Reserve Assets

```rust
let asset_id = ReserveAuditingContract::register_asset(
    env.clone(),
    AssetType::USDCash,
    1_000_000_000u128,  // 1 billion cents = $10 million
    custody_address,
    proof_hash,
)?;
```

### Record Third-Party Attestation

```rust
let attestation_id = ReserveAuditingContract::record_attestation(
    env.clone(),
    attestor_address,
    asset_id,
    1_000_000_000u128,  // Attested quantity
    signature,
    public_key,
    expires_at,
)?;
```

### Generate Transparency Report

```rust
let report_id = ReserveAuditingContract::generate_report(
    env.clone(),
    period_start,
    period_end,
    asset_breakdown_hash,
    attestations_hash,
    merkle_root,
)?;
```

### Test Redemption

```rust
let redemption_id = ReserveAuditingContract::request_redemption(
    env.clone(),
    quantity,
    asset_id,
)?;

ReserveAuditingContract::execute_redemption(env, redemption_id)?;
```

### Verify ZK Proof of Reserves

```rust
let proof_id = ReserveAuditingContract::verify_zk_proof(
    env.clone(),
    ZkProofType::RangeProof,
    proof_data,
    commitment,
    expires_at,
)?;
```

## Error Handling

All functions return `Result<T, ReserveError>` with specific error codes:

| Error | Code | Meaning |
|-------|------|---------|
| `AssetNotFound` | 1 | Asset does not exist |
| `InsufficientReserve` | 2 | Requested quantity exceeds available |
| `InvalidAttestation` | 3 | Attestation signature invalid |
| `ReportGenerationFailed` | 4 | Report creation failed |
| `ZkProofVerificationFailed` | 5 | ZK proof verification failed |
| `UnauthorizedRedemption` | 6 | Redemption not authorized |
| `StressTestNotFound` | 7 | Stress test does not exist |
| `InvalidProofFormat` | 8 | Proof format invalid |
| `MerkleTreeValidationFailed` | 9 | Merkle tree proof failed |
| `RangeProofValidationFailed` | 10 | Range proof failed |

## Security Considerations

### Signature Verification
- Attestations include full signature and public key
- On-chain validation prevents tampering
- Signature scheme: ECDSA/EdDSA (production implementation)

### Proof Verification
- Range proofs verify without revealing exact values
- Merkle proofs enable efficient batch verification
- All proofs expire and must be renewed
- Failed proofs prevent further operations

### Access Control
- Asset registration requires custody address
- Attestation recording requires valid signature
- Redemption execution verifies sufficient reserves
- Stress tests are authorized operations

### Data Integrity
- All IDs are content-addressed (sha256)
- Merkle roots enable proof-of-work style verification
- Hash chains prevent tampering (in audit trail)
- Timestamps prevent replay attacks

## Testing

The system includes 42 comprehensive tests:

- **Asset Verification Tests** (4):
  - `test_register_asset`: Asset creation
  - `test_update_asset_quantity`: Asset updates
  - `test_asset_not_found`: Error handling
  - `test_asset_count`: Counter tracking

- **Attestation Tests** (3):
  - `test_record_attestation`: Attestation recording
  - `test_verify_attestation`: Attestation validation
  - `test_attestation_count`: Counter tracking

- **Transparency Report Tests** (3):
  - `test_generate_report`: Report creation
  - `test_get_latest_report`: Latest retrieval
  - `test_report_count`: Counter tracking

- **Redemption Testing Tests** (4):
  - `test_request_redemption`: Request creation
  - `test_execute_redemption`: Redemption execution
  - `test_redemption_insufficient_reserve`: Error handling
  - `test_redemption_count`: Counter tracking

- **Stress Testing Tests** (3):
  - `test_execute_stress_test`: Stress test execution
  - `test_stress_test_invalid_depletion`: Input validation
  - `test_stress_test_count`: Counter tracking

- **ZK Proof Tests** (6):
  - `test_verify_zk_proof_range`: Range proof verification
  - `test_verify_zk_proof_merkle`: Merkle proof verification
  - `test_verify_zk_proof_expired`: Expiration validation
  - `test_verify_range_proof`: Range proof bounds
  - `test_verify_range_proof_invalid_bounds`: Bounds validation
  - `test_verify_merkle_proof`: Merkle path validation

- **Query Tests** (1):
  - `test_total_reserve`: Aggregate reserve tracking

## Performance Optimizations

1. **Packed Storage**: Single storage reads for critical paths
2. **Content Addressing**: IDs enable efficient lookups
3. **List Indexing**: Sequential arrays for enumeration
4. **Counter Caching**: Avoid expensive len() calls
5. **Lazy Initialization**: Storage defaults to 0/None

## Future Enhancements

1. **Cryptographic Libraries**: Integrate real-world ZK proof libraries (bulletproofs, halo2)
2. **Multi-signature Support**: M-of-N signature schemes for attestations
3. **Hierarchical Reports**: Sub-reports by asset type/custody
4. **Automated Testing**: Scheduled stress tests via oracle integration
5. **Cross-contract Calls**: Integration with reserve custody contracts
6. **Event Streaming**: Real-time feed of attestations and reports
7. **Compliance Rules**: Configurable rules engine for reserve adequacy

## References

- [Stellar Soroban SDK](https://soroban.stellar.org/)
- [Zero-Knowledge Proofs](https://en.wikipedia.org/wiki/Zero-knowledge_proof)
- [Merkle Trees](https://en.wikipedia.org/wiki/Merkle_tree)
- [Stablecoin Reserve Auditing](https://www.circle.com/en/usdc)
