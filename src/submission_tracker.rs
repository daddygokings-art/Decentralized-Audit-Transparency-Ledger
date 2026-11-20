//! Submission Tracker — Lifecycle State Machine
//!
//! Manages the full lifecycle of a report submission from dispatch to final
//! resolution (accepted or rejected).
//!
//! State machine transitions:
//!
//! ```text
//! Draft ──▶ Validated ──▶ Submitted ──▶ Acknowledged ──▶ Accepted  (terminal)
//!                                              │
//!                                              └──▶ Rejected ──▶ Resubmitted (attempt+1)
//!                                                                       │
//!                                                                (back to Submitted)
//! ```
//!
//! Any state can transition to `Cancelled` (operator action) or `Overdue`
//! (deadline check).
//!
//! # Retry policy
//!
//! On rejection or transient failures the tracker creates a new
//! `RegulatorySubmission` with `attempt` incremented.  If `attempt` exceeds
//! `AuthorityConfig::max_retries`, the report is set to `Rejected` (terminal).
//!
//! Retry delay respects `retry_delay_seconds` and optionally doubles it on each
//! attempt when `exponential_backoff` is `true`.

use soroban_sdk::{contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::rbac::require_active_submitter;
use crate::regulatory_reporting::{
    AuthorityConfig, RegulatoryAuthority, RegulatoryReport, RegulatorySubmission,
    ReportAction, ReportStatus, ReportingError, SubmissionAcknowledgment,
};

// ─────────────────────────────────────────────────────────────────────────────
// Deterministic bounds and overflow-safety limits
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of automatic retry attempts permitted (u32 domain guard).
pub const MAX_RETRY_ATTEMPTS: u32 = 1_000;

/// Maximum acknowledgment payload length in bytes (malformed-input guard).
pub const MAX_ACK_PAYLOAD_LEN: usize = 64 * 1024; // 64 KiB

/// Maximum number of machine-readable error codes per acknowledgment.
pub const MAX_ERROR_CODES: usize = 64;

/// Maximum rejection-reason length in bytes.
pub const MAX_REJECTION_REASON_LEN: usize = 4 * 1024; // 4 KiB

/// Maximum source event IDs tracked on a single report.
pub const MAX_SOURCE_EVENT_IDS: usize = 1_024;

/// Cap on exponential back-off factor to prevent u64 overflow in retry_after.
pub const MAX_BACKOFF_FACTOR: u64 = 1u64 << 30; // ~34 years of seconds at base=1

// ─────────────────────────────────────────────────────────────────────────────
// Transition guard
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `Ok(())` if transitioning from `current` to `next` is valid,
/// or `Err(ReportingError::InvalidStatusTransition)` otherwise.
pub fn check_transition(
    current: ReportStatus,
    next: ReportStatus,
) -> Result<(), ReportingError> {
    let allowed = match current {
        ReportStatus::Draft       => matches!(next, ReportStatus::Validated | ReportStatus::Cancelled),
        ReportStatus::Validated   => matches!(next, ReportStatus::Submitted  | ReportStatus::Cancelled),
        ReportStatus::Submitted   => matches!(next, ReportStatus::Acknowledged | ReportStatus::Rejected | ReportStatus::Cancelled | ReportStatus::Overdue),
        ReportStatus::Acknowledged => matches!(next, ReportStatus::Accepted  | ReportStatus::Rejected  | ReportStatus::Cancelled),
        ReportStatus::Rejected    => matches!(next, ReportStatus::Submitted  | ReportStatus::Cancelled),  // resubmission
        // Terminal states
        ReportStatus::Accepted | ReportStatus::Cancelled | ReportStatus::Overdue => false,
    };

    if allowed {
        Ok(())
    } else {
        Err(ReportingError::InvalidStatusTransition)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Submission creation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a deterministic submission ID from report_id + attempt number.
pub fn compute_submission_id(env: &Env, report_id: &BytesN<32>, attempt: u32) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&report_id.to_bytes());
    buf.extend_from_array(&attempt.to_le_bytes());
    env.crypto().sha256(&buf).to_bytes()
}

/// Build a new `RegulatorySubmission` for the given report and attempt number.
pub fn create_submission(
    env: &Env,
    caller: &Address,
    report: &RegulatoryReport,
    attempt: u32,
    config: &AuthorityConfig,
    now: u64,
) -> Result<RegulatorySubmission, ReportingError> {
    // RBAC: only an authenticated, non-suspended submitter (or higher) may create submissions.
    require_active_submitter(env, caller)?;

    // Validate state
    if report.status == ReportStatus::Draft {
        return Err(ReportingError::InvalidStatusTransition);
    }
    if !config.enabled {
        return Err(ReportingError::AuthorityDisabled);
    }
    if now > report.deadline {
        return Err(ReportingError::DeadlineExceeded);
    }
    // Deterministic upper bound on attempts to keep the attempt domain bounded.
    if attempt > MAX_RETRY_ATTEMPTS {
        return Err(ReportingError::MaxRetriesExceeded);
    }
    if attempt > config.max_retries {
        return Err(ReportingError::MaxRetriesExceeded);
    }

    // Compute retry-after for back-off using saturating/checked arithmetic only.
    let retry_after = if attempt > 1 {
        let base = config.retry_delay_seconds as u64;
        if config.exponential_backoff {
            // 2^(attempt-1) * base, capped to avoid u64 overflow.
            let shift = (attempt - 1) as u32;
            let factor: u64 = if shift >= 63 {
                MAX_BACKOFF_FACTOR
            } else {
                1u64.checked_shl(shift).unwrap_or(MAX_BACKOFF_FACTOR)
            };
            let delayed = base.saturating_mul(factor).min(MAX_BACKOFF_FACTOR as u64);
            now.saturating_add(delayed.min(86_400))
        } else {
            now.saturating_add(base as u64)
        }
    } else {
        0 // submit immediately on first attempt
    };

    let id = compute_submission_id(env, &report.id, attempt);

    let submission = RegulatorySubmission {
        id,
        report_id: report.id.clone(),
        attempt,
        submitted_at: now,
        endpoint: config.endpoint.clone(),
        reference_number: None,
        response_code: 0,
        response_payload: Bytes::new(env),
        status: ReportStatus::Submitted,
        retry_eligible: attempt < config.max_retries,
        retry_after,
    };

    // Audit trail event: submission dispatched (tamper-evident, persisted off-chain).
    emit_audit_event(
        env,
        caller,
        "submission_created",
        &submission.id,
        &Bytes::from_slice(env, b"ok"),
        now,
    );

    Ok(submission)
}

/// Emit a tamper-evident audit event for a state-mutating submission-tracker action.
fn emit_audit_event(
    env: &Env,
    actor: &Address,
    operation: &str,
    target_id: &BytesN<32>,
    outcome: &Bytes,
    timestamp: u64,
) {
    let topic = (Symbol::new(env, "submission_tracker"), Symbol::new(env, operation));
    env.events().publish(
        topic,
        (actor.clone(), env.ledger().timestamp(), target_id.clone(), outcome.clone()),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Acknowledgment ingestion
// ─────────────────────────────────────────────────────────────────────────────

/// Compute an acknowledgment ID from submission_id + received_at timestamp.
pub fn compute_ack_id(env: &Env, submission_id: &BytesN<32>, received_at: u64) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&submission_id.to_bytes());
    buf.extend_from_array(&received_at.to_le_bytes());
    env.crypto().sha256(&buf).to_bytes()
}

/// Compute the hash of an acknowledgment payload for tamper-evidence.
pub fn compute_ack_hash(env: &Env, payload: &Bytes) -> BytesN<32> {
    env.crypto().sha256(payload).to_bytes()
}

/// Ingest a raw acknowledgment response from the authority's API.
///
/// `payload`          — raw response body bytes
/// `reference_number` — authority-assigned reference ID from the response
/// `accepted`         — whether the authority accepted the report
/// `rejection_reason` — human-readable rejection description (empty on accept)
/// `error_codes`      — machine-readable error codes (empty on accept)
pub fn ingest_acknowledgment(
    env: &Env,
    caller: &Address,
    submission: &RegulatorySubmission,
    payload: Bytes,
    reference_number: Bytes,
    accepted: bool,
    rejection_reason: Bytes,
    error_codes: Vec<Bytes>,
    received_at: u64,
) -> Result<SubmissionAcknowledgment, ReportingError> {
    // RBAC: only an authenticated, non-suspended submitter (or higher) may ingest acknowledgments.
    require_active_submitter(env, caller)?;

    // The submission must be in Submitted state to receive an ack
    if submission.status != ReportStatus::Submitted
        && submission.status != ReportStatus::Acknowledged
    {
        return Err(ReportingError::InvalidStatusTransition);
    }
    if reference_number.is_empty() {
        return Err(ReportingError::AcknowledgmentOrphan);
    }

    // Malformed-input bounds: reject oversized payloads and error-code lists.
    if payload.len() > MAX_ACK_PAYLOAD_LEN {
        return Err(ReportingError::InvalidFieldValue);
    }
    if rejection_reason.len() > MAX_REJECTION_REASON_LEN {
        return Err(ReportingError::InvalidFieldValue);
    }
    if error_codes.len() > MAX_ERROR_CODES {
        return Err(ReportingError::InvalidFieldValue);
    }

    let ack_hash = compute_ack_hash(env, &payload);
    let id = compute_ack_id(env, &submission.id, received_at);

    let ack = SubmissionAcknowledgment {
        id,
        submission_id: submission.id.clone(),
        report_id: submission.report_id.clone(),
        reference_number,
        accepted,
        rejection_reason,
        error_codes,
        received_at,
        ack_hash,
    };

    // Audit trail event: acknowledgment received.
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(if accepted { b"accepted=1" } else { b"accepted=0" });
    emit_audit_event(
        env,
        caller,
        "acknowledgment_ingested",
        &submission.id,
        &ctx,
        received_at,
    );

    Ok(ack)
}

// ─────────────────────────────────────────────────────────────────────────────
// Status transitions
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a validated acknowledgment to a mutable report, updating its status.
///
/// Returns the `ReportAction` that was performed so callers can record
/// it in the audit trail.
pub fn apply_acknowledgment(
    env: &Env,
    caller: &Address,
    report: &mut RegulatoryReport,
    ack: &SubmissionAcknowledgment,
    now: u64,
) -> Result<ReportAction, ReportingError> {
    require_active_submitter(env, caller)?;

    // First, transition to Acknowledged
    check_transition(report.status, ReportStatus::Acknowledged)?;
    report.status = ReportStatus::Acknowledged;
    report.updated_at = now;

    // Then, resolve to Accepted or Rejected
    let (next_status, action) = if ack.accepted {
        (ReportStatus::Accepted, ReportAction::Accepted)
    } else {
        (ReportStatus::Rejected, ReportAction::Rejected)
    };

    check_transition(report.status, next_status)?;
    report.status = next_status;
    report.updated_at = now;

    // Audit trail event: final resolution.
    let outcome = if ack.accepted {
        Bytes::from_slice(env, b"accepted=1")
    } else {
        Bytes::from_slice(env, b"accepted=0")
    };
    emit_audit_event(env, caller, "acknowledgment_applied", &report.id, &outcome, now);

    Ok(action)
}

/// Mark a report as overdue (deadline has passed without acceptance).
pub fn mark_overdue(
    env: &Env,
    caller: &Address,
    report: &mut RegulatoryReport,
    now: u64,
) -> Result<(), ReportingError> {
    require_active_submitter(env, caller)?;
    check_transition(report.status, ReportStatus::Overdue)?;
    report.status = ReportStatus::Overdue;
    report.updated_at = now;

    emit_audit_event(
        env,
        caller,
        "report_marked_overdue",
        &report.id,
        &Bytes::from_slice(env, b"overdue=1"),
        now,
    );
    Ok(())
}

/// Cancel a report manually.
pub fn cancel_report(
    env: &Env,
    caller: &Address,
    report: &mut RegulatoryReport,
    now: u64,
) -> Result<(), ReportingError> {
    require_active_submitter(env, caller)?;
    check_transition(report.status, ReportStatus::Cancelled)?;
    report.status = ReportStatus::Cancelled;
    report.updated_at = now;

    emit_audit_event(
        env,
        caller,
        "report_cancelled",
        &report.id,
        &Bytes::from_slice(env, b"cancelled=1"),
        now,
    );
    Ok(())
}

/// Transition a validated report to Submitted status.
pub fn mark_submitted(
    env: &Env,
    caller: &Address,
    report: &mut RegulatoryReport,
    now: u64,
) -> Result<(), ReportingError> {
    require_active_submitter(env, caller)?;
    check_transition(report.status, ReportStatus::Submitted)?;
    report.status = ReportStatus::Submitted;
    report.updated_at = now;

    emit_audit_event(
        env,
        caller,
        "report_marked_submitted",
        &report.id,
        &Bytes::from_slice(env, b"submitted=1"),
        now,
    );
    Ok(())
}

/// Transition a draft report to Validated status.
pub fn mark_validated(
    env: &Env,
    caller: &Address,
    report: &mut RegulatoryReport,
    now: u64,
) -> Result<(), ReportingError> {
    require_active_submitter(env, caller)?;
    check_transition(report.status, ReportStatus::Validated)?;
    report.status = ReportStatus::Validated;
    report.updated_at = now;

    emit_audit_event(
        env,
        caller,
        "report_marked_validated",
        &report.id,
        &Bytes::from_slice(env, b"validated=1"),
        now,
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Deadline check
// ─────────────────────────────────────────────────────────────────────────────

/// Check whether the report's deadline has passed.
///
/// Returns `true` when the current time exceeds the deadline AND the report
/// has not yet been accepted or cancelled.
pub fn is_overdue(report: &RegulatoryReport, now: u64) -> bool {
    now > report.deadline && !report.status.is_terminal()
}

// ─────────────────────────────────────────────────────────────────────────────
// Retry scheduling
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the next attempt number for a resubmission, or
/// `Err(MaxRetriesExceeded)` if the limit has been reached.
pub fn next_attempt(
    current_attempt: u32,
    config: &AuthorityConfig,
) -> Result<u32, ReportingError> {
    let next = current_attempt.checked_add(1).ok_or(ReportingError::MaxRetriesExceeded)?;
    if next > MAX_RETRY_ATTEMPTS || next > config.max_retries {
        Err(ReportingError::MaxRetriesExceeded)
    } else {
        Ok(next)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage keys, pagination, filtering and TTL management (#897 #896)
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of index entries scanned per paginated read, bounding
/// on-chain CPU/storage reads so large datasets cannot exhaust the budget.
const SUBMISSION_PAGE_SCAN_CAP: u32 = 512;

/// Persistent storage keys for the submission tracker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionTrackerKey {
    /// RegulatorySubmission keyed by its content-addressed id.
    Submission(BytesN<32>),
    /// SubmissionAcknowledgment keyed by its id.
    Receipt(BytesN<32>),
    /// Last consumed submission nonce per submitter address (idempotency).
    Nonce(Address),
    /// Append-only ordered index: sequence u32 → submission id BytesN<32>.
    SubmissionIndex(u32),
    /// Total number of submissions recorded.
    SubmissionCount,
}

/// Multi-predicate filter for read queries over submissions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionFilter {
    /// Optional authority filter.
    pub authority: Option<RegulatoryAuthority>,
    /// Optional submission status filter.
    pub status: Option<ReportStatus>,
    /// Optional report id filter.
    pub report_id: Option<BytesN<32>>,
    /// Only include submissions submitted at or after this timestamp.
    pub min_submitted_at: Option<u64>,
    /// Only include submissions submitted before this timestamp.
    pub max_submitted_at: Option<u64>,
}

impl Default for SubmissionFilter {
    fn default() -> Self {
        SubmissionFilter {
            authority: None,
            status: None,
            report_id: None,
            min_submitted_at: None,
            max_submitted_at: None,
        }
    }
}

/// Cursor-based pagination result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionPage {
    /// Matching submissions for this page (at most `limit`).
    pub items: Vec<RegulatorySubmission>,
    /// Next starting index to continue pagination, or `None` when exhausted.
    pub next_cursor: Option<u32>,
}

/// Store a submission and append it to the ordered index.
pub fn store_submission(env: &Env, submission: &RegulatorySubmission) {
    let sub_key = SubmissionTrackerKey::Submission(submission.id.clone());
    env.storage().persistent().set(&sub_key, submission);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&SubmissionTrackerKey::SubmissionCount)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&SubmissionTrackerKey::SubmissionIndex(count), &submission.id.clone());
    env.storage()
        .persistent()
        .set(&SubmissionTrackerKey::SubmissionCount, &(count + 1));
}

/// Retrieve a submission by id.
pub fn get_submission(env: &Env, id: &BytesN<32>) -> Option<RegulatorySubmission> {
    env.storage()
        .persistent()
        .get(&SubmissionTrackerKey::Submission(id.clone()))
}

/// Store an acknowledgment receipt.
pub fn store_receipt(env: &Env, receipt: &SubmissionAcknowledgment) {
    let key = SubmissionTrackerKey::Receipt(receipt.id.clone());
    env.storage().persistent().set(&key, receipt);
}

/// Retrieve an acknowledgment receipt by id.
pub fn get_receipt(env: &Env, id: &BytesN<32>) -> Option<SubmissionAcknowledgment> {
    env.storage()
        .persistent()
        .get(&SubmissionTrackerKey::Receipt(id.clone()))
}

/// Get the current (last consumed) nonce for a submitter.
pub fn get_nonce(env: &Env, submitter: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&SubmissionTrackerKey::Nonce(submitter.clone()))
        .unwrap_or(0)
}

