# EU ESPR Digital Product Passport - Quick Start Guide

Get started with ESPR-compliant digital product passports in 5 minutes.

## Installation

Add to your imports:

```rust
use crate::digital_passport::*;
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, vec};
```

## Basic Workflow

### 1. Prepare Material Data

```rust
let env = Env::default();
env.mock_all_auths();

// Define materials
let mut materials = vec![&env];
materials.push_back(Material {
    material_name: Bytes::from_slice(&env, b"Aluminum"),
    material_code: Symbol::new(&env, "AL"),
    percentage_by_weight: 60,
    source_type: Symbol::new(&env, "recycled"),
    hazardous: false,
    hazard_classification: Bytes::from_slice(&env, b""),
});

materials.push_back(Material {
    material_name: Bytes::from_slice(&env, b"Plastic"),
    material_code: Symbol::new(&env, "PL"),
    percentage_by_weight: 40,
    source_type: Symbol::new(&env, "virgin"),
    hazardous: false,
    hazard_classification: Bytes::from_slice(&env, b""),
});
```

### 2. Define Durability

```rust
let durability = Durability {
    expected_lifetime_years: 5,
    warranty_years: 2,
    spare_parts_available: true,
    spare_parts_years: 10,
    repair_information: Bytes::from_slice(&env, b"https://repair.example.com"),
    repairability_score: 8,
};
```

### 3. Define Circularity

```rust
let circularity = Circularity {
    recyclable_materials: materials.clone(),
    recycled_content_percent: 30,
    reuse_potential: true,
    refurbishment_potential: true,
    disassembly_instructions: Bytes::from_slice(&env, b"See manual page 5"),
    recycling_instructions: Bytes::from_slice(&env, b"Place in metal bin"),
    end_of_life_score: 85,
};
```

### 4. Define Carbon Footprint

```rust
let carbon = CarbonFootprint {
    manufacturing_emissions: 500,      // kg CO2e
    distribution_emissions: 100,       // kg CO2e
    use_phase_emissions: 50,           // kg CO2e/year
    end_of_life_emissions: 25,         // kg CO2e
    total_embodied_carbon: 675,        // kg CO2e
    carbon_neutral: false,
    carbon_offset_program: Bytes::from_slice(&env, b""),
    measurement_standard: Symbol::new(&env, "ISO_14040"),
    measurement_date: env.ledger().timestamp(),
};
```

### 5. Create Passport

```rust
let manufacturer = Address::random(&env);
let product_id = Bytes::from_slice(&env, b"PRODUCT-ABC-001");

let passport_id = create_passport(
    &env,
    product_id.clone(),
    Bytes::from_slice(&env, b"Premium Widget"),
    Symbol::new(&env, "consumer_goods"),
    manufacturer.clone(),
    Bytes::from_slice(&env, b"v1.0"),
    Bytes::from_slice(&env, b"BATCH-2024-001"),
    materials,
    durability,
    circularity,
    carbon,
);

println!("Passport created: {:?}", passport_id);
```

## Common Operations

### Verify ESPR Compliance

```rust
let verifier = Address::random(&env);
let status = verify_espr_compliance(&env, passport_id.clone(), verifier);

match status {
    ComplianceStatus::Compliant => println!("✓ Fully compliant"),
    ComplianceStatus::PartiallyCompliant => println!("⚠ Partially compliant"),
    ComplianceStatus::NonCompliant => println!("✗ Non-compliant"),
    ComplianceStatus::PendingVerification => println!("⏳ Under review"),
}
```

### Record a Repair

```rust
let repair_facility = Address::random(&env);

record_repair(
    &env,
    passport_id.clone(),
    repair_facility,
    Symbol::new(&env, "major"),  // maintenance, major, or minor
    vec![&env, Bytes::from_slice(&env, b"Motor")],  // parts replaced
    Bytes::from_slice(&env, b"Replaced motor and bearing assembly"),
);
```

### Record Recycling

```rust
let recycling_facility = Address::random(&env);

record_recycling(
    &env,
    passport_id.clone(),
    recycling_facility,
    92,  // recovery rate percent
    materials.clone(),
    Bytes::from_slice(&env, b"CERT-RECYCLE-2024-001"),
);
```

### Get Passport Details

```rust
// Get complete passport
let passport = get_passport(&env, passport_id.clone());
println!("Product: {}", String::from_utf8(passport.identity.product_name.to_vec()).unwrap());

// Get materials
let breakdown = get_material_breakdown(&env, passport_id.clone());
for material in breakdown.iter() {
    println!("{}: {}%", 
        String::from_utf8(material.material_name.to_vec()).unwrap(),
        material.percentage_by_weight
    );
}

// Get carbon footprint
let carbon = get_carbon_footprint(&env, passport_id.clone());
println!("Total emissions: {} kg CO2e", carbon.total_embodied_carbon);

// Get repair history
let repairs = get_repair_history(&env, passport_id.clone());
println!("Repairs: {}", repairs.len());
```

