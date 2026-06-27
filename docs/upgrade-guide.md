# Contract Upgrade Guide

A step-by-step guide for upgrading AuditLedger contract logic and migrating event data safely without data loss.

---

## Upgrade Overview

### When upgrades are needed

- **Bug fixes** — incorrect hash chain computation, auth bypass, off-by-one in index tracking.
- **New features** — additional `DataKey` variants, new governance functions, schema extensions.
- **Dependency bumps** — Soroban SDK major version updates that change XDR encoding.

### Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Data loss | Permanent; events are immutable but storage keys can become unreachable | Backup all events off-chain before upgrading |
| Contract downtime | New WASM replaces the old one atomically; there is no swap/warmup period | Freeze logging before upgrade to prevent in-flight writes |
| Broken integrations | Callers using removed functions or changed argument order will fail | Notify integrators; version the API |
| Storage key collision | Old `DataKey` variants encoded differently than new ones silently corrupt reads | Keep tombstone variants in the enum; never reuse ordinals |

---

## Pre-Upgrade Checklist

Complete every item before invoking the WASM upgrade.

- [ ] **Back up all event data off-chain.** Run the backup script:
  ```bash
  bash tools/backup/backup.sh
  # verifies integrity after download
  bash tools/backup/verify.sh
  ```
  Store the backup in at least two locations (e.g., S3 + local).

- [ ] **Record the pre-upgrade state snapshot:**
  ```bash
  soroban contract invoke --id $CONTRACT_ID --network testnet -- total_events
  soroban contract invoke --id $CONTRACT_ID --network testnet -- get_owner
  ```
  Save these values — you will compare them after the upgrade.

- [ ] **Notify all integrators** of the planned upgrade window, expected downtime, and any API changes. Allow at least 48 hours notice for production systems.

- [ ] **Freeze event logging** by setting `global_max_logs` to the current `total_events` value:
  ```bash
  TOTAL=$(soroban contract invoke --id $CONTRACT_ID --network testnet -- total_events)
  soroban contract invoke \
    --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
    -- set_global_max_logs --caller $OWNER_ADDRESS --new_max "$TOTAL"
  ```
  From this point, `log_event` and `log_events` will return `GlobalMaxLogsReached` (2), preventing new writes during migration.

- [ ] **Test the new WASM on a forked or standalone network** before applying to testnet/mainnet. The GitHub Actions workflow runs tests automatically on every push.

---

## WASM Upgrade

Soroban supports in-place WASM replacement via `env.deployer().update_current_contract_wasm()`. The contract address, storage, and state are preserved; only the executable code changes.

### Build the new WASM

```bash
cargo build --target wasm32-unknown-unknown --release
soroban contract optimize \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm
# output: target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm
```

### Upload and verify the WASM hash

```bash
NEW_HASH=$(soroban contract install \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm \
  --source $OWNER_KEY \
  --network testnet)
echo "New WASM hash: $NEW_HASH"
```

Record `$NEW_HASH` — you will pass it to the upgrade function.

### Invoke the upgrade

The `upgrade` governance function (owner only) calls `env.deployer().update_current_contract_wasm()` internally:

```bash
soroban contract invoke \
  --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
  -- upgrade \
  --caller $OWNER_ADDRESS \
  --new_wasm_hash "$NEW_HASH"
```

The contract bytecode is replaced atomically in the same transaction. Existing storage is untouched.

---

## Data Migration

Most upgrades do not require data migration — if you only add new `DataKey` variants and do not rename or reorder existing ones, old data remains readable.

### When migration is required

- A storage key's XDR encoding changed (e.g., a `contracttype` struct field was reordered).
- A key was renamed or replaced (e.g., `GlobalMaxLogs` + `TotalEvents` → `Config`).
- A new mandatory field was added to `Event` with no default.

### Running a migration function

Add a one-time `migrate_v1_to_v2` function to the contract before upgrading:

