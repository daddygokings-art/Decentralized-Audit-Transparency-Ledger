# Automated Regulatory Reporting

Complete pipeline for generating, validating, submitting, and tracking regulatory reports to **FINRA, SEC, CFTC, FCA, BaFin, MAS, and MiCA** — backed by an immutable on-chain audit trail.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Supported Authorities and Forms](#supported-authorities-and-forms)
3. [Rust Module Reference](#rust-module-reference)
4. [Report Pipeline](#report-pipeline)
5. [Validation Rules](#validation-rules)
6. [Submission and Retry Policy](#submission-and-retry-policy)
7. [Acknowledgment Tracking](#acknowledgment-tracking)
8. [Audit Trail](#audit-trail)
9. [REST API Reference](#rest-api-reference)
10. [Configuration](#configuration)
11. [Robustness and Fault Tolerance](#robustness-and-fault-tolerance)
12. [Error Codes](#error-codes)
13. [Testing](#testing)
14. [Deployment Notes](#deployment-notes)

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│  Operator / Scheduler                                                 │
│  (triggers generate → validate → submit on schedule or on-demand)    │
└─────────────────────────────┬────────────────────────────────────────┘
                              │ REST API  /regulatory-reports/*
                              ▼
┌──────────────────────────────────────────────────────────────────────┐
│  api/rest/src/regulatory_reporting.ts                                 │
│  13 routes: generate, list, get, validate, submit, acknowledge,       │
│  cancel, submissions, audit-trail, pending, overdue, authorities      │
└─────────────────────────────┬────────────────────────────────────────┘
                              │ calls Soroban contract / off-chain logic
                              ▼
┌──────────────┬──────────────┬──────────────┬──────────────────────────┐
│ regulatory_  │ report_      │ report_      │ submission_tracker.rs     │
│ reporting.rs │ generators.rs│ validation.rs│ State machine + retry     │
│ Core types   │ Per-authority│ Schema +     │                           │
│ enums/structs│ generators   │ business     ├──────────────────────────┤
│              │              │ rules        │ reporting_audit_trail.rs  │
└──────────────┴──────────────┴──────────────│ Hash-chained audit log   │
                                             └──────────────────────────┘
                              │
                              ▼ reads source events from
┌──────────────────────────────────────────────────────────────────────┐
│  AuditLedger (Soroban contract) — append-only event log               │
│  source_event_ids links reports back to on-chain audit events         │
└──────────────────────────────────────────────────────────────────────┘
```

Data flows in one direction: on-chain audit events → report generators → validators → submission tracker → authority API → acknowledgment webhook → audit trail.

---

## Supported Authorities and Forms

### FINRA (US)

| Form | Enum | Description |
|------|------|-------------|
| `FINRA_OATS` | `FinraOATS` | Order Audit Trail System |
| `FINRA_CAT` | `FinraCAT` | Consolidated Audit Trail |
| `FINRA_RULE4370` | `FinraRule4370` | Business Continuity Plan |
| `FINRA_SAR` | `FinraSAR` | Suspicious Activity Report |

Required extra fields for OATS: `mpid`, `orderCount`, `routeCount`.  
Required extra fields for CAT: `mpid`, `catReporterId`, `eventCount`.

### SEC (US)

| Form | Enum | Description |
|------|------|-------------|
| `SEC_FORM_ADV` | `SecFormADV` | Investment Adviser Registration |
| `SEC_FORM_PF` | `SecFormPF` | Reporting for Private Funds |
| `SEC_FORM_13F` | `SecForm13F` | Institutional Investment Manager Holdings |
| `SEC_FORM_NPORT` | `SecFormNPORT` | Monthly Portfolio Investments |
| `SEC_SAR` | `SecSAR` | Suspicious Activity Report (FinCEN) |

Required extra fields for Form ADV: `adviserName`, `aumUsd`, `clientCount`.

### CFTC (US)

| Form | Enum | Description |
|------|------|-------------|
| `CFTC_LARGE_TRADER` | `CftcLargeTrader` | Large Trader Reporting |
| `CFTC_SWAP_DATA` | `CftcSwapData` | Swap Data Repository report |
| `CFTC_PART20` | `CftcPart20` | Part 20 large trader swaps |
| `CFTC_FORM40` | `CftcForm40` | Statement of Reporting Trader |

Required extra fields for Swap Data: `swapType`, `notionalUsd`, `counterpartyLei`, `uti`.

### FCA (UK)

| Form | Enum | Description |
|------|------|-------------|
| `FCA_MIFID_II` | `FcaMiFIDII` | MiFID II Transaction Report |
| `FCA_EMIR` | `FcaEMIR` | EMIR Trade Repository Report |
| `FCA_STOR` | `FcaSTOR` | Suspicious Transaction and Order Report |
| `FCA_COREP` | `FcaCOREP` | Capital Adequacy Report |

Required extra fields for MiFID II: `isin`, `quantity`, `price`, `venueMic`, `executingEntityId`.

### BaFin (Germany)

| Form | Enum | Description |
|------|------|-------------|
| `BAFIN_WPHG` | `BaFinWpHG` | Securities Trading Act (WpHG) |
| `BAFIN_MELDEPFLICHT` | `BaFinMeldepflicht` | Notification obligation |
| `BAFIN_ANACREDIT` | `BaFinAnaCredit` | Credit data reporting |
| `BAFIN_AML` | `BaFinAML` | Anti-money laundering report |

Required extra fields for WpHG: `isin`, `votingRightsPct`, `thresholdCrossed`, `direction`.

### MAS (Singapore)

| Form | Enum | Description |
|------|------|-------------|
| `MAS_SGX` | `MasSGX` | SGX market position report |
| `MAS_TRR` | `MasTRR` | Trade Repository Report |
| `MAS_FORM610` | `MasForm610` | Statistical return (Form 610) |
| `MAS_CMS` | `MasCMS` | Capital Markets Services disclosure |

Required extra fields for TRR: `productType`, `notionalSgd`, `counterpartyLei`, `uti`.

### MiCA (EU)

| Form | Enum | Description |
|------|------|-------------|
| `MICA_CASP` | `MiCACASP` | Crypto-Asset Service Provider report |
| `MICA_WHITE_PAPER` | `MiCAWhitePaper` | White paper / issuance disclosure |
| `MICA_RESERVE_ASSET` | `MiCAReserveAsset` | Reserve asset backing report (ARTs/EMTs) |
| `MICA_SIGNIFICANT` | `MiCASignificant` | Significant CASP enhanced obligations |

Required extra fields for CASP: `serviceType`, `userCount`, `transactionVolumeEur`, `countriesServed`.  
Required extra fields for Reserve Asset: `tokenSymbol`, `tokensOutstanding`, `reserveValueEur`, `reserveComposition`, `custodianLei`.

---

## Rust Module Reference

### `src/regulatory_reporting.rs` — Core Types

```rust
pub enum RegulatoryAuthority { FINRA, SEC, CFTC, FCA, BaFin, MAS, MiCA }
pub enum ReportFormat { FinraOATS, FinraCAT, ..., MiCACASP, MiCAReserveAsset, ... }
pub enum ReportStatus { Draft, Validated, Submitted, Acknowledged, Accepted, Rejected, Cancelled, Overdue }
pub enum ReportAction { Generated, Validated, Submitted, AcknowledgmentReceived, Accepted, Rejected, ... }

pub struct RegulatoryReport { id, authority, format, entity, lei, period_start, period_end, deadline, content, ... }
pub struct RegulatorySubmission { id, report_id, attempt, submitted_at, endpoint, reference_number, ... }
pub struct SubmissionAcknowledgment { id, submission_id, report_id, reference_number, accepted, ... }
pub struct ReportingAuditEntry { sequence, report_id, action, actor, timestamp, prev_entry_hash, entry_hash, ... }
pub struct AuthorityConfig { authority, enabled, endpoint, credential_ref, max_retries, ... }
```

### `src/report_generators.rs` — Generators

Each authority has a dedicated struct with methods per form:

```rust
FinraGenerators::oats(input, now)        // OATS
FinraGenerators::cat(input, now)         // CAT
SecGenerators::form_adv(input, now)      // Form ADV
SecGenerators::form_pf(input, now)       // Form PF
CftcGenerators::large_trader(input, now) // Large Trader
CftcGenerators::swap_data(input, now)    // Swap Data
FcaGenerators::mifid_ii(input, now)      // MiFID II
FcaGenerators::emir(input, now)          // EMIR
BaFinGenerators::wphg(input, now)        // WpHG
BaFinGenerators::anacredit(input, now)   // AnaCredit
MasGenerators::trr(input, now)           // TRR
MasGenerators::form_610(input, now)      // Form 610
MiCaGenerators::casp(input, now)         // CASP
MiCaGenerators::reserve_asset(input, now) // Reserve Asset
MiCaGenerators::white_paper(input, now)  // White Paper

// Dispatch by format:
report_generators::generate_report(format, &input, &config, now)
```

All generators return a `RegulatoryReport` in `Draft` status with a content-addressed `id`.

### `src/report_validation.rs` — Validation

```rust
pub fn validate_report(env: &Env, report: &RegulatoryReport, now: u64) -> ValidationResult
```

Runs four layers:
1. **Common** — LEI format (20 alphanumeric), period (start < end), deadline (≥ period_end), content non-empty.
2. **Authority-specific** — required KV keys present for the report's form.
3. **Cross-field** — deadline vs period_end consistency warning.
4. Returns `ValidationResult { passed, error_count, warning_count, errors, warnings, validated_at }`.

### `src/submission_tracker.rs` — Lifecycle

```rust
pub fn check_transition(current: ReportStatus, next: ReportStatus) -> Result<(), ReportingError>
pub fn create_submission(env, report, attempt, config, now) -> Result<RegulatorySubmission, ReportingError>
pub fn ingest_acknowledgment(env, submission, payload, reference_number, accepted, ...) -> Result<SubmissionAcknowledgment, ReportingError>
pub fn apply_acknowledgment(report: &mut RegulatoryReport, ack, now) -> Result<ReportAction, ReportingError>
pub fn mark_validated / mark_submitted / mark_overdue / cancel_report
pub fn is_overdue(report, now) -> bool
pub fn next_attempt(current_attempt, config) -> Result<u32, ReportingError>
```

State machine:
```
Draft → Validated → Submitted → Acknowledged → Accepted  (terminal)
                                      └──→ Rejected → Submitted (retry)
Any non-terminal → Cancelled (terminal)
Non-terminal + past deadline → Overdue (terminal)
```

### `src/reporting_audit_trail.rs` — Audit Trail

```rust
pub fn append_entry(env, trail, report_id, action, actor, resulting_status, context, timestamp)
pub fn verify_trail(env, trail) -> Result<(), u32>  // Err(sequence) = first broken link

// Convenience constructors:
pub fn record_generated / record_validated / record_submitted /
       record_acknowledgment_received / record_accepted / record_rejected /
       record_cancelled / record_overdue
```

Each entry's `entry_hash` = `sha256(report_id || action || sequence || timestamp || context || prev_entry_hash)`.

---

## Report Pipeline

Full end-to-end flow:

```
1. Generate
   ReportInput { entity, lei, period_start, period_end, deadline, extra_fields }
   → generator::generate_report(format, &input, &config, now)
   → RegulatoryReport { status: Draft }
   → audit: record_generated()

2. Validate
   → validate_report(env, &report, now) → ValidationResult
   → if passed: mark_validated(&mut report, now)
   → audit: record_validated(passed, error_count)

3. Submit
   → create_submission(env, &report, attempt=1, &config, now)
   → mark_submitted(&mut report, now)
   → audit: record_submitted(submission_id, attempt)
   → [dispatch to regulator's API endpoint]

4. Receive Acknowledgment
   → ingest_acknowledgment(env, &submission, payload, reference_number, accepted, ...)
   → apply_acknowledgment(&mut report, &ack, now)
   → audit: record_acknowledgment_received() + record_accepted() or record_rejected()

5a. On Acceptance → status: Accepted (terminal) ✓

5b. On Rejection
    → check next_attempt(current_attempt, &config)
    → if within max_retries: fix content, go to step 3 (attempt+1)
    → else: status: Rejected (terminal)

6. Deadline Sweep (periodic)
   → is_overdue(report, now) → mark_overdue / record_overdue
```

---

## Validation Rules

### Common (all authorities)

| Rule | Error code |
|------|-----------|
| LEI must be exactly 20 ASCII alphanumeric chars | `lei: must be exactly 20 characters` |
| `period_start` < `period_end` | `period: period_start must be strictly before period_end` |
| `deadline` ≥ `period_end` | `deadline: must not be earlier than period_end` |
| Content payload must be non-empty | `content: report payload must not be empty` |

### Authority-Specific Required Fields

Each form has its own required field set. See [Supported Authorities and Forms](#supported-authorities-and-forms). Fields are validated as KV keys in the content payload (`key=value\n` format in Rust / JSON object keys in TypeScript).

### Warnings (non-blocking)

- `period_end > deadline` — deadline urgency warning
- FINRA CAT `schema_version < 2` — deprecated schema
- SEC Form ADV `aum_usd=0` — possible missing data

---

## Submission and Retry Policy

Configuration per authority (`AuthorityConfig`):

| Field | Type | Description |
|-------|------|-------------|
| `enabled` | `bool` | Toggle reporting to this authority |
| `endpoint` | `Bytes` | Submission endpoint URI |
| `credential_ref` | `Bytes` | Reference to API key (never stored as plaintext) |
| `max_retries` | `u32` | Maximum automatic retry attempts (default: 3) |
| `retry_delay_seconds` | `u32` | Base delay between retries |
| `exponential_backoff` | `bool` | Double the delay on each attempt |

**Retry delay formula (exponential backoff)**:

```
retry_after = now + min(base_delay × 2^(attempt-1), 86400)
```

First attempt has `retry_after = 0` (immediate). Maximum back-off is capped at 24 hours.

---

## Acknowledgment Tracking

Every submission produces a `RegulatorySubmission` with a unique `id` (SHA-256 of `report_id || attempt`).

When the authority's API sends a callback (webhook), `ingest_acknowledgment` correlates it via the submission. The `reference_number` returned by the authority is stored and surfaced through the audit trail.

Rejection triggers the retry path if `attempt < max_retries`. The operator can also fix the report content and trigger a manual resubmission.

---

## Audit Trail

Every action is recorded in a hash-chained `Vec<ReportingAuditEntry>` keyed by report ID.

**Entry hash formula**:
```
entry_hash = sha256(
  report_id       (32 bytes)
  || action       (u32 LE)
  || sequence     (u32 LE)
  || timestamp    (u64 LE)
  || context      (variable)
  || prev_entry_hash (32 bytes)
)
```

The chain can be independently verified by any party with access to the entry sequence using `verify_trail(env, &trail)`. Any modification to any historical entry invalidates all subsequent hashes.

**Actions recorded**:
- `Generated` — report payload assembled
- `Validated` — schema/business rule check result
- `Submitted` — dispatch to authority
- `AcknowledgmentReceived` — callback from authority
- `Accepted` — authority confirmed compliance
- `Rejected` — authority flagged issues
- `Resubmitted` — second or later attempt
- `Cancelled` — operator cancelled
- `MarkedOverdue` — deadline sweep

---

## REST API Reference

Base path: `/regulatory-reports`

### `GET /regulatory-reports/authorities`

List all supported regulatory authorities and their available forms.

**Response** `200`:
```json
{
  "authorities": [
    { "authority": "FINRA", "jurisdiction": "US", "forms": ["FINRA_OATS", "FINRA_CAT", ...] },
    { "authority": "SEC",   "jurisdiction": "US", "forms": ["SEC_FORM_ADV", ...] },
    ...
  ]
}
```

---

### `POST /regulatory-reports/generate`

Generate a new report in `draft` status.

**Request body**:
```json
{
  "format": "FCA_MIFID_II",
  "entity": "GADDCA8ABCDE1234",
  "lei": "HWUPKR0MPOU8LEYPWAT0",
  "periodStart": "2025-10-01T00:00:00Z",
  "periodEnd": "2025-10-31T23:59:59Z",
  "deadline": "2025-11-15T17:00:00Z",
  "content": {
    "isin": "GB0002634946",
    "quantity": 1000,
    "price": "10.50",
    "venueMic": "XLON",
    "executingEntityId": "EID-001"
  },
  "sourceEventIds": ["abc123..."],
  "actor": "compliance-system"
}
```

**Response** `201`:
```json
{
  "report": {
    "id": "rpt-abc123...",
    "authority": "FCA",
    "format": "FCA_MIFID_II",
    "status": "draft",
    ...
  }
}
```

---

### `GET /regulatory-reports`

List reports with optional filters.

Query params: `authority`, `status`, `entity`, `limit` (default 50), `offset` (default 0).

---

### `GET /regulatory-reports/:id`

Get a single report by ID.

---

### `POST /regulatory-reports/:id/validate`

Run validation. Updates `status` to `validated` on success.

**Request body**: `{ "actor": "compliance-engine" }`

**Response** `200`:
```json
{
  "validation": {
    "passed": true,
    "errorCount": 0,
    "warningCount": 1,
    "errors": [],
    "warnings": [{ "code": "DEADLINE_TIGHT", "message": "..." }],
    "validatedAt": "2025-10-01T12:00:00Z"
  },
  "report": { "status": "validated", ... }
}
```

---

### `POST /regulatory-reports/:id/submit`

Dispatch the report to the regulator. Report must be in `validated` or `rejected` status.

**Request body**: `{ "endpoint": "https://fca-gateway.example/tr", "actor": "scheduler" }`

**Response** `202`:
```json
{
  "submission": {
    "id": "sub-xyz...",
    "attempt": 1,
    "status": "submitted",
    ...
  },
  "report": { "status": "submitted", ... }
}
```

---

### `POST /regulatory-reports/:id/acknowledge`

Ingest an acknowledgment webhook from the regulator.

**Request body**:
```json
{
  "referenceNumber": "FCA-REF-20251101-001",
  "accepted": true,
  "rejectionReason": "",
  "errorCodes": [],
  "actor": "fca-webhook"
}
```

**Response** `200`:
```json
{
  "acknowledgment": {
    "id": "ack-...",
    "referenceNumber": "FCA-REF-20251101-001",
    "accepted": true,
    "ackHash": "sha256-of-payload"
  },
  "report": { "status": "accepted", ... }
}
```

---

### `POST /regulatory-reports/:id/cancel`

Cancel a report. Not allowed on terminal statuses (`accepted`, `cancelled`, `overdue`).

**Request body**: `{ "reason": "Amended report supersedes this one", "actor": "compliance-officer" }`

---

### `GET /regulatory-reports/:id/submissions`

List all submission attempts for a report.

---

### `GET /regulatory-reports/:id/audit-trail`

Get the complete immutable audit trail with chain integrity verification.

**Response** `200`:
```json
{
  "reportId": "rpt-abc123",
  "trail": [
    {
      "sequence": 0,
      "action": "generated",
      "actor": "compliance-system",
      "timestamp": "2025-10-01T08:00:00Z",
      "prevEntryHash": "0000...0000",
      "entryHash": "a1b2c3...",
      "resultingStatus": "draft",
      "context": { "format": "FCA_MIFID_II", "authority": "FCA" }
    },
    ...
  ],
  "total": 5,
  "chainIntegrity": "valid"
}
```

---

### `GET /regulatory-reports/pending`

Reports in `validated` or `rejected` status awaiting submission.

### `GET /regulatory-reports/overdue`

Reports past their deadline that are not yet in a terminal state. Calling this endpoint also marks the overdue reports in the store and appends a `marked_overdue` audit entry.

---

## Configuration

Mount the router in `api/rest/src/server.ts`:

```typescript
import { mountRegulatoryReporting } from "./regulatory_reporting";

// Inside server setup:
mountRegulatoryReporting(app, "/v1/regulatory-reports");
```

### Authority Configuration (Rust)

```rust
AuthorityConfig {
    authority: RegulatoryAuthority::FCA,
    enabled: true,
    endpoint: Bytes::from_slice(env, b"https://fca-gateway.example/tr"),
    credential_ref: Bytes::from_slice(env, b"fca-api-key-ref"),  // key reference, not value
    max_retries: 3,
    retry_delay_seconds: 60,
    exponential_backoff: true,
    retention_ledgers: 52_560,  // ~1 year at 6s/ledger
}
```

---

## Robustness and Fault Tolerance

### Concurrent validator attestations

The pure transition helpers (`apply_acknowledgment`, `mark_*`) mutate a
caller-held copy of a report, so two validators working from the same snapshot
could both apply a transition, and acknowledgments with different
`received_at` clocks got different ack ids. `attest_acknowledgment` is the
race-free, storage-backed entry point:

- **Compare-and-swap versions.** Every report stored via `save_report` has a
  monotonic version. Attestors pass the version they observed; a mismatch
  fails with `StaleReportVersion` and writes nothing.
- **First-writer-wins acknowledgments.** The first ack committed for a
  submission is canonical. Replaying the same outcome and reference is an
  idempotent no-op that returns the canonical ack (`replayed = true`) in any
  arrival order. A different outcome or reference fails with
  `ConflictingAcknowledgment`.
- **Idempotent submission storage.** `store_submission` never re-indexes an
  identical submission. A different submission under an existing id fails with
  `DuplicateSubmission`.
- **Validate-then-commit.** All checks run before the first storage write.

### Network partitions

When the path to the regulator, or between validators, partitions after a
submission is dispatched, the acknowledgment may never arrive:

- **Detection.** `find_stalled_submissions(ack_timeout, now, cursor, limit)`
  scans the submission index in bounded pages. It returns latest-attempt
  submissions that are still `Submitted` and have no acknowledgment after
  `ack_timeout` seconds.
- **Recovery.** `recover_stalled_submission` is CAS-guarded by the report
  version. Before the deadline, it creates and stores the next attempt using
  the normal `max_retries` and back-off rules. Past the deadline, it marks the
  report `Overdue`. Concurrent recoverers get `StaleReportVersion`. Calling it
  on a healthy submission fails with `SubmissionNotStalled`.
- **Healing.** The superseded submission stays open. If a late *acceptance*
  arrives after the partition heals, it still resolves the report. A late
  *rejection* of a superseded attempt is recorded for audit
  (`superseded = true`) but does not change the report, so the live attempt's
  lifecycle is not disturbed.
- **Trail reconciliation.** `reconcile_trails(local, remote)` verifies both
  hash chains (`TrailCorrupted`). If one trail is a prefix of the other, it
  fast-forwards to the longer one in either argument order. If they diverge,
  it reports `TrailForkDetected` with the first diverging sequence number.
  Forks are never merged automatically.

### Large-batch CPU and memory

The Soroban budget meter showed the following changes (host test build,
soroban-sdk 28):

| Operation | CPU before → after | Memory before → after |
|---|---|---|
| Append 200 audit entries | 5.68M → 4.67M (−18%) | 554K → 453K (−18%) |
| Verify 200-entry trail | 4.59M → 3.19M (−30%) | 226K → 124K (−45%) |
| 200 submission ids | 2.03M → 1.68M (−17%) | 97K → 55K (−43%) |
| Store 60 submissions (sequential) | 26.7M → 15.1M (−43%) | 6.74M → 4.25M (−37%) |

- **Hash preimages** for entries, submission ids and ack ids pack their
  fixed-width fields into a stack buffer. Each preimage now takes 3 host calls
  instead of 6. The byte layout is unchanged, and tests compare the output to
  the old encoding to prove that.
- **`verify_trail`** decodes each entry once and carries the running hash
  forward.
- **`verify_trail_segment(segment, start_sequence, prev_hash)`** verifies a
  very long trail in chunks across several invocations, so each call stays
  within its CPU budget.
- **`store_submissions_batch`** stores up to `MAX_SUBMISSION_BATCH` (48)
  submissions. All items are validated before any write. It reads and writes
  the counter once per batch and fits Soroban's per-invocation footprint
  limits. Larger batches fail with `BatchTooLarge`. (Without a bound, about
  67 or more stores in one invocation exceed the 200 written-entries limit.)
- First-attempt submissions skip the `LatestAttempt` write, and
  `list_submissions` no longer probes storage with a placeholder id when an
  index slot is missing.

---

## Error Codes

| Code | Description |
|------|-------------|
| `ReportNotFound` (200) | No report with the given ID exists |
| `SubmissionNotFound` (201) | No submission with the given ID |
| `InvalidStatusTransition` (202) | Requested state change is not permitted |
| `ValidationFailed` (203) | Schema or business rule check failed |
| `MissingRequiredField` (204) | A required form field is absent |
| `InvalidFieldValue` (205) | Field value is out of range or malformed |
| `AuthorityDisabled` (206) | The authority is not enabled in config |
| `MaxRetriesExceeded` (207) | Retry limit reached |
| `DeadlineExceeded` (208) | Submission deadline has passed |
| `InvalidLEI` (209) | LEI must be 20 uppercase alphanumeric chars |
| `InvalidReportingPeriod` (210) | period_start must be before period_end |
| `AcknowledgmentOrphan` (211) | Acknowledgment reference not matched |
| `ConfigNotFound` (212) | Authority configuration not found |
| `PipelinePaused` (213) | Reporting pipeline is paused |
| `UnauthorizedEntity` (214) | Entity not authorized to manage this report |
| `NonceTooLow` (215) | Supplied nonce does not match the stored nonce |
| `NonceExhausted` (216) | Submitter nonce space exhausted |
| `ConflictingAcknowledgment` (217) | A different ack outcome is already recorded for the submission |
| `StaleReportVersion` (218) | Expected report version is stale (another writer committed first) |
| `DuplicateSubmission` (219) | A different submission already exists under this id |
| `SubmissionNotStalled` (220) | Recovery requested for a submission that is acknowledged, superseded, or not timed out |
| `TrailForkDetected` (221) | Two copies of a report's audit trail diverge |
| `TrailCorrupted` (222) | An audit trail failed hash-chain verification |
| `BatchTooLarge` (223) | Batch exceeds `MAX_SUBMISSION_BATCH` |

---

## Testing

### Rust tests

```bash
# Run all regulatory reporting tests
cargo test regulatory_reporting

# Run a single test section
cargo test test_validate_fca_mifid_ii_pass

# Run with output
cargo test regulatory_reporting -- --nocapture
```

Test coverage (990 lines, 60+ tests in `src/regulatory_reporting_tests.rs`):

| Section | Tests |
|---------|-------|
| §1 Authority metadata | 3 |
| §2 Report generation (all 7 authorities) | 18 |
| §3 Validation — pass cases | 16 |
| §4 Validation — failure cases | 6 |
| §5 Submission lifecycle | 5 |
| §6 State machine transitions | 5 |
| §7 Deadline / overdue | 5 |
| §8 Retry back-off | 4 |
| §9 Acknowledgment tracking | 3 |
| §10 Audit trail integrity | 8 |
| §11 ReportStatus helpers | 3 |

### TypeScript integration tests

```bash
cd api/rest
npm test
```

Or use the REST API directly:

```bash
# Generate a FINRA OATS report
curl -X POST http://localhost:3002/v1/regulatory-reports/generate \
  -H "Content-Type: application/json" \
  -d '{
    "format": "FINRA_OATS",
    "entity": "GADDCA8ABCDE1234",
    "lei": "HWUPKR0MPOU8LEYPWAT0",
    "periodStart": "2025-11-01T00:00:00Z",
    "periodEnd": "2025-11-30T23:59:59Z",
    "deadline": "2025-12-10T17:00:00Z",
    "content": {
      "mpid": "ABCD",
      "orderCount": 1234,
      "routeCount": 987
    }
  }'

# Validate it
curl -X POST http://localhost:3002/v1/regulatory-reports/rpt-abc123.../validate \
  -H "Content-Type: application/json" \
  -d '{ "actor": "compliance-system" }'

# Submit it
curl -X POST http://localhost:3002/v1/regulatory-reports/rpt-abc123.../submit \
  -H "Content-Type: application/json" \
  -d '{ "actor": "scheduler" }'

# Ingest acknowledgment
curl -X POST http://localhost:3002/v1/regulatory-reports/rpt-abc123.../acknowledge \
  -H "Content-Type: application/json" \
  -d '{
    "referenceNumber": "FINRA-REF-001",
    "accepted": true,
    "actor": "finra-webhook"
  }'

# Get audit trail
curl http://localhost:3002/v1/regulatory-reports/rpt-abc123.../audit-trail
```

---

## Deployment Notes

1. **Credential management** — `AuthorityConfig.credential_ref` stores a reference key, not the actual API secret. Resolve the actual credential from your secrets manager (e.g., AWS Secrets Manager, HashiCorp Vault) at submission time. Never store API keys on-chain.

2. **Deadline sweep** — Schedule a periodic job (every hour or on a cron) to call `GET /regulatory-reports/overdue`. This marks stale reports and triggers alerts.

3. **Webhook endpoint** — Configure each regulator's API callback URL to point to `POST /regulatory-reports/:id/acknowledge`. The `:id` should be included in the callback URL registered with the regulator.

4. **Persistence** — The TypeScript API uses an in-memory store by default. Replace `reports`, `submissions`, `auditTrails`, and `ackStore` with a persistent database (PostgreSQL recommended) before deploying to production.

5. **Audit trail retention** — `AuthorityConfig.retention_ledgers` controls how long on-chain audit entries persist. Set to `52_560` (~1 year at 6s/ledger) or longer as required by each jurisdiction.

6. **MiCA Reserve Asset** — The `custodianLei` must be a valid 20-character LEI. The validator checks for its presence; the custodian's identity should be verified against the GLEIF database out-of-band.

7. **FINRA CAT** — CAT submissions require `schema_version >= 2`. The generator sets this automatically; the validator emits a warning for v1 content.
