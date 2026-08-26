# DeFi Protocol Auditing - Quick Start Guide

## Setup

```bash
# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Run all DeFi auditing tests
cargo test defi_auditing_tests

# Run specific feature tests
cargo test defi_auditing_tests::test_register_protocol
cargo test defi_auditing_tests::test_update_pool_tvl
cargo test defi_auditing_tests::test_record_oracle_price
cargo test defi_auditing_tests::test_record_liquidation
cargo test defi_auditing_tests::test_create_governance_proposal
cargo test defi_auditing_tests::test_calculate_risk_metrics
cargo test defi_auditing_tests::test_generate_audit_report
```

## Basic Workflow

### 1. Register a Protocol

```rust
use crate::defi_auditing::*;
use crate::defi_auditing_impl::DeFiAuditingContract;

let env = Env::default();
let protocol = Address::generate(&env);

DeFiAuditingContract::register_protocol(
    env.clone(),
    protocol.clone(),
    Symbol::new(&env, "aave"),
    ProtocolType::Lending,
    Symbol::new(&env, "ethereum"),
    None,  // governance token address, optional
)?;
```

### 2. Update Pool TVL

```rust
let pool_id = BytesN::<32>::from_array(&[1u8; 32]);

DeFiAuditingContract::update_pool_tvl(
    env.clone(),
    pool_id,
    protocol.clone(),
    Symbol::new(&env, "DAI"),  // pool identifier
    5_000_000_000u128,         // TVL in USD
    1_000_000_000u128,         // TVL in native units
    500u32,                    // LP count
)?;

// Query total protocol TVL
let tvl = DeFiAuditingContract::get_protocol_tvl(env.clone(), protocol.clone())?;
println!("Protocol TVL: ${}", tvl);
```

### 3. Record Oracle Prices

```rust
let asset = Address::generate(&env);
let oracle_id = BytesN::<32>::from_array(&[2u8; 32]);

DeFiAuditingContract::record_oracle_price(
    env.clone(),
    oracle_id,
    asset.clone(),
    2000u128,                      // price in USD
    Symbol::new(&env, "chainlink"),// oracle source
    100u32,                        // confidence (1%)
    3600u64,                       // update frequency (1 hour)
)?;

// Check for price anomalies
let is_anomaly = DeFiAuditingContract::verify_price_anomaly(
    env.clone(),
    asset.clone(),
    2200u128,  // new price
    500u32,    // 5% threshold
)?;

if is_anomaly {
    println!("Price anomaly detected!");
}
```

### 4. Track Liquidations

```rust
let position = Address::generate(&env);
let liquidator = Address::generate(&env);
let collateral = Address::generate(&env);
let debt_asset = Address::generate(&env);

let liquidation_id = DeFiAuditingContract::record_liquidation(
    env.clone(),
    protocol.clone(),
    position,
    liquidator,
    collateral,
    debt_asset,
    50_000_000u128,     // collateral amount
    100_000_000u128,    // debt amount
    2000u128,           // liquidation price
)?;

// Track at-risk positions
let position_owner = Address::generate(&env);
let position_id = DeFiAuditingContract::add_at_risk_position(
    env.clone(),
    protocol.clone(),
    position_owner,
    200_000_000u128,    // collateral value
    100_000_000u128,    // debt value
    15000u128,          // health factor (needs to be > 11000 for safety)
)?;

// Query counts
let liquidations = DeFiAuditingContract::get_protocol_liquidations(
    env.clone(),
    protocol.clone()
)?;
let at_risk = DeFiAuditingContract::get_at_risk_positions(
    env.clone(),
    protocol.clone()
)?;

println!("Liquidations: {}", liquidations);
println!("At-risk positions: {}", at_risk);
```

### 5. Monitor Governance

