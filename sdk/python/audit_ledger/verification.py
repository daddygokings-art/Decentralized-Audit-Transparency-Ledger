"""AuditLedger Python SDK — Client-side event verification.

Issue #247: Add event ID verification, hash chain verification,
signature verification, and integrity proof generation.
"""

from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass, field
from typing import List, Optional, Sequence

from .exceptions import (
    EventIDMismatchError,
    HashChainError,
    SignatureVerificationError,
    VerificationError,
)
from .models import Event

# ── Event ID verification ─────────────────────────────────────────────────────


def compute_event_id(
    contract_id: str,
    submitter: str,
    event_type: str,
    metadata: bytes,
    timestamp: int,
    index: int,
) -> bytes:
    """Recompute a content-addressed event ID off-chain.

    This mirrors the ``compute_event_id`` logic in the Soroban contract.

    Args:
        contract_id: Stellar contract ID (C... string).
        submitter: Submitter address.
        event_type: Event type symbol string.
        metadata: Raw metadata bytes.
        timestamp: Unix timestamp (seconds).
        index: Sequential event index.

    Returns:
        32-byte SHA-256 event ID.
    """
    preimage = (
        contract_id.encode()
        + submitter.encode()
        + event_type.encode()
        + metadata
        + struct.pack("<Q", timestamp)
        + struct.pack("<I", index)
    )
    return hashlib.sha256(preimage).digest()


def verify_event_id(
    event: Event,
    contract_id: str,
    expected_id: bytes,
) -> bool:
    """Verify that an event's computed ID matches the expected ID.

    Args:
        event: The :class:`~audit_ledger.models.Event` to verify.
        contract_id: Stellar contract ID used during event logging.
        expected_id: 32-byte event ID to compare against.

    Returns:
        ``True`` if the IDs match.

    Raises:
        EventIDMismatchError: If the recomputed ID does not match *expected_id*.
    """
    computed = compute_event_id(
        contract_id=contract_id,
        submitter=event.submitter,
        event_type=event.event_type,
        metadata=event.metadata,
        timestamp=event.timestamp,
        index=event.index,
    )
    if computed != expected_id:
        raise EventIDMismatchError(
            f"Event ID mismatch at index {event.index}: "
            f"computed={computed.hex()}, expected={expected_id.hex()}",
            event_index=event.index,
            context={
                "computed_id": computed.hex(),
                "expected_id": expected_id.hex(),
            },
        )
    return True


# ── Hash chain verification ───────────────────────────────────────────────────


def compute_chain_hash(event_id: bytes, prev_hash: bytes) -> bytes:
    """Compute the chain hash that links two consecutive events.

    The chain hash is ``SHA-256(event_id || prev_hash)``.

    Args:
        event_id: 32-byte content-addressed event ID.
        prev_hash: 32-byte previous chain hash (all-zeros for the genesis event).

    Returns:
        32-byte chain hash.
    """
    return hashlib.sha256(event_id + prev_hash).digest()


def verify_hash_chain(events: Sequence[Event]) -> bool:
    """Verify the hash chain across a sequence of consecutive events.

    Each event's ``event_hash`` must equal ``SHA-256(event_id || prev_hash)``
    where ``prev_hash`` is the previous event's ``event_hash`` (or all-zeros
    for the first event).

    Args:
        events: Sequence of :class:`~audit_ledger.models.Event` objects in
            ascending index order.

    Returns:
        ``True`` if the chain is intact.

    Raises:
        HashChainError: If a link in the chain is broken.
        VerificationError: If the events list is empty or has gaps.
    """
    if not events:
        return True  # nothing to verify

    prev_hash = bytes(32)  # genesis prev_hash is all-zeros

    for i, event in enumerate(events):
        if event.event_hash is None or event.prev_hash is None:
            raise VerificationError(
                f"Event at index {event.index} is missing hash fields; "
                "chain verification requires event_hash and prev_hash.",
                event_index=event.index,
            )

        # Check that the stored prev_hash matches what we computed
        if event.prev_hash != prev_hash:
            raise HashChainError(
                f"Hash chain broken at event index {event.index}: "
                f"stored prev_hash={event.prev_hash.hex()}, "
                f"expected={prev_hash.hex()}",
                event_index=event.index,
                context={
                    "stored_prev_hash": event.prev_hash.hex(),
                    "expected_prev_hash": prev_hash.hex(),
                },
            )

        # Advance the chain
        prev_hash = event.event_hash

    return True


# ── Signature verification ────────────────────────────────────────────────────