```rust
pub fn migrate_v1_to_v2(env: Env, caller: Address) {
    caller.require_auth();
    Self::require_owner(&env, &caller);
    // Example: fold separate GlobalMaxLogs + TotalEvents into Config
    let max: u32 = env.storage().instance().get(&DataKey::GlobalMaxLogs).unwrap();
    let total: u32 = env.storage().instance().get(&DataKey::TotalEvents).unwrap();
    env.storage().instance().set(&DataKey::Config, &Config { global_max_logs: max, total_events: total });
    // Leave old keys as tombstones; do not remove them to avoid accidental re-use
}
```

After the WASM upgrade:

```bash
soroban contract invoke \
  --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
  -- migrate_v1_to_v2 --caller $OWNER_ADDRESS
```

### Verify event data integrity

Spot-check a sample of events to confirm the hash chain is intact:

```bash
# Check first event (genesis: prev_hash should be all zeros)
soroban contract invoke --id $CONTRACT_ID --network testnet \
  -- get_event_by_order --order_index 0

# Check last event
LAST=$(($(soroban contract invoke --id $CONTRACT_ID --network testnet -- total_events) - 1))
soroban contract invoke --id $CONTRACT_ID --network testnet \
  -- get_event_by_order --order_index "$LAST"
```

---

## Post-Upgrade Verification

1. **Confirm total event count matches pre-upgrade snapshot:**
   ```bash
   soroban contract invoke --id $CONTRACT_ID --network testnet -- total_events
   # Must equal the value recorded in the pre-upgrade checklist
   ```

2. **Spot-check specific events** by their known IDs (saved in the off-chain backup):
   ```bash
   soroban contract invoke --id $CONTRACT_ID --network testnet \
     -- get_event --id <KNOWN_EVENT_ID>
   ```

3. **Verify ownership is intact:**
   ```bash
   soroban contract invoke --id $CONTRACT_ID --network testnet -- get_owner
   ```

4. **Unfreeze logging** by restoring the desired `global_max_logs`:
   ```bash
   soroban contract invoke \
     --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
     -- set_global_max_logs --caller $OWNER_ADDRESS --new_max 500000
   ```

5. **Run a smoke-test log event** to confirm writes work end-to-end:
   ```bash
   soroban contract invoke \
     --id $CONTRACT_ID --source $SUBMITTER_KEY --network testnet \
     -- log_event \
     --submitter $SUBMITTER_ADDRESS \
     --event_type smoke_test \
     --metadata "upgrade-verified"
   ```

---

## Rollback Plan

Soroban does not provide a built-in rollback; a rollback is another upgrade to the previous WASM hash.

### Prerequisites

- The previous WASM hash must still exist on-chain (it does until ledger garbage collection removes it — typically days to weeks). Upload it again if needed:
  ```bash
  PREV_HASH=$(soroban contract install \
    --wasm path/to/previous/audit_ledger.optimized.wasm \
    --source $OWNER_KEY \
    --network testnet)
  ```

### Steps

1. **Freeze logging** (same as pre-upgrade step) to prevent writes during rollback.
2. **Upgrade back** to the previous WASM:
   ```bash
   soroban contract invoke \
     --id $CONTRACT_ID --source $OWNER_KEY --network testnet \
     -- upgrade \
     --caller $OWNER_ADDRESS \
     --new_wasm_hash "$PREV_HASH"
   ```
3. **If data migration ran**, undo it by invoking a `rollback_v2_to_v1` function (write this before upgrading — not after):
   - Restoring from the off-chain backup via `tools/backup/restore.sh` may be the only option if forward-migration is not reversible.
4. **Verify state** using the post-upgrade verification steps above.
5. **Unfreeze logging.**
6. **Notify integrators** of the rollback.

### Restoring from backup

If on-chain state is unrecoverable, redeploy a fresh contract and replay events from the off-chain backup:

```bash
# Deploy fresh contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/audit_ledger.optimized.wasm \
  --source $OWNER_KEY \
  --network testnet

# Initialize
soroban contract invoke --id $NEW_CONTRACT_ID --source $OWNER_KEY --network testnet \
  -- initialize --owner $OWNER_ADDRESS --global_max_logs 500000

# Replay from backup (adapt the restore script to call log_event per record)
bash tools/backup/restore.sh --contract-id $NEW_CONTRACT_ID
```

> **Warning:** Replayed events will have new timestamps and new IDs. External systems referencing old event IDs must be updated.
