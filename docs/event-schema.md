# Event Schema

This document defines the on-chain event data format used by AuditLedger and the conventions off-chain consumers should follow when indexing, auditing, and displaying events.

## Event Struct Specification

Canonical event fields:

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `index` | `u32` | Yes | Auto-incremented, globally unique, zero-based insertion order. |
| `timestamp` | `u64` | Yes | Ledger timestamp in seconds since Unix epoch. |
| `event_type` | `Symbol` | Yes | Short event classifier. Maximum 32 bytes. Use alphanumeric characters and underscores. |
| `submitter` | `Address` | Yes | Stellar account or contract address that authenticated the submission. |
| `metadata` | `Bytes` | Yes | Opaque payload. JSON-encoded UTF-8 bytes are recommended for interoperability. |

Current contract versions also store integrity fields:

| Field | Type | Description |
| --- | --- | --- |
| `category` | `Symbol` | Optional higher-level category. Defaults to `general` when omitted. |
| `sub_event_type` | `Option<Symbol>` | Optional lower-level subtype. |
| `event_hash` | `BytesN<32>` | SHA-256 hash for this event in the contract hash chain. |
| `prev_hash` | `BytesN<32>` | Previous event hash, or 32 zero bytes for the genesis event. |

Consumers should treat unknown fields as forward-compatible extensions and preserve them when exporting raw records.

## Field Constraints

`index`:

- Starts at `0`.
- Increments by exactly one for each accepted event.
- Must be unique across the contract.
- Consumers should report gaps, duplicates, or out-of-order records as integrity failures.

`timestamp`:

- Comes from the ledger timestamp, not from submitter metadata.
- Is expressed in seconds since epoch.
- Should be used for ordering only after `index`; multiple events can share the same timestamp.

`event_type`:

- Maximum 32 bytes.
- Use lowercase snake case: `payment`, `refund`, `compliance_check`, `audit_report`.
- Use only letters, numbers, and underscores.
- Avoid embedding user IDs, invoice IDs, or other high-cardinality values in the type. Put those in `metadata`.

`submitter`:

- Must be the authenticated address passed to `log_event`.
- Off-chain systems should store both the raw address and any known organization or account label separately.

`metadata`:

- Opaque bytes on-chain.
- Recommended format is compact JSON encoded as UTF-8.
- Keep under 1 KB for predictable fees unless the contract governance explicitly raises metadata limits.
- Do not store secrets, credentials, private customer data, or data that cannot be made public.

## Metadata Conventions

Use JSON objects for common event types. Prefer strings for decimal values to avoid floating-point ambiguity.

Payment:

```json
{
  "amount": "100.50",
  "currency": "USD",
  "reference": "INV-001"
}
```

Refund:

```json
{
  "amount": "25.00",
  "currency": "USD",
  "reference": "RF-001",
  "original_reference": "INV-001",
  "reason": "duplicate_payment"
}
```

Compliance check:

```json
{
  "subject": "G...",
  "status": "passed",
  "policy": "kyc_v2",
  "checked_at": "2026-06-27T12:00:00Z"
}
```

Audit report:

```json
{
  "report_id": "AUD-2026-001",
  "scope": "q2_revenue",
  "result": "clean",
  "document_hash": "sha256:..."
}
```

Recommended JSON schema shape:

```json
{
  "type": "object",
  "additionalProperties": true,
  "properties": {
    "reference": {
      "type": "string",
      "maxLength": 128
    },
    "amount": {
      "type": "string",
      "pattern": "^[0-9]+(\\.[0-9]+)?$"
    },
    "currency": {
      "type": "string",
      "minLength": 3,
      "maxLength": 12
    },
    "document_hash": {
      "type": "string"
    }
  }
}
```

Metadata recommendations:

- Include a stable `reference` when the event corresponds to an external invoice, transaction, report, or ticket.
- Store large files off-chain and include a content hash such as `sha256:<hex>`.
- Use ISO 8601 timestamps for metadata timestamps, even though the canonical on-chain `timestamp` is a Unix second.
- Use strings for IDs and decimal amounts.
- Keep field names lowercase snake case.

## Event Type Naming Convention

Use short, stable, lowercase snake-case event types:

- `payment`
- `refund`
- `compliance_check`
- `audit_report`
- `ownership_change`
- `cap_update`

Avoid:

- Spaces or punctuation: `Compliance Check`, `payment.created`
- Version suffixes for every small metadata change: `payment_v1`, `payment_v2`
- High-cardinality types: `payment_inv_001`

When introducing a new type, publish:

- The event type name.
- Metadata schema and examples.
- Whether the event is append-only, corrective, or superseding a previous event.
- Any off-chain verification rules.

Community registries should reserve common names and document who maintains each schema.

## Consumption Guidelines

### Paginate through events

Use `total_events()` to discover the upper bound, then read by global order:

```bash
TOTAL=$(soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" -- total_events)

for ((i = 0; i < TOTAL; i++)); do
  soroban contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_event_by_order \
    --order "$i"
done
```

For resumable indexing, store the next `index` to read. If the last processed index is `249`, resume at `250`.

Consumers should process events in `index` order, not timestamp order.

### Filter by type

Use `event_count(event_type)` to get the type-local count, then call `get_event_by_type(event_type, type_index)`:

```bash
COUNT=$(soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- event_count \
  --event_type payment)

for ((i = 0; i < COUNT; i++)); do
  soroban contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_event_by_type \
    --event_type payment \
    --type_index "$i"
done
```

Type-local order is the order in which matching events were logged. The event's global `index` remains the canonical ledger-wide sequence number.

### Verify authenticity

At minimum, consumers should verify:

- The event was read from the expected `CONTRACT_ID`.
- `index` matches the requested global order.
- The `submitter` is on the consumer's allowlist for the given `event_type`, if the integration requires allowlisting.
- The hash chain is intact by checking each event's `prev_hash` against the previous event's `event_hash`.
- Any metadata `document_hash` or external reference resolves to the expected off-chain artifact.

For signed events, retrieve the signature payload and verify it off-chain against the event ID:

```bash
soroban contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  -- get_event_signature \
  --event_id "$EVENT_ID"
```

Recommended indexing record:

```json
{
  "contract_id": "C...",
  "index": 42,
  "timestamp": 1782561600,
  "event_type": "payment",
  "submitter": "G...",
  "metadata": {
    "amount": "100.50",
    "currency": "USD",
    "reference": "INV-001"
  },
  "event_hash": "hex-or-strkey-encoded-by-indexer",
  "prev_hash": "hex-or-strkey-encoded-by-indexer",
  "ingested_at": "2026-06-27T12:00:05Z"
}
```

## Recommended Practices

- Treat the contract as the source of sequence truth and your database as a cache.
- Make ingestion idempotent by using `(contract_id, index)` as a unique key.
- Re-read the final page after indexing catches up, because new events can arrive while pagination is running.
- Preserve raw metadata bytes alongside parsed JSON so future parsers can recover exact on-chain data.
- Alert on non-contiguous indices, changed event hashes, duplicate global indices, or failed JSON parsing for event types that require JSON.
- Keep per-type schemas backward compatible. Add optional fields instead of changing the meaning of existing fields.
