#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Bytes, BytesN, Env, Symbol, Vec};
use proptest::prelude::*;

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &100, &4096);
    (env, owner, client)
}

fn assign_submitter(client: &AuditLedgerClient<'static>, owner: &Address, submitter: &Address) {
    client.set_role(owner, submitter, &Some(Role::Submitter));
}

#[test]
fn test_cqrs_log_event_command_appends_to_event_store_and_updates_projection() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assign_submitter(&client, &owner, &submitter);
    env.ledger().set_timestamp(1000);

    let cmd = LogEventCommand {
        submitter: submitter.clone(),
        event_type: Symbol::new(&env, "transfer"),
        metadata: Bytes::from_slice(&env, b"cqrs_meta_1"),
        category: Some(Symbol::new(&env, "finance")),
        sub_event_type: None,
        idempotency_key: None,
        force: false,
    };

    let result = client.cqrs_log_event(&cmd);
    assert!(result.success);
    assert_eq!(result.sequence_number, 1);
    assert!(result.event_id.is_some());
    let event_id = result.event_id.unwrap();

    // Query side: verify projection state
    let projection = client.cqrs_get_projection();
    assert_eq!(projection.total_events, 1);
    assert_eq!(projection.total_commands, 1);
    assert_eq!(projection.last_sequence, 1);
    assert_eq!(projection.last_updated, 1000);
    assert_eq!(projection.type_counts.len(), 1);
    assert_eq!(projection.type_counts.get(0).unwrap(), (Symbol::new(&env, "transfer"), 1));
    assert_eq!(projection.submitter_counts.len(), 1);
    assert_eq!(projection.submitter_counts.get(0).unwrap(), (submitter.clone(), 1));

    // Query side: verify EventView by event_id
    let view = client.cqrs_get_event_view(&event_id).unwrap();
    assert_eq!(view.event_id, event_id);
    assert_eq!(view.sequence, 1);
    assert_eq!(view.submitter, submitter);
    assert_eq!(view.event_type, Symbol::new(&env, "transfer"));
    assert_eq!(view.metadata, Bytes::from_slice(&env, b"cqrs_meta_1"));
    assert_eq!(view.version, 1);

    // Query side: verify EventView by sequence
    let view_by_seq = client.cqrs_get_event_by_sequence(&1).unwrap();
    assert_eq!(view_by_seq.event_id, event_id);
    assert_eq!(view_by_seq.sequence, 1);
}

#[test]
fn test_cqrs_query_separation_does_not_mutate_state() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assign_submitter(&client, &owner, &submitter);
    env.ledger().set_timestamp(1000);

    let cmd = LogEventCommand {
        submitter: submitter.clone(),
        event_type: Symbol::new(&env, "audit"),
        metadata: Bytes::from_slice(&env, b"audit_data"),
        category: None,
        sub_event_type: None,
        idempotency_key: None,
        force: false,
    };
    let result = client.cqrs_log_event(&cmd);
    let event_id = result.event_id.unwrap();

    // Perform multiple read queries
    let _ = client.cqrs_get_event_view(&event_id);
    let _ = client.cqrs_get_event_by_sequence(&1);
    let _ = client.cqrs_get_projection();
    let _ = client.cqrs_mat_type_count(&Symbol::new(&env, "audit"));

    // State must remain strictly unchanged
    let proj = client.cqrs_get_projection();
    assert_eq!(proj.total_commands, 1);
    assert_eq!(proj.last_sequence, 1);
}

#[test]
fn test_cqrs_command_idempotency() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assign_submitter(&client, &owner, &submitter);
    env.ledger().set_timestamp(1000);

    let idemp_key = BytesN::from_array(&env, &[42u8; 32]);
    let cmd = LogEventCommand {
        submitter: submitter.clone(),
        event_type: Symbol::new(&env, "payment"),
        metadata: Bytes::from_slice(&env, b"idemp_test"),
        category: None,
        sub_event_type: None,
        idempotency_key: Some(idemp_key.clone()),
        force: false,
    };

    let res1 = client.cqrs_log_event(&cmd);
    assert!(res1.success);
    assert_eq!(res1.sequence_number, 1);

    // Resend exact same command with identical idempotency key
    let res2 = client.cqrs_log_event(&cmd);
    assert_eq!(res1, res2);

    // Projection count must not increment
    let proj = client.cqrs_get_projection();
    assert_eq!(proj.total_events, 1);
    assert_eq!(proj.total_commands, 1);
}

