"""AuditLedger Python SDK — Soroban contract client.

This module provides :class:`AuditLedgerClient`, the main entry-point for
interacting with the AuditLedger Soroban smart contract on Stellar.

New in this release
-------------------
- **#243** Full ``PEP 484`` type hints and type aliases throughout.
- **#244** Generator-based event streaming via :meth:`stream_events` and the
  :mod:`audit_ledger.streaming` module.
- **#245** Batch operations via :meth:`batch_submit`, :meth:`batch_get`, and
  :meth:`batch_verify`.
- **#246** LRU caching layer with invalidation, configuration, and statistics
  via the :mod:`audit_ledger.cache` module.
"""

from __future__ import annotations

import base64
import hashlib
import struct
import time
from typing import Any, Dict, Generator, List, Optional, Sequence

from .batch import (
    BatchProgress,
    BatchResult,
    BatchSubmitRequest,
    batch_get,
    batch_submit,
    batch_verify,
)
from .cache import CacheConfig, CacheStats, LRUCache
from .models import (
    Address,
    AuditLedgerError,
    ContractError,
    Event,
    EventId,
    EventType,
    Metadata,
    Page,
    RPCError,
)
from .streaming import StreamConfig, StreamError, stream_events, stream_by_type

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
        contract_id: Stellar contract ID (``C...`` string).
        rpc_url: Soroban RPC endpoint URL.
        network_passphrase: Stellar network passphrase.
        source_keypair: Optional Stellar secret key for signing transactions.
        cache_config: Optional :class:`~audit_ledger.cache.CacheConfig`.
            When omitted, a default config (256 entries, 60 s TTL) is used.

    Usage::

        >>> client = AuditLedgerClient(
        ...     contract_id="CCXMTP7...",
        ...     rpc_url="https://soroban-testnet.stellar.org",
        ...     network_passphrase="Test SDF Network ; September 2015",
        ... )
        >>> client.total_events()
        42
    """

    def __init__(
        self,
        contract_id: str,
        rpc_url: str = "https://soroban-testnet.stellar.org",
        network_passphrase: str = "Test SDF Network ; September 2015",
        source_keypair: Optional[str] = None,
        cache_config: Optional[CacheConfig] = None,
    ) -> None:
        if not STELLAR_SDK_AVAILABLE:
            raise ImportError(
                "stellar-sdk is required. Install with: pip install stellar-sdk"
            )
        self.contract_id: str = contract_id
        self.rpc_url: str = rpc_url
        self.network_passphrase: str = network_passphrase
        self.server: Any = SorobanServer(rpc_url)
        self.source: Optional[Any] = (
            Keypair.from_secret(source_keypair) if source_keypair else None
        )
        # #246 — LRU cache
        self._cache: LRUCache[Any] = LRUCache(
            cache_config or CacheConfig(max_size=256, ttl_seconds=60.0)
        )

    # ------------------------------------------------------------------
    # Cache management (#246)
    # ------------------------------------------------------------------

    def configure_cache(self, config: CacheConfig) -> None:
        """Replace the cache configuration at runtime.

        Args:
            config: New :class:`~audit_ledger.cache.CacheConfig`.
        """
        self._cache.configure(config)

    def cache_stats(self) -> CacheStats:
        """Return a snapshot of the current cache statistics.

        Returns:
            :class:`~audit_ledger.cache.CacheStats` with hit/miss counters.
        """
        return self._cache.stats

    def invalidate_cache(self, key: Optional[str] = None) -> None:
        """Invalidate one cache entry or the entire cache.

        Args:
            key: When given, remove only this key.  When ``None``, clear
                the whole cache.
        """
        if key is None:
            self._cache.clear()
        else:
            self._cache.invalidate(key)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _invoke(self, method: str, params: Optional[Dict[str, Any]] = None) -> Any:
        """Invoke a contract function and return the parsed result.

        Args:
            method: Contract function name.
            params: Dictionary of call parameters.

        Returns:
            Raw result from the RPC layer.

        Raises:
            :class:`~audit_ledger.models.ContractError`: On contract-level errors.
            :class:`~audit_ledger.models.RPCError`: On RPC/network errors.
        """
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
        except Exception as exc:
            error_msg = str(exc)
            for code in sorted(ContractError.ERROR_CODES, reverse=True):
                if f"#{code}" in error_msg or f"Error(Contract, #{code})" in error_msg:
                    raise ContractError(code) from exc
            raise RPCError(f"RPC call failed: {error_msg}") from exc

    def _parse_u32(self, result: Any) -> int:
        """Parse a ``u32`` return value from the RPC layer.

        Args:
            result: Raw RPC result (dict or scalar).

        Returns:
            Python ``int``.
        """
        if isinstance(result, dict):
            for v in result.values():
                return int(v)
        return int(result)

    # ------------------------------------------------------------------
    # Write functions
    # ------------------------------------------------------------------

    def initialize(self, owner: Address, global_max_logs: int) -> None:
        """Initialize the contract with an owner and global max log count.

        Args:
            owner: Stellar address of the contract owner.
            global_max_logs: Maximum total events allowed on-chain.
        """
        self._invoke("initialize", {
            "owner": owner,
            "global_max_logs": global_max_logs,
        })

    def log_event(
        self,
        submitter: Address,
        event_type: EventType,
        metadata: Metadata,
    ) -> EventId:
        """Log a single event and return its 32-byte content-addressed ID.

        Args:
            submitter: Stellar address of the event submitter.
            event_type: Symbolic event type string.
            metadata: Raw binary metadata for the event.

        Returns:
            32-byte :data:`~audit_ledger.models.EventId`.
        """
        result = self._invoke("log_event", {
            "submitter": submitter,
            "event_type": event_type,
            "metadata": base64.b64encode(metadata).decode(),
        })
        # Invalidate total_events cache on mutation
        self._cache.invalidate("total_events")
        if isinstance(result, dict):
            return bytes.fromhex(list(result.values())[0])
        return bytes.fromhex(result)

    def log_events(self, events: List[Dict[str, Any]]) -> List[int]:
        """Log a batch of events and return their sequential indices.

        Args:
            events: List of dicts, each with keys ``submitter``,
                ``event_type``, and ``metadata`` (bytes).

        Returns:
            List of integer sequential indices.
        """
        payload: List[Dict[str, Any]] = []
        for event in events:
            payload.append({
                "submitter": event["submitter"],
                "event_type": event["event_type"],
                "metadata": base64.b64encode(event["metadata"]).decode(),
            })
        result = self._invoke("log_events", {"events": payload})
        self._cache.invalidate("total_events")
        if isinstance(result, list):
            return [self._parse_u32(item) for item in result]
        if isinstance(result, dict):
            return [self._parse_u32(value) for value in result.values()]
        return [self._parse_u32(result)]

    def log_event_signed(
        self,
        submitter: Address,
        event_type: EventType,
        metadata: Metadata,
        signature_payload: bytes,
    ) -> EventId:
        """Log an event with a 96-byte signature payload (pubkey + sig).

        Args:
            submitter: Stellar address of the submitter.
            event_type: Symbolic event type string.
            metadata: Raw binary metadata.
            signature_payload: 96-byte Ed25519 pubkey + signature blob.

        Returns:
            32-byte :data:`~audit_ledger.models.EventId`.
        """
        result = self._invoke("log_event_signed", {
            "submitter": submitter,
            "event_type": event_type,
            "metadata": base64.b64encode(metadata).decode(),
            "signature_payload": base64.b64encode(signature_payload).decode(),
        })
        self._cache.invalidate("total_events")
        if isinstance(result, dict):
            return bytes.fromhex(list(result.values())[0])
        return bytes.fromhex(result)

    # ------------------------------------------------------------------
    # Read functions
    # ------------------------------------------------------------------

    def total_events(self) -> int:
        """Return the total number of events on-chain.

        Result is cached under the key ``"total_events"`` with the
        configured TTL.

        Returns:
            Total event count as an integer.
        """
        cached = self._cache.get("total_events")
        if cached is not None:
            return int(cached)
        result = self._invoke("total_events")
        value = self._parse_u32(result)
        self._cache.set("total_events", value)
        return value

    def get_event(self, event_id: EventId) -> Event:
        """Retrieve an event by its 32-byte content-addressed ID.

        Args:
            event_id: 32-byte event identifier.

        Returns:
            :class:`~audit_ledger.models.Event`.
        """
        cache_key = f"event_id:{event_id.hex()}"
        cached = self._cache.get(cache_key)
        if cached is not None:
            return cached  # type: ignore[return-value]
        result = self._invoke("get_event", {"id": event_id.hex()})
        event = Event.from_dict(result) if isinstance(result, dict) else result
        self._cache.set(cache_key, event)
        return event

    def get_event_by_order(self, order: int) -> Event:
        """Retrieve an event by its sequential order index.

        Args:
            order: Zero-based sequential index.

        Returns:
            :class:`~audit_ledger.models.Event`.
        """
        cache_key = f"event_order:{order}"
        cached = self._cache.get(cache_key)
        if cached is not None:
            return cached  # type: ignore[return-value]
        result = self._invoke("get_event_by_order", {"order": order})
        event = Event.from_dict(result) if isinstance(result, dict) else result
        self._cache.set(cache_key, event)
        return event

    def event_count(self, event_type: EventType) -> int:
        """Return the count of events for a specific type.

        Args:
            event_type: Symbolic event type to query.

        Returns:
            Count as an integer.
        """
        cache_key = f"event_count:{event_type}"
        cached = self._cache.get(cache_key)
        if cached is not None:
            return int(cached)
        result = self._invoke("event_count", {"event_type": event_type})
        value = self._parse_u32(result)
        self._cache.set(cache_key, value)
        return value

    def get_event_by_type(self, event_type: EventType, type_index: int) -> Event:
        """Retrieve an event by type and type-relative index.

        Args:
            event_type: Symbolic event type string.
            type_index: Zero-based index within that event type.

        Returns:
            :class:`~audit_ledger.models.Event`.
        """
        cache_key = f"event_type:{event_type}:{type_index}"
        cached = self._cache.get(cache_key)
        if cached is not None:
            return cached  # type: ignore[return-value]
        result = self._invoke("get_event_by_type", {
            "event_type": event_type,
            "type_index": type_index,
        })
        event = Event.from_dict(result) if isinstance(result, dict) else result
        self._cache.set(cache_key, event)
        return event

    def get_events(self, offset: int = 0, limit: int = 50) -> Page[Event]:
        """Return a paginated slice of all events.

        Args:
            offset: Zero-based index of the first event to return.
            limit: Maximum number of events to return.

        Returns:
            :class:`~audit_ledger.models.Page` with items, total, offset, and
            limit fields.
        """
        total = self.total_events()
        items: List[Event] = []
        end = min(offset + limit, total)
        for i in range(offset, end):
            items.append(self.get_event_by_order(i))
        return Page(items=items, total=total, offset=offset, limit=limit)

    # ------------------------------------------------------------------
    # Streaming (#244)
    # ------------------------------------------------------------------

    def stream_events(
        self,
        after_index: int = 0,
        poll_interval_s: float = 5.0,
        config: Optional[StreamConfig] = None,
    ) -> Generator[Event, None, None]:
        """Yield new :class:`~audit_ledger.models.Event` objects as they appear.

        The generator polls the chain at ``poll_interval_s`` intervals and
        yields events in ascending order.  It runs indefinitely until the
        caller breaks out of the loop or calls ``.close()``.

        Args:
            after_index: Resume from this sequential index (exclusive).  Pass
                the index of the last-seen event + 1 to resume streaming.
            poll_interval_s: Seconds to wait between polls when no new events
                are available.
            config: Optional :class:`~audit_ledger.streaming.StreamConfig`.
                When provided, ``poll_interval_s`` is taken from *config*;
                the *poll_interval_s* argument is ignored.

        Yields:
            :class:`~audit_ledger.models.Event` in chronological order.

        Example::

            cursor = 0
            for event in client.stream_events(after_index=cursor):
                handle(event)
                cursor = event.index + 1
        """
        if config is None:
            config = StreamConfig(poll_interval_s=poll_interval_s)
        yield from stream_events(self, after_index=after_index, config=config)

    def stream_by_type(
        self,
        event_type: EventType,
        after_index: int = 0,
        poll_interval_s: float = 5.0,
    ) -> Generator[Event, None, None]:
        """Yield events of a single *event_type* as they are logged.

        Args:
            event_type: Only events with this type are yielded.
            after_index: Resume cursor (see :meth:`stream_events`).
            poll_interval_s: Poll interval in seconds.

        Yields:
            :class:`~audit_ledger.models.Event` whose ``event_type`` matches.
        """
        config = StreamConfig(
            poll_interval_s=poll_interval_s,
            event_type_filter=event_type,
        )
        yield from stream_by_type(
            self, event_type=event_type, after_index=after_index, config=config
        )

    # ------------------------------------------------------------------
    # Batch operations (#245)
    # ------------------------------------------------------------------

    def batch_submit(
        self,
        requests: Sequence[BatchSubmitRequest],
        *,
        chunk_size: int = 50,
        on_progress: Optional[Any] = None,
        stop_on_error: bool = False,
    ) -> BatchResult:
        """Submit multiple events in chunks with progress tracking.

        Args:
            requests: Sequence of :class:`~audit_ledger.batch.BatchSubmitRequest`.
            chunk_size: Max events per RPC call.
            on_progress: Optional ``(BatchProgress) -> None`` callback.
            stop_on_error: Abort on first error when ``True``.

        Returns:
            :class:`~audit_ledger.batch.BatchResult`.
        """
        return batch_submit(
            self,
            requests,
            chunk_size=chunk_size,
            on_progress=on_progress,
            stop_on_error=stop_on_error,
        )

    def batch_get(
        self,
        indices: Sequence[int],
        *,
        on_progress: Optional[Any] = None,
        stop_on_error: bool = False,
    ) -> BatchResult:
        """Retrieve multiple events by their sequential indices.

        Args:
            indices: Sequence of zero-based event indices.
            on_progress: Optional progress callback.
            stop_on_error: Abort on first retrieval error when ``True``.

        Returns:
            :class:`~audit_ledger.batch.BatchResult` with ``events`` list.
        """
        return batch_get(
            self,
            indices,
            on_progress=on_progress,
            stop_on_error=stop_on_error,
        )

    def batch_verify(
        self,
        event_ids: Sequence[EventId],
        *,
        on_progress: Optional[Any] = None,
    ) -> BatchResult:
        """Verify integrity for a set of events by their IDs.

        Args:
            event_ids: Sequence of 32-byte event IDs.
            on_progress: Optional progress callback.

        Returns:
            :class:`~audit_ledger.batch.BatchResult` with ``verified`` list.
        """
        return batch_verify(self, event_ids, on_progress=on_progress)

    # ------------------------------------------------------------------
    # Governance
    # ------------------------------------------------------------------

    def set_global_max_logs(self, caller: Address, new_max: int) -> None:
        """Set the global maximum log count (owner-only).

        Args:
            caller: Stellar address of the contract owner.
            new_max: New global maximum.
        """
        self._invoke("set_global_max_logs", {
            "caller": caller,
            "new_max": new_max,
        })

    def set_event_max_logs(
        self, caller: Address, event_type: EventType, new_max: int
    ) -> None:
        """Set per-event-type max logs (owner-only).

        Args:
            caller: Stellar address of the contract owner.
            event_type: Symbolic event type to cap.
            new_max: New per-type maximum.
        """
        self._invoke("set_event_max_logs", {
            "caller": caller,
            "event_type": event_type,
            "new_max": new_max,
        })

    def remove_event_cap(self, caller: Address, event_type: EventType) -> None:
        """Remove a per-event-type cap (owner-only).

        Args:
            caller: Stellar address of the contract owner.
            event_type: Symbolic event type whose cap will be removed.
        """
        self._invoke("remove_event_cap", {
            "caller": caller,
            "event_type": event_type,
        })

    def transfer_ownership(self, caller: Address, new_owner: Address) -> None:
        """Transfer contract ownership (owner-only).

        Args:
            caller: Current owner's Stellar address.
            new_owner: Stellar address of the new owner.
        """
        self._invoke("transfer_ownership", {
            "caller": caller,
            "new_owner": new_owner,
        })

    # ------------------------------------------------------------------
    # Metadata size cap
    # ------------------------------------------------------------------

    def set_metadata_max_size(self, caller: Address, max_size: int) -> None:
        """Set the global metadata size cap (owner-only).

        Args:
            caller: Contract owner's Stellar address.
            max_size: Maximum metadata byte length.
        """
        self._invoke("set_metadata_max_size", {
            "caller": caller,
            "max_size": max_size,
        })

    def set_event_metadata_max_size(
        self, caller: Address, event_type: EventType, max_size: int
    ) -> None:
        """Set per-event-type metadata size cap (owner-only).

        Args:
            caller: Contract owner's Stellar address.
            event_type: Symbolic event type to cap.
            max_size: Maximum metadata byte length for that type.
        """
        self._invoke("set_event_metadata_max_size", {
            "caller": caller,
            "event_type": event_type,
            "max_size": max_size,
        })

    def get_metadata_max_size(self, event_type: EventType) -> int:
        """Get the effective metadata size cap for a given event type.

        Args:
            event_type: Symbolic event type to query.

        Returns:
            Maximum allowed metadata byte length.
        """
        result = self._invoke("get_metadata_max_size", {
            "event_type": event_type,
        })
        return self._parse_u32(result)

    # ------------------------------------------------------------------
    # Signatures
    # ------------------------------------------------------------------

    def get_event_signature(self, event_id: EventId) -> Optional[bytes]:
        """Return the stored 96-byte signature payload for an event.

        Args:
            event_id: 32-byte event identifier.

        Returns:
            96-byte blob ``(pubkey || signature)``, or ``None`` if absent.
        """
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

    # ------------------------------------------------------------------
    # Integrity
    # ------------------------------------------------------------------

    def verify_integrity(self) -> bool:
        """Verify the full hash chain.

        Returns:
            ``True`` if the chain is valid.
        """
        result = self._invoke("verify_integrity")
        if isinstance(result, dict):
            return list(result.values())[0] is True
        return bool(result)

    def verify_integrity_range(self, from_idx: int, to_idx: int) -> bool:
        """Verify a range of the hash chain.

        Args:
            from_idx: Start index (inclusive).
            to_idx: End index (inclusive).

        Returns:
            ``True`` if the chain segment is valid.
        """
        result = self._invoke("verify_integrity_range", {
            "from": from_idx,
            "to": to_idx,
        })
        if isinstance(result, dict):
            return list(result.values())[0] is True
        return bool(result)

    # ------------------------------------------------------------------
    # Utility
    # ------------------------------------------------------------------

    @staticmethod
    def compute_event_id(
        contract_id: str,
        submitter: Address,
        event_type: EventType,
        metadata: Metadata,
        timestamp: int,
        index: int,
    ) -> EventId:
        """Recompute the content-addressed event ID off-chain.

        Matches ``compute_event_id`` in the Soroban contract.

        Args:
            contract_id: Stellar contract identifier string.
            submitter: Stellar address of the submitter.
            event_type: Symbolic event type string.
            metadata: Raw binary metadata.
            timestamp: Unix timestamp of the event.
            index: Global sequential index of the event.

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

    @staticmethod
    def verify_signature(
        event_id: EventId,
        pubkey: bytes,
        signature: bytes,
    ) -> bool:
        """Verify an Ed25519 signature against an event ID.

        Args:
            event_id: 32-byte event ID (the signed message).
            pubkey: 32-byte Ed25519 public key.
            signature: 64-byte Ed25519 signature.

        Returns:
            ``True`` if the signature is valid for the given event ID.
        """
        try:
            from stellar_sdk.keypair import Keypair as _Keypair

            verified = _Keypair.from_public_key(pubkey.hex()).verify(
                event_id, signature
            )
            return verified
        except Exception:
            return False
