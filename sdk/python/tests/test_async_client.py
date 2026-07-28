"""Tests for the async SDK client (#242)."""

from __future__ import annotations

import asyncio
import sys
import types
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from audit_ledger.models import Event, Page


def _stub_stellar_sdk():
    stub = types.ModuleType("stellar_sdk")
    stub.SorobanServer = MagicMock
    stub.Keypair = MagicMock
    stub.soroban = types.ModuleType("stellar_sdk.soroban")
    stub.soroban.SorobanClient = MagicMock
    sys.modules.setdefault("stellar_sdk", stub)
    sys.modules.setdefault("stellar_sdk.soroban", stub.soroban)
    return stub


def _make_async_client():
    """Return an AsyncAuditLedgerClient with a mocked sync inner client."""
    _stub_stellar_sdk()
    for mod in list(sys.modules):
        if "audit_ledger.client" in mod or "audit_ledger.async_client" in mod:
            del sys.modules[mod]

    from audit_ledger.async_client import AsyncAuditLedgerClient
    from audit_ledger.client import AuditLedgerClient

    # Build async client bypassing __init__ so we can inject a mock sync client
    async_client = AsyncAuditLedgerClient.__new__(AsyncAuditLedgerClient)
    sync = AuditLedgerClient.__new__(AuditLedgerClient)
    sync.contract_id = "CTEST"
    sync.rpc_url = "https://soroban-testnet.stellar.org"
    sync.network_passphrase = "Test SDF Network ; September 2015"
    sync.server = MagicMock()
    sync.source = None
    async_client._sync = sync
    async_client._executor = None
    return async_client


def _make_event(i: int) -> Event:
    return Event(
        index=i,
        timestamp=1_700_000_000 + i,
        event_type="TX",
        submitter="GABC",
        metadata=b"",
        event_hash=bytes(32),
        prev_hash=bytes(32),
    )


# ── Read operations ───────────────────────────────────────────────────────────

class TestAsyncReadOps:
    def test_total_events(self):
        client = _make_async_client()
        client._sync.total_events = MagicMock(return_value=7)
        result = asyncio.run(client.total_events())
        assert result == 7

    def test_get_event_by_order(self):
        client = _make_async_client()
        client._sync.get_event_by_order = MagicMock(return_value=_make_event(3))
        event = asyncio.run(client.get_event_by_order(3))
        assert event.index == 3

    def test_get_events_pagination(self):
        client = _make_async_client()
        page = Page(items=[_make_event(0)], total=1, offset=0, limit=50)
        client._sync.get_events = MagicMock(return_value=page)
        result = asyncio.run(client.get_events())
        assert result.total == 1
        assert result.items[0].index == 0

    def test_event_count(self):
        client = _make_async_client()
        client._sync.event_count = MagicMock(return_value=5)
        result = asyncio.run(client.event_count("payment"))
        assert result == 5

    def test_get_event(self):
        client = _make_async_client()
        client._sync.get_event = MagicMock(return_value=_make_event(0))
        result = asyncio.run(client.get_event(b"\x00" * 32))
        assert isinstance(result, Event)

    def test_get_event_by_type(self):
        client = _make_async_client()
        client._sync.get_event_by_type = MagicMock(return_value=_make_event(2))
        result = asyncio.run(client.get_event_by_type("TX", 2))
        assert result.index == 2


# ── Write operations ──────────────────────────────────────────────────────────

class TestAsyncWriteOps:
    def test_log_event(self):
        client = _make_async_client()
        client._sync.log_event = MagicMock(return_value=b"\xab" * 32)
        result = asyncio.run(client.log_event("GA", "payment", b"data"))
        assert result == b"\xab" * 32

    def test_log_events(self):
        client = _make_async_client()
        client._sync.log_events = MagicMock(return_value=[0, 1, 2])
        events = [{"submitter": "GA", "event_type": "TX", "metadata": b"x"}]
        result = asyncio.run(client.log_events(events))
        assert result == [0, 1, 2]

    def test_initialize(self):
        client = _make_async_client()
        client._sync.initialize = MagicMock(return_value=None)
        asyncio.run(client.initialize("GOWNER", 1000))
        client._sync.initialize.assert_called_once_with("GOWNER", 1000)


# ── Batch operations (#242) ───────────────────────────────────────────────────

