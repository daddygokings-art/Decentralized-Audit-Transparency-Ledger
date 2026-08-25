# Supply Chain Transparency - Quick Start Guide

Get started with the supply chain transparency features in 5 minutes.

## Basic Flow

### 1. Register Your Brand

```rust
use supply_chain::*;

let owner = Address::from_string("GXXXXX...");
let brand_id = Symbol::new(&env, "MYCOMPANY");

register_brand(
    &env,
    owner,
    brand_id.clone(),
    Bytes::from_slice(&env, b"My Company"),
    Bytes::from_slice(&env, b"Quality products since 2020"),
    Bytes::from_slice(&env, b"https://mycompany.com"),
    Bytes::from_slice(&env, b"support@mycompany.com"),
);
```

### 2. Register Products

```rust
let sku = Bytes::from_slice(&env, b"WIDGET-001");

register_product_sku(
    &env,
    brand_id.clone(),
    sku.clone(),
    Bytes::from_slice(&env, b"Premium Widget"),
    Bytes::from_slice(&env, b"High quality widget"),
);
```

### 3. Log Product Origin

```rust
let factory_location = Location {
    name: Bytes::from_slice(&env, b"Factory A"),
    country: Symbol::new(&env, "US"),
    coordinates: Bytes::from_slice(&env, b"37.7749,-122.4194"),
    facility_id: Bytes::from_slice(&env, b"FAC001"),
};

let producer = Address::from_string("GYYYYY...");
let event_id = BytesN::<32>::random(&env);

log_provenance_event(
    &env,
    event_id,
    factory_location,
    Bytes::from_slice(&env, b"Recycled aluminum"),
    producer,
    Bytes::from_slice(&env, b"BATCH-2024-001"),
);
```

### 4. Add Certifications

```rust
let certifier = Address::from_string("GZZZZ...");

log_certification(
    &env,
    Bytes::from_slice(&env, b"CERT-ISO9001"),
    Symbol::new(&env, "ISO_9001"),
    certifier,
    365,  // Valid for 1 year
    Bytes::from_slice(&env, b"ISO 9001 Quality Management"),
);
```

### 5. Audit Labor Conditions

```rust
let auditor = Address::from_string("GAAA...");

log_labor_conditions(
    &env,
    Bytes::from_slice(&env, b"FAC001"),
    250,     // workers
    true,    // wage compliant
    true,    // hours compliant
    true,    // no child labor
    true,    // safety met
    true,    // freedom of association
    BytesN::<32>::random(&env),  // report hash
    auditor,
);
```

### 6. Track Environmental Impact

```rust
let reporter = Address::from_string("GBBB...");
let now = env.ledger().timestamp();

log_environmental_impact(
    &env,
    Bytes::from_slice(&env, b"FAC001"),
    now,
    now + 30 * 86400,  // 30 day report period
    5000,              // kg CO2e
    1000000,           // liters water
    500,               // kg waste
    80,                // 80% renewable energy
    10,                // 10% emissions reduction
    BytesN::<32>::random(&env),
    reporter,
);
```

### 7. Track Custody Transfers

```rust
let distributor = Address::from_string("GCCC...");
let transfer_location = Location {
    name: Bytes::from_slice(&env, b"Distribution Center"),
    country: Symbol::new(&env, "CA"),
    coordinates: Bytes::from_slice(&env, b"43.6532,-79.3832"),
    facility_id: Bytes::from_slice(&env, b"DC001"),
};

log_custody_transfer(
    &env,
    event_id,
    producer,
    distributor,
    transfer_location,
    Bytes::from_slice(&env, b"Shipped via truck, 2 days in transit"),
);
```

### 8. Verify Product

```rust
let verification = verify_product_chain(
    &env,
    brand_id.clone(),
    sku.clone(),
);

println!("Product verified: {}", verification.is_verified);
println!("Compliance score: {}", verification.verification_score);
println!("Provenance verified: {}", verification.provenance_verified);
println!("Labor compliant: {}", verification.labor_compliant);
println!("Environmental standards met: {}", verification.environmental_standards_met);

for issue in verification.issues.iter() {
    println!("Issue: {:?}", issue);
}
```

