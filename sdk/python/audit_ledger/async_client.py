"""AuditLedger Python SDK — async client (#242).

Provides an asyncio-compatible counterpart to :class:`AuditLedgerClient`
using ``asyncio.get_event_loop().run_in_executor`` to wrap the synchronous
Soroban RPC calls so they don't block the event loop.

All public methods are ``async`` and mirror the synchronous API.

Example::

    import asyncio
    from audit_ledger.async_client import AsyncAuditLedgerClient

    async def main():
        client = AsyncAuditLedgerClient(
            contract_id="CCXMTP7...",
            rpc_url="https://soroban-testnet.stellar.org",
        )
        total = await client.total_events()
        async for event in client.stream_events():
            print(event)

    asyncio.run(main())
"""

from __future__ import annotations

import asyncio
from typing import Any, AsyncGenerator, Optional

from .client import AuditLedgerClient
from .models import Event, Page


class AsyncAuditLedgerClient:
    """Async/await wrapper around :class:`AuditLedgerClient`.

    Delegates all network I/O to a thread-pool executor so calls are
    non-blocking inside an asyncio event loop.

    Args:
        contract_id: Stellar contract ID (C... string).
        rpc_url: Soroban RPC endpoint URL.
        network_passphrase: Stellar network passphrase.
        source_keypair: Optional Stellar keypair secret string.
        executor: Optional :class:`concurrent.futures.Executor` to use for
            thread offloading.  Defaults to the loop's default executor.

    Example::

        client = AsyncAuditLedgerClient(contract_id="CXXX...")
        total = await client.total_events()
    """

    def __init__(
        self,
        contract_id: str,
        rpc_url: str = "https://soroban-testnet.stellar.org",
        network_passphrase: str = "Test SDF Network ; September 2015",
        source_keypair: Optional[str] = None,
        executor=None,
    ):
        self._sync = AuditLedgerClient(
            contract_id=contract_id,
            rpc_url=rpc_url,
            network_passphrase=network_passphrase,
            source_keypair=source_keypair,
        )
        self._executor = executor

    # ── Helpers ───────────────────────────────────────────────────────────

    async def _run(self, fn, *args, **kwargs):
        """Run *fn* in a thread-pool executor and return the awaitable result."""
        loop = asyncio.get_event_loop()
        return await loop.run_in_executor(
            self._executor,
            lambda: fn(*args, **kwargs),
        )

    # ── Write functions ───────────────────────────────────────────────────

    async def initialize(self, owner: str, global_max_logs: int) -> None:
        """Async version of :meth:`AuditLedgerClient.initialize`."""
        await self._run(self._sync.initialize, owner, global_max_logs)

    async def log_event(
        self, submitter: str, event_type: str, metadata: bytes
    ) -> bytes:
        """Async version of :meth:`AuditLedgerClient.log_event`."""
        return await self._run(self._sync.log_event, submitter, event_type, metadata)

    async def log_events(self, events: list[dict[str, Any]]) -> list[int]:
        """Async version of :meth:`AuditLedgerClient.log_events`."""
        return await self._run(self._sync.log_events, events)

    async def log_event_signed(
        self,
        submitter: str,
        event_type: str,
        metadata: bytes,
        signature_payload: bytes,
    ) -> bytes:
        """Async version of :meth:`AuditLedgerClient.log_event_signed`."""
        return await self._run(
            self._sync.log_event_signed,
            submitter,
            event_type,
            metadata,
            signature_payload,
        )

    # ── Read functions ────────────────────────────────────────────────────

    async def total_events(self) -> int:
        """Async version of :meth:`AuditLedgerClient.total_events`."""
        return await self._run(self._sync.total_events)

    async def get_event(self, event_id: bytes) -> Event:
        """Async version of :meth:`AuditLedgerClient.get_event`."""
        return await self._run(self._sync.get_event, event_id)

    async def get_event_by_order(self, order: int) -> Event:
        """Async version of :meth:`AuditLedgerClient.get_event_by_order`."""
        return await self._run(self._sync.get_event_by_order, order)

    async def event_count(self, event_type: str) -> int:
        """Async version of :meth:`AuditLedgerClient.event_count`."""
        return await self._run(self._sync.event_count, event_type)

    async def get_event_by_type(self, event_type: str, type_index: int) -> Event:
        """Async version of :meth:`AuditLedgerClient.get_event_by_type`."""
        return await self._run(self._sync.get_event_by_type, event_type, type_index)

    async def get_events(self, offset: int = 0, limit: int = 50) -> "Page[Event]":
        """Async version of :meth:`AuditLedgerClient.get_events`."""
        return await self._run(self._sync.get_events, offset, limit)

    # ── Async event streaming (#242) ──────────────────────────────────────

    async def stream_events(
        self,
        after_index: int = 0,
        poll_interval_s: float = 5.0,
    ) -> AsyncGenerator[Event, None]:
        """Async generator that yields new events as they appear on-chain.

        Args:
            after_index: Resume from this sequential order index (exclusive).
            poll_interval_s: Seconds to wait between polls when no new events.

        Yields:
            :class:`~audit_ledger.models.Event` objects in ascending order.

        Example::

            async for event in client.stream_events(after_index=0):
                print(event)
        """
        cursor = after_index
        while True:
            total = await self.total_events()
            while cursor < total:
                event = await self.get_event_by_order(cursor)
                yield event
                cursor += 1
            await asyncio.sleep(poll_interval_s)

    # ── Async batch operations (#242) ─────────────────────────────────────

    async def batch_get_events(self, indices: list[int]) -> list[Event]:
        """Fetch multiple events concurrently by their order indices.

        Args:
            indices: List of sequential order indices to fetch.

        Returns:
            List of :class:`~audit_ledger.models.Event` objects in the same
            order as *indices*.
        """
        tasks = [self.get_event_by_order(i) for i in indices]
        return list(await asyncio.gather(*tasks))

    async def batch_log_events(
        self, batches: list[list[dict[str, Any]]]
    ) -> list[list[int]]:
        """Submit multiple batches of events concurrently.

        Args:
            batches: Each element is a list of event dicts suitable for
                :meth:`log_events`.

        Returns:
            A list of results, one per batch, where each result is the list
            of indices returned by the contract.
        """
        tasks = [self.log_events(batch) for batch in batches]
        return list(await asyncio.gather(*tasks))

    # ── Health check ──────────────────────────────────────────────────────

    async def health_check(self) -> dict:
        """Async version of :meth:`AuditLedgerClient.health_check`."""
        return await self._run(self._sync.health_check)

    async def health_status(self) -> str:
        """Async version of :meth:`AuditLedgerClient.health_status`."""
        return await self._run(self._sync.health_status)

    # ── Governance ────────────────────────────────────────────────────────

    async def set_global_max_logs(self, caller: str, new_max: int) -> None:
        """Async version of :meth:`AuditLedgerClient.set_global_max_logs`."""
        await self._run(self._sync.set_global_max_logs, caller, new_max)

    async def set_event_max_logs(
        self, caller: str, event_type: str, new_max: int
    ) -> None:
        """Async version of :meth:`AuditLedgerClient.set_event_max_logs`."""
        await self._run(self._sync.set_event_max_logs, caller, event_type, new_max)

    async def remove_event_cap(self, caller: str, event_type: str) -> None:
        """Async version of :meth:`AuditLedgerClient.remove_event_cap`."""
        await self._run(self._sync.remove_event_cap, caller, event_type)

    async def transfer_ownership(self, caller: str, new_owner: str) -> None:
        """Async version of :meth:`AuditLedgerClient.transfer_ownership`."""
        await self._run(self._sync.transfer_ownership, caller, new_owner)

    # ── Integrity ─────────────────────────────────────────────────────────

    async def verify_integrity(self) -> bool:
        """Async version of :meth:`AuditLedgerClient.verify_integrity`."""
        return await self._run(self._sync.verify_integrity)

    async def verify_integrity_range(self, from_idx: int, to_idx: int) -> bool:
        """Async version of :meth:`AuditLedgerClient.verify_integrity_range`."""
        return await self._run(self._sync.verify_integrity_range, from_idx, to_idx)

    # ── Pass-through static helpers ───────────────────────────────────────

    @staticmethod
    def compute_event_id(
        contract_id: str,
        submitter: str,
        event_type: str,
        metadata: bytes,
        timestamp: int,
        index: int,
    ) -> bytes:
        """See :meth:`AuditLedgerClient.compute_event_id`."""
        return AuditLedgerClient.compute_event_id(
            contract_id, submitter, event_type, metadata, timestamp, index
        )

    @staticmethod
    def verify_signature(
        event_id: bytes, pubkey: bytes, signature: bytes
    ) -> bool:
        """See :meth:`AuditLedgerClient.verify_signature`."""
        return AuditLedgerClient.verify_signature(event_id, pubkey, signature)
