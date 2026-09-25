# GENERATED FILE — DO NOT EDIT.
# Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
# Source of truth: abi/audit-ledger.json
"""

Contract data types for the AuditLedger Soroban contract.

Field names are preserved verbatim from the contract IDL so a value can be
handed straight to the RPC layer without translation.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Any, Dict, List, Mapping, Optional, Tuple

__all__ = [
    "SchemaFormat",
    "SchemaCompatibility",
    "Schema",
    "MigrationFunction",
    "Event",
    "EventHeader",
    "EventVersion",
    "NonceState",
    "ContractStatistics",
    "ProposalAction",
    "Snapshot",
    "TtlCleanupStats",
    "Role",
    "DedupPolicy",
    "ArchiveConfig",
    "ArchivedEventRef",
    "ArchiveStats",
    "FieldChange",
    "VersionComparison",
]

class SchemaFormat(int, Enum):
    """Contract enum SchemaFormat (3 unit variant(s))."""

    JsonSchemaDraft7 = 0
    JsonSchema201909 = 1
    Protobuf = 2

    @classmethod
    def from_wire(cls, value: int) -> "SchemaFormat":
        """Decode a wire ordinal, raising `ValueError` on unknown values."""
        try:
            return cls(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"Unknown SchemaFormat ordinal: {value}") from exc

class SchemaCompatibility(int, Enum):
    """Contract enum SchemaCompatibility (5 unit variant(s))."""

    Full = 0
    Backward = 1
    Forward = 2
    Breaking = 3
    Unknown = 4

    @classmethod
    def from_wire(cls, value: int) -> "SchemaCompatibility":
        """Decode a wire ordinal, raising `ValueError` on unknown values."""
        try:
            return cls(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"Unknown SchemaCompatibility ordinal: {value}") from exc

@dataclass(frozen=True)
class Schema:
    """Contract type Schema (4 field(s))."""

    format: SchemaFormat
    version: int
    definition: str
    compatibility: SchemaCompatibility

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "format": self.format,
            "version": self.version,
            "definition": self.definition,
            "compatibility": self.compatibility,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "Schema":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            format=payload["format"],
            version=payload["version"],
            definition=payload["definition"],
            compatibility=payload["compatibility"],
        )

@dataclass(frozen=True)
class MigrationFunction:
    """Contract type MigrationFunction (4 field(s))."""

    from_version: int
    to_version: int
    name: str
    body: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "from_version": self.from_version,
            "to_version": self.to_version,
            "name": self.name,
            "body": self.body,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "MigrationFunction":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            from_version=payload["from_version"],
            to_version=payload["to_version"],
            name=payload["name"],
            body=payload["body"],
        )

@dataclass(frozen=True)
class Event:
    """Contract type Event (11 field(s))."""

    index: int
    timestamp: int
    event_type: str
    category: str
    submitter: str
    metadata: str
    sub_event_type: Optional[str]
    version: int
    event_hash: str
    prev_hash: str
    parent_event_id: Optional[str]

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "index": self.index,
            "timestamp": self.timestamp,
            "event_type": self.event_type,
            "category": self.category,
            "submitter": self.submitter,
            "metadata": self.metadata,
            "sub_event_type": self.sub_event_type,
            "version": self.version,
            "event_hash": self.event_hash,
            "prev_hash": self.prev_hash,
            "parent_event_id": self.parent_event_id,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "Event":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            index=payload["index"],
            timestamp=payload["timestamp"],
            event_type=payload["event_type"],
            category=payload["category"],
            submitter=payload["submitter"],
            metadata=payload["metadata"],
            sub_event_type=payload["sub_event_type"],
            version=payload["version"],
            event_hash=payload["event_hash"],
            prev_hash=payload["prev_hash"],
            parent_event_id=payload["parent_event_id"],
        )

@dataclass(frozen=True)
class EventHeader:
    """Contract type EventHeader (4 field(s))."""

    index: int
    timestamp: int
    event_type: str
    submitter: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "index": self.index,
            "timestamp": self.timestamp,
            "event_type": self.event_type,
            "submitter": self.submitter,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "EventHeader":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            index=payload["index"],
            timestamp=payload["timestamp"],
            event_type=payload["event_type"],
            submitter=payload["submitter"],
        )

@dataclass(frozen=True)
class EventVersion:
    """Contract type EventVersion (4 field(s))."""

    version: int
    data: Event
    updated_at: int
    updated_by: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "version": self.version,
            "data": self.data,
            "updated_at": self.updated_at,
            "updated_by": self.updated_by,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "EventVersion":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            version=payload["version"],
            data=payload["data"],
            updated_at=payload["updated_at"],
            updated_by=payload["updated_by"],
        )

@dataclass(frozen=True)
class NonceState:
    """Contract type NonceState (3 field(s))."""

    last_nonce: int
    window_size: int
    max_nonce: int

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "last_nonce": self.last_nonce,
            "window_size": self.window_size,
            "max_nonce": self.max_nonce,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "NonceState":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            last_nonce=payload["last_nonce"],
            window_size=payload["window_size"],
            max_nonce=payload["max_nonce"],
        )

@dataclass(frozen=True)
class ContractStatistics:
    """Contract type ContractStatistics (6 field(s))."""

    total_events: int
    events_by_type: List[Tuple[str, int]]
    events_last_hour: int
    events_last_day: int
    events_last_week: int
    top_submitters: List[Tuple[str, int]]

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "total_events": self.total_events,
            "events_by_type": self.events_by_type,
            "events_last_hour": self.events_last_hour,
            "events_last_day": self.events_last_day,
            "events_last_week": self.events_last_week,
            "top_submitters": self.top_submitters,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "ContractStatistics":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            total_events=payload["total_events"],
            events_by_type=payload["events_by_type"],
            events_last_hour=payload["events_last_hour"],
            events_last_day=payload["events_last_day"],
            events_last_week=payload["events_last_week"],
            top_submitters=payload["top_submitters"],
        )

@dataclass(frozen=True)
class ProposalAction:
    """Contract enum ProposalAction (9 payload variant(s))."""

    variant: str
    ordinal: int
    value: Optional[Tuple[Any, ...]] = None

    @staticmethod
    def TransferOwnership(field_0: str) -> "ProposalAction":
        return ProposalAction("TransferOwnership", 0, (field_0,))

    @staticmethod
    def AddOwner(field_0: str) -> "ProposalAction":
        return ProposalAction("AddOwner", 1, (field_0,))

    @staticmethod
    def RemoveOwner(field_0: str) -> "ProposalAction":
        return ProposalAction("RemoveOwner", 2, (field_0,))

    @staticmethod
    def SetRequiredSignatures(field_0: int) -> "ProposalAction":
        return ProposalAction("SetRequiredSignatures", 3, (field_0,))

    @staticmethod
    def SetGlobalMaxLogs(field_0: int) -> "ProposalAction":
        return ProposalAction("SetGlobalMaxLogs", 4, (field_0,))

    @staticmethod
    def SetMetadataSchema(field_0: str, field_1: str) -> "ProposalAction":
        return ProposalAction("SetMetadataSchema", 5, (field_0, field_1,))

    @staticmethod
    def RollbackEvent(field_0: int, field_1: int) -> "ProposalAction":
        return ProposalAction("RollbackEvent", 6, (field_0, field_1,))

    @staticmethod
    def Pause() -> "ProposalAction":
        return ProposalAction("Pause", 7, None)

    @staticmethod
    def Unpause() -> "ProposalAction":
        return ProposalAction("Unpause", 8, None)

    @classmethod
    def variants(cls) -> Tuple[str, ...]:
        """Return every variant name, in wire order."""
        return (
            "TransferOwnership",
            "AddOwner",
            "RemoveOwner",
            "SetRequiredSignatures",
            "SetGlobalMaxLogs",
            "SetMetadataSchema",
            "RollbackEvent",
            "Pause",
            "Unpause",
        )

@dataclass(frozen=True)
class Snapshot:
    """Contract type Snapshot (5 field(s))."""

    id_: int
    timestamp: int
    event_count: int
    event_hash: str
    description: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "id": self.id_,
            "timestamp": self.timestamp,
            "event_count": self.event_count,
            "event_hash": self.event_hash,
            "description": self.description,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "Snapshot":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            id_=payload["id"],
            timestamp=payload["timestamp"],
            event_count=payload["event_count"],
            event_hash=payload["event_hash"],
            description=payload["description"],
        )

@dataclass(frozen=True)
class TtlCleanupStats:
    """Contract type TtlCleanupStats (4 field(s))."""

    runs: int
    ttl_extensions: int
    cleaned: int
    last_run_ledger: int

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "runs": self.runs,
            "ttl_extensions": self.ttl_extensions,
            "cleaned": self.cleaned,
            "last_run_ledger": self.last_run_ledger,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "TtlCleanupStats":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            runs=payload["runs"],
            ttl_extensions=payload["ttl_extensions"],
            cleaned=payload["cleaned"],
            last_run_ledger=payload["last_run_ledger"],
        )

class Role(int, Enum):
    """Contract enum Role (4 unit variant(s))."""

    Admin = 0
    Auditor = 1
    Submitter = 2
    Viewer = 3

    @classmethod
    def from_wire(cls, value: int) -> "Role":
        """Decode a wire ordinal, raising `ValueError` on unknown values."""
        try:
            return cls(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"Unknown Role ordinal: {value}") from exc

class DedupPolicy(int, Enum):
    """Contract enum DedupPolicy (4 unit variant(s))."""

    None_ = 0
    ContentHash = 1
    ContentHashWithTimestamp = 2
    Custom = 3

    @classmethod
    def from_wire(cls, value: int) -> "DedupPolicy":
        """Decode a wire ordinal, raising `ValueError` on unknown values."""
        try:
            return cls(value)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"Unknown DedupPolicy ordinal: {value}") from exc

@dataclass(frozen=True)
class ArchiveConfig:
    """Contract type ArchiveConfig (3 field(s))."""

    offchain_storage: bool
    base_url: str
    compression: int

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "offchain_storage": self.offchain_storage,
            "base_url": self.base_url,
            "compression": self.compression,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "ArchiveConfig":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            offchain_storage=payload["offchain_storage"],
            base_url=payload["base_url"],
            compression=payload["compression"],
        )

@dataclass(frozen=True)
class ArchivedEventRef:
    """Contract type ArchivedEventRef (5 field(s))."""

    id_: str
    index: int
    checksum: str
    url: str
    archived_at: int

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "id": self.id_,
            "index": self.index,
            "checksum": self.checksum,
            "url": self.url,
            "archived_at": self.archived_at,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "ArchivedEventRef":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            id_=payload["id"],
            index=payload["index"],
            checksum=payload["checksum"],
            url=payload["url"],
            archived_at=payload["archived_at"],
        )

@dataclass(frozen=True)
class ArchiveStats:
    """Contract type ArchiveStats (3 field(s))."""

    total_archived: int
    total_compressed: int
    total_offchain: int

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "total_archived": self.total_archived,
            "total_compressed": self.total_compressed,
            "total_offchain": self.total_offchain,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "ArchiveStats":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            total_archived=payload["total_archived"],
            total_compressed=payload["total_compressed"],
            total_offchain=payload["total_offchain"],
        )

@dataclass(frozen=True)
class FieldChange:
    """Contract type FieldChange (3 field(s))."""

    field: str
    from_: str
    to: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "field": self.field,
            "from": self.from_,
            "to": self.to,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "FieldChange":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            field=payload["field"],
            from_=payload["from"],
            to=payload["to"],
        )

@dataclass(frozen=True)
class VersionComparison:
    """Contract type VersionComparison (7 field(s))."""

    index: int
    from_version: int
    to_version: int
    same: bool
    changes: List[FieldChange]
    from_hash: str
    to_hash: str

    def to_wire(self) -> Dict[str, Any]:
        """Return the field mapping expected by the RPC invoke layer."""
        return {
            "index": self.index,
            "from_version": self.from_version,
            "to_version": self.to_version,
            "same": self.same,
            "changes": self.changes,
            "from_hash": self.from_hash,
            "to_hash": self.to_hash,
        }

    @classmethod
    def from_wire(cls, payload: Mapping[str, Any]) -> "VersionComparison":
        """Build the dataclass from an RPC payload keyed by contract field name."""
        return cls(
            index=payload["index"],
            from_version=payload["from_version"],
            to_version=payload["to_version"],
            same=payload["same"],
            changes=payload["changes"],
            from_hash=payload["from_hash"],
            to_hash=payload["to_hash"],
        )

# contract: AuditLedger v0.1.0
# idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
