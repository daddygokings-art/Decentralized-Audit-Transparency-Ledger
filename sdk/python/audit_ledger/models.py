"""AuditLedger SDK data models."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Generic, List, TypeVar

T = TypeVar("T")


@dataclass
class Page(Generic[T]):
    """A paginated result set."""
    items: List[T]
    total: int
    offset: int
    limit: int


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


# ── Re-export exception classes from exceptions module for backward compatibility.
# The canonical definitions live in audit_ledger.exceptions (issue #249).
from .exceptions import (  # noqa: E402  (import after class defs is intentional)
    AuditLedgerError,
    ContractError,
    RPCError,
)

__all__ = [
    "Page",
    "Event",
    "AuditLedgerError",
    "ContractError",
    "RPCError",
]
