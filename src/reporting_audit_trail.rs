//! Reporting Audit Trail — On-Chain Immutable Action Log
//!
//! Every action performed on a regulatory report is recorded as a
//! `ReportingAuditEntry`.  Entries form a hash-chained sequence keyed by
//! `(report_id, sequence)`, providing an append-only, tamper-evident history
//! of everything that happened to each report.
//!
//! Hash chain:
//!
//! ```text
//! entry[0].entry_hash = sha256(report_id || action || actor || timestamp || context || [0;32])
//! entry[n].entry_hash = sha256(report_id || action || actor || timestamp || context || entry[n-1].entry_hash)
//! ```
//!
//! Verifying the chain requires only the sequence of entries — no trusted
//! external state is needed.

use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

use crate::regulatory_reporting::{
    ReportAction, ReportingAuditEntry, ReportStatus,
};

// ─────────────────────────────────────────────────────────────────────────────
// Hash computation
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the entry hash for a single audit entry.
///
/// Hash input: `report_id || action(u32 LE) || sequence(u32 LE) ||
///              timestamp(u64 LE) || context || prev_entry_hash`
pub fn compute_entry_hash(
    env: &Env,
    report_id: &BytesN<32>,
    action: ReportAction,
    sequence: u32,
    timestamp: u64,
    context: &Bytes,
    prev_entry_hash: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, report_id.as_ref()));
    buf.extend_from_array(&(action as u32).to_le_bytes());
    buf.extend_from_array(&sequence.to_le_bytes());
    buf.extend_from_array(&timestamp.to_le_bytes());
    buf.append(context);
    buf.append(&Bytes::from_slice(env, prev_entry_hash.as_ref()));
    env.crypto().sha256(&buf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry creation
// ─────────────────────────────────────────────────────────────────────────────

/// Create a new `ReportingAuditEntry` and append it to `trail`.
///
/// The entry's `prev_entry_hash` is taken from the last entry in `trail`
/// (or `[0u8;32]` if the trail is empty), forming an unbroken hash chain.
///
/// Returns the newly created entry.
pub fn append_entry(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    action: ReportAction,
    actor: Address,
    resulting_status: ReportStatus,
    context: Bytes,
    timestamp: u64,
) -> ReportingAuditEntry {
    let sequence = trail.len();
    let prev_entry_hash: BytesN<32> = if sequence == 0 {
        BytesN::from_array(env, &[0u8; 32])
    } else {
        trail.get(sequence - 1).unwrap().entry_hash.clone()
    };

    let entry_hash = compute_entry_hash(
        env,
        report_id,
        action,
        sequence,
        timestamp,
        &context,
        &prev_entry_hash,
    );

    let entry = ReportingAuditEntry {
        sequence,
        report_id: report_id.clone(),
        action,
        actor,
        timestamp,
        prev_entry_hash,
        entry_hash,
        context,
        resulting_status,
    };

    trail.push_back(entry.clone());
    entry
}

// ─────────────────────────────────────────────────────────────────────────────
// Chain verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verify the integrity of a reporting audit trail.
///
/// Returns `Ok(())` when every entry's `entry_hash` is correctly computed
/// from its own fields, and every `prev_entry_hash` matches the preceding
/// entry's `entry_hash`.
///
/// Returns `Err(sequence)` when the first broken link is at that sequence
/// number.
pub fn verify_trail(env: &Env, trail: &Vec<ReportingAuditEntry>) -> Result<(), u32> {
    let zero_hash = BytesN::<32>::from_array(env, &[0u8; 32]);

    for i in 0..trail.len() {
        let entry = trail.get(i).unwrap();

        // Verify prev_entry_hash linkage
        let expected_prev = if i == 0 {
            zero_hash.clone()
        } else {
            trail.get(i - 1).unwrap().entry_hash.clone()
        };
        if entry.prev_entry_hash != expected_prev {
            return Err(i);
        }

        // Recompute and verify entry_hash
        let expected_hash = compute_entry_hash(
            env,
            &entry.report_id,
            entry.action,
            entry.sequence,
            entry.timestamp,
            &entry.context,
            &entry.prev_entry_hash,
        );
        if entry.entry_hash != expected_hash {
            return Err(i);
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Convenience constructors for each pipeline stage
// ─────────────────────────────────────────────────────────────────────────────

/// Record the "report generated" event.
pub fn record_generated(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    format_bytes: Bytes,
    timestamp: u64,
) -> ReportingAuditEntry {
    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Generated,
        actor,
        ReportStatus::Draft,
        format_bytes,
        timestamp,
    )
}

/// Record the outcome of a validation run.
///
/// `passed` is encoded as `passed=1\n` or `passed=0\n` in context.
pub fn record_validated(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    passed: bool,
    error_count: u32,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(if passed { b"passed=1\n" } else { b"passed=0\n" });
    ctx.extend_from_slice(b"errors=");
    ctx.extend_from_array(&error_count.to_le_bytes());
    ctx.extend_from_slice(b"\n");

    let status = if passed {
        ReportStatus::Validated
    } else {
        ReportStatus::Draft
    };

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Validated,
        actor,
        status,
        ctx,
        timestamp,
    )
}

/// Record a submission dispatch.
///
/// `submission_id_bytes` should be the raw 32-byte submission ID.
pub fn record_submitted(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    submission_id: &BytesN<32>,
    attempt: u32,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"submission_id=");
    ctx.append(&Bytes::from_slice(env, submission_id.as_ref()));
    ctx.extend_from_slice(b"\nattempt=");
    ctx.extend_from_array(&attempt.to_le_bytes());
    ctx.extend_from_slice(b"\n");

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Submitted,
        actor,
        ReportStatus::Submitted,
        ctx,
        timestamp,
    )
}

/// Record an acknowledgment received from the authority.
pub fn record_acknowledgment_received(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    ack_id: &BytesN<32>,
    accepted: bool,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"ack_id=");
    ctx.append(&Bytes::from_slice(env, ack_id.as_ref()));
    ctx.extend_from_slice(b"\naccepted=");
    ctx.extend_from_slice(if accepted { b"1\n" } else { b"0\n" });

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::AcknowledgmentReceived,
        actor,
        ReportStatus::Acknowledged,
        ctx,
        timestamp,
    )
}

/// Record final acceptance by the authority.
pub fn record_accepted(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    reference_number: Bytes,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"reference=");
    ctx.append(&reference_number);
    ctx.extend_from_slice(b"\n");

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Accepted,
        actor,
        ReportStatus::Accepted,
        ctx,
        timestamp,
    )
}

/// Record rejection by the authority.
pub fn record_rejected(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    reason: Bytes,
    error_codes: Vec<Bytes>,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"reason=");
    ctx.append(&reason);
    ctx.extend_from_slice(b"\nerror_count=");
    ctx.extend_from_array(&(error_codes.len() as u32).to_le_bytes());
    ctx.extend_from_slice(b"\n");

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Rejected,
        actor,
        ReportStatus::Rejected,
        ctx,
        timestamp,
    )
}

/// Record an operator cancellation.
pub fn record_cancelled(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    reason: Bytes,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"reason=");
    ctx.append(&reason);
    ctx.extend_from_slice(b"\n");

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::Cancelled,
        actor,
        ReportStatus::Cancelled,
        ctx,
        timestamp,
    )
}

/// Record a report being marked overdue.
pub fn record_overdue(
    env: &Env,
    trail: &mut Vec<ReportingAuditEntry>,
    report_id: &BytesN<32>,
    actor: Address,
    deadline: u64,
    timestamp: u64,
) -> ReportingAuditEntry {
    let mut ctx = Bytes::new(env);
    ctx.extend_from_slice(b"deadline=");
    ctx.extend_from_array(&deadline.to_le_bytes());
    ctx.extend_from_slice(b"\n");

    append_entry(
        env,
        trail,
        report_id,
        ReportAction::MarkedOverdue,
        actor,
        ReportStatus::Overdue,
        ctx,
        timestamp,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

    fn report_id(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[42u8; 32])
    }

    // ── append_entry ──────────────────────────────────────────────────────

    #[test]
    fn test_first_entry_has_zero_prev_hash() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = append_entry(
            &env,
            &mut trail,
            &report_id(&env),
            ReportAction::Generated,
            Address::generate(&env),
            ReportStatus::Draft,
            Bytes::from_slice(&env, b"form=FinraOATS\n"),
            1_700_000_000,
        );
        assert_eq!(entry.prev_entry_hash, BytesN::from_array(&env, &[0u8; 32]));
        assert_eq!(entry.sequence, 0);
    }

    #[test]
    fn test_second_entry_links_to_first() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let e1 = append_entry(
            &env,
            &mut trail,
            &report_id(&env),
            ReportAction::Generated,
            Address::generate(&env),
            ReportStatus::Draft,
            Bytes::from_slice(&env, b"ctx1\n"),
            1_700_000_000,
        );
        let e2 = append_entry(
            &env,
            &mut trail,
            &report_id(&env),
            ReportAction::Validated,
            Address::generate(&env),
            ReportStatus::Validated,
            Bytes::from_slice(&env, b"passed=1\n"),
            1_700_000_010,
        );
        assert_eq!(e2.prev_entry_hash, e1.entry_hash);
        assert_eq!(e2.sequence, 1);
    }

    #[test]
    fn test_trail_length_grows() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        for i in 0u64..5 {
            append_entry(
                &env,
                &mut trail,
                &report_id(&env),
                ReportAction::Generated,
                Address::generate(&env),
                ReportStatus::Draft,
                Bytes::new(&env),
                1_700_000_000 + i,
            );
        }
        assert_eq!(trail.len(), 5);
    }

    // ── verify_trail ─────────────────────────────────────────────────────

    #[test]
    fn test_verify_empty_trail_passes() {
        let env = Env::default();
        let trail = Vec::new(&env);
        assert!(verify_trail(&env, &trail).is_ok());
    }

    #[test]
    fn test_verify_valid_trail_passes() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = report_id(&env);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"FinraOATS"), 1_700_000_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 1_700_000_010);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[5u8; 32]), 1, 1_700_000_020);

        assert!(verify_trail(&env, &trail).is_ok());
    }

    #[test]
    fn test_verify_tampered_trail_fails() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = report_id(&env);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"FinraOATS"), 1_700_000_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 1_700_000_010);

        // Tamper: replace first entry's hash with garbage
        let mut e0 = trail.get(0).unwrap();
        e0.entry_hash = BytesN::from_array(&env, &[0xFFu8; 32]);
        trail.set(0, e0);

        // Verification should detect the broken link at entry 1 (whose prev_entry_hash won't match)
        assert!(verify_trail(&env, &trail).is_err());
    }

    // ── record_* convenience functions ───────────────────────────────────

    #[test]
    fn test_record_generated_status_is_draft() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_generated(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            Bytes::from_slice(&env, b"FinraOATS"),
            1_700_000_000,
        );
        assert_eq!(entry.action, ReportAction::Generated);
        assert_eq!(entry.resulting_status, ReportStatus::Draft);
    }

    #[test]
    fn test_record_validated_pass_status_is_validated() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_validated(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            true,
            0,
            1_700_000_010,
        );
        assert_eq!(entry.resulting_status, ReportStatus::Validated);
    }

    #[test]
    fn test_record_validated_fail_status_is_draft() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_validated(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            false,
            3,
            1_700_000_010,
        );
        assert_eq!(entry.resulting_status, ReportStatus::Draft);
    }

    #[test]
    fn test_record_accepted_status_is_accepted() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_accepted(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            Bytes::from_slice(&env, b"REF-001"),
            1_700_001_000,
        );
        assert_eq!(entry.action, ReportAction::Accepted);
        assert_eq!(entry.resulting_status, ReportStatus::Accepted);
    }

    #[test]
    fn test_record_rejected_status_is_rejected() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_rejected(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            Bytes::from_slice(&env, b"Invalid LEI"),
            Vec::new(&env),
            1_700_001_000,
        );
        assert_eq!(entry.resulting_status, ReportStatus::Rejected);
    }

    #[test]
    fn test_record_cancelled_status_is_cancelled() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_cancelled(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            Bytes::from_slice(&env, b"Operator request"),
            1_700_001_000,
        );
        assert_eq!(entry.resulting_status, ReportStatus::Cancelled);
    }

    #[test]
    fn test_record_overdue_status_is_overdue() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let entry = record_overdue(
            &env,
            &mut trail,
            &report_id(&env),
            Address::generate(&env),
            1_700_172_800,
            1_700_200_000,
        );
        assert_eq!(entry.resulting_status, ReportStatus::Overdue);
    }

    // ── Full pipeline trail ───────────────────────────────────────────────

    #[test]
    fn test_full_accepted_pipeline_trail_is_valid() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = report_id(&env);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"FinraOATS"), 1_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[9u8; 32]), 1, 3_000);
        record_acknowledgment_received(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[11u8; 32]), true, 4_000);
        record_accepted(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"REF-XYZ"), 5_000);

        assert_eq!(trail.len(), 5);
        assert!(verify_trail(&env, &trail).is_ok());
    }

    #[test]
    fn test_full_rejected_then_resubmit_trail_is_valid() {
        let env = Env::default();
        let mut trail = Vec::new(&env);
        let rid = report_id(&env);
        let actor = Address::generate(&env);

        record_generated(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"CFTC-Swap"), 1_000);
        record_validated(&env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[9u8; 32]), 1, 3_000);
        record_acknowledgment_received(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[11u8; 32]), false, 4_000);
        record_rejected(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"Bad UTI"), Vec::new(&env), 4_100);
        record_submitted(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[12u8; 32]), 2, 5_000);
        record_acknowledgment_received(&env, &mut trail, &rid, actor.clone(), &BytesN::from_array(&env, &[13u8; 32]), true, 6_000);
        record_accepted(&env, &mut trail, &rid, actor.clone(), Bytes::from_slice(&env, b"REF-ABC"), 6_100);

        assert_eq!(trail.len(), 8);
        assert!(verify_trail(&env, &trail).is_ok());
    }
}
