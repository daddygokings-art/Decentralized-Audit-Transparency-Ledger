# AuditLedger ContractError Reference

This document provides a comprehensive reference for all 18 `ContractError` variants returned by the AuditLedger Soroban contract. Integrators can use error codes to implement retry logic, user-facing messaging, or monitoring.

## Error Table

| Code | Variant | Description | Common Cause | Resolution |
|------|---------|-------------|--------------|-----------|
| 1 | `CallerNotOwner` | Caller does not have owner privileges | Non-owner attempting governance function | Contact current owner for delegation, or ensure caller is authorized |
| 2 | `GlobalMaxLogsReached` | Global event log capacity reached | Total events ≥ `global_max_logs` | Owner should increase cap or archive off-chain |
| 3 | `EventTypeMaxLogsReached` | Per-event-type log capacity reached | Event type count ≥ type-specific cap | Owner should increase cap or call `remove_event_cap` |
| 4 | `EventDoesNotExist` | Event ID does not exist | Querying non-existent index or invalid hash | Verify event ID against `total_events()` |
| 5 | `EventTypeIndexOutOfBounds` | Index out of bounds for sub-ledger | `type_index` ≥ `event_count(event_type)` | Ensure `type_index < event_count(event_type)` |
| 6 | `NewOwnerIsZero` | New owner address is zero/invalid | Invalid address in `transfer_ownership()` | Provide valid Stellar account address |
| 7 | `CapNotSet` | Event-type cap not set | `remove_event_cap()` on uncapped type | Use `set_event_max_logs()` first |
| 8 | `MetadataTooLarge` | Metadata exceeds max size | Payload > limit | Reduce metadata size or owner increases limit |
| 9 | `ContractNotInitialized` | Contract not initialized | Functions called before `initialize()` | Owner must call `initialize()` at deployment |
| 10 | `TotalEventsOverflow` | Total events would overflow | Architectural limit reached (rare) | Contact developers; consider migration |
| 11 | `TimestampOutOfRange` | Timestamp outside acceptable range | Clock skew > 3600 seconds | Verify system clock, resubmit with correct timestamp |
| 12 | `InvalidSignature` | Event signature validation failed | Data tampering or wrong signing key | Re-sign with correct key, verify data integrity |
| 13 | `ContractPaused` | Contract paused; writes blocked | Owner called `set_paused(true)` | Contact owner to resume with `set_paused(false)` |
| 14 | `RateLimitExceeded` | Submitter exceeded rate limit | Too many events in single ledger | Wait for next ledger or owner increases limit |
| 15 | `SameOwner` | Transfer to same owner address | Same address in `transfer_ownership()` | Provide different owner address |
| 16 | `MaxLogsBelowCurrentCount` | New max below current count | `set_global_max_logs()` or `set_event_max_logs()` with low value | Set max ≥ current count, or prune first |
| 17 | `CapAlreadyRemoved` | Cap already removed | `remove_event_cap()` called twice | No action needed; cap is lifted |
| 18 | `CapNeverSet` | Cap never set (state inconsistency) | Rare internal state issue | Use `set_event_max_logs()` to set cap |

## Error Handling Patterns

### Retriable Errors

Errors that may succeed on retry (after state change or time):
- **Code 2** (`GlobalMaxLogsReached`) — Retry after owner increases `global_max_logs`
- **Code 3** (`EventTypeMaxLogsReached`) — Retry after owner increases type cap or calls `remove_event_cap`
- **Code 13** (`ContractPaused`) — Retry after owner resumes contract
- **Code 14** (`RateLimitExceeded`) — Retry in next ledger or after rate limit adjustment

### Non-Retriable Errors

Errors that require corrective action before retry:
- **Code 1** (`CallerNotOwner`) — Caller must be authorized as owner
- **Code 4** (`EventDoesNotExist`) — Event does not exist; verify correct ID
- **Code 5** (`EventTypeIndexOutOfBounds`) — Index is invalid; use lower index
- **Code 6** (`NewOwnerIsZero`) — Provide valid owner address
- **Code 7** (`CapNotSet`) — Use `set_event_max_logs()` first
- **Code 8** (`MetadataTooLarge`) — Reduce metadata or increase limit
- **Code 9** (`ContractNotInitialized`) — Call `initialize()` first
- **Code 11** (`TimestampOutOfRange`) — Correct timestamp and resubmit
- **Code 12** (`InvalidSignature`) — Re-sign event or verify data
- **Code 15** (`SameOwner`) — Provide different owner address
- **Code 16** (`MaxLogsBelowCurrentCount`) — Increase max or prune events first
- **Code 17** (`CapAlreadyRemoved`) — Cap already lifted; no action needed
- **Code 18** (`CapNeverSet`) — Set cap via `set_event_max_logs()`

### Unrecoverable Errors

Errors indicating system limits or degradation:
- **Code 10** (`TotalEventsOverflow`) — Architectural limit; consider contract migration

## Integration Examples

### Rust

```rust
use soroban_sdk::contracterror;

match result {
    Err(ContractError::GlobalMaxLogsReached) => {
        eprintln!("Error: Log capacity reached. Contact owner.");
        // Implement backoff or fallback
    },
    Err(ContractError::MetadataTooLarge) => {
        eprintln!("Error: Metadata too large. Reduce size to < {} bytes.", max_size);
        // Truncate or compress metadata
    },
    Err(err) => {
        eprintln!("Error: {:?}", err);
        // Generic error handling
    },
    Ok(index) => println!("Event logged at index {}", index),
}
```

### TypeScript

```typescript
const ERROR_CODES: Record<number, string> = {
  1: "CallerNotOwner",
  2: "GlobalMaxLogsReached",
  3: "EventTypeMaxLogsReached",
  // ... etc
};

try {
  await contract.logEvent(event);
} catch (err: any) {
  const code = err.code ?? err.message.match(/(\d+)/)?.[0];
  const name = ERROR_CODES[code] ?? "Unknown";
  console.error(`Contract error ${code}: ${name}`);
  
  if ([2, 3, 13, 14].includes(code)) {
    // Retriable error
    console.log("Retrying in 5s...");
    await new Promise(r => setTimeout(r, 5000));
  }
}
```

## Monitoring & Alerts

Recommended monitoring thresholds:
- **Code 2 & 3 Frequency**: If occurring >5% of requests, alert owner to increase caps
- **Code 13 Duration**: If paused >1 hour, alert operator (maintenance window may be stuck)
- **Code 14 Frequency**: If exceeding rate limit >10% of requests, consider higher limit
- **Code 10 Occurrence**: Any occurrence is critical; immediate escalation required

## See Also

- [API Reference](../README.md#api-reference)
- [Troubleshooting Guide](./troubleshooting.md)
- [Soroban SDK Documentation](https://soroban.stellar.org/docs)
