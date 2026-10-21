#![cfg(test)]
//! RBAC Gating and Precedence Unit Tests (#686 #689 #688 #687)
//!
//! Includes the positive/negative RBAC matrix integration tests for the
//! `esg_reporting` module (#875): role hierarchy precedence, revocation
//! behavior, rejection of unauthorized/suspended submitter DIDs with stable
//! error codes, and structured audit event emission for both successful
//! mutations and rejected access attempts.

use super::esg_reporting::*;
use super::rbac::*;
use super::supply_chain::*;
use super::{AuditLedger, AuditLedgerClient, LogEventCommand, Role as LedgerRole};
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Bytes, Env, IntoVal, String, Symbol};

#[test]
fn test_rbac_roles_and_privilege_ladder() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let auditor = Address::generate(&env);
    let submitter = Address::generate(&env);
    let viewer = Address::generate(&env);

    env.mock_all_auths();
    Rbac::init_admin(&env, &admin);

    assert_eq!(Rbac::get_role(&env, &admin), Some(Role::Admin));
    assert!(Rbac::has_role_min(&env, &admin, Role::Admin));

    // Assign Auditor
    Rbac::set_role(&env, &admin, &auditor, Role::Auditor).unwrap();
    assert_eq!(Rbac::get_role(&env, &auditor), Some(Role::Auditor));
    assert!(Rbac::has_role_min(&env, &auditor, Role::Auditor));
    assert!(Rbac::has_role_min(&env, &auditor, Role::Submitter));
    assert!(Rbac::has_role_min(&env, &auditor, Role::Viewer));
    assert!(!Rbac::has_role_min(&env, &auditor, Role::Admin));

    // Assign Submitter
    Rbac::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();
    assert_eq!(Rbac::get_role(&env, &submitter), Some(Role::Submitter));
    assert!(Rbac::has_role_min(&env, &submitter, Role::Submitter));
    assert!(Rbac::has_role_min(&env, &submitter, Role::Viewer));
    assert!(!Rbac::has_role_min(&env, &submitter, Role::Auditor));

    // Assign Viewer
    Rbac::set_role(&env, &admin, &viewer, Role::Viewer).unwrap();
    assert_eq!(Rbac::get_role(&env, &viewer), Some(Role::Viewer));
    assert!(Rbac::has_role_min(&env, &viewer, Role::Viewer));
    assert!(!Rbac::has_role_min(&env, &viewer, Role::Submitter));
}

#[test]
fn test_rbac_non_admin_cannot_promote() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let rogue = Address::generate(&env);
    let target = Address::generate(&env);

    env.mock_all_auths();
    Rbac::init_admin(&env, &admin);

    let err = Rbac::set_role(&env, &rogue, &target, Role::Admin);
    assert_eq!(err, Err(RbacError::InsufficientRole));
}

#[test]
fn test_rbac_cannot_remove_sole_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    Rbac::init_admin(&env, &admin);

    let err = Rbac::set_role(&env, &admin, &admin, Role::Viewer);
    assert_eq!(err, Err(RbacError::CannotRevokeLastAdmin));
}

// ---------------------------------------------------------------------------
// ESG reporting RBAC matrix integration tests (#875)
// ---------------------------------------------------------------------------

/// Helper: set up an ESG ledger with an admin and return the admin address.
fn setup_esg(env: &Env) -> Address {
    let admin = Address::generate(env);
    env.mock_all_auths();
    Rbac::init_admin(env, &admin);
    EsgReporting::init(env, &admin);
    admin
}

/// Helper: assert that an audit event with the given topic/action was emitted.
fn assert_audit_event(env: &Env, action: Symbol) {
    let events = env.events().all();
    let mut found = false;
    for (_contract, topics, _data) in events.iter() {
        if let Some(first) = topics.get(0) {
            let topic: Symbol = first.into_val(env);
            if topic == action {
                found = true;
                break;
            }
        }
    }
    assert!(found, "expected audit event for action");
}

#[test]
fn test_esg_rbac_matrix_positive_roles() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let submitter = Address::generate(&env);
    let auditor = Address::generate(&env);

    Rbac::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();
    Rbac::set_role(&env, &admin, &auditor, Role::Auditor).unwrap();

    // Submitter may record emissions (state mutation).
    let res = EsgReporting::record_emissions(&env, &submitter, 1, 100, 0);
    assert!(res.is_ok(), "submitter should be allowed to record emissions");

    // Auditor may verify/attest but not mutate the ledger.
    let res = EsgReporting::verify_emissions(&env, &auditor, 1);
    assert!(res.is_ok(), "auditor should be allowed to verify emissions");

    // Admin retains full privileges.
    let res = EsgReporting::record_emissions(&env, &admin, 2, 50, 0);
    assert!(res.is_ok(), "admin should be allowed to record emissions");
}

