"""Tests for the AuditLedger Python SDK — models and offline utilities."""

from __future__ import annotations

import sys
import types
import pytest
from unittest.mock import MagicMock, patch


# ── Model tests ───────────────────────────────────────────────────────────────

class TestEvent:
    def _sample_dict(self) -> dict:
        return {
            "index": 0,
            "timestamp": 1_700_000_000,
            "event_type": "TRANSFER",
            "submitter": "GABC123",
            "metadata": "deadbeef",
            "event_hash": "ab" * 32,
            "prev_hash": "00" * 32,
        }

    def test_from_dict_round_trip(self):
        from audit_ledger.models import Event
        d = self._sample_dict()
        ev = Event.from_dict(d)
        assert ev.index == 0
        assert ev.timestamp == 1_700_000_000
        assert ev.event_type == "TRANSFER"
        assert ev.submitter == "GABC123"
        assert ev.metadata == bytes.fromhex("deadbeef")
        assert ev.event_hash == bytes.fromhex("ab" * 32)
        # "00" * 32 is a valid non-empty hex string → decoded to bytes(32)
        assert ev.prev_hash == bytes(32)

    def test_from_dict_defaults_missing_hashes(self):
        from audit_ledger.models import Event
        d = {"index": 1, "timestamp": 0, "event_type": "X", "submitter": "G", "metadata": ""}
        ev = Event.from_dict(d)
        # Missing hash fields → None
        assert ev.event_hash is None
        assert ev.prev_hash is None

    def test_from_dict_empty_metadata(self):
        from audit_ledger.models import Event
        d = self._sample_dict()
        d["metadata"] = ""
        ev = Event.from_dict(d)
        assert ev.metadata == b""


# ── Error tests ───────────────────────────────────────────────────────────────

class TestContractError:
    def test_known_error_code(self):
        from audit_ledger.models import ContractError
        err = ContractError(1)
        assert err.code == 1
        assert err.name == "CallerNotOwner"
        assert "CallerNotOwner" in str(err)

    def test_unknown_error_code(self):
        from audit_ledger.models import ContractError
        err = ContractError(99)
        assert "UnknownError(99)" in err.name

    def test_all_defined_codes(self):
        from audit_ledger.models import ContractError
        for code in range(1, 10):
            err = ContractError(code)
            assert err.code == code

    def test_is_audit_ledger_error(self):
        from audit_ledger.models import ContractError, AuditLedgerError
        assert isinstance(ContractError(1), AuditLedgerError)

    def test_rpc_error_is_audit_ledger_error(self):
        from audit_ledger.models import RPCError, AuditLedgerError
        err = RPCError("timeout")
        assert isinstance(err, AuditLedgerError)
        assert "timeout" in str(err)


# ── Client offline tests ──────────────────────────────────────────────────────

def _stub_stellar_sdk():
    """Inject a minimal stub for stellar_sdk so client.py can be imported."""
    stub = types.ModuleType("stellar_sdk")
    stub.SorobanServer = MagicMock  # type: ignore[attr-defined]
    stub.Keypair = MagicMock  # type: ignore[attr-defined]
    stub.soroban = types.ModuleType("stellar_sdk.soroban")
    stub.soroban.SorobanClient = MagicMock  # type: ignore[attr-defined]
    sys.modules.setdefault("stellar_sdk", stub)
    sys.modules.setdefault("stellar_sdk.soroban", stub.soroban)
    return stub


def _make_bare_client():
    """Return an AuditLedgerClient with no live server (server is a MagicMock)."""
    _stub_stellar_sdk()
    for mod in ["audit_ledger.client", "audit_ledger.cache",
                "audit_ledger.streaming", "audit_ledger.batch"]:
        sys.modules.pop(mod, None)
    from audit_ledger.client import AuditLedgerClient
    from audit_ledger.cache import LRUCache, CacheConfig
    client = AuditLedgerClient.__new__(AuditLedgerClient)
    client.contract_id = "CTEST"
    client.rpc_url = "https://soroban-testnet.stellar.org"
    client.network_passphrase = "Test SDF Network ; September 2015"
    client.server = MagicMock()
    client.source = None
    client._cache = LRUCache(CacheConfig(max_size=64, ttl_seconds=60.0))
    return client


