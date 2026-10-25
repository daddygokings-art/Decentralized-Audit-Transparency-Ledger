use super::*;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Bytes, BytesN, Env, Vec};

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

// ── Chaos Engineering: Key Rotation ───────────────────────────────────────────

#[test]
fn chaos_key_rotation_old_key_denied() {
    let (env, owner, client) = create_ledger();
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    client.transfer_ownership(&owner, &new_owner);

    // Verify new owner can call governance
    let result = client.try_set_global_max_logs(&new_owner, &200);
    assert!(result.is_ok());

    // Verify old owner is now denied
    let result = client.try_set_global_max_logs(&owner, &300);
    assert!(result.is_err());
}

#[test]
fn chaos_key_rotation_events_preserved() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    client.transfer_ownership(&owner, &new_owner);

    // Events should still be readable after ownership transfer
    let evt = client.get_event(&id);
    assert_eq!(evt.submitter, submitter);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"tx1"));
}

#[test]
fn chaos_key_rotation_multiple_rotations() {
    let (env, owner1, client) = create_ledger();
    let owner2 = Address::generate(&env);
    let owner3 = Address::generate(&env);

    env.mock_all_auths();
    client.transfer_ownership(&owner1, &owner2);
    client.transfer_ownership(&owner2, &owner3);

    let result = client.try_set_global_max_logs(&owner1, &200);
    assert!(result.is_err());

    let result = client.try_set_global_max_logs(&owner2, &200);
    assert!(result.is_err());

    let result = client.try_set_global_max_logs(&owner3, &200);
    assert!(result.is_ok());
}

// ── Chaos Engineering: Permission Changes ─────────────────────────────────────

#[test]
fn chaos_permission_change_pause_blocks_writes() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.pause(&owner);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    client.unpause(&owner);

    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
}

#[test]
fn chaos_permission_change_governance_denied_while_paused() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    client.pause(&owner);

    let result = client.try_set_global_max_logs(&owner, &200);
    assert!(result.is_err());

    client.unpause(&owner);

    let result = client.try_set_global_max_logs(&owner, &200);
    assert!(result.is_ok());
}

#[test]
fn chaos_permission_change_submitter_blocklist() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    client.block_submitter(&owner, &submitter);

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    client.unblock_submitter(&owner, &submitter);

    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 2);
}

#[test]
fn chaos_permission_change_allowlist_mode() {
    let (env, owner, client) = create_ledger();
    let allowed = Address::generate(&env);
    let blocked = Address::generate(&env);

    env.mock_all_auths();
    client.enable_allowlist_mode(&owner);
    client.allow_submitter(&owner, &allowed);

    let result = client.try_log_event(
        &blocked,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    let id = client.log_event(
        &allowed,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 1);
}

// ── Chaos Engineering: Metadata Schema Changes ─────────────────────────────────

#[test]
fn chaos_metadata_schema_change_enforces_constraint() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_metadata_schema(
        &owner,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, &8u32.to_le_bytes()),
    );

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"short"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"long enough"),
        &None,
        &None,
        &false,
    );
}

// ── Chaos Engineering: Event Cap Changes ──────────────────────────────────────

#[test]
fn chaos_event_cap_change_removal() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &2);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &None, &None, &false);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &None, &None, &false);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"), &None, &None, &false);
    assert!(result.is_err());

    client.remove_event_cap(&owner, &payment);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"), &None, &None, &false);
    assert_eq!(client.event_count(&payment), 3);
}

// ── Chaos Engineering: TTL and Storage ────────────────────────────────────────

#[test]
fn chaos_ttl_configuration_change() {
    let (env, owner, client) = create_ledger();

    env.mock_all_auths();
    client.set_event_ttl(&owner, &100);

    assert_eq!(client.get_event_ttl(), 100);

    client.set_event_ttl(&owner, &0);
    assert_eq!(client.get_event_ttl(), 0);
}

// ── Chaos Engineering: Nonce Configuration Changes ────────────────────────────

#[test]
fn chaos_nonce_configuration_change() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_submitter_nonce_config(&owner, &submitter, &10, &1000);

    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &1,
    );
    assert_eq!(client.get_submitter_nonce(&submitter), 1);

    client.log_event_with_nonce(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx10"),
        &10,
    );
    assert_eq!(client.get_submitter_nonce(&submitter), 10);
}

