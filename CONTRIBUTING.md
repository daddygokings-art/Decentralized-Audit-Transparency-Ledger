# Contributing to AuditLedger

## Quick Start for Local Development

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- WASM target: `rustup target add wasm32-unknown-unknown`
- Soroban CLI: `cargo install soroban-cli --features opt`
- (Optional) Node.js 20+ for the UI and metrics exporter

### Local Contract Iteration

```bash
# 1. Clone and build
git clone <your-fork>
cd Decentralized-Audit-Transparency-Ledger
cargo build

# 2. Run the full test suite
cargo test

# 3. Check formatting and lint
cargo fmt --check
cargo clippy -- -D warnings

# 4. Build WASM for deployment
cargo build --target wasm32-unknown-unknown --release

# 5. Check WASM size
ls -lh target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

### Running Tests

```bash
# All tests
cargo test

# Single test
cargo test test_log_event

# With output
cargo test -- --nocapture
```

### Local Docker Stack

```bash
# Start all services (metrics exporter, Prometheus, Grafana, UI)
docker compose up --build

# The UI will be available at http://localhost:3001
# Grafana at http://localhost:3000 (admin:password from GRAFANA_PASSWORD)
```

### Deploy to Testnet

See `scripts/deploy_testnet.sh` and the deployment section in `README.md`.

## Code Style

- Follow existing patterns in `src/lib.rs` and `src/test.rs`.
- All public functions must have doc comments.
- Tests are required for new features and should be added to `src/test.rs`.

## Pull Request Process

1. Fork the repository and create a feature branch.
2. Make your changes and ensure all tests pass (`cargo test`).
3. Run `cargo fmt --check` and `cargo clippy -- -D warnings`.
4. Build the WASM binary and verify the size is reasonable.
5. Open a pull request with a clear description of the changes.

## Bounty Program

See `README.md` for details on the bounty point system.