#[test]
fn test_esg_rbac_matrix_negative_roles() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let viewer = Address::generate(&env);
    let rogue = Address::generate(&env);

    Rbac::set_role(&env, &admin, &viewer, Role::Viewer).unwrap();

    // Viewer lacks Submitter privileges: mutation must be rejected.
    let err = EsgReporting::record_emissions(&env, &viewer, 1, 100, 0);
    assert_eq!(err, Err(EsgError::InsufficientRole));

    // Unknown caller (no role at all) must be rejected.
    let err = EsgReporting::record_emissions(&env, &rogue, 1, 100, 0);
    assert_eq!(err, Err(EsgError::InsufficientRole));

    // Viewer cannot verify either (requires Auditor).
    let err = EsgReporting::verify_emissions(&env, &viewer, 1);
    assert_eq!(err, Err(EsgError::InsufficientRole));
}

#[test]
fn test_esg_role_hierarchy_precedence() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let auditor = Address::generate(&env);
    Rbac::set_role(&env, &admin, &auditor, Role::Auditor).unwrap();

    // Auditor outranks Submitter: inherits Submitter capabilities.
    assert!(Rbac::has_role_min(&env, &auditor, Role::Submitter));
    let res = EsgReporting::record_emissions(&env, &auditor, 1, 100, 0);
    assert!(res.is_ok(), "auditor should inherit submitter capability");

    // Admin outranks Auditor.
    assert!(Rbac::has_role_min(&env, &admin, Role::Auditor));
}

#[test]
fn test_esg_revocation_blocks_mutation() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let submitter = Address::generate(&env);
    Rbac::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();

    // Allowed while holding the role.
    assert!(EsgReporting::record_emissions(&env, &submitter, 1, 100, 0).is_ok());

    // Revoke the role.
    Rbac::set_role(&env, &admin, &submitter, Role::Viewer).unwrap();

    // Subsequent mutation must be rejected with a stable error code.
    let err = EsgReporting::record_emissions(&env, &submitter, 2, 100, 0);
    assert_eq!(err, Err(EsgError::InsufficientRole));
}

#[test]
fn test_esg_suspended_submitter_rejected() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let submitter = Address::generate(&env);
    Rbac::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();

    // Suspend the submitter DID.
    Rbac::suspend(&env, &admin, &submitter).unwrap();

    let err = EsgReporting::record_emissions(&env, &submitter, 1, 100, 0);
    assert_eq!(err, Err(EsgError::Suspended));

    // Reinstating restores access.
    Rbac::unsuspend(&env, &admin, &submitter).unwrap();
    assert!(EsgReporting::record_emissions(&env, &submitter, 1, 100, 0).is_ok());
}

#[test]
fn test_esg_audit_events_on_success_and_rejection() {
    let env = Env::default();
    let admin = setup_esg(&env);

    let submitter = Address::generate(&env);
    let rogue = Address::generate(&env);
    Rbac::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();

    // Successful mutation emits a structured audit event.
    EsgReporting::record_emissions(&env, &submitter, 1, 100, 0).unwrap();
    assert_audit_event(&env, Symbol::new(&env, "esg_record"));

    // Rejected access attempt also emits an audit event.
    let _ = EsgReporting::record_emissions(&env, &rogue, 1, 100, 0);
    assert_audit_event(&env, Symbol::new(&env, "esg_denied"));
}

// ---------------------------------------------------------------------------
// Supply chain mutating entrypoint RBAC matrix (#870)
// ---------------------------------------------------------------------------

fn sc_setup(env: &Env) -> (Address, Address, Address, Address) {
    let admin = Address::generate(env);
    let auditor = Address::generate(env);
    let submitter = Address::generate(env);
    let viewer = Address::generate(env);

    env.mock_all_auths();
    Rbac::init_admin(env, &admin);
    Rbac::set_role(env, &admin, &auditor, Role::Auditor).unwrap();
    Rbac::set_role(env, &admin, &submitter, Role::Submitter).unwrap();
    Rbac::set_role(env, &admin, &viewer, Role::Viewer).unwrap();

    (admin, auditor, submitter, viewer)
}

fn sc_did(env: &Env, tag: &str) -> String {
    String::from_str(env, tag)
}

