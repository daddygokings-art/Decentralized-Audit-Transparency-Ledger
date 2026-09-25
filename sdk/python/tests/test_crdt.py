from audit_ledger.crdt import CRDTOperation, EventCRDT
from audit_ledger.models import Event


def event() -> Event:
    return Event(
        index=7,
        timestamp=100,
        event_type="payment",
        submitter="GABC",
        metadata=b"invoice-1",
        event_hash=bytes.fromhex("11" * 32),
        prev_hash=bytes.fromhex("00" * 32),
    )


def test_merges_are_idempotent_and_converge_in_any_order():
    left = EventCRDT(event())
    right = EventCRDT(event())
    left.set_field("metadata", b"invoice-2", "alice")
    right.set_field("event_type", "refund", "bob")

    left.merge(right.operations())
    right.merge(left.operations())
    left.merge(left.operations())

    assert left.converged_with(right)
    assert left.snapshot().metadata == b"invoice-2"
    assert right.snapshot().event_type == "refund"


def test_same_clock_conflict_has_deterministic_winner():
    left = EventCRDT(event())
    right = EventCRDT(event())
    left.merge([CRDTOperation(left.event_id, "submitter", b"alice", 1, "alice")])
    right.merge([CRDTOperation(left.event_id, "submitter", b"bob", 1, "bob")])

    left.merge(right.operations())
    right.merge(left.operations())
    assert left.converged_with(right)
    assert left.snapshot().submitter == "bob"


def test_tombstone_propagates_and_rejects_other_events():
    replica = EventCRDT(event())
    tombstone = replica.delete("alice")
    other = EventCRDT(event())
    other.apply(tombstone)

    assert replica.snapshot() is None
    assert other.snapshot() is None
    try:
        other.apply(CRDTOperation(bytes.fromhex("22" * 32), "metadata", b"x", 1, "bob"))
    except ValueError as error:
        assert "different event" in str(error)
    else:
        raise AssertionError("operations for another event must be rejected")
