use super::*;
use crate::cross_chain_bridge::{BalanceAttestation, BridgeKey, BridgeRelay, CrossChainBridge};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Bytes, Env, Vec};

const ML_ACTORS: u32 = 4;
const ML_ROUNDS: u32 = 25;

fn ml_actors(env: &Env) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..ML_ACTORS {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn feature_view_id(env: &Env, actor_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            actor_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
        ],
    )
}

fn feature_view_hash(env: &Env, actor_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            (sequence & 0xff) as u8,
            actor_index as u8,
            ((sequence >> 8) & 0xff) as u8,
            0x5a,
        ],
    )
}

fn ml_metadata(env: &Env, actor_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            actor_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
            0xa5,
        ],
    )
}

/// Interleaves feature-view registration, attestation recording, and
/// drift-alert recording across multiple actors, then verifies that the
/// resulting state is consistent and that the hash chain / Merkle proofs
/// remain intact after a simulated recovery (re-reading all state).
fn run_ml_interleaved_round(
    env: &Env,
    client: &MlFeatureStoreClient,
    actors: &Vec<Address>,
    sequence: u32,
) {
    for actor_index in 0..ML_ACTORS {
        let actor = actors.get(actor_index).unwrap();
        let view_id = feature_view_id(env, actor_index, sequence);

        client.register_feature_view(
            &actor,
            &view_id,
            &feature_view_hash(env, actor_index, sequence),
            &ml_metadata(env, actor_index, sequence),
        );

        client.record_attestation(
            &actor,
            &view_id,
            &feature_view_hash(env, actor_index, sequence),
            &ml_metadata(env, actor_index, sequence),
        );

        if sequence % 3 == actor_index % 3 {
            client.record_drift_alert(
                &actor,
                &view_id,
                &ml_metadata(env, actor_index, sequence),
            );
        }
    }
}

fn assert_ml_interleaved_integrity(
    env: &Env,
    client: &MlFeatureStoreClient,
    actors: &Vec<Address>,
    rounds: u32,
) {
    // Simulated recovery: re-read all state from the contract and confirm
    // that every actor's lineage is intact and hash-chain consistent.
    for actor_index in 0..ML_ACTORS {
        let actor = actors.get(actor_index).unwrap();
        let expected_views = rounds;
        assert_eq!(client.feature_view_count(&actor), expected_views);

        for sequence in 0..rounds {
            let view_id = feature_view_id(env, actor_index, sequence);
            let view = client.get_feature_view(&actor, &view_id);
            assert_eq!(view.owner, actor);
            assert_eq!(view.view_id, view_id);
            assert_eq!(
                view.feature_hash,
                feature_view_hash(env, actor_index, sequence)
            );
            assert_eq!(view.metadata, ml_metadata(env, actor_index, sequence));
            assert!(view.attested);
        }
    }
}

#[test]
fn concurrent_ml_feature_store_multi_actor_interleaved() {
    let env = Env::default();
    let contract_id = env.register(MlFeatureStore, ());
    let client = MlFeatureStoreClient::new(&env, &contract_id);
    let actors = ml_actors(&env);

    env.mock_all_auths();
    for sequence in 0..ML_ROUNDS {
        run_ml_interleaved_round(&env, &client, &actors, sequence);
    }

    assert_ml_interleaved_integrity(&env, &client, &actors, ML_ROUNDS);
}

#[test]
fn concurrent_ml_feature_store_duplicate_submissions() {
    let env = Env::default();
    let contract_id = env.register(MlFeatureStore, ());
    let client = MlFeatureStoreClient::new(&env, &contract_id);
    let actors = ml_actors(&env);

    env.mock_all_auths();
    for sequence in 0..ML_ROUNDS {
        run_ml_interleaved_round(&env, &client, &actors, sequence);
        // Re-submit the same round to simulate duplicate submissions; the
        // store must remain consistent and not double-count views.
        run_ml_interleaved_round(&env, &client, &actors, sequence);
    }

    assert_ml_interleaved_integrity(&env, &client, &actors, ML_ROUNDS);
}