### 9. Get Product Timeline

```rust
let event_ids = vec![&env, event_id];
let timeline = get_product_timeline(&env, event_ids);

for entry in timeline.iter() {
    println!(
        "{}: {} at {:?}",
        entry.timestamp,
        entry.entry_type,
        entry.location
    );
}
```

### 10. Generate QR Code

```rust
let qr_url = generate_qr_code_url(
    &env,
    brand_id.clone(),
    sku.clone(),
    Bytes::from_slice(&env, b"https://verify.mycompany.com"),
);

// Use qr_url to generate actual QR code image
```

## Complete Example: Chocolate Bar

Here's a complete example tracking a fair-trade chocolate bar:

```rust
#[test]
fn test_chocolate_bar_supply_chain() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Register brand
    let owner = Address::random(&env);
    let brand_id = Symbol::new(&env, "FAIR_TRADE_CO");

    register_brand(
        &env,
        owner.clone(),
        brand_id.clone(),
        Bytes::from_slice(&env, b"Fair Trade Company"),
        Bytes::from_slice(&env, b"Ethical chocolate sourcing"),
        Bytes::from_slice(&env, b"https://fairtradecos.com"),
        Bytes::from_slice(&env, b"info@fairtradecos.com"),
    );

    // 2. Register product
    let sku = Bytes::from_slice(&env, b"CHOCO-DARK-70");
    register_product_sku(
        &env,
        brand_id.clone(),
        sku.clone(),
        Bytes::from_slice(&env, b"Dark Chocolate 70%"),
        Bytes::from_slice(&env, b"Fair-trade dark chocolate"),
    );

    // 3. Log provenance (cocoa farm)
    let cacao_farm = Location {
        name: Bytes::from_slice(&env, b"Sunshine Cacao Farm"),
        country: Symbol::new(&env, "EC"),
        coordinates: Bytes::from_slice(&env, b"-1.8312,78.1834"),
        facility_id: Bytes::from_slice(&env, b"FARM-EC-001"),
    };

    let farmer = Address::random(&env);
    let event_id = BytesN::<32>::random(&env);

    log_provenance_event(
        &env,
        event_id,
        cacao_farm.clone(),
        Bytes::from_slice(&env, b"Organic cacao beans"),
        farmer.clone(),
        Bytes::from_slice(&env, b"BATCH-2024-CACAO-001"),
    );

    // 4. Add fair trade certification
    let certifier = Address::random(&env);
    log_certification(
        &env,
        Bytes::from_slice(&env, b"CERT-FAIR-TRADE-001"),
        Symbol::new(&env, "FAIR_TRADE"),
        certifier.clone(),
        730,  // 2 years
        Bytes::from_slice(&env, b"Fair Trade Certified cacao"),
    );

    // 5. Audit farm labor (excellent conditions)
    let labor_auditor = Address::random(&env);
    log_labor_conditions(
        &env,
        Bytes::from_slice(&env, b"FARM-EC-001"),
        150,   // workers
        true,  // wages meet or exceed minimum
        true,  // hours compliant
        true,  // no child labor
        true,  // safety standards met
        true,  // freedom of association
        BytesN::<32>::random(&env),
        labor_auditor.clone(),
    );

    // 6. Environmental impact (sustainable practices)
    let env_auditor = Address::random(&env);
    let now = env.ledger().timestamp();

    log_environmental_impact(
        &env,
        Bytes::from_slice(&env, b"FARM-EC-001"),
        now,
        now + 90 * 86400,
        2000,   // carbon footprint (lower due to sustainability)
        500000, // water (rainwater harvesting)
        100,    // waste (composted)
        70,     // renewable energy
        15,     // emissions reduced from prior year
        BytesN::<32>::random(&env),
        env_auditor.clone(),
    );

    // 7. Track to chocolate factory
    let chocolate_factory = Location {
        name: Bytes::from_slice(&env, b"Premium Chocolate Factory"),
        country: Symbol::new(&env, "CH"),
        coordinates: Bytes::from_slice(&env, b"47.3769,8.5472"),
        facility_id: Bytes::from_slice(&env, b"FACTORY-CH-001"),
    };

    let manufacturer = Address::random(&env);

    log_custody_transfer(
        &env,
        event_id,
        farmer.clone(),
        manufacturer.clone(),
        chocolate_factory.clone(),
        Bytes::from_slice(&env, b"Shipped in sealed containers, 2 weeks at sea"),
    );

    // 8. Verify complete supply chain
    let verification = verify_product_chain(&env, brand_id.clone(), sku.clone());

    assert_eq!(verification.product_sku, sku);
    assert!(verification.provenance_verified);
    assert!(verification.labor_compliant);
    assert!(verification.environmental_standards_met);
    println!("Chocolate bar verified with score: {}", verification.verification_score);

    // 9. Get timeline for consumer
    let timeline = get_product_timeline(&env, vec![&env, event_id]);
    assert!(!timeline.is_empty());
    println!("Product timeline has {} events", timeline.len());

    // 10. Generate QR code
    let qr_url = generate_qr_code_url(
        &env,
        brand_id.clone(),
        sku.clone(),
        Bytes::from_slice(&env, b"https://verify.fairtradecos.com"),
    );
    assert!(!qr_url.is_empty());
}
```