```rust
let proposer = Address::generate(&env);
let title = Bytes::from_slice(&env, b"Increase Reserve Factor");
let description = Bytes::from_slice(&env, b"Proposal to increase reserve factor to 20%");

let proposal_id = DeFiAuditingContract::create_proposal(
    env.clone(),
    protocol.clone(),
    title,
    description,
    proposer,
    env.ledger().timestamp(),
    env.ledger().timestamp() + 86400u64,  // 1 day duration
)?;

// Record votes
let voter1 = Address::generate(&env);
let voter2 = Address::generate(&env);

DeFiAuditingContract::record_vote(
    env.clone(),
    proposal_id,
    voter1,
    1u32,              // 1 = vote for
    1000_000u128,      // voting power
)?;

DeFiAuditingContract::record_vote(
    env.clone(),
    proposal_id,
    voter2,
    0u32,              // 0 = vote against
    500_000u128,
)?;

// Get vote counts
let (for_votes, against_votes, abstain_votes) = 
    DeFiAuditingContract::get_proposal_votes(env.clone(), proposal_id)?;

println!("For: {} vs Against: {}", for_votes, against_votes);

// Update proposal status
DeFiAuditingContract::update_proposal_status(
    env.clone(),
    proposal_id,
    2u32,  // 2 = passed
)?;

// Query proposal count
let proposals = DeFiAuditingContract::protocol_proposal_count(env.clone(), protocol.clone());
println!("Total proposals: {}", proposals);
```

### 6. Calculate Risk Metrics

```rust
let metrics_id = DeFiAuditingContract::calculate_risk_metrics(
    env.clone(),
    protocol.clone(),
)?;

// Retrieve metrics
let metrics = DeFiAuditingContract::get_risk_metrics(env.clone(), metrics_id)?;

println!("Protocol Health: {}", metrics.protocol_health);
println!("Concentration Risk: {}", metrics.concentration_risk);
println!("Liquidation Risk: {}", metrics.liquidation_risk);
println!("Price Volatility: {}", metrics.price_volatility);

// Get overall health score
let health = DeFiAuditingContract::get_protocol_health_score(
    env.clone(),
    protocol.clone()
)?;

println!("Overall Health Score: {}", health);
```

### 7. Generate Audit Reports

```rust
let now = env.ledger().timestamp();
let period_start = now - 86400u64 * 30;  // 30 days ago
let findings = BytesN::<32>::from_array(&[1u8; 32]);

let report_id = DeFiAuditingContract::generate_audit_report(
    env.clone(),
    protocol.clone(),
    period_start,
    now,
    findings,  // hash of findings document
)?;

// Retrieve report
let report = DeFiAuditingContract::get_audit_report(env.clone(), report_id)?;

println!("Report TVL: {} (avg), {} (peak), {} (min)",
    report.avg_tvl, report.peak_tvl, report.min_tvl);
println!("Liquidations: {}", report.total_liquidations);
println!("Governance Activity: {} proposals", report.proposals_count);
println!("Health Score: {}", report.health_score);

// Get latest report
let latest = DeFiAuditingContract::get_latest_audit_report(
    env.clone(),
    protocol.clone()
)?;

println!("Latest report generated at: {}", latest.generated_at);
```

## Advanced Features

### Multi-Protocol Monitoring

```rust
let protocols = vec![
    ("aave", ProtocolType::Lending),
    ("uniswap", ProtocolType::AMM),
    ("compound", ProtocolType::Lending),
];

for (name, ptype) in protocols {
    let protocol = Address::generate(&env);
    DeFiAuditingContract::register_protocol(
        env.clone(),
        protocol,
        Symbol::new(&env, name),
        ptype,
        Symbol::new(&env, "ethereum"),
        None,
    )?;
}

let count = DeFiAuditingContract::total_protocol_count(env);
println!("Monitoring {} protocols", count);
```

### Automated Monitoring Loop

```rust
// Pseudocode for continuous monitoring
loop {
    // Update TVL
    update_all_pool_tvl(&env)?;
    
    // Fetch and verify oracle prices
    verify_oracle_prices(&env)?;
    
    // Check for at-risk positions
    identify_at_risk_positions(&env)?;
    
    // Calculate metrics
    for protocol in get_protocols(&env) {
        DeFiAuditingContract::calculate_risk_metrics(&env, protocol)?;
    }
    
    // Generate daily reports
    if should_generate_report() {
        generate_daily_reports(&env)?;
    }
    
    sleep(Duration::from_secs(300)); // 5 minute interval
}
```

### Error Handling

