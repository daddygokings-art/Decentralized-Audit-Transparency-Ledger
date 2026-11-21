#![cfg(test)]

//! Multi-actor fuzzing for the regulatory-reporting pipeline under Byzantine
//! validator behaviour.
//!
//! Each proptest case builds a fresh ledger with several reports in
//! `Submitted`, then drives a random interleaving of operations from a fixed
//! cast of actors:
//!
//! | actor | role                                     |
//! |-------|------------------------------------------|
//! | 0–2   | validators (`Submitter`)                 |
//! | 3     | auditor (`Auditor`, outranks submitter)  |
//! | 4     | suspended validator                      |
//! | 5     | outsider (no role)                       |
//!
//! Any actor may behave Byzantine: equivocating outcomes or references,
//! presenting stale / future report versions, replaying acknowledgments with
//! skewed clocks, forging submission↔report linkage, sending oversized
//! payloads, replaying or tampering with submission batches, and triggering
//! premature partition recovery.
//!
//! After **every** operation the full ledger snapshot is checked against the
//! pipeline invariants (see `check_step`).  Separate properties check that
//! execution is deterministic (identical op sequences yield byte-identical
//! XDR state) and that any tampering with an audit trail is detected.

extern crate std;

use proptest::prelude::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, BytesN, Env, Vec};
use std::vec::Vec as StdVec;

use crate::rbac::Role;
use crate::regulatory_reporting::{
    AuthorityConfig, RegulatoryAuthority, RegulatoryReport, RegulatorySubmission, ReportAction,
    ReportFormat, ReportStatus, ReportingAuditEntry, ReportingError, SubmissionAcknowledgment,
    ValidationResult,
};
use crate::reporting_audit_trail::{
    reconcile_trails, record_accepted, record_generated, record_rejected, record_submitted,
    verify_trail,
};
use crate::submission_tracker::test_support::{
    attest, create_submission, get_submission, grant, in_ctx, load_report, recover, seed, setup,
    submission_count, suspend,
};
use crate::submission_tracker::{
    check_transition, compute_submission_id, get_submission_ack, store_submission,
    store_submissions_batch, RecoveryOutcome, MAX_ACK_PAYLOAD_LEN,
};

// ─────────────────────────────────────────────────────────────────────────────
// World
// ─────────────────────────────────────────────────────────────────────────────

const REPORTS: u8 = 3;
const ACTORS: usize = 6;
const MAX_ATTEMPTS: u32 = 3;
const ACK_TIMEOUT: u64 = 3_600;
const T0: u64 = 1_700_000_100;
const DEADLINE: u64 = 1_700_172_800;
const SUSPENDED: usize = 4;
const OUTSIDER: usize = 5;

fn config(env: &Env) -> AuthorityConfig {
    AuthorityConfig {
        authority: RegulatoryAuthority::FINRA,
        enabled: true,
        endpoint: Bytes::from_slice(env, b"https://finra.example/api"),
        credential_ref: Bytes::from_slice(env, b"cred-ref"),
        max_retries: MAX_ATTEMPTS,
        retry_delay_seconds: 60,
        exponential_backoff: true,
        retention_ledgers: 52_560,
    }
}

fn report_id(env: &Env, r: u8) -> BytesN<32> {
    BytesN::from_array(env, &[0xA0 | r; 32])
}

fn report(env: &Env, r: u8) -> RegulatoryReport {
    RegulatoryReport {
        id: report_id(env, r),
        authority: RegulatoryAuthority::FINRA,
        format: ReportFormat::FinraOATS,
        entity: Address::generate(env),
        lei: Bytes::from_slice(env, b"HWUPKR0MPOU8LEYPWAT0"),
        period_start: 1_700_000_000,
        period_end: 1_700_086_400,
        deadline: DEADLINE,
        content: Bytes::from_slice(env, b"authority=FINRA\nmpid=TEST\n"),
        schema_version: 1,
        status: ReportStatus::Validated,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        last_validation: ValidationResult {
            passed: true,
            error_count: 0,
            warning_count: 0,
            errors: Vec::new(env),
            warnings: Vec::new(env),
            validated_at: 1_700_000_000,
        },
        prev_report_hash: BytesN::from_array(env, &[0u8; 32]),
        report_hash: BytesN::from_array(env, &[r; 32]),
        source_event_ids: Vec::new(env),
    }
}

struct World {
    env: Env,
    actors: StdVec<Address>,
    /// Off-chain audit trail per report, extended on every committed action.
    trails: StdVec<Vec<ReportingAuditEntry>>,
}

