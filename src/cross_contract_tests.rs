use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, Env, Symbol};

// ── Mock Caller Contract for Cross-Contract Tests ─────────────────────────────────

#[contract]
pub struct CallerContract;

#[contractimpl]
impl CallerContract {
    /// Calls AuditLedger's log_event on behalf of a user
    pub fn log_on_behalf(
        env: Env,
        audit_ledger: Address,
        user: Address,
        event_type: Symbol,
        metadata: Bytes,
    ) {
        // The caller contract invokes log_event with the user as submitter
        // The user must authorize this call
        let client = AuditLedgerClient::new(&env, &audit_ledger);
        client.log_event(&user, &event_type, &metadata);
    }

    /// Calls AuditLedger's governance functions
    pub fn try_governance_call(env: Env, audit_ledger: Address, caller: Address) {
        let client = AuditLedgerClient::new(&env, &audit_ledger);
        // This should fail if caller is not the owner
        client.set_global_max_logs(&caller, &200);
    }

    /// Attempts to trigger GlobalMaxLogsReached error
    pub fn trigger_global_max_error(env: Env, audit_ledger: Address, submitter: Address) {
        let client = AuditLedgerClient::new(&env, &audit_ledger);
        // Try to log when at capacity
        client.log_event(&submitter, &symbol_short!("test"), &Bytes::new(&env));
    }

    /// Attempts reentrancy by calling back into AuditLedger
    pub fn attempt_reentrancy(
        env: Env,
        audit_ledger: Address,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
    ) {
        let client = AuditLedgerClient::new(&env, &audit_ledger);
        // First call
        client.log_event(&submitter, &event_type, &metadata);
        
        // Attempt second call (reentrancy)
        // In a real scenario, this would be called from within a callback
        // For testing, we just make a second call
        client.log_event(&submitter, &event_type, &metadata);
    }
}

fn create_ledger() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&owner, &100);
    (env, owner, client)
}

// ── Cross-Contract Logging Tests ───────────────────────────────────────────────────

#[test]
fn cross_contract_log_event_with_correct_submitter() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let user = Address::generate(&env);
    
    // Register both contracts
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Caller contract logs on behalf of user
    // The user must authorize
    caller_client.log_on_behalf(
        &audit_ledger_id,
        &user,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx-data"),
    );

    // Verify the event was logged
    assert_eq!(audit_client.total_events(), 1);
    
    // Verify the submitter is the user, not the caller contract
    let evt = audit_client.get_event_by_order(&0);
    assert_eq!(evt.submitter, user);
    assert_ne!(evt.submitter, caller_contract_id);
}

#[test]
fn cross_contract_multiple_logs_different_users() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Log for user1
    caller_client.log_on_behalf(
        &audit_ledger_id,
        &user1,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"user1-tx"),
    );

    // Log for user2
    caller_client.log_on_behalf(
        &audit_ledger_id,
        &user2,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"user2-tx"),
    );

    assert_eq!(audit_client.total_events(), 2);
    
    let evt0 = audit_client.get_event_by_order(&0);
    let evt1 = audit_client.get_event_by_order(&1);
    
    assert_eq!(evt0.submitter, user1);
    assert_eq!(evt1.submitter, user2);
}

// ── Authorization Across Contracts Tests ───────────────────────────────────────────

#[test]
fn cross_contract_governance_requires_owner() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Try to call governance from caller contract with non-owner
    let result = caller_client.try_try_governance_call(&audit_ledger_id, &attacker);
    
    // Should fail because attacker is not owner
    assert!(result.is_err());
}

#[test]
fn cross_contract_governance_with_owner_succeeds() {
    let env = Env::default();
    let owner = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Call governance from caller contract with owner
    caller_client.try_governance_call(&audit_ledger_id, &owner);
    
    // Should succeed (no panic)
}

#[test]
fn cross_contract_transfer_ownership_across_contracts() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Transfer ownership
    audit_client.transfer_ownership(&owner, &new_owner);

    // New owner can govern
    audit_client.set_global_max_logs(&new_owner, &200);
}

// ── Error Propagation Tests ───────────────────────────────────────────────────────

