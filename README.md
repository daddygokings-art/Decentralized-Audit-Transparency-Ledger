# Decentralized Audit & Transparency Ledger

A Soroban smart contract for immutably logging financial transactions on the Stellar network, providing a publicly verifiable audit trail. Built with the [Soroban SDK](https://soroban.stellar.org/).

This repository also includes ongoing work for retention policies, event export workflows, schema validation, event chaining support, and **supply chain transparency**.

## Security Headers (ZAP Baseline)

The web frontend is scanned by OWASP ZAP as part of CI (see the ZAP baseline report issue). To keep the baseline clean, the app's HTTP responses must set the following headers consistently on the root path, `/favicon.ico`, `/robots.txt`, and `/sitemap.xml`:

- **Content-Security-Policy** — must explicitly define directives that have no fallback, at minimum:
  - `object-src 'none'`
  - `base-uri 'self'`
  - `frame-ancestors 'none'`
  - `form-action 'self'`

  This resolves ZAP rule **10055 (CSP: Failure to Define Directive with No Fallback)**.
- **Cache-Control** — dynamic/HTML responses should use `no-store` (or `no-cache, must-revalidate`), while static assets (`favicon.ico`, `robots.txt`, `sitemap.xml`) should set an explicit cache policy (e.g. `public, max-age=86400`). This resolves ZAP rule **10049 (Non-Storable Content)**.

Apply these via the app's existing header/middleware configuration so every route above is covered.

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
- **Stablecoin Reserve Auditing** — New module for tracking reserve assets, collecting third-party attestations, generating transparency reports, testing redemptions, executing stress tests, and verifying zero-knowledge proofs of reserves.

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
fn get_total_events_at_ledger(env: Env, ledger: u32) -> u32;
fn get_event_at_ledger(env: Env, event_id: BytesN<32>, ledger: u32) -> Event;
fn get_event_by_type_at_ledger(env: Env, event_type: Symbol, type_index: u32, ledger: u32) -> Event;
```

Historical queries return the latest contract-maintained event snapshot at or before the requested ledger. Event additions and updates are indexed by ledger, event ID, and event type; ledgers before the first recorded snapshot return a total of zero and have no events.

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
- **Oracle Verification** — Validate oracle price feeds and detect manipulation
- **Liquidity Monitoring** — Track pool depth, slippage, and impermanent loss
- **Risk Scoring** — Aggregate protocol risk from multiple dimensions
- **Incident Response** — Record and track security incidents with remediation

See [docs/DEFI_AUDITING.md](docs/DEFI_AUDITING.md) for the full API reference.

## Development

```bash
# Build the contract
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test
```

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
