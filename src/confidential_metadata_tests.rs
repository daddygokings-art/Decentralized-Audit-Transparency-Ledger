#[cfg(test)]
mod tests {
    use super::confidential_metadata::{
        AccessControlPolicy, ConfidentialError, ConfidentialEventLedger,
        ConfidentialEventLedgerClient,
    };
    use crate::rbac::{RbacManager, Role};
    use soroban_sdk::{
        testutils::Address as _, Address, Bytes, BytesN, Env, Symbol, Vec,
    };

    fn bootstrap(env: &Env, admin: &Address, submitter: &Address) {
        env.mock_all_auths();
        RbacManager::initialize_admin(env, admin);
        RbacManager::set_role(env, admin, submitter, Role::Submitter).unwrap();
    }

    #[test]
    fn test_log_and_access_confidential_event() {
        let env = Env::default();
        let contract_id = env.register(ConfidentialEventLedger, ());
        let client = ConfidentialEventLedgerClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let recipient = Address::generate(&env);
        let stranger = Address::generate(&env);
        bootstrap(&env, &admin, &submitter);

        client.initialize(&admin);

        let event_type = Symbol::new(&env, "audit_priv");
        let encrypted_metadata = Bytes::from_slice(&env, b"encrypted_secret_data_payload_123");
        let zk_proof = Bytes::from_slice(&env, b"valid_zk_snark_proof_commitment_bytes_32");
        let key_id = BytesN::from_array(&env, &[7u8; 32]);

        let mut allowed = Vec::new(&env);
        allowed.push_back(recipient.clone());
        let policy = AccessControlPolicy::CustomAllowlist(allowed);

        let event_id = client
            .log_confidential_event(
                &submitter,
                &event_type,
                &encrypted_metadata,
                &policy,
                &zk_proof,
                &key_id,
            )
            .unwrap();

        // Access check
        assert!(client.check_access(&event_id, &submitter));
        assert!(client.check_access(&event_id, &recipient));
        assert!(!client.check_access(&event_id, &stranger));

        // Decrypt / retrieve payload
        let decrypted = client.decrypt_event_metadata(&event_id, &recipient);
        assert_eq!(decrypted, encrypted_metadata);

        // Unauthorized role (viewer) cannot log an event.
        let viewer = Address::generate(&env);
        let err = client.log_confidential_event(
            &viewer,
            &event_type,
            &encrypted_metadata,
            &policy,
            &zk_proof,
            &key_id,
        );
        assert_eq!(err, Err(ConfidentialError::InsufficientRole));
    }

    #[test]
    fn test_key_registration_and_rotation() {
        let env = Env::default();
        let contract_id = env.register(ConfidentialEventLedger, ());
        let client = ConfidentialEventLedgerClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        bootstrap(&env, &admin, &user);

        client.initialize(&admin);

        let pub_key1 = Bytes::from_slice(&env, b"public_key_v1");
        client.register_encryption_key(&user, &pub_key1).unwrap();

        let pub_key2 = Bytes::from_slice(&env, b"public_key_v2");
        client.rotate_encryption_key(&user, &pub_key2).unwrap();

        client.revoke_encryption_key(&user).unwrap();
    }

    #[test]
    fn test_byzantine_actor_fuzz_invariants() {
        let env = Env::default();
        let contract_id = env.register(ConfidentialEventLedger, ());
        let client = ConfidentialEventLedgerClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        bootstrap(&env, &admin, &submitter);
        client.initialize(&admin);

        let event_type = Symbol::new(&env, "byzantine");
        let key_id = BytesN::from_array(&env, &[41u8; 32]);
        let proof = Bytes::from_slice(&env, b"deterministic-zk-proof-32");

        for seed in 0u32..32 {
            let mut payload = [0u8; 40];
            payload[0..4].copy_from_slice(&seed.to_be_bytes());
            payload[4..].fill(seed as u8);
            let result = client.try_log_confidential_event(
                &submitter,
                &event_type,
                &Bytes::from_slice(&env, &payload),
                &AccessControlPolicy::Public,
                &proof,
                &key_id,
            );
            assert!(result.is_ok());
        }

        let too_short = Bytes::from_slice(&env, b"short");
        let rejected = client.try_log_confidential_event(
            &unauthorized,
            &event_type,
            &Bytes::from_slice(&env, b"payload"),
            &AccessControlPolicy::Public,
            &too_short,
            &key_id,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn test_confidential_analytics_aggregation() {
        let env = Env::default();
        let contract_id = env.register(ConfidentialEventLedger, ());
        let client = ConfidentialEventLedgerClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);
        bootstrap(&env, &admin, &submitter);

        client.initialize(&admin);

        let event_type = Symbol::new(&env, "audit");
        let zk_proof = Bytes::from_slice(&env, b"valid_zk_snark_proof_commitment_bytes_32");
        let key_id = BytesN::from_array(&env, &[1u8; 32]);

        let id1 = client
            .log_confidential_event(
                &submitter,
                &event_type,
                &Bytes::from_slice(&env, b"chunk_1"),
                &AccessControlPolicy::Public,
                &zk_proof,
                &key_id,
            )
            .unwrap();
        let id2 = client
            .log_confidential_event(
                &submitter,
                &event_type,
                &Bytes::from_slice(&env, b"chunk_2"),
                &AccessControlPolicy::Public,
                &zk_proof,
                &key_id,
            )
            .unwrap();

        let mut list = Vec::new(&env);
        list.push_back(id1);
        list.push_back(id2);

        let agg = client.aggregate_confidential_events(&list);
        assert_eq!(agg.total_events, 2);
    }
}