#[test]
fn cross_contract_global_max_logs_error_propagates() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &1); // Max 1 event

    // Log one event to reach capacity
    audit_client.log_event(&submitter, &symbol_short!("test"), &Bytes::from_slice(&env, b"first"));

    // Try to log another event via caller contract
    let result = caller_client.try_trigger_global_max_error(&audit_ledger_id, &submitter);
    
    // Error should propagate
    assert!(result.is_err());
}

#[test]
fn cross_contract_event_type_max_logs_error_propagates() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);
    audit_client.set_event_max_logs(&owner, &symbol_short!("payment"), &1);

    // Log one event of type payment
    audit_client.log_event(&submitter, &symbol_short!("payment"), &Bytes::from_slice(&env, b"first"));

    // Try to log another payment event via caller contract
    let result = caller_client.try_log_on_behalf(
        &audit_ledger_id,
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"second"),
    );
    
    // Error should propagate
    assert!(result.is_err());
}

#[test]
fn cross_contract_metadata_too_large_error_propagates() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);
    audit_client.set_metadata_max_size(&owner, &10);

    // Try to log event with oversized metadata via caller contract
    let result = caller_client.try_log_on_behalf(
        &audit_ledger_id,
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, &[0u8; 11]),
    );
    
    // Error should propagate
    assert!(result.is_err());
}

#[test]
fn cross_contract_contract_paused_error_propagates() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);
    audit_client.pause(&owner);

    // Try to log event while paused via caller contract
    let result = caller_client.try_log_on_behalf(
        &audit_ledger_id,
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );
    
    // Error should propagate
    assert!(result.is_err());
}

// ── Reentrancy Tests ───────────────────────────────────────────────────────────────

#[test]
fn cross_contract_sequential_calls_work() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Make sequential calls (not true reentrancy, but multiple calls)
    caller_client.attempt_reentrancy(
        &audit_ledger_id,
        &submitter,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );

    // Both events should be logged
    assert_eq!(audit_client.total_events(), 2);
}

#[test]
fn cross_contract_caller_cannot_impersonate_user() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let user = Address::generate(&env);
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_ledger_id = env.register(AuditLedger, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Try to log without user authorization
    // This should fail because the user hasn't authorized
    let result = caller_client.try_log_on_behalf(
        &audit_ledger_id,
        &user,
        &symbol_short!("test"),
        &Bytes::from_slice(&env, b"data"),
    );
    
    // Should fail without user auth
    assert!(result.is_err());
}

#[test]
fn cross_contract_event_integrity_preserved() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let user = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller_contract_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller_client = CallerContractClient::new(&env, &caller_contract_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Log event via caller contract
    caller_client.log_on_behalf(
        &audit_ledger_id,
        &user,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx-data"),
    );

    // Verify hash chain integrity
    assert!(audit_client.verify_integrity());
}

#[test]
fn cross_contract_multiple_caller_contracts() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let user = Address::generate(&env);
    
    let audit_ledger_id = env.register(AuditLedger, ());
    let caller1_id = env.register(CallerContract, ());
    let caller2_id = env.register(CallerContract, ());
    
    let audit_client = AuditLedgerClient::new(&env, &audit_ledger_id);
    let caller1_client = CallerContractClient::new(&env, &caller1_id);
    let caller2_client = CallerContractClient::new(&env, &caller2_id);

    env.mock_all_auths();
    audit_client.initialize(&owner, &100);

    // Log via caller1
    caller1_client.log_on_behalf(
        &audit_ledger_id,
        &user,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"via-caller1"),
    );

    // Log via caller2
    caller2_client.log_on_behalf(
        &audit_ledger_id,
        &user,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"via-caller2"),
    );

    assert_eq!(audit_client.total_events(), 2);
    
    let evt0 = audit_client.get_event_by_order(&0);
    let evt1 = audit_client.get_event_by_order(&1);
    
    assert_eq!(evt0.submitter, user);
    assert_eq!(evt1.submitter, user);
    assert_eq!(evt0.metadata, Bytes::from_slice(&env, b"via-caller1"));
    assert_eq!(evt1.metadata, Bytes::from_slice(&env, b"via-caller2"));
}