impl World {
    fn new() -> Self {
        let (env, v0) = setup();
        let mut actors = std::vec![v0];
        for i in 1..ACTORS {
            let a = Address::generate(&env);
            match i {
                1 | 2 => grant(&env, &a, Role::Submitter),
                3 => grant(&env, &a, Role::Auditor),
                SUSPENDED => {
                    grant(&env, &a, Role::Submitter);
                    suspend(&env, &a);
                }
                _ => {} // OUTSIDER: no role
            }
            actors.push(a);
        }

        let mut trails = StdVec::new();
        for r in 0..REPORTS {
            let mut rep = report(&env, r);
            let sub = create_submission(&env, &actors[0], &rep, 1, &config(&env), T0).unwrap();
            rep.status = ReportStatus::Submitted;
            seed(&env, &rep, &sub);

            let mut trail = Vec::new(&env);
            record_generated(&env, &mut trail, &rep.id, actors[0].clone(), Bytes::from_slice(&env, b"FinraOATS"), T0);
            record_submitted(&env, &mut trail, &rep.id, actors[0].clone(), &sub.id, 1, T0);
            trails.push(trail);
        }
        World { env, actors, trails }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operations
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Op {
    /// Attest an outcome for one of the report's submissions.  `attempt`
    /// 0..=2 picks an existing attempt (modulo the latest); 3 targets the
    /// not-yet-existing next attempt.  `version_skew` models stale (-1) /
    /// current (0) / future (+1) snapshots; `reference` lets the same actor
    /// equivocate on the reference number.
    Attest {
        actor: usize,
        report: u8,
        attempt: u32,
        version_skew: i8,
        accepted: bool,
        reference: u8,
        clock_skew: u16,
    },
    /// Attest a submission belonging to a *different* report (forged linkage).
    ForgeOrphan { actor: usize, report: u8, other: u8 },
    /// Attest with a payload above `MAX_ACK_PAYLOAD_LEN`.
    Oversized { actor: usize, report: u8 },
    /// Partition recovery of the latest attempt at `now = T0 + elapsed`.
    Recover {
        actor: usize,
        report: u8,
        version_skew: i8,
        elapsed: u64,
    },
    /// Replay every stored submission of a report as a batch, optionally
    /// tampering with one of them.
    ReplayBatch { report: u8, tamper: bool },
}

/// Mostly-honest version views, with stale and future snapshots mixed in.
fn arb_skew() -> impl Strategy<Value = i8> {
    prop_oneof![4 => Just(0i8), 1 => Just(-1i8), 1 => Just(1i8)]
}

fn arb_op() -> impl Strategy<Value = Op> {
    let actor = 0..ACTORS;
    let rep = 0..REPORTS;
    prop_oneof![
        5 => (actor.clone(), rep.clone(), 0u32..=3, arb_skew(), any::<bool>(), 0u8..2, any::<u16>())
            .prop_map(|(actor, report, attempt, version_skew, accepted, reference, clock_skew)| Op::Attest {
                actor, report, attempt, version_skew, accepted, reference, clock_skew,
            }),
        1 => (actor.clone(), rep.clone(), rep.clone())
            .prop_map(|(actor, report, other)| Op::ForgeOrphan { actor, report, other }),
        1 => (actor.clone(), rep.clone()).prop_map(|(actor, report)| Op::Oversized { actor, report }),
        3 => (actor, rep.clone(), arb_skew(), prop_oneof![
                0u64..ACK_TIMEOUT,
                ACK_TIMEOUT..4 * ACK_TIMEOUT,
                4 * ACK_TIMEOUT..DEADLINE - T0 + ACK_TIMEOUT,
            ])
            .prop_map(|(actor, report, version_skew, elapsed)| Op::Recover {
                actor, report, version_skew, elapsed,
            }),
        1 => (rep, any::<bool>()).prop_map(|(report, tamper)| Op::ReplayBatch { report, tamper }),
    ]
}

fn skew(version: u32, by: i8) -> u32 {
    version.wrapping_add_signed(i32::from(by))
}

/// Execute `op`, extending the off-chain trail on commits.  Returns the
/// acting address (if any) and the outcome.
fn run(w: &mut World, op: &Op) -> (Option<usize>, Result<(), ReportingError>) {
    let env = &w.env;
    match *op {
        Op::Attest { actor, report, attempt, version_skew, accepted, reference, clock_skew } => {
            let rid = report_id(env, report);
            let latest = in_ctx(env, || crate::submission_tracker::latest_attempt(env, &rid)).max(1);
            let attempt = if attempt < 3 { 1 + attempt % latest } else { latest + 1 };
            let sid = compute_submission_id(env, &rid, attempt);
            let version = load_report(env, &rid).unwrap().1;
            let prior = in_ctx(env, || get_submission_ack(env, &sid));
            let reference = [b'R', b'0' + reference];
            let r = attest(
                env,
                &w.actors[actor],
                &rid,
                &sid,
                skew(version, version_skew),
                &reference,
                accepted,
                T0 + 1_000 + u64::from(clock_skew),
            );

            // Oracles for authorised actors (identity checks live in check_step).
            if actor != SUSPENDED && actor != OUTSIDER {
                match &prior {
                    // First-writer-wins: identical replays are idempotent and
                    // return the canonical ack; anything else conflicts.
                    Some(canonical) => {
                        if canonical.accepted == accepted && canonical.reference_number == Bytes::from_slice(env, &reference) {
                            let out = r.as_ref().expect("identical replay must succeed");
                            assert!(out.replayed && out.ack == *canonical, "replay not idempotent: {out:?}");
                        } else {
                            assert_eq!(r.as_ref().err(), Some(&ReportingError::ConflictingAcknowledgment));
                        }
                    }
                    // Compare-and-swap: only the current version may commit a
                    // report transition.
                    None => {
                        if let Ok(out) = &r {
                            assert!(version_skew == 0 || out.superseded, "CAS bypass with skew {version_skew}: {out:?}");
                        }
                    }
                }
            }
            if let Ok(out) = &r {
                if !out.replayed && !out.superseded {
                    let trail = &mut w.trails[usize::from(report)];
                    let who = w.actors[actor].clone();
                    match out.action {
                        ReportAction::Accepted => {
                            record_accepted(env, trail, &rid, who, out.ack.reference_number.clone(), out.ack.received_at);
                        }
                        _ => {
                            record_rejected(env, trail, &rid, who, Bytes::new(env), Vec::new(env), out.ack.received_at);
                        }
                    }
                }
            }
            (Some(actor), r.map(|_| ()))
        }
        Op::ForgeOrphan { actor, report, other } => {
            let rid = report_id(env, report);
            let foreign = compute_submission_id(env, &report_id(env, other), 1);
            let version = load_report(env, &rid).unwrap().1;
            let r = attest(env, &w.actors[actor], &rid, &foreign, version, b"R0", true, T0 + 1_000);
            if report != other && actor != SUSPENDED && actor != OUTSIDER {
                assert_eq!(r.as_ref().err(), Some(&ReportingError::AcknowledgmentOrphan));
            }
            if let Ok(out) = &r {
                assert!(out.replayed || out.superseded || report == other, "forged linkage committed: {out:?}");
            }
            (Some(actor), r.map(|_| ()))
        }
        Op::Oversized { actor, report } => {
            let rid = report_id(env, report);
            let sid = compute_submission_id(env, &rid, 1);
            let version = load_report(env, &rid).unwrap().1;
            let big = Bytes::from_slice(env, &std::vec![0u8; MAX_ACK_PAYLOAD_LEN as usize + 1]);
            let r = in_ctx(env, || {
                crate::submission_tracker::attest_acknowledgment(
                    env,
                    &w.actors[actor],
                    &rid,
                    &sid,
                    version,
                    big,
                    Bytes::from_slice(env, b"R9"),
                    true,
                    Bytes::new(env),
                    Vec::new(env),
                    T0 + 1_000,
                )
            });
            (Some(actor), r.map(|_| ()))
        }
        Op::Recover { actor, report, version_skew, elapsed } => {
            let rid = report_id(env, report);
            let latest = in_ctx(env, || crate::submission_tracker::latest_attempt(env, &rid)).max(1);
            let sid = compute_submission_id(env, &rid, latest);
            let version = load_report(env, &rid).unwrap().1;
            let now = T0 + elapsed;
            let r = recover(env, &w.actors[actor], &rid, &sid, &config(env), ACK_TIMEOUT, skew(version, version_skew), now);
            if r.is_ok() {
                assert_eq!(version_skew, 0, "recovery CAS bypass: {r:?}");
            }
            if let Ok(RecoveryOutcome::Resubmitted(sub)) = &r {
                let who = w.actors[actor].clone();
                record_submitted(env, &mut w.trails[usize::from(report)], &rid, who, &sub.id, sub.attempt, now);
            }
            (Some(actor), r.map(|_| ()))
        }
        Op::ReplayBatch { report, tamper } => {
            let rid = report_id(env, report);
            let mut batch = Vec::new(env);
            for a in 1..=MAX_ATTEMPTS {
                if let Some(s) = get_submission(env, &compute_submission_id(env, &rid, a)) {
                    batch.push_back(s);
                }
            }
            if tamper {
                let mut s = batch.get(0).unwrap();
                s.response_code = s.response_code.wrapping_add(1);
                batch.set(0, s);
            }
            if !tamper {
                // Single-item replays must be idempotent too.
                in_ctx(env, || {
                    for sub in batch.iter() {
                        store_submission(env, &sub).expect("identical replay must succeed");
                    }
                });
            }
            let r = in_ctx(env, || store_submissions_batch(env, &batch));
            match (tamper, &r) {
                (false, Ok(n)) => assert_eq!(*n, 0, "replayed batch must store nothing"),
                (true, Err(ReportingError::DuplicateSubmission)) => {}
                other => panic!("unexpected batch replay result {other:?}"),
            }
            (None, r.map(|_| ()))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Snapshot + invariants
// ─────────────────────────────────────────────────────────────────────────────

/// A stored submission together with its canonical acknowledgment.
type SubSlot = Option<(RegulatorySubmission, Option<SubmissionAcknowledgment>)>;

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    reports: StdVec<(RegulatoryReport, u32)>,
    /// Per report, per attempt 1..=MAX_ATTEMPTS.
    subs: StdVec<StdVec<SubSlot>>,
    count: u32,
}

fn snapshot(env: &Env) -> Snapshot {
    let mut reports = StdVec::new();
    let mut subs = StdVec::new();
    for r in 0..REPORTS {
        let rid = report_id(env, r);
        reports.push(load_report(env, &rid).unwrap());
        let mut per = StdVec::new();
        for a in 1..=MAX_ATTEMPTS {
            let sid = compute_submission_id(env, &rid, a);
            per.push(get_submission(env, &sid).map(|s| (s, in_ctx(env, || get_submission_ack(env, &sid)))));
        }
        subs.push(per);
    }
    Snapshot { reports, subs, count: submission_count(env) }
}

/// Byte-exact XDR encoding of a snapshot, comparable across `Env`s.
fn snapshot_xdr(env: &Env, s: &Snapshot) -> StdVec<u8> {
    let mut out = StdVec::new();
    let mut push = |b: Bytes| out.extend(b.iter());
    for (rep, v) in &s.reports {
        push(rep.clone().to_xdr(env));
        push(v.to_xdr(env));
    }
    for (sub, ack) in s.subs.iter().flatten().flatten() {
        push(sub.clone().to_xdr(env));
        if let Some(a) = ack {
            push(a.clone().to_xdr(env));
        }
    }
    push(s.count.to_xdr(env));
    out
}

/// `next` is reachable from `prev` through at most two legal transitions
/// (an attestation goes Submitted → Acknowledged → Accepted/Rejected).
fn reachable(prev: ReportStatus, next: ReportStatus) -> bool {
    prev == next
        || check_transition(prev, next).is_ok()
        || (check_transition(prev, ReportStatus::Acknowledged).is_ok()
            && check_transition(ReportStatus::Acknowledged, next).is_ok())
}

fn check_step(op: &Op, actor: Option<usize>, result: &Result<(), ReportingError>, before: &Snapshot, after: &Snapshot) {
    // Error atomicity: a failed operation never mutates state.
    if result.is_err() {
        assert_eq!(before, after, "failed {op:?} ({result:?}) mutated state");
    }

    // Byzantine identities can never change anything.
    if matches!(actor, Some(SUSPENDED | OUTSIDER)) {
        assert_eq!(result, &Err(ReportingError::UnauthorizedEntity), "{op:?}");
    }

    let mut new_subs = 0;
    for (r, ((rb, vb), (ra, va))) in before.reports.iter().zip(&after.reports).enumerate() {
        // Versions are monotonic and step by at most one per operation, and a
        // report never changes without a version bump.
        assert!(*va == *vb || *va == vb + 1, "report {r}: version {vb} -> {va}");
        if va == vb {
            assert_eq!(rb, ra, "report {r} changed without a version bump");
        }
        // Lifecycle only moves along legal edges; terminal states are final.
        assert!(reachable(rb.status, ra.status), "report {r}: {:?} -> {:?}", rb.status, ra.status);
        if rb.status.is_terminal() {
            assert_eq!(rb, ra, "terminal report {r} changed");
        }
        // Immutable identity fields.
        assert_eq!((&rb.id, &rb.report_hash, &rb.content, rb.deadline), (&ra.id, &ra.report_hash, &ra.content, ra.deadline));

        for (a, (sb, sa)) in before.subs[r].iter().zip(&after.subs[r]).enumerate() {
            match (sb, sa) {
                (Some(_), None) => panic!("report {r} attempt {}: submission deleted", a + 1),
                (None, Some(_)) => new_subs += 1,
                (Some((_, Some(ab))), Some((_, aa))) => {
                    // First-writer-wins: a canonical ack never changes.
                    assert_eq!(Some(ab), aa.as_ref(), "report {r} attempt {}: ack rewritten", a + 1);
                }
                _ => {}
            }
        }
        // Attempts are contiguous: attempt n exists only if n-1 does.
        let present: StdVec<bool> = after.subs[r].iter().map(Option::is_some).collect();
        assert!(present.windows(2).all(|w| w[0] || !w[1]), "report {r}: attempt gap {present:?}");
        // At most one stored ack per report is an acceptance.
        let accepts = after.subs[r]
            .iter()
            .flatten()
            .filter(|(_, ack)| ack.as_ref().is_some_and(|a| a.accepted))
            .count();
        assert!(accepts <= 1, "report {r}: {accepts} accepted acks");
        if ra.status == ReportStatus::Accepted {
            assert_eq!(accepts, 1, "report {r} Accepted without an accepted ack");
        }
    }
    // The index only grows by genuinely new submissions (no double-indexing).
    assert_eq!(after.count, before.count + new_subs, "index count drift after {op:?}");
}

fn execute(ops: &[Op]) -> (World, Snapshot) {
    let mut w = World::new();
    let mut before = snapshot(&w.env);
    for op in ops {
        let (actor, result) = run(&mut w, op);
        let after = snapshot(&w.env);
        check_step(op, actor, &result, &before, &after);
        before = after;
    }
    (w, before)
}

// ─────────────────────────────────────────────────────────────────────────────
// Properties
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// All pipeline invariants hold after every step of any interleaving of
    /// honest and Byzantine actors, and every report's off-chain audit trail
    /// remains a valid hash chain.
    #[test]
    fn fuzz_byzantine_validators_preserve_invariants(ops in prop::collection::vec(arb_op(), 1..40)) {
        let (w, _) = execute(&ops);
        for trail in &w.trails {
            prop_assert!(verify_trail(&w.env, trail).is_ok());
            prop_assert_eq!(reconcile_trails(&w.env, trail, trail).unwrap(), trail.clone());
        }
    }

    /// Execution is deterministic: replaying the same operation sequence in a
    /// fresh ledger yields byte-identical state and identical audit-trail
    /// hashes (cryptographic proofs are reproducible).
    #[test]
    fn fuzz_execution_is_deterministic(ops in prop::collection::vec(arb_op(), 1..25)) {
        let (w1, s1) = execute(&ops);
        let (w2, s2) = execute(&ops);
        prop_assert_eq!(snapshot_xdr(&w1.env, &s1), snapshot_xdr(&w2.env, &s2));
        for (t1, t2) in w1.trails.iter().zip(&w2.trails) {
            let h1: StdVec<[u8; 32]> = t1.iter().map(|e| e.entry_hash.to_array()).collect();
            let h2: StdVec<[u8; 32]> = t2.iter().map(|e| e.entry_hash.to_array()).collect();
            prop_assert_eq!(h1, h2);
        }
    }

    /// A Byzantine actor rewriting any field of any audit entry is detected
    /// at or before the tampered sequence, and the tampered copy can never be
    /// reconciled with the honest one.
    #[test]
    fn fuzz_trail_tampering_is_detected(
        ops in prop::collection::vec(arb_op(), 0..20),
        report in 0..REPORTS,
        pick in any::<prop::sample::Index>(),
        field in 0u8..5,
        delta in 1u64..1_000,
    ) {
        let (w, _) = execute(&ops);
        let env = &w.env;
        let honest = &w.trails[usize::from(report)];
        let i = pick.index(honest.len() as usize) as u32;
        let mut e = honest.get(i).unwrap();
        match field {
            0 => e.timestamp = e.timestamp.wrapping_add(delta),
            1 => e.sequence = e.sequence.wrapping_add(delta as u32),
            2 => e.context.push_back(delta as u8),
            3 => e.action = if e.action == ReportAction::Accepted { ReportAction::Rejected } else { ReportAction::Accepted },
            _ => e.report_id = BytesN::from_array(env, &[delta as u8; 32]),
        }
        let mut forged = honest.clone();
        forged.set(i, e);

        let broken = verify_trail(env, &forged).unwrap_err();
        prop_assert!(broken <= i, "tamper at {} detected only at {}", i, broken);
        prop_assert!(reconcile_trails(env, honest, &forged).is_err());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Targeted Byzantine scenarios (regression seeds)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn byzantine_equivocation_race_is_first_writer_wins() {
    let ops = [
        Op::Attest { actor: 0, report: 0, attempt: 0, version_skew: 0, accepted: true, reference: 0, clock_skew: 5 },
        Op::Attest { actor: 1, report: 0, attempt: 0, version_skew: -1, accepted: false, reference: 0, clock_skew: 0 },
        Op::Attest { actor: 2, report: 0, attempt: 0, version_skew: -1, accepted: true, reference: 1, clock_skew: 0 },
        Op::Attest { actor: 3, report: 0, attempt: 0, version_skew: -1, accepted: true, reference: 0, clock_skew: 9 },
    ];
    let (w, s) = execute(&ops);
    assert_eq!(s.reports[0].0.status, ReportStatus::Accepted);
    assert_eq!(s.reports[0].1, 2);
    assert_eq!(w.trails[0].len(), 3);
}

#[test]
fn byzantine_partition_then_late_conflicting_acks() {
    let ops = [
        Op::Recover { actor: 1, report: 1, version_skew: 0, elapsed: ACK_TIMEOUT + 1 },
        // Stale-snapshot recoverer loses the CAS.
        Op::Recover { actor: 2, report: 1, version_skew: -1, elapsed: ACK_TIMEOUT + 1 },
        // Partition heals: late rejection of attempt 1 is quarantined ...
        Op::Attest { actor: 0, report: 1, attempt: 0, version_skew: 0, accepted: false, reference: 0, clock_skew: 0 },
        // ... and the live attempt resolves the report.
        Op::Attest { actor: 3, report: 1, attempt: 1, version_skew: 0, accepted: true, reference: 1, clock_skew: 0 },
        Op::ReplayBatch { report: 1, tamper: false },
        Op::ReplayBatch { report: 1, tamper: true },
    ];
    let (_, s) = execute(&ops);
    assert_eq!(s.reports[1].0.status, ReportStatus::Accepted);
    assert_eq!(s.count, u32::from(REPORTS) + 1);
}

#[test]
fn byzantine_identities_are_inert() {
    let mut ops = StdVec::new();
    for actor in [SUSPENDED, OUTSIDER] {
        for report in 0..REPORTS {
            ops.push(Op::Attest { actor, report, attempt: 0, version_skew: 0, accepted: true, reference: 0, clock_skew: 0 });
            ops.push(Op::Recover { actor, report, version_skew: 0, elapsed: 200_000 });
            ops.push(Op::Oversized { actor, report });
            ops.push(Op::ForgeOrphan { actor, report, other: (report + 1) % REPORTS });
        }
    }
    let (_, s) = execute(&ops);
    assert!(s.reports.iter().all(|(r, v)| r.status == ReportStatus::Submitted && *v == 1));
}

#[test]
fn byzantine_retry_exhaustion_during_partition_is_atomic() {
    // Each recovery happens once the previous attempt has timed out; the
    // third would exceed max_retries and must fail without side effects
    // (check_step asserts the failed op left state untouched).
    let rec = |elapsed| Op::Recover { actor: 0, report: 2, version_skew: 0, elapsed };
    let ops = [rec(ACK_TIMEOUT + 1), rec(2 * ACK_TIMEOUT + 2), rec(3 * ACK_TIMEOUT + 3)];
    let mut w = World::new();
    let mut before = snapshot(&w.env);
    let mut results = StdVec::new();
    for op in &ops {
        let (actor, result) = run(&mut w, op);
        let after = snapshot(&w.env);
        check_step(op, actor, &result, &before, &after);
        results.push(result);
        before = after;
    }
    assert_eq!(results, [Ok(()), Ok(()), Err(ReportingError::MaxRetriesExceeded)]);
    assert_eq!(before.reports[2].1, 3);
}