class TestAsyncBatchOps:
    def test_batch_get_events(self):
        client = _make_async_client()
        client._sync.get_event_by_order = MagicMock(side_effect=_make_event)
        results = asyncio.run(client.batch_get_events([0, 1, 2]))
        assert len(results) == 3
        assert [e.index for e in results] == [0, 1, 2]

    def test_batch_get_events_empty(self):
        client = _make_async_client()
        results = asyncio.run(client.batch_get_events([]))
        assert results == []

    def test_batch_log_events(self):
        client = _make_async_client()
        client._sync.log_events = MagicMock(side_effect=[[0], [1]])
        batches = [
            [{"submitter": "GA", "event_type": "TX", "metadata": b"a"}],
            [{"submitter": "GB", "event_type": "TX", "metadata": b"b"}],
        ]
        results = asyncio.run(client.batch_log_events(batches))
        assert results == [[0], [1]]

    def test_batch_get_events_order_preserved(self):
        client = _make_async_client()
        # Return events in reverse so we confirm order comes from indices list
        client._sync.get_event_by_order = MagicMock(side_effect=_make_event)
        results = asyncio.run(client.batch_get_events([5, 3, 1]))
        assert results[0].index == 5
        assert results[1].index == 3
        assert results[2].index == 1


# ── Async streaming (#242) ────────────────────────────────────────────────────

class TestAsyncStreamEvents:
    def test_yields_events_in_order(self):
        client = _make_async_client()
        call_count = 0

        def total_events_side_effect():
            nonlocal call_count
            call_count += 1
            return 3 if call_count <= 2 else 3

        client._sync.total_events = MagicMock(side_effect=total_events_side_effect)
        client._sync.get_event_by_order = MagicMock(side_effect=_make_event)

        async def collect():
            events = []
            async for event in client.stream_events(after_index=0, poll_interval_s=0):
                events.append(event)
                if len(events) == 3:
                    break
            return events

        results = asyncio.run(collect())
        assert [e.index for e in results] == [0, 1, 2]

    def test_resumes_from_after_index(self):
        client = _make_async_client()
        client._sync.total_events = MagicMock(return_value=5)
        client._sync.get_event_by_order = MagicMock(side_effect=_make_event)

        async def collect():
            events = []
            async for event in client.stream_events(after_index=3, poll_interval_s=0):
                events.append(event)
                if len(events) == 2:
                    break
            return events

        results = asyncio.run(collect())
        assert [e.index for e in results] == [3, 4]

    def test_polls_when_no_new_events(self):
        client = _make_async_client()
        poll_counts = [0]
        total_sequence = [0, 0, 1]
        idx = [0]

        def total_events_side_effect():
            v = total_sequence[min(idx[0], len(total_sequence) - 1)]
            idx[0] += 1
            return v

        client._sync.total_events = MagicMock(side_effect=total_events_side_effect)
        client._sync.get_event_by_order = MagicMock(side_effect=_make_event)

        async def collect():
            events = []
            async for event in client.stream_events(after_index=0, poll_interval_s=0):
                events.append(event)
                break
            return events

        results = asyncio.run(collect())
        assert len(results) == 1
        assert results[0].index == 0


# ── Health check (async) ──────────────────────────────────────────────────────

class TestAsyncHealthCheck:
    def test_health_check_healthy(self):
        client = _make_async_client()
        client._sync.health_check = MagicMock(return_value={
            "ok": True,
            "rpc_reachable": True,
            "contract_reachable": True,
            "rpc_url": "https://example.com",
            "contract_id": "CTEST",
            "error": None,
        })
        result = asyncio.run(client.health_check())
        assert result["ok"] is True

    def test_health_status_healthy(self):
        client = _make_async_client()
        client._sync.health_status = MagicMock(return_value="healthy")
        result = asyncio.run(client.health_status())
        assert result == "healthy"


# ── Static helpers pass-through ───────────────────────────────────────────────

class TestAsyncStaticHelpers:
    def test_compute_event_id(self):
        _stub_stellar_sdk()
        from audit_ledger.async_client import AsyncAuditLedgerClient
        id1 = AsyncAuditLedgerClient.compute_event_id(
            "C1", "G1", "TX", b"data", 1000, 0
        )
        assert len(id1) == 32

    def test_verify_signature_invalid(self):
        _stub_stellar_sdk()
        from audit_ledger.async_client import AsyncAuditLedgerClient
        result = AsyncAuditLedgerClient.verify_signature(
            b"\x00" * 32, b"\x01" * 32, b"\x02" * 64
        )
        assert result is False


# ── Governance ────────────────────────────────────────────────────────────────

class TestAsyncGovernance:
    def test_set_global_max_logs(self):
        client = _make_async_client()
        client._sync.set_global_max_logs = MagicMock(return_value=None)
        asyncio.run(client.set_global_max_logs("GOWNER", 500))
        client._sync.set_global_max_logs.assert_called_once_with("GOWNER", 500)

    def test_transfer_ownership(self):
        client = _make_async_client()
        client._sync.transfer_ownership = MagicMock(return_value=None)
        asyncio.run(client.transfer_ownership("GOLD", "GNEW"))
        client._sync.transfer_ownership.assert_called_once_with("GOLD", "GNEW")

    def test_verify_integrity(self):
        client = _make_async_client()
        client._sync.verify_integrity = MagicMock(return_value=True)
        result = asyncio.run(client.verify_integrity())
        assert result is True