// ── Chaos Engineering: Rate Limit Changes ─────────────────────────────────────

#[test]
fn chaos_rate_limit_change() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();
    client.set_submitter_rate_limit(&owner, &submitter, &1);

    env.ledger().set_timestamp(1000);
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());
}

// ── Chaos Engineering: Suptech Feeds ──────────────────────────────────────────

fn create_suptech_feeds() -> (Env, Address, SuptechFeedsClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(SuptechFeeds, ());
    let client = SuptechFeedsClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&admin);
    (env, admin, client)
}

#[test]
fn chaos_suptech_feeds_duplicate_submission_rejected() {
    let (env, admin, client) = create_suptech_feeds();
    let reporter = Address::generate(&env);

    env.mock_all_auths();
    let payload = Bytes::from_slice(&env, b"feed-1");

    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);

    // Duplicate submission with the same sequence must be rejected.
    let result = client.try_submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);
    assert!(result.is_err());

    // State remains consistent after the rejected duplicate.
    assert_eq!(client.feed_count(&reporter), 1);
}

#[test]
fn chaos_suptech_feeds_out_of_order_timestamps() {
    let (env, admin, client) = create_suptech_feeds();
    let reporter = Address::generate(&env);

    env.mock_all_auths();
    let payload = Bytes::from_slice(&env, b"feed");

    // Submit events out of timestamp order; the dispatcher must accept them
    // and preserve the hash chain regardless of arrival order.
    env.ledger().set_timestamp(3000);
    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &3);

    env.ledger().set_timestamp(1000);
    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);

    env.ledger().set_timestamp(2000);
    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &2);

    assert_eq!(client.feed_count(&reporter), 3);
    assert!(client.verify_hash_chain());
}

#[test]
fn chaos_suptech_feeds_mid_batch_rollback() {
    let (env, admin, client) = create_suptech_feeds();
    let reporter = Address::generate(&env);

    env.mock_all_auths();
    let payload = Bytes::from_slice(&env, b"feed");

    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);

    // A batch that fails partway through must not leave partial state.
    let result = client.try_submit_batch(
        &reporter,
        &Vec::from_array(
            &env,
            [
                (symbol_short!("trade"), payload.clone(), 2u64),
                (symbol_short!("trade"), payload.clone(), 2u64), // duplicate -> fails
            ],
        ),
    );
    assert!(result.is_err());

    // No partial writes from the rolled-back batch.
    assert_eq!(client.feed_count(&reporter), 1);
    assert!(client.verify_hash_chain());
}

#[test]
fn chaos_suptech_feeds_recovery_preserves_merkle_proof() {
    let (env, admin, client) = create_suptech_feeds();
    let reporter = Address::generate(&env);

    env.mock_all_auths();
    let payload = Bytes::from_slice(&env, b"feed");

    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);
    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &2);

    let root_before = client.merkle_root();

    // Simulate a failure and recovery cycle.
    client.pause(&admin);
    let result = client.try_submit_feed(&reporter, &symbol_short!("trade"), &payload, &3);
    assert!(result.is_err());
    client.unpause(&admin);

    // Merkle root and hash chain must be unchanged after recovery.
    assert_eq!(client.merkle_root(), root_before);
    assert!(client.verify_hash_chain());

    // The proof for an existing leaf still verifies after recovery.
    let proof = client.get_merkle_proof(&1);
    assert!(client.verify_merkle_proof(&1, &proof));
}

#[test]
fn chaos_suptech_feeds_recovery_after_rollback() {
    let (env, admin, client) = create_suptech_feeds();
    let reporter = Address::generate(&env);

    env.mock_all_auths();
    let payload = Bytes::from_slice(&env, b"feed");

    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &1);

    let result = client.try_submit_batch(
        &reporter,
        &Vec::from_array(
            &env,
            [
                (symbol_short!("trade"), payload.clone(), 2u64),
                (symbol_short!("trade"), payload.clone(), 2u64),
            ],
        ),
    );
    assert!(result.is_err());

    // Recovery: a valid submission after the rollback succeeds and the chain
    // remains intact.
    client.submit_feed(&reporter, &symbol_short!("trade"), &payload, &2);
    assert_eq!(client.feed_count(&reporter), 2);
    assert!(client.verify_hash_chain());
}

