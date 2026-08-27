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

use soroban_sdk::{Bytes, BytesN, Env, Vec};

use crate::regulatory_reporting::{
    AuthorityConfig, RegulatoryAuthority, RegulatoryReport, RegulatorySubmission,
    ReportAction, ReportStatus, ReportingError, SubmissionAcknowledgment,
};

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
    buf.append(&Bytes::from_slice(env, report_id.as_ref()));
    buf.extend_from_array(&attempt.to_le_bytes());
    env.crypto().sha256(&buf)
}

/// Build a new `RegulatorySubmission` for the given report and attempt number.
pub fn create_submission(
    env: &Env,
    report: &RegulatoryReport,
    attempt: u32,
    config: &AuthorityConfig,
    now: u64,
) -> Result<RegulatorySubmission, ReportingError> {
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
    if attempt > config.max_retries {
        return Err(ReportingError::MaxRetriesExceeded);
    }

    // Compute retry-after for back-off
    let retry_after = if attempt > 1 {
        let base = config.retry_delay_seconds as u64;
        if config.exponential_backoff {
            // 2^(attempt-1) * base, capped at 24 h
            let factor: u64 = 1u64.saturating_shl((attempt - 1) as u32);
            now + (base.saturating_mul(factor)).min(86_400)
        } else {
            now + base
        }
    } else {
        0 // submit immediately on first attempt
    };

    let id = compute_submission_id(env, &report.id, attempt);

    Ok(RegulatorySubmission {
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
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Acknowledgment ingestion
// ─────────────────────────────────────────────────────────────────────────────

/// Compute an acknowledgment ID from submission_id + received_at timestamp.
pub fn compute_ack_id(env: &Env, submission_id: &BytesN<32>, received_at: u64) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, submission_id.as_ref()));
    buf.extend_from_array(&received_at.to_le_bytes());
    env.crypto().sha256(&buf)
}

/// Compute the hash of an acknowledgment payload for tamper-evidence.
pub fn compute_ack_hash(env: &Env, payload: &Bytes) -> BytesN<32> {
    env.crypto().sha256(payload)
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
    submission: &RegulatorySubmission,
    payload: Bytes,
    reference_number: Bytes,
    accepted: bool,
    rejection_reason: Bytes,
    error_codes: Vec<Bytes>,
    received_at: u64,
) -> Result<SubmissionAcknowledgment, ReportingError> {
    // The submission must be in Submitted state to receive an ack
    if submission.status != ReportStatus::Submitted
        && submission.status != ReportStatus::Acknowledged
    {
        return Err(ReportingError::InvalidStatusTransition);
    }
    if reference_number.is_empty() {
        return Err(ReportingError::AcknowledgmentOrphan);
    }

    let ack_hash = compute_ack_hash(env, &payload);
    let id = compute_ack_id(env, &submission.id, received_at);

    Ok(SubmissionAcknowledgment {
        id,
        submission_id: submission.id.clone(),
        report_id: submission.report_id.clone(),
        reference_number,
        accepted,
        rejection_reason,
        error_codes,
        received_at,
        ack_hash,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Status transitions
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a validated acknowledgment to a mutable report, updating its status.
///
/// Returns the `ReportAction` that was performed so callers can record
/// it in the audit trail.
pub fn apply_acknowledgment(
    report: &mut RegulatoryReport,
    ack: &SubmissionAcknowledgment,
    now: u64,
) -> Result<ReportAction, ReportingError> {
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

    Ok(action)
}

/// Mark a report as overdue (deadline has passed without acceptance).
pub fn mark_overdue(report: &mut RegulatoryReport, now: u64) -> Result<(), ReportingError> {
    check_transition(report.status, ReportStatus::Overdue)?;
    report.status = ReportStatus::Overdue;
    report.updated_at = now;
    Ok(())
}

/// Cancel a report manually.
pub fn cancel_report(report: &mut RegulatoryReport, now: u64) -> Result<(), ReportingError> {
    check_transition(report.status, ReportStatus::Cancelled)?;
    report.status = ReportStatus::Cancelled;
    report.updated_at = now;
    Ok(())
}

/// Transition a validated report to Submitted status.
pub fn mark_submitted(report: &mut RegulatoryReport, now: u64) -> Result<(), ReportingError> {
    check_transition(report.status, ReportStatus::Submitted)?;
    report.status = ReportStatus::Submitted;
    report.updated_at = now;
    Ok(())
}

/// Transition a draft report to Validated status.
pub fn mark_validated(report: &mut RegulatoryReport, now: u64) -> Result<(), ReportingError> {
    check_transition(report.status, ReportStatus::Validated)?;
    report.status = ReportStatus::Validated;
    report.updated_at = now;
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
    let next = current_attempt + 1;
    if next > config.max_retries {
        Err(ReportingError::MaxRetriesExceeded)
    } else {
        Ok(next)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let sub = create_submission(&env, &report, 1, &config, 1_700_000_100).unwrap();
        assert_eq!(sub.attempt, 1);
        assert_eq!(sub.status, ReportStatus::Submitted);
        assert_eq!(sub.retry_after, 0); // first attempt has no delay
    }

    #[test]
    fn test_create_submission_after_deadline_fails() {
        let env = Env::default();
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let result = create_submission(&env, &report, 1, &config, 2_000_000_000); // past deadline
        assert!(matches!(result, Err(ReportingError::DeadlineExceeded)));
    }

    #[test]
    fn test_create_submission_max_retries_exceeded_fails() {
        let env = Env::default();
        let report = validated_report(&env);
        let config = dummy_config(&env);
        let result = create_submission(&env, &report, 4, &config, 1_700_000_100); // attempt 4 > max 3
        assert!(matches!(result, Err(ReportingError::MaxRetriesExceeded)));
    }

    #[test]
    fn test_create_submission_disabled_authority_fails() {
        let env = Env::default();
        let report = validated_report(&env);
        let mut config = dummy_config(&env);
        config.enabled = false;
        let result = create_submission(&env, &report, 1, &config, 1_700_000_100);
        assert!(matches!(result, Err(ReportingError::AuthorityDisabled)));
    }

    #[test]
    fn test_exponential_backoff_second_attempt() {
        let env = Env::default();
        let report = validated_report(&env);
        let config = dummy_config(&env); // backoff=true, delay=60s
        let now = 1_700_000_100u64;
        let sub = create_submission(&env, &report, 2, &config, now).unwrap();
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
        let sub = dummy_submission(&env);
        let ack = ingest_acknowledgment(
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
        assert!(ack.accepted);
        assert_eq!(ack.submission_id, sub.id);
    }

    #[test]
    fn test_ingest_rejection_ok() {
        let env = Env::default();
        let sub = dummy_submission(&env);
        let mut error_codes = Vec::new(&env);
        error_codes.push_back(Bytes::from_slice(&env, b"ERR001"));
        let ack = ingest_acknowledgment(
            &env,
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
        let sub = dummy_submission(&env);
        let result = ingest_acknowledgment(
            &env,
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
    }

    #[test]
    fn test_apply_rejection_sets_rejected() {
        let env = Env::default();
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

        let action = apply_acknowledgment(&mut report, &ack, 1_700_001_000).unwrap();
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
        let mut report = validated_report(&env);
        report.status = ReportStatus::Submitted;
        mark_overdue(&mut report, 1_700_200_000).unwrap();
        assert_eq!(report.status, ReportStatus::Overdue);
    }

    #[test]
    fn test_cancel_from_validated() {
        let env = Env::default();
        let mut report = validated_report(&env);
        cancel_report(&mut report, 1_700_000_200).unwrap();
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
}
