/**
 * crossChainSync.ts — Cross-Chain Event Synchronisation Relayer
 *
 * Issue #375: Cross-chain event synchronisation.
 *
 * Implements the `CrossChainSynchronizer` class that bridges events from the
 * Stellar/Soroban AuditLedger to EVM chains via the CrossChainSync.sol contract.
 *
 * Responsibilities:
 *   - Fetch events from the Soroban RPC in ordered batches.
 *   - Compute Merkle roots and register checkpoints on-chain.
 *   - Detect and record hash conflicts between chains.
 *   - Resolve conflicts according to a configurable strategy.
 *   - Retry failed operations with exponential backoff.
 *
 * Usage:
 *   const sync = new CrossChainSynchronizer({ rpcUrl, contractAddress, ... });
 *   const result = await sync.syncEventRange(0, 99, CHAIN_ID_ETHEREUM);
 */

import { computeMerkleRoot, buildMerkleProof } from './zkProofGenerator';
import { createHash } from 'crypto';

// ── Types ─────────────────────────────────────────────────────────────────────

/**
 * A single event record fetched from the Soroban RPC.
 */
export interface EventRecord {
  index:        number;
  timestamp:    number;
  eventType:    string;
  category:     string;
  submitter:    string;
  eventHash:    string;  // 0x hex bytes32
  prevHash:     string;  // 0x hex bytes32
  metadata:     string;  // raw bytes as hex string
}

/**
 * Result of a sync operation.
 */
export interface SyncResult {
  success:         boolean;
  chainId:         number;
  fromIndex:       number;
  toIndex:         number;
  syncedCount:     number;
  conflictCount:   number;
  checkpointHash:  string;  // Merkle root of synced batch
  durationMs:      number;
  errors:          string[];
}

/**
 * A detected conflict between local and remote event hashes.
 */
export interface ConflictRecord {
  chainId:     number;
  eventIndex:  number;
  localHash:   string;
  remoteHash:  string;
  detectedAt:  number;  // unix ms
  resolved:    boolean;
  resolvedHash?: string;
}

/**
 * Status of the checkpoint for a given chain.
 */
export interface CheckpointStatus {
  chainId:           number;
  lastSyncedIndex:   number;
  lastSyncTimestamp: number;
  totalSynced:       number;
  pendingConflicts:  number;
  merkleRoot:        string;
  latestCheckpoint?: {
    eventIndex: number;
    merkleRoot: string;
    timestamp:  number;
  };
}

/**
 * Constructor options for CrossChainSynchronizer.
 */
export interface SyncOptions {
  /** Soroban RPC endpoint URL. */
  sorobanRpcUrl: string;
  /** AuditLedger Soroban contract ID. */
  contractId: string;
  /** CrossChainSync.sol contract address on the EVM chain. */
  evmContractAddress: string;
  /** EVM RPC endpoint URL. */
  evmRpcUrl: string;
  /** Max events per sync batch (default 100). */
  batchSize?: number;
  /** Max retry attempts for failed operations (default 3). */
  maxRetries?: number;
  /** Base delay in ms for exponential backoff (default 1000). */
  backoffBaseMs?: number;
}

// ── CrossChainSynchronizer ────────────────────────────────────────────────────

export class CrossChainSynchronizer {
  private readonly opts: Required<SyncOptions>;

  /** In-memory conflict registry (production: persist to DB). */
  private conflicts: ConflictRecord[] = [];

  /** In-memory checkpoint cache: chainId → CheckpointStatus */
  private checkpoints = new Map<number, CheckpointStatus>();

  constructor(options: SyncOptions) {
    this.opts = {
      batchSize:     100,
      maxRetries:    3,
      backoffBaseMs: 1000,
      ...options,
    };
  }

  // ── Main sync API ──────────────────────────────────────────────────────────

