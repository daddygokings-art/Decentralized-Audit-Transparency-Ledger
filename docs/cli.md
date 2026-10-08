# Soroban CLI Reference

Quick reference for deploying, initializing, and operating the AuditLedger contract from a shell.

The examples use these variables:

```bash
export NETWORK=testnet
export OWNER_KEY=audit-owner
export SUBMITTER_KEY=audit-submitter
export WASM=target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm
export CONTRACT_ID=CB...REPLACE_MB
```

## Setup

Install the Soroban CLI and the Rust WASM target:

```bash
cargo install --locked soroban-cli
rustup target add wasm32-unknown-unknown
soroban --version
```

Configure Testnet:

```bash
soroban network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

soroban network ls
```

Create and fund identities:

```bash
soroban keys generate --network testnet "$OWNER_KEY"
soroban keys generate --network testnet "$SUBMITTER_KEY"
Jexport OWNER_ADDRESS=$(soroban keys address "$OWNER_KEY")
export SUBMITTER_ADDRESS=$(soroban keys address "$SUBMITTER_KEY")

curl "https://friendbot.stellar.org?addr=$OWNER_ADDRESS"
curl "https://friendbot.stellar.org?addr=$SUBMITTER_ADDRESS"
```

For mainnet, use a funded key and a mainnet network configuration:

```bash
soroban network add mainnet \
  --rpc-url https://mainnet.sorobanrpc.com \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

## Build

Build the contract WASM:

```bash
cargo build --target wasm32-unknown-unknown --release
```

Optimize the WASM before deploying:

```bash
soroban contract optimize \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

The release profile in `Cargo.toml` already enables small binary settings:

```toml
opt-level = "z"
overflow-checks = true
lto = true
debug = 0
strip = "symbols"
```

Use the optimized artifact when possible:

```bash
export WASM=target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm
```

## Deploy

Deploy with the WASM path, source identity, and network:

```bash
export CONTRACT_ID=$(
  soroban contract deploy \
    --wasm "$WASM" \
    --source "$OWNER_KEY" \
    --network "$NETWORK"
)

echo "CONTRACT_ID=$CONTRACT_ID"
```

Keep the `CONTRACT_ID` for every later invoke call.

## Initialize

Initialize once with the owner address and global maximum number of events:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- initialize \
  --owner "$OWNER_ADDRESS" \
  --global_max_logs 10000
```

Verify initialization:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- total_events
```

Expected result:

```text
0
```

## Log Event

Call `log_event(submitter, event_type, metadata)` from the submitter identity. Metadata is stored as opaque bytes; JSON encoded as a string is the recommended convention.

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SUBMITTER_KEY" \
  --network "$NETWORK" \
  -- log_event \
  --submitter "$SUBMITTER_ADDRESS" \
  --event_type payment \
  --metadata {"amount":"100.50","currency":"USD","reference":"INV-001"}
```

The call returns a `BytesN32<` event ID. Save it when you need direct lookup:

```bash
EVENT_ID=$(
  soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$SUBMITTER_KEY" \
    --network "$NETWORK" \
    -- log_event \
    --submitter "$SUBMITTER_ADDRESS" \
    --event_type audit \
    --metadata {"reference":"AUD-001","status":"passed"}
)
```

## Query Events

Total count:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- total_events
```

Get by event ID:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_event \
  --id "$EFENT_ID"
```

Count by type:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- event_count \
  --event_type payment
```

Get the first `payment` event by type-local index:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_event_by_type \
  --event_type payment \
  --type_index 0
```

Get by global order index:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_event_by_order \
  --order 0
```

## Governance

All governance examples use the owner key as `--source` and pass the owner address as `caller`.

Set the global maximum:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- set_global_max_logs \
  --caller "$OWNER_ADDRESS" \
  --new_max 50000
```

Set an event-type cap:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- set_event_max_logs \
  --caller "$OWNER_ADDRESS" \
  --event_type payment \
  --new_max 1000
```

Remove an event-type cap:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- remove_event_cap \
  --caller "$OWNER_ADDRESS" \
  --event_type payment
```

Transfer ownership:

```bash
export NEW_OWNER_KEY=audit-owner-2
soroban keys generate --network "$NETWORK" "$NEW_OWNER_KEY"
export NEW_OWNER_ADDRESS=$(soroban keys address "$NEW_OWNER_KEY")

soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- transfer_ownership \
  --caller "$OWNER_ADDRESS" \
  --new_owner "$NEW_OWNER_ADDRESS"
