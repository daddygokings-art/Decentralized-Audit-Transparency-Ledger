from dataclasses import dataclass
from typing import Optional


@dataclass
class Event:
    """Represents a single audit event stored on-chain."""
    index: int
    timestamp: int
    event_type: str
    submitter: str
    metadata: bytes
    event_hash: bytes
    prev_hash: bytes

    @classmethod
    def from_dict(cls, d: dict) -> "Event":
        return cls(
            index=d["index"],
            timestamp=d["timestamp"],
            event_type=d["event_type"],
            submitter=d["submitter"],
            metadata=bytes.fromhex(d.get("metadata", "")),
            event_hash=bytes.fromhex(d.get("event_hash", "00" * 32)),
            prev_hash=bytes.fromhex(d.get("prev_hash", "00" * 32)),
        )


class AuditLedgerError(Exception):
    """Base exception for AuditLedger SDK errors."""


class ContractError(AuditLedgerError):
    """Raised when the contract returns an error code."""

    ERROR_CODES = {
        1: "CallerNotOwner",
        2: "GlobalMaxLogsReached",
        3: "EventTypeMaxLogsReached",
        4: "EventDoesNotExist",
        5: "EventTypeIndexOutOfBounds",
        6: "NewOwnerIsZero",
        7: "CapNotSet",
        8: "MetadataTooLarge",
        9: "InvalidSignature",
    }

    def __init__(self, code: int):
        self.code = code
        self.name = self.ERROR_CODES.get(code, f"UnknownError({code})")
        super().__init__(f"ContractError #{code}: {self.name}")


class RPCError(AuditLedgerError):
    """Raised when the Soroban RPC call fails."""