// ── Chaos Engineering: CBDC Offline ───────────────────────────────────────────

fn create_cbdc_offline() -> (Env, Address, CbdcOfflineClient<'static>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(CbdcOffline, ());
    let client = CbdcOfflineClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.initialize(&admin);
    (env, admin, client)
}

#[test]
fn chaos_cbdc_offline_duplicate_voucher_rejected() {
    let (env, admin, client) = create_cbdc_offline();
    let holder = Address::generate(&env);

    env.mock_all_auths();
    let voucher = BytesN::from_array(&env, &[7u8; 32]);

    client.redeem_voucher(&holder, &voucher, &100);

    // Re-submitting the same voucher is a double-spend and must be rejected.
    let result = client.try_redeem_voucher(&holder, &voucher, &100);
    assert!(result.is_err());

    assert_eq!(client.redeemed_count(), 1);
}

#[test]
fn chaos_cbdc_offline_out_of_order_timestamps() {
    let (env, admin, client) = create_cbdc_offline();
    let holder = Address::generate(&env);

    env.mock_all_auths();
    let v1 = BytesN::from_array(&env, &[1u8; 32]);
    let v2 = BytesN::from_array(&env, &[2u8; 32]);
    let v3 = BytesN::from_array(&env, &[3u8; 32]);

    // Vouchers arrive out of order; reconciliation must still converge.
    env.ledger().set_timestamp(3000);
    client.redeem_voucher(&holder, &v3, &100);

    env.ledger().set_timestamp(1000);
    client.redeem_voucher(&holder, &v1, &100);

    env.ledger().set_timestamp(2000);
    client.redeem_voucher(&holder, &v2, &100);

    assert_eq!(client.redeemed_count(), 3);
    assert!(client.verify_hash_chain());
}

#[test]
fn chaos_cbdc_offline_mid_batch_rollback() {
    let (env, admin, client) = create_cbdc_offline();
    let holder = Address::generate(&env);

    env.mock_all_auths();
    let v1 = BytesN::from_array(&env, &[1u8; 32]);
    let v2 = BytesN::from_array(&env, &[2u8; 32]);

    client.redeem_voucher(&holder, &v1, &100);

    // Batch contains a duplicate voucher -> must roll back entirely.
    let result = client.try_redeem_batch(
        &holder,
        &Vec::from_array(&env, [(v2.clone(), 100i128), (v2.clone(), 100i128)]),
    );
    assert!(result.is_err());

    assert_eq!(client.redeemed_count(), 1);
    assert!(client.verify_hash_chain());
}

#[test]
fn chaos_cbdc_offline_recovery_preserves_merkle_proof() {
    let (env, admin, client) = create_cbdc_offline();
    let holder = Address::generate(&env);

    env.mock_all_auths();
    let v1 = BytesN::from_array(&env, &[1u8; 32]);
    let v2 = BytesN::from_array(&env, &[2u8; 32]);

    client.redeem_voucher(&holder, &v1, &100);
    client.redeem_voucher(&holder, &v2, &100);

    let root_before = client.merkle_root();

    // Simulate a failure and recovery cycle.
    client.pause(&admin);
    let v3 = BytesN::from_array(&env, &[3u8; 32]);
    let result = client.try_redeem_voucher(&holder, &v3, &100);
    assert!(result.is_err());
    client.unpause(&admin);

    // Merkle root and hash chain must be unchanged after recovery.
    assert_eq!(client.merkle_root(), root_before);
    assert!(client.verify_hash_chain());

    let proof = client.get_merkle_proof(&v1);
    assert!(client.verify_merkle_proof(&v1, &proof));
}

#[test]
fn chaos_cbdc_offline_recovery_after_rollback() {
    let (env, admin, client) = create_cbdc_offline();
    let holder = Address::generate(&env);

    env.mock_all_auths();
    let v1 = BytesN::from_array(&env, &[1u8; 32]);
    let v2 = BytesN::from_array(&env, &[2u8; 32]);

    client.redeem_voucher(&holder, &v1, &100);

    let result = client.try_redeem_batch(
        &holder,
        &Vec::from_array(&env, [(v2.clone(), 100i128), (v2.clone(), 100i128)]),
    );
    assert!(result.is_err());

    // Recovery: a valid redemption after the rollback succeeds.
    client.redeem_voucher(&holder, &v2, &100);
    assert_eq!(client.redeemed_count(), 2);
    assert!(client.verify_hash_chain());
}

