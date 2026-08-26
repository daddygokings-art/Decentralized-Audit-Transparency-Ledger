"""Tests for AuditLedgerClient health check methods (#239)."""

from __future__ import annotations

import sys
import types
from unittest.mock import MagicMock, patch

import pytest


def _stub_stellar_sdk():
    stub = types.ModuleType("stellar_sdk")
    stub.SorobanServer = MagicMock
    stub.Keypair = MagicMock
    stub.soroban = types.ModuleType("stellar_sdk.soroban")
    stub.soroban.SorobanClient = MagicMock
    sys.modules.setdefault("stellar_sdk", stub)
    sys.modules.setdefault("stellar_sdk.soroban", stub.soroban)
    return stub


def _make_client():
    _stub_stellar_sdk()
    for mod in list(sys.modules):
        if mod.startswith("audit_ledger.client"):
            del sys.modules[mod]
    from audit_ledger.client import AuditLedgerClient

    client = AuditLedgerClient.__new__(AuditLedgerClient)
    client.contract_id = "CTEST"
    client.rpc_url = "https://soroban-testnet.stellar.org"
    client.network_passphrase = "Test SDF Network ; September 2015"
    client.server = MagicMock()
    client.source = None
    return client


class TestValidateRpcEndpoint:
    """Tests for AuditLedgerClient.validate_rpc_endpoint (static, offline)."""

    def setup_method(self):
        _stub_stellar_sdk()
        for mod in list(sys.modules):
            if mod.startswith("audit_ledger.client"):
                del sys.modules[mod]

    def _cls(self):
        from audit_ledger.client import AuditLedgerClient
        return AuditLedgerClient

    def test_valid_https_url(self):
        self._cls().validate_rpc_endpoint("https://soroban-testnet.stellar.org")

    def test_valid_http_url(self):
        self._cls().validate_rpc_endpoint("http://localhost:8000")

    def test_empty_url_raises(self):
        with pytest.raises(ValueError, match="must not be empty"):
            self._cls().validate_rpc_endpoint("")

    def test_missing_scheme_raises(self):
        with pytest.raises(ValueError, match="must start with"):
            self._cls().validate_rpc_endpoint("soroban-testnet.stellar.org")

    def test_ftp_scheme_raises(self):
        with pytest.raises(ValueError, match="must start with"):
            self._cls().validate_rpc_endpoint("ftp://example.com")

    def test_uppercase_https_is_accepted(self):
        # scheme check is case-insensitive
        self._cls().validate_rpc_endpoint("HTTPS://example.com")


class TestCheckConnectivity:
    """Tests for AuditLedgerClient.check_connectivity."""

    def test_returns_true_for_valid_url(self):
        client = _make_client()
        assert client.check_connectivity() is True

    def test_returns_false_for_invalid_url(self):
        client = _make_client()
        client.rpc_url = "not-a-url"
        assert client.check_connectivity() is False

    def test_returns_false_when_validate_raises(self):
        client = _make_client()
        client.rpc_url = ""
        assert client.check_connectivity() is False


class TestHealthCheck:
    """Tests for AuditLedgerClient.health_check."""

    def test_healthy_when_all_ok(self):
        client = _make_client()
        client.total_events = MagicMock(return_value=5)
        result = client.health_check()
        assert result["ok"] is True
        assert result["rpc_reachable"] is True
        assert result["contract_reachable"] is True
        assert result["error"] is None
        assert result["rpc_url"] == client.rpc_url
        assert result["contract_id"] == client.contract_id

    def test_unhealthy_when_rpc_url_invalid(self):
        client = _make_client()
        client.rpc_url = "bad-url"
        result = client.health_check()
        assert result["ok"] is False
        assert result["rpc_reachable"] is False
        assert result["error"] is not None

    def test_degraded_when_contract_unreachable(self):
        client = _make_client()
        from audit_ledger.models import RPCError
        client.total_events = MagicMock(side_effect=RPCError("timeout"))
        result = client.health_check()
        assert result["ok"] is False
        assert result["rpc_reachable"] is True
        assert result["contract_reachable"] is False
        assert "timeout" in result["error"]

    def test_result_contains_all_keys(self):
        client = _make_client()
        client.total_events = MagicMock(return_value=0)
        result = client.health_check()
        for key in ("ok", "rpc_reachable", "contract_reachable", "rpc_url",
                    "contract_id", "error"):
            assert key in result


class TestHealthStatus:
    """Tests for AuditLedgerClient.health_status."""

    def test_healthy_string(self):
        client = _make_client()
        client.total_events = MagicMock(return_value=0)
        assert client.health_status() == "healthy"

    def test_unhealthy_string_on_bad_url(self):
        client = _make_client()
        client.rpc_url = "not-valid"
        assert client.health_status() == "unhealthy"

    def test_degraded_string_on_contract_error(self):
        client = _make_client()
        from audit_ledger.models import RPCError
        client.total_events = MagicMock(side_effect=RPCError("down"))
        assert client.health_status() == "degraded"