#[test]
fn test_supply_chain_register_requires_submitter_role() {
    let env = Env::default();
    let (_admin, auditor, submitter, viewer) = sc_setup(&env);

    // Positive: Submitter (and higher) may register a custody record.
    let ok = SupplyChain::register_record(
        &env,
        &submitter,
        &sc_did(&env, "did:sc:submitter"),
        &sc_did(&env, "lot-1"),
    );
    assert!(ok.is_ok());

    // Auditor sits above Submitter in the privilege ladder -> allowed.
    let ok_auditor = SupplyChain::register_record(
        &env,
        &auditor,
        &sc_did(&env, "did:sc:auditor"),
        &sc_did(&env, "lot-2"),
    );
    assert!(ok_auditor.is_ok());

    // Negative: Viewer is below Submitter -> stable RBAC error code.
    let err = SupplyChain::register_record(
        &env,
        &viewer,
        &sc_did(&env, "did:sc:viewer"),
        &sc_did(&env, "lot-3"),
    );
    assert_eq!(err, Err(RbacError::InsufficientRole));

    // Negative: unregistered caller has no role at all.
    let stranger = Address::generate(&env);
    let err_stranger = SupplyChain::register_record(
        &env,
        &stranger,
        &sc_did(&env, "did:sc:stranger"),
        &sc_did(&env, "lot-4"),
    );
    assert_eq!(err_stranger, Err(RbacError::InsufficientRole));
}

#[test]
fn test_supply_chain_handoff_requires_submitter_role() {
    let env = Env::default();
    let (_admin, auditor, submitter, viewer) = sc_setup(&env);

    SupplyChain::register_record(
        &env,
        &submitter,
        &sc_did(&env, "did:sc:submitter"),
        &sc_did(&env, "lot-h1"),
    )
    .unwrap();

    // Positive: Submitter performs the custody handoff.
    let ok = SupplyChain::handoff(
        &env,
        &submitter,
        &sc_did(&env, "lot-h1"),
        &sc_did(&env, "did:sc:next"),
    );
    assert!(ok.is_ok());

    // Negative: Viewer cannot mutate custody state.
    let err = SupplyChain::handoff(
        &env,
        &viewer,
        &sc_did(&env, "lot-h1"),
        &sc_did(&env, "did:sc:next2"),
    );
    assert_eq!(err, Err(RbacError::InsufficientRole));

    // Auditor is above Submitter, so the handoff is permitted.
    let ok_auditor = SupplyChain::handoff(
        &env,
        &auditor,
        &sc_did(&env, "lot-h1"),
        &sc_did(&env, "did:sc:next3"),
    );
    assert!(ok_auditor.is_ok());
}

#[test]
fn test_supply_chain_suspended_submitter_is_blocked() {
    let env = Env::default();
    let (admin, _auditor, submitter, _viewer) = sc_setup(&env);

    SupplyChain::register_record(
        &env,
        &submitter,
        &sc_did(&env, "did:sc:submitter"),
        &sc_did(&env, "lot-s1"),
    )
    .unwrap();

    // Suspend the submitter DID; further mutations must be rejected.
    Rbac::suspend(&env, &admin, &submitter).unwrap();

    let err = SupplyChain::handoff(
        &env,
        &submitter,
        &sc_did(&env, "lot-s1"),
        &sc_did(&env, "did:sc:next"),
    );
    assert_eq!(err, Err(RbacError::Suspended));

    // Revocation behaves the same way.
    Rbac::set_role(&env, &admin, &submitter, Role::Viewer).unwrap();
    let err_revoked = SupplyChain::handoff(
        &env,
        &submitter,
        &sc_did(&env, "lot-s1"),
        &sc_did(&env, "did:sc:next"),
    );
    assert_eq!(err_revoked, Err(RbacError::InsufficientRole));
}

#[test]
fn test_supply_chain_emits_audit_events_on_success_and_rejection() {
    let env = Env::default();
    let (_admin, _auditor, submitter, viewer) = sc_setup(&env);

    // Successful mutation emits a structured audit event.
    SupplyChain::register_record(
        &env,
        &submitter,
        &sc_did(&env, "did:sc:submitter"),
        &sc_did(&env, "lot-e1"),
    )
    .unwrap();
    let success_events = env.events().all();
    assert!(!success_events.is_empty());

    // Rejected access attempt also emits an audit event.
    let before = env.events().all().len();
    let err = SupplyChain::register_record(
        &env,
        &viewer,
        &sc_did(&env, "did:sc:viewer"),
        &sc_did(&env, "lot-e2"),
    );
    assert_eq!(err, Err(RbacError::InsufficientRole));
    let after = env.events().all().len();
    assert!(after > before);
}

