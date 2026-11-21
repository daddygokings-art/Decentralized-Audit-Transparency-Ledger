#![cfg(test)]

use crate::cross_chain_bridge::{
    BridgeError, BridgeKey, BridgeRelay, CrossChainBridge, BalanceAttestation,
};
use soroban_sdk::{
    testutils::Address as _,
    Address, Bytes, BytesN, Env, Vec,
};

fn sample_pubkey(env: &Env) -> Bytes {
    let mut b = [0u8; 65];
    b[0] = 0x04;
    b[1] = 0xAA;
    Bytes::from_array(env, &b)
}

fn sample_attestation(env: &Env, relay: &Address, token_id: u128, block: u64, ts: u64) -> BalanceAttestation {
    BalanceAttestation {
        user_address: Bytes::from_array(env, &[0x11; 20]),
        token_address: Bytes::from_array(env, &[0x22; 20]),
        token_id,
        balance: 1_000_000,
        block_height: block,
        timestamp: ts,
        relay: relay.clone(),
        signature: Bytes::from_array(env, &[0x77; 65]),
    }
}

#[test]
fn test_byzantine_duplicate_attestation_replay_rejection() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relayer = Address::generate(&env);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 1, 100, 1);
    CrossChainBridge::register_relay(env.clone(), relayer.clone(), sample_pubkey(&env));

    let att = sample_attestation(&env, &relayer, 1, 1000, 100);
    CrossChainBridge::submit_attestation(env.clone(), att.clone());

    // Second submission of identical attestation must be rejected as Byzantine replay
    let hash = CrossChainBridge::compute_attestation_hash(&env, &att);
    assert!(env.storage().instance().has(&BridgeKey::SeenAttestation(hash)));
}

#[test]
fn test_byzantine_multi_actor_batch_equivocation() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let honest_relayer = Address::generate(&env);
    let byzantine_relayer = Address::generate(&env);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 2, 100, 1);
    CrossChainBridge::register_relay(env.clone(), honest_relayer.clone(), sample_pubkey(&env));
    // Byzantine relayer remains unregistered or inactive

    let att1 = sample_attestation(&env, &honest_relayer, 1, 1000, 100);
    let att2 = sample_attestation(&env, &byzantine_relayer, 1, 1000, 100);

    let mut batch = Vec::new(&env);
    batch.push_back(att1);
    batch.push_back(att2);

    // Multi-actor batch submission containing Byzantine inactive actor must fail atomically
    let res = CrossChainBridge::submit_attestation_batch(env.clone(), batch);
    assert!(res.is_err());
}

#[test]
fn test_byzantine_fuzz_timestamp_and_zero_block() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let relayer = Address::generate(&env);

    CrossChainBridge::initialize_bridge(env.clone(), owner, 1, 100, 1);
    CrossChainBridge::register_relay(env.clone(), relayer.clone(), sample_pubkey(&env));

    // Byzantine fuzzing: zero timestamp
    let bad_att = sample_attestation(&env, &relayer, 1, 0, 0);
    let mut batch = Vec::new(&env);
    batch.push_back(bad_att);

    let res = CrossChainBridge::submit_attestation_batch(env.clone(), batch);
    assert_eq!(res, Err(BridgeError::OutOfOrderAttestation));
}