/// Atomically increment and return the next nonce for a submitter.
/// Rejects replay when the supplied `expected` nonce does not match the
/// stored one, providing idempotent submission handling.
pub fn increment_nonce(
    env: &Env,
    submitter: &Address,
    expected: u64,
) -> Result<u64, ReportingError> {
    let current = get_nonce(env, submitter);
    if current != expected {
        return Err(ReportingError::NonceTooLow);
    }
    let next = current.checked_add(1).ok_or(ReportingError::NonceExhausted)?;
    env.storage()
        .persistent()
        .set(&SubmissionTrackerKey::Nonce(submitter.clone()), &next);
    Ok(next)
}

/// Return `true` when `submission` matches every predicate set in `filter`.
fn matches_filter(_env: &Env, submission: &RegulatorySubmission, filter: &SubmissionFilter) -> bool {
    if let Some(status) = filter.status {
        if submission.status != status {
            return false;
        }
    }
    if let Some(report_id) = filter.report_id {
        if submission.report_id != *report_id {
            return false;
        }
    }
    if let Some(min_submitted_at) = filter.min_submitted_at {
        if submission.submitted_at < min_submitted_at {
            return false;
        }
    }
    if let Some(max_submitted_at) = filter.max_submitted_at {
        if submission.submitted_at >= max_submitted_at {
            return false;
        }
    }
    true
}