// ---------------------------------------------------------------------------
// CQRS RBAC matrix integration tests (#800)
// ---------------------------------------------------------------------------

fn cqrs_setup() -> (Env, Address, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);
    env.mock_all_auths();
    let mut owners = soroban_sdk::Vec::new(&env);
    owners.push_back(admin.clone());
    client.initialize(&owners, &256, &1024);
    (env, admin, contract_id, client)
}

fn cqrs_command(env: &Env, submitter: &Address, event_type: &str) -> LogEventCommand {
    LogEventCommand {
        submitter: submitter.clone(),
        event_type: Symbol::new(env, event_type),
        metadata: Bytes::new(env),
        category: None,
        sub_event_type: None,
        idempotency_key: None,
        force: true,
    }
}

fn assert_cqrs_audit(env: &Env, action: &str, expected_outcome: &str, expected_code: u32) {
    let events = env.events().all();
    let mut found = false;
    for (_contract, topics, data) in events.iter() {
        if topics.len() < 2 {
            continue;
        }
        let topic: Symbol = topics.get(0).unwrap().into_val(env);
        let operation: Symbol = topics.get(1).unwrap().into_val(env);
        if topic != Symbol::new(env, "cqrs_audit")
            || operation != Symbol::new(env, action)
        {
            continue;
        }
        let (_caller, _timestamp, digest, outcome, code):
            (Address, u64, soroban_sdk::BytesN<32>, Symbol, u32) = data.into_val(env);
        assert_eq!(digest.len(), 32);
        assert_eq!(outcome, Symbol::new(env, expected_outcome));
        assert_eq!(code, expected_code);
        found = true;
    }
    assert!(found, "expected structured CQRS audit event");
}

#[test]
fn test_cqrs_rbac_hierarchy_revocation_and_audit_matrix() {
    let (env, admin, _contract_id, client) = cqrs_setup();
    let submitter = Address::generate(&env);
    let auditor = Address::generate(&env);
    let viewer = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.set_role(&admin, &submitter, &Some(LedgerRole::Submitter));
    client.set_role(&admin, &auditor, &Some(LedgerRole::Auditor));
    client.set_role(&admin, &viewer, &Some(LedgerRole::Viewer));

    let accepted = client.cqrs_log_event(&cqrs_command(&env, &submitter, "submitter"));
    assert!(accepted.success);
    assert_cqrs_audit(&env, "log", "ok", 0);

    // Auditor outranks Submitter and inherits submission permission.
    let accepted = client.cqrs_log_event(&cqrs_command(&env, &auditor, "auditor"));
    assert!(accepted.success);

    let denied_viewer = client.cqrs_log_event(&cqrs_command(&env, &viewer, "viewer"));
    assert_eq!(
        denied_viewer.error_code,
        Some(RbacError::InsufficientRole as u32)
    );
    assert_cqrs_audit(
        &env,
        "log",
        "denied",
        RbacError::InsufficientRole as u32,
    );

    let denied_unknown = client.cqrs_log_event(&cqrs_command(&env, &stranger, "stranger"));
    assert_eq!(
        denied_unknown.error_code,
        Some(RbacError::UnauthorizedSubmitter as u32)
    );

    // Revocation is reflected in the shared contract and CQRS role stores.
    client.set_role(&admin, &submitter, &None);
    let revoked = client.cqrs_log_event(&cqrs_command(&env, &submitter, "revoked"));
    assert_eq!(
        revoked.error_code,
        Some(RbacError::UnauthorizedSubmitter as u32)
    );
}

#[test]
fn test_cqrs_suspended_submitter_is_rejected_with_audit() {
    let (env, admin, contract_id, client) = cqrs_setup();
    let submitter = Address::generate(&env);
    client.set_role(&admin, &submitter, &Some(LedgerRole::Submitter));
    env.as_contract(&contract_id, || {
        RbacManager::suspend_submitter(&env, &admin, &submitter).unwrap();
    });

    let denied = client.cqrs_log_event(&cqrs_command(&env, &submitter, "suspended"));
    assert_eq!(
        denied.error_code,
        Some(RbacError::SubmitterSuspended as u32)
    );
    assert_cqrs_audit(
        &env,
        "log",
        "denied",
        RbacError::SubmitterSuspended as u32,
    );
}