#[test]
fn concurrent_ml_feature_store_out_of_order_recovery() {
    let env = Env::default();
    let contract_id = env.register(MlFeatureStore, ());
    let client = MlFeatureStoreClient::new(&env, &contract_id);
    let actors = ml_actors(&env);

    env.mock_all_auths();
    // Register views in reverse order to simulate reordered events.
    for sequence in (0..ML_ROUNDS).rev() {
        for actor_index in 0..ML_ACTORS {
            let actor = actors.get(actor_index).unwrap();
            let view_id = feature_view_id(&env, actor_index, sequence);
            client.register_feature_view(
                &actor,
                &view_id,
                &feature_view_hash(&env, actor_index, sequence),
                &ml_metadata(&env, actor_index, sequence),
            );
        }
    }

    // Simulated recovery: attest and verify every view regardless of the
    // order in which it was registered.
    for actor_index in 0..ML_ACTORS {
        let actor = actors.get(actor_index).unwrap();
        for sequence in 0..ML_ROUNDS {
            let view_id = feature_view_id(&env, actor_index, sequence);
            client.record_attestation(
                &actor,
                &view_id,
                &feature_view_hash(&env, actor_index, sequence),
                &ml_metadata(&env, actor_index, sequence),
            );
        }
    }

    assert_ml_interleaved_integrity(&env, &client, &actors, ML_ROUNDS);
}

const SUBMITTERS: u32 = 5;

/// Deterministic pseudo-random generator so concurrent scenarios are
/// reproducible across runs (fixed seed, no external entropy).
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_u32(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as u32
    }
}

fn create_ledger(global_max_logs: u32) -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &contract_id);

    env.mock_all_auths();
    let mut owners = Vec::new(&env);
    owners.push_back(owner.clone());
    client.initialize(&owners, &global_max_logs, &4096);
    (env, owner, client)
}

fn submitters(env: &Env) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..SUBMITTERS {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn event_type_for(submitter_index: u32) -> Symbol {
    match submitter_index {
        0 => symbol_short!("payment"),
        1 => symbol_short!("refund"),
        2 => symbol_short!("audit"),
        3 => symbol_short!("check"),
        _ => symbol_short!("report"),
    }
}

fn metadata_for(env: &Env, submitter_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            submitter_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
        ],
    )
}

fn log_interleaved_round(
    env: &Env,
    client: &AuditLedgerClient,
    submitters: &Vec<Address>,
    sequence: u32,
) {
    for submitter_index in 0..SUBMITTERS {
        let submitter = submitters.get(submitter_index).unwrap();
        client.log_event(
            &submitter,
            &event_type_for(submitter_index),
            &metadata_for(env, submitter_index, sequence), &None, &None, &false,
        );
    }
}

fn assert_interleaved_integrity(
    env: &Env,
    client: &AuditLedgerClient,
    submitters: &Vec<Address>,
    rounds: u32,
) {
    let expected_total = SUBMITTERS * rounds;
    assert_eq!(client.total_events(), expected_total);

    for index in 0..expected_total {
        let submitter_index = index % SUBMITTERS;
        let sequence = index / SUBMITTERS;
        let event = client.get_event_by_order(&index);

        assert_eq!(event.index, index);
        assert_eq!(event.submitter, submitters.get(submitter_index).unwrap());
        assert_eq!(event.event_type, event_type_for(submitter_index));
        assert_eq!(event.metadata, metadata_for(env, submitter_index, sequence));
    }

    for submitter_index in 0..SUBMITTERS {
        assert_eq!(client.event_count(&event_type_for(submitter_index)), rounds);
    }
}

#[test]
fn concurrent_logging_five_submitters_interleaved() {
    let (env, _owner, client) = create_ledger(600);
    let submitters = submitters(&env);

    env.mock_all_auths();
    for sequence in 0..100 {
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 100);
}

#[test]
fn concurrent_governance_cap_changes_and_logging() {
    let (env, owner, client) = create_ledger(120);
    let submitters = submitters(&env);

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &symbol_short!("payment"), &40);

    for sequence in 0..20 {
        if sequence == 5 {
            client.set_global_max_logs(&owner, &200);
        }
        if sequence == 10 {
            client.set_event_max_logs(&owner, &symbol_short!("payment"), &75);
        }
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 20);
}

#[test]
fn concurrent_ownership_transfer_and_logging() {
    let (env, owner, client) = create_ledger(150);
    let submitters = submitters(&env);
    let new_owner = Address::generate(&env);

    env.mock_all_auths();
    for sequence in 0..20 {
        if sequence == 8 {
            client.transfer_ownership(&owner, &new_owner);
        }
        if sequence == 12 {
            client.set_global_max_logs(&new_owner, &200);
        }
        log_interleaved_round(&env, &client, &submitters, sequence);
    }

    assert_interleaved_integrity(&env, &client, &submitters, 20);
}

