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
        assert ev.prev_hash == bytes(32)

    def test_from_dict_defaults_missing_hashes(self):
        from audit_ledger.models import Event
        d = {"index": 1, "timestamp": 0, "event_type": "X", "submitter": "G", "metadata": ""}
        ev = Event.from_dict(d)
        assert ev.event_hash == bytes(32)
        assert ev.prev_hash == bytes(32)

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


class TestAuditLedgerClientOffline:
    """Tests that don't require a live Stellar RPC."""

    def _make_client(self):
        _stub_stellar_sdk()
        # Force re-import with sdk available
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
        from audit_ledger.client import AuditLedgerClient
        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.rpc_url = "https://soroban-testnet.stellar.org"
        client.network_passphrase = "Test SDF Network ; September 2015"
        client.server = MagicMock()
        client.source = None
        return client

    def test_compute_event_id_is_deterministic(self):
        _stub_stellar_sdk()
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
        from audit_ledger.client import AuditLedgerClient
        id1 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        id2 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        assert id1 == id2
        assert len(id1) == 32

    def test_compute_event_id_differs_on_params(self):
        _stub_stellar_sdk()
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
        from audit_ledger.client import AuditLedgerClient
        id1 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data", 1000, 0)
        id2 = AuditLedgerClient.compute_event_id("C1", "G1", "TX", b"data2", 1000, 0)
        assert id1 != id2

    def test_verify_signature_invalid(self):
        _stub_stellar_sdk()
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
        from audit_ledger.client import AuditLedgerClient
        result = AuditLedgerClient.verify_signature(b"\x00" * 32, b"\x01" * 32, b"\x02" * 64)
        assert result is False

    def test_client_raises_without_stellar_sdk(self):
        # Temporarily hide stellar_sdk
        saved = sys.modules.pop("stellar_sdk", None)
        saved_soroban = sys.modules.pop("stellar_sdk.soroban", None)
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
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
        client.server.invoke_contract = MagicMock(side_effect=Exception("Error(Contract, #2)"))
        with pytest.raises(ContractError) as exc:
            client._invoke("total_events")
        assert exc.value.code == 2

    def test_invoke_raises_rpc_error_on_unknown(self):
        from audit_ledger.models import RPCError
        client = self._make_client()
        client.server.invoke_contract = MagicMock(side_effect=Exception("network timeout"))
        with pytest.raises(RPCError):
            client._invoke("total_events")


<<<<<<< fix/127-stream-events
# ── Streaming tests (#127) ────────────────────────────────────────────────────

class TestStreamEvents:
    """Tests for AuditLedgerClient.stream_events() generator."""

    def _make_streaming_client(self, event_counts):
        """event_counts: list of totals returned on successive polls."""
=======
# ── Pagination tests (#128) ───────────────────────────────────────────────────

class TestGetEvents:
    """Tests for AuditLedgerClient.get_events() pagination."""

    def _make_client_with_events(self, n: int):
>>>>>>> master
        _stub_stellar_sdk()
        if "audit_ledger.client" in sys.modules:
            del sys.modules["audit_ledger.client"]
        from audit_ledger.client import AuditLedgerClient
        from audit_ledger.models import Event

        def _make_event(i):
            return Event(
                index=i, timestamp=1_700_000_000 + i,
                event_type="TX", submitter="GABC",
                metadata=b"", event_hash=bytes(32), prev_hash=bytes(32),
            )

        client = AuditLedgerClient.__new__(AuditLedgerClient)
        client.contract_id = "CTEST"
        client.server = MagicMock()
        client.source = None
<<<<<<< fix/127-stream-events
        client.total_events = MagicMock(side_effect=event_counts)
        client.get_event_by_order = MagicMock(side_effect=_make_event)
        return client

    def test_yields_existing_events_in_order(self):
        client = self._make_streaming_client([3, 3])
        gen = client.stream_events(after_index=0, poll_interval_s=0)
        with patch("time.sleep"):
            events = [next(gen) for _ in range(3)]
        assert [e.index for e in events] == [0, 1, 2]

    def test_resumes_from_after_index(self):
        client = self._make_streaming_client([5, 5])
        gen = client.stream_events(after_index=3, poll_interval_s=0)
        with patch("time.sleep"):
            events = [next(gen) for _ in range(2)]
        assert [e.index for e in events] == [3, 4]

    def test_yields_new_events_as_they_appear(self):
        client = self._make_streaming_client([2, 4, 4])
        gen = client.stream_events(after_index=0, poll_interval_s=0)
        with patch("time.sleep"):
            events = [next(gen) for _ in range(4)]
        assert [e.index for e in events] == [0, 1, 2, 3]

    def test_no_events_sleeps(self):
        client = self._make_streaming_client([0, 0, 1])
        gen = client.stream_events(after_index=0, poll_interval_s=1.5)
        with patch("time.sleep") as mock_sleep:
            next(gen)
        assert mock_sleep.call_count >= 2
        mock_sleep.assert_called_with(1.5)
=======
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
>>>>>>> master