### Calculate Environmental Score

```rust
let score = calculate_environmental_score(&env, passport_id.clone());
println!("Environmental score: {}/100", score);
```

### Export Passport

```rust
let export = generate_passport_export(&env, passport_id.clone(), ExportFormat::JsonLd);
println!("Export URL: {}", String::from_utf8(export.verification_url.to_vec()).unwrap());
```

## Complete Example: Laptop

```rust
#[test]
fn test_laptop_passport() {
    let env = Env::default();
    env.mock_all_auths();

    let manufacturer = Address::random(&env);

    // 1. Define materials
    let mut materials = vec![&env];
    materials.push_back(Material {
        material_name: Bytes::from_slice(&env, b"Aluminum"),
        material_code: Symbol::new(&env, "AL"),
        percentage_by_weight: 35,
        source_type: Symbol::new(&env, "recycled"),
        hazardous: false,
        hazard_classification: Bytes::from_slice(&env, b""),
    });
    materials.push_back(Material {
        material_name: Bytes::from_slice(&env, b"Steel"),
        material_code: Symbol::new(&env, "FE"),
        percentage_by_weight: 30,
        source_type: Symbol::new(&env, "virgin"),
        hazardous: false,
        hazard_classification: Bytes::from_slice(&env, b""),
    });
    materials.push_back(Material {
        material_name: Bytes::from_slice(&env, b"Plastic"),
        material_code: Symbol::new(&env, "PL"),
        percentage_by_weight: 20,
        source_type: Symbol::new(&env, "bio_based"),
        hazardous: false,
        hazard_classification: Bytes::from_slice(&env, b""),
    });
    materials.push_back(Material {
        material_name: Bytes::from_slice(&env, b"Glass"),
        material_code: Symbol::new(&env, "GL"),
        percentage_by_weight: 15,
        source_type: Symbol::new(&env, "virgin"),
        hazardous: false,
        hazard_classification: Bytes::from_slice(&env, b""),
    });

    // 2. Durability
    let durability = Durability {
        expected_lifetime_years: 7,
        warranty_years: 2,
        spare_parts_available: true,
        spare_parts_years: 10,
        repair_information: Bytes::from_slice(&env, b"https://support.example.com/laptop/repair"),
        repairability_score: 7,
    };

    // 3. Circularity
    let circularity = Circularity {
        recyclable_materials: materials.clone(),
        recycled_content_percent: 35,
        reuse_potential: true,
        refurbishment_potential: true,
        disassembly_instructions: Bytes::from_slice(&env, b"https://www.ifixit.com/laptop-teardown"),
        recycling_instructions: Bytes::from_slice(&env, b"https://www.e-waste.org/how-to-recycle"),
        end_of_life_score: 80,
    };

    // 4. Carbon Footprint
    let carbon = CarbonFootprint {
        manufacturing_emissions: 150,
        distribution_emissions: 30,
        use_phase_emissions: 100,
        end_of_life_emissions: 15,
        total_embodied_carbon: 295,
        carbon_neutral: false,
        carbon_offset_program: Bytes::from_slice(&env, b""),
        measurement_standard: Symbol::new(&env, "ISO_14040"),
        measurement_date: env.ledger().timestamp(),
    };

    // 5. Create passport
    let passport_id = create_passport(
        &env,
        Bytes::from_slice(&env, b"LAPTOP-PRO-2024"),
        Bytes::from_slice(&env, b"Pro Laptop 16-inch"),
        Symbol::new(&env, "electronics"),
        manufacturer.clone(),
        Bytes::from_slice(&env, b"v2024.1"),
        Bytes::from_slice(&env, b"BATCH-2024-Q2-001"),
        materials,
        durability,
        circularity,
        carbon,
    );

    // 6. Verify compliance
    let verifier = Address::random(&env);
    let status = verify_espr_compliance(&env, passport_id.clone(), verifier);
    assert_eq!(status, ComplianceStatus::Compliant);

    // 7. Transition through lifecycle
    transition_lifecycle_stage(
        &env,
        passport_id.clone(),
        PassportLifecycleStage::InProduction,
        manufacturer.clone(),
        Bytes::from_slice(&env, b"Manufacturing started"),
    );

    transition_lifecycle_stage(
        &env,
        passport_id.clone(),
        PassportLifecycleStage::ReadyForMarket,
        manufacturer.clone(),
        Bytes::from_slice(&env, b"QA testing complete"),
    );

    transition_lifecycle_stage(
        &env,
        passport_id.clone(),
        PassportLifecycleStage::InMarket,
        manufacturer.clone(),
        Bytes::from_slice(&env, b"Available for sale"),
    );

    // 8. Record lifecycle events
    let repair_facility = Address::random(&env);
    record_repair(
        &env,
        passport_id.clone(),
        repair_facility,
        Symbol::new(&env, "maintenance"),
        vec![&env, Bytes::from_slice(&env, b"Fan assembly")],
        Bytes::from_slice(&env, b"Replaced cooling fan"),
    );

    // 9. Check environmental score
    let score = calculate_environmental_score(&env, passport_id.clone());
    println!("Environmental score: {}/100", score);

    // 10. Get details
    let passport = get_passport(&env, passport_id.clone());
    assert_eq!(passport.lifecycle_stage, PassportLifecycleStage::InMarket);
    assert_eq!(passport.carbon_footprint.total_embodied_carbon, 295);

    // 11. Export for consumer
    let export = generate_passport_export(&env, passport_id, ExportFormat::JsonLd);
    assert!(export.digital_signature.is_some());
}
```