/// Cursor-based, filtered paginated read over submissions.
///
/// At most `limit` index entries are scanned per call, bounding storage reads
/// and CPU cost. `cursor` is the starting index (0-based); `next_cursor` in
/// the result is the index to resume from, or `None` when no more entries
/// remain.
pub fn list_submissions(
    env: &Env,
    filter: &SubmissionFilter,
    cursor: u32,
    limit: u32,
) -> SubmissionPage {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&SubmissionTrackerKey::SubmissionCount)
        .unwrap_or(0);

    let start = cursor.min(count);
    let scan_cap = limit.min(SUBMISSION_PAGE_SCAN_CAP);
    let mut end = start.saturating_add(scan_cap);
    if end > count {
        end = count;
    }

    let mut items = Vec::new(env);
    let mut i = start;
    while i < end && (items.len() as u32) < limit {
        let id: BytesN<32> = env
            .storage()
            .persistent()
            .get(&SubmissionTrackerKey::SubmissionIndex(i))
            .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]));
        let key = SubmissionTrackerKey::Submission(id.clone());
        if let Some(submission) = env.storage().persistent().get::<_, RegulatorySubmission>(&key) {
            if matches_filter(env, &submission, filter) {
                items.push_back(submission);
            }
        }
        i += 1;
    }

    let next_cursor = if end < count { Some(end) } else { None };

    SubmissionPage {
        items,
        next_cursor,
    }
}

