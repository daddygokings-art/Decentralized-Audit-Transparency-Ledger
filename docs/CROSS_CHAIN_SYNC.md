# Cross-Chain Event Synchronisation

Issue #375 — This document describes the cross-chain synchronisation system
that mirrors AuditLedger events from Stellar/Soroban to EVM chains.

---

## Overview

The AuditLedger stores immutable audit events on Stellar. To make these events
accessible on EVM chains (for DeFi integrations, regulatory reports, etc.),
a relayer network bridges event batches and commits Merkle-root checkpoints to
`CrossChainSync.sol`.

```
Stellar Network                  Relayer                  EVM Chain
──────────────                   ───────                  ─────────
AuditLedger.logEvent()  ──────→  CrossChainSynchronizer   CrossChainSync.sol
                                  │                        │
                                  ├─ fetchEventBatch()     │
                                  ├─ computeMerkleRoot()   │
                                  ├─ detectConflicts()     │
                                  ├─ syncEventsToChain()  ──→ SyncInitiated event
                                  └─ recordCheckpoint()   ──→ CheckpointRecorded event
```

### Design goals

- **Verifiability**: any third party can independently verify checkpoints using
  the Merkle root and event hashes from the Soroban RPC.
- **Conflict safety**: hash mismatches are surfaced as on-chain `ConflictDetected`
  events; they never silently corrupt the chain state.
- **Resilience**: relayers retry with exponential backoff; multiple relayers can
  operate in parallel.
- **Access control**: only authorised relayers may sync and checkpoint; only the
  owner may resolve conflicts.

---

## Components

### CrossChainSync.sol (EVM)

Solidity contract at `bridge/evm/CrossChainSync.sol`.

| Function | Access | Description |
|----------|--------|-------------|
| `syncEventsToChain(chainId, fromIndex, toIndex)` | Relayer | Initiate a sync batch |
| `recordCheckpoint(chainId, eventIndex, merkleRoot, timestamp)` | Relayer | Record a Merkle-root checkpoint |
| `registerEventHash(chainId, eventIndex, hash)` | Relayer | Register an individual event hash |
| `getLatestCheckpoint(chainId)` | Public | Return the latest checkpoint |
| `resolveSyncConflict(chainId, eventIndex, canonicalHash)` | Owner | Accept Stellar-side hash |
| `setRelayer(relayer, authorised)` | Owner | Add/remove relayers |
| `setPaused(paused)` | Owner | Emergency pause |

### CrossChainSynchronizer (TypeScript relayer)

At `bridge/relayer/crossChainSync.ts`.

| Method | Description |
|--------|-------------|
| `syncEventRange(from, to, chainId)` | Full sync pipeline for an event range |
| `detectConflicts(local, remote)` | Compare event hash arrays |
| `resolveConflict(conflict, strategy)` | Submit conflict resolution |
| `createCheckpoint(index, root, chainId)` | Record a checkpoint |
| `getCheckpointStatus(chainId)` | Query latest checkpoint |

---

## Data Structures

### Checkpoint

```solidity
struct Checkpoint {
    uint32  eventIndex;  // highest event index covered
    bytes32 merkleRoot;  // Merkle root of events [0, eventIndex]
    uint64  timestamp;   // Stellar ledger timestamp
    address recordedBy;  // relayer address
    bool    exists;
}
```

### SyncStatus

```solidity
struct SyncStatus {
    uint32  lastSyncedIndex;    // last event index successfully synced
    uint64  lastSyncTimestamp;  // when it was synced (EVM block time)
    uint256 totalSynced;        // cumulative events synced
    uint32  pendingConflicts;   // unresolved hash conflicts
    bool    active;             // whether a sync has been initiated
}
```

### ConflictRecord (on-chain)

```solidity
struct ConflictRecord {
    uint32  eventIndex;
    bytes32 localHash;   // hash stored on EVM chain
    bytes32 remoteHash;  // hash reported from Stellar
    uint64  detectedAt;
    bool    resolved;
    bytes32 resolvedHash;
}
```

---

## Events

| Event | Topics | Description |
|-------|--------|-------------|
| `SyncInitiated` | `chainId`, `relayer` | Sync batch started |
| `CheckpointRecorded` | `chainId`, `relayer` | Merkle root committed |
| `ConflictDetected` | `chainId` | Hash mismatch found |
| `ConflictResolved` | `chainId`, `resolver` | Conflict resolved by owner |
| `SyncCompleted` | `chainId` | Sync batch fully processed |
| `RelayerUpdated` | `relayer` | Relayer added/revoked |