class TestAuditLedgerClientOffline:
    """Tests that don't require a live Stellar RPC."""

    def _make_client(self):
        return _make_bare_client()

    def test_compute_event_id_is_deterministic(self):
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient
        id1 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        id2 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        assert id1 == id2
        assert len(id1) == 32

    def test_compute_event_id_differs_on_params(self):
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient
        id1 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        id2 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data2", 1000, 0)
        assert id1 != id2

    def test_verify_signature_invalid(self):
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient
        result = AuditLedgerClient.verify_signature(b"\x00" * 32, b"\x01" * 32, b"\x02" * 64)
        assert result is False

    def test_client_raises_without_stellar_sdk(self):
        saved = sys.modules.pop("stellar_sdk", None)
        saved_soroban = sys.modules.pop("stellar_sdk.soroban", None)
        sys.modules.pop("audit_ledger.client", None)
        try:
            from audit_ledger.client import AuditLedgerClient
            with pytest.raises(ImportError, match="stellar-sdk"):
                AuditLedgerClient(contract_id="X")
        finally:
            if saved:
                sys.modules["stellar_sdk"] = saved
            if saved_soroban:
                sys.modules["stellar_sdk.soroban"] = saved_soroban

    def test_parse_u32_from_dict(self):
        client = self._make_client()
        assert client._parse_u32({"u32": 42}) == 42
        assert client._parse_u32(7) == 7

    def test_invoke_raises_contract_error(self):
        from audit_ledger.models import ContractError
        client = self._make_client()
        client.server.invoke_contract = MagicMock(
            side_effect=Exception("Error(Contract, #2)")
        )
        with pytest.raises(ContractError) as exc:
            client._invoke("total_events")
        assert exc.value.code == 2

    def test_invoke_raises_rpc_error_on_unknown(self):
        from audit_ledger.models import RPCError
        client = self._make_client()
        client.server.invoke_contract = MagicMock(
            side_effect=Exception("network timeout")
        )
        with pytest.raises(RPCError):
            client._invoke("total_events")


# ── Streaming tests (#244) ────────────────────────────────────────────────────

class TestStreamEvents:
    """Tests for AuditLedgerClient.stream_events() generator."""

    def _make_streaming_client(self, event_counts):
        """event_counts: list of totals returned on successive polls."""
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient
        from audit_ledger.models import Event
        from audit_ledger.cache import LRUCache, CacheConfig

        def _make_event(i):
            return Event(
                index=i, timestamp=1_700_000_000 + i,
                event_type="TX", submitter="GABC",
                metadata=b"", event_hash=None, prev_hash=None,
            )

        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.server = MagicMock()
        client.source = None
        client._cache = LRUCache(CacheConfig(enabled=False))
        client.total_events = MagicMock(side_effect=event_counts)
        client.get_event_by_order = MagicMock(side_effect=_make_event)
        return client

    def test_yields_existing_events_in_order(self):
        client = self._make_streaming_client([3, 3])
        gen = client.stream_events(after_index=0, poll_interval_s=0)
        with patch("audit_ledger.streaming.time.sleep"):
            events = [next(gen) for _ in range(3)]
        assert [e.index for e in events] == [0, 1, 2]

    def test_resumes_from_after_index(self):
        client = self._make_streaming_client([5, 5])
        gen = client.stream_events(after_index=3, poll_interval_s=0)
        with patch("audit_ledger.streaming.time.sleep"):
            events = [next(gen) for _ in range(2)]
        assert [e.index for e in events] == [3, 4]

    def test_yields_new_events_as_they_appear(self):
        client = self._make_streaming_client([2, 4, 4])
        gen = client.stream_events(after_index=0, poll_interval_s=0)
        with patch("audit_ledger.streaming.time.sleep"):
            events = [next(gen) for _ in range(4)]
        assert [e.index for e in events] == [0, 1, 2, 3]

    def test_no_events_sleeps(self):
        client = self._make_streaming_client([0, 0, 1])
        gen = client.stream_events(after_index=0, poll_interval_s=1.5)
        with patch("audit_ledger.streaming.time.sleep") as mock_sleep:
            next(gen)
        assert mock_sleep.call_count >= 2

    def test_stream_filter_by_type(self):
        """Only events matching event_type_filter are yielded."""
        from audit_ledger.models import Event
        from audit_ledger.streaming import StreamConfig
        from audit_ledger.cache import LRUCache, CacheConfig

        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient

        types_seq = ["TX", "REFUND", "TX", "REFUND", "TX"]

        def _make_typed_event(i):
            return Event(
                index=i, timestamp=i, event_type=types_seq[i],
                submitter="G", metadata=b"", event_hash=None, prev_hash=None,
            )

        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.server = MagicMock()
        client.source = None
        client._cache = LRUCache(CacheConfig(enabled=False))
        client.total_events = MagicMock(return_value=5)
        client.get_event_by_order = MagicMock(side_effect=_make_typed_event)

        cfg = StreamConfig(poll_interval_s=0, event_type_filter="TX", max_errors=1)
        gen = client.stream_events(config=cfg)
        with patch("audit_ledger.streaming.time.sleep"):
            collected = [next(gen) for _ in range(3)]
        assert all(e.event_type == "TX" for e in collected)


