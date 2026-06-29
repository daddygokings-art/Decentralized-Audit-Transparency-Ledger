# Security Audit Report

## Scope
- Contract version: current `master` branch implementation of `src/lib.rs`
- Files audited: `src/lib.rs`, `src/test.rs`
- Focus: event logging, governance, caps, metadata limits, and storage consistency

## Methodology
- Manual code review of Rust Soroban contract logic
- Test coverage analysis based on existing contract tests
- Pattern inspection for common vulnerabilities: auth checks, panics, overflow, storage integrity

## Findings

### Critical
- No critical issues were identified in reviewed code paths.

### High
- `update_event()` rewrites event data and hash chain for later events. If current event ID changes, later event entries are updated by global order only, which may risk stale indices in external systems.
  - Location: `src/lib.rs` around `update_event`
  - Impact: unauthorized or buggy updates could corrupt chain integrity if external ordering assumptions exist
  - Likelihood: medium
  - Recommendation: add explicit versioning for event order and ensure event hash chain updates are atomic and validated.

### Medium
- `event_count()` is unavailable in low-cost mode and panics instead of returning a safe error.
  - Location: `src/lib.rs` around `event_count`
  - Impact: a user may encounter unexpected panic when reading counts under low-cost mode.
  - Likelihood: medium
  - Recommendation: return an explicit error or document it clearly.

- `log_event_signed()` stores payload without verifying signature format beyond length.
  - Location: `src/lib.rs` around `log_event_signed`
  - Impact: malicious or malformed signatures may be stored, though on-chain verification is intentionally omitted for gas savings.
  - Likelihood: medium
  - Recommendation: require standardized payload structure or off-chain validation conventions.

### Low
- There are legacy tombstone `DataKey` variants such as `EventCapSet` and `EventMaxLogs` still present.
  - Location: `src/lib.rs` `DataKey` enum
  - Impact: code clarity and storage layout management
  - Likelihood: low
  - Recommendation: remove or isolate deprecated variants if no longer needed.

- `bytes_contains()` uses a custom scanning loop that is correct but could be simplified or optimized.
  - Location: `src/lib.rs` `bytes_contains`
  - Impact: low performance, low risk
  - Likelihood: low
  - Recommendation: preserve correctness but consider built-in search helpers if available.

## Positive Observations
- Authorization checks are present for owner-only governance actions: `set_global_max_logs`, `set_event_max_logs`, `remove_event_cap`, `pause`, `unpause`, `transfer_ownership`, and metadata governance functions.
- Metadata caps are enforced both globally and per event type.
- Rate limiting and timestamp monotonicity checks reduce replay and timestamp-drift risks.
- Event IDs are content-addressed and hashes include the previous event hash, supporting tamper-evident history.

## Test Coverage Analysis
- `src/test.rs` covers initialization, logging, event retrieval, per-type indexing, and integrity verification.
- Additional coverage for governance failure cases exists via `try_*` tests.
- Missing coverage: low-cost mode failure behavior, explicit cap removal edge cases, malicious symbol/metadata fuzzing, and archived event retrieval.

## Recommendations
1. Add explicit on-chain error handling for low-cost mode and read-only access paths.
2. Harden `update_event()` with stronger invariants and chain revalidation.
3. Document signature storage expectations in `log_event_signed()`.
4. Add dedicated fuzz tests for string/symbol bounds, empty event types, and metadata edge values.
5. Add CI coverage for the new benchmark and audit documentation validation.
