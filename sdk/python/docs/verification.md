# Verification Reference

Full documentation for the `audit_ledger.verification` module (issue #247).

---

## Event ID verification

### `compute_event_id(...) -> bytes`

Recompute the content-addressed event ID off-chain, matching the contract's
`compute_event_id` implementation.

```python
from audit_ledger.verification import compute_event_id

event_id = compute_event_id(
    contract_id="CCXMTP7...",
    submitter="GABCDEF...",
    event_type="payment",
    metadata=b'{"amount":"100"}',
    timestamp=1_700_000_000,
    index=0,
)
print(event_id.hex())
```

**Signature**

```python
compute_event_id(
    contract_id: str,
    submitter: str,
    event_type: str,
    metadata: bytes,
    timestamp: int,
    index: int,
) -> bytes  # 32-byte SHA-256 digest
```

---

### `verify_event_id(event, contract_id, expected_id) -> bool`

Verify that an event's recomputed ID matches the expected stored ID.

```python
from audit_ledger.verification import verify_event_id
from audit_ledger.exceptions import EventIDMismatchError

try:
    verify_event_id(event, contract_id="C...", expected_id=stored_id)
    print("ID matches ✓")
except EventIDMismatchError as exc:
    print(f"Mismatch: {exc.context}")
```

**Raises** `EventIDMismatchError` on mismatch.

---

## Hash chain verification

### `compute_chain_hash(event_id, prev_hash) -> bytes`

Compute the chain hash that links two consecutive events:
`SHA-256(event_id || prev_hash)`.

```python
from audit_ledger.verification import compute_chain_hash

chain_hash = compute_chain_hash(event.event_hash, prev_event.event_hash)
```

---

### `verify_hash_chain(events) -> bool`

Verify the hash chain across a sequence of consecutive events in ascending
order.

```python
from audit_ledger.verification import verify_hash_chain
from audit_ledger.exceptions import HashChainError, VerificationError

events = [client.get_event_by_order(i) for i in range(100)]
try:
    verify_hash_chain(events)
    print("Chain intact ✓")
except HashChainError as exc:
    print(f"Chain broken at index {exc.event_index}")
except VerificationError as exc:
    print(f"Verification error: {exc}")
```

**Rules**

- The first event's `prev_hash` must be `bytes(32)` (all zeros).
- Each subsequent event's `prev_hash` must equal the preceding event's `event_hash`.
- Events with `None` hash fields raise `VerificationError`.

**Returns** `True` if the chain is intact; raises otherwise.

---

## Signature verification

### `verify_event_signature(event_id, pubkey, signature) -> bool`

Verify an Ed25519 signature over an event ID using `stellar-sdk`.

```python
from audit_ledger.verification import verify_event_signature
from audit_ledger.exceptions import SignatureVerificationError

try:
    verify_event_signature(event_id, pubkey=pubkey_bytes, signature=sig_bytes)
    print("Signature valid ✓")
except SignatureVerificationError as exc:
    print(f"Invalid: {exc}")
```

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `event_id` | `bytes` | 32-byte event ID (the signed message). |
| `pubkey` | `bytes` | 32-byte Ed25519 public key. |
| `signature` | `bytes` | 64-byte Ed25519 signature. |

**Raises** `SignatureVerificationError` if verification fails or `stellar-sdk`
is not installed.

---

## Integrity proof

### `IntegrityProof`

A dataclass capturing the result of verifying a sequence of events.

```python
from audit_ledger.verification import generate_integrity_proof

proof = generate_integrity_proof(events)
print(proof.is_valid)       # True / False
print(proof.hex_root())     # Merkle root as hex string
print(proof.to_dict())      # Full dict representation
```

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `event_count` | `int` | Number of events in the proof. |
| `first_index` | `int` | Index of the first event. |
| `last_index` | `int` | Index of the last event. |
| `root_hash` | `bytes` | 32-byte Merkle root of all event hashes. |
| `chain_valid` | `bool` | Whether hash chain verification passed. |
| `errors` | `List[str]` | Error messages for any failed checks. |

**Properties**

- `is_valid` — `True` if `chain_valid` and `errors` is empty.

**Methods**

- `hex_root()` — Root hash as a lowercase hex string.
- `to_dict()` — Serialise to a plain dict.

---

### `generate_integrity_proof(events, verify_chain=True) -> IntegrityProof`

Generate an integrity proof for a sequence of events.

```python
from audit_ledger.verification import generate_integrity_proof

proof = generate_integrity_proof(events, verify_chain=True)
if proof.is_valid:
    print(f"✓ All {proof.event_count} events verified. Root: {proof.hex_root()}")
else:
    for err in proof.errors:
        print(f"  ✗ {err}")
```

**Parameters**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `events` | — | Sequence of `Event` objects in ascending order. |
| `verify_chain` | `True` | Whether to run hash chain verification. |

The Merkle root is computed using a standard binary Merkle tree over the
`event_hash` values. An odd number of leaves is handled by duplicating the
last leaf.