## Testing Your Code

Run all supply chain tests:

```bash
cargo test supply_chain
```

Run a specific test:

```bash
cargo test test_full_supply_chain_scenario
```

## Error Handling

Common errors and solutions:

| Error | Cause | Solution |
|-------|-------|----------|
| `BrandNotRegistered` | Brand doesn't exist | Call `register_brand()` first |
| `SkuNotFound` | Product not registered | Call `register_product_sku()` |
| `CertificationExpired` | Cert date has passed | Recertify with new expiry date |
| `UnauthorizedBrandAccess` | Not brand owner | Use correct owner address |
| `IncompleteProvenance` | Missing provenance link | Call `log_provenance_event()` |

## Next Steps

1. Read [docs/SUPPLY_CHAIN.md](../SUPPLY_CHAIN.md) for complete API reference
2. Review [SUPPLY_CHAIN_IMPLEMENTATION.md](../SUPPLY_CHAIN_IMPLEMENTATION.md) for architecture
3. Check `src/supply_chain_tests.rs` for more examples
4. Integrate with your brand/product database
5. Build consumer-facing QR code verification UI

## Common Patterns

### Pattern: Register and Track a Product

```rust
// 1. Register brand (once)
register_brand(...);

// 2. Register product (once per SKU)
register_product_sku(...);

// 3. Log provenance (once at origin)
log_provenance_event(...);

// 4. Log certifications (as obtained)
log_certification(...);

// 5. Log audits (periodically)
log_labor_conditions(...);
log_environmental_impact(...);

// 6. Track transfers (at each handoff)
log_custody_transfer(...);

// 7. Consumer verification (on demand)
verify_product_chain(...);
```

### Pattern: Facility Auditing

```rust
// Annual labor audit
log_labor_conditions(
    &env,
    facility_id,
    worker_count,
    true,  // wage compliant
    true,  // hours compliant
    true,  // no child labor
    true,  // safety
    true,  // freedom of association
    report_hash,
    auditor,
);

// Annual environmental audit
log_environmental_impact(
    &env,
    facility_id,
    period_start,
    period_end,
    carbon,
    water,
    waste,
    renewable_percent,
    reduction_percent,
    report_hash,
    auditor,
);
```

### Pattern: Consumer Verification

```rust
// Get verification result
let verification = verify_product_chain(&env, brand_id, sku);

// Check compliance
if verification.is_verified {
    println!("✓ Product verified");
    println!("Score: {}/100", verification.verification_score);
} else {
    for issue in verification.issues {
        println!("⚠ Issue: {:?}", issue);
    }
}

// Get timeline
let timeline = get_product_timeline(&env, event_ids);
for entry in timeline {
    println!("{}: {}", entry.timestamp, entry.description);
}
```

## Support

For questions or issues:
1. Check the [full API documentation](SUPPLY_CHAIN.md)
2. Review test examples in [src/supply_chain_tests.rs](../src/supply_chain_tests.rs)
3. Open an issue on GitHub