// ── Chaos Engineering: Recovery After Failure ─────────────────────────────────

#[test]
fn chaos_recovery_after_pause_unpause() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();

    // Log some events
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    // Pause
    client.pause(&owner);

    // Attempt to log during pause
    let result = client.try_log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert!(result.is_err());

    // Unpause
    client.unpause(&owner);

    // Recover and log again
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 2);
}

#[test]
fn chaos_recovery_after_key_rotation() {
    let (env, owner1, client) = create_ledger();
    let owner2 = Address::generate(&env);
    let submitter = Address::generate(&env);

    env.mock_all_auths();

    // Log event with old owner
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx1"),
        &None,
        &None,
        &false,
    );

    // Rotate key
    client.transfer_ownership(&owner1, &owner2);

    // Continue operating with new owner
    client.set_global_max_logs(&owner2, &200);
    client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"tx2"),
        &None,
        &None,
        &false,
    );

    assert_eq!(client.total_events(), 2);
}

#[test]
fn chaos_recovery_after_cap_removal() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    let payment = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &payment, &1);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx1"), &None, &None, &false);

    let result = client.try_log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &None, &None, &false);
    assert!(result.is_err());

    client.remove_event_cap(&owner, &payment);

    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx2"), &None, &None, &false);
    client.log_event(&submitter, &payment, &Bytes::from_slice(&env, b"tx3"), &None, &None, &false);
    assert_eq!(client.event_count(&payment), 3);
}

// ── Merkle Proof and Hash-Chain Integrity Verification (issue #876, #831, #830, #827) ──

fn hash_sorted_pair(env: &Env, a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let (left, right) = if a.to_array() <= b.to_array() {
        (a, b)
    } else {
        (b, a)
    };
    let mut concat = Bytes::new(env);
    concat.append(&left.clone().into());
    concat.append(&right.clone().into());
    env.crypto().sha256(&concat)
}

fn compute_event_merkle_root(env: &Env, leaves: &Vec<BytesN<32>>) -> BytesN<32> {
    if leaves.is_empty() {
        return BytesN::from_array(env, &[0u8; 32]);
    }
    let mut current_level = leaves.clone();
    while current_level.len() > 1 {
        let mut next_level = Vec::new(env);
        let len = current_level.len();
        let mut i = 0;
        while i < len {
            let left = current_level.get(i).unwrap();
            let right = if i + 1 < len {
                current_level.get(i + 1).unwrap()
            } else {
                left.clone()
            };
            next_level.push_back(hash_sorted_pair(env, &left, &right));
            i += 2;
        }
        current_level = next_level;
    }
    current_level.get(0).unwrap()
}

fn build_event_merkle_proof(env: &Env, leaves: &Vec<BytesN<32>>, index: u32) -> Vec<BytesN<32>> {
    let mut proof = Vec::new(env);
    if leaves.is_empty() || index >= leaves.len() {
        return proof;
    }
    let mut current_level = leaves.clone();
    let mut curr_idx = index;

    while current_level.len() > 1 {
        let len = current_level.len();
        let sibling_idx = if curr_idx % 2 == 0 {
            if curr_idx + 1 < len { curr_idx + 1 } else { curr_idx }
        } else {
            curr_idx - 1
        };
        proof.push_back(current_level.get(sibling_idx).unwrap());

        let mut next_level = Vec::new(env);
        let mut i = 0;
        while i < len {
            let left = current_level.get(i).unwrap();
            let right = if i + 1 < len {
                current_level.get(i + 1).unwrap()
            } else {
                left.clone()
            };
            next_level.push_back(hash_sorted_pair(env, &left, &right));
            i += 2;
        }
        current_level = next_level;
        curr_idx /= 2;
    }
    proof
}

fn verify_event_merkle_proof(
    env: &Env,
    leaf: &BytesN<32>,
    proof: &Vec<BytesN<32>>,
    expected_root: &BytesN<32>,
) -> bool {
    let mut current = leaf.clone();
    for sibling in proof.iter() {
        current = hash_sorted_pair(env, &current, &sibling);
    }
    &current == expected_root
}

#[test]
fn chaos_audit_registry_recovery_preserves_hash_chain_and_merkle_root() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();

    // 1. Log sequenced events across multiple ledgers
    env.ledger().set_timestamp(1000);
    client.log_event(
        &submitter,
        &symbol_short!("ev1"),
        &Bytes::from_slice(&env, b"payload-1"),
        &None,
        &None,
        &false,
    );
    env.ledger().set_timestamp(2000);
    client.log_event(
        &submitter,
        &symbol_short!("ev2"),
        &Bytes::from_slice(&env, b"payload-2"),
        &None,
        &None,
        &false,
    );
    env.ledger().set_timestamp(3000);
    client.log_event(
        &submitter,
        &symbol_short!("ev3"),
        &Bytes::from_slice(&env, b"payload-3"),
        &None,
        &None,
        &false,
    );
    env.ledger().set_timestamp(4000);
    client.log_event(
        &submitter,
        &symbol_short!("ev4"),
        &Bytes::from_slice(&env, b"payload-4"),
        &None,
        &None,
        &false,
    );

    assert_eq!(client.total_events(), 4);
    assert!(client.verify_integrity(), "hash-chain must be intact prior to recovery");

    // Extract leaf hashes and compute pre-recovery Merkle root
    let mut leaves = Vec::new(&env);
    for i in 0..4 {
        let evt = client.get_event_by_order(&i);
        leaves.push_back(evt.event_hash);
    }
    let pre_root = compute_event_merkle_root(&env, &leaves);

    // Verify initial Merkle proofs for all leaves
    for i in 0..4 {
        let leaf = leaves.get(i).unwrap();
        let proof = build_event_merkle_proof(&env, &leaves, i);
        assert!(verify_event_merkle_proof(&env, &leaf, &proof, &pre_root));
    }

    // 2. Simulate failure / pause event
    client.pause(&owner);
    let fail_result = client.try_log_event(
        &submitter,
        &symbol_short!("fail"),
        &Bytes::from_slice(&env, b"unauthorized-mid-pause"),
        &None,
        &None,
        &false,
    );
    assert!(fail_result.is_err(), "must reject event logs during pause");

    // 3. Simulated recovery cycle (unpause & re-verify state)
    client.unpause(&owner);

    // Re-verify hash-chain integrity
    assert!(client.verify_integrity(), "hash-chain must verify after simulated recovery");

    // Ensure links and order hashes remained continuous
    for i in 1..4 {
        let current_evt = client.get_event_by_order(&i);
        let prev_evt = client.get_event_by_order(&(i - 1));
        assert_eq!(
            current_evt.prev_hash, prev_evt.event_hash,
            "link integrity preserved"
        );
    }

    // Re-derive Merkle tree after recovery and ensure root match
    let mut recovered_leaves = Vec::new(&env);
    for i in 0..4 {
        let evt = client.get_event_by_order(&i);
        recovered_leaves.push_back(evt.event_hash);
    }
    let recovered_root = compute_event_merkle_root(&env, &recovered_leaves);
    assert_eq!(recovered_root, pre_root, "merkle root must remain identical post-recovery");

    // Re-verify Merkle proofs on recovered tree
    for i in 0..4 {
        let leaf = recovered_leaves.get(i).unwrap();
        let proof = build_event_merkle_proof(&env, &recovered_leaves, i);
        assert!(
            verify_event_merkle_proof(&env, &leaf, &proof, &recovered_root),
            "proof for leaf {i} must verify post-recovery"
        );
    }

    // 4. Log subsequent event post-recovery to guarantee chain continuity
    env.ledger().set_timestamp(5000);
    client.log_event(
        &submitter,
        &symbol_short!("ev5"),
        &Bytes::from_slice(&env, b"payload-5"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.total_events(), 5);
    assert!(client.verify_integrity(), "hash-chain must verify after appending post-recovery");
}

