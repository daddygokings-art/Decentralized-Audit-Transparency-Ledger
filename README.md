# Decentralized Audit & Transparency Ledger

A Soroban smart contract for immutably logging financial transactions on the Stellar network, providing a publicly verifiable audit trail. Built with the [Soroban SDK](https://soroban.stellar.org/).

## Overview

`AuditLedger` acts as an append-only log for financial and operational events. Each entry is sealed with a timestamp, event type (`Symbol`), and submitter address (`Address`), producing a tamper-evident historical record that any party can independently verify. Configurable global and per-event logging limits prevent state bloat while maintaining a complete, ordered history.

## Core Features

- **Immutable Event Logging** — Every event is recorded on-chain with a standardized `Event` struct, creating a permanent audit trail.
- **Configurable Logging Limits** — Separate global and per-event-type caps (`u32`) prevent contract state spam. Caps can be set to any value (including `0` to freeze logging) or removed entirely via `remove_event_cap`.
- **Public Verifiability** — Anyone can enumerate and verify the full log history or filter by event type — no trusted intermediary required.
- **Metadata Standardization** — Events carry opaque `Bytes` metadata, encouraging off-chain consumers to adopt a consistent schema.
- **Boundary-Safe Validation** — Contract logic is hardened against edge cases: zero-maximum configurations, equal min/max value ranges, empty metadata, and cap removal.

## Smart Contract Architecture

| Component | Description |
|-----------|-------------|
| **Global Log Registry** | Sequential array of all events, capped by `global_max_logs`. |
| **Per-Event Sub-Ledgers** | Namespaced event types (`Symbol`), each with an optional independent maximum log limit. |
| **Cap Gates** | `event_cap_set` boolean gates per-event enforcement — caps are opt-in and can be removed via `remove_event_cap`. |

### Event Structure

```rust
pub struct Event {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
}
```

## API Reference

### Write

```rust
fn initialize(env: Env, owner: Address, global_max_logs: u32);
fn log_event(env: Env, submitter: Address, event_type: Symbol, metadata: Bytes) -> u32;
```

### Read

```rust
fn total_events(env: Env) -> u32;
fn get_event(env: Env, index: u32) -> Event;
fn event_count(env: Env, event_type: Symbol) -> u32;
fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event;
```

### Governance (Owner Only)

```rust
fn set_global_max_logs(env: Env, caller: Address, new_max: u32);
fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32);
fn remove_event_cap(env: Env, caller: Address, event_type: Symbol);
fn transfer_ownership(env: Env, caller: Address, new_owner: Address);
```

## Quick Start

```bash
# Build
cargo build

# Run all tests
cargo test

# Format
cargo fmt

# Lint
cargo clippy
```

## Development

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- Soroban SDK (included as a Cargo dependency)

### Build for WASM

```bash
cargo build --target wasm32-unknown-unknown --release
```

### Deploy with Soroban CLI

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm \
  --source <secret_key> \
  --network testnet
```

### Initialize

```bash
soroban contract invoke \
  --id <contract_id> \
  --source <owner_secret> \
  --network testnet \
  -- \
  initialize \
  --owner <owner_address> \
  --global_max_logs 100000
```

## Test Coverage (22 tests)

| Test File | Count | Description |
|-----------|-------|-------------|
| `src/test.rs` | 22 | Logging, queries, governance, ownership transfers, cap management, event emission, empty metadata, access control, boundary conditions |

Boundary tests include: zero global/event max logs, setting max equal to current count, removing caps after zero-lock, mixed multi-type limits, and panic-on-nonexistent access.

## Contributing & Bounty Program

Contributions are organized into **Wave Issues** with point values:

| Difficulty | Points | Example Task |
|------------|--------|--------------|
| High | 200 | Implement global vs. per-event logging limits to prevent contract state spam. |
| Medium | 150 | Write edge-case tests validating boundary conditions (e.g., zero maximum logs, equal min/max values). |
| Trivial | 100 | Standardize the metadata structure for all logged events. |

1. Claim an issue or submit a proposal.
2. Fork the repo and implement the feature/fix.
3. Open a pull request with tests and documentation.
4. Earn points redeemable for rewards.

## CI/CD

Every push and pull request triggers a GitHub Actions workflow that:
1. Installs the Rust toolchain via `dtolnay/rust-toolchain`
2. Checks formatting with `cargo fmt --check`
3. Lints with `cargo clippy`
4. Builds with `cargo build`
5. Runs the full test suite with `cargo test`

## License


[MIT](LICENSE)
