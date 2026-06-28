# Contributing to Decentralized Audit & Transparency Ledger

Thank you for helping improve the Decentralized Audit Transparency Ledger. These guidelines explain how to set up the project, keep contributions consistent, and move changes through review.

## Code of Conduct

All contributors are expected to follow the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). Be respectful, inclusive, and constructive in issues, pull requests, reviews, and community discussions.

## Getting Started

### Prerequisites

- Rust toolchain, installed with [rustup](https://rustup.rs/)
- WASM target for contract builds:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- Soroban CLI:
  ```bash
  cargo install soroban-cli --features opt
  ```
- Optional: Node.js 20+ for the UI and metrics exporter
- Optional: Docker and Docker Compose for the local stack

### Fork, Clone, and Set Up

1. Fork the repository on GitHub.
2. Clone your fork and enter the project directory:
   ```bash
   git clone https://github.com/<your-username>/Decentralized-Audit-Transparency-Ledger.git
   cd Decentralized-Audit-Transparency-Ledger
   ```
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger.git
   ```
4. Build the project:
   ```bash
   cargo build
   ```
5. Run the baseline checks before making changes:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```

### Local Docker Stack

To run the metrics exporter, Prometheus, Grafana, and UI locally:

```bash
docker compose up --build
```

The UI is available at `http://localhost:3001`, and Grafana is available at `http://localhost:3000`.

### Deploying to Testnet

See `scripts/deploy_testnet.sh` and the deployment section in `README.md` for testnet deployment details.

## Coding Standards

- Run `cargo fmt` before committing Rust changes. Pull requests should pass `cargo fmt --check`.
- Run `cargo clippy -- -D warnings`; clippy warnings should be fixed rather than allowed.
- Follow existing module organization and patterns in `src/`, `services/`, `api/`, and related workspaces.
- Use clear Rust naming conventions:
  - `snake_case` for functions, methods, variables, and modules.
  - `PascalCase` for structs, enums, traits, and type aliases.
  - `SCREAMING_SNAKE_CASE` for constants and statics.
- Keep functions focused and prefer explicit error handling over panics in production code.
- Add doc comments (`///`) for public functions, structs, enums, traits, modules, and non-obvious behavior.
- Use inline comments sparingly to explain intent, invariants, security assumptions, or complex logic. Avoid comments that simply restate the code.
- Do not commit generated build artifacts, local secrets, private keys, or environment-specific configuration.

## Testing Requirements

- Add or update tests for every new function, feature, bug fix, and edge case where behavior changes.
- Run the full Rust test suite before submitting:
  ```bash
  cargo test
  ```
- Useful test commands:
  ```bash
  cargo test test_log_event
  cargo test -- --nocapture
  ```
- The project targets at least 90% test coverage for new and modified code. If coverage cannot be added for a change, explain why in the pull request.
- For contract-facing changes, include tests for success paths, authorization or permission failures, validation failures, and boundary conditions.

## Pull Request Workflow

### Branch Naming

Create a branch from the latest upstream default branch. Use one of these prefixes:

- `feature/<short-description>` for new features.
- `bugfix/<short-description>` for bug fixes.
- `docs/<short-description>` for documentation-only changes.

Example:

```bash
git fetch upstream
git checkout master
git merge upstream/master
git checkout -b docs/contribution-guidelines
```
feat: add rate-limit enforcement per submitter
fix: prevent integer overflow in log_events batch
docs: add deployment guide for mainnet
test: cover zero global_max_logs edge case
```

### PR Checklist

Before opening your pull request, confirm all of the following:

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes (all tests green)
- [ ] New functions have doc comments
- [ ] New behaviour has test coverage
- [ ] The PR description explains *what* changed and *why*
- [ ] No secrets, `.env` files, or generated artefacts are committed

### Review Process

1. Open a pull request against `master` (or the active feature branch).
2. At least **1 maintainer approval** is required before merging.
3. Address all review comments; re-request review once resolved.
4. The maintainer will squash-merge once CI is green and approval is given.
5. Delete your branch after merging.

---

## Issue Tracking & Bounty Program

### Claiming an Issue

1. Find an open issue labelled `bounty` or `good first issue`.
2. Comment "I'd like to work on this" — a maintainer will assign it to you.
3. If you have a new idea, open an issue first and wait for a maintainer to confirm scope before starting work.

### Bounty Points

### Before Opening a PR

Make sure the following checklist is complete:

- [ ] The branch is up to date with the upstream default branch.
- [ ] Code is formatted with `cargo fmt`.
- [ ] `cargo clippy -- -D warnings` passes with zero warnings.
- [ ] `cargo test` passes.
- [ ] New or changed behavior has tests.
- [ ] Public APIs and non-obvious logic are documented.
- [ ] The PR description links the related issue or bounty, if applicable.

### PR Description Template

Use a clear pull request description that includes:

```markdown
## Summary
- What changed?

## Testing
- What commands did you run?

## Related Issues
- Closes #<issue-number>
```

### Review Process

- At least one maintainer approval is required before merge.
- Address review feedback with additional commits or a small follow-up commit series.
- Keep discussions constructive and explain trade-offs when accepting or declining suggestions.
- Maintainers may request additional tests, documentation, or security notes before approving.
- Prefer squash merging unless maintainers request a different merge strategy.

## Issue Tracking and Bounties

- Search existing issues before opening a new one.
- Use clear titles and include reproduction steps, expected behavior, actual behavior, logs, and environment details for bugs.
- To work on an issue, comment that you would like to claim it and wait for maintainer confirmation when the issue requires assignment.
- For bounty issues, follow the bounty provider's instructions and include the issue number in your branch, commits, and pull request.
- Bounty points are awarded based on the issue requirements, implementation quality, tests, documentation, review responsiveness, and maintainer acceptance.
- If you can no longer work on a claimed issue, comment promptly so another contributor can pick it up.