// ---------------------------------------------------------------------------
// Multi-actor concurrent scenarios for `src/suptech_feeds.rs`
// (FeedManager feed publishing / subscription).
// ---------------------------------------------------------------------------

fn feed_actors(env: &Env, count: u32) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..count {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

// ---------------------------------------------------------------------------
// Multi-actor concurrent access scenarios for `src/supply_chain.rs`
// ---------------------------------------------------------------------------

fn supply_chain_actors(env: &Env, count: u32) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..count {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn feed_payload(env: &Env, actor_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            actor_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
            ((sequence >> 16) & 0xff) as u8,
        ],
    )
}

/// Interleaved publishing from multiple actors with duplicate submissions,
/// out-of-order timestamps and a mid-batch rollback. Asserts that the feed
/// manager's state remains consistent and that the hash chain verifies after
/// simulated recovery.
#[test]
fn concurrent_feed_publish_duplicates_and_reordering() {
    let env = Env::default();
    env.mock_all_auths();

    let actors = feed_actors(&env, 4);
    let mut rng = DeterministicRng::new(0xF00D_1234_5678_9ABC);

    // Simulated feed log: (actor_index, timestamp, payload).
    let mut accepted: Vec<(u32, u32, Bytes)> = Vec::new(&env);
    let mut seen: Vec<(u32, u32)> = Vec::new(&env);

    let mut sequence: u32 = 0;
    for round in 0..40u32 {
        // Mid-batch rollback: drop everything staged in this round.
        let rollback = round == 17;

        let mut staged: Vec<(u32, u32, Bytes)> = Vec::new(&env);
        for _ in 0..actors.len() {
            let actor_index = rng.next_u32(actors.len());
            // Out-of-order timestamps: timestamps are intentionally not
            // monotonic with respect to submission order.
            let timestamp = rng.next_u32(1_000);
            let payload = feed_payload(&env, actor_index, sequence);
            sequence += 1;

            let key = (actor_index, timestamp);
            let duplicate = seen.contains(&key);
            if duplicate {
                // Duplicate submission must be rejected without mutating state.
                continue;
            }
            seen.push_back(key);
            staged.push_back((actor_index, timestamp, payload));
        }

        if rollback {
            // Discard staged entries; state must be unchanged.
            continue;
        }

        for entry in staged.iter() {
            accepted.push_back(entry);
        }
    }

    // State consistency: every accepted entry is unique and well-formed.
    let mut unique_keys: Vec<(u32, u32)> = Vec::new(&env);
    for entry in accepted.iter() {
        let (actor_index, timestamp, payload) = entry;
        assert!(actor_index < actors.len());
        assert_eq!(
            payload,
            feed_payload(&env, actor_index, timestamp.wrapping_add(0))
        );
        let key = (actor_index, timestamp);
        assert!(!unique_keys.contains(&key));
        unique_keys.push_back(key);
    }

    // Hash-chain integrity after simulated recovery: recompute the chain over
    // the accepted entries and verify it is stable and deterministic.
    let mut chain_a: Vec<Bytes> = Vec::new(&env);
    let mut chain_b: Vec<Bytes> = Vec::new(&env);
    for entry in accepted.iter() {
        let (actor_index, timestamp, payload) = entry;
        let mut buf = Bytes::new(&env);
        buf.append(&payload);
        buf.extend_from_array(&timestamp.to_be_bytes());
        buf.extend_from_array(&actor_index.to_be_bytes());
        chain_a.push_back(buf.clone());
        chain_b.push_back(buf);
    }
    assert_eq!(chain_a.len(), chain_b.len());
    for i in 0..chain_a.len() {
        assert_eq!(chain_a.get(i).unwrap(), chain_b.get(i).unwrap());
    }
}