/// Bump the TTL of a submission so it is not reclaimed by rent/expiry.
/// Returns the ledger sequence the key was extended to.
pub fn bump_submission_ttl(env: &Env, id: &BytesN<32>, threshold: u32, expires_to: u32) -> u32 {
    let key = SubmissionTrackerKey::Submission(id.clone());
    env.storage().persistent().extend_ttl(&key, threshold, expires_to)
}

/// Bump the TTL of an acknowledgment receipt.
pub fn bump_receipt_ttl(env: &Env, id: &BytesN<32>, threshold: u32, expires_to: u32) -> u32 {
    let key = SubmissionTrackerKey::Receipt(id.clone());
    env.storage().persistent().extend_ttl(&key, threshold, expires_to)
}

/// Bump the TTL of a submitter nonce record.
pub fn bump_nonce_ttl(env: &Env, submitter: &Address, threshold: u32, expires_to: u32) -> u32 {
    let key = SubmissionTrackerKey::Nonce(submitter.clone());
    env.storage().persistent().extend_ttl(&key, threshold, expires_to)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

    use crate::regulatory_reporting::{
        AuthorityConfig, RegulatoryAuthority, RegulatoryReport, ReportFormat, ReportStatus,
        ValidationResult,
    };

    fn dummy_config(env: &Env) -> AuthorityConfig {
        AuthorityConfig {
            authority: RegulatoryAuthority::FINRA,
            enabled: true,
            endpoint: Bytes::from_slice(env, b"https://finra.example/api"),
            credential_ref: Bytes::from_slice(env, b"cred-ref-001"),
            max_retries: 3,
            retry_delay_seconds: 60,
            exponential_backoff: true,
            retention_ledgers: 52_560,
        }
    }

    fn validated_report(env: &Env) -> RegulatoryReport {
        RegulatoryReport {
            id: BytesN::from_array(env, &[1u8; 32]),
            authority: RegulatoryAuthority::FINRA,
            format: ReportFormat::FinraOATS,
            entity: Address::generate(env),
            lei: Bytes::from_slice(env, b"HWUPKR0MPOU8LEYPWAT0"),
            period_start: 1_700_000_000,
            period_end:   1_700_086_400,
            deadline:     1_700_172_800,
            content: Bytes::from_slice(env, b"authority=FINRA\nmpid=TEST\n"),
            schema_version: 1,
            status: ReportStatus::Validated,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            last_validation: Some(ValidationResult {
                passed: true,
                error_count: 0,
                warning_count: 0,
                errors: Vec::new(env),
                warnings: Vec::new(env),
                validated_at: 1_700_000_000,
            }),
            prev_report_hash: BytesN::from_array(env, &[0u8; 32]),
            report_hash: BytesN::from_array(env, &[2u8; 32]),
            source_event_ids: Vec::new(env),
        }
    }

    // ── State machine transition guards ───────────────────────────────────

    #[test]
    fn test_draft_to_validated_allowed() {
        assert!(check_transition(ReportStatus::Draft, ReportStatus::Validated).is_ok());
    }

    #[test]
    fn test_draft_to_submitted_disallowed() {
        assert!(check_transition(ReportStatus::Draft, ReportStatus::Submitted).is_err());
    }

    #[test]
    fn test_validated_to_submitted_allowed() {
        assert!(check_transition(ReportStatus::Validated, ReportStatus::Submitted).is_ok());
    }

    #[test]
    fn test_submitted_to_acknowledged_allowed() {
        assert!(check_transition(ReportStatus::Submitted, ReportStatus::Acknowledged).is_ok());
    }

    #[test]
    fn test_accepted_to_anything_disallowed() {
        for next in [
            ReportStatus::Draft,
            ReportStatus::Validated,
            ReportStatus::Submitted,
            ReportStatus::Cancelled,
        ] {
            assert!(
                check_transition(ReportStatus::Accepted, next).is_err(),
                "Accepted should be terminal; transition to {:?} must be disallowed",
                next
            );
        }
    }

    #[test]
    fn test_rejected_to_submitted_allowed_for_retry() {
        assert!(check_transition(ReportStatus::Rejected, ReportStatus::Submitted).is_ok());
    }

    // ── Submission creation ───────────────────────────────────────────────

    #[test]
    fn test_create_first_submission_ok() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let sub = create_submission(&env, &caller, &report, 1, &config, 1_700_000_100).unwrap();
        assert_eq!(sub.attempt, 1);
        assert_eq!(sub.status, ReportStatus::Submitted);
        assert_eq!(sub.retry_after, 0); // first attempt has no delay
    }

    #[test]
    fn test_create_submission_after_deadline_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let result = create_submission(&env, &caller, &report, 1, &config, 2_000_000_000); // past deadline
        assert!(matches!(result, Err(ReportingError::DeadlineExceeded)));
    }

    #[test]
    fn test_create_submission_max_retries_exceeded_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let result = create_submission(&env, &caller, &report, 4, &config, 1_700_000_100); // attempt 4 > max 3
        assert!(matches!(result, Err(ReportingError::MaxRetriesExceeded)));
    }

    #[test]
    fn test_create_submission_disabled_authority_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let report = validated_report(&env);
        let mut config = dummy_config(&env);
        config.enabled = false;
        let result = create_submission(&env, &caller, &report, 1, &config, 1_700_000_100);
        assert!(matches!(result, Err(ReportingError::AuthorityDisabled)));
    }

    #[test]
    fn test_exponential_backoff_second_attempt() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let report = validated_report(&env);
        let config = dummy_config(&env); // backoff=true, delay=60s
        let now = 1_700_000_100u64;
        let sub = create_submission(&env, &caller, &report, 2, &config, now).unwrap();
        // 2^(2-1) * 60 = 120 seconds
        assert_eq!(sub.retry_after, now + 120);
    }

    // ── Acknowledgment ingestion ──────────────────────────────────────────

    fn dummy_submission(env: &Env) -> RegulatorySubmission {
        RegulatorySubmission {
            id: BytesN::from_array(env, &[10u8; 32]),
            report_id: BytesN::from_array(env, &[1u8; 32]),
            attempt: 1,
            submitted_at: 1_700_000_100,
            endpoint: Bytes::from_slice(env, b"https://finra.example/api"),
            reference_number: None,
            response_code: 200,
            response_payload: Bytes::new(env),
            status: ReportStatus::Submitted,
            retry_eligible: true,
            retry_after: 0,
        }
    }

    #[test]
    fn test_ingest_acceptance_ok() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let sub = dummy_submission(&env);
        let ack = ingest_acknowledgment(
            &env,
            &caller,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-001"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        )
        .unwrap();
        assert!(ack.accepted);
        assert_eq!(ack.submission_id, sub.id);
    }

    #[test]
    fn test_ingest_rejection_ok() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let sub = dummy_submission(&env);
        let mut error_codes = Vec::new(&env);
        error_codes.push_back(Bytes::from_slice(&env, b"ERR001"));
        let ack = ingest_acknowledgment(
            &env,
            &caller,
            &sub,
            Bytes::from_slice(&env, b"{\"status\":\"rejected\"}"),
            Bytes::from_slice(&env, b"REF-002"),
            false,
            Bytes::from_slice(&env, b"Missing required field"),
            error_codes,
            1_700_001_000,
        )
        .unwrap();
        assert!(!ack.accepted);
        assert_eq!(ack.error_codes.len(), 1);
    }

    #[test]
    fn test_ingest_missing_reference_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let sub = dummy_submission(&env);
        let result = ingest_acknowledgment(
            &env,
            &caller,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::new(&env), // empty reference
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        );
        assert!(matches!(result, Err(ReportingError::AcknowledgmentOrphan)));
    }

    // ── Apply acknowledgment / status transitions ─────────────────────────

    #[test]
    fn test_apply_acceptance_sets_accepted() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;

        let ack = SubmissionAcknowledgment {
            id: BytesN::from_array(&env, &[20u8; 32]),
            submission_id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: report.id.clone(),
            reference_number: Bytes::from_slice(&env, b"REF-001"),
            accepted: true,
            rejection_reason: Bytes::new(&env),
            error_codes: Vec::new(&env),
            received_at: 1_700_001_000,
            ack_hash: BytesN::from_array(&env, &[3u8; 32]),
        };

        let action = apply_acknowledgment(&env, &caller, &mut report, &ack, 1_700_001_000).unwrap();
        assert_eq!(report.status, ReportStatus::Accepted);
        assert_eq!(action, ReportAction::Accepted);
    }

    #[test]
    fn test_apply_rejection_sets_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;

        let ack = SubmissionAcknowledgment {
            id: BytesN::from_array(&env, &[20u8; 32]),
            submission_id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: report.id.clone(),
            reference_number: Bytes::from_slice(&env, b"REF-002"),
            accepted: false,
            rejection_reason: Bytes::from_slice(&env, b"Invalid LEI"),
            error_codes: Vec::new(&env),
            received_at: 1_700_001_000,
            ack_hash: BytesN::from_array(&env, &[3u8; 32]),
        };

        let action = apply_acknowledgment(&env, &caller, &mut report, &ack, 1_700_001_000).unwrap();
        assert_eq!(report.status, ReportStatus::Rejected);
        assert_eq!(action, ReportAction::Rejected);
    }

    // ── Overdue / cancel ──────────────────────────────────────────────────

    #[test]
    fn test_is_overdue_returns_true_past_deadline() {
        let env = Env::default();
        let report = validated_report(&env); // deadline = 1_700_172_800
        assert!(is_overdue(&report, 1_700_172_801));
    }

    #[test]
    fn test_is_overdue_returns_false_before_deadline() {
        let env = Env::default();
        let report = validated_report(&env);
        assert!(!is_overdue(&report, 1_700_000_100));
    }

    #[test]
    fn test_is_overdue_returns_false_when_accepted() {
        let env = Env::default();
        let mut report = validated_report(&env);
        report.status = ReportStatus::Accepted;
        assert!(!is_overdue(&report, 2_000_000_000));
    }

    #[test]
    fn test_mark_overdue_transitions() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;
        mark_overdue(&env, &caller, &mut report, 1_700_200_000).unwrap();
        assert_eq!(report.status, ReportStatus::Overdue);
    }

    #[test]
    fn test_cancel_from_validated() {
        let env = Env::default();
        env.mock_all_auths();
        let caller = Address::generate(&env);
        let mut report = validated_report(&env);
        cancel_report(&env, &caller, &mut report, 1_700_000_200).unwrap();
        assert_eq!(report.status, ReportStatus::Cancelled);
    }

    // ── Next attempt ──────────────────────────────────────────────────────

    #[test]
    fn test_next_attempt_within_limit() {
        let env = Env::default();
        let config = dummy_config(&env); // max_retries=3
        assert_eq!(next_attempt(1, &config).unwrap(), 2);
        assert_eq!(next_attempt(2, &config).unwrap(), 3);
    }

    #[test]
    fn test_next_attempt_exceeds_limit() {
        let env = Env::default();
        let config = dummy_config(&env);
        let result = next_attempt(3, &config); // 3+1=4 > max 3
        assert!(matches!(result, Err(ReportingError::MaxRetriesExceeded)));
    }

    // ── Submission ID determinism ─────────────────────────────────────────

    #[test]
    fn test_submission_ids_differ_by_attempt() {
        let env = Env::default();
        let report_id = BytesN::from_array(&env, &[5u8; 32]);
        let id1 = compute_submission_id(&env, &report_id, 1);
        let id2 = compute_submission_id(&env, &report_id, 2);
        assert_ne!(id1, id2);
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Property-based tests (proptest) — invariant & determinism guarantees
    // ════════════════════════════════════════════════════════════════════════════════

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(200))]

        /// Submission IDs are deterministic: same (report_id, attempt) always
        /// yields the same ID, and different attempts always differ.
        #[test]
        fn prop_submission_id_determinism(
            report_id_bytes in prop::array::uniform32(0u8..255),
            attempt1 in 1u32..10,
            attempt2 in 1u32..10,
        ) {
            let env = Env::default();
            let report_id = BytesN::from_array(&env, &report_id_bytes);
            let id1 = compute_submission_id(&env, &report_id, attempt1);
            let id1_again = compute_submission_id(&env, &report_id, attempt1);
            prop_assert_eq!(id1, id1_again);
            if attempt1 != attempt2 {
                let id2 = compute_submission_id(&env, &report_id, attempt2);
                prop_assert_ne!(id1, id2);
            }
        }

        /// Ack IDs are deterministic: same (submission_id, received_at) always
        /// yields the same ID.
        #[test]
        fn prop_ack_id_determinism(
            submission_id_bytes in prop::array::uniform32(0u8..255),
            received_at in 0u64..1_000_000,
        ) {
            let env = Env::default();
            let submission_id = BytesN::from_array(&env, &submission_id_bytes);
            let id1 = compute_ack_id(&env, &submission_id, received_at);
            let id1_again = compute_ack_id(&env, &submission_id, received_at);
            prop_assert_eq!(id1, id1_again);
        }

        /// Ack hash is deterministic: same payload always yields the same hash.
        #[test]
        fn prop_ack_hash_determinism(payload_bytes in prop::collection::vec(any::<u8>(), 0..64)) {
            let env = Env::default();
            let payload = Bytes::from_slice(&env, &payload_bytes);
            let hash1 = compute_ack_hash(&env, &payload);
            let hash1_again = compute_ack_hash(&env, &payload);
            prop_assert_eq!(hash1, hash1_again);
        }

        /// next_attempt is monotonic and bounded by max_retries + 1.
        #[test]
        fn prop_next_attempt_bounds(
            current_attempt in 0u32..10,
            max_retries in 1u32..10,
        ) {
            let env = Env::default();
            let config = AuthorityConfig {
                authority: RegulatoryAuthority::FINRA,
                enabled: true,
                endpoint: Bytes::from_slice(&env, b"https://example/api"),
                credential_ref: Bytes::from_slice(&env, b"cred"),
                max_retries,
                retry_delay_seconds: 60,
                exponential_backoff: true,
                retention_ledgers: 52_560,
            };
            match next_attempt(current_attempt, &config) {
                Ok(next) => {
                    prop_assert_eq!(next, current_attempt + 1);
                    prop_assert!(next <= max_retries + 1);
                }
                Err(ReportingError::MaxRetriesExceeded) => {
                    prop_assert_eq!(current_attempt + 1, max_retries + 1);
                }
                _ => prop_assert!(false, "unexpected error variant"),
            }
        }

        /// is_overdue is monotonic in time: once overdue, always overdue
        /// (unless terminal).
        #[test]
        fn prop_is_overdue_monotonic(
            deadline in 1_000_000u64..2_000_000,
            now_before in 1u64..1_000_000,
            now_after in 2_000_000u64..3_000_000,
        ) {
            let env = Env::default();
            let report = validated_report(&env);
            // Override deadline to the generated value
            let mut report = report;
            report.deadline = deadline;
            prop_assert!(!is_overdue(&report, now_before));
            prop_assert!(is_overdue(&report, now_after));
        }

        /// Exponential backoff retry_after is always >= now and non-decreasing
        /// with attempt number.
        #[test]
        fn prop_backoff_monotonic(
            now in 1_000_000u64..2_000_000,
            attempt1 in 1u32..5,
            attempt2 in 1u32..5,
        ) {
            let env = Env::default();
            let report = validated_report(&env);
            let config = dummy_config(&env);
            let sub1 = create_submission(&env, &report, attempt1, &config, now);
            let sub2 = create_submission(&env, &report, attempt2, &config, now);
            if let (Ok(s1), Ok(s2)) = (sub1, sub2) {
                if attempt2 > attempt1 && attempt1 > 1 {
                    prop_assert!(s2.retry_after >= s1.retry_after);
                }
                // retry_after is never before now
                if s1.attempt > 1 {
                    prop_assert!(s1.retry_after >= now);
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Concurrency & reordering tests — interleaved submissions and acks
    // ════════════════════════════════════════════════════════════════════════════════

    /// Interleaved submissions for distinct reports never collide on IDs and
    /// each report's state machine evolves independently.
    #[test]
    fn test_interleaved_submissions_distinct_reports() {
        let env = Env::default();
        let config = dummy_config(&env);

        let mut reports = Vec::new(&env);
        let mut submissions = Vec::new(&env);
        for i in 0..10u32 {
            let mut report = validated_report(&env);
            report.id = BytesN::from_array(&env, &[i as u8; 32]);
            report.status = ReportStatus::Validated;
            reports.push_back(report);

            let sub = create_submission(
                &env,
                &reports.get(i as usize).unwrap(),
                1,
                &config,
                1_700_000_100 + i,
            )
            .unwrap();
            submissions.push_back(sub);
        }

        // All submission IDs must be distinct (different report_ids).
        let mut seen_ids = Vec::new(&env);
        for i in 0..submissions.len() {
            let id = submissions.get(i).unwrap().id.clone();
            assert!(
                !seen_ids.iter().any(|existing| *existing == id),
                "duplicate submission ID at index {i}"
            );
            seen_ids.push_back(id);
        }

        // Each submission retains its own report_id and attempt.
        for i in 0..submissions.len() {
            let sub = submissions.get(i).unwrap();
            let report = reports.get(i).unwrap();
            assert_eq!(sub.report_id, report.id);
            assert_eq!(sub.attempt, 1);
            assert_eq!(sub.status, ReportStatus::Submitted);
        }
    }

    /// Reordered acknowledgments for the same submission must be idempotent:
    /// ingesting the same ack twice yields identical ack objects, and applying
    /// the ack after the report is already terminal is a no-op / error.
    #[test]
    fn test_reordered_acknowledgments_idempotent() {
        let env = Env::default();
        let sub = dummy_submission(&env);

        let ack1 = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-001"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        )
        .unwrap();

        // Re-ingesting the identical ack produces an identical object (idempotent).
        let ack2 = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-001"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        )
        .unwrap();
        assert_eq!(ack1.id, ack2.id);
        assert_eq!(ack1.ack_hash, ack2.ack_hash);
        assert_eq!(ack1.received_at, ack2.received_at);
    }

    /// An acknowledgment applied twice to the same report: second application
    /// must fail because the report is already terminal (Accepted/Rejected).
    #[test]
    fn test_duplicate_ack_application_rejected() {
        let env = Env::default();
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;

        let ack = SubmissionAcknowledgment {
            id: BytesN::from_array(&env, &[20u8; 32]),
            submission_id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: report.id.clone(),
            reference_number: Bytes::from_slice(&env, b"REF-001"),
            accepted: true,
            rejection_reason: Bytes::new(&env),
            error_codes: Vec::new(&env),
            received_at: 1_700_001_000,
            ack_hash: BytesN::from_array(&env, &[3u8; 32]),
        };

        let action = apply_acknowledgment(&mut report, &ack, 1_700_001_000).unwrap();
        assert_eq!(report.status, ReportStatus::Accepted);
        assert_eq!(action, ReportAction::Accepted);

        // Second application must fail (Accepted is terminal).
        let result = apply_acknowledgment(&mut report, &ack, 1_700_002_000);
        assert!(result.is_err());
        assert_eq!(report.status, ReportStatus::Accepted);
    }

    /// Retry chain: rejection → resubmission → acceptance. Each resubmission
    /// increments attempt and produces a distinct submission ID.
    #[test]
    fn test_retry_chain_increments_attempt_and_id() {
        let env = Env::default();
        let mut report = validated_report(&env);
        let config = dummy_config(&env); // max_retries = 3

        // Attempt 1: submission
        let sub1 = create_submission(&env, &report, 1, &config, 1_700_000_100).unwrap();
        assert_eq!(sub1.attempt, 1);

        // Apply rejection → report becomes Rejected
        report.status = ReportStatus::Submitted;
        let ack_reject = SubmissionAcknowledgment {
            id: BytesN::from_array(&env, &[21u8; 32]),
            submission_id: sub1.id.clone(),
            report_id: report.id.clone(),
            reference_number: Bytes::from_slice(&env, b"REF-REJ"),
            accepted: false,
            rejection_reason: Bytes::from_slice(&env, b"rejected"),
            error_codes: Vec::new(&env),
            received_at: 1_700_002_000,
            ack_hash: BytesN::from_array(&env, &[4u8; 32]),
        };
        apply_acknowledgment(&mut report, &ack_reject, 1_700_002_000).unwrap();
        assert_eq!(report.status, ReportStatus::Rejected);

        // Resubmit: attempt 2
        let sub2 = create_submission(&env, &report, 2, &config, 1_700_003_000).unwrap();
        assert_eq!(sub2.attempt, 2);
        assert_ne!(sub1.id, sub2.id);

        // Resubmit: attempt 3 (max_retries)
        let sub3 = create_submission(&env, &report, 3, &config, 1_700_004_000).unwrap();
        assert_eq!(sub3.attempt, 3);
        assert_ne!(sub2.id, sub3.id);

        // Attempt 4 exceeds max_retries → error
        let result = create_submission(&env, &report, 4, &config, 1_700_005_000);
        assert!(matches!(result, Err(ReportingError::MaxRetriesExceeded)));
    }

    /// Retry exhaustion: after max retries, next_attempt returns error and
    /// no further submission can be created.
    #[test]
    fn test_retry_exhaustion_terminal() {
        let env = Env::default();
        let config = dummy_config(&env); // max_retries = 3

        assert_eq!(next_attempt(1, &config).unwrap(), 2);
        assert_eq!(next_attempt(2, &config).unwrap(), 3);
        assert_eq!(next_attempt(3, &config).unwrap(), 4);
        assert!(matches!(
            next_attempt(4, &config),
            Err(ReportingError::MaxRetriesExceeded)
        ));
    }

    // ════════════════════════════════════════════════════════════════════════════════
    // Fault-injection tests — partial failures, deadline races, malformed input
    // ════════════════════════════════════════════════════════════════════════════════

    /// Submission created just before the deadline succeeds; one second after
    /// fails with DeadlineExceeded.
    #[test]
    fn test_deadline_race_boundary() {
        let env = Env::default();
        let report = validated_report(&env); // deadline = 1_700_172_800
        let config = dummy_config(&env);

        // One second before deadline: OK.
        let ok = create_submission(&env, &report, 1, &config, 1_700_172_799);
        assert!(ok.is_ok());

        // Exactly at deadline: OK (now > deadline is the failure condition).
        let ok_at = create_submission(&env, &report, 1, &config, 1_700_172_800);
        assert!(ok_at.is_ok());

        // One second after deadline: DeadlineExceeded.
        let late = create_submission(&env, &report, 1, &config, 1_700_172_801);
        assert!(matches!(late, Err(ReportingError::DeadlineExceeded)));
    }

    /// Disabled authority blocks all submissions regardless of deadline/state.
    #[test]
    fn test_disabled_authority_blocks_all_attempts() {
        let env = Env::default();
        let report = validated_report(&env);
        let mut config = dummy_config(&env);
        config.enabled = false;

        for attempt in 1..=4u32 {
            let result = create_submission(&env, &report, attempt, &config, 1_700_000_100);
            assert!(
                matches!(result, Err(ReportingError::AuthorityDisabled)),
                "attempt {attempt} should fail with AuthorityDisabled"
            );
        }
    }

    /// Malformed/missing reference number on ack ingestion → AcknowledgmentOrphan.
    #[test]
    fn test_malformed_ack_reference_rejected() {
        let env = Env::default();
        let sub = dummy_submission(&env);

        // Empty reference.
        let result = ingest_acknowledgment(
            &env,
            &sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::new(&env),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        );
        assert!(matches!(result, Err(ReportingError::AcknowledgmentOrphan)));

        // Non-Submitted / non-Acknowledged submission status → InvalidStatusTransition.
        let mut bad_sub = dummy_submission(&env);
        bad_sub.status = ReportStatus::Draft;
        let result = ingest_acknowledgment(
            &env,
            &bad_sub,
            Bytes::from_slice(&env, b"{}"),
            Bytes::from_slice(&env, b"REF-001"),
            true,
            Bytes::new(&env),
            Vec::new(&env),
            1_700_001_000,
        );
        assert!(
            matches!(result, Err(ReportingError::InvalidStatusTransition)),
            "Draft status should be rejected"
        );
    }

    /// Terminal states cannot transition to any other state (invariant).
    #[test]
    fn test_terminal_states_are_irreversible() {
        let env = Env::default();
        let terminal_states = [
            ReportStatus::Accepted,
            ReportStatus::Cancelled,
            ReportStatus::Overdue,
        ];
        for current in terminal_states {
            for next in [
                ReportStatus::Draft,
                ReportStatus::Validated,
                ReportStatus::Submitted,
                ReportStatus::Acknowledged,
                ReportStatus::Rejected,
            ] {
                assert!(
                    check_transition(current, next).is_err(),
                    "{current:?} → {next:?} must be disallowed"
                );
            }
        }
    }

    /// Overdue marking is idempotent-ish: once Overdue (terminal), cannot be
    /// marked overdue again or transition elsewhere.
    #[test]
    fn test_overdue_terminal_invariant() {
        let env = Env::default();
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;
        mark_overdue(&mut report, 1_700_200_000).unwrap();
        assert_eq!(report.status, ReportStatus::Overdue);

        // Cannot cancel an overdue (terminal) report.
        assert!(cancel_report(&mut report, 1_700_300_000).is_err());
        // Cannot mark overdue again.
        assert!(mark_overdue(&mut report, 1_700_400_000).is_err());
        // is_overdue returns false for terminal state.
        assert!(!is_overdue(&report, 2_000_000_000));
    }

    /// apply_acknowledgment preserves report_id and submission_id linkage.
    #[test]
    fn test_apply_ack_preserves_linkage() {
        let env = Env::default();
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;

        let ack = SubmissionAcknowledgment {
            id: BytesN::from_array(&env, &[20u8; 32]),
            submission_id: BytesN::from_array(&env, &[10u8; 32]),
            report_id: report.id.clone(),
            reference_number: Bytes::from_slice(&env, b"REF-001"),
            accepted: false,
            rejection_reason: Bytes::from_slice(&env, b"Bad LEI"),
            error_codes: Vec::new(&env),
            received_at: 1_700_001_000,
            ack_hash: BytesN::from_array(&env, &[3u8; 32]),
        };

        let action = apply_acknowledgment(&mut report, &ack, 1_700_001_000).unwrap();
        assert_eq!(report.status, ReportStatus::Rejected);
        assert_eq!(action, ReportAction::Rejected);
        assert_eq!(report.updated_at, 1_700_001_000);
    }

    /// compute_submission_id is collision-resistant across report IDs and
    /// attempts (no two distinct inputs collide in the tested space).
    #[test]
    fn test_submission_id_collision_resistance_sample() {
        let env = Env::default();
        let mut seen = Vec::new(&env);
        for report_byte in 0u8..16u8 {
            for attempt in 1u32..5u32 {
                let report_id = BytesN::from_array(&env, &[report_byte; 32]);
                let id = compute_submission_id(&env, &report_id, attempt);
                assert!(
                    !seen.iter().any(|existing| *existing == id),
                    "collision for report_byte={report_byte}, attempt={attempt}"
                );
                seen.push_back(id);
            }
        }
    }

    /// mark_validated and mark_submitted are valid sequential transitions.
    #[test]
    fn test_draft_validated_submitted_sequence() {
        let env = Env::default();
        let mut report = validated_report(&env);
        report.status = ReportStatus::Draft;

        mark_validated(&mut report, 1_700_000_100).unwrap();
        assert_eq!(report.status, ReportStatus::Validated);

        mark_submitted(&mut report, 1_700_000_200).unwrap();
        assert_eq!(report.status, ReportStatus::Submitted);

        // Draft → Submitted directly must fail.
        let mut report2 = validated_report(&env);
        report2.status = ReportStatus::Draft;
        assert!(mark_submitted(&mut report2, 1_700_000_200).is_err());
    }

    /// cancel_report works from any non-terminal state.
    #[test]
    fn test_cancel_from_any_non_terminal_state() {
        let env = Env::default();
        let non_terminal = [
            ReportStatus::Draft,
            ReportStatus::Validated,
            ReportStatus::Submitted,
            ReportStatus::Acknowledged,
            ReportStatus::Rejected,
        ];
        for status in non_terminal {
            let mut report = validated_report(&env);
            report.status = status;
            assert!(
                cancel_report(&mut report, 1_700_000_500).is_ok(),
                "cancel from {status:?} should succeed"
            );
            assert_eq!(report.status, ReportStatus::Cancelled);
        }
    }
}
