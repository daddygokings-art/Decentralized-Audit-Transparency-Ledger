# Decentralized Audit & Transparency Ledger

A Soroban smart contract for immutably logging financial transactions on the Stellar network, providing a publicly verifiable audit trail. Built with the [Soroban SDK](https://soroban.stellar.org/).

This repository also includes ongoing work for retention policies, event export workflows, schema validation, and event chaining support.

## Overview

`AuditLedger` acts as an append-only log for financial and operational events. Each entry is sealed with a timestamp, event type (`Symbol`), and submitter address (`Address`), producing a tamper-evident historical record that any party can independently verify. Configurable global and per-event logging limits prevent state bloat while maintaining a complete, ordered history.

## Core Features

- **Immutable Event Logging** — Every event is recorded on-chain with a standardized `Event` struct, creating a permanent audit trail.
- **Configurable Logging Limits** — Separate global and per-event-type caps (`u32`) prevent contract state spam. Caps can be set to any value (including `0` to freeze logging) or removed entirely via `remove_event_cap`.
- **Public Verifiability** — Anyone can enumerate and verify the full log history or filter by event type — no trusted intermediary required.
- **Metadata Standardization** — Events carry opaque `Bytes` metadata, encouraging off-chain consumers to adopt a consistent schema.
- **Boundary-Safe Validation** — Contract logic is hardened against edge cases: zero-maximum configurations, equal min/max value ranges, empty metadata, and cap removal.
- **Stablecoin Reserve Auditing** — New module for tracking reserve assets, collecting third-party attestations, generating transparency reports, testing redemptions, executing stress tests, and verifying zero-knowledge proofs of reserves.

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

## Stablecoin Reserve Auditing

The system now includes a comprehensive reserve auditing module for stablecoins with:

- **Asset Verification** — Register and track reserve assets (USD cash, Treasury bills, bank deposits, cryptocurrency)
- **Third-Party Attestations** — Record auditor attestations with digital signatures
- **Transparency Reports** — Generate periodic public audit reports with Merkle root verification
- **Redemption Testing** — Simulate and validate redemption workflows
- **Stress Testing** — Execute and record stress test scenarios with recovery procedures
- **Zero-Knowledge Proofs** — Support for range proofs, Merkle proofs, commitment proofs, and aggregated proofs

### Reserve Auditing Quick Start

```rust
// Register an asset
let asset_id = ReserveAuditingContract::register_asset(
    env, AssetType::USDCash, 1_000_000_000, custody_address, proof_hash)?;

// Record attestation
let attestation_id = ReserveAuditingContract::record_attestation(
    env, attestor, asset_id, 1_000_000_000, signature, public_key, expires_at)?;

// Generate report
let report_id = ReserveAuditingContract::generate_report(
    env, period_start, period_end, breakdown_hash, attestations_hash, merkle_root)?;

// Verify ZK proof
let proof_id = ReserveAuditingContract::verify_zk_proof(
    env, ZkProofType::RangeProof, proof_data, commitment, expires_at)?;
```

See [docs/STABLECOIN_RESERVE_AUDITING.md](docs/STABLECOIN_RESERVE_AUDITING.md) for complete API reference and [docs/STABLECOIN_RESERVE_QUICK_START.md](docs/STABLECOIN_RESERVE_QUICK_START.md) for workflow examples.

## DeFi Protocol Auditing

The system now includes comprehensive DeFi protocol auditing with real-time monitoring:

- **TVL Tracking** — Monitor total value locked across pools and protocols
- **Oracle Verification** — Validate oracle prices and detect price anomalies
- **Liquidation Monitoring** — Track liquidation events and at-risk positions
- **Governance Tracking** — Monitor governance proposals and voting activity
- **Risk Metrics** — Calculate concentration risk, volatility, and protocol health
- **Automated Audit Reports** — Generate periodic comprehensive audit reports with metrics and findings

### DeFi Auditing Quick Start

```rust
// Register protocol
DeFiAuditingContract::register_protocol(
    env, protocol_address, Symbol::new(&env, "aave"), ProtocolType::Lending,
    Symbol::new(&env, "ethereum"), None)?;

// Update pool TVL
DeFiAuditingContract::update_pool_tvl(
    env, pool_id, protocol, Symbol::new(&env, "DAI"), tvl_usd, tvl_native, lp_count)?;

// Record oracle prices
DeFiAuditingContract::record_oracle_price(
    env, oracle_id, asset, price_usd, Symbol::new(&env, "chainlink"),
    confidence, update_frequency)?;

// Track liquidations
DeFiAuditingContract::record_liquidation(
    env, protocol, position, liquidator, collateral_asset, debt_asset,
    collateral_amount, debt_amount, liquidation_price)?;

// Monitor governance
let proposal_id = DeFiAuditingContract::create_proposal(
    env, protocol, title, description, proposer, start_time, end_time)?;

// Calculate risk metrics
let metrics_id = DeFiAuditingContract::calculate_risk_metrics(env, protocol)?;

// Generate audit report
let report_id = DeFiAuditingContract::generate_audit_report(
    env, protocol, period_start, period_end, findings_hash)?;
```

See [docs/DEFI_PROTOCOL_AUDITING.md](docs/DEFI_PROTOCOL_AUDITING.md) for complete API reference and [docs/DEFI_PROTOCOL_QUICK_START.md](docs/DEFI_PROTOCOL_QUICK_START.md) for detailed workflow examples.

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