#[test]
fn chaos_audit_registry_merkle_proof_rejects_tampered_recovery() {
    let (env, _owner, client) = create_ledger();
    let submitter = Address::generate(&env);

    env.mock_all_auths();

    for i in 0..3 {
        client.log_event(
            &submitter,
            &symbol_short!("ev"),
            &Bytes::from_slice(&env, &[i as u8]),
            &None,
            &None,
            &false,
        );
    }

    let mut leaves = Vec::new(&env);
    for i in 0..3 {
        leaves.push_back(client.get_event_by_order(&i).event_hash);
    }

    let root = compute_event_merkle_root(&env, &leaves);
    let target_idx = 1u32;
    let target_leaf = leaves.get(target_idx).unwrap();
    let proof = build_event_merkle_proof(&env, &leaves, target_idx);

    // Valid proof verifies
    assert!(verify_event_merkle_proof(&env, &target_leaf, &proof, &root));

    // Tampered leaf hash must be rejected
    let mut tampered_bytes = target_leaf.to_array();
    tampered_bytes[0] ^= 0xff;
    let tampered_leaf = BytesN::from_array(&env, &tampered_bytes);
    assert!(
        !verify_event_merkle_proof(&env, &tampered_leaf, &proof, &root),
        "tampered leaf must be rejected"
    );

    // Tampered root must be rejected
    let mut tampered_root_bytes = root.to_array();
    tampered_root_bytes[0] ^= 0xff;
    let tampered_root = BytesN::from_array(&env, &tampered_root_bytes);
    assert!(
        !verify_event_merkle_proof(&env, &target_leaf, &proof, &tampered_root),
        "tampered root must be rejected"
    );

    // Tampered proof sibling must be rejected
    let mut tampered_proof = proof.clone();
    if !tampered_proof.is_empty() {
        let mut bad_sibling = tampered_proof.get(0).unwrap().to_array();
        bad_sibling[1] ^= 0xff;
        tampered_proof.set(0, BytesN::from_array(&env, &bad_sibling));
        assert!(
            !verify_event_merkle_proof(&env, &target_leaf, &tampered_proof, &root),
            "tampered proof sibling must be rejected"
        );
    }
}

#[test]
fn chaos_reputation_engine_recovery_and_overflow_bounds() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let submitter = Address::generate(&env);
    let reviewer = Address::generate(&env);

    env.mock_all_auths();

    // Initialize reputation system
    crate::reputation_system::ReputationSystem::initialize(env.clone(), owner.clone());

    // Record events with saturating bounds
    for _ in 0..5 {
        crate::reputation_system::ReputationSystem::update_event_count(
            env.clone(),
            owner.clone(),
            submitter.clone(),
        );
    }

    let initial_score = crate::reputation_system::ReputationSystem::get_reputation(env.clone(), submitter.clone());
    assert_eq!(initial_score.event_count, 5);

    // Submit peer review
    let review = crate::reputation_system::ReputationSystem::submit_review(
        env.clone(),
        reviewer.clone(),
        submitter.clone(),
        5,
        Bytes::from_slice(&env, b"positive audit review"),
        BytesN::from_array(&env, &[1u8; 32]),
    );
    assert_eq!(review.rating, 5);

    let updated_score = crate::reputation_system::ReputationSystem::get_reputation(env.clone(), submitter.clone());
    assert_eq!(updated_score.positive_reviews, 1);
    assert!(updated_score.score <= crate::reputation_system::MAX_REPUTATION_SCORE);

    // File dispute and resolve
    let dispute = crate::reputation_system::ReputationSystem::file_dispute(
        env.clone(),
        reviewer.clone(),
        submitter.clone(),
        Bytes::from_slice(&env, b"audit discrepancy"),
    );
    assert_eq!(dispute.status, Symbol::new(&env, "open"));

    crate::reputation_system::ReputationSystem::resolve_dispute(
        env.clone(),
        owner.clone(),
        dispute.id.clone(),
        true, // defendant won
        Bytes::from_slice(&env, b"cleared after verification"),
    );

    let resolved_dispute = crate::reputation_system::ReputationSystem::get_dispute(env.clone(), dispute.id);
    assert_eq!(resolved_dispute.status, Symbol::new(&env, "resolved"));

    // Explicit TTL extension
    crate::reputation_system::ReputationSystem::extend_reputation_ttl(env.clone());

    let final_score = crate::reputation_system::ReputationSystem::get_reputation(env.clone(), submitter);
    assert_eq!(final_score.disputes_won, 1);
    assert!(final_score.score <= crate::reputation_system::MAX_REPUTATION_SCORE);
}

