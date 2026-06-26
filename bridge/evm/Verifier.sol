// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title AuditLedger Cross-Chain Verifier (#79)
 * @notice Verifies Stellar AuditLedger event proofs on EVM chains.
 *
 * Trust model (testnet):
 *   A single trusted relayer key signs each proof. The Verifier recovers the
 *   signer from the ECDSA signature and checks it against `trustedSigner`.
 *   For production, replace with threshold / quorum verification against the
 *   full Stellar validator set.
 *
 * Proof format:
 *   (uint64 ledgerSeq, bytes32 txHash, uint32 eventIndex,
 *    bytes32 eventHash, bytes signature)
 */
contract AuditLedgerVerifier {
    // ── Storage ──────────────────────────────────────────────────────────────

    address public owner;
    address public trustedSigner;

    /// @dev Maximum ledger age (in ledgers) accepted for a proof.
    uint64 public maxLedgerAge = 1000;

    /// @dev Latest accepted ledger sequence.
    uint64 public latestAcceptedLedger;

    /// @dev Maps eventHash → verified (prevents replay).
    mapping(bytes32 => bool) public verifiedEvents;

    // ── Events ────────────────────────────────────────────────────────────────

    event EventVerified(bytes32 indexed eventHash, uint64 ledgerSeq, uint32 eventIndex);
    event TrustedSignerUpdated(address indexed oldSigner, address indexed newSigner);
    event OwnershipTransferred(address indexed oldOwner, address indexed newOwner);

    // ── Errors ────────────────────────────────────────────────────────────────

    error InvalidProof();
    error AlreadyVerified();
    error ProofTooOld();
    error Unauthorized();

    // ── Constructor ───────────────────────────────────────────────────────────

    constructor(address _trustedSigner) {
        owner = msg.sender;
        trustedSigner = _trustedSigner;
    }

    // ── Modifiers ─────────────────────────────────────────────────────────────

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    // ── Core ──────────────────────────────────────────────────────────────────

    /**
     * @notice Verify a Stellar AuditLedger event proof.
     * @param ledgerSeq   Stellar ledger sequence containing the event.
     * @param txHash      Transaction hash on Stellar (as bytes32).
     * @param eventIndex  Event's sequential index.
     * @param eventHash   keccak256 of the ABI-encoded event data.
     * @param signature   65-byte ECDSA signature over the proof digest.
     * @return true if the proof is valid.
     */
    function verifyEvent(
        uint64 ledgerSeq,
        bytes32 txHash,
        uint32 eventIndex,
        bytes32 eventHash,
        bytes calldata signature
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

        // Recover signer
        address recovered = _recover(ethSignedDigest, signature);
        if (recovered != trustedSigner) revert InvalidProof();

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

    function updateTrustedSigner(address newSigner) external onlyOwner {
        emit TrustedSignerUpdated(trustedSigner, newSigner);
        trustedSigner = newSigner;
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