# ── Pagination tests (#128 / get_events) ─────────────────────────────────────

class TestGetEvents:
    """Tests for AuditLedgerClient.get_events() pagination."""

    def _make_client_with_events(self, n: int):
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient
        from audit_ledger.models import Event
        from audit_ledger.cache import LRUCache, CacheConfig

        def _make_event(i):
            return Event(
                index=i, timestamp=1_700_000_000 + i,
                event_type="TX", submitter="GABC",
                metadata=b"", event_hash=None, prev_hash=None,
            )

        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.server = MagicMock()
        client.source = None
        client._cache = LRUCache(CacheConfig(enabled=False))
        client.total_events = MagicMock(return_value=n)
        client.get_event_by_order = MagicMock(side_effect=_make_event)
        return client

    def test_default_limit(self):
        client = self._make_client_with_events(100)
        page = client.get_events()
        assert page.offset == 0
        assert page.limit == 50
        assert page.total == 100
        assert len(page.items) == 50
        assert page.items[0].index == 0
        assert page.items[49].index == 49

    def test_custom_offset_and_limit(self):
        client = self._make_client_with_events(100)
        page = client.get_events(offset=10, limit=20)
        assert page.offset == 10
        assert page.limit == 20
        assert len(page.items) == 20
        assert page.items[0].index == 10

    def test_boundary_offset_at_end(self):
        client = self._make_client_with_events(10)
        page = client.get_events(offset=10, limit=50)
        assert page.total == 10
        assert page.items == []

    def test_partial_last_page(self):
        client = self._make_client_with_events(7)
        page = client.get_events(offset=5, limit=50)
        assert len(page.items) == 2
        assert page.items[0].index == 5
        assert page.items[1].index == 6

    def test_page_dataclass_fields(self):
        from audit_ledger.models import Page
        p = Page(items=[], total=0, offset=0, limit=50)
        assert p.items == []
        assert p.total == 0


# ── Cache tests (#246) ────────────────────────────────────────────────────────

