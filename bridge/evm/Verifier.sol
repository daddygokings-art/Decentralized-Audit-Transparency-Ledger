// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title AuditLedger Cross-Chain Verifier (#79, #139)
 * @notice Verifies Stellar AuditLedger event proofs on EVM chains using N-of-M threshold scheme.
 *
 * Trust model:
 *   Multiple signers required to verify each proof. At least `threshold` valid signatures
 *   from the registered signer set are needed. Prevents single point of failure.
 *
 * Proof format:
 *   (uint64 ledgerSeq, bytes32 txHash, uint32 eventIndex,
 *    bytes32 eventHash, bytes[] signatures)
 */
contract AuditLedgerVerifier {
    // ── Storage ──────────────────────────────────────────────────────────────

    address public owner;
    address[] public signers;
    mapping(address => bool) public isSigner;
    uint8 public threshold;

    /// @dev Maximum ledger age (in ledgers) accepted for a proof.
    uint64 public maxLedgerAge = 1000;

    /// @dev Latest accepted ledger sequence.
    uint64 public latestAcceptedLedger;

    /// @dev Maps eventHash → verified (prevents replay).
    mapping(bytes32 => bool) public verifiedEvents;

    // ── Events ────────────────────────────────────────────────────────────────

    event EventVerified(bytes32 indexed eventHash, uint64 ledgerSeq, uint32 eventIndex);
    event SignersUpdated(address[] signers, uint8 threshold);
    event OwnershipTransferred(address indexed oldOwner, address indexed newOwner);

    // ── Errors ────────────────────────────────────────────────────────────────

    error InvalidProof();
    error AlreadyVerified();
    error ProofTooOld();
    error Unauthorized();
    error InvalidThreshold();
    error DuplicateSigner();
    error InvalidSignature();

    // ── Constructor ───────────────────────────────────────────────────────────

    constructor(address[] memory _signers, uint8 _threshold) {
        owner = msg.sender;
        if (_threshold == 0 || _threshold > _signers.length) revert InvalidThreshold();
        
        for (uint256 i = 0; i < _signers.length; i++) {
            if (isSigner[_signers[i]]) revert DuplicateSigner();
            isSigner[_signers[i]] = true;
        }
        
        signers = _signers;
        threshold = _threshold;
    }

    // ── Modifiers ─────────────────────────────────────────────────────────────

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    // ── Core ──────────────────────────────────────────────────────────────────

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
        bytes32 ethSignedDigest = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", digest));

        // Verify threshold signatures
        address[] memory recoveredSigners = new address[](signatures.length);
        uint8 validCount = 0;

        for (uint256 i = 0; i < signatures.length; i++) {
            address recovered = _recover(ethSignedDigest, signatures[i]);
            if (!isSigner[recovered]) revert InvalidProof();

            // Check for duplicates
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
     * @notice Check whether an event has already been verified.
     * @param eventHash  keccak256 of the ABI-encoded event data.
     */
    function isVerified(bytes32 eventHash) external view returns (bool) {
        return verifiedEvents[eventHash];
    }

    // ── Governance ────────────────────────────────────────────────────────────

    function updateSigners(address[] calldata newSigners, uint8 newThreshold) external onlyOwner {
        if (newThreshold == 0 || newThreshold > newSigners.length) revert InvalidThreshold();

        // Clear old signers
        for (uint256 i = 0; i < signers.length; i++) {
            isSigner[signers[i]] = false;
        }

        // Add new signers
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
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    function _recover(bytes32 digest, bytes calldata sig) internal pure returns (address) {
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