```rust
use crate::defi_auditing::DeFiAuditError;

match DeFiAuditingContract::get_protocol(env, protocol) {
    Ok(registry) => println!("Protocol: {}", registry.name),
    Err(DeFiAuditError::ProtocolNotFound) => {
        println!("Protocol not registered");
        // Register it
    }
    Err(e) => println!("Error: {:?}", e),
}

// Handle oracle price errors
match DeFiAuditingContract::get_oracle_price(env, oracle_id) {
    Ok(price) => {
        if price.timestamp < now - 7200u64 {
            println!("Price data stale (> 2 hours old)");
        }
    }
    Err(DeFiAuditError::OraclePriceInvalid) => {
        println!("Invalid or missing oracle price");
    }
    Err(e) => println!("Price fetch error: {:?}", e),
}
```

## Key Concepts

### Health Factor
- Calculated as: `(collateral_value * liquidation_threshold) / debt_value`
- Values > 1.1 (11000 scaled) indicate safe positions
- Values < 1.1 indicate liquidation risk
- Used to identify at-risk positions

### Price Anomaly Detection
- Tracks price history with percentage changes
- Detects when price movement exceeds threshold
- Compares against configurable basis point thresholds
- Common threshold: 500 bp (5%)

### Risk Metrics
- **Concentration Risk**: Percentage of TVL in top 3 assets
- **Liquidation Risk**: Percentage of positions at risk
- **Health Score**: Composite score (0-100) based on risks
- Updated periodically (recommended: hourly)

### Proposal Status Lifecycle
- 0: Pending (awaiting activation)
- 1: Active (voting open)
- 2: Passed (voting closed, passed)
- 3: Failed (voting closed, rejected)
- 4: Executed (proposal implemented)

## Testing

Run the full test suite:

```bash
cargo test defi_auditing_tests
```

Individual test categories:

```bash
# Protocol management
cargo test defi_auditing_tests::test_register_protocol
cargo test defi_auditing_tests::test_protocol_count

# TVL tracking
cargo test defi_auditing_tests::test_update_pool_tvl
cargo test defi_auditing_tests::test_protocol_tvl_aggregation

# Oracle verification
cargo test defi_auditing_tests::test_record_oracle_price
cargo test defi_auditing_tests::test_price_anomaly_detection

# Liquidation monitoring
cargo test defi_auditing_tests::test_record_liquidation
cargo test defi_auditing_tests::test_at_risk_position_tracking

# Governance tracking
cargo test defi_auditing_tests::test_create_governance_proposal
cargo test defi_auditing_tests::test_record_governance_votes
cargo test defi_auditing_tests::test_update_proposal_status

# Risk metrics
cargo test defi_auditing_tests::test_calculate_risk_metrics
cargo test defi_auditing_tests::test_protocol_health_score

# Audit reports
cargo test defi_auditing_tests::test_generate_audit_report
cargo test defi_auditing_tests::test_get_latest_audit_report
cargo test defi_auditing_tests::test_report_count

# Full workflow
cargo test defi_auditing_tests::test_full_workflow
```

## Performance Tips

1. **Batch Updates**: Update multiple pools in single transaction when possible
2. **Cache Metrics**: Calculate risk metrics periodically, not every query
3. **Lazy Evaluation**: Load protocol data on-demand
4. **Compression**: Use BytesN<32> for IDs (content-addressed hashes)
5. **Indexing**: Maintain protocol and pool lists for fast enumeration

## Troubleshooting

**Protocol not found error:**
- Ensure protocol is registered first
- Check protocol address matches registration

**Oracle price stale:**
- Verify oracle source and update frequency
- Check timestamp vs current time

**Liquidation risk miscalculation:**
- Verify health factor calculation
- Ensure collateral and debt values are current
- Check liquidation threshold configuration

**Report generation failed:**
- Ensure protocol has activity (pools, proposals, etc.)
- Verify findings_hash is valid BytesN<32>
- Check sufficient storage available

## Next Steps

1. Deploy to testnet and monitor real protocols
2. Integrate with off-chain dashboard
3. Set up automated reporting schedule
4. Implement custom alert thresholds
5. Add additional risk metrics and analytics

For detailed API reference, see [DEFI_PROTOCOL_AUDITING.md](DEFI_PROTOCOL_AUDITING.md).