class TestLRUCache:
    """Unit tests for the LRU cache module."""

    def test_basic_set_and_get(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("k", 42)
        assert cache.get("k") == 42

    def test_miss_returns_none(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        assert cache.get("missing") is None

    def test_lru_eviction(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=3, ttl_seconds=None))
        cache.set("a", 1)
        cache.set("b", 2)
        cache.set("c", 3)
        # Access "a" to make it recently used
        cache.get("a")
        # Adding "d" should evict "b" (LRU)
        cache.set("d", 4)
        assert cache.get("b") is None
        assert cache.get("a") == 1
        assert cache.get("c") == 3
        assert cache.get("d") == 4

    def test_ttl_expiry(self):
        import time
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[str] = LRUCache(CacheConfig(max_size=10, ttl_seconds=0.05))
        cache.set("x", "hello")
        assert cache.get("x") == "hello"
        time.sleep(0.1)
        assert cache.get("x") is None

    def test_invalidate_single_key(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("a", 1)
        cache.set("b", 2)
        removed = cache.invalidate("a")
        assert removed is True
        assert cache.get("a") is None
        assert cache.get("b") == 2

    def test_invalidate_missing_key(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        assert cache.invalidate("nope") is False

    def test_invalidate_prefix(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("event:1", 1)
        cache.set("event:2", 2)
        cache.set("total", 99)
        removed = cache.invalidate_prefix("event:")
        assert removed == 2
        assert cache.get("event:1") is None
        assert cache.get("event:2") is None
        assert cache.get("total") == 99

    def test_clear(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("a", 1)
        cache.set("b", 2)
        cache.clear()
        assert len(cache) == 0

    def test_stats_hit_miss(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("k", 1)
        cache.get("k")   # hit
        cache.get("nope")  # miss
        stats = cache.stats
        assert stats.hits == 1
        assert stats.misses == 1
        assert stats.hit_rate == 0.5

    def test_stats_evictions(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=2, ttl_seconds=None))
        cache.set("a", 1)
        cache.set("b", 2)
        cache.set("c", 3)  # evicts "a"
        assert cache.stats.evictions >= 1

    def test_disabled_cache_always_misses(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10, enabled=False))
        cache.set("k", 99)
        assert cache.get("k") is None

    def test_configure_at_runtime(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("a", 1)
        # Disable at runtime → cache should be cleared
        cache.configure(CacheConfig(max_size=0, enabled=False))
        assert cache.get("a") is None

    def test_reset_stats(self):
        from audit_ledger.cache import LRUCache, CacheConfig
        cache: LRUCache[int] = LRUCache(CacheConfig(max_size=10))
        cache.set("k", 1)
        cache.get("k")
        cache.reset_stats()
        assert cache.stats.hits == 0
        assert cache.stats.misses == 0

    def test_client_cache_stats(self):
        """AuditLedgerClient.cache_stats() returns a CacheStats object."""
        client = _make_bare_client()
        from audit_ledger.cache import CacheStats
        stats = client.cache_stats()
        assert isinstance(stats, CacheStats)

    def test_client_invalidate_all(self):
        client = _make_bare_client()
        client._cache.set("k", 1)
        client.invalidate_cache()
        assert client._cache.get("k") is None

    def test_client_invalidate_key(self):
        client = _make_bare_client()
        client._cache.set("k", 1)
        client.invalidate_cache("k")
        assert client._cache.get("k") is None

    def test_client_configure_cache(self):
        from audit_ledger.cache import CacheConfig
        client = _make_bare_client()
        client.configure_cache(CacheConfig(max_size=512, ttl_seconds=120.0))
        assert client._cache.config.max_size == 512


# ── Batch tests (#245) ────────────────────────────────────────────────────────

class TestBatchOperations:
    """Unit tests for the batch module."""

    def _make_batch_client(self, n_events: int = 5):
        """Return a client with n_events pre-loaded via mocks."""
        from audit_ledger.models import Event
        from audit_ledger.cache import LRUCache, CacheConfig
        _stub_stellar_sdk()
        sys.modules.pop("audit_ledger.client", None)
        from audit_ledger.client import AuditLedgerClient

        events = [
            Event(index=i, timestamp=1000 + i, event_type="TX",
                  submitter="GA", metadata=b"x", event_hash=None, prev_hash=None)
            for i in range(n_events)
        ]

        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.server = MagicMock()
        client.source = None
        client._cache = LRUCache(CacheConfig(enabled=False))

        # log_event returns a fake 32-byte hex ID
        client.server.invoke_contract = MagicMock(
            return_value={"id": "aa" * 32}
        )

        def _get_by_order(i):
            return events[i]

        client.total_events = MagicMock(return_value=n_events)
        client.get_event_by_order = MagicMock(side_effect=_get_by_order)
        client.get_event = MagicMock(side_effect=lambda eid: events[0])
        return client, events

    def test_batch_submit_success(self):
        from audit_ledger.batch import BatchSubmitRequest, batch_submit
        client, _ = self._make_batch_client()

        # Mock log_events to return sequential indices
        client.log_events = MagicMock(return_value=[0, 1, 2])

        reqs = [
            BatchSubmitRequest("GA", "TX", b"a"),
            BatchSubmitRequest("GB", "TX", b"b"),
            BatchSubmitRequest("GC", "TX", b"c"),
        ]
        result = batch_submit(client, reqs)
        assert result.succeeded == 3
        assert result.failed == 0
        assert result.all_succeeded

    def test_batch_submit_progress_callback(self):
        from audit_ledger.batch import BatchSubmitRequest, batch_submit
        client, _ = self._make_batch_client()
        client.log_events = MagicMock(return_value=[0, 1])

        progress_snapshots = []

        def on_progress(p):
            progress_snapshots.append(p.completed)

        reqs = [BatchSubmitRequest("GA", "TX", b"a"), BatchSubmitRequest("GB", "TX", b"b")]
        batch_submit(client, reqs, on_progress=on_progress)
        assert len(progress_snapshots) >= 1

    def test_batch_get_retrieves_events(self):
        from audit_ledger.batch import batch_get
        client, events = self._make_batch_client(5)
        result = batch_get(client, [0, 1, 2])
        assert result.succeeded == 3
        assert result.failed == 0
        assert result.events[0].index == 0
        assert result.events[1].index == 1
        assert result.events[2].index == 2

    def test_batch_get_handles_error(self):
        from audit_ledger.batch import batch_get
        from audit_ledger.models import ContractError
        client, _ = self._make_batch_client(3)
        client.get_event_by_order = MagicMock(
            side_effect=[
                Exception("boom"),
                Exception("boom"),
                Exception("boom"),
            ]
        )
        result = batch_get(client, [0, 1, 2])
        assert result.failed == 3
        assert result.succeeded == 0

    def test_batch_verify_all_valid(self):
        from audit_ledger.batch import batch_verify
        client, _ = self._make_batch_client(3)
        ids = [b"\xaa" * 32, b"\xbb" * 32]
        result = batch_verify(client, ids)
        assert result.succeeded == 2
        assert all(result.verified)

    def test_batch_result_success_rate(self):
        from audit_ledger.batch import BatchResult
        r = BatchResult(total=4, succeeded=3, failed=1)
        assert r.success_rate == 0.75
        assert not r.all_succeeded

    def test_batch_progress_percent(self):
        from audit_ledger.batch import BatchProgress
        p = BatchProgress(total=10, completed=5)
        assert p.percent == 50.0
        assert not p.is_done
        p2 = BatchProgress(total=10, completed=10)
        assert p2.is_done

    def test_batch_submit_raises_on_empty(self):
        from audit_ledger.batch import batch_submit, BatchError
        client, _ = self._make_batch_client()
        with pytest.raises(BatchError):
            batch_submit(client, [])

    def test_client_batch_submit(self):
        from audit_ledger.batch import BatchSubmitRequest
        client, _ = self._make_batch_client()
        client.log_events = MagicMock(return_value=[0])
        reqs = [BatchSubmitRequest("GA", "TX", b"data")]
        result = client.batch_submit(reqs)
        assert result.succeeded == 1

    def test_client_batch_get(self):
        client, events = self._make_batch_client(3)
        result = client.batch_get([0, 1])
        assert len(result.events) == 2

    def test_client_batch_verify(self):
        client, _ = self._make_batch_client(3)
        result = client.batch_verify([b"\xaa" * 32])
        assert result.succeeded == 1