---

## Conflict Detection and Resolution

### How conflicts arise

A conflict occurs when the EVM chain has a hash registered for event index N,
and the relayer reports a different hash from the Stellar side. This can happen
due to:

1. **Reorg on Stellar** (very rare — Stellar has finality in ~5s).
2. **Malicious relayer** reporting a fake hash.
3. **Data corruption** during serialisation.

### Automatic detection

When `recordCheckpoint` is called with a `merkleRoot` that differs from a
previously stored root at the same `eventIndex`, a `ConflictDetected` event
is emitted and the conflict is stored for manual resolution.

When `registerEventHash` is called with a hash that differs from the stored
value, a conflict is similarly recorded.

### Resolution strategies

```typescript
// Accept Stellar-side (canonical) hash
await sync.resolveConflict(conflict, 'canonical');

// Accept EVM-side hash (when EVM is authoritative)
await sync.resolveConflict(conflict, 'remote');
```

On-chain, the owner calls:

```solidity
CrossChainSync.resolveSyncConflict(chainId, eventIndex, canonicalHash);
```

This updates `eventHashes[chainId][eventIndex]` to the accepted hash and
emits `ConflictResolved`.

---

## Relayer Deployment

### Running the TypeScript relayer

```bash
cd bridge/relayer

# Install dependencies
npm install

# Configure
export SOROBAN_RPC_URL="https://soroban-testnet.stellar.org"
export CONTRACT_ID="C..."
export EVM_RPC_URL="https://mainnet.infura.io/v3/..."
export EVM_CONTRACT_ADDRESS="0x..."
export RELAYER_PRIVATE_KEY="0x..."

# Run (syncs every 30 seconds)
npx ts-node crossChainSync.ts
```

### Minimal sync loop

```typescript
import { CrossChainSynchronizer } from './crossChainSync';

const sync = new CrossChainSynchronizer({
    sorobanRpcUrl:      process.env.SOROBAN_RPC_URL!,
    contractId:         process.env.CONTRACT_ID!,
    evmContractAddress: process.env.EVM_CONTRACT_ADDRESS!,
    evmRpcUrl:          process.env.EVM_RPC_URL!,
    batchSize:          200,
    maxRetries:         5,
    backoffBaseMs:      2000,
});

// Sync the last 1000 events
const total = await stellarContract.totalEvents();
const result = await sync.syncEventRange(
    Math.max(0, total - 1000),
    total - 1,
    1,  // Ethereum mainnet chain ID
);

console.log(`Synced ${result.syncedCount} events, ${result.conflictCount} conflicts`);
```

---

## Retry and Backoff

The relayer retries failed operations with exponential backoff + jitter:

```
delay = backoffBaseMs × 2^attempt + jitter(0..100ms)

Attempt 0: immediate
Attempt 1: ~1000ms
Attempt 2: ~2000ms
Attempt 3: ~4000ms (then fail)
```

Configure via `maxRetries` and `backoffBaseMs` in `SyncOptions`.

---

## Security Considerations

| Risk | Mitigation |
|------|-----------|
| Malicious relayer injecting false event hashes | Only authorised relayers can write; conflicts require owner resolution |
| Single relayer failure | Run multiple relayers from different operators; any one can record checkpoints |
| Owner key compromise | Use a multisig wallet as the owner |
| Conflict storm (many hash mismatches) | Conflicts are queued, not blocking; owner resolves in batches |
| Replay of old checkpoint | `recordCheckpoint` only advances `latestCheckpoint`; older indices don't overwrite newer state |

---

## On-Chain Verification

Any third party can independently verify a checkpoint:

1. Fetch the Merkle root from `CrossChainSync.getLatestCheckpoint(chainId)`.
2. Fetch all event hashes from the Soroban RPC for events `[0, checkpoint.eventIndex]`.
3. Compute `computeMerkleRoot(hashes)` locally.
4. Compare with `checkpoint.merkleRoot` — they must match.

This requires no trust in the relayer.

---

## References

- [`bridge/evm/CrossChainSync.sol`](../bridge/evm/CrossChainSync.sol)
- [`bridge/relayer/crossChainSync.ts`](../bridge/relayer/crossChainSync.ts)
- [`docs/ZK_BRIDGE.md`](ZK_BRIDGE.md)
- [Stellar consensus protocol](https://stellar.org/papers/stellar-consensus-protocol)
