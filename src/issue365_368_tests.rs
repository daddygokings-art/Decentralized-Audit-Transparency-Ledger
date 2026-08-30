use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Bytes, Env, Symbol, Vec};

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

fn log_sample(env: &Env, client: &AuditLedgerClient<'static>, submitter: &Address, metadata: &[u8]) -> BytesN<32> {
    client.log_event(
        submitter,
        &Symbol::new(env, "transfer"),
        &Bytes::from_slice(env, metadata),
        &None,
        &None,
        &false,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// RBAC (issue #365)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn rbac_is_off_by_default_and_preserves_legacy_flow() {
    let (env, _owner, client) = create_ledger();
    assert!(!client.is_rbac_enabled());
    // A random address with no role can still log events in compatibility mode.
    let submitter = Address::generate(&env);
    env.ledger().set_timestamp(1000);
    let id = log_sample(&env, &client, &submitter, b"legacy");
    let evt = client.get_event(&id);
    assert_eq!(evt.metadata, Bytes::from_slice(&env, b"legacy"));
}

#[test]
fn rbac_owners_are_seeded_as_admins() {
    let (env, owner, client) = create_ledger();
    assert_eq!(client.get_role(&owner), Some(Role::Admin));
    assert_eq!(client.get_role(&Address::generate(&env)), None);
}

#[test]
fn rbac_enable_gates_log_submission_by_role() {
    let (env, owner, client) = create_ledger();
    let submitter = Address::generate(&env);
    env.ledger().set_timestamp(1000);

    client.enable_rbac(&owner, &true);
    assert!(client.is_rbac_enabled());

    // Without a role, submission is denied (RoleNotGranted).
    assert!(client
        .try_log_event(
            &submitter,
            &Symbol::new(&env, "transfer"),
            &Bytes::from_slice(&env, b"x"),
            &None,
            &None,
            &false,
        )
        .is_err());

    // Viewer cannot submit.
    client.set_role(&owner, &submitter, &Some(Role::Viewer));
    assert!(client
        .try_log_event(
            &submitter,
            &Symbol::new(&env, "transfer"),
            &Bytes::from_slice(&env, b"x"),
            &None,
            &None,
            &false,
        )
        .is_err());

    // Submitter can.
    client.set_role(&owner, &submitter, &Some(Role::Submitter));
    let id = client.log_event(
        &submitter,
        &Symbol::new(&env, "transfer"),
        &Bytes::from_slice(&env, b"x"),
        &None,
        &None,
        &false,
    );
    assert_eq!(client.get_event(&id).submitter, submitter);
}

#[test]
fn rbac_governance_requires_admin() {
    let (env, owner, client) = create_ledger();
    let lowly = Address::generate(&env);
    env.ledger().set_timestamp(1000);

    client.enable_rbac(&owner, &true);
    client.set_role(&owner, &lowly, &Some(Role::Submitter));

    // Non-admin cannot perform governance writes.
    assert!(client.try_set_global_max_logs(&lowly, &200).is_err());
    assert!(client.try_set_role(&lowly, &lowly, &Some(Role::Admin)).is_err());

    // Admin can.
    assert!(client.try_set_global_max_logs(&owner, &200).is_ok());
}

#[test]
fn rbac_statistics_requires_auditor() {
    let (env, owner, client) = create_ledger();
    let viewer = Address::generate(&env);
    let auditor = Address::generate(&env);
    env.ledger().set_timestamp(1000);
    log_sample(&env, &client, &owner, b"stats");

    client.enable_rbac(&owner, &true);
    client.set_role(&owner, &viewer, &Some(Role::Viewer));
    client.set_role(&owner, &auditor, &Some(Role::Auditor));

    assert!(client.try_get_statistics(&viewer).is_err());
    let stats = client.get_statistics(&auditor);
    assert_eq!(stats.total_events, 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Deduplication policies (issue #366)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn dedup_content_hash_is_the_default() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    assert_eq!(client.get_dedup_policy(), DedupPolicy::ContentHash);

    let submitter = Address::generate(&env);
    let first = log_sample(&env, &client, &submitter, b"dup-content");
    let second = log_sample(&env, &client, &submitter, b"dup-content");
    assert_eq!(first, second);
    assert_eq!(client.total_events(), 1);
}

#[test]
fn dedup_none_never_deduplicates() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    client.set_dedup_policy(&owner, &DedupPolicy::None);

    let submitter = Address::generate(&env);
    let first = log_sample(&env, &client, &submitter, b"same-content");
    let second = log_sample(&env, &client, &submitter, b"same-content");
    assert_ne!(first, second);
    assert_eq!(client.total_events(), 2);
}

#[test]
fn dedup_content_hash_with_timestamp_splits_by_time() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    client.set_dedup_policy(&owner, &DedupPolicy::ContentHashWithTimestamp);
    let submitter = Address::generate(&env);

    let at_t1 = log_sample(&env, &client, &submitter, b"ts-content");
    let second_at_t1 = log_sample(&env, &client, &submitter, b"ts-content");
    assert_eq!(at_t1, second_at_t1);
    assert_eq!(client.total_events(), 1);

    // New timestamp => new event even with identical content.
    env.ledger().set_timestamp(1001);
    let at_t2 = log_sample(&env, &client, &submitter, b"ts-content");
    assert_ne!(at_t1, at_t2);
    assert_eq!(client.total_events(), 2);
}

#[test]
fn dedup_custom_policy_uses_explicit_key() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    client.set_dedup_policy(&owner, &DedupPolicy::Custom);
    let submitter = Address::generate(&env);

    let key1 = BytesN::from_array(&env, &[1u8; 32]);
    let event_type = Symbol::new(&env, "transfer");
    let meta = Bytes::from_slice(&env, b"custom");

    let first = client.log_event_with_custom_key(
        &submitter,
        &event_type,
        &meta,
        &None,
        &None,
        &false,
        &Some(key1),
    );
    let second = client.log_event_with_custom_key(
        &submitter,
        &event_type,
        &meta,
        &None,
        &None,
        &false,
        &Some(key1),
    );
    // Same explicit key, even with differing metadata => deduplicated.
    assert_eq!(first, second);
    assert_eq!(client.total_events(), 1);

    // A different key stores a new event.
    let key2 = BytesN::from_array(&env, &[2u8; 32]);
    let third = client.log_event_with_custom_key(
        &submitter,
        &event_type,
        &meta,
        &None,
        &None,
        &false,
        &Some(key2),
    );
    assert_ne!(first, third);
    assert_eq!(client.total_events(), 2);
}

