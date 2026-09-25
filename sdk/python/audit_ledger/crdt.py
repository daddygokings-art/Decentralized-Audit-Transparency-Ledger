"""Conflict-free replicated data types for collaborative contract events.

The CRDT deliberately uses only the Python standard library. Each editable
field is a last-writer-wins register, where the logical clock and actor id
provide a deterministic total order. Applying an operation more than once is
therefore safe and replicas converge after exchanging operations.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Iterable, Mapping, Optional, Union

from .models import Event

__all__ = ["CRDTOperation", "EventCRDT", "EDITABLE_FIELDS"]

EDITABLE_FIELDS = frozenset({"timestamp", "event_type", "submitter", "metadata"})


@dataclass(frozen=True)
class CRDTOperation:
    """An immutable field update that can be sent to another replica."""

    event_id: bytes
    field: str
    value: bytes
    clock: int
    actor: str
    deleted: bool = False

    def __post_init__(self) -> None:
        if not self.event_id:
            raise ValueError("event_id must not be empty")
        if self.field not in EDITABLE_FIELDS and self.field != "__deleted__":
            raise ValueError(f"field is not editable: {self.field}")
        if self.clock < 0:
            raise ValueError("clock must be non-negative")
        if not self.actor:
            raise ValueError("actor must not be empty")

    @property
    def key(self) -> tuple[bytes, str]:
        """Return the register key addressed by this operation."""

        return self.event_id, self.field

    @property
    def order(self) -> tuple[int, str, bytes, bool]:
        """Return the deterministic conflict-resolution order."""

        return self.clock, self.actor, self.value, self.deleted


class EventCRDT:
    """A collaborative replica of one :class:`~audit_ledger.models.Event`.

    The event hash and predecessor hash remain immutable chain identifiers. The
    four descriptive fields can be edited independently; deleting the event is
    represented by a monotonically ordered tombstone register.
    """

    def __init__(self, event: Event) -> None:
        event_id = event.event_hash or event.prev_hash
        if not event_id:
            raise ValueError("event must have an event_hash or prev_hash")
        self._event = event
        self._registers: Dict[tuple[bytes, str], CRDTOperation] = {}
        self._clock = 0

    @property
    def event_id(self) -> bytes:
        return self._event.event_hash or self._event.prev_hash  # type: ignore[return-value]

    def apply(self, operation: CRDTOperation) -> bool:
        """Apply an operation and return whether it changed local state.

        Replaying an old or identical operation is a no-op. Conflicting
        updates are resolved by :attr:`CRDTOperation.order`, independent of
        arrival order.
        """

        if operation.event_id != self.event_id:
            raise ValueError("operation belongs to a different event")
        current = self._registers.get(operation.key)
        if current is None or operation.order > current.order:
            self._registers[operation.key] = operation
            self._clock = max(self._clock, operation.clock)
            return current != operation
        return False

    def merge(self, operations: Iterable[CRDTOperation]) -> int:
        """Merge operations received from another replica.

        Returns the number of operations that changed this replica. The method
        is safe to call repeatedly and in any delivery order.
        """

        return sum(1 for operation in operations if self.apply(operation))

    def merge_replicas(self, *replicas: "EventCRDT") -> int:
        """Merge all operations from one or more compatible replicas."""

        return self.merge(
            operation
            for replica in replicas
            for operation in replica.operations()
        )

    def operations(self) -> tuple[CRDTOperation, ...]:
        """Return operations in stable order for transport or persistence."""

        return tuple(sorted(self._registers.values(), key=lambda op: (op.event_id, op.field)))

    def set_field(self, field: str, value: Union[str, int, bytes], actor: str) -> CRDTOperation:
        """Create and apply a local field update."""

        if field not in EDITABLE_FIELDS:
            raise ValueError(f"field is not editable: {field}")
        self._clock += 1
        raw_value = value.encode() if isinstance(value, str) else value
        if isinstance(raw_value, int):
            raw_value = str(raw_value).encode()
        operation = CRDTOperation(self.event_id, field, raw_value, self._clock, actor)
        self.apply(operation)
        return operation

    def delete(self, actor: str) -> CRDTOperation:
        """Create and apply a local event tombstone."""

        self._clock += 1
        operation = CRDTOperation(self.event_id, "__deleted__", b"", self._clock, actor, True)
        self.apply(operation)
        return operation

    def snapshot(self) -> Optional[Event]:
        """Return the converged event, or ``None`` when deleted.

        Field values are decoded according to the existing SDK model. A
        malformed concurrent value cannot make the replica inconsistent: the
        original value is retained for that field.
        """

        if "__deleted__" in self._registers:
            return None
        values: Mapping[str, Union[str, int, bytes]] = {
            "timestamp": self._event.timestamp,
            "event_type": self._event.event_type,
            "submitter": self._event.submitter,
            "metadata": self._event.metadata,
        }
        for (event_id, field), operation in self._registers.items():
            if event_id != self.event_id or field == "__deleted__":
                continue
            raw = operation.value
            try:
                if field == "timestamp":
                    values[field] = int(raw)
                elif field == "metadata":
                    values[field] = raw
                else:
                    values[field] = raw.decode()
            except (UnicodeDecodeError, ValueError):
                continue
        return Event(
            index=self._event.index,
            timestamp=int(values["timestamp"]),
            event_type=str(values["event_type"]),
            submitter=str(values["submitter"]),
            metadata=bytes(values["metadata"]),
            event_hash=self._event.event_hash,
            prev_hash=self._event.prev_hash,
        )

    def converged_with(self, other: "EventCRDT") -> bool:
        """Return whether two replicas have the same resolved state."""

        return self.snapshot() == other.snapshot()

