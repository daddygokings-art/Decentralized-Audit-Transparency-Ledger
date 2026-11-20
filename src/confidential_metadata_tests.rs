#[cfg(test)]
mod tests {
    use crate::confidential_metadata::{
        AccessControlPolicy, ConfidentialAuditEvent, ConfidentialError, ConfidentialEventLedger,
        ConfidentialEventLedgerClient, ThresholdPolicy, MAX_AGGREGATE_BATCH,
        MAX_ENCRYPTED_PAYLOAD_BYTES,
    };
    use crate::rbac::{RbacManager, Role};
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        Address, Bytes, BytesN, Env, Event as _, Symbol, Vec,
    };

    struct Ctx<'a> {
        env: Env,
        /// Distinguishes proofs across events: ZK proofs are single-use.
        proof_nonce: core::cell::Cell<u8>,
        contract_id: Address,
        client: ConfidentialEventLedgerClient<'a>,
        admin: Address,
        submitter: Address,
    }

    /// Register the contract, seed RBAC *inside* the contract's storage, and
    /// initialize it.
    fn setup<'a>() -> Ctx<'a> {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ConfidentialEventLedger, ());
        let client = ConfidentialEventLedgerClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let submitter = Address::generate(&env);

        env.as_contract(&contract_id, || {
            RbacManager::initialize_admin(&env, &admin);
            RbacManager::set_role(&env, &admin, &submitter, Role::Submitter).unwrap();
        });
        client.initialize(&admin);

        Ctx { env, proof_nonce: core::cell::Cell::new(0), contract_id, client, admin, submitter }
    }

    fn grant(ctx: &Ctx, who: &Address, role: Role) {
        ctx.env.as_contract(&ctx.contract_id, || {
            RbacManager::set_role(&ctx.env, &ctx.admin, who, role).unwrap();
        });
    }

    fn proof(env: &Env) -> Bytes {
        Bytes::from_slice(env, b"valid_zk_snark_proof_commitment_bytes_32")
    }

    fn fresh_proof(ctx: &Ctx) -> Bytes {
        let n = ctx.proof_nonce.get();
        ctx.proof_nonce.set(n + 1);
        let mut p = proof(&ctx.env);
        p.push_back(n);
        p
    }

    fn key_id(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[7u8; 32])
    }

    fn log(ctx: &Ctx, payload: &[u8], policy: &AccessControlPolicy) -> BytesN<32> {
        ctx.client.log_confidential_event(
            &ctx.submitter,
            &Symbol::new(&ctx.env, "audit_priv"),
            &Bytes::from_slice(&ctx.env, payload),
            policy,
            &fresh_proof(ctx),
            &key_id(&ctx.env),
        )
    }

    fn try_log(
        ctx: &Ctx,
        payload: &Bytes,
        policy: &AccessControlPolicy,
        zk_proof: &Bytes,
    ) -> Result<BytesN<32>, ConfidentialError> {
        ctx.client
            .try_log_confidential_event(
                &ctx.submitter,
                &Symbol::new(&ctx.env, "audit_priv"),
                payload,
                policy,
                zk_proof,
                &key_id(&ctx.env),
            )
            .map(|ok| ok.unwrap())
            .map_err(|err| err.unwrap())
    }

    #[test]
    fn test_log_and_access_confidential_event() {
        let ctx = setup();
        let env = &ctx.env;
        let recipient = Address::generate(env);
        let stranger = Address::generate(env);

        let mut allowed = Vec::new(env);
        allowed.push_back(recipient.clone());
        let policy = AccessControlPolicy::CustomAllowlist(allowed);
        let payload = b"encrypted_secret_data_payload_123";
        let event_id = log(&ctx, payload, &policy);

        assert!(ctx.client.check_access(&event_id, &ctx.submitter));
        assert!(ctx.client.check_access(&event_id, &recipient));
        assert!(!ctx.client.check_access(&event_id, &stranger));

        let decrypted = ctx.client.decrypt_event_metadata(&event_id, &recipient);
        assert_eq!(decrypted, Bytes::from_slice(env, payload));

        assert_eq!(
            ctx.client.try_decrypt_event_metadata(&event_id, &stranger),
            Err(Ok(ConfidentialError::AccessDenied))
        );

        // Unauthorized role (viewer) cannot log an event.
        let viewer = Address::generate(env);
        let err = ctx.client.try_log_confidential_event(
            &viewer,
            &Symbol::new(env, "audit_priv"),
            &Bytes::from_slice(env, payload),
            &policy,
            &proof(env),
            &key_id(env),
        );
        assert_eq!(err, Err(Ok(ConfidentialError::InsufficientRole)));
    }

    #[test]
    fn test_key_registration_and_rotation() {
        let ctx = setup();
        let env = &ctx.env;
        let user = &ctx.submitter;

        ctx.client
            .register_encryption_key(user, &Bytes::from_slice(env, b"public_key_v1"));
        ctx.client
            .rotate_encryption_key(user, &Bytes::from_slice(env, b"public_key_v2"));
        ctx.client.revoke_encryption_key(user);
    }

    #[test]
    fn test_revoked_key_cannot_be_rotated() {
        let ctx = setup();
        let env = &ctx.env;
        let user = &ctx.submitter;

        ctx.client
            .register_encryption_key(user, &Bytes::from_slice(env, b"public_key_v1"));
        ctx.client.revoke_encryption_key(user);
        assert_eq!(
            ctx.client
                .try_rotate_encryption_key(user, &Bytes::from_slice(env, b"public_key_v2")),
            Err(Ok(ConfidentialError::KeyRevoked))
        );
    }

    #[test]
    fn test_missing_key_returns_typed_error() {
        let ctx = setup();
        assert_eq!(
            ctx.client.try_revoke_encryption_key(&ctx.submitter),
            Err(Ok(ConfidentialError::KeyNotFound))
        );
        assert_eq!(
            ctx.client
                .try_extend_encryption_key_ttl(&Address::generate(&ctx.env)),
            Err(Ok(ConfidentialError::KeyNotFound))
        );
    }

    #[test]
    fn test_double_initialize_returns_error() {
        let ctx = setup();
        assert_eq!(
            ctx.client.try_initialize(&ctx.admin),
            Err(Ok(ConfidentialError::AlreadyInitialized))
        );
    }

    #[test]
    fn test_validation_errors_are_propagated() {
        let ctx = setup();
        let env = &ctx.env;
        let payload = Bytes::from_slice(env, b"ciphertext");

        // Oversized payload
        let big = Bytes::from_array(env, &[0u8; (MAX_ENCRYPTED_PAYLOAD_BYTES + 1) as usize]);
        assert_eq!(
            try_log(&ctx, &big, &AccessControlPolicy::Public, &proof(env)),
            Err(ConfidentialError::PayloadTooLarge)
        );

        // Short proof
        assert_eq!(
            try_log(&ctx, &payload, &AccessControlPolicy::Public, &Bytes::from_slice(env, b"x")),
            Err(ConfidentialError::InvalidZKProof)
        );

        // Empty allowlist
        assert_eq!(
            try_log(
                &ctx,
                &payload,
                &AccessControlPolicy::CustomAllowlist(Vec::new(env)),
                &proof(env)
            ),
            Err(ConfidentialError::InvalidPolicy)
        );

        // Threshold above party count
        let mut parties = Vec::new(env);
        parties.push_back(Address::generate(env));
        let threshold = AccessControlPolicy::ThresholdDecryption(ThresholdPolicy {
            threshold: 2,
            authorized_parties: parties,
        });
        assert_eq!(
            try_log(&ctx, &payload, &threshold, &proof(env)),
            Err(ConfidentialError::InvalidPolicy)
        );

        // Unknown role level
        assert_eq!(
            try_log(&ctx, &payload, &AccessControlPolicy::RoleBased(9), &proof(env)),
            Err(ConfidentialError::InvalidPolicy)
        );
    }

    #[test]
    fn test_unknown_event_returns_typed_error() {
        let ctx = setup();
        let missing = BytesN::from_array(&ctx.env, &[0xAB; 32]);
        assert_eq!(
            ctx.client.try_get_confidential_event(&missing),
            Err(Ok(ConfidentialError::EventNotFound))
        );
        assert_eq!(
            ctx.client.try_decrypt_event_metadata(&missing, &ctx.submitter),
            Err(Ok(ConfidentialError::EventNotFound))
        );
        assert_eq!(
            ctx.client.try_extend_event_ttl(&missing),
            Err(Ok(ConfidentialError::EventNotFound))
        );
    }

    #[test]
    fn test_role_based_policy_enforces_rbac() {
        let ctx = setup();
        let env = &ctx.env;
        let auditor = Address::generate(env);
        let viewer = Address::generate(env);
        let stranger = Address::generate(env);
        grant(&ctx, &auditor, Role::Auditor);
        grant(&ctx, &viewer, Role::Viewer);

        let auditor_level = log(&ctx, b"payload", &AccessControlPolicy::RoleBased(1));
        assert!(ctx.client.check_access(&auditor_level, &ctx.submitter));
        assert!(ctx.client.check_access(&auditor_level, &ctx.admin));
        assert!(ctx.client.check_access(&auditor_level, &auditor));
        assert!(!ctx.client.check_access(&auditor_level, &viewer));
        assert!(!ctx.client.check_access(&auditor_level, &stranger));

        let admin_level = log(&ctx, b"payload", &AccessControlPolicy::RoleBased(3));
        assert!(ctx.client.check_access(&admin_level, &ctx.admin));
        assert!(!ctx.client.check_access(&admin_level, &auditor));
    }

    #[test]
    fn test_structured_audit_event_emitted() {
        let ctx = setup();
        let env = &ctx.env;
        let event_id = log(&ctx, b"payload", &AccessControlPolicy::Public);

        let expected = ConfidentialAuditEvent {
            operation: Symbol::new(env, "log_confidential_event"),
            caller: ctx.submitter.clone(),
            outcome: Symbol::new(env, "ok"),
            error_code: 0,
            subject: event_id,
            timestamp: env.ledger().timestamp(),
            ledger: env.ledger().sequence(),
        };
        assert_eq!(
            env.events().all().filter_by_contract(&ctx.contract_id),
            [expected.to_xdr(env, &ctx.contract_id)]
        );
    }

    #[test]
    fn test_confidential_analytics_aggregation() {
        let ctx = setup();
        let id1 = log(&ctx, b"chunk_1", &AccessControlPolicy::Public);
        let id2 = log(&ctx, b"chunk_2", &AccessControlPolicy::Public);

        let mut list = Vec::new(&ctx.env);
        list.push_back(id1);
        list.push_back(id2);

        let agg = ctx.client.aggregate_confidential_events(&list);
        assert_eq!(agg.total_events, 2);
        assert_eq!(agg.encrypted_accumulator, Bytes::from_slice(&ctx.env, b"chunk_1chunk_2"));
    }

    #[test]
    fn test_aggregation_errors_are_propagated() {
        let ctx = setup();
        let env = &ctx.env;

        let mut missing = Vec::new(env);
        missing.push_back(BytesN::from_array(env, &[1u8; 32]));
        assert_eq!(
            ctx.client.try_aggregate_confidential_events(&missing),
            Err(Ok(ConfidentialError::EventNotFound))
        );

        let mut too_many = Vec::new(env);
        for _ in 0..=MAX_AGGREGATE_BATCH {
            too_many.push_back(BytesN::from_array(env, &[1u8; 32]));
        }
        assert_eq!(
            ctx.client.try_aggregate_confidential_events(&too_many),
            Err(Ok(ConfidentialError::BatchTooLarge))
        );
    }

    // ── Replay protection ───────────────────────────────────────────────

    #[test]
    fn test_zk_proof_cannot_be_replayed() {
        let ctx = setup();
        let env = &ctx.env;
        let p = proof(env);

        try_log(&ctx, &Bytes::from_slice(env, b"ciphertext_a"), &AccessControlPolicy::Public, &p)
            .unwrap();

        // Same proof re-attached to a different ciphertext.
        assert_eq!(
            try_log(&ctx, &Bytes::from_slice(env, b"ciphertext_b"), &AccessControlPolicy::Public, &p),
            Err(ConfidentialError::ProofReplay)
        );

        // Same proof replayed by a different submitter.
        let other = Address::generate(env);
        grant(&ctx, &other, Role::Submitter);
        assert_eq!(
            ctx.client.try_log_confidential_event(
                &other,
                &Symbol::new(env, "audit_priv"),
                &Bytes::from_slice(env, b"ciphertext_a"),
                &AccessControlPolicy::Public,
                &p,
                &key_id(env),
            ),
            Err(Ok(ConfidentialError::ProofReplay))
        );
    }

    #[test]
    fn test_rejected_replay_does_not_advance_state() {
        let ctx = setup();
        let env = &ctx.env;
        let p = proof(env);
        let payload = Bytes::from_slice(env, b"ciphertext");

        let first = try_log(&ctx, &payload, &AccessControlPolicy::Public, &p).unwrap();
        assert!(try_log(&ctx, &payload, &AccessControlPolicy::Public, &p).is_err());

        // The rejected replay leaves the original event intact, and a fresh
        // proof is still accepted as a distinct event.
        let second = log(&ctx, b"ciphertext", &AccessControlPolicy::Public);
        assert_ne!(first, second);
        assert_eq!(ctx.client.get_confidential_event(&first).encrypted_payload, payload);
        ctx.client.extend_event_ttl(&first);
    }

    #[test]
    fn test_active_key_cannot_be_silently_overwritten() {
        let ctx = setup();
        let env = &ctx.env;
        let user = &ctx.submitter;

        ctx.client.register_encryption_key(user, &Bytes::from_slice(env, b"pk_v1"));
        assert_eq!(
            ctx.client
                .try_register_encryption_key(user, &Bytes::from_slice(env, b"pk_other")),
            Err(Ok(ConfidentialError::KeyAlreadyRegistered))
        );
    }

    #[test]
    fn test_retired_keys_cannot_be_replayed() {
        let ctx = setup();
        let env = &ctx.env;
        let user = &ctx.submitter;
        let v1 = Bytes::from_slice(env, b"pk_v1");
        let v2 = Bytes::from_slice(env, b"pk_v2");
        let v3 = Bytes::from_slice(env, b"pk_v3");

        ctx.client.register_encryption_key(user, &v1);

        // Rotating to the current key is a no-op replay.
        assert_eq!(
            ctx.client.try_rotate_encryption_key(user, &v1),
            Err(Ok(ConfidentialError::KeyReplay))
        );

        // v1 is retired by rotation; rotating back to it is rejected.
        ctx.client.rotate_encryption_key(user, &v2);
        assert_eq!(
            ctx.client.try_rotate_encryption_key(user, &v1),
            Err(Ok(ConfidentialError::KeyReplay))
        );

        // v2 is retired by revocation; re-registering it is rejected.
        ctx.client.revoke_encryption_key(user);
        assert_eq!(
            ctx.client.try_register_encryption_key(user, &v2),
            Err(Ok(ConfidentialError::KeyReplay))
        );
        assert_eq!(
            ctx.client.try_register_encryption_key(user, &v1),
            Err(Ok(ConfidentialError::KeyReplay))
        );

        // A genuinely new key can be registered after revocation.
        ctx.client.register_encryption_key(user, &v3);
    }

    #[test]
    fn test_retired_keys_are_scoped_per_owner() {
        let ctx = setup();
        let env = &ctx.env;
        let other = Address::generate(env);
        grant(&ctx, &other, Role::Submitter);
        let shared = Bytes::from_slice(env, b"pk_shared");

        ctx.client.register_encryption_key(&ctx.submitter, &shared);
        ctx.client.revoke_encryption_key(&ctx.submitter);

        // Another owner's history does not block this key.
        ctx.client.register_encryption_key(&other, &shared);
    }
}