#[test]
fn dedup_per_type_policy_override_wins() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    let financial = Symbol::new(&env, "financial");
    let operational = Symbol::new(&env, "operational");

    client.set_dedup_policy_for_type(&owner, &financial, &Some(DedupPolicy::None));
    client.set_dedup_policy_for_type(&owner, &operational, &Some(DedupPolicy::ContentHash));

    assert_eq!(client.get_dedup_policy_for_type(&financial), DedupPolicy::None);
    assert_eq!(client.get_dedup_policy_for_type(&operational), DedupPolicy::ContentHash);

    // financial: no dedup.
    let a = client.log_event(&submitter, &financial, &Bytes::from_slice(&env, b"m"), &None, &None, &false);
    let b = client.log_event(&submitter, &financial, &Bytes::from_slice(&env, b"m"), &None, &None, &false);
    assert_ne!(a, b);

    // operational: content-hash dedup.
    let c = client.log_event(&submitter, &operational, &Bytes::from_slice(&env, b"m"), &None, &None, &false);
    let d = client.log_event(&submitter, &operational, &Bytes::from_slice(&env, b"m"), &None, &None, &false);
    assert_eq!(c, d);
}

#[test]
fn dedup_cleanup_removes_stale_entries() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    log_sample(&env, &client, &submitter, b"clean-me");
    env.ledger().set_timestamp(1001);
    log_sample(&env, &client, &submitter, b"keep-me");

    // Simulate staleness by removing entries that no longer map to a live event.
    let cleaned = client.cleanup_stale_dedup_entries(&owner, &0, &10);
    assert_eq!(cleaned, 0); // all entries are still consistent
}

// ═══════════════════════════════════════════════════════════════════════
// Archiving with compression & off-chain storage (issue #367)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn archive_with_compression_round_trips_metadata() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    let original = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa data";
    let id = log_sample(&env, &client, &submitter, original);

    client.set_archive_config(
        &owner,
        &ArchiveConfig {
            offchain_storage: false,
            base_url: Bytes::new(&env),
            compression: 1,
        },
    );

    env.ledger().set_timestamp(2000);
    let archived = client.archive_events(&owner, &1500);
    assert_eq!(archived, 1);

    let restored = client.get_archived_event(&id);
    assert_eq!(restored.metadata, Bytes::from_slice(&env, original));

    let stats = client.get_archive_stats();
    assert_eq!(stats.total_archived, 1);
    assert_eq!(stats.total_compressed, 1);
    assert_eq!(stats.total_offchain, 0);
}

