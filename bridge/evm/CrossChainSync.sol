// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title CrossChainSync — Cross-Chain Event Synchronisation for AuditLedger
 * @notice Issue #375: Cross-chain event synchronisation.
 *
 * This contract maintains a mirror of the Stellar/Soroban AuditLedger event
 * stream on EVM chains. Authorised relayer nodes push checkpoints (Merkle
 * roots of event batches) and sync requests. Conflicts (hash mismatches
 * between chains) are detected and can be resolved by the contract owner.
 *
 * Architecture:
 *   - Relayers call `syncEventsToChain` to initiate a sync batch.
 *   - Relayers call `recordCheckpoint` after each successful batch.
 *   - `getLatestCheckpoint` lets clients verify the latest synced state.
 *   - `resolveSyncConflict` allows the owner to override a conflicting hash
 *     with the canonical Stellar-side hash.
 *
 * Access control:
 *   - Owner: full governance (add/remove relayers, resolve conflicts, pause).
 *   - Authorised relayers: sync and checkpoint operations.
 */
contract CrossChainSync {

    // ── Errors ────────────────────────────────────────────────────────────────

    error NotOwner();
    error NotRelayer();
    error ContractPaused();
    error InvalidRange();
    error CheckpointAlreadyExists();
    error NoCheckpointFound();
    error ConflictNotPending();
    error InvalidChainId();
    error BatchTooLarge();

    // ── Constants ─────────────────────────────────────────────────────────────

    uint32 public constant MAX_SYNC_BATCH_SIZE = 1000;

    // ── Events ────────────────────────────────────────────────────────────────

    /**
     * @notice Emitted when a sync batch is initiated by a relayer.
     * @param chainId     Target chain identifier (Stellar network passphrase hash).
     * @param fromIndex   First event index in the batch (inclusive).
     * @param toIndex     Last event index in the batch (inclusive).
     * @param relayer     Address of the relayer initiating the sync.
     */
    event SyncInitiated(
        uint32  indexed chainId,
        uint32          fromIndex,
        uint32          toIndex,
        address indexed relayer,
        uint256         timestamp
    );

    /**
     * @notice Emitted when a checkpoint (Merkle root) is recorded.
     * @param chainId     Chain this checkpoint applies to.
     * @param eventIndex  Event index at which this checkpoint was taken.
     * @param merkleRoot  Merkle root of all events up to and including eventIndex.
     * @param relayer     Relayer that recorded the checkpoint.
     */
    event CheckpointRecorded(
        uint32  indexed chainId,
        uint32          eventIndex,
        bytes32         merkleRoot,
        address indexed relayer,
        uint64          timestamp
    );

    /**
     * @notice Emitted when a hash conflict is detected between chains.
     * @param chainId       Chain where the conflict was detected.
     * @param eventIndex    Event index where hashes diverge.
     * @param localHash     Hash recorded on this (EVM) chain.
     * @param remoteHash    Hash reported from the Stellar side.
     */
    event ConflictDetected(
        uint32  indexed chainId,
        uint32          eventIndex,
        bytes32         localHash,
        bytes32         remoteHash,
        uint256         timestamp
    );

    /**
     * @notice Emitted when a conflict is resolved.
     * @param chainId       Chain where the conflict was resolved.
     * @param eventIndex    Event index that was reconciled.
     * @param canonicalHash Accepted canonical hash (Stellar-side).
     * @param resolver      Address that resolved the conflict.
     */
    event ConflictResolved(
        uint32  indexed chainId,
        uint32          eventIndex,
        bytes32         canonicalHash,
        address indexed resolver,
        uint256         timestamp
    );

    /**
     * @notice Emitted when a sync batch completes successfully.
     */
    event SyncCompleted(
        uint32  indexed chainId,
        uint32          fromIndex,
        uint32          toIndex,
        bytes32         merkleRoot,
        uint256         timestamp
    );

    /**
     * @notice Emitted when a relayer is authorised or revoked.
     */
    event RelayerUpdated(address indexed relayer, bool authorised);

    // ── Structs ───────────────────────────────────────────────────────────────

    /**
     * @notice A Merkle-root checkpoint for a specific event index.
     */
    struct Checkpoint {
        uint32  eventIndex;
        bytes32 merkleRoot;
        uint64  timestamp;
        address recordedBy;
        bool    exists;
    }

    /**
     * @notice Per-chain synchronisation status.
     */
    struct SyncStatus {
        uint32  lastSyncedIndex;
        uint64  lastSyncTimestamp;
        uint256 totalSynced;
        uint32  pendingConflicts;
        bool    active;
    }

    /**
     * @notice A pending conflict record.
     */
    struct ConflictRecord {
        uint32  eventIndex;
        bytes32 localHash;
        bytes32 remoteHash;
        uint64  detectedAt;
        bool    resolved;
        bytes32 resolvedHash;
    }

    // ── State ─────────────────────────────────────────────────────────────────

    address public immutable owner;
    bool    public paused;

    /// Authorised relayer addresses.
    mapping(address => bool) public authorisedRelayers;

    /// Per-chain latest checkpoint: chainId → Checkpoint.
    mapping(uint32 => Checkpoint) public latestCheckpoints;

    /// Per-chain checkpoint history: chainId → eventIndex → Checkpoint.
    mapping(uint32 => mapping(uint32 => Checkpoint)) public checkpointHistory;

    /// Per-chain sync status.
    mapping(uint32 => SyncStatus) public syncStatus;

    /// Conflict records: chainId → conflictId → ConflictRecord.
    mapping(uint32 => mapping(uint256 => ConflictRecord)) public conflicts;

    /// Number of conflicts per chain.
    mapping(uint32 => uint256) public conflictCount;

    /// Per-chain event hash registry: chainId → eventIndex → hash.
    mapping(uint32 => mapping(uint32 => bytes32)) public eventHashes;

    // ── Modifiers ─────────────────────────────────────────────────────────────

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    modifier onlyRelayer() {
        if (!authorisedRelayers[msg.sender] && msg.sender != owner) revert NotRelayer();
        _;
    }

    modifier whenNotPaused() {
        if (paused) revert ContractPaused();
        _;
    }

    // ── Constructor ───────────────────────────────────────────────────────────

    constructor(address[] memory initialRelayers) {
        owner = msg.sender;
        for (uint256 i = 0; i < initialRelayers.length; ++i) {
            authorisedRelayers[initialRelayers[i]] = true;
            emit RelayerUpdated(initialRelayers[i], true);
        }
    }

    // ── Sync operations ───────────────────────────────────────────────────────

    /**
     * @notice Initiate synchronisation of a range of events to a target chain.
     *
     * Called by a relayer when it has fetched events [fromIndex, toIndex] from
     * the Stellar AuditLedger and wants to record them on this EVM chain.
     *
     * Emits {SyncInitiated}.
     *
     * @param chainId    Numeric ID representing the target chain context.
     * @param fromIndex  First event global index (inclusive).
     * @param toIndex    Last event global index (inclusive).
     */
    function syncEventsToChain(
        uint32 chainId,
        uint32 fromIndex,
        uint32 toIndex
    ) external onlyRelayer whenNotPaused {
        if (chainId == 0) revert InvalidChainId();
        if (fromIndex > toIndex) revert InvalidRange();
        if (toIndex - fromIndex + 1 > MAX_SYNC_BATCH_SIZE) revert BatchTooLarge();

        syncStatus[chainId].active = true;

        emit SyncInitiated(chainId, fromIndex, toIndex, msg.sender, block.timestamp);
    }

    /**
     * @notice Record a Merkle-root checkpoint after a successful sync batch.
     *
     * The checkpoint commits to all events up to and including `eventIndex`
     * via the provided Merkle root. If the root differs from a previously
     * seen root at the same index, a ConflictDetected event is emitted.
     *
     * Emits {CheckpointRecorded} and optionally {ConflictDetected}.
     *
     * @param chainId     Target chain ID.
     * @param eventIndex  Highest event index covered by this checkpoint.
     * @param merkleRoot  Merkle root of events [0, eventIndex].
     * @param timestamp   Ledger timestamp from the Stellar side.
     */
    function recordCheckpoint(
        uint32  chainId,
        uint32  eventIndex,
        bytes32 merkleRoot,
        uint64  timestamp
    ) external onlyRelayer whenNotPaused {
        if (chainId == 0) revert InvalidChainId();

        Checkpoint memory existing = checkpointHistory[chainId][eventIndex];
        if (existing.exists && existing.merkleRoot != merkleRoot) {
            // Hash mismatch — record a conflict.
            _recordConflict(chainId, eventIndex, existing.merkleRoot, merkleRoot);
        }

        Checkpoint memory cp = Checkpoint({
            eventIndex: eventIndex,
            merkleRoot: merkleRoot,
            timestamp:  timestamp,
            recordedBy: msg.sender,
            exists:     true
        });

        checkpointHistory[chainId][eventIndex] = cp;

        // Update latest if this is newer.
        if (eventIndex >= latestCheckpoints[chainId].eventIndex || !latestCheckpoints[chainId].exists) {
            latestCheckpoints[chainId] = cp;
        }

        // Update sync status.
        SyncStatus storage status = syncStatus[chainId];
        if (eventIndex > status.lastSyncedIndex) {
            uint256 newTotal = (eventIndex - status.lastSyncedIndex);
            status.lastSyncedIndex   = eventIndex;
            status.lastSyncTimestamp = uint64(block.timestamp);
            status.totalSynced      += newTotal;
        }

        emit CheckpointRecorded(chainId, eventIndex, merkleRoot, msg.sender, timestamp);
    }

    /**
     * @notice Store or update the hash for a specific event on a specific chain.
     *
     * Used by relayers to register individual event hashes. If the stored hash
     * differs from `reportedHash`, a conflict is detected.
     *
     * @param chainId       Target chain ID.
     * @param eventIndex    Event index.
     * @param reportedHash  Hash reported by the Stellar side for this event.
     */
    function registerEventHash(
        uint32  chainId,
        uint32  eventIndex,
        bytes32 reportedHash
    ) external onlyRelayer whenNotPaused {
        if (chainId == 0) revert InvalidChainId();

        bytes32 stored = eventHashes[chainId][eventIndex];
        if (stored != bytes32(0) && stored != reportedHash) {
            _recordConflict(chainId, eventIndex, stored, reportedHash);
        } else {
            eventHashes[chainId][eventIndex] = reportedHash;
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /**
     * @notice Return the latest checkpoint for a given chain.
     *
     * @param chainId Target chain ID.
     * @return cp     Latest Checkpoint struct.
     */
    function getLatestCheckpoint(uint32 chainId)
        external
        view
        returns (Checkpoint memory cp)
    {
        cp = latestCheckpoints[chainId];
        if (!cp.exists) revert NoCheckpointFound();
    }

    /**
     * @notice Return the checkpoint at a specific event index for a chain.
     */
    function getCheckpoint(uint32 chainId, uint32 eventIndex)
        external
        view
        returns (Checkpoint memory)
    {
        return checkpointHistory[chainId][eventIndex];
    }

    /**
     * @notice Return the sync status for a chain.
     */
    function getSyncStatus(uint32 chainId)
        external
        view
        returns (SyncStatus memory)
    {
        return syncStatus[chainId];
    }

    /**
     * @notice Return a conflict record by chain ID and conflict index.
     */
    function getConflict(uint32 chainId, uint256 conflictId)
        external
        view
        returns (ConflictRecord memory)
    {
        return conflicts[chainId][conflictId];
    }

    // ── Conflict resolution ───────────────────────────────────────────────────

    /**
     * @notice Resolve a sync conflict by accepting the canonical Stellar hash.
     *
     * Only the contract owner may resolve conflicts. The canonical hash
     * replaces the local hash in `eventHashes` and the conflict is marked
     * resolved.
     *
     * Emits {ConflictResolved}.
     *
     * @param chainId       Chain where the conflict exists.
     * @param eventIndex    Event index of the conflict.
     * @param canonicalHash Authoritative hash from the Stellar side.
     */
    function resolveSyncConflict(
        uint32  chainId,
        uint32  eventIndex,
        bytes32 canonicalHash
    ) external onlyOwner {
        if (chainId == 0) revert InvalidChainId();

        // Find the unresolved conflict at this event index.
        uint256 count = conflictCount[chainId];
        bool found = false;
        for (uint256 i = 0; i < count; ++i) {
            ConflictRecord storage cr = conflicts[chainId][i];
            if (cr.eventIndex == eventIndex && !cr.resolved) {
                cr.resolved     = true;
                cr.resolvedHash = canonicalHash;
                found = true;
                break;
            }
        }
        if (!found) revert ConflictNotPending();

        // Accept the canonical hash.
        eventHashes[chainId][eventIndex] = canonicalHash;

        // Decrement pending conflicts counter.
        SyncStatus storage status = syncStatus[chainId];
        if (status.pendingConflicts > 0) {
            status.pendingConflicts -= 1;
        }

        emit ConflictResolved(chainId, eventIndex, canonicalHash, msg.sender, block.timestamp);
    }

    // ── Governance ────────────────────────────────────────────────────────────

    /**
     * @notice Authorise or revoke a relayer.
     */
    function setRelayer(address relayer, bool authorised) external onlyOwner {
        authorisedRelayers[relayer] = authorised;
        emit RelayerUpdated(relayer, authorised);
    }

    /**
     * @notice Pause or unpause the contract.
     */
    function setPaused(bool _paused) external onlyOwner {
        paused = _paused;
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    /**
     * @dev Record a new conflict and increment the pending counter.
     */
    function _recordConflict(
        uint32  chainId,
        uint32  eventIndex,
        bytes32 localHash,
        bytes32 remoteHash
    ) internal {
        uint256 cid = conflictCount[chainId]++;
        conflicts[chainId][cid] = ConflictRecord({
            eventIndex:   eventIndex,
            localHash:    localHash,
            remoteHash:   remoteHash,
            detectedAt:   uint64(block.timestamp),
            resolved:     false,
            resolvedHash: bytes32(0)
        });
        syncStatus[chainId].pendingConflicts += 1;

        emit ConflictDetected(chainId, eventIndex, localHash, remoteHash, block.timestamp);
    }
}
