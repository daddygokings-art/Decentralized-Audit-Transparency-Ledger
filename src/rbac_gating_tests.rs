#![cfg(test)]
//! RBAC Gating and Precedence Unit Tests (#686 #689 #688 #687)
//!
//! Includes the positive/negative RBAC matrix integration tests for the
//! `esg_reporting` module (#875): role hierarchy precedence, revocation
//! behavior, rejection of unauthorized/suspended submitter DIDs with stable
//! error codes, and structured audit event emission for both successful
//! mutations and rejected access attempts.
//!
//! The `cqrs_rbac_matrix` module at the end of this file covers the same
//! properties for every state-mutating entrypoint of `src/cqrs.rs`.

use super::esg_reporting::*;
use super::rbac::*;
use super::supply_chain::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, IntoVal, String, Symbol};

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
// CQRS read/write projection engine RBAC matrix (src/cqrs.rs)
// ---------------------------------------------------------------------------

mod cqrs_rbac_matrix {
    use crate::cqrs::*;
    use crate::rbac::{Rbac, RbacError, Role};
    use crate::test_events::{count_topic, events_during};
    use crate::{AuditLedger, AuditLedgerClient};
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, Vec};

    const ALL_OPS: [CqrsOperation; 5] = [
        CqrsOperation::LogEvent,
        CqrsOperation::UpdateEvent,
        CqrsOperation::Governance,
        CqrsOperation::Snapshot,
        CqrsOperation::RebuildProjection,
    ];

    fn op_name(op: CqrsOperation) -> &'static str {
        match op {
            CqrsOperation::LogEvent => "log",
            CqrsOperation::UpdateEvent => "update",
            CqrsOperation::Governance => "governance",
            CqrsOperation::Snapshot => "snapshot",
            CqrsOperation::RebuildProjection => "rebuild",
        }
    }

    // ── Engine-level harness (direct `CqrsEngine` calls) ────────────────────

    struct Engine {
        env: Env,
        contract: Address,
        admin: Address,
    }

    impl Engine {
        fn new() -> Engine {
            let env = Env::default();
            env.mock_all_auths_allowing_non_root_auth();
            let contract = env.register(AuditLedger, ());
            let admin = Address::generate(&env);
            env.as_contract(&contract, || Rbac::init_admin(&env, &admin));
            Engine { env, contract, admin }
        }

        fn with_role(&self, role: Option<Role>) -> Address {
            let who = Address::generate(&self.env);
            if let Some(role) = role {
                self.env.as_contract(&self.contract, || {
                    Rbac::set_role(&self.env, &self.admin, &who, role).unwrap()
                });
            }
            who
        }

        fn authorize(&self, who: &Address, op: CqrsOperation) -> Result<CqrsAuthorization, RbacError> {
            self.env
                .as_contract(&self.contract, || CqrsEngine::authorize(&self.env, who, op))
        }

        fn log_cmd(&self, submitter: &Address) -> LogEventCommand {
            LogEventCommand {
                submitter: submitter.clone(),
                event_type: Symbol::new(&self.env, "matrix"),
                metadata: Bytes::from_slice(&self.env, b"m"),
                category: None,
                sub_event_type: None,
                idempotency_key: None,
                force: true,
            }
        }
    }

    #[test]
    fn test_cqrs_role_matrix_positive_and_negative() {
        let eng = Engine::new();
        let roles: [Option<Role>; 5] = [
            None,
            Some(Role::Viewer),
            Some(Role::Submitter),
            Some(Role::Auditor),
            Some(Role::Admin),
        ];

        for role in roles {
            let who = eng.with_role(role);
            for op in ALL_OPS {
                let res = eng.authorize(&who, op);
                match role {
                    None => assert_eq!(
                        res.err(),
                        Some(RbacError::UnauthorizedSubmitter),
                        "no-role caller must get 424 for {}",
                        op_name(op)
                    ),
                    Some(r) if r >= op.required_role() => assert!(
                        res.is_ok(),
                        "{:?} must be allowed to {}",
                        r,
                        op_name(op)
                    ),
                    Some(r) => assert_eq!(
                        res.err(),
                        Some(RbacError::InsufficientRole),
                        "{:?} must be denied {}",
                        r,
                        op_name(op)
                    ),
                }
            }
        }
    }

    #[test]
    fn test_cqrs_role_hierarchy_precedence() {
        // Higher roles inherit every lower-role capability.
        assert!(Role::Admin > Role::Auditor);
        assert!(Role::Auditor > Role::Submitter);
        assert!(Role::Submitter > Role::Viewer);
        assert_eq!(CqrsOperation::LogEvent.required_role(), Role::Submitter);
        assert_eq!(CqrsOperation::UpdateEvent.required_role(), Role::Auditor);
        for op in [CqrsOperation::Governance, CqrsOperation::Snapshot, CqrsOperation::RebuildProjection] {
            assert_eq!(op.required_role(), Role::Admin);
        }
    }

    #[test]
    fn test_cqrs_suspension_overrides_admin_role() {
        let eng = Engine::new();
        let other_admin = eng.with_role(Some(Role::Admin));
        eng.env.as_contract(&eng.contract, || {
            Rbac::suspend(&eng.env, &eng.admin, &other_admin).unwrap()
        });
        for op in ALL_OPS {
            assert_eq!(eng.authorize(&other_admin, op).err(), Some(RbacError::SubmitterSuspended));
        }
    }

    #[test]
    fn test_cqrs_authorization_cannot_be_escalated_across_operations() {
        let eng = Engine::new();
        let submitter = eng.with_role(Some(Role::Submitter));
        let auth = eng.authorize(&submitter, CqrsOperation::LogEvent).unwrap();

        let (res, ev) = events_during(&eng.env, || {
            eng.env
                .as_contract(&eng.contract, || CqrsEngine::create_snapshot(&eng.env, &auth))
        });
        assert_eq!(res, Err(RbacError::InsufficientRole));
        assert_eq!(count_topic(&ev, DENIED_TOPIC, "snapshot"), 1);

        let res = eng.env.as_contract(&eng.contract, || {
            CqrsEngine::record_governance_command(&eng.env, &auth, Symbol::new(&eng.env, "x"), 1)
        });
        assert_eq!(res, Err(RbacError::InsufficientRole));

        let res = eng.env.as_contract(&eng.contract, || {
            CqrsEngine::rebuild_projection(&eng.env, &auth, false)
        });
        assert_eq!(res, Err(RbacError::InsufficientRole));
        assert_eq!(
            eng.env.as_contract(&eng.contract, || CqrsEngine::get_projection(&eng.env).total_commands),
            0
        );
    }

    #[test]
    fn test_cqrs_submitter_cannot_log_on_behalf_of_another_did() {
        let eng = Engine::new();
        let alice = eng.with_role(Some(Role::Submitter));
        let bob = eng.with_role(Some(Role::Submitter));
        let auth = eng.authorize(&alice, CqrsOperation::LogEvent).unwrap();
        let id = BytesN::from_array(&eng.env, &[9; 32]);

        let res = eng.env.as_contract(&eng.contract, || {
            CqrsEngine::record_log_event(&eng.env, &auth, id.clone(), &eng.log_cmd(&bob), 1)
        });
        assert_eq!(res, Err(RbacError::UnauthorizedSubmitter));

        // An Admin may replay history on behalf of another submitter.
        let admin_auth = eng.authorize(&eng.admin, CqrsOperation::RebuildProjection).unwrap();
        let res = eng.env.as_contract(&eng.contract, || {
            CqrsEngine::record_log_event(&eng.env, &admin_auth, id.clone(), &eng.log_cmd(&bob), 1)
        });
        assert!(res.unwrap().success);
    }

    #[test]
    fn test_cqrs_state_diff_digest_is_deterministic_and_binding() {
        let eng = Engine::new();
        let a = Address::generate(&eng.env);
        let b = Address::generate(&eng.env);
        let id = BytesN::from_array(&eng.env, &[1; 32]);
        eng.env.ledger().set_timestamp(77);
        let d = |who: &Address, op: CqrsOperation, prev: u32, next: u32, subj: Option<&BytesN<32>>| {
            CqrsEngine::state_diff_digest(&eng.env, who, op, prev, next, subj)
        };

        let base = d(&a, CqrsOperation::LogEvent, 0, 1, Some(&id));
        assert_eq!(base, d(&a, CqrsOperation::LogEvent, 0, 1, Some(&id)));
        assert_ne!(base, d(&b, CqrsOperation::LogEvent, 0, 1, Some(&id)));
        assert_ne!(base, d(&a, CqrsOperation::UpdateEvent, 0, 1, Some(&id)));
        assert_ne!(base, d(&a, CqrsOperation::LogEvent, 0, 2, Some(&id)));
        assert_ne!(base, d(&a, CqrsOperation::LogEvent, 0, 1, None));
        eng.env.ledger().set_timestamp(78);
        assert_ne!(base, d(&a, CqrsOperation::LogEvent, 0, 1, Some(&id)));
    }

    // ── Contract-level harness (`AuditLedger` CQRS entrypoints) ─────────────

    fn ledger() -> (Env, Address, AuditLedgerClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuditLedger, ());
        let client = AuditLedgerClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let mut owners = Vec::new(&env);
        owners.push_back(owner.clone());
        client.initialize(&owners, &100, &4096);
        (env, owner, client)
    }

    fn log_cmd(env: &Env, submitter: &Address, tag: u8) -> LogEventCommand {
        LogEventCommand {
            submitter: submitter.clone(),
            event_type: Symbol::new(env, "rbac_cqrs"),
            metadata: Bytes::from_slice(env, &[tag]),
            category: None,
            sub_event_type: None,
            idempotency_key: None,
            force: true,
        }
    }

    #[test]
    fn test_cqrs_unregistered_submitter_rejected_with_audit_events() {
        let (env, _owner, client) = ledger();
        let rogue = Address::generate(&env);

        let (res, ev) = events_during(&env, || client.cqrs_log_event(&log_cmd(&env, &rogue, 1)));
        assert!(!res.success);
        assert_eq!(res.error_code, Some(RbacError::UnauthorizedSubmitter as u32));
        assert_eq!(count_topic(&ev, AUDIT_TOPIC, "log"), 1);
        assert_eq!(count_topic(&ev, DENIED_TOPIC, "log"), 1);

        // No state mutation reached the event store or projection.
        assert_eq!(client.cqrs_get_projection().total_commands, 0);
        assert!(client.cqrs_get_event_by_sequence(&1).is_none());
    }

    #[test]
    fn test_cqrs_successful_mutation_emits_audit_event() {
        let (env, owner, client) = ledger();
        let submitter = Address::generate(&env);
        client.cqrs_set_role(&owner, &submitter, &(Role::Submitter as u32));

        let (res, ev) = events_during(&env, || client.cqrs_log_event(&log_cmd(&env, &submitter, 1)));
        assert!(res.success);
        assert_eq!(count_topic(&ev, AUDIT_TOPIC, "log"), 1);
        assert_eq!(count_topic(&ev, DENIED_TOPIC, "log"), 0);

        let (_, ev) = events_during(&env, || client.cqrs_create_snapshot(&owner));
        assert_eq!(count_topic(&ev, AUDIT_TOPIC, "snapshot"), 1);

        let (_, ev) = events_during(&env, || client.cqrs_rebuild_projection(&owner, &false));
        assert_eq!(count_topic(&ev, AUDIT_TOPIC, "rebuild"), 1);
    }

    #[test]
    fn test_cqrs_viewer_rejected_and_hierarchy_allows_higher_roles() {
        let (env, owner, client) = ledger();
        let viewer = Address::generate(&env);
        let auditor = Address::generate(&env);
        client.cqrs_set_role(&owner, &viewer, &(Role::Viewer as u32));
        client.cqrs_set_role(&owner, &auditor, &(Role::Auditor as u32));

        let res = client.cqrs_log_event(&log_cmd(&env, &viewer, 1));
        assert_eq!(res.error_code, Some(RbacError::InsufficientRole as u32));

        // Auditor and Admin (the bootstrapped owner) inherit Submitter rights.
        assert!(client.cqrs_log_event(&log_cmd(&env, &auditor, 2)).success);
        assert!(client.cqrs_log_event(&log_cmd(&env, &owner, 3)).success);
    }

    #[test]
    fn test_cqrs_revocation_blocks_further_mutations() {
        let (env, owner, client) = ledger();
        let submitter = Address::generate(&env);
        client.cqrs_set_role(&owner, &submitter, &(Role::Submitter as u32));
        assert!(client.cqrs_log_event(&log_cmd(&env, &submitter, 1)).success);

        // Revoke write access by downgrading to Viewer.
        client.cqrs_set_role(&owner, &submitter, &(Role::Viewer as u32));
        let res = client.cqrs_log_event(&log_cmd(&env, &submitter, 2));
        assert_eq!(res.error_code, Some(RbacError::InsufficientRole as u32));
        assert_eq!(client.cqrs_get_projection().total_events, 1);
    }

    #[test]
    fn test_cqrs_suspended_submitter_rejected_until_reinstated() {
        let (env, owner, client) = ledger();
        let submitter = Address::generate(&env);
        client.cqrs_set_role(&owner, &submitter, &(Role::Submitter as u32));
        client.cqrs_suspend_submitter(&owner, &submitter);

        let (res, ev) = events_during(&env, || client.cqrs_log_event(&log_cmd(&env, &submitter, 1)));
        assert_eq!(res.error_code, Some(RbacError::SubmitterSuspended as u32));
        assert_eq!(count_topic(&ev, DENIED_TOPIC, "log"), 1);

        client.cqrs_reinstate_submitter(&owner, &submitter);
        assert!(client.cqrs_log_event(&log_cmd(&env, &submitter, 2)).success);
    }

    #[test]
    fn test_cqrs_admin_only_entrypoints_reject_lower_roles() {
        let (env, owner, client) = ledger();
        let auditor = Address::generate(&env);
        client.cqrs_set_role(&owner, &auditor, &(Role::Auditor as u32));

        let gov = GovernanceCommand {
            caller: auditor.clone(),
            action: Symbol::new(&env, "set_paused"),
            target_address: None,
            param_u32: None,
            param_symbol: None,
            param_bool: Some(true),
        };
        let (res, ev) = events_during(&env, || client.cqrs_governance(&gov));
        assert_eq!(res.error_code, Some(RbacError::InsufficientRole as u32));
        assert_eq!(count_topic(&ev, DENIED_TOPIC, "governance"), 1);
        assert!(!client.is_paused());

        assert!(client.try_cqrs_create_snapshot(&auditor).is_err());
        assert!(client.try_cqrs_rebuild_projection(&auditor, &false).is_err());
        assert!(client.cqrs_get_snapshot(&1).is_none());
    }

    #[test]
    fn test_cqrs_role_administration_requires_admin() {
        let (env, owner, client) = ledger();
        let submitter = Address::generate(&env);
        let target = Address::generate(&env);
        client.cqrs_set_role(&owner, &submitter, &(Role::Submitter as u32));

        // Non-admins cannot grant roles or suspend DIDs.
        assert!(client.try_cqrs_set_role(&submitter, &target, &(Role::Admin as u32)).is_err());
        assert!(client.try_cqrs_suspend_submitter(&submitter, &owner).is_err());
        // Unknown role levels are rejected.
        assert!(client.try_cqrs_set_role(&owner, &target, &9).is_err());
    }
}