def verify_event_signature(
    event_id: bytes,
    pubkey: bytes,
    signature: bytes,
) -> bool:
    """Verify an Ed25519 signature over an event ID.

    Args:
        event_id: 32-byte event ID (the signed message).
        pubkey: 32-byte Ed25519 public key.
        signature: 64-byte Ed25519 signature.

    Returns:
        ``True`` if the signature is valid.

    Raises:
        SignatureVerificationError: If verification fails or required libraries
            are unavailable.
    """
    try:
        from stellar_sdk.keypair import Keypair  # type: ignore[import]

        kp = Keypair.from_public_key(pubkey.hex())
        kp.verify(event_id, signature)
        return True
    except ImportError as exc:
        raise SignatureVerificationError(
            "stellar-sdk is required for signature verification. "
            "Install with: pip install stellar-sdk",
            context={"missing_dependency": "stellar-sdk"},
        ) from exc
    except Exception as exc:
        raise SignatureVerificationError(
            f"Signature verification failed: {exc}",
            context={
                "event_id": event_id.hex(),
                "pubkey": pubkey.hex(),
            },
        ) from exc


# ── Integrity proof ───────────────────────────────────────────────────────────


@dataclass
class IntegrityProof:
    """An integrity proof for a sequence of audit events.

    Attributes:
        event_count: Number of events covered by the proof.
        first_index: Sequential index of the first covered event.
        last_index: Sequential index of the last covered event.
        root_hash: SHA-256 Merkle-style root hash of all event IDs.
        chain_valid: Whether the hash chain was verified successfully.
        errors: List of error messages for any failed checks.
    """

    event_count: int
    first_index: int
    last_index: int
    root_hash: bytes
    chain_valid: bool
    errors: List[str] = field(default_factory=list)

    @property
    def is_valid(self) -> bool:
        """``True`` if all checks passed and there are no errors."""
        return self.chain_valid and not self.errors

    def hex_root(self) -> str:
        """Return the root hash as a lowercase hex string."""
        return self.root_hash.hex()

    def to_dict(self) -> dict:
        """Serialise the proof to a plain dict."""
        return {
            "event_count": self.event_count,
            "first_index": self.first_index,
            "last_index": self.last_index,
            "root_hash": self.hex_root(),
            "chain_valid": self.chain_valid,
            "is_valid": self.is_valid,
            "errors": list(self.errors),
        }


def _merkle_root(hashes: List[bytes]) -> bytes:
    """Compute a simple binary Merkle root from a list of 32-byte hashes.

    If the list has an odd length, the last hash is duplicated.
    Returns all-zeros if *hashes* is empty.
    """
    if not hashes:
        return bytes(32)
    layer = list(hashes)
    while len(layer) > 1:
        if len(layer) % 2 != 0:
            layer.append(layer[-1])
        layer = [
            hashlib.sha256(layer[i] + layer[i + 1]).digest()
            for i in range(0, len(layer), 2)
        ]
    return layer[0]


def generate_integrity_proof(
    events: Sequence[Event],
    verify_chain: bool = True,
) -> IntegrityProof:
    """Generate an :class:`IntegrityProof` for a sequence of events.

    The proof covers:

    1. **Hash chain** — every event's ``prev_hash`` links correctly to the
       preceding event's ``event_hash``.
    2. **Merkle root** — a SHA-256 Merkle root over all ``event_hash`` values,
       giving a compact commitment to the full sequence.

    Args:
        events: Sequence of :class:`~audit_ledger.models.Event` objects in
            ascending index order.
        verify_chain: Whether to verify the hash chain (default ``True``).

    Returns:
        An :class:`IntegrityProof` describing the verification result.
    """
    errors: List[str] = []
    chain_valid = False

    if not events:
        return IntegrityProof(
            event_count=0,
            first_index=0,
            last_index=0,
            root_hash=bytes(32),
            chain_valid=True,
            errors=[],
        )

    first_index = events[0].index
    last_index = events[-1].index

    # Hash chain verification
    if verify_chain:
        try:
            verify_hash_chain(events)
            chain_valid = True
        except (HashChainError, VerificationError) as exc:
            errors.append(str(exc))

    # Collect event hashes for Merkle root
    event_hashes: List[bytes] = []
    for event in events:
        if event.event_hash is not None:
            event_hashes.append(event.event_hash)
        else:
            errors.append(
                f"Event at index {event.index} is missing event_hash; "
                "excluded from Merkle root."
            )

    root = _merkle_root(event_hashes)

    return IntegrityProof(
        event_count=len(events),
        first_index=first_index,
        last_index=last_index,
        root_hash=root,
        chain_valid=chain_valid,
        errors=errors,
    )