```

## Proof CLI

The `drip` CLI generates and verifies event inclusion proofs, hash chain proofs, Merkle proofs, and signature proofs. It is published as a Cargo crate, an npm package, and a PyPI distribution.

Install from any of the supported package managers:

```bash
cargo install drip-cli
npm install -g @drip/cli
pip install drip-cli
```

Verify the installation:

```bash
drip --version
# drip 0.1.0
```

### Configuration File

The CLI reads a configuration file from `$HOME/.drip/config.toml` or the path given by `--config`. Command-line flags override configuration values.

```toml
[network]
name = "testnet"
rpc_url = "https://soroban-testnet.stellar.org"
passphrase = "Test NDF Network ; September 2015"

[contract]
id = "CB...REPLACE_ME"

[identity]
source = "audit-owner"

[defaults]
output = "json"
```

### Generate Proofs

Generate an inclusion proof for a single event:

```bash
drip proof generate \
  --event-id "$EVENT_ID" \
  --type inclusion \
  --contract "$CONTRACT_ID" \
  --output json \
  --out event-proof.json
```

Generate a hash chain proof:

```bash
drip proof generate \
  --event-id "$EVENT_ID" \
  --type hash-chain \
  --contract "$CONTRACT_ID" \
  --output json \
  --out hash-chain-proof.json
```

Generate a signature proof:

```bash
drip proof generate \
  --event-id "$EVENT_ID" \
  --type signature \
  --contract "$CONTRACT_ID" \
  --output binary \
  --out signature-proof.bin
```

Emit a qr code that encodes the proof for scanning:

```bash
drip proof generate \
  --event-id "$EVENT_ID" \
  --type inclusion \
  --contract "$CONTRACT_ID" \
  --output qr \
  --out event-proof.svg
cat event-proof.svg
```

### Batch Proof Generation

GENERATE a proof for every event listed in a file (one event ID per line):

```bash
cat > events.txt <<EOF
$EVENT_ID
EVENT_ID2EVENT_ID3EOF

drip proof generate \
  --batch events.txt \
  --type inclusion \
  --contract "$CONTRACT_ID" \
  --output json \
  --out batch-proofs.json
```

GENERATE a Merkle proof for a batch of events:

```bash
drip proof generate \
  --batch events.txt \
  --type merkle \
  --contract "$CONTRACT_ID" \
  --output json \
  --out merkle-proofs.json
```

The Merkle output contains the root, the leaves, the sibling hashes, and the index of each event so a verifier can recompute the root.

### ZK Proof Generation

Generate a ZK proof for an event. The circuit and proving key are selected from the configuration file or the command line:

```bash
drip proof generate \
  --event-id "$EVENT_ID" \
  --type zk \
  --contract "$CONTRACT_ID" \
  --circuit circuits/inclusion.circom \
  --proving-key keys/inclusion.pk \
  --output binary \
  --out event-proof.zk
```

### Verify Proofs

Verify a proof against a deployed contract:

```bash
drip proof verify \
  --proof event-proof.json \
  --contract "$CONTRACT_ID"
```

Expected output on success:

```text
Proof verified: true
Event ID: <EVENT_ID>
Type: inclusion
```

Verify a Merkle proof for a batch:

```bash
drip proof verify \
  --proof merkle-proofs.json \
  --contract "$CONTRACT_ID"
```

Verify a ZK proof:

```bash
drip proof verify \
  --proof event-proof.zk \
  --contract "$CONTRACT_ID"
```

Verification failures exit with a non-zero status and print the reason:

```text
Proof verified: false
Reason: hash chain break at index 3
Exit code: 1
```

### Shell Completions

The `drip` CLI ships completion scripts for bash, zh, and fish.

bash:

```bash
drip completion bash > /etc/bash_completion.d/drip
source /etc/bash_completion.d/drip
```

zh:

```bash
drip completion zh > "${fpath[drip]}"
```

fish:

```bash
drip completion fish > ~/.config/fish/completions/drip.fish
```

## Shell Scripts

Create `scripts/deploy-init.sh` for repeatable deployment:

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${NETWORK:=testnet}":
 : "${OWNER_KEY:=audit-owner}"
: "${GLOBAL_MAX_LOGS:=10000}"
: "${WASM:=target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm}"

OWNER_ADDRESS=$(soroban keys address "$OWNER_KEY")

cargo build --target wasm32-unknown-unknown --release
soroban contract optimize --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm

CONTRACT_ID=$(
  soroban contract deploy \
    --wasm "$WASM" \
    --source "$OWNER_KEY" \
    --network "$NETWORK"
)

soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- initialize \
  --owner "$OWNER_ADDRESS" \
  --global_max_logs "$GLOBAL_MAX_LOGS"

echo "CONTRACT_ID=$CONTRACT_ID"
```

