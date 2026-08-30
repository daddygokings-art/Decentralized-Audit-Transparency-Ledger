# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Commit messages follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
specification. Changelogs for each release are generated automatically via
[git-cliff](https://git-cliff.org/) using `cliff.toml`.

---

## [Unreleased]

### Added
- Kubernetes runtime security monitoring with Falco (`infra/k8s/falco/`).
- Custom Falco rules for contract-specific threats (`monitoring/falco/rules.d/audit-ledger-workloads.yaml`).
- Falco PrometheusRule alerts for privilege escalation, crypto mining, container escape, reverse shell, and signing-key tampering.
- Alertmanager routing and in-cluster incident-response webhook integration.
- Automated changelog generation via git-cliff (`cliff.toml`).
- Release workflow (`.github/workflows/release.yml`) that creates GitHub Releases on `v*` tags.
- Release notes template (`docs/release-notes-template.md`).
- Migration guide template (`docs/migrations/MIGRATION-TEMPLATE.md`).

---

## [0.1.0] — 2025-01-01

### Added
- `AuditLedger` Soroban smart contract: append-only, tamper-evident event log on the Stellar network.
- Core `Event` struct with `index`, `timestamp`, `event_type`, `submitter`, and opaque `Bytes` metadata.
- `initialize(owner, global_max_logs)` — single-call contract initialization guarded by `AlreadyInitialized`.
- `log_event(submitter, event_type, metadata) -> u32` — log a single event; returns its global index.
- `log_events(events) -> Vec<u32>` — batch variant for atomic multi-event writes.
- `total_events()`, `get_event(id)`, `event_count(event_type)`, `get_event_by_type(event_type, type_index)` — read-only query API.
- Global log cap (`global_max_logs`) enforced on every write.
- Per-event-type log caps (`set_event_max_logs`, `remove_event_cap`) with opt-in `event_cap_set` gate.
- Governance functions: `set_global_max_logs`, `set_event_max_logs`, `remove_event_cap`, `transfer_ownership`.
- Optional TTL-based persistent storage: `set_event_ttl`, `get_event_ttl` — events written to `env.storage().persistent()` when `ttl_ledgers > 0`.
- Governance Soroban events emitted with topic `("governance", "<function_name>")` for off-chain monitoring.
- 22 unit tests covering logging, queries, governance, ownership transfers, cap management, event emission, empty metadata, access control, and boundary conditions.
- Boundary-condition tests: zero max logs, max equal to current count, remove caps after zero-lock, mixed multi-type limits, panic on non-existent access.
- Property-based tests via `proptest`.
- Fuzz harness (`src/fuzz.rs`).
- REST API server (`api/rest/`).
- GraphQL API server (`api/graphql/`).
- WebSocket API (`api/ws/`).
- JavaScript/TypeScript SDK (`sdk/js/`).
- Python SDK (`sdk/python/`) with PyPI publish workflow.
- EVM bridge and Solidity verifier contract (`bridge/evm/Verifier.sol`, `bridge/relayer/`).
- Metrics exporter (`tools/metrics-exporter/`) with Prometheus scrape target.
- Grafana dashboard and Prometheus alert rules (`monitoring/`).
- Next.js UI (`ui/`).
- Notifier service (`services/notifier/`).
- Docker Compose stack for the full local development environment.
- Deployment script `scripts/deploy_testnet.sh`.
- Benchmark script `scripts/benchmark.sh`.
- Off-chain backup and restore scripts (`tools/backup/`).
- Architecture documentation (`docs/architecture.md`).
- API reference (`docs/api.md`, `docs/cli.md`).
- Contract upgrade guide (`docs/upgrade-guide.md`).
- ADRs 001–005 covering append-only log, logging limits, owner governance, storage-key design, and event emission.
- CI pipeline: formatting, Clippy, build, test, `cargo audit`, ShellCheck, WASM size regression check (128 KB ceiling).
- Reproducible-build workflow (`.github/workflows/reproducible-build.yml`).
- Dependency review workflow (`.github/workflows/dependency-review.yml`).

---

<!-- Releases are linked at the bottom; update these URLs when the repository is public. -->
[Unreleased]: https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger/releases/tag/v0.1.0
