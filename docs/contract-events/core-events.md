# Core Ledger & Administrative Events

This document details all core audit trail, administrative lifecycle, and tamper-evidence events emitted by the core Soroban contract (`src/lib.rs`).

---

## 1. `event_stored`

Emitted whenever a new audit record is appended to the immutable log.

- **Topic 0**: `Symbol("event_stored")`
- **Topic 1**: `submitter: Address`
- **Payload**: `(index: u64, timestamp: u64, topic: Symbol, hash: BytesN<32>)`

### Example Payload

```json
{
  "index": 1042,
  "submitter": "GAK4K6K4Z67A5M7X5SLLM2...",
  "timestamp": 1756483200,
  "topic": "compliance_audit",
  "hash": "0x7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
}
```

---

## 2. `events_archived`

Emitted when historical records are compressed or pruned to cold storage / decentralized storage (IPFS/Arweave).

- **Topic 0**: `Symbol("events_archived")`
- **Payload**: `(archived_count: u32, root_hash: BytesN<32>)`

### Example Payload

```json
{
  "archived_count": 5000,
  "root_hash": "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

---

## 3. `contract_paused` / `contract_unpaused`

Emitted during emergency administrative operations or scheduled maintenance.

- **Topic 0**: `Symbol("contract_paused")` / `Symbol("contract_unpaused")`
- **Topic 1**: `admin: Address`
- **Payload**: `(reason: Symbol, timestamp: u64)`

---

## 4. `owner_added` / `owner_removed`

Emitted when multi-signature governance owners are modified.

- **Topic 0**: `Symbol("owner_added")` / `Symbol("owner_removed")`
- **Topic 1**: `target_owner: Address`
- **Payload**: `(new_quorum: u32, total_owners: u32)`

---

## 5. `proposal_submitted` / `proposal_voted` / `proposal_executed`

Emitted across the multi-signature administrative proposal lifecycle.

- **Topic 0**: `Symbol("proposal_submitted")`
- **Topic 1**: `proposal_id: u64`
- **Payload**: `(proposer: Address, action_type: Symbol, required_votes: u32)`