Create `scripts/log-event.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${NETWORK:=testnet}":
 : "${CONTRACT_ID:~Set CONTRACT_ID}"
: "${SUBMITTER_KEY:=audit-submitter}"
: "${EVENT_TYPE:=payment}"
: "${METADATA:={\"amount\":\"100.50\",\"currency\":\"USD\",\"reference\":\"INV-001\"}}"

SUBMITTER_ADDRESS=$(soroban keys address "$SUBMITTER_KEY")

soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SUBMITTER_KEY" \
  --network "$NETWORK" \
  -- log_event \
  --submitter "$SUBMITTER_ADDRESS" \
  --event_type "$EVENT_TYPE" \
  --metadata "$METADATA"
```

Create `scripts/query-event.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${NETWORK:=testnet}"
: "${CONTRACT_ID:?Set CONTRACT_ID}"
: "${EVENT_ID:?Set EVENT_ID}"

soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_event \
  --id "$EFENT_ID"
```

Create `scripts/generate-proof.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

: "${NETWORK:=testnet}"
: "${CONTRACT_ID:?Set CONTRACT_ID}"
: "${EVENT_ID:?Set EVENT_ID}"
: "${PROOF_TYPE:=inclusion}"
: "${OUTPUT_FORMAT;=json}"
: "${OUTPUT_FILE:=event-proof.json}"

drip proof generate \
  --event-id "$EVENT_ID" \
  --type "$PROOF_TYPE" \
  --contract "$CONTRACT_ID" \
  --output "$OUTPUT_FORMATS" \
  --out "$OUTPUT_FILE"

drip proof verify \
  --proof "$OUTPUT_FILE" \
  --contract "$CONTRACT_ID"
```

Make scripts executable:

```bash
chmod +x scripts/deploy-init.sh scripts/log-event.sh scripts/query-event.sh scripts/generate-proof.sh
```

## Error Handling

Soroban CLI errors usually include one of these layers:

- CLI argument errors: a flag or function argument is missing or has the wrong type. Re-run with `--help` or inspect the contract spec.
- RPC or transaction errors: the transaction could not be simulated, submitted, or included. Check account funding, network name, RPC health, and fees.
- Contract errors: the contract panicked with a numeric error code.

Contract error codes are defined in `src/lib.rs`:

| Code | Error | Meaning |
| --- | --- | --- |
| 1 | `CallerNotOwner` | Governance call was not signed by the owner. |
| 2 | `GlobalMaxLogsReached` | `total_events` reached `global_max_logs`. |
| 3 | `EventTypeMaxLogsReached` | The event type reached its configured cap. |
| 4 | `EventDoesNotExist` | The requested event ID or index does not exist. |
| 5 | `EventTypeIndexOutOfBounds` | The type-local index is invalid. |
| 6 | `NewOwnerIsZero` | Ownership transfer attempted to the null account. |
| 7 | `CapNotSet` | A required cap was not configured. |
| 8 | `MetadataTooLarge` | Metadata exceeds the configured size limit. |
| 9 | `ContractNotInitialized` | The contract must be initialized first. |
| 10 | `TotalEventsOverflow` | Event counter overflow protection triggered. |
| 11 | `TimestampOutOfRange` | Ledger timestamp moved backward or too far forward. |
| 12 | `InvalidSignature` | Signature payload is malformed. |
| 13 | `ContractPaused` | Writes are paused. |
| 14 | `RateLimitExceeded` | Submitter exceeded the configured rate limit. |
| 15 | `SameOwner` | New owner equals current owner. |
| 16 | `MaxLogsBelowCurrentCount` | New global max is below the current event count. |
| 17 | `CapAlreadyRemoved` | Cap removal was requested more than once. |
| 18 | `CapNeverSet` | Cap removal was requested before a cap was set. |

When a command fails, capture verbose output:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --source "$OWNER_KEY" \
  --network "$NETWORK" \
  -- set_global_max_logs \
  --caller "$OWNER_ADDRESS" \
  --new_max 1 2>&1 | tee soroban-error.log
```

Use the code in `Error(Contract, #N)` to map the failure to the table above. If simulation succeeds but submission fails, retry with a higher fee and confirm the source account has enough XLM for fees and rent.
