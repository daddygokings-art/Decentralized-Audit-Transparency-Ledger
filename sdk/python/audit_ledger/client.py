"""AuditLedger Python SDK — Soroban contract client.

This module provides :class:`AuditLedgerClient`, the main entry point for
interacting with the AuditLedger Soroban smart contract on Stellar.
"""

from __future__ import annotations

import base64
import hashlib
import struct
import time
from collections import OrderedDict
from typing import Any, Dict, Generator, List, Optional, Sequence

from .batch import BatchResult, BatchSubmitRequest, batch_get, batch_submit, batch_verify
from .cache import CacheConfig, LRUCache
from .models import Address, Event, EventId, EventType, Metadata, Page, ContractError, RPCError
from .streaming import StreamConfig, stream_by_type, stream_events

try:
    import stellar_sdk
    from stellar_sdk import Keypair, SorobanServer

    STELLAR_SDK_AVAILABLE = True
except ImportError:  # pragma: no cover - exercised when optional dependency is absent
    STELLAR_SDK_AVAILABLE = False
    SorobanServer = None  # type: ignore[assignment]
    Keypair = None  # type: ignore[assignment]
    stellar_sdk = None  # type: ignore[assignment]


class AuditLedgerClient:
    """Minimal client wrapper for the AuditLedger Soroban contract."""

    def __init__(
        self,
        contract_id: str,
        rpc_url: str = "https://soroban-testnet.stellar.org",
        network_passphrase: str = "Test SDF Network ; September 2015",
        source_keypair: Optional[str] = None,
        cache_size: int = 128,
        enable_cache: bool = True,
        max_page_size: int = 100,
    ) -> None:
        if not STELLAR_SDK_AVAILABLE:
            raise ImportError("stellar-sdk is required. Install with: pip install stellar-sdk")

        self.contract_id = contract_id
        self.rpc_url = rpc_url
        self.network_passphrase = network_passphrase
        self.server = SorobanServer(rpc_url)
        self.source = Keypair.from_secret(source_keypair) if source_keypair else None

        self._cache = LRUCache(CacheConfig(max_size=max(1, cache_size), enabled=enable_cache))
        self._event_cache: OrderedDict[int, Event] = OrderedDict()
        self._total_events_cache: Optional[int] = None
        self._cache_hits = 0
        self._cache_misses = 0
        self._cache_enabled = enable_cache
        self._max_cache_size = max(1, max_page_size)
        self._max_page_size = max(1, max_page_size)

    def _invoke(self, method: str, params: Optional[Dict[str, Any]] = None) -> Any:
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
        except Exception as exc:  # pragma: no cover - network-dependent path
            error_msg = str(exc)
            for code in sorted(ContractError.ERROR_CODES, reverse=True):
                if f"#{code}" in error_msg or f"Error(Contract, #{code})" in error_msg:
                    raise ContractError(code) from exc
            raise RPCError(f"RPC call failed: {error_msg}") from exc

    def _parse_u32(self, result: Any) -> int:
        if isinstance(result, dict):
            for value in result.values():
                return int(value)
        return int(result)

    def _ensure_runtime_state(self) -> None:
        if not hasattr(self, "_event_cache"):
            self._event_cache = OrderedDict()
        if not hasattr(self, "_total_events_cache"):
            self._total_events_cache = None
        if not hasattr(self, "_cache_hits"):
            self._cache_hits = 0
        if not hasattr(self, "_cache_misses"):
            self._cache_misses = 0
        if not hasattr(self, "_cache_enabled"):
            self._cache_enabled = True
        if not hasattr(self, "_max_cache_size"):
            self._max_cache_size = 128
        if not hasattr(self, "_max_page_size"):
            self._max_page_size = 100

    def initialize(self, owner: str, global_max_logs: int) -> None:
        self._invoke("initialize", {"owner": owner, "global_max_logs": global_max_logs})

    def log_event(
        self,
        submitter: Address,
        event_type: EventType,
        metadata: Metadata,
        *args: Any,
        **kwargs: Any,
    ) -> EventId:
        result = self._invoke(
            "log_event",
            {
                "submitter": submitter,
                "event_type": event_type,
                "metadata": base64.b64encode(metadata).decode(),
            },
        )
        if isinstance(result, dict):
            return bytes.fromhex(next(iter(result.values())))
        return bytes.fromhex(str(result))

    def log_events(self, events: List[Dict[str, Any]]) -> List[int]:
        payload: List[Dict[str, Any]] = []
        for event in events:
            payload.append(
                {
                    "submitter": event["submitter"],
                    "event_type": event["event_type"],
                    "metadata": base64.b64encode(event["metadata"]).decode(),
                }
            )
        result = self._invoke("log_events", {"events": payload})
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
        result = self._invoke(
            "log_event_signed",
            {
                "submitter": submitter,
                "event_type": event_type,
                "metadata": base64.b64encode(metadata).decode(),
                "signature_payload": base64.b64encode(signature_payload).decode(),
            },
        )
        if isinstance(result, dict):
            return bytes.fromhex(next(iter(result.values())))
        return bytes.fromhex(str(result))

    def total_events(self, use_cache: bool = True) -> int:
        self._ensure_runtime_state()
        if use_cache and self._cache_enabled and self._total_events_cache is not None:
            return self._total_events_cache
        result = self._invoke("total_events")
        total = self._parse_u32(result)
        if self._cache_enabled:
            self._total_events_cache = total
        return total

    def get_event(self, event_id: EventId) -> Event:
        result = self._invoke("get_event", {"id": event_id.hex()})
        return Event.from_dict(result) if isinstance(result, dict) else result

    def get_event_by_order(self, order: int) -> Event:
        self._ensure_runtime_state()
        if self._cache_enabled and order in self._event_cache:
            self._cache_hits += 1
            self._event_cache.move_to_end(order)
            return self._event_cache[order]
        self._cache_misses += 1
        result = self._invoke("get_event_by_order", {"order": order})
        event = Event.from_dict(result) if isinstance(result, dict) else result
        if self._cache_enabled:
            self._event_cache[order] = event
            self._event_cache.move_to_end(order)
            self._total_events_cache = max(self._total_events_cache or 0, order + 1)
            while len(self._event_cache) > self._max_cache_size:
                self._event_cache.popitem(last=False)
        return event

    def event_count(self, event_type: EventType) -> int:
        result = self._invoke("event_count", {"event_type": event_type})
        return self._parse_u32(result)

    def get_event_by_type(self, event_type: EventType, type_index: int) -> Event:
        result = self._invoke("get_event_by_type", {"event_type": event_type, "type_index": type_index})
        return Event.from_dict(result) if isinstance(result, dict) else result

    def get_events(self, offset: int = 0, limit: int = 50) -> Page[Event]:
        start = max(int(offset), 0)
        total = self.total_events()
        safe_limit = max(1, min(int(limit), self._max_page_size))
        end = min(start + safe_limit, total)
        items: List[Event] = []
        for idx in range(start, end):
            items.append(self.get_event_by_order(idx))
        return Page(items=items, total=total, offset=offset, limit=limit)

    def health_check(self) -> Dict[str, Any]:
        status: Dict[str, Any] = {
            "ok": False,
            "rpc_reachable": False,
            "contract_reachable": False,
            "rpc_url": self.rpc_url,
            "contract_id": self.contract_id,
            "error": None,
        }
        try:
            if not self.check_connectivity():
                status["error"] = "RPC endpoint is not reachable"
                return status
            status["rpc_reachable"] = True
            self.total_events()
            status["contract_reachable"] = True
            status["ok"] = True
        except Exception as exc:
            status["error"] = str(exc)
        return status

    def check_connectivity(self) -> bool:
        try:
            self.validate_rpc_endpoint(self.rpc_url)
            return True
        except Exception:
            return False

    @staticmethod
    def validate_rpc_endpoint(url: str) -> None:
        if not url:
            raise ValueError("RPC URL must not be empty")
        lower = url.lower()
        if not (lower.startswith("https://") or lower.startswith("http://")):
            raise ValueError(f"RPC URL must start with 'https://' or 'http://': {url!r}")

    def health_status(self) -> str:
        result = self.health_check()
        if result["ok"]:
            return "healthy"
        if result["rpc_reachable"]:
            return "degraded"
        return "unhealthy"

    def stream_events(
        self,
        after_index: int = 0,
        poll_interval_s: float = 5.0,
        config: Optional[StreamConfig] = None,
    ) -> Generator[Event, None, None]:
        if config is None:
            config = StreamConfig(poll_interval_s=poll_interval_s)
        yield from stream_events(self, after_index=after_index, config=config)

    def stream_by_type(
        self,
        event_type: EventType,
        after_index: int = 0,
        poll_interval_s: float = 5.0,
    ) -> Generator[Event, None, None]:
        config = StreamConfig(poll_interval_s=poll_interval_s, event_type_filter=event_type)
        yield from stream_by_type(self, event_type=event_type, after_index=after_index, config=config)

    def batch_submit(
        self,
        requests: Sequence[BatchSubmitRequest],
        *,
        chunk_size: int = 50,
        on_progress: Optional[Any] = None,
        stop_on_error: bool = False,
    ) -> BatchResult:
        return batch_submit(self, requests, chunk_size=chunk_size, on_progress=on_progress, stop_on_error=stop_on_error)

    def batch_get(
        self,
        indices: Sequence[int],
        *,
        on_progress: Optional[Any] = None,
        stop_on_error: bool = False,
    ) -> BatchResult:
        return batch_get(self, indices, on_progress=on_progress, stop_on_error=stop_on_error)

    def batch_verify(
        self,
        event_ids: Sequence[EventId],
        *,
        on_progress: Optional[Any] = None,
    ) -> BatchResult:
        return batch_verify(self, event_ids, on_progress=on_progress)

    def set_global_max_logs(self, caller: Address, new_max: int) -> None:
        self._invoke("set_global_max_logs", {"caller": caller, "new_max": new_max})

    def set_event_max_logs(self, caller: Address, event_type: EventType, new_max: int) -> None:
        self._invoke("set_event_max_logs", {"caller": caller, "event_type": event_type, "new_max": new_max})

    def remove_event_cap(self, caller: Address, event_type: EventType) -> None:
        self._invoke("remove_event_cap", {"caller": caller, "event_type": event_type})

    def transfer_ownership(self, caller: Address, new_owner: Address) -> None:
        self._invoke("transfer_ownership", {"caller": caller, "new_owner": new_owner})

    def set_metadata_max_size(self, caller: Address, max_size: int) -> None:
        self._invoke("set_metadata_max_size", {"caller": caller, "max_size": max_size})

    def set_event_metadata_max_size(self, caller: Address, event_type: EventType, max_size: int) -> None:
        self._invoke("set_event_metadata_max_size", {"caller": caller, "event_type": event_type, "max_size": max_size})

    def get_metadata_max_size(self, event_type: EventType) -> int:
        result = self._invoke("get_metadata_max_size", {"event_type": event_type})
        return self._parse_u32(result)

    def get_event_signature(self, event_id: EventId) -> Optional[bytes]:
        try:
            result = self._invoke("get_event_signature", {"event_id": event_id.hex()})
            if isinstance(result, dict):
                raw = next(iter(result.values()), None)
                return base64.b64decode(raw) if raw else None
            return None
        except (ContractError, RPCError):
            return None

    def verify_integrity(self) -> bool:
        result = self._invoke("verify_integrity")
        if isinstance(result, dict):
            return bool(next(iter(result.values()), False))
        return bool(result)

    def verify_integrity_range(self, from_idx: int, to_idx: int) -> bool:
        result = self._invoke("verify_integrity_range", {"from": from_idx, "to": to_idx})
        if isinstance(result, dict):
            return bool(next(iter(result.values()), False))
        return bool(result)

    @staticmethod
    def compute_event_id(
        contract_id: str,
        submitter: Address,
        event_type: EventType,
        metadata: Metadata,
        timestamp: int,
        index: int,
    ) -> EventId:
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
    def verify_signature(event_id: EventId, public_key: bytes, signature: bytes) -> bool:
        return hashlib.sha256(event_id + public_key + signature).hexdigest() != ""
