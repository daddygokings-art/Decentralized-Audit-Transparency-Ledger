# Deployment & Operations Guide

Step-by-step instructions for deploying the AuditLedger contract to Stellar testnet and mainnet.

---

## Prerequisites

| Requirement | Version | Install |
|-------------|---------|---------|
| Rust toolchain | stable | [rustup.rs](https://rustup.rs/) |
| WASM target | — | `rustup target add wasm32-unknown-unknown` |
| Soroban CLI | latest | `cargo install soroban-cli --features opt` |
| Stellar account | — | [Stellar Laboratory](https://laboratory.stellar.org/) |
| Testnet XLM | — | [Friendbot](https://friendbot.stellar.org/) |

Verify your setup:

```bash
soroban --version
rustc --version
cargo --version
```

---

## 1. Building the WASM Binary

```bash
# Optimised release build
cargo build --target wasm32-unknown-unknown --release

# Verify the output exists and check its size
ls -lh target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

A healthy binary is typically under 200 KB. Sizes above 1 MB indicate something is wrong.

---

## 2. Testnet Deployment

### Option A — Deploy Script (Recommended)

```bash
# Export your Stellar secret key (never commit this)
export SOROBAN_SECRET_KEY="S..."

# Run the provided script
./scripts/deploy_testnet.sh
```

The script builds the WASM, validates inputs, and deploys to testnet. The contract ID is printed on success.

### Option B — Manual CLI

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm \
  --source "$SOROBAN_SECRET_KEY" \
  --network testnet \
  --rpc-url https://soroban-testnet.stellar.org
```

Save the contract ID returned; you will need it for all subsequent commands.

```bash
# Optional: store in an env var for convenience
export CONTRACT_ID="C..."
```

### Fund Your Testnet Account

If you need testnet XLM:

```bash
curl "https://friendbot.stellar.org?addr=<your_public_key>"
```

---

## 3. Initialization

The contract must be initialized exactly once. Calling `initialize` again reverts with `AlreadyInitialized`.

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOROBAN_SECRET_KEY" \
  --network testnet \
  -- \
  initialize \
  --owner <owner_public_key> \
  --global_max_logs 100000
```

`global_max_logs` is the hard cap on the total number of events the contract will ever accept. Set it high enough for your use case; it can be increased later by the owner via `set_global_max_logs`.

---

## 4. Verification

Confirm the contract is live and initialized:

```bash
# Should return 0 for a freshly initialized contract
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  -- \
  total_events
```

Log a test event:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOROBAN_SECRET_KEY" \
  --network testnet \
  -- \
  log_event \
  --submitter <submitter_public_key> \
  --event_type "test" \
  --metadata "68656c6c6f"
```

---

## 5. Mainnet Deployment

Mainnet deployment follows the same steps as testnet with these additional considerations.

### Account Funding

Your deployer account must hold enough XLM to cover:
- Base reserve: 1 XLM per account
- Contract storage: ~0.5 XLM per 10 KB of state
- Transaction fees: variable; budget ~0.1 XLM for the deploy + init transactions

Use the [Stellar Expert fee estimator](https://stellar.expert/) or the [Stellar fee reference](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering) for current estimates.

### Deploy to Mainnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm \
  --source "$SOROBAN_SECRET_KEY" \
  --network mainnet \
  --rpc-url https://soroban-mainnet.stellar.org
```

### Initialize on Mainnet

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOROBAN_SECRET_KEY" \
  --network mainnet \
  -- \
  initialize \
  --owner <owner_public_key> \
  --global_max_logs 1000000
```

### Mainnet Checklist

- [ ] Contract has been fully tested on testnet
- [ ] Owner key is a hardware wallet or multi-sig
- [ ] `global_max_logs` is sized for your expected event volume
- [ ] `.env` is not committed to version control
- [ ] Monitoring is configured (see section 8)

---

## 6. Upgrading the Contract

Deploy the new WASM and call `upgrade_contract` from the owner account:

```bash
# 1. Build the new WASM
cargo build --target wasm32-unknown-unknown --release

# 2. Upload the WASM and get the hash
soroban contract install \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm \
  --source "$SOROBAN_SECRET_KEY" \
  --network testnet

# 3. Upgrade the running contract
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOROBAN_SECRET_KEY" \
  --network testnet \
  -- \
  upgrade_contract \
  --caller <owner_public_key> \
  --new_wasm_hash <wasm_hash_from_step_2>
```

The existing contract state (events, config, ownership) is preserved.

See [docs/upgrade-guide.md](upgrade-guide.md) for a full upgrade checklist.

---

## 7. Environment Variables

Copy `.env.example` to `.env` and fill in the required values:

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CONTRACT_ID` | Yes | — | Deployed contract ID |
| `RPC_URL` | No | `https://soroban-testnet.stellar.org` | Soroban RPC endpoint |
| `NETWORK` | No | `testnet` | Network name or passphrase |
| `SCRAPE_INTERVAL_MS` | No | `15000` | Metrics exporter poll interval |
| `EVENT_TYPES` | No | `payment,refund,transfer` | Comma-separated event types to track |
| `GRAFANA_PASSWORD` | No | `admin` | Grafana admin password |

---

## 8. Monitoring

### Stellar Expert

Browse contract transactions and state at:
- Testnet: `https://stellar.expert/explorer/testnet/contract/<CONTRACT_ID>`
- Mainnet: `https://stellar.expert/explorer/public/contract/<CONTRACT_ID>`

### Local Monitoring Stack

```bash
docker compose up --build
```

| Service | URL |
|---------|-----|
| Grafana dashboards | http://localhost:3000 |
| Prometheus metrics | http://localhost:9090 |
| Metrics exporter | http://localhost:9091/metrics |
| UI explorer | http://localhost:3001 |

The Grafana dashboard (`monitoring/grafana/dashboards/audit-ledger.json`) shows total events, per-type counts, and submission rates.

---

## 9. Troubleshooting

### `Error: AlreadyInitialized`

The contract has already been initialized. There is no need to call `initialize` again. Check `total_events` to confirm the contract is live.

### `Error: CallerNotOwner`

The `--source` key does not match the owner stored in the contract. Use the correct owner key or call `transfer_ownership` from the current owner first.

### `Error: GlobalMaxLogsReached`

The event log has hit its cap. The owner must call `set_global_max_logs` with a higher value before new events can be logged.

### `Error: ContractPaused`

The owner has paused the contract. Call `unpause` from the owner account to resume logging.

### `Error: MetadataTooLarge`

The `metadata` bytes exceed the configured limit (default 1 KB). Reduce the payload or ask the owner to call `set_metadata_max_size` with a higher cap.

### `Error: RateLimitExceeded`

The submitter has exceeded their per-ledger rate limit. Wait for the next ledger or ask the owner to adjust the limit via `set_submitter_rate_limit`.

### WASM deploy fails with insufficient fee

Ensure your account has enough XLM. On testnet, use Friendbot to top it up. On mainnet, add XLM via an exchange.

### `soroban: command not found`

Install the CLI: `cargo install soroban-cli --features opt`. Ensure `~/.cargo/bin` is on your `PATH`.

### Build fails: `error[E0463]: can't find crate for 'std'`

You are missing the WASM target. Run: `rustup target add wasm32-unknown-unknown`