  /**
   * Synchronise a range of events to a target EVM chain.
   *
   * Steps:
   *   1. Fetch events [fromIndex, toIndex] from the Soroban RPC.
   *   2. Compute the Merkle root of the batch.
   *   3. Detect conflicts with any previously stored hashes.
   *   4. Submit a `syncEventsToChain` transaction to CrossChainSync.sol.
   *   5. Record the checkpoint.
   *
   * @param fromIndex     First event index (inclusive).
   * @param toIndex       Last event index (inclusive).
   * @param targetChainId Numeric chain ID.
   * @returns             SyncResult.
   */
  async syncEventRange(
    fromIndex:     number,
    toIndex:       number,
    targetChainId: number,
  ): Promise<SyncResult> {
    const start = Date.now();
    const errors: string[] = [];
    let conflictCount = 0;
    let syncedCount = 0;
    let checkpointHash = '0x' + '00'.repeat(32);

    if (fromIndex > toIndex) {
      return {
        success:        false,
        chainId:        targetChainId,
        fromIndex,
        toIndex,
        syncedCount:    0,
        conflictCount:  0,
        checkpointHash,
        durationMs:     Date.now() - start,
        errors:         ['fromIndex must be <= toIndex'],
      };
    }

    try {
      // Step 1: Fetch events from Soroban RPC in sub-batches.
      const allEvents: EventRecord[] = [];
      for (let i = fromIndex; i <= toIndex; i += this.opts.batchSize) {
        const batchEnd = Math.min(i + this.opts.batchSize - 1, toIndex);
        const batch = await this._withRetry(() =>
          this._fetchEventBatch(i, batchEnd),
        );
        allEvents.push(...batch);
      }

      if (allEvents.length === 0) {
        return {
          success:        true,
          chainId:        targetChainId,
          fromIndex,
          toIndex,
          syncedCount:    0,
          conflictCount:  0,
          checkpointHash,
          durationMs:     Date.now() - start,
          errors:         [],
        };
      }

      // Step 2: Compute Merkle root.
      const leafHashes = allEvents.map((e) => e.eventHash);
      checkpointHash = computeMerkleRoot(leafHashes);

      // Step 3: Detect conflicts.
      const existing = await this._fetchRemoteEvents(
        targetChainId,
        fromIndex,
        toIndex,
      );
      const newConflicts = this.detectConflicts(allEvents, existing);
      for (const c of newConflicts) {
        this.conflicts.push(c);
        conflictCount++;
      }

      // Step 4: Initiate sync on-chain.
      await this._withRetry(() =>
        this._submitSyncEventsToChain(
          targetChainId,
          fromIndex,
          toIndex,
        ),
      );

      // Step 5: Record checkpoint.
      await this.createCheckpoint(toIndex, checkpointHash, targetChainId);

      syncedCount = allEvents.length;

      // Update local cache.
      this.checkpoints.set(targetChainId, {
        chainId:           targetChainId,
        lastSyncedIndex:   toIndex,
        lastSyncTimestamp: Date.now(),
        totalSynced:       syncedCount,
        pendingConflicts:  conflictCount,
        merkleRoot:        checkpointHash,
        latestCheckpoint: {
          eventIndex: toIndex,
          merkleRoot: checkpointHash,
          timestamp:  Date.now(),
        },
      });

    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      errors.push(msg);
      return {
        success:        false,
        chainId:        targetChainId,
        fromIndex,
        toIndex,
        syncedCount,
        conflictCount,
        checkpointHash,
        durationMs:     Date.now() - start,
        errors,
      };
    }

    return {
      success:        true,
      chainId:        targetChainId,
      fromIndex,
      toIndex,
      syncedCount,
      conflictCount,
      checkpointHash,
      durationMs:     Date.now() - start,
      errors,
    };
  }

