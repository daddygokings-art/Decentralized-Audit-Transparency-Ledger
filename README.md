# Decentralized Audit & Transparency Ledger

A Soroban smart contract for immutably logging financial transactions on the Stellar network, providing a publicly verifiable audit trail. Built with the [Soroban SDK](https://soroban.stellar.org/).

This repository also includes ongoing work for retention policies, event export workflows, schema validation, event chaining support, and **supply chain transparency**.

## Overview

`AuditLedger` acts as an append-only log for financial and operational events. Each entry is sealed with a timestamp, event type (`Symbol`), and submitter address (`Address`), producing a tamper-evident historical record that any party can independently verify. Configurable global and per-event logging limits prevent state bloat while maintaining a complete, ordered history.

Additionally, the **Supply Chain Transparency Module** provides immutable tracking of products through their entire lifecycle with support for:
- Product provenance and origin tracking
- Third-party certifications (ISO, organic, fair trade, etc.)
- Labor conditions and worker welfare audits
- Environmental impact metrics (carbon, water, waste, energy)
- Complete chain of custody
- Consumer-facing verification and QR codes
- Brand integrity reporting

## Core Features

### Audit Ledger
- **Immutable Event Logging** — Every event is recorded on-chain with a standardized `Event` struct, creating a permanent audit trail.
- **Configurable Logging Limits** — Separate global and per-event-type caps (`u32`) prevent contract state spam. Caps can be set to any value (including `0` to freeze logging) or removed entirely via `remove_event_cap`.
- **Public Verifiability** — Anyone can enumerate and verify the full log history or filter by event type — no trusted intermediary required.
- **Metadata Standardization** — Events carry opaque `Bytes` metadata, encouraging off-chain consumers to adopt a consistent schema.
- **Boundary-Safe Validation** — Contract logic is hardened against edge cases: zero-maximum configurations, equal min/max value ranges, empty metadata, and cap removal.

### Supply Chain Transparency
- **Provenance Tracking** — Immutably record product origin, raw materials source, and batch information
- **Certification Management** — Track ISO, organic, fair trade, and other third-party certifications with expiry tracking
- **Labor Conditions Auditing** — Record worker welfare, wage compliance, safety standards, and freedom of association
- **Environmental Impact** — Track carbon footprint, water usage, waste, renewable energy, and emissions reduction
- **Chain of Custody** — Complete ownership and transfer history with locations and timestamps
- **Consumer Verification** — QR codes and timeline views for end-consumers to verify product authenticity
- **Brand Integrity Reports** — Aggregate compliance metrics and transparency scores for brands

## Smart Contract Architecture

| Component | Description |
|-----------|-------------|
| **Global Log Registry** | Sequential array of all events, capped by `global_max_logs`. |
| **Per-Event Sub-Ledgers** | Namespaced event types (`Symbol`), each with an optional independent maximum log limit. |
| **Cap Gates** | `event_cap_set` boolean gates per-event enforcement — caps are opt-in and can be removed via `remove_event_cap`. |
| **Supply Chain Registry** | Brand, product, and event tracking with multi-dimensional verification |

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
fn log_events(env: Env, events: Vec<(Address, Symbol, Bytes)>) -> Vec<u32>;
```

### Read

```rust
fn total_events(env: Env) -> u32;
fn get_event(env: Env, id: BytesN<32>) -> Event;
fn event_count(env: Env, event_type: Symbol) -> u32;
fn get_event_by_type(env: Env, event_type: Symbol, type_index: u32) -> Event;
```

### Governance (Owner Only)

```rust
fn set_global_max_logs(env: Env, caller: Address, new_max: u32);
fn set_event_max_logs(env: Env, caller: Address, event_type: Symbol, new_max: u32);
fn remove_event_cap(env: Env, caller: Address, event_type: Symbol);
fn transfer_ownership(env: Env, caller: Address, new_owner: Address);
fn set_event_ttl(env: Env, caller: Address, ttl_ledgers: u32);
fn get_event_ttl(env: Env) -> u32;
```

All governance functions publish a typed Soroban event with topic `("governance", "<function_name>")` and payload `(caller, old_value, new_value)` so off-chain monitors can track admin activity without polling state.

#### TTL Storage

`set_event_ttl(ttl_ledgers)` enables optional persistent storage for events. When `ttl_ledgers > 0`, each `log_event` call additionally writes the event to `env.storage().persistent()` and extends its TTL to `ttl_ledgers` ledgers, making events eligible for network expiry after that point. See [`docs/fees.md#ttl-storage`](docs/fees.md#ttl-storage) for cost tradeoffs.

## Architecture

For a high-level view of how the contract, SDKs, APIs, bridge, monitoring, and UI fit together, see [docs/architecture.md](docs/architecture.md).

For backup, recovery, failover, and DR testing procedures, see [docs/disaster-recovery.md](docs/disaster-recovery.md).

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
- WASM target: `rustup target add wasm32-unknown-unknown`
- Soroban CLI: `cargo install soroban-cli --features opt`
- Docker & Docker Compose (for local infrastructure)
- Node.js 20+ (for UI and metrics exporter)

### Local Contract Iteration

The fastest way to iterate on the contract locally:

```bash
# 1. Build and test in one cycle
cargo build && cargo test

# 2. Run a single test to narrow down issues
cargo test test_log_event

# 3. Format and lint before committing
cargo fmt --check && cargo clippy -- -D warnings

# 4. Build the WASM binary for size checks
cargo build --target wasm32-unknown-unknown --release
ls -lh target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

### Build for WASM

```bash
cargo build --target wasm32-unknown-unknown --release
```

### Deploy to Testnet

**Using the deploy script (recommended):**

```bash
# Set your secret key (never commit this)
export SOROBAN_SECRET_KEY="<your_secret_key>"

# Run the deployment script
./scripts/deploy_testnet.sh
```

The script validates required environment variables, builds the WASM binary, and deploys it to Stellar testnet. See `scripts/deploy_testnet.sh` for details.

**Using the Soroban CLI directly:**

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm \
  --source <secret_key> \
  --network testnet
```

### Initialize

The contract must only be initialized once. Repeated calls to `initialize()` will revert with `AlreadyInitialized`.

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

### Local Docker Stack

Run the full monitoring and UI stack locally:

```bash
# Copy and configure environment variables
cp .env.example .env

# Start all services
docker compose up --build
```

- UI: http://localhost:3001
- Grafana: http://localhost:3000
- Prometheus metrics: http://localhost:9090

### Environment Variables

Copy `.env.example` to `.env` and configure the required variables:

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CONTRACT_ID` | Yes | — | Deployed contract ID |
| `RPC_URL` | No | `https://soroban-testnet.stellar.org` | Soroban RPC endpoint |
| `NETWORK` | No | `testnet` | Stellar network passphrase |
| `SCRAPE_INTERVAL_MS` | No | `15000` | Metrics exporter poll interval |
| `EVENT_TYPES` | No | `payment,refund,transfer` | Event types to track |
| `GRAFANA_PASSWORD` | No | `admin` | Grafana admin password |

## Test Coverage (22 tests)

| Test File | Count | Description |
|-----------|-------|-------------|
| `src/test.rs` | 22 | Logging, queries, governance, ownership transfers, cap management, event emission, empty metadata, access control, boundary conditions |

Boundary tests include: zero global/event max logs, setting max equal to current count, removing caps after zero-lock, mixed multi-type limits, and panic-on-nonexistent access.

## Supply Chain Module Documentation

For detailed information about the supply chain transparency features, see [docs/SUPPLY_CHAIN.md](docs/SUPPLY_CHAIN.md).

The supply chain module includes:
- **Product Provenance Tracking** — Record origin, batch numbers, and raw material sources
- **Certification Management** — Track ISO, organic, fair trade, and custom certifications
- **Labor Conditions Audits** — Verify worker welfare, wages, and safety standards
- **Environmental Impact** — Monitor carbon footprint, water usage, waste, and renewable energy
- **Chain of Custody** — Complete transfer history from producer to consumer
- **Consumer Verification** — QR codes and product timelines for end-consumers
- **Brand Integrity Reporting** — Compliance scores and transparency metrics

[Read the Supply Chain Implementation Guide](SUPPLY_CHAIN_IMPLEMENTATION.md) for technical details.

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
6. Scans Rust dependencies for known vulnerabilities with `cargo audit --deny warnings` (checks [RustSec Advisory Database](https://rustsec.org/))

## Security

This project follows security best practices:
- **Dependency Vulnerability Scanning**: All transitive and direct dependencies are scanned via the [RustSec Advisory Database](https://rustsec.org/) on every CI run.
- **Boundary Validation**: Contract logic validates all edge cases and boundary conditions.
- **Immutable Audit Trail**: Events are cryptographically chained to prevent tampering.

## License


[MIT](LICENSE)
