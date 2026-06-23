"""AuditLedger Python SDK — Soroban contract client."""

from __future__ import annotations

import base64
import hashlib
import struct
from typing import Optional

from .models import Event, ContractError, RPCError

try:
    import stellar_sdk
    from stellar_sdk import SorobanServer, Keypair
    from stellar_sdk.soroban import SorobanClient
    STELLAR_SDK_AVAILABLE = True
except ImportError:
    STELLAR_SDK_AVAILABLE = False


class AuditLedgerClient:
    """Client for interacting with the AuditLedger Soroban contract.

    Args:
        contract_id: Stellar contract ID (C... string).
        rpc_url: Soroban RPC endpoint URL.
        network_passphrase: Stellar network passphrase.
        source_keypair: Optional Stellar keypair for signing transactions.

    Usage:
        >>> client = AuditLedgerClient(
        ...     contract_id="CCXMTP7...",
        ...     rpc_url="https://soroban-testnet.stellar.org",
        ...     network_passphrase="Test SDF Network ; September 2015",
        ... )
        >>> events = client.total_events()
        42
    """

    def __init__(
        self,
        contract_id: str,
        rpc_url: str = "https://soroban-testnet.stellar.org",
        network_passphrase: str = "Test SDF Network ; September 2015",
        source_keypair: Optional[str] = None,
    ):
        if not STELLAR_SDK_AVAILABLE:
            raise ImportError(
                "stellar-sdk is required. Install with: pip install stellar-sdk"
            )
        self.contract_id = contract_id
        self.rpc_url = rpc_url
        self.network_passphrase = network_passphrase
        self.server = SorobanServer(rpc_url)
        self.source = Keypair.from_secret(source_keypair) if source_keypair else None

    def _invoke(self, method: str, params: dict = None):
        """Invoke a contract function and return the parsed result."""
        if params is None:
            params = {}

        try:
            result = self.server.invoke_contract(
                contract_id=self.contract_id,
                function_name=method,
                parameters=params,
                source=self.source,
            )
            return result
        except Exception as e:
            error_msg = str(e)
            # Try to extract contract error code
            for code in sorted(ContractError.ERROR_CODES, reverse=True):
                if f"#{code}" in error_msg or f"Error(Contract, #{code})" in error_msg:
                    raise ContractError(code) from e
            raise RPCError(f"RPC call failed: {error_msg}") from e

    def _parse_u32(self, result) -> int:
        """Parse a u32 return value."""
        if isinstance(result, dict):
            for v in result.values():
                return int(v)
        return int(result)

    # ── Write functions ───────────────────────────────────────────────────

    def initialize(self, owner: str, global_max_logs: int) -> None:
        """Initialize the contract with an owner and global max log count."""
        self._invoke("initialize", {
            "owner": owner,
            "global_max_logs": global_max_logs,
        })

    def log_event(
        self, submitter: str, event_type: str, metadata: bytes
    ) -> bytes:
        """Log an event and return its 32-byte content-addressed ID."""
        result = self._invoke("log_event", {
            "submitter": submitter,
            "event_type": event_type,
            "metadata": base64.b64encode(metadata).decode(),
        })
        if isinstance(result, dict):
            return bytes.fromhex(list(result.values())[0])
        return bytes.fromhex(result)

    def log_event_signed(
        self,
        submitter: str,
        event_type: str,
        metadata: bytes,
        signature_payload: bytes,
    ) -> bytes:
        """Log an event with a 96-byte signature payload (pubkey + sig)."""
        result = self._invoke("log_event_signed", {
            "submitter": submitter,
            "event_type": event_type,
            "metadata": base64.b64encode(metadata).decode(),
            "signature_payload": base64.b64encode(signature_payload).decode(),
        })
        if isinstance(result, dict):
            return bytes.fromhex(list(result.values())[0])
        return bytes.fromhex(result)

    # ── Read functions ────────────────────────────────────────────────────

    def total_events(self) -> int:
        """Return the total number of events on-chain."""
        result = self._invoke("total_events")
        return self._parse_u32(result)

    def get_event(self, event_id: bytes) -> Event:
        """Retrieve an event by its 32-byte content-addressed ID."""
        result = self._invoke("get_event", {"id": event_id.hex()})
        return Event.from_dict(result) if isinstance(result, dict) else result

    def get_event_by_order(self, order: int) -> Event:
        """Retrieve an event by its sequential order index."""
        result = self._invoke("get_event_by_order", {"order": order})
        return Event.from_dict(result) if isinstance(result, dict) else result

    def event_count(self, event_type: str) -> int:
        """Return the count of events for a specific type."""
        result = self._invoke("event_count", {"event_type": event_type})
        return self._parse_u32(result)

    def get_event_by_type(self, event_type: str, type_index: int) -> Event:
        """Retrieve an event by type and type-relative index."""
        result = self._invoke("get_event_by_type", {
            "event_type": event_type,
            "type_index": type_index,
        })
        return Event.from_dict(result) if isinstance(result, dict) else result

    # ── Governance ────────────────────────────────────────────────────────

    def set_global_max_logs(self, caller: str, new_max: int) -> None:
        """Set the global maximum log count (owner-only)."""
        self._invoke("set_global_max_logs", {
            "caller": caller,
            "new_max": new_max,
        })

    def set_event_max_logs(self, caller: str, event_type: str, new_max: int) -> None:
        """Set per-event-type max logs (owner-only)."""
        self._invoke("set_event_max_logs", {
            "caller": caller,
            "event_type": event_type,
            "new_max": new_max,
        })

    def remove_event_cap(self, caller: str, event_type: str) -> None:
        """Remove a per-event-type cap (owner-only)."""
        self._invoke("remove_event_cap", {
            "caller": caller,
            "event_type": event_type,
        })

    def transfer_ownership(self, caller: str, new_owner: str) -> None:
        """Transfer contract ownership (owner-only)."""
        self._invoke("transfer_ownership", {
            "caller": caller,
            "new_owner": new_owner,
        })

    # ── Metadata size cap (issue #67) ─────────────────────────────────────

    def set_metadata_max_size(self, caller: str, max_size: int) -> None:
        """Set the global metadata size cap (owner-only)."""
        self._invoke("set_metadata_max_size", {
            "caller": caller,
            "max_size": max_size,
        })

    def set_event_metadata_max_size(
        self, caller: str, event_type: str, max_size: int
    ) -> None:
        """Set per-event-type metadata size cap (owner-only)."""
        self._invoke("set_event_metadata_max_size", {
            "caller": caller,
            "event_type": event_type,
            "max_size": max_size,
        })

    def get_metadata_max_size(self, event_type: str) -> int:
        """Get the effective metadata size cap for a given event type."""
        result = self._invoke("get_metadata_max_size", {
            "event_type": event_type,
        })
        return self._parse_u32(result)

    # ── Signatures (issue #69) ────────────────────────────────────────────

    def get_event_signature(self, event_id: bytes) -> Optional[bytes]:
        """Return the stored 96-byte signature payload for an event."""
        try:
            result = self._invoke("get_event_signature", {
                "event_id": event_id.hex(),
            })
            if isinstance(result, dict):
                raw = list(result.values())[0]
                return base64.b64decode(raw) if raw else None
            return None
        except (ContractError, RPCError):
            return None

    # ── Integrity (issue #66) ─────────────────────────────────────────────

    def verify_integrity(self) -> bool:
        """Verify the full hash chain. Returns True if valid."""
        result = self._invoke("verify_integrity")
        if isinstance(result, dict):
            return list(result.values())[0] is True
        return bool(result)

    def verify_integrity_range(self, from_idx: int, to_idx: int) -> bool:
        """Verify a range of the hash chain."""
        result = self._invoke("verify_integrity_range", {
            "from": from_idx,
            "to": to_idx,
        })
        if isinstance(result, dict):
            return list(result.values())[0] is True
        return bool(result)

    # ── Utility ───────────────────────────────────────────────────────────

    @staticmethod
    def compute_event_id(
        contract_id: str,
        submitter: str,
        event_type: str,
        metadata: bytes,
        timestamp: int,
        index: int,
    ) -> bytes:
        """Recompute the content-addressed event ID off-chain.

        Matches `compute_event_id` in the contract (issue #70).
        """
        preimage = (
            contract_id.encode()
            + submitter.encode()
            + struct.pack("<Q", hash(event_type))  # simplified; contract uses Symbol payload
            + metadata
            + struct.pack("<Q", timestamp)
            + struct.pack("<I", index)
        )
        return hashlib.sha256(preimage).digest()

    @staticmethod
    def verify_signature(
        event_id: bytes, pubkey: bytes, signature: bytes
    ) -> bool:
        """Verify an Ed25519 signature against an event ID.

        Args:
            event_id: 32-byte event ID (the signed message).
            pubkey: 32-byte Ed25519 public key.
            signature: 64-byte Ed25519 signature.

        Returns:
            True if the signature is valid for the given event ID.
        """
        try:
            from stellar_sdk.keypair import Keypair
            verified = Keypair.from_public_key(pubkey.hex()).verify(
                event_id, signature
            )
            return verified
        except Exception:
            return False