  /**
   * Detect hash conflicts between local (Soroban) and remote (EVM) event sets.
   *
   * Matches events by `index` and compares `eventHash` values.
   *
   * @param localEvents  Events fetched from the Soroban AuditLedger.
   * @param remoteEvents Events stored on the EVM chain.
   * @returns            Array of ConflictRecord for mismatches.
   */
  detectConflicts(
    localEvents:  EventRecord[],
    remoteEvents: EventRecord[],
  ): ConflictRecord[] {
    const remoteMap = new Map<number, EventRecord>();
    for (const r of remoteEvents) {
      remoteMap.set(r.index, r);
    }

    const conflicts: ConflictRecord[] = [];
    for (const local of localEvents) {
      const remote = remoteMap.get(local.index);
      if (!remote) continue; // No remote record — not a conflict yet.

      const localHash  = normaliseHash(local.eventHash);
      const remoteHash = normaliseHash(remote.eventHash);

      if (localHash !== remoteHash) {
        conflicts.push({
          chainId:    0, // set by caller
          eventIndex: local.index,
          localHash:  '0x' + localHash,
          remoteHash: '0x' + remoteHash,
          detectedAt: Date.now(),
          resolved:   false,
        });
      }
    }

    return conflicts;
  }

  /**
   * Resolve a conflict using the specified strategy.
   *
   * - `'canonical'`: Accept the Stellar-side (local) hash as authoritative.
   * - `'remote'`:    Accept the EVM-side (remote) hash (use when EVM chain
   *                  holds an independently verified authoritative record).
   *
   * @param conflict  ConflictRecord to resolve.
   * @param strategy  Resolution strategy.
   */
  async resolveConflict(
    conflict: ConflictRecord,
    strategy: 'canonical' | 'remote',
  ): Promise<void> {
    const canonicalHash =
      strategy === 'canonical' ? conflict.localHash : conflict.remoteHash;

    // Submit resolution to CrossChainSync.sol.
    await this._withRetry(() =>
      this._submitResolveSyncConflict(
        conflict.chainId,
        conflict.eventIndex,
        canonicalHash,
      ),
    );

    // Update in-memory record.
    const idx = this.conflicts.findIndex(
      (c) =>
        c.chainId === conflict.chainId &&
        c.eventIndex === conflict.eventIndex &&
        !c.resolved,
    );
    if (idx >= 0) {
      this.conflicts[idx].resolved = true;
      this.conflicts[idx].resolvedHash = canonicalHash;
    }
  }

  /**
   * Record a Merkle-root checkpoint for a given event index.
   *
   * @param eventIndex  Highest event index in this checkpoint.
   * @param merkleRoot  Merkle root covering all events [0, eventIndex].
   * @param chainId     Target chain ID (default: use first registered chain).
   */
  async createCheckpoint(
    eventIndex: number,
    merkleRoot: string,
    chainId = 1,
  ): Promise<void> {
    const timestamp = Math.floor(Date.now() / 1000);
    await this._withRetry(() =>
      this._submitRecordCheckpoint(chainId, eventIndex, merkleRoot, timestamp),
    );
  }

  /**
   * Return the current checkpoint status for a chain.
   *
   * @param chainId Target chain ID.
   * @returns       CheckpointStatus.
   */
  async getCheckpointStatus(chainId: number): Promise<CheckpointStatus> {
    // Return from local cache if available.
    if (this.checkpoints.has(chainId)) {
      return this.checkpoints.get(chainId)!;
    }

    // Fetch from EVM chain.
    const status = await this._withRetry(() =>
      this._fetchCheckpointStatus(chainId),
    );
    this.checkpoints.set(chainId, status);
    return status;
  }

  /**
   * Return all detected conflicts (resolved and unresolved).
   */
  getAllConflicts(): ConflictRecord[] {
    return [...this.conflicts];
  }

  /**
   * Return unresolved conflicts for a specific chain.
   */
  getPendingConflicts(chainId: number): ConflictRecord[] {
    return this.conflicts.filter((c) => c.chainId === chainId && !c.resolved);
  }

  // ── Private: Soroban RPC ──────────────────────────────────────────────────

