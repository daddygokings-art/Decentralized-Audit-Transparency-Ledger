"""Integration tests for the Python SDK client.""" 

import hashlib
import struct
import time
from unittest.mock import MagicMock, patch

import pytest

from audit_ledger import AuditLedgerClient
from audit_ledger.models import Event, ContractError, Page


@pytest.fixture
def mock_sdk():
    """Fixture that patches stellar_sdk so we can instantiate the client."""
    with patch("audit_ledger.client.STELLAR_SDK_AVAILABLE", True):
        with patch("audit_ledger.client.SorobanServer") as mock_server:
            with patch("audit_ledger.client.Keypair") as mock_keypair:
                client = AuditLedgerClient(
                    contract_id="CCXMTP7ABCDEF",
                    rpc_url="https://testnet.stellar.org",
                    network_passphrase="Test SDF Network ; September 2015",
                )
                client.server = MagicMock()
                yield client


class TestPythonSDKIntegration:
    """Integration tests for the Python SDK client."""

    def test_initialize(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = None
        mock_sdk.initialize("GOWNER123", 1000)
        mock_sdk.server.invoke_contract.assert_called_once()

    def test_log_event(self, mock_sdk):
        expected_id = "a1b2c3d4e5f6"
        mock_sdk.server.invoke_contract.return_value = {"id": expected_id}
        result = mock_sdk.log_event("GSUBMITTER", "payment", b"tx-data")
        assert result == bytes.fromhex(expected_id)

    def test_log_events_batch(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = [0, 1, 2]
        events = [
            {"submitter": "GA", "event_type": "payment", "metadata": b"a"},
            {"submitter": "GA", "event_type": "refund", "metadata": b"b"},
            {"submitter": "GB", "event_type": "audit", "metadata": b"c"},
        ]
        indices = mock_sdk.log_events(events)
        assert indices == [0, 1, 2]

    def test_total_events(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = {"value": 42}
        total = mock_sdk.total_events()
        assert total == 42

    def test_get_event(self, mock_sdk):
        raw = {
            "index": 0,
            "timestamp": 1000,
            "event_type": "payment",
            "submitter": "GA",
            "metadata": "74782d64617461",
            "event_hash": "00" * 32,
            "prev_hash": "00" * 32,
        }
        mock_sdk.server.invoke_contract.return_value = raw
        event = mock_sdk.get_event(b"some_id")
        assert isinstance(event, Event)
        assert event.index == 0
        assert event.event_type == "payment"

    def test_get_event_by_order(self, mock_sdk):
        raw = {
            "index": 5,
            "timestamp": 2000,
            "event_type": "audit",
            "submitter": "GB",
            "metadata": "",
            "event_hash": "11" * 32,
            "prev_hash": "00" * 32,
        }
        mock_sdk.server.invoke_contract.return_value = raw
        event = mock_sdk.get_event_by_order(5)
        assert event.index == 5
        assert event.submitter == "GB"

    def test_event_count(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = {"count": 7}
        count = mock_sdk.event_count("payment")
        assert count == 7

    def test_get_event_by_type(self, mock_sdk):
        raw = {
            "index": 2,
            "timestamp": 1500,
            "event_type": "refund",
            "submitter": "GC",
            "metadata": "",
            "event_hash": "22" * 32,
            "prev_hash": "11" * 32,
        }
        mock_sdk.server.invoke_contract.return_value = raw
        event = mock_sdk.get_event_by_type("refund", 2)
        assert event.event_type == "refund"
        assert event.index == 2

    def test_governance_set_global_max(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = None
        mock_sdk.set_global_max_logs("GOWNER", 500)
        mock_sdk.server.invoke_contract.assert_called_once()

    def test_governance_set_event_max(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = None
        mock_sdk.set_event_max_logs("GOWNER", "payment", 100)
        mock_sdk.server.invoke_contract.assert_called_once()

    def test_governance_transfer_ownership(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = None
        mock_sdk.transfer_ownership("GOWNER", "GNEWOWNER")
        mock_sdk.server.invoke_contract.assert_called_once()

    def test_metadata_size_management(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = {"size": 2048}
        size = mock_sdk.get_metadata_max_size("payment")
        assert size == 2048

    def test_verify_integrity(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = {"result": True}
        assert mock_sdk.verify_integrity() is True

    def test_verify_integrity_range(self, mock_sdk):
        mock_sdk.server.invoke_contract.return_value = {"result": True}
        assert mock_sdk.verify_integrity_range(0, 10) is True

    def test_compute_event_id_matches_contract(self):
        contract_id = "CCXMTP7ABCDEF"
        submitter = "GABCDEF123"
        event_type = "payment"
        metadata = b"test-data"
        timestamp = 1000
        index = 0

        result = AuditLedgerClient.compute_event_id(
            contract_id, submitter, event_type, metadata, timestamp, index
        )

        expected = hashlib.sha256(
            contract_id.encode()
            + submitter.encode()
            + event_type.encode()
            + metadata
            + struct.pack("<Q", timestamp)
            + struct.pack("<I", index)
        ).digest()
        assert result == expected

    def test_get_events_pagination(self, mock_sdk):
        mock_sdk.server.invoke_contract.side_effect = [
            {"value": 10},
        ] + [
            {
                "index": i,
                "timestamp": 1000 + i,
                "event_type": "payment",
                "submitter": f"G{i}",
                "metadata": f"{i}".encode().hex(),
                "event_hash": "00" * 32,
                "prev_hash": "00" * 32,
            }
            for i in range(10)
        ]

        total = mock_sdk.total_events()
        assert total == 10

    def test_contract_error_propagation(self, mock_sdk):
        class MockRPCError(Exception):
            pass

        mock_sdk.server.invoke_contract.side_effect = MockRPCError("Error(Contract, #1)")
        with pytest.raises(ContractError) as exc:
            mock_sdk.set_global_max_logs("GATTACKER", 100)
        assert exc.value.code == 1


class TestEventModel:
    def test_from_dict_round_trip(self):
        raw = {
            "index": 0,
            "timestamp": 1000,
            "event_type": "payment",
            "submitter": "GA",
            "metadata": "74782d64617461",
            "event_hash": "ab" * 32,
            "prev_hash": "00" * 32,
        }
        event = Event.from_dict(raw)
        assert event.index == 0
        assert event.timestamp == 1000
        assert event.event_type == "payment"
        assert event.submitter == "GA"
        assert event.metadata == b"tx-data"
        assert len(event.event_hash) == 32

    def test_from_dict_defaults(self):
        raw = {
            "index": 5,
            "timestamp": 2000,
            "event_type": "audit",
            "submitter": "GB",
        }
        event = Event.from_dict(raw)
        assert event.metadata == b""
        assert event.event_hash == bytes(32)
        assert event.prev_hash == bytes(32)

    def test_from_dict_empty_metadata(self):
        raw = {
            "index": 0,
            "timestamp": 1000,
            "event_type": "test",
            "submitter": "GA",
            "metadata": "",
        }
        event = Event.from_dict(raw)
        assert event.metadata == b""


class TestContractError:
    def test_known_error_code(self):
        err = ContractError(1)
        assert err.code == 1

    def test_unknown_error_code(self):
        err = ContractError(99)
        assert err.code == 99

    def test_error_inherits_exception(self):
        assert issubclass(ContractError, Exception)