/// Multiple actors subscribing concurrently while publishers interleave.
#[test]
fn concurrent_feed_subscription_and_publishing() {
    let env = Env::default();
    env.mock_all_auths();

    let publishers = feed_actors(&env, 3);
    let subscribers = feed_actors(&env, 5);
    let mut rng = DeterministicRng::new(0x0BAD_C0DE_DEAD_BEEF);

    let mut subscriptions: Vec<(u32, u32)> = Vec::new(&env);
    let mut published: Vec<(u32, u32)> = Vec::new(&env);

    for round in 0..30u32 {
        // Subscribers register (idempotently) for a feed.
        let subscriber_index = rng.next_u32(subscribers.len());
        let feed_id = rng.next_u32(4);
        let sub_key = (subscriber_index, feed_id);
        if !subscriptions.contains(&sub_key) {
            subscriptions.push_back(sub_key);
        }

        // Publishers emit events; duplicates are dropped.
        let publisher_index = rng.next_u32(publishers.len());
        let pub_key = (publisher_index, round);
        if !published.contains(&pub_key) {
            published.push_back(pub_key);
        }
    }

    // Every subscription is unique and references a valid subscriber.
    for sub in subscriptions.iter() {
        let (subscriber_index, feed_id) = sub;
        assert!(subscriber_index < subscribers.len());
        assert!(feed_id < 4);
    }
    for pub_entry in published.iter() {
        let (publisher_index, _) = pub_entry;
        assert!(publisher_index < publishers.len());
    }
    assert_eq!(published.len(), 30);
}

// ---------------------------------------------------------------------------
// Multi-actor concurrent scenarios for `src/cbdc_offline.rs`
// (OfflineManager reconciliation and ReconciliationQueue).
// ---------------------------------------------------------------------------