## Key Concepts

### Material Percentages
Material percentages must sum to ~100% (±5% tolerance):
```rust
let valid = vec![
    ("Aluminum", 60),  // ✓ 60%
    ("Steel", 40),     // ✓ 40% = 100% total
];
```

### Lifecycle Stages
Products flow through stages:
```
Created → InProduction → ReadyForMarket → InMarket → EndOfLife → Recycled
```

### Compliance Status
- **Compliant** — All ESPR requirements met
- **PartiallyCompliant** — Some data missing
- **NonCompliant** — Critical data missing
- **PendingVerification** — Undergoing audit

### Environmental Score
Calculated from:
- Carbon footprint (0-40 points)
- Recycled content (0-30 points)
- Recyclability (0-30 points)
- **Total: 0-100 points**

## Testing

Run all tests:
```bash
cargo test digital_passport
```

Run specific test:
```bash
cargo test test_full_product_lifecycle
```

Run with output:
```bash
cargo test digital_passport -- --nocapture
```

## Common Patterns

### Pattern 1: Create and Verify

```rust
// Create
let id = create_passport(...);

// Verify
let status = verify_espr_compliance(&env, id, verifier);
assert_eq!(status, ComplianceStatus::Compliant);
```

### Pattern 2: Track Lifecycle

```rust
// Manufacturing phase
transition_lifecycle_stage(&env, id, PassportLifecycleStage::InProduction, ...);

// Ready for customers
transition_lifecycle_stage(&env, id, PassportLifecycleStage::ReadyForMarket, ...);

// Sold
transition_lifecycle_stage(&env, id, PassportLifecycleStage::InMarket, ...);

// End of life
transition_lifecycle_stage(&env, id, PassportLifecycleStage::EndOfLife, ...);

// Recycled
transition_lifecycle_stage(&env, id, PassportLifecycleStage::Recycled, ...);
```

### Pattern 3: Collect Lifecycle Data

```rust
// Record repair
record_repair(&env, id, facility, repair_type, parts, notes);

// Record recycling
record_recycling(&env, id, facility, recovery_rate, materials, cert);

// Get history
let repairs = get_repair_history(&env, id);
let recyclings = get_recycling_history(&env, id);
let lifecycle = get_lifecycle_history(&env, id);
```

### Pattern 4: Export for Consumer

```rust
// Generate export
let export = generate_passport_export(&env, id, ExportFormat::JsonLd);

// Consumer verifies via URL
let url = export.verification_url;
// User opens: https://verify.example.com/passport/{id}
```

## Error Handling

```rust
use crate::digital_passport::DigitalPassportError;

// Handle errors
match result {
    Ok(id) => println!("Success: {:?}", id),
    Err(DigitalPassportError::PassportNotFound) => println!("Not found"),
    Err(DigitalPassportError::MissingMandatoryData) => println!("Incomplete data"),
    Err(DigitalPassportError::InvalidMaterialComposition) => println!("Material error"),
    _ => println!("Other error"),
}
```

## Next Steps

1. **Read Full Documentation**: `docs/EU_ESPR_DIGITAL_PASSPORT.md`
2. **Review Tests**: `src/digital_passport_tests.rs`
3. **Check API**: `src/digital_passport.rs`
4. **Build UI**: Consumer verification app
5. **Integrate**: With existing product systems

## Support

- **API Reference**: `docs/EU_ESPR_DIGITAL_PASSPORT.md`
- **Test Examples**: `src/digital_passport_tests.rs`
- **Code**: `src/digital_passport.rs`

---

**Version:** 1.0
**ESPR Compliance:** ✅ Yes
**Last Updated:** August 25, 2026
