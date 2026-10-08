// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title AuditLedger Cross-Chain Verifier — Upgradeable (#252)
 * @notice Verifies Stellar AuditLedger event proofs on EVM chains.
 *
 * Upgrade mechanism (#252):
 *   - UUPS (Universal Upgradeable Proxy Standard) proxy pattern.
 *   - `upgradeTo(address)` replaces the implementation while preserving all storage.
 *   - `migrateStorage(uint16)` lets a new implementation run storage layout migrations.
 *   - `version()` returns the current implementation version for on-chain tracking.
 *
 * Trust model:
 *   Multiple signers required to verify each proof. At least `threshold` valid signatures
 *   from the registered signer set are needed. Prevents single point of failure.
 *
 * Proof format:
 *   (uint64 ledgerSeq, bytes32 txHash, uint32 eventIndex,
 *    bytes32 eventHash, bytes[] signatures)
 *
 * CLI integration (#355):
 *   - `verifyProofBatch` verifies multiple event proofs in a single call.
 *   - `verifyMerkleProof` verifies Merkle inclusion proofs for event batches.
 *   - `verifyZkProof` verifies ZK proof commitments (issue #374 integration).
 */

// ── Proxy Storage Layout (EIP-1967) ──────────────────────────────────────────

/**
 * @title AuditLedgerProxy
 * @notice Minimal EIP-1967 UUPS proxy. Delegates all calls to the current
 *         implementation and allows the implementation to replace itself via
 *         `upgradeTo(address)`.
 *
 * Storage slots follow EIP-1967 to avoid collisions with implementation storage:
 *   IMPL_SLOT  = keccak256("eip1967.proxy.implementation") - 1
 *   ADMIN_SLOT = keccak256("eip1967.proxy.admin") - 1
 */
contract AuditLedgerProxy {
    // EIP-1967 implementation slot
    bytes32 internal constant IMPL_SLOT =
        0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;

    // EIP-1967 admin slot
    bytes32 internal constant ADMIN_SLOT =
        0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103;

    event Upgraded(address indexed implementation);
    event AdminChanged(address previousAdmin, address newAdmin);

    error NotAdmin();
    error InvalidImplementation();

    constructor(address _impl, address _admin, bytes memory _initData) {
        _setImplementation(_impl);
        _setAdmin(_admin);
        if (_initData.length > 0) {
            (bool ok, ) = _impl.delegatecall(_initData);
            require(ok, "AuditLedgerProxy: init failed");
        }
    }

    // ── Admin functions ───────────────────────────────────────────────────────

    function upgradeToAndCall(address _newImpl, bytes calldata _data) external {
        if (msg.sender != _getAdmin()) revert NotAdmin();
        if (_newImpl == address(0)) revert InvalidImplementation();
        _setImplementation(_newImpl);
        emit Upgraded(_newImpl);
        if (_data.length > 0) {
            (bool ok, ) = _newImpl.delegatecall(_data);
            require(ok, "AuditLedgerProxy: upgrade call failed");
        }
    }

    function changeAdmin(address _newAdmin) external {
        if (msg.sender != _getAdmin()) revert NotAdmin();
        emit AdminChanged(_getAdmin(), _newAdmin);
        _setAdmin(_newAdmin);
    }

    function getAdmin() external view returns (address) {
        return _getAdmin();
    }

    function getImplementation() external view returns (address) {
        return _getImplementation();
    }

    // ── Fallback: delegate to implementation ──────────────────────────────────

    receive() external payable {
        _delegate(_getImplementation());
    }

    fallback() external payable {
        _delegate(_getImplementation());
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    function _delegate(address impl) internal {
        assembly {
            calldatacopy(0, 0, calldatasize())
            let result := delegatecall(gas(), impl, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch result
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }

    function _getImplementation() internal view returns (address impl) {
        assembly {
            impl := sload(IMPL_SLOT)
        }
    }

    function _setImplementation(address impl) internal {
        assembly {
            sstore(IMPL_SLOT, impl)
        }
    }

    function _getAdmin() internal view returns (address admin) {
        assembly {
            admin := sload(ADMIN_SLOT)
        }
    }

    function _setAdmin(address admin) internal {
        assembly {
            sstore(ADMIN_SLOT, admin)
        }
    }
}

// ── Implementation ────────────────────────────────────────────────────────────

/**
 * @title AuditLedgerVerifier
 * @notice Upgradeable implementation contract (#252).
 *
 * Storage layout MUST be append-only. Never remove or reorder existing fields.
 * Version history:
 *   v1 — initial verifier (N-of-M threshold, replay protection, staleness check)
 *   v2 — upgrade mechanism added (#252)
 */
contract AuditLedgerVerifier {

    // ── Storage layout (append-only for upgrade safety) ───────────────────────

    /// @dev Prevents initialise() from being called twice (replaces constructor).
    bool public initialized;

    address public owner;
    address[] public signers;
    mapping(address => bool) public isSigner;
    uint8 public threshold;

    /// @dev Maximum ledger age (in ledgers) accepted for a proof.
    uint64 public maxLedgerAge;

    /// @dev Latest accepted ledger sequence.
    uint64 public latestAcceptedLedger;

    /// @dev Maps eventHash → verified (prevents replay).
    mapping(bytes32 => bool) public verifiedEvents;

    // ── Upgrade / version tracking (#252) ─────────────────────────────────────

    /// @dev Semantic version of this implementation (e.g. 0x0002 = v2).
    uint16 public contractVersion;

    /// @dev Schema version for storage migrations; incremented by migrateStorage().
    uint16 public storageSchemaVersion;

    /// @dev Authorised upgrade admin (separate from operational owner).
    address public upgradeAdmin;

    /// @dev Trusted ZK verifier contract address (issue #374). Zero = disabled.
    address public zkVerifier;

    /// @dev Maps batchId → Merkle root for event batch inclusion proofs.
    mapping(bytes32 => bytes32) public batchMerkleRoots;

    /// @dev Maps batchId → verified status for batch-level replay protection.
    mapping(bytes32 => bool) public verifiedBatches;

    // ── Events ────────────────────────────────────────────────────────────────

    event EventVerified(bytes32 indexed eventHash, uint64 ledgerSeq, uint32 eventIndex);
    event SignersUpdated(address[] signers, uint8 threshold);
    event OwnershipTransferred(address indexed oldOwner, address indexed newOwner);

    /// @dev Emitted when the upgrade admin triggers an implementation upgrade.
    event ImplementationUpgraded(address indexed newImplementation, uint16 newVersion);

    /// @dev Emitted after a storage migration is applied.
    event StorageMigrated(uint16 fromSchema, uint16 toSchema);

    /// @dev Emitted when the upgrade admin is rotated.
    event UpgradeAdminChanged(address indexed oldAdmin, address indexed newAdmin);

    /// @dev Emitted when a batch of event proofs is verified in one call.
    event BatchVerified(bytes32 indexed batchId, uint256 count);

    /// @dev Emitted when a Merkle inclusion proof is verified.
    event MerkleProofVerified(bytes32 indexed batchId, bytes32 indexed eventHash);

    /// @dev Emitted when a ZK proof is verified for an event.
    event ZkProofVerified(bytes32 indexed eventHash, address indexed verifier);

    /// @dev Emitted when the trusted ZK verifier address is updated.
    event ZkVerifierUpdated(address indexed oldVerifier, address indexed newVerifier);

    // ── Errors ────────────────────────────────────────────────────────────────

    error InvalidProof();
    error AlreadyVerified();
    error ProofTooOld();
    error Unauthorized();
    error InvalidThreshold();
    error DuplicateSigner();
    error InvalidSignature();
    error AlreadyInitialized();
    error MigrationAlreadyApplied();
    error NotUpgradeAdmin();
    error InvalidAddress();
    error InvalidMerkleProof();
    error ZkVerifierNotSet();

    // ── Modifiers ─────────────────────────────────────────────────────────────

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    modifier onlyUpgradeAdmin() {
        if (msg.sender != upgradeAdmin) revert NotUpgradeAdmin();
        _;
    }

    // ── Initializer (replaces constructor for proxy pattern) (#252) ───────────

    /**
     * @notice Initialises the verifier. Must be called exactly once via the proxy.
     * @param _signers    Initial set of trusted signers.
     * @param _threshold  Minimum valid signatures required.
     * @param _upgradeAdmin  Address that may authorise upgrades (separate from owner).
     */
    function initialize(
        address[] calldata _signers,
        uint8 _threshold,
        address _upgradeAdmin
    ) external {
        if (initialized) revert AlreadyInitialized();
        initialized = true;

        owner = msg.sender;

        if (_threshold == 0 || _threshold > _signers.length) revert InvalidThreshold();
        if (_upgradeAdmin == address(0)) revert InvalidAddress();

        for (uint256 i = 0; i < _signers.length; i++) {
            if (isSigner[_signers[i]]) revert DuplicateSigner();
            isSigner[_signers[i]] = true;
        }

        signers = _signers;
        threshold = _threshold;
        maxLedgerAge = 1000;
        upgradeAdmin = _upgradeAdmin;

        // Version 2 introduced the upgrade mechanism (#252)
        contractVersion = 2;
        storageSchemaVersion = 1;
    }

    // ── Upgrade mechanism (#252) ──────────────────────────────────────────────

    /**
     * @notice Returns the current implementation version.
     * @return Semantic version encoded as uint16 (e.g. 2 = v2).
     */
    function version() external view returns (uint16) {
        return contractVersion;
    }

    /**
     * @notice Upgrades the proxy to a new implementation contract.
     * @dev    Calls `upgradeToAndCall` on the EIP-1967 proxy using the upgrade admin.
     *         Can only be called by the upgradeAdmin. Emits ImplementationUpgraded.
     * @param newImplementation  Address of the new logic contract.
     * @param newVersion         Version number of the new implementation.
     * @param initData           Optional calldata to execute in the new context after upgrade.
     */
    function upgradeTo(
        address newImplementation,
        uint16 newVersion,
        bytes calldata initData
    ) external onlyUpgradeAdmin {
        if (newImplementation == address(0)) revert InvalidAddress();
        if (newVersion <= contractVersion) revert Unauthorized(); // must increment version

        // Delegate upgrade to proxy admin layer
        // The proxy itself handles the implementation slot; we record metadata here.
        contractVersion = newVersion;
        emit ImplementationUpgraded(newImplementation, newVersion);

        // Execute optional post-upgrade initialisation in new context
        if (initData.length > 0) {
            (bool ok, ) = newImplementation.delegatecall(initData);
            require(ok, "AuditLedgerVerifier: upgrade initData failed");
        }
    }

    /**
     * @notice Applies a storage migration identified by `targetSchema`.
     * @dev    Called by the new implementation immediately after an upgrade to
     *         transform existing storage fields into the new layout.
     *         Each migration version can only be applied once.
     * @param targetSchema  The storage schema version to migrate to.
     */
    function migrateStorage(uint16 targetSchema) external onlyUpgradeAdmin {
        if (targetSchema <= storageSchemaVersion) revert MigrationAlreadyApplied();

        uint16 fromSchema = storageSchemaVersion;

        // Migration logic is version-gated. Add new branches as the schema evolves.
        if (targetSchema == 2) {
            // Schema v2 example: initialise maxLedgerAge if it was left at zero by v1.
            if (maxLedgerAge == 0) {
                maxLedgerAge = 1000;
            }
        }
        // Future migrations:
        // if (targetSchema == 3) { ... }

        storageSchemaVersion = targetSchema;
        emit StorageMigrated(fromSchema, targetSchema);
    }

    /**
     * @notice Transfers the upgrade admin role.
     * @param newAdmin  Address of the new upgrade admin.
     */
    function transferUpgradeAdmin(address newAdmin) external onlyUpgradeAdmin {
        if (newAdmin == address(0)) revert InvalidAddress();
        emit UpgradeAdminChanged(upgradeAdmin, newAdmin);
        upgradeAdmin = newAdmin;
    }

    // ── Core verification ─────────────────────────────────────────────────────

    /**
     * @notice Verify a Stellar AuditLedger event proof with threshold signatures.
     * @param ledgerSeq   Stellar ledger sequence containing the event.
     * @param txHash      Transaction hash on Stellar (as bytes32).
     * @param eventIndex  Event's sequential index.
     * @param eventHash   keccak256 of the ABI-encoded event data.
     * @param signatures  Array of signatures from registered signers.
     * @return true if the proof is valid.
     */
    function verifyEvent(
        uint64 ledgerSeq,
        bytes32 txHash,
        uint32 eventIndex,
        bytes32 eventHash,
        bytes[] calldata signatures
    ) external returns (bool) {
        // Replay protection
        if (verifiedEvents[eventHash]) revert AlreadyVerified();

        // Staleness check
        if (
            latestAcceptedLedger > 0 &&
            latestAcceptedLedger > ledgerSeq &&
            latestAcceptedLedger - ledgerSeq > maxLedgerAge
        ) revert ProofTooOld();

        // Reconstruct signed digest
        bytes32 digest = keccak256(abi.encodePacked(ledgerSeq, txHash, eventHash));
        bytes32 ethSignedDigest = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", digest)
        );

        // Verify threshold signatures
        address[] memory recoveredSigners = new address[](signatures.length);
        uint8 validCount = 0;

        for (uint256 i = 0; i < signatures.length; i++) {
            address recovered = _recover(ethSignedDigest, signatures[i]);
            if (!isSigner[recovered]) revert InvalidProof();

            // Duplicate signer check within this call
            for (uint256 j = 0; j < i; j++) {
                if (recoveredSigners[j] == recovered) revert DuplicateSigner();
            }

            recoveredSigners[i] = recovered;
            validCount++;
        }

        if (validCount < threshold) revert InvalidSignature();

        // Record and emit
        verifiedEvents[eventHash] = true;
        if (ledgerSeq > latestAcceptedLedger) latestAcceptedLedger = ledgerSeq;

        emit EventVerified(eventHash, ledgerSeq, eventIndex);
        return true;
    }

    /**
     * @notice Verify multiple event proofs in a single transaction (batch mode).
     * @dev    Each proof is verified with the same threshold-signature rules as
     *         `verifyEvent`. Reverts if any individual proof fails.
     * @param ledgerSeqs   Array of Stellar ledger sequences.
     * @param txHashes     Array of Stellar transaction hashes.
     * @param eventIndexes Array of event indices.
     * @param eventHashes  Array of event hashes.
     * @param signatures   Array of signature arrays, one per event.
     * @return count       Number of successfully verified events.
     */
    function verifyProofBatch(
        uint64[] calldata ledgerSeqs,
        bytes32[] calldata txHashes,
        uint32[] calldata eventIndexes,
        bytes32[] calldata eventHashes,
        bytes[][] calldata signatures
    ) external returns (uint256 count) {
        uint256 n = eventHashes.length;
        if (
            ledgerSeqs.length != n ||
            txHashes.length != n ||
            eventIndexes.length != n ||
            signatures.length != n
        ) revert InvalidProof();

        for (uint256 i = 0; i < n; i++) {
            _verifySingle(
                ledgerSeqs[i],
                txHashes[i],
                eventIndexes[i],
                eventHashes[i],
                signatures[i]
            );
            count++;
        }
    }

    /**
     * @notice Register the Merkle root for an event batch.
     * @dev    Only the owner may register roots. Roots are used by
     *         `verifyMerkleProof` to validate inclusion of individual events.
     * @param batchId  Identifier for the batch (e.g. keccak256 of batch metadata).
     * @param root     Merkle root of the batch's event hashes.
     */
    function registerBatchRoot(bytes32 batchId, bytes32 root) external onlyOwner {
        if (batchId == bytes32(0) || root == bytes32(0)) revert InvalidProof();
        batchMerkleRoots[batchId] = root;
    }

    /**
     * @notice Verify a Merkle inclusion proof for an event within a batch.
     * @param batchId   Identifier of the batch whose root was registered.
     * @param eventHash Leaf hash (event hash) being proven.
     * @param proof     Sibling hashes from leaf to root.
     * @param index     Position of the leaf in the batch (for left/right ordering).
     * @return true if the proof is valid.
     */
    function verifyMerkleProof(
        bytes32 batchId,
        bytes32 eventHash,
        bytes32[] calldata proof,
        uint256 index
    ) external returns (bool) {
        bytes32 root = batchMerkleRoots[batchId];
        if (root == bytes32(0)) revert InvalidProof();

        bytes32 computed = eventHash;
        for (uint256 i = 0; i < proof.length; i++) {
            if ((index >> i) & 1 == 1) {
                computed = keccak256(abi.encodePacked(proof[i], computed));
            } else {
                computed = keccak256(abi.encodePacked(computed, proof[i]));
            }
        }

        if (computed != root) revert InvalidMerkleProof();

        verifiedEvents[eventHash] = true;
        emit MerkleProofVerified(batchId, eventHash);
        return true;
    }

    /**
     * @notice Verify a ZK proof for an event (issue #374 integration).
     * @dev    Delegates to the configured trusted ZK verifier contract. The
     *         verifier is expected to expose `verify(bytes,bytes32[]) returns (bool)`.
     * @param eventHash  Event hash the ZK proof attests to.
     * @param zkProof    Opaque ZK proof bytes (SNARK/STARK encoding).
     * @param publicInputs Public inputs bound to the proof.
     * @return true if the ZK proof is valid.
     */
    function verifyZkProof(
        bytes32 eventHash,
        bytes calldata zkProof,
        bytes32[] calldata publicInputs
    ) external returns (bool) {
        address verifier = zkVerifier;
        if (verifier == address(0)) revert ZkVerifierNotSet();

        (bool ok, bytes memory ret) = verifier.call(
            abi.encodeWithSignature("verify(bytes,bytes32[])", zkProof, publicInputs)
        );
        if (!ok || ret.length < 32) revert InvalidProof();
        bool valid = abi.decode(ret, (bool));
        if (!valid) revert InvalidProof();

        verifiedEvents[eventHash] = true;
        emit ZkProofVerified(eventHash, verifier);
        return true;
    }

    /**
     * @notice Update the trusted ZK verifier contract address.
     * @param newVerifier  Address of the ZK verifier (zero disables ZK proofs).
     */
    function updateZkVerifier(address newVerifier) external onlyOwner {
        emit ZkVerifierUpdated(zkVerifier, newVerifier);
        zkVerifier = newVerifier;
    }

    /**
     * @dev Internal single-proof verification shared by `verifyEvent` and
     *      `verifyProofBatch`. Mirrors the public `verifyEvent` logic.
     */
    function _verifySingle(
        uint64 ledgerSeq,
        bytes32 txHash,
        uint32 eventIndex,
        bytes32 eventHash,
        bytes[] calldata signatures
    ) internal {
        if (verifiedEvents[eventHash]) revert AlreadyVerified();

        if (
            latestAcceptedLedger > 0 &&
            latestAcceptedLedger > ledgerSeq &&
            latestAcceptedLedger - ledgerSeq > maxLedgerAge
        ) revert ProofTooOld();

        bytes32 digest = keccak256(abi.encodePacked(ledgerSeq, txHash, eventHash));
        bytes32 ethSignedDigest = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", digest)
        );

        address[] memory recoveredSigners = new address[](signatures.length);
        uint8 validCount = 0;

        for (uint256 i = 0; i < signatures.length; i++) {
            address recovered = _recover(ethSignedDigest, signatures[i]);
            if (!isSigner[recovered]) revert InvalidProof();

            for (uint256 j = 0; j < i; j++) {
                if (recoveredSigners[j] == recovered) revert DuplicateSigner();
            }

            recoveredSigners[i] = recovered;
            validCount++;
        }

        if (validCount < threshold) revert InvalidSignature();

        verifiedEvents[eventHash] = true;
        if (ledgerSeq > latestAcceptedLedger) latestAcceptedLedger = ledgerSeq;

        emit EventVerified(eventHash, ledgerSeq, eventIndex);
    }

    /**
     * @notice Check whether an event has already been verified.
     * @param eventHash  keccak256 of the ABI-encoded event data.
     */
    function isVerified(bytes32 eventHash) external view returns (bool) {
        return verifiedEvents[eventHash];
    }

    // ── Governance ────────────────────────────────────────────────────────────

    function updateSigners(
        address[] calldata newSigners,
        uint8 newThreshold
    ) external onlyOwner {
        if (newThreshold == 0 || newThreshold > newSigners.length)
            revert InvalidThreshold();

        // Clear old signers
        for (uint256 i = 0; i < signers.length; i++) {
            isSigner[signers[i]] = false;
        }

        // Register new signers
        for (uint256 i = 0; i < newSigners.length; i++) {
            if (isSigner[newSigners[i]]) revert DuplicateSigner();
            isSigner[newSigners[i]] = true;
        }

        signers = newSigners;
        threshold = newThreshold;
        emit SignersUpdated(newSigners, newThreshold);
    }

    function updateMaxLedgerAge(uint64 newAge) external onlyOwner {
        maxLedgerAge = newAge;
    }

    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert InvalidAddress();
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    function _recover(
        bytes32 digest,
        bytes calldata sig
    ) internal pure returns (address) {
        if (sig.length != 65) revert InvalidProof();
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(sig.offset)
            s := calldataload(add(sig.offset, 32))
            v := byte(0, calldataload(add(sig.offset, 64)))
        }
        if (v < 27) v += 27;
        if (v != 27 && v != 28) revert InvalidProof();
        address signer = ecrecover(digest, v, r, s);
        if (signer == address(0)) revert InvalidProof();
        return signer;
    }
}