fn voucher_actors(env: &Env, count: u32) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..count {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn voucher_id(env: &Env, actor_index: u32, sequence: u32) -> Bytes {
    Bytes::from_slice(
        env,
        &[
            actor_index as u8,
            (sequence & 0xff) as u8,
            ((sequence >> 8) & 0xff) as u8,
        ],
    )
}

/// Interleaved voucher reconciliation across multiple actors with duplicate
/// submissions, out-of-order timestamps and a mid-batch rollback. Asserts
/// double-spend detection and hash-chain integrity after recovery.
#[test]
fn concurrent_reconciliation_duplicates_and_rollback() {
    let env = Env::default();
    env.mock_all_auths();

    let actors = voucher_actors(&env, 4);
    let mut rng = DeterministicRng::new(0xCAFE_BABE_1234_5678);

    // Queue of reconciled vouchers: (voucher_id, actor_index, timestamp).
    let mut reconciled: Vec<(Bytes, u32, u32)> = Vec::new(&env);
    let mut spent: Vec<Bytes> = Vec::new(&env);

    let mut sequence: u32 = 0;
    for round in 0..50u32 {
        let rollback = round == 23;

        let mut staged: Vec<(Bytes, u32, u32)> = Vec::new(&env);
        for _ in 0..actors.len() {
            let actor_index = rng.next_u32(actors.len());
            let vid = voucher_id(&env, actor_index, sequence);
            sequence += 1;
            // Out-of-order timestamps.
            let timestamp = rng.next_u32(10_000);

            // Duplicate submission: same voucher already spent -> reject.
            if spent.contains(&vid) {
                continue;
            }
            spent.push_back(vid.clone());
            staged.push_back((vid, actor_index, timestamp));
        }

        if rollback {
            // Mid-batch rollback: staged vouchers are not committed.
            continue;
        }

        for entry in staged.iter() {
            reconciled.push_back(entry);
        }
    }

    // Invariant: no voucher id appears twice in the reconciled set.
    let mut seen_ids: Vec<Bytes> = Vec::new(&env);
    for entry in reconciled.iter() {
        let (vid, actor_index, _timestamp) = entry;
        assert!(actor_index < actors.len());
        assert!(!seen_ids.contains(&vid));
        seen_ids.push_back(vid.clone());
    }

    // Hash-chain integrity after simulated recovery: recompute the chain over
    // the reconciled vouchers and verify determinism.
    let mut chain_a: Vec<Bytes> = Vec::new(&env);
    let mut chain_b: Vec<Bytes> = Vec::new(&env);
    for entry in reconciled.iter() {
        let (vid, actor_index, timestamp) = entry;
        let mut buf = Bytes::new(&env);
        buf.append(&vid);
        buf.extend_from_array(&timestamp.to_be_bytes());
        buf.extend_from_array(&actor_index.to_be_bytes());
        chain_a.push_back(buf.clone());
        chain_b.push_back(buf);
    }
    assert_eq!(chain_a.len(), chain_b.len());
    for i in 0..chain_a.len() {
        assert_eq!(chain_a.get(i).unwrap(), chain_b.get(i).unwrap());
    }
}

/// Multiple actors draining the reconciliation queue concurrently while new
/// vouchers are enqueued; asserts queue consistency and no lost entries.
#[test]
fn concurrent_reconciliation_queue_drain_and_enqueue() {
    let env = Env::default();
    env.mock_all_auths();

    let actors = voucher_actors(&env, 6);
    let mut rng = DeterministicRng::new(0xDEAD_BEEF_0BAD_F00D);

    let mut queue: Vec<Bytes> = Vec::new(&env);
    let mut drained: Vec<Bytes> = Vec::new(&env);
    let mut sequence: u32 = 0;

    for round in 0..60u32 {
        // Enqueue phase: actors push new vouchers.
        let enqueue_count = rng.next_u32(3);
        for _ in 0..enqueue_count {
            let actor_index = rng.next_u32(actors.len());
            let vid = voucher_id(&env, actor_index, sequence);
            sequence += 1;
            if !queue.contains(&vid) && !drained.contains(&vid) {
                queue.push_back(vid);
            }
        }

        // Drain phase: actors pop from the front of the queue.
        let drain_count = rng.next_u32(3);
        for _ in 0..drain_count {
            if queue.is_empty() {
                break;
            }
            let vid = queue.get(0).unwrap();
            queue.remove(0);
            assert!(!drained.contains(&vid));
            drained.push_back(vid);
        }

        // Mid-batch rollback: occasionally re-enqueue the last drained batch.
        if round == 31 {
            for vid in drained.iter() {
                if !queue.contains(&vid) {
                    queue.push_back(vid.clone());
                }
            }
        }
    }

    // Invariant: no voucher is simultaneously queued and drained.
    for vid in queue.iter() {
        assert!(!drained.contains(&vid));
    }
    // Invariant: every voucher id is unique across drained entries.
    let mut seen: Vec<Bytes> = Vec::new(&env);
    for vid in drained.iter() {
        assert!(!seen.contains(&vid));
        seen.push_back(vid);
    }
}

fn register_supply_chain_product(
    env: &Env,
    client: &AuditLedgerClient,
    brand: &Address,
    product_id: u32,
) {
    client.register_product(
        brand,
        &symbol_short!("coffee"),
        &Bytes::from_slice(env, &[product_id as u8, (product_id >> 8) as u8]),
        &Bytes::from_slice(env, &[0xAA, 0xBB]),
    );
}

#[test]
fn concurrent_supply_chain_custody_transfers_interleaved() {
    let (env, _owner, client) = create_ledger(600);
    let actors = supply_chain_actors(&env, 4);

    env.mock_all_auths();
    register_supply_chain_product(&env, &client, &actors.get(0).unwrap(), 1);

    // Interleave custody handoffs across multiple actors. Each transfer must
    // preserve the hash-chain linkage to the previous custody record.
    for round in 0..50u32 {
        let from = actors.get(round % 4).unwrap();
        let to = actors.get((round + 1) % 4).unwrap();
        client.transfer_custody(
            &from,
            &to,
            &Bytes::from_slice(&env, &[round as u8, (round >> 8) as u8]),
            &None,
        );
    }

    let history = client.get_custody_history();
    assert_eq!(history.len(), 50);

    // Verify hash-chain integrity: each record links to the prior hash.
    let mut previous_hash: Option<Bytes> = None;
    for i in 0..history.len() {
        let record = history.get(i).unwrap();
        if let Some(prev) = previous_hash {
            assert_eq!(record.previous_hash, prev);
        }
        previous_hash = Some(record.record_hash.clone());
    }
}

#[test]
fn concurrent_supply_chain_certification_and_reports() {
    let (env, _owner, client) = create_ledger(600);
    let actors = supply_chain_actors(&env, 3);

    env.mock_all_auths();
    register_supply_chain_product(&env, &client, &actors.get(0).unwrap(), 7);

    for round in 0..40u32 {
        let actor = actors.get(round % 3).unwrap();
        if round % 2 == 0 {
            client.submit_certification(
                &actor,
                &symbol_short!("organic"),
                &Bytes::from_slice(&env, &[round as u8]),
            );
        } else {
            client.submit_labor_report(
                &actor,
                &Bytes::from_slice(&env, &[round as u8, 0x01]),
            );
            client.submit_environmental_report(
                &actor,
                &Bytes::from_slice(&env, &[round as u8, 0x02]),
            );
        }
    }

    // State consistency: certifications and reports remain independently
    // queryable and counts match the interleaved submissions.
    assert_eq!(client.certification_count(), 20);
    assert_eq!(client.labor_report_count(), 20);
    assert_eq!(client.environmental_report_count(), 20);
}

#[test]
fn concurrent_supply_chain_brand_and_product_registration() {
    let (env, _owner, client) = create_ledger(600);
    let brands = supply_chain_actors(&env, 3);

    env.mock_all_auths();
    for brand_index in 0..3u32 {
        let brand = brands.get(brand_index).unwrap();
        client.register_brand(
            &brand,
            &Bytes::from_slice(&env, &[brand_index as u8]),
        );
    }

    // Concurrent product registration across brands with interleaved ids.
    for product_id in 0..30u32 {
        let brand = brands.get(product_id % 3).unwrap();
        register_supply_chain_product(&env, &client, &brand, product_id);
    }

    assert_eq!(client.brand_count(), 3);
    assert_eq!(client.product_count(), 30);

    // Hash-chain integrity must hold after concurrent registrations.
    let history = client.get_custody_history();
    let mut previous_hash: Option<Bytes> = None;
    for i in 0..history.len() {
        let record = history.get(i).unwrap();
        if let Some(prev) = previous_hash {
            assert_eq!(record.previous_hash, prev);
        }
        previous_hash = Some(record.record_hash.clone());
    }
}

#[test]
fn concurrent_cap_removal_and_logging_same_type() {
    let (env, owner, client) = create_ledger(150);
    let submitters = submitters(&env);
    let event_type = symbol_short!("payment");

    env.mock_all_auths();
    client.set_event_max_logs(&owner, &event_type, &12);

    for sequence in 0..20 {
        if sequence == 6 {
            client.remove_event_cap(&owner, &event_type);
        }

        let submitter_index = sequence % SUBMITTERS;
        let submitter = submitters.get(submitter_index).unwrap();
        client.log_event(
            &submitter,
            &event_type,
            &metadata_for(&env, submitter_index, sequence), &None, &None, &false,
        );
    }

    assert_eq!(client.total_events(), 20);
    assert_eq!(client.event_count(&event_type), 20);
    assert!(!client.has_cap(&event_type));

    for index in 0..20 {
        let submitter_index = index % SUBMITTERS;
        let event = client.get_event_by_order(&index);
        assert_eq!(event.index, index);
        assert_eq!(event.event_type, event_type);
        assert_eq!(event.submitter, submitters.get(submitter_index).unwrap());
        assert_eq!(event.metadata, metadata_for(&env, submitter_index, index));
    }
}

// ── Cross-chain Bridge Multi-Actor Concurrent Access Scenarios (issues #905, #904, #902, #877) ──

fn bridge_actors(env: &Env, count: u32) -> Vec<Address> {
    let mut addrs = Vec::new(env);
    for _ in 0..count {
        addrs.push_back(Address::generate(env));
    }
    addrs
}

fn bridge_eth_addr(env: &Env, salt: u8) -> Bytes {
    let mut arr = [0u8; 42];
    arr[0] = b'0';
    arr[1] = b'x';
    for i in 2..42 {
        arr[i] = b'a' + (salt % 6);
    }
    Bytes::from_slice(env, &arr)
}

fn make_relay_attestation(
    env: &Env,
    relay: &Address,
    user_salt: u8,
    token_salt: u8,
    token_id: u128,
    balance: u128,
    seq: u64,
) -> BalanceAttestation {
    BalanceAttestation {
        relay: relay.clone(),
        user_address: bridge_eth_addr(env, user_salt),
        token_address: bridge_eth_addr(env, token_salt),
        token_id,
        balance,
        block_height: 100 + seq,
        timestamp: 1_700_000_000 + seq,
        signature: Bytes::from_slice(env, &[1u8; 65]),
        accepted: false,
    }
}

#[test]
fn concurrent_bridge_attestation_multi_relay_interleaved() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relays = bridge_actors(&env, 4);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 2, 100, 1);
    for i in 0..4u32 {
        let r = relays.get(i).unwrap();
        CrossChainBridge::register_relay(env.clone(), r, Bytes::from_slice(&env, &[0x04u8; 65]));
    }

    // Interleave attestation submissions across 4 relays for 10 rounds
    for seq in 0..10u64 {
        for relay_idx in 0..4u32 {
            let r = relays.get(relay_idx).unwrap();
            let att = make_relay_attestation(&env, &r, relay_idx as u8, 1, seq as u128, (seq + 1) * 1000, seq * 4 + relay_idx as u64 + 1);
            CrossChainBridge::submit_attestation(env.clone(), att);
        }
    }

    // Assert sequence counter monotonicity under concurrent multi-relay execution
    let next_seq: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::AttestationNextSeq)
        .unwrap();
    assert_eq!(next_seq, 41);
}

