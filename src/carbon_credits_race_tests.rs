//! Regression tests for simultaneous validator attestations on carbon credits.
//!
//! Soroban serialises transactions, but many can land in the *same ledger*
//! (identical sequence number and timestamp). These tests pin the behaviour
//! when several validators — or the same validator twice — act on one credit
//! inside a single ledger, and when an attestation races a retirement.

use crate::carbon_credits_harness::*;
use crate::rbac::Role;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Symbol};

struct Actors {
    issuer: Address,
    validators: [Address; 3],
}

fn source(env: &Env, mwh: u32) -> RenewableEnergySource {
    RenewableEnergySource {
        source_type: RenewableEnergyType::Wind,
        facility_id: Bytes::from_slice(env, b"WND-7"),
        location: Bytes::from_slice(env, b"North Sea"),
        capacity_mw: 80,
        energy_generated_mwh: mwh,
        verification_date: env.ledger().timestamp(),
        certifications: vec![env],
    }
}

fn offset(env: &Env) -> Offset {
    let now = env.ledger().timestamp();
    Offset {
        offset_type: Symbol::new(env, "wind"),
        project_id: Bytes::from_slice(env, b"WND-P"),
        tonnes_co2e: 10,
        project_location: Bytes::from_slice(env, b"NL"),
        verification_body: Address::generate(env),
        verification_date: now,
        expiration_date: now + 365 * 86_400,
    }
}

fn registry(env: &Env, by: &Address) -> RegistryEntry {
    RegistryEntry {
        registry_id: Bytes::from_slice(env, b"REG-R"),
        registry_name: Bytes::from_slice(env, b"Gold"),
        registry_url: Bytes::from_slice(env, b"https://gs.example"),
        issuance_date: env.ledger().timestamp(),
        verified_by: by.clone(),
        compliance_standard: ComplianceStandard::Gold,
    }
}

fn actors(env: &Env) -> Actors {
    let admin = Address::generate(env);
    let issuer = Address::generate(env);
    let validators = [
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    ];
    setup_rbac(env, &admin, &issuer, Role::Submitter);
    for v in validators.iter() {
        setup_rbac(env, &admin, v, Role::Auditor);
    }
    Actors { issuer, validators }
}

fn issue(env: &Env, a: &Actors, tonnes: u32, mwh: u32) -> BytesN<32> {
    issue_carbon_credit(
        env,
        a.issuer.clone(),
        tonnes,
        source(env, mwh),
        offset(env),
        registry(env, &a.validators[0]),
        ComplianceStandard::Gold,
    )
}

#[test]
fn concurrent_attestations_get_distinct_record_ids() {
    let env = setup_env();
    let a = actors(&env);
    let id = issue(&env, &a, 100, 400);

    // All three validators attest within the same ledger.
    for v in a.validators.iter() {
        assert!(verify_renewable_energy(&env, id.clone(), v.clone(), 400));
    }

    let credit = get_credit_details(&env, id);
    assert_eq!(credit.verification_records.len(), 3);
    let r = &credit.verification_records;
    let (a0, a1, a2) = (
        r.get_unchecked(0).verification_id,
        r.get_unchecked(1).verification_id,
        r.get_unchecked(2).verification_id,
    );
    assert_ne!(a0, a1);
    assert_ne!(a1, a2);
    assert_ne!(a0, a2);
}

#[test]
fn duplicate_attestation_by_same_validator_is_rejected() {
    let env = setup_env();
    let a = actors(&env);
    let id = issue(&env, &a, 100, 400);
    let v = a.validators[1].clone();

    assert!(verify_renewable_energy(&env, id.clone(), v.clone(), 400));
    let version_after_first = get_credit_details(&env, id.clone()).version;

    assert!(!verify_renewable_energy(&env, id.clone(), v, 400));

    let credit = get_credit_details(&env, id);
    assert_eq!(credit.verification_records.len(), 1);
    assert_eq!(credit.version, version_after_first, "rejected attestation must not mutate");
}

