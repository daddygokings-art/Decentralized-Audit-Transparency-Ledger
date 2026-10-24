# ESG Reporting Error Handling Guide

## Overview
After issue #874 hardening, all ESG reporting functions return `Result<T, ESGReportingError>` for safe error handling without panics.

## Error Codes

| Code | Error | Cause | Resolution |
|------|-------|-------|------------|
| 4001 | `CollectionFailed` | Data collection failed | Verify inputs and RBAC permissions |
| 4002 | `IncompletData` | Incomplete ESG data | Provide all required metrics |
| 4003 | `AlignmentFailed` | Framework alignment failed | Check framework compatibility |
| 4004 | `InvalidMetric` | Invalid metric value | Validate metric ranges |
| 4005 | `GenerationFailed` | Report generation failed or not found | Verify report ID exists |
| 4006 | `UnknownStakeholder` | Stakeholder type not recognized | Use valid StakeholderType variant |
| 4007 | `ValidationFailed` | Validation failed | Check input constraints |
| 4008 | `KPIError` | KPI tracking error | Verify KPI data structure |
| 4009 | `Unauthorized` | RBAC authorization failed | Grant required permission |
| 4010 | `Suspended` | RBAC role suspended | Reinstate suspended role |
| 4011 | `Revoked` | RBAC role revoked | Re-assign role |
| 4012 | `InsufficientRole` | Insufficient RBAC role | Upgrade to required role level |
| 4013 | `ArithmeticOverflow` | Arithmetic overflow detected | Reduce input magnitudes |
| 4014 | `CollectionOverflow` | Collection exceeds max bound | Archive old data or reduce items |
| 4015 | `InvalidInputRange` | Input outside valid range (e.g., >100%) | Validate inputs (percentages: 0-100) |
| 4016 | `DivisionByZero` | Division by zero attempted | Ensure non-zero denominators |

## Collection Limits

```rust
MAX_FRAMEWORK_ALIGNMENTS = 10    // Per ESG report
MAX_KPIS = 100                   // Per organization
MAX_ORG_REPORTS = 1000           // Per organization
MAX_ALIGNMENT_GAPS = 50          // Per framework alignment
```

## Common Patterns

### Basic Error Handling

```rust
// Pattern 1: Propagate errors with ?
pub fn my_function(env: &Env) -> Result<(), ESGReportingError> {
    let org = get_organization();
    let metrics = collect_environmental_data(&env, org, 5000, 80, 75, 1000)?;
    // ... rest of logic
    Ok(())
}

// Pattern 2: Match specific errors
let result = generate_esg_report(&env, org, start, end, e, s, g);
match result {
    Ok(report_id) => {
        // Success path
    }
    Err(ESGReportingError::CollectionOverflow) => {
        // Handle overflow: archive old reports
    }
    Err(ESGReportingError::InvalidInputRange) => {
        // Handle validation: fix inputs
    }
    Err(e) => {
        // Handle other errors
    }
}

// Pattern 3: Unwrap in tests
#[test]
fn test_something() {
    let metrics = collect_social_data(&env, org, 500, 40, 40, 2).unwrap();
    assert_eq!(metrics.employee_count, 500);
}
```

### Input Validation

```rust
// Percentages must be 0-100
let result = collect_environmental_data(&env, org, 5000, 150, 75, 1000);
assert_eq!(result, Err(ESGReportingError::InvalidInputRange));

// Valid range
let result = collect_environmental_data(&env, org, 5000, 80, 75, 1000);
assert!(result.is_ok());
```

### Collection Overflow Handling

```rust
// Check before adding alignments
let mut count = 0;
for framework in frameworks {
    match align_to_gri(&env, report_id.clone(), org.clone()) {
        Ok(alignment) => count += 1,
        Err(ESGReportingError::CollectionOverflow) => {
            // Max alignments reached, stop adding
            break;
        }
        Err(e) => return Err(e),
    }
}
```

### Division by Zero

```rust
// Trend analysis requires non-zero previous value
let result = trend_analysis(&env, metric_name, current, 0);
assert_eq!(result, Err(ESGReportingError::DivisionByZero));

// Use with valid denominator
let result = trend_analysis(&env, metric_name, 5000, 4500);
assert!(result.is_ok());
```

### Report Retrieval

```rust
// Check if report exists
match get_esg_report(&env, report_id) {
    Ok(report) => {
        // Process report
    }
    Err(ESGReportingError::GenerationFailed) => {
        // Report not found, handle gracefully
    }
    Err(e) => return Err(e),
}
```

## RBAC Permission Mapping

| Operation | Required Permission | Min Role |
|-----------|---------------------|----------|
| `collect_environmental_data` | `EsgDataWrite` | Submitter |
| `collect_social_data` | `EsgDataWrite` | Submitter |
| `collect_governance_data` | `EsgDataWrite` | Submitter |
| `generate_esg_report` | `EsgReportGenerate` | Submitter |
| `align_to_*` | `EsgFrameworkAlign` | Auditor |
| `generate_*_report` | `EsgReportGenerate` | Submitter |
| `track_kpis` | `EsgDataWrite` | Submitter |

## Testing Recommendations

### Boundary Tests
```rust
#[test]
fn test_max_values() {
    // Test with u32::MAX
    let metrics = collect_environmental_data(
        &env, org, u32::MAX, 50, 50, u32::MAX
    ).unwrap();
    assert_eq!(metrics.carbon_emissions, u32::MAX);
}

#[test]
fn test_invalid_percentage() {
    let result = collect_social_data(&env, org, 500, 101, 40, 2);
    assert_eq!(result, Err(ESGReportingError::InvalidInputRange));
}
```

### Property Tests
```rust
proptest! {
    #[test]
    fn prop_valid_percentages(pct in 0u32..=100) {
        let result = collect_environmental_data(&env, org, 1000, pct, 50, 1000);
        prop_assert!(result.is_ok());
    }
}
```

## Migration Checklist

- [ ] Update all function calls to handle `Result` return type
- [ ] Add error handling for `CollectionOverflow` errors
- [ ] Validate percentage inputs before calling collect functions
- [ ] Add tests for boundary conditions (u32::MAX, 0, 100)
- [ ] Check RBAC permissions are properly assigned
- [ ] Verify report IDs exist before retrieval
- [ ] Handle division-by-zero scenarios in trend analysis
- [ ] Archive old data before hitting collection limits

## Performance Notes

- **Storage footprint**: 48KB worst-case per organization
- **CPU bounds**: 5K instructions worst-case per operation
- **All operations**: Deterministic, bounded execution
- **No panics**: All errors return typed variants

## Further Reading

- Implementation: `src/esg_reporting.rs`
- Test suite: `src/esg_reporting_tests.rs`
- RBAC integration: `src/rbac.rs`
- Issue tracking: #874