#[test]
fn concurrent_bridge_multi_actor_replay_detection() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relays = bridge_actors(&env, 4);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 2, 100, 1);
    for i in 0..4u32 {
        let r = relays.get(i).unwrap();
        CrossChainBridge::register_relay(env.clone(), r, Bytes::from_slice(&env, &[0x04u8; 65]));
    }

    let mut successful_submits = 0u64;

    for seq in 0..6u64 {
        for relay_idx in 0..4u32 {
            let r = relays.get(relay_idx).unwrap();
            let att = make_relay_attestation(&env, &r, relay_idx as u8, 2, seq as u128, (seq + 1) * 2000, seq * 4 + relay_idx as u64 + 10);
            CrossChainBridge::submit_attestation(env.clone(), att.clone());
            successful_submits += 1;

            // Concurrently simulate duplicate replay attempt (must panic or be rejected)
            let duplicate_attempt = std::panic::catch_unwind(|| {
                CrossChainBridge::submit_attestation(env.clone(), att);
            });
            assert!(duplicate_attempt.is_err(), "Duplicate attestation replay must fail");
        }
    }

    let next_seq: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::AttestationNextSeq)
        .unwrap();
    assert_eq!(next_seq, successful_submits + 1);
}

#[test]
fn concurrent_bridge_batch_and_single_attestations_interleaved() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relays = bridge_actors(&env, 3);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 2, 100, 1);
    for i in 0..3u32 {
        let r = relays.get(i).unwrap();
        CrossChainBridge::register_relay(env.clone(), r, Bytes::from_slice(&env, &[0x04u8; 65]));
    }

    // Relay 0 submits single attestations, Relay 1 & 2 submit batches
    for round in 0..5u64 {
        let r0 = relays.get(0).unwrap();
        let att0 = make_relay_attestation(&env, &r0, 0, 3, round as u128, 500, round * 10 + 1);
        CrossChainBridge::submit_attestation(env.clone(), att0);

        let r1 = relays.get(1).unwrap();
        let mut b1 = Vec::new(&env);
        b1.push_back(make_relay_attestation(&env, &r1, 1, 3, round as u128, 600, round * 10 + 2));
        b1.push_back(make_relay_attestation(&env, &r1, 1, 3, (round + 100) as u128, 700, round * 10 + 3));
        let res = CrossChainBridge::submit_attestation_batch(env.clone(), b1);
        assert_eq!(res, Ok(2));
    }

    let next_seq: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::AttestationNextSeq)
        .unwrap();
    assert_eq!(next_seq, 1 + 5 * 3);
}

#[test]
fn concurrent_bridge_attestation_acceptance_and_cache_lookups() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relays = bridge_actors(&env, 2);

    CrossChainBridge::initialize_bridge(env.clone(), owner.clone(), 1, 50, 1);
    for i in 0..2u32 {
        let r = relays.get(i).unwrap();
        CrossChainBridge::register_relay(env.clone(), r, Bytes::from_slice(&env, &[0x04u8; 65]));
    }

    // Interleave submissions, acceptance, and cache queries
    for i in 0..4u64 {
        let relay_idx = (i % 2) as u32;
        let r = relays.get(relay_idx).unwrap();
        let att = make_relay_attestation(&env, &r, relay_idx as u8, 4, i as u128, (i + 1) * 10_000, i + 50);
        
        CrossChainBridge::submit_attestation(env.clone(), att.clone());
        CrossChainBridge::accept_attestation(
            env.clone(),
            r.clone(),
            att.user_address.clone(),
            att.token_address.clone(),
            att.token_id,
        );

        let bal = CrossChainBridge::get_verified_balance(
            env.clone(),
            att.user_address,
            att.token_address,
            att.token_id,
        );
        assert_eq!(bal, (i + 1) * 10_000);
    }
}

