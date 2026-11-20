//! Concurrency / race-condition tests for the digital-passport subsystem.
//!
//! Soroban executes the transactions of a ledger sequentially, so "simultaneous"
//! validator attestations manifest as several attestations landing in the same
//! ledger, possibly interleaved with a manufacturer update or a lifecycle
//! transition. These tests replay those interleavings deterministically.

use crate::digital_passport::*;
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, vec, Address, Bytes, BytesN, Env, Symbol,
};

#[contract]
struct PassportTestContract;

#[contractimpl]
impl PassportTestContract {}

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PassportTestContract, ());
    (env, contract_id)
}

fn material(env: &Env, percent: u32) -> Material {
    Material {
        material_name: Bytes::from_slice(env, b"Aluminum"),
        material_code: Symbol::new(env, "AL"),
        percentage_by_weight: percent,
        source_type: Symbol::new(env, "virgin"),
        hazardous: false,
        hazard_classification: Bytes::new(env),
    }
}

fn new_passport(env: &Env, manufacturer: &Address, product: &[u8]) -> BytesN<32> {
    create_passport(
        env,
        Bytes::from_slice(env, product),
        Bytes::from_slice(env, b"Widget"),
        Symbol::new(env, "electronics"),
        manufacturer.clone(),
        Bytes::from_slice(env, b"M-1"),
        Bytes::from_slice(env, b"B-1"),
        vec![env, material(env, 100)],
        Durability {
            expected_lifetime_years: 10,
            warranty_years: 2,
            spare_parts_available: true,
            spare_parts_years: 5,
            repair_information: Bytes::new(env),
            repairability_score: 8,
        },
        Circularity {
            recyclable_materials: vec![env],
            recycled_content_percent: 30,
            reuse_potential: true,
            refurbishment_potential: true,
            disassembly_instructions: Bytes::new(env),
            recycling_instructions: Bytes::new(env),
            end_of_life_score: 85,
        },
        CarbonFootprint {
            manufacturing_emissions: 500,
            distribution_emissions: 100,
            use_phase_emissions: 50,
            end_of_life_emissions: 25,
            total_embodied_carbon: 675,
            carbon_neutral: false,
            carbon_offset_program: Bytes::new(env),
            measurement_standard: Symbol::new(env, "ISO_14040"),
            measurement_date: 0,
        },
    )
}

#[test]
fn distinct_validators_attest_same_version_in_one_ledger() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let pid = new_passport(&env, &Address::generate(&env), b"P-1");
        let v1 = Address::generate(&env);
        let v2 = Address::generate(&env);
        let v3 = Address::generate(&env);

        for v in [&v1, &v2, &v3] {
            assert_eq!(
                attest_espr_compliance(&env, pid.clone(), v.clone(), 1),
                Ok(ComplianceStatus::Compliant)
            );
            assert_eq!(get_attested_version(&env, pid.clone(), v.clone()), Some(1));
        }
        // Attestations do not bump the version, so a quorum can form.
        assert_eq!(get_passport(&env, pid).version, 1);
    });
}

#[test]
fn duplicate_attestation_is_rejected_without_side_effects() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let pid = new_passport(&env, &Address::generate(&env), b"P-2");
        let v = Address::generate(&env);

        assert!(attest_espr_compliance(&env, pid.clone(), v.clone(), 1).is_ok());
        let trail_len = get_audit_trail(&env, pid.clone()).len();

        assert_eq!(
            attest_espr_compliance(&env, pid.clone(), v.clone(), 1),
            Err(DigitalPassportError::DuplicateAttestation)
        );
        assert_eq!(get_audit_trail(&env, pid).len(), trail_len);
    });
}

#[test]
fn attestation_racing_an_update_is_rejected_as_stale() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let manufacturer = Address::generate(&env);
        let pid = new_passport(&env, &manufacturer, b"P-3");
        let v = Address::generate(&env);

        // The manufacturer's update lands first in the same ledger…
        update_passport(&env, pid.clone(), Some(vec![&env, material(&env, 40)]), None, None, manufacturer);
        // …so an attestation pinned to the inspected version 1 must fail.
        assert_eq!(
            attest_espr_compliance(&env, pid.clone(), v.clone(), 1),
            Err(DigitalPassportError::StaleVersion)
        );
        assert_eq!(get_attested_version(&env, pid.clone(), v.clone()), None);
        // Re-reading and re-attesting the fresh version succeeds and sees the new data.
        assert_eq!(
            attest_espr_compliance(&env, pid, v, 2),
            Ok(ComplianceStatus::NonCompliant)
        );
    });
}

#[test]
fn lifecycle_transition_bumps_version_and_invalidates_attestation() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let manufacturer = Address::generate(&env);
        let pid = new_passport(&env, &manufacturer, b"P-4");
        let v = Address::generate(&env);

        transition_lifecycle_stage(
            &env,
            pid.clone(),
            PassportLifecycleStage::InProduction,
            manufacturer,
            Bytes::from_slice(&env, b"start"),
        );
        let p = get_passport(&env, pid.clone());
        assert_eq!(p.lifecycle_stage, PassportLifecycleStage::InProduction);
        assert_eq!(p.version, 2);
        assert_eq!(get_lifecycle_history(&env, pid.clone()).len(), 1);

        assert_eq!(
            attest_espr_compliance(&env, pid.clone(), v.clone(), 1),
            Err(DigitalPassportError::StaleVersion)
        );
        // A validator may attest again once the passport has a new version.
        assert!(attest_espr_compliance(&env, pid, v, 2).is_ok());
    });
}

#[test]
fn attesting_unknown_passport_returns_typed_error() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let missing = BytesN::from_array(&env, &[7u8; 32]);
        assert_eq!(
            attest_espr_compliance(&env, missing, Address::generate(&env), 1),
            Err(DigitalPassportError::PassportNotFound)
        );
    });
}

#[test]
fn audit_seq_stays_strictly_monotonic_past_retention_cap() {
    let (env, cid) = setup();
    env.as_contract(&cid, || {
        let pid = new_passport(&env, &Address::generate(&env), b"P-5");
        let v = Address::generate(&env);
        let writes = MAX_AUDIT_TRAIL_LEN + 5;
        for _ in 0..writes {
            verify_espr_compliance(&env, pid.clone(), v.clone());
        }
        let trail = get_audit_trail(&env, pid);
        assert_eq!(trail.len(), MAX_AUDIT_TRAIL_LEN);
        let mut prev: Option<PassportAuditEvent> = None;
        for e in trail.iter() {
            if let Some(p) = prev {
                assert_eq!(e.seq, p.seq + 1, "seq must never repeat after trimming");
            }
            prev = Some(e);
        }
        assert_eq!(prev.unwrap().seq, u64::from(writes) - 1);
    });
}

#[test]
fn attestation_hashes_are_deterministic_across_replays() {
    let run = || {
        let (env, cid) = setup();
        let v = Address::generate(&env);
        env.as_contract(&cid, || {
            let pid = new_passport(&env, &Address::generate(&env), b"P-6");
            attest_espr_compliance(&env, pid.clone(), v, 1).unwrap();
            get_audit_trail(&env, pid).last().unwrap().payload_hash.to_array()
        })
    };
    assert_eq!(run(), run());
}