#[test]
fn test_cqrs_update_event_command() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assign_submitter(&client, &owner, &submitter);
    env.ledger().set_timestamp(1000);

    let log_cmd = LogEventCommand {
        submitter,
        event_type: Symbol::new(&env, "report"),
        metadata: Bytes::from_slice(&env, b"initial_report"),
        category: None,
        sub_event_type: None,
        idempotency_key: None,
        force: false,
    };
    let log_res = client.cqrs_log_event(&log_cmd);
    let event_id = log_res.event_id.unwrap();

    // Owner sends UpdateEventCommand
    env.ledger().set_timestamp(2000);
    let update_cmd = UpdateEventCommand {
        caller: owner.clone(),
        event_id: event_id.clone(),
        new_metadata: Bytes::from_slice(&env, b"updated_report"),
        version_tag: Some(Symbol::new(&env, "v2")),
    };

    let update_res = client.cqrs_update_event(&update_cmd);
    assert!(update_res.success);
    assert_eq!(update_res.sequence_number, 2);
    let updated_event_id = update_res.event_id.unwrap();

    // Query side: projection has 2 total commands (1 log + 1 update)
    let proj = client.cqrs_get_projection();
    assert_eq!(proj.total_commands, 2);
    assert_eq!(proj.total_events, 1);
    assert_eq!(proj.last_sequence, 2);
    assert_eq!(proj.last_updated, 2000);

    // Query side: view reflects the updated event
    let view = client.cqrs_get_event_view(&updated_event_id).unwrap();
    assert_eq!(view.metadata, Bytes::from_slice(&env, b"updated_report"));
    assert_eq!(view.version, 2);
}

#[test]
fn test_cqrs_governance_command() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1500);

    let gov_cmd = GovernanceCommand {
        caller: owner.clone(),
        action: Symbol::new(&env, "set_paused"),
        target_address: None,
        param_u32: None,
        param_symbol: None,
        param_bool: Some(true),
    };

    let res = client.cqrs_governance(&gov_cmd);
    assert!(res.success);
    assert_eq!(res.sequence_number, 1);

    let proj = client.cqrs_get_projection();
    assert_eq!(proj.total_commands, 1);
    assert_eq!(proj.last_sequence, 1);

    // Verify governance command effect: paused state updated
    assert!(client.is_paused());
}

#[test]
fn test_cqrs_materialized_views_and_query_filtering() {
    let (env, owner, client) = create_ledger();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    assign_submitter(&client, &owner, &alice);
    assign_submitter(&client, &owner, &bob);
    env.ledger().set_timestamp(1000);

    // Log 2 events for Alice (type "security")
    for i in 0..2 {
        let cmd = LogEventCommand {
            submitter: alice.clone(),
            event_type: Symbol::new(&env, "security"),
            metadata: Bytes::from_slice(&env, &[i as u8]),
            category: None,
            sub_event_type: None,
            idempotency_key: None,
            force: true,
        };
        client.cqrs_log_event(&cmd);
    }

    // Log 1 event for Bob (type "network")
    let bob_cmd = LogEventCommand {
        submitter: bob.clone(),
        event_type: Symbol::new(&env, "network"),
        metadata: Bytes::from_slice(&env, b"bob_data"),
        category: None,
        sub_event_type: None,
        idempotency_key: None,
        force: true,
    };
    client.cqrs_log_event(&bob_cmd);

    // Check materialized type counts
    assert_eq!(client.cqrs_mat_type_count(&Symbol::new(&env, "security")), 2);
    assert_eq!(client.cqrs_mat_type_count(&Symbol::new(&env, "network")), 1);
    assert_eq!(client.cqrs_mat_type_count(&Symbol::new(&env, "unknown")), 0);

    // Check materialized type events pagination
    let sec_events = client.cqrs_mat_type_events(
        &Symbol::new(&env, "security"),
        &0,
        &10,
    );
    assert_eq!(sec_events.len(), 2);

    // Check materialized submitter events pagination
    let alice_seqs = client.cqrs_mat_sub_events(&alice, &0, &10);
    assert_eq!(alice_seqs.len(), 2);
    assert_eq!(alice_seqs.get(0).unwrap(), 1);
    assert_eq!(alice_seqs.get(1).unwrap(), 2);

    // Test EventQuery filtering by type
    let query_sec = EventQuery {
        event_type: Some(Symbol::new(&env, "security")),
        submitter: None,
        from_sequence: 1,
        to_sequence: 10,
        limit: 10,
    };
    let res_sec = client.cqrs_query_events(&query_sec);
    assert_eq!(res_sec.len(), 2);

    // Test EventQuery filtering by submitter
    let query_bob = EventQuery {
        event_type: None,
        submitter: Some(bob.clone()),
        from_sequence: 1,
        to_sequence: 10,
        limit: 10,
    };
    let res_bob = client.cqrs_query_events(&query_bob);
    assert_eq!(res_bob.len(), 1);
    assert_eq!(res_bob.get(0).unwrap().submitter, bob);
}

