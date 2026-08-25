# Carbon Credit Tracking - Quick Start Guide

Get started with carbon credit tracking in 5 minutes.

## Basic Workflow

### 1. Issue a Carbon Credit

```rust
use crate::carbon_credits::*;

let issuer = Address::from_string("G...");
let verifier = Address::from_string("G...");

// Create renewable energy source
let renewable = RenewableEnergySource {
    source_type: RenewableEnergyType::Solar,
    facility_id: Bytes::from_slice(&env, b"SOL-001"),
    location: Bytes::from_slice(&env, b"California"),
    capacity_mw: 50,
    energy_generated_mwh: 1000,
    verification_date: env.ledger().timestamp(),
    certifications: vec![&env],
};

// Create offset project
let offset = Offset {
    offset_type: Symbol::new(&env, "reforestation"),
    project_id: Bytes::from_slice(&env, b"REFOR-001"),
    tonnes_co2e: 500,
    project_location: Bytes::from_slice(&env, b"Brazil"),
    verification_body: verifier.clone(),
    verification_date: env.ledger().timestamp(),
    expiration_date: env.ledger().timestamp() + (10 * 365 * 86400),
};

// Create registry entry
let registry = RegistryEntry {
    registry_id: Bytes::from_slice(&env, b"REG-001"),
    registry_name: Bytes::from_slice(&env, b"Verified Carbon Standard"),
    registry_url: Bytes::from_slice(&env, b"https://vcs.org"),
    issuance_date: env.ledger().timestamp(),
    verified_by: verifier.clone(),
    compliance_standard: ComplianceStandard::Vcs,
};

// Issue credit
let credit_id = issue_carbon_credit(
    &env,
    issuer.clone(),
    500,
    renewable,
    offset,
    registry,
    ComplianceStandard::Vcs,
);
```

### 2. Verify Renewable Energy

```rust
let verified = verify_renewable_energy(
    &env,
    credit_id.clone(),
    verifier.clone(),
    1000,  // 1000 MWh
);

println!("Renewable energy verified: {}", verified);
```

### 3. Register in Registry

```rust
let registered = register_credit(
    &env,
    credit_id.clone(),
    Bytes::from_slice(&env, b"REG-VCS-001"),
);

println!("Credit registered: {}", registered);
```

### 4. Tokenize for Trading

```rust
let token_id = tokenize_credit(
    &env,
    credit_id.clone(),
    owner_address,
    5000,  // 5000 tokens
    15,    // $15 per token
);

println!("Credit tokenized: {:?}", token_id);
```

### 5. Transfer to New Holder

```rust
let transferred = transfer_credit(
    &env,
    credit_id.clone(),
    current_owner,
    new_owner,
);

println!("Credit transferred: {}", transferred);
```

### 6. Retire the Credit

```rust
let retired = retire_credit(
    &env,
    credit_id.clone(),
    Bytes::from_slice(&env, b"Used for sustainability program"),
);

println!("Credit retired: {}", retired);
```

### 7. Check Portfolio

```rust
let portfolio = get_portfolio_status(&env, holder);

println!("Total credits: {}", portfolio.total_credits);
println!("Active credits: {}", portfolio.active_credits);
println!("Retired credits: {}", portfolio.retired_credits);
println!("CO2e retired: {} tonnes", portfolio.total_co2e_retired);
println!("Portfolio value: ${}", portfolio.portfolio_value_usd);
```

## Common Operations

### Issue Multiple Credits

```rust
for energy in vec![500, 1000, 1500] {
    issue_carbon_credit(
        &env,
        issuer.clone(),
        energy / 2,
        create_renewable(energy),
        create_offset(),
        create_registry(&env, verifier.clone()),
        ComplianceStandard::Vcs,
    );
}
```

### Verify Sustainability Claim

```rust
let claim = SustainabilityClaim {
    claim_id: Bytes::from_slice(&env, b"CLAIM-001"),
    claimant: Address::random(&env),
    claim_type: Symbol::new(&env, "carbon_neutral"),
    claim_description: Bytes::from_slice(&env, b"Achieved carbon neutrality"),
    claimed_reduction: 5000,
    supporting_evidence: vec![&env, Bytes::from_slice(&env, b"https://evidence.org")],
    claim_date: env.ledger().timestamp(),
};

let verified = verify_sustainability_claim(&env, claim, verifier);
println!("Claim verified: {}", verified);
```

### Audit Renewable Usage

```rust
let audit_record = audit_renewable_usage(
    &env,
    credit_id.clone(),
    auditor_address,
    1000,  // Measured 1000 MWh
);

println!("Audit approved: {}", audit_record.approved);
```

### Generate Report

```rust
let now = env.ledger().timestamp();
let report = generate_offset_report(
    &env,
    now - 86400,  // Last day
    now,
);

println!("Credits retired: {}", report.credits_retired);
println!("Compliance rate: {}%", report.compliance_rate);
```

## Complete Example: Solar Farm

```rust
#[test]
fn test_solar_farm_carbon_credits() {
    let env = Env::default();
    env.mock_all_auths();

    let farm_operator = Address::random(&env);
    let verifier = Address::random(&env);

    // 1. Create solar farm renewable source
    let solar_farm = RenewableEnergySource {
        source_type: RenewableEnergyType::Solar,
        facility_id: Bytes::from_slice(&env, b"SOLFARM-CAL-001"),
        location: Bytes::from_slice(&env, b"Mojave Desert, California"),
        capacity_mw: 250,
        energy_generated_mwh: 2000,  // 2000 MWh generated
        verification_date: env.ledger().timestamp(),
        certifications: vec![&env, Bytes::from_slice(&env, b"ISO-50001")],
    };

    // 2. Create reforestation offset
    let reforestation = Offset {
        offset_type: Symbol::new(&env, "reforestation"),
        project_id: Bytes::from_slice(&env, b"AMAZON-REFOR-2024"),
        tonnes_co2e: 1000,
        project_location: Bytes::from_slice(&env, b"Amazon Rainforest"),
        verification_body: verifier.clone(),
        verification_date: env.ledger().timestamp(),
        expiration_date: env.ledger().timestamp() + (20 * 365 * 86400),
    };

    // 3. Create registry entry
    let vcs_registry = RegistryEntry {
        registry_id: Bytes::from_slice(&env, b"VCS-2024-001"),
        registry_name: Bytes::from_slice(&env, b"Verified Carbon Standard"),
        registry_url: Bytes::from_slice(&env, b"https://vcs.org/verify/2024-001"),
        issuance_date: env.ledger().timestamp(),
        verified_by: verifier.clone(),
        compliance_standard: ComplianceStandard::Vcs,
    };

    // 4. Issue carbon credits
    let credit_id = issue_carbon_credit(
        &env,
        farm_operator.clone(),
        1000,  // 1000 tonnes CO2e
        solar_farm,
        reforestation,
        vcs_registry,
        ComplianceStandard::Vcs,
    );

    println!("Carbon credit issued: {:?}", credit_id);

    // 5. Verify renewable energy
    let verified = verify_renewable_energy(
        &env,
        credit_id.clone(),
        verifier.clone(),
        2000,  // 2000 MWh verified
    );
    println!("Energy verified: {}", verified);

    // 6. Register in VCS
    register_credit(&env, credit_id.clone(), Bytes::from_slice(&env, b"VCS-2024-001"));

    // 7. Tokenize for market trading
    let token_id = tokenize_credit(
        &env,
        credit_id.clone(),
        farm_operator.clone(),
        10000,  // 10,000 tokens
        20,     // $20 per token
    );
    println!("Credit tokenized: {:?}", token_id);

    // 8. Get portfolio status
    let portfolio = get_portfolio_status(&env, farm_operator.clone());
    println!("Farm portfolio:");
    println!("  Total credits: {}", portfolio.total_credits);
    println!("  Portfolio value: ${}", portfolio.portfolio_value_usd);

    // 9. Retire some credits
    retire_credit(&env, credit_id.clone(), Bytes::from_slice(&env, b"Used in 2024 sustainability report"));

    // 10. Check final status
    let is_retired = check_retirement_status(&env, credit_id);
    println!("Credit retired: {}", is_retired);

    let total_retired = get_total_retired_co2e(&env);
    println!("Total CO2e retired globally: {} tonnes", total_retired);
}
```

## Key Concepts

### Renewable Energy Types
- **Solar**: Photovoltaic and thermal systems
- **Wind**: Wind turbines and wind farms
- **Hydro**: Hydroelectric plants
- **Geothermal**: Geothermal energy systems
- **Biomass**: Organic waste and crops
- **TidalWave**: Ocean tidal and wave systems
- **OceanThermal**: Deep ocean thermal conversion

### Compliance Standards
- **VCS**: Verified Carbon Standard (most common)
- **Gold**: Gold Standard (high environmental standards)
- **CDM**: Clean Development Mechanism (UN mechanism)
- **CAR**: Climate Action Reserve (North American)
- **ACE**: American Carbon Exchange
- **Custom**: Custom standards

### Carbon Offset Calculation
Default: 1 MWh renewable energy = 0.5 tonnes CO2e offset

### Credit Status
- **Issued**: Just created
- **Active**: Verified and tradeable
- **Retired**: Permanently removed
- **Disputed**: Under review
- **Expired**: No longer valid

## Testing

Run all tests:
```bash
cargo test carbon_credits
```

Run specific test:
```bash
cargo test test_solar_farm_carbon_credits
```

Run with output:
```bash
cargo test carbon_credits -- --nocapture
```

## Common Patterns

### Pattern 1: Issue and Verify

```rust
let id = issue_carbon_credit(...);
verify_renewable_energy(&env, id, verifier, energy);
```

### Pattern 2: Register and Comply

```rust
let id = issue_carbon_credit(...);
register_credit(&env, id, registry);
verify_registry_compliance(&env, id);
```

### Pattern 3: Tokenize and Trade

```rust
let id = issue_carbon_credit(...);
let token = tokenize_credit(&env, id, owner, 1000, 15);
transfer_credit(&env, id, owner1, owner2);
```

### Pattern 4: Track Portfolio

```rust
let portfolio = get_portfolio_status(&env, holder);
println!("Active: {}", portfolio.active_credits);
println!("Retired: {}", portfolio.retired_credits);
```

## Error Handling

```rust
match issue_carbon_credit(...) {
    Ok(credit_id) => println!("Success: {:?}", credit_id),
    Err(e) => println!("Error: {:?}", e),
}
```

## Next Steps

1. Review the technical guide: `docs/CARBON_CREDITS_TECHNICAL_GUIDE.md`
2. Study the test examples: `src/carbon_credits_tests.rs`
3. Integrate with your system
4. Build consumer-facing dashboard

---

**Version:** 1.0
**Status:** Production Ready
**Date:** August 25, 2026