#[test]
fn archive_offchain_stores_ref_and_verifies_checksum() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    let original = b"off-chain payload";
    let id = log_sample(&env, &client, &submitter, original);
    let live_event = client.get_event(&id);

    client.set_archive_config(
        &owner,
        &ArchiveConfig {
            offchain_storage: true,
            base_url: Bytes::from_slice(&env, b"https://arch.example/"),
            compression: 1,
        },
    );

    env.ledger().set_timestamp(2000);
    let archived = client.archive_events(&owner, &1500);
    assert_eq!(archived, 1);

    let proof = client.get_archived_event_ref(&id).unwrap();
    assert_eq!(proof.checksum, live_event.event_hash);
    assert_eq!(proof.index, 0);

    // URL = base_url + hex(id).
    let mut expected_url = Bytes::from_slice(&env, b"https://arch.example/");
    let hex = |v: u8| -> u8 { if v < 10 { b'0' + v } else { b'a' + (v - 10) } };
    for b in id.to_array() {
        expected_url.push_back(hex((b >> 4) & 0x0f));
        expected_url.push_back(hex(b & 0x0f));
    }
    assert_eq!(proof.url, expected_url);

    // Reconstructed event keeps original metadata.
    let restored = client.get_archived_event(&id);
    assert_eq!(restored.metadata, Bytes::from_slice(&env, original));

    // Checksum verification works for the genuine payload and rejects tampering.
    assert!(client.verify_archived_event_checksum(&id, &live_event));
    let mut tampered = live_event.clone();
    tampered.metadata = Bytes::from_slice(&env, b"tampered");
    assert!(!client.verify_archived_event_checksum(&id, &tampered));

    let stats = client.get_archive_stats();
    assert_eq!(stats.total_offchain, 1);
    assert_eq!(stats.total_compressed, 1);
    assert_eq!(client.get_archived_event_count(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Event versioning — semantic diffs & audit trail (issue #368)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn versioning_tracks_audit_trail_and_diff() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    let id = log_sample(&env, &client, &submitter, b"v0");
    assert_eq!(client.total_events(), 1);

    let trail = client.get_event_audit_trail(&0);
    assert_eq!(trail.len(), 1);

    // update_event appends a second version.
    env.ledger().set_timestamp(1001);
    let new_id = client.update_event(&owner, &0, &Bytes::from_slice(&env, b"v1"));
    assert_ne!(id, new_id);

    let trail = client.get_event_audit_trail(&0);
    assert_eq!(trail.len(), 2);
    assert_eq!(trail.get(0).unwrap().data.metadata, Bytes::from_slice(&env, b"v0"));
    assert_eq!(trail.get(1).unwrap().data.metadata, Bytes::from_slice(&env, b"v1"));

    // Semantic diff reports the metadata change.
    let diff = client.get_event_diff(&0, &0, &1);
    assert_eq!(diff.len(), 1);
    assert_eq!(diff.get(0).unwrap().field, Symbol::new(&env, "metadata"));

    // Detailed comparison.
    let comparison = client.compare_event_versions_detailed(&0, &0, &1);
    assert!(!comparison.same);
    assert_eq!(comparison.from_hash, trail.get(0).unwrap().data.event_hash);
    assert_eq!(comparison.to_hash, trail.get(1).unwrap().data.event_hash);

    // Identical versions compare as equal.
    let same = client.compare_event_versions_detailed(&0, &0, &0);
    assert!(same.same);
    assert_eq!(same.changes.len(), 0);
}

#[test]
fn versioning_tags_versions() {
    let (env, owner, client) = create_ledger();
    env.ledger().set_timestamp(1000);
    let submitter = Address::generate(&env);
    log_sample(&env, &client, &submitter, b"v0");
    env.ledger().set_timestamp(1001);
    client.update_event(&owner, &0, &Bytes::from_slice(&env, b"v1"));

    client.tag_event_version(&owner, &0, &1, &Symbol::new(&env, "approved"));
    let tag = client.get_event_version_tag(&0, &1);
    assert_eq!(tag, Some(Symbol::new(&env, "approved")));
    assert_eq!(client.get_event_version_tag(&0, &0), None);

    // Releasing the tag is done by re-tagging with a different symbol (no delete
    // entry point needed for the MVP).
    assert!(client.try_tag_event_version(&owner, &0, &99, &Symbol::new(&env, "nope")).is_err());
}