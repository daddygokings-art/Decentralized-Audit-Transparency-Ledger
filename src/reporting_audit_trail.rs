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
//! entry[0].entry_hash = env.crypto().sha256(report_id || action || actor || timestamp || context || [0;32])
//! entry[n].entry_hash = env.crypto().sha256(report_id || action || actor || timestamp || context || entry[n-1].entry_hash)
//! ```
//!
//! Verifying the chain requires only the sequence of entries — no trusted
//! external state is needed.

use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};

use crate::regulatory_reporting::{
    ReportAction, ReportingAuditEntry, ReportStatus, ReportingError,
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
    // The fixed-width fields are packed into one stack buffer so the preimage
    // is built with 3 host calls instead of 6.  The byte layout (and therefore
    // every hash) is identical to the field-by-field encoding.
    let mut header = [0u8; 16];
    header[0..4].copy_from_slice(&(action as u32).to_le_bytes());
    header[4..8].copy_from_slice(&sequence.to_le_bytes());
    header[8..16].copy_from_slice(&timestamp.to_le_bytes());

    let mut buf = report_id.to_bytes();
    buf.extend_from_array(&header);
    buf.append(context);
    buf.append(prev_entry_hash.as_ref());
    env.crypto().sha256(&buf).to_bytes()
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
#[allow(clippy::too_many_arguments)]
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
    let prev_entry_hash: BytesN<32> = match trail.last() {
        Some(last) => last.entry_hash,
        None => BytesN::from_array(env, &[0u8; 32]),
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
    verify_trail_segment(env, trail, 0, &BytesN::from_array(env, &[0u8; 32])).map(|_| ())
}

/// Verify a contiguous segment of a trail that starts at `start_sequence`
/// and must link to `prev_hash` (the zero hash for the genesis segment).
///
/// Returns the last verified `entry_hash` (or `prev_hash` for an empty
/// segment) so very large trails can be verified across several invocations,
/// each staying within the Soroban CPU budget:
///
/// ```text
/// let h = verify_trail_segment(env, &chunk0, 0,  &zero)?;
/// let h = verify_trail_segment(env, &chunk1, n0, &h)?;
/// ```
///
/// Each entry is decoded exactly once and the running hash is carried
/// forward, rather than re-reading the previous entry.  `Err(sequence)`
/// reports the first broken link (absolute sequence number).
pub fn verify_trail_segment(
    env: &Env,
    segment: &Vec<ReportingAuditEntry>,
    start_sequence: u32,
    prev_hash: &BytesN<32>,
) -> Result<BytesN<32>, u32> {
    let mut expected_prev = prev_hash.clone();
    let mut expected_seq = start_sequence;
    for entry in segment.iter() {
        if entry.sequence != expected_seq || entry.prev_entry_hash != expected_prev {
            return Err(expected_seq);
        }
        let recomputed = compute_entry_hash(
            env,
            &entry.report_id,
            entry.action,
            entry.sequence,
            entry.timestamp,
            &entry.context,
            &entry.prev_entry_hash,
        );
        if entry.entry_hash != recomputed {
            return Err(expected_seq);
        }
        expected_prev = recomputed;
        expected_seq = expected_seq.checked_add(1).ok_or(expected_seq)?;
    }
    Ok(expected_prev)
}

// ─────────────────────────────────────────────────────────────────────────────
// Partition recovery: trail reconciliation
// ─────────────────────────────────────────────────────────────────────────────

/// Reconcile two copies of a report's audit trail after a network partition.
///
/// Both trails are verified first (`TrailCorrupted` with the offending
/// sequence otherwise).  If one is a prefix of the other, the longer trail is
/// returned (fast-forward); this is deterministic and independent of argument
/// order.  If they diverge, `TrailForkDetected` is returned with the first
/// diverging sequence number — forks are never merged automatically because
/// that would rewrite tamper-evident history.
pub fn reconcile_trails(
    env: &Env,
    local: &Vec<ReportingAuditEntry>,
    remote: &Vec<ReportingAuditEntry>,
) -> Result<Vec<ReportingAuditEntry>, (ReportingError, u32)> {
    verify_trail(env, local).map_err(|i| (ReportingError::TrailCorrupted, i))?;
    verify_trail(env, remote).map_err(|i| (ReportingError::TrailCorrupted, i))?;

    let common = local.len().min(remote.len());
    for i in 0..common {
        let a = local.get_unchecked(i);
        let b = remote.get_unchecked(i);
        if a.entry_hash != b.entry_hash || a.report_id != b.report_id {
            return Err((ReportingError::TrailForkDetected, i));
        }
    }
    Ok(if remote.len() > local.len() {
        remote.clone()
    } else {
        local.clone()
    })
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
    ctx.append(&submission_id.to_bytes());
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
    ctx.append(&ack_id.to_bytes());
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
    ctx.extend_from_array(&error_codes.len().to_le_bytes());
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
    use soroban_sdk::testutils::Address as _;
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

    // ── Partition reconciliation ──────────────────────────────────────────

    fn base_trail(env: &Env, actor: &Address) -> Vec<ReportingAuditEntry> {
        let mut trail = Vec::new(env);
        let rid = report_id(env);
        record_generated(env, &mut trail, &rid, actor.clone(), Bytes::from_slice(env, b"FinraOATS"), 1_000);
        record_validated(env, &mut trail, &rid, actor.clone(), true, 0, 2_000);
        trail
    }

    #[test]
    fn test_reconcile_fast_forwards_either_order() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let short = base_trail(&env, &actor);
        let mut long = short.clone();
        record_submitted(&env, &mut long, &report_id(&env), actor, &BytesN::from_array(&env, &[9u8; 32]), 1, 3_000);
        assert_eq!(reconcile_trails(&env, &short, &long).unwrap(), long);
        assert_eq!(reconcile_trails(&env, &long, &short).unwrap(), long);
        assert_eq!(reconcile_trails(&env, &long, &long).unwrap(), long);
    }

    #[test]
    fn test_reconcile_detects_fork() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let mut a = base_trail(&env, &actor);
        let mut b = a.clone();
        record_accepted(&env, &mut a, &report_id(&env), actor.clone(), Bytes::from_slice(&env, b"REF-A"), 3_000);
        record_cancelled(&env, &mut b, &report_id(&env), actor, Bytes::from_slice(&env, b"partition"), 3_000);
        assert_eq!(reconcile_trails(&env, &a, &b), Err((ReportingError::TrailForkDetected, 2)));
    }

    #[test]
    fn test_reconcile_rejects_corrupted_trail() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let good = base_trail(&env, &actor);
        let mut bad = good.clone();
        let mut e = bad.get(1).unwrap();
        e.timestamp += 1;
        bad.set(1, e);
        assert_eq!(reconcile_trails(&env, &good, &bad), Err((ReportingError::TrailCorrupted, 1)));
    }

    // ── CPU / memory optimisation guards ──────────────────────────────────

    /// Reference encoding (pre-optimisation, field by field).  Pins the hash
    /// format so the optimised preimage can never silently change proofs.
    fn legacy_entry_hash(
        env: &Env,
        report_id: &BytesN<32>,
        action: ReportAction,
        sequence: u32,
        timestamp: u64,
        context: &Bytes,
        prev: &BytesN<32>,
    ) -> BytesN<32> {
        let mut buf = Bytes::new(env);
        buf.append(&report_id.to_bytes());
        buf.extend_from_array(&(action as u32).to_le_bytes());
        buf.extend_from_array(&sequence.to_le_bytes());
        buf.extend_from_array(&timestamp.to_le_bytes());
        buf.append(context);
        buf.append(&prev.to_bytes());
        env.crypto().sha256(&buf).to_bytes()
    }

    #[test]
    fn test_entry_hash_matches_legacy_encoding() {
        let env = Env::default();
        let ctx = Bytes::from_slice(&env, b"reason=x\n");
        for (seq, ts, act) in [
            (0u32, 0u64, ReportAction::Generated),
            (7, 1_700_000_000, ReportAction::Rejected),
            (u32::MAX, u64::MAX, ReportAction::ConfigUpdated),
        ] {
            let prev = BytesN::from_array(&env, &[seq as u8; 32]);
            assert_eq!(
                compute_entry_hash(&env, &report_id(&env), act, seq, ts, &ctx, &prev),
                legacy_entry_hash(&env, &report_id(&env), act, seq, ts, &ctx, &prev),
            );
        }
    }

    fn long_trail(env: &Env, n: u64) -> Vec<ReportingAuditEntry> {
        let actor = Address::generate(env);
        let mut trail = Vec::new(env);
        for i in 0..n {
            record_generated(env, &mut trail, &report_id(env), actor.clone(), Bytes::from_slice(env, b"x"), i);
        }
        trail
    }

    #[test]
    fn test_chunked_verification_equals_full_verification() {
        let env = Env::default();
        let trail = long_trail(&env, 25);
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        let mut prev = zero.clone();
        let mut start = 0;
        while start < trail.len() {
            let end = (start + 7).min(trail.len());
            prev = verify_trail_segment(&env, &trail.slice(start..end), start, &prev).unwrap();
            start = end;
        }
        assert_eq!(prev, trail.last().unwrap().entry_hash);
        assert_eq!(verify_trail_segment(&env, &trail, 0, &zero).unwrap(), prev);
    }

    #[test]
    fn test_chunked_verification_rejects_misaligned_segment() {
        let env = Env::default();
        let trail = long_trail(&env, 10);
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        // Wrong anchor hash.
        assert_eq!(verify_trail_segment(&env, &trail.slice(5..10), 5, &zero), Err(5));
        // Wrong starting sequence.
        let anchor = trail.get(4).unwrap().entry_hash;
        assert_eq!(verify_trail_segment(&env, &trail.slice(5..10), 6, &anchor), Err(6));
        assert!(verify_trail_segment(&env, &trail.slice(5..10), 5, &anchor).is_ok());
    }

    #[test]
    fn test_verify_trail_cpu_scales_linearly() {
        // Guard against reintroducing per-entry double decoding: verifying a
        // trail must cost at most ~1.1x the per-entry hashing work.
        let env = Env::default();
        let trail = long_trail(&env, 100);
        let mut budget = env.cost_estimate().budget();

        budget.reset_unlimited();
        for e in trail.iter() {
            compute_entry_hash(&env, &e.report_id, e.action, e.sequence, e.timestamp, &e.context, &e.prev_entry_hash);
        }
        let hashing = budget.cpu_instruction_cost();

        budget.reset_unlimited();
        verify_trail(&env, &trail).unwrap();
        let verify = budget.cpu_instruction_cost();
        assert!(verify * 10 <= hashing * 11, "verify={verify} hashing={hashing}");
    }
}