#[test]
fn test_cqrs_snapshot_and_projection_rebuilding() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    assign_submitter(&client, &owner, &submitter);
    env.ledger().set_timestamp(1000);

    // Log 3 events
    for i in 0..3 {
        let cmd = LogEventCommand {
            submitter: submitter.clone(),
            event_type: Symbol::new(&env, "metric"),
            metadata: Bytes::from_slice(&env, &[i as u8]),
            category: None,
            sub_event_type: None,
            idempotency_key: None,
            force: true,
        };
        client.cqrs_log_event(&cmd);
    }

    // Capture point-in-time snapshot
    let snapshot_id = client.cqrs_create_snapshot(&owner);
    assert_eq!(snapshot_id, 1);

    let snapshot = client.cqrs_get_snapshot(&snapshot_id).unwrap();
    assert_eq!(snapshot.snapshot_id, 1);
    assert_eq!(snapshot.sequence, 3);
    assert_eq!(snapshot.projection.total_events, 3);

    // Log 2 more events
    for i in 3..5 {
        let cmd = LogEventCommand {
            submitter: submitter.clone(),
            event_type: Symbol::new(&env, "metric"),
            metadata: Bytes::from_slice(&env, &[i as u8]),
            category: None,
            sub_event_type: None,
            idempotency_key: None,
            force: true,
        };
        client.cqrs_log_event(&cmd);
    }

    // Rebuild projection starting from the latest snapshot
    let rebuilt_from_snap = client.cqrs_rebuild_projection(&owner, &true);
    assert_eq!(rebuilt_from_snap.total_events, 5);
    assert_eq!(rebuilt_from_snap.total_commands, 5);
    assert_eq!(rebuilt_from_snap.last_sequence, 5);

    // Rebuild projection from genesis (replays entire event store)
    let rebuilt_from_genesis = client.cqrs_rebuild_projection(&owner, &false);
    assert_eq!(rebuilt_from_genesis.total_events, 5);
    assert_eq!(rebuilt_from_genesis.total_commands, 5);
    assert_eq!(rebuilt_from_genesis.last_sequence, 5);
}

#[test]
fn test_cqrs_rebuild_projection_from_primary_ledger_events() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.ledger().set_timestamp(1000);

    // Log 2 events using existing legacy API
    client.log_event(
        &submitter,
        &Symbol::new(&env, "legacy"),
        &Bytes::from_slice(&env, b"legacy_event_1"),
        &None,
        &None,
        &false,
    );
    client.log_event(
        &submitter,
        &Symbol::new(&env, "legacy"),
        &Bytes::from_slice(&env, b"legacy_event_2"),
        &None,
        &None,
        &false,
    );

    // Before rebuilding, CQRS sequence is 0
    assert_eq!(client.cqrs_get_projection().total_events, 0);

    // Rebuild projection: imports existing primary ledger events into CQRS store
    let rebuilt = client.cqrs_rebuild_projection(&owner, &false);
    assert_eq!(rebuilt.total_events, 2);
    assert_eq!(rebuilt.last_sequence, 2);

    // Query side: views now available for legacy events
    let view = client.cqrs_get_event_by_sequence(&1).unwrap();
    assert_eq!(view.event_type, Symbol::new(&env, "legacy"));
    assert_eq!(view.metadata, Bytes::from_slice(&env, b"legacy_event_1"));
}

proptest! {
    #[test]
    fn cqrs_sequence_allocation_never_wraps_or_exceeds_limit(current in any::<u32>()) {
        let expected = current
            .checked_add(1)
            .filter(|sequence| *sequence <= MAX_CQRS_EVENTS);
        prop_assert_eq!(CqrsEngine::checked_next_sequence(current), expected);
    }

    #[test]
    fn cqrs_metadata_length_is_bounded(length in any::<u32>()) {
        prop_assert_eq!(
            CqrsEngine::metadata_size_is_valid(length),
            length <= MAX_CQRS_METADATA_BYTES
        );
    }
}

#[test]
fn test_cqrs_exact_resource_boundaries() {
    assert_eq!(
        CqrsEngine::checked_next_sequence(MAX_CQRS_EVENTS - 1),
        Some(MAX_CQRS_EVENTS)
    );
    assert_eq!(CqrsEngine::checked_next_sequence(MAX_CQRS_EVENTS), None);
    assert_eq!(CqrsEngine::checked_next_sequence(u32::MAX), None);
    assert!(CqrsEngine::metadata_size_is_valid(MAX_CQRS_METADATA_BYTES));
    assert!(!CqrsEngine::metadata_size_is_valid(MAX_CQRS_METADATA_BYTES + 1));
}

#[test]
fn test_cqrs_extreme_timestamp_and_digest_inputs_are_deterministic() {
    let env = Env::default();
    let caller = Address::generate(&env);
    env.ledger().set_timestamp(u64::MAX);
    assert_eq!(env.ledger().timestamp(), u64::MAX);

    let operation = Symbol::new(&env, "extreme_diff");
    let digest = crate::rbac::RbacManager::state_diff_digest(
        &env,
        &caller,
        &operation,
        i128::MAX,
        i128::MIN,
    );
    let repeated = crate::rbac::RbacManager::state_diff_digest(
        &env,
        &caller,
        &operation,
        i128::MAX,
        i128::MIN,
    );
    assert_eq!(digest.len(), 32);
    assert_eq!(digest, repeated);
}