  /**
   * Fetch a batch of events from the Soroban AuditLedger RPC.
   *
   * In production, this calls the Soroban RPC `getEvents` or invokes
   * `getEventByOrder` for each index.
   */
  private async _fetchEventBatch(
    fromIndex: number,
    toIndex:   number,
  ): Promise<EventRecord[]> {
    // Stub implementation. Replace with actual Soroban RPC calls:
    //   const rpc = new SorobanRpc.Server(this.opts.sorobanRpcUrl);
    //   const events = await rpc.getEvents({ ... });

    const events: EventRecord[] = [];
    for (let i = fromIndex; i <= toIndex; ++i) {
      events.push(_mockEvent(i, this.opts.contractId));
    }
    return events;
  }

  /**
   * Fetch events previously registered on the EVM chain for conflict detection.
   */
  private async _fetchRemoteEvents(
    chainId:   number,
    fromIndex: number,
    toIndex:   number,
  ): Promise<EventRecord[]> {
    // Stub: return empty array (no remote events yet).
    // Production: query CrossChainSync.sol `eventHashes` for each index.
    return [];
  }

  // ── Private: EVM transactions ─────────────────────────────────────────────

  private async _submitSyncEventsToChain(
    chainId:   number,
    fromIndex: number,
    toIndex:   number,
  ): Promise<void> {
    // Production: call CrossChainSync.syncEventsToChain(chainId, fromIndex, toIndex)
    // via ethers.js or viem.
  }

  private async _submitRecordCheckpoint(
    chainId:     number,
    eventIndex:  number,
    merkleRoot:  string,
    timestamp:   number,
  ): Promise<void> {
    // Production: call CrossChainSync.recordCheckpoint(chainId, eventIndex, merkleRoot, timestamp)
  }

  private async _submitResolveSyncConflict(
    chainId:       number,
    eventIndex:    number,
    canonicalHash: string,
  ): Promise<void> {
    // Production: call CrossChainSync.resolveSyncConflict(chainId, eventIndex, canonicalHash)
  }

  private async _fetchCheckpointStatus(
    chainId: number,
  ): Promise<CheckpointStatus> {
    // Production: call CrossChainSync.getSyncStatus(chainId) + getLatestCheckpoint(chainId)
    return {
      chainId,
      lastSyncedIndex:   0,
      lastSyncTimestamp: 0,
      totalSynced:       0,
      pendingConflicts:  0,
      merkleRoot:        '0x' + '00'.repeat(32),
    };
  }

  // ── Private: retry helper ─────────────────────────────────────────────────

  /**
   * Execute `fn` with exponential backoff, retrying up to `maxRetries` times.
   */
  private async _withRetry<T>(fn: () => Promise<T>): Promise<T> {
    let lastError: unknown;
    for (let attempt = 0; attempt <= this.opts.maxRetries; ++attempt) {
      try {
        return await fn();
      } catch (err) {
        lastError = err;
        if (attempt < this.opts.maxRetries) {
          const delayMs =
            this.opts.backoffBaseMs * Math.pow(2, attempt) +
            Math.floor(Math.random() * 100); // jitter
          await sleep(delayMs);
        }
      }
    }
    throw lastError;
  }
}

// ── Utility ───────────────────────────────────────────────────────────────────

function normaliseHash(hash: string): string {
  return hash.toLowerCase().replace(/^0x/, '').padStart(64, '0');
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Build a deterministic mock event for a given index (development only).
 */
function _mockEvent(index: number, contractId: string): EventRecord {
  const preimage = `${contractId}:${index}`;
  const hash = createHash('sha256').update(preimage).digest('hex');
  const prevHash = index === 0
    ? '00'.repeat(32)
    : createHash('sha256').update(`${contractId}:${index - 1}`).digest('hex');

  return {
    index,
    timestamp:  1_700_000_000 + index * 5,
    eventType:  'payment',
    category:   'finance',
    submitter:  'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF',
    eventHash:  '0x' + hash,
    prevHash:   '0x' + prevHash,
    metadata:   '0x',
  };
}
