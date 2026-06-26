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