#[test]
fn late_attestation_cannot_resurrect_retired_credit() {
    let env = setup_env();
    let a = actors(&env);
    let id = issue(&env, &a, 100, 400);

    assert!(verify_renewable_energy(&env, id.clone(), a.validators[0].clone(), 400));
    assert!(retire_credit(
        &env,
        a.issuer.clone(),
        id.clone(),
        Bytes::from_slice(&env, b"offset")
    ));

    // A second validator's attestation lands after retirement in the same ledger.
    assert!(!verify_renewable_energy(&env, id.clone(), a.validators[1].clone(), 400));
    assert_eq!(get_credit_status(&env, id.clone()), CreditStatus::Retired);
    assert_eq!(get_credit_details(&env, id).verification_records.len(), 1);
}

#[test]
fn same_ledger_issuance_never_collides() {
    let env = setup_env();
    let a = actors(&env);

    // Identical issuer, tonnage, and timestamp: previously the same credit id.
    let first = issue(&env, &a, 100, 400);
    let second = issue(&env, &a, 100, 400);

    assert_ne!(first, second);
    assert_eq!(get_total_credits_issued(&env), 2);
    assert_eq!(get_issuer_credits(&env, a.issuer.clone()).len(), 2);
    assert_eq!(get_credit_details(&env, first).carbon_tonnes, 100);
    assert_eq!(get_credit_details(&env, second).carbon_tonnes, 100);
}

#[test]
fn same_ledger_tokenization_yields_distinct_tokens_and_is_one_shot() {
    let env = setup_env();
    let a = actors(&env);
    let c1 = issue(&env, &a, 100, 400);
    let c2 = issue(&env, &a, 200, 800);

    let t1 = tokenize_credit(&env, c1.clone(), a.issuer.clone(), 1_000, 15);
    let t2 = tokenize_credit(&env, c2, a.issuer.clone(), 2_000, 15);
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_ne!(t1, zero);
    assert_ne!(t2, zero);
    assert_ne!(t1, t2);

    // Re-tokenizing must not overwrite the existing token record.
    assert_eq!(tokenize_credit(&env, c1.clone(), a.issuer.clone(), 9_999, 99), zero);
    let credit = get_credit_details(&env, c1);
    assert_eq!(credit.tokenization.token_id, t1);
    assert_eq!(credit.tokenization.total_tokens, 1_000);
}

#[test]
fn concurrent_audits_get_distinct_ids_and_extreme_readings_do_not_overflow() {
    let env = setup_env();
    let a = actors(&env);
    let id = issue(&env, &a, 100, 0);

    let r1 = audit_renewable_usage(&env, id.clone(), a.validators[0].clone(), u32::MAX);
    let r2 = audit_renewable_usage(&env, id.clone(), a.validators[1].clone(), 0);

    assert_ne!(r1.verification_id, r2.verification_id);
    assert!(!r1.approved, "a 4.29e9 MWh discrepancy must be flagged");
    assert!(r2.approved);
}

#[test]
fn record_ids_are_deterministic_across_replays() {
    fn run() -> (BytesN<32>, BytesN<32>) {
        let env = setup_env();
        let a = actors(&env);
        let id = issue(&env, &a, 100, 400);
        verify_renewable_energy(&env, id.clone(), a.validators[0].clone(), 400);
        let rec = get_credit_details(&env, id.clone())
            .verification_records
            .get_unchecked(0);
        (id, rec.verification_id)
    }
    // Same inputs → identical ids; address generation in tests is itself
    // deterministic for a fresh `Env`.
    assert_eq!(run(), run());
}

#[test]
fn every_mutation_advances_version() {
    let env = setup_env();
    let a = actors(&env);
    let holder = Address::generate(&env);
    let id = issue(&env, &a, 100, 400);
    let mut last = get_credit_details(&env, id.clone()).version;

    let mut step = |env: &Env| {
        let v = get_credit_details(env, id.clone()).version;
        assert!(v > last, "version must strictly increase ({v} <= {last})");
        last = v;
    };

    verify_renewable_energy(&env, id.clone(), a.validators[0].clone(), 400);
    step(&env);
    link_to_standard(&env, a.issuer.clone(), id.clone(), ComplianceStandard::Vcs);
    step(&env);
    register_credit(&env, a.issuer.clone(), id.clone(), Bytes::from_slice(&env, b"REG-2"));
    step(&env);
    tokenize_credit(&env, id.clone(), a.issuer.clone(), 10, 1);
    step(&env);
    transfer_credit(&env, id.clone(), a.issuer.clone(), holder);
    step(&env);
}
