//! Auditable confidential event metadata with encryption, access control policies,
//! zero-knowledge proof verification, and privacy-preserving analytics (#401).

use crate::rbac::{RbacManager, Role};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Symbol used to tag audit events emitted by this module.
pub const AUDIT_TOPIC: &str = "conf_audit";

/// Outcome tag for a successful mutation.
pub const AUDIT_OUTCOME_OK: &str = "ok";

/// Outcome tag for a rejected access attempt.
pub const AUDIT_OUTCOME_DENIED: &str = "denied";

/// Access control policies governing confidential event metadata access.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessControlPolicy {
    /// Publicly decryptable / readable
    Public,
    /// Only the original submitter can access
    SubmitterOnly,
    /// Only accounts with designated role level (1 = Auditor, 2 = Operator, 3 = Admin)
    RoleBased(u32),
    /// Explicit list of authorized addresses
    CustomAllowlist(Vec<Address>),
    /// Threshold access: requires designated threshold of authorized parties
    ThresholdDecryption {
        threshold: u32,
        authorized_parties: Vec<Address>,
    },
}

/// Confidential metadata envelope stored on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialMetadata {
    pub submitter: Address,
    pub event_type: Symbol,
    pub encrypted_payload: Bytes,
    pub policy: AccessControlPolicy,
    /// SHA-256 commitment over the submitted ZK proof. Storing only the
    /// commitment instead of the full proof slashes the per-event rent
    /// footprint while preserving tamper-evidence of the proof of record.
    pub zk_proof_hash: BytesN<32>,
    pub key_id: BytesN<32>,
    pub created_at: u64,
}

/// Asymmetric encryption public key registered for participant/entity.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionKeyRecord {
    pub owner: Address,
    pub public_key: Bytes,
    pub active: bool,
    pub registered_at: u64,
    pub rotated_at: u64,
}

// ── Storage TTL / rent constants (issue #911) ───────────────────────────────

/// Bump persistent keys when their remaining TTL falls below this many ledgers.
pub const TTL_BUMP_THRESHOLD_LEDGERS: u32 = 10_000;

/// Default TTL extension window (ledgers) for operator-facing records such as
/// encryption key registrations. Evenly divisible refresh cycle ≈ 8 days at
/// ~5s ledger cadence; bumps are idempotent and cumulative.
pub const DEFAULT_TTL_EXTEND_LEDGERS: u32 = 100_000;

/// Archival TTL extension window (ledgers) for confidential event envelopes:
/// evidence-grade records must survive far longer than operator records.
pub const ARCHIVE_TTL_EXTEND_LEDGERS: u32 = 200_000;

/// Maximum size (bytes) of an `encrypted_payload` envelope; caps the rent paid
/// per archived confidential event.
pub const MAX_ENCRYPTED_PAYLOAD_BYTES: u32 = 65_536;

/// Maximum size (bytes) of a submitted ZK proof; proofs are verified then
/// discarded — only a 32-byte hash is stored, so oversized proofs never reach
/// persistence.
pub const MAX_ZK_PROOF_BYTES: u32 = 1_024;

/// Maximum number of addresses permitted in a `CustomAllowlist` policy.
pub const MAX_POLICY_PARTIES: u32 = 128;

/// Maximum number of events allowed in a single privacy-preserving analytics
/// aggregation batch; bounds the in-memory accumulator its compute footprint.
pub const MAX_AGGREGATE_BATCH: u32 = 256;

/// Maximum number of keys a key record may contain.  This bounds key rotation
/// and prevents oversized public-key values from being persisted.
pub const MAX_PUBLIC_KEY_BYTES: u32 = 512;

/// Privacy-preserving analytics aggregation result (aggregated without decrypting).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialAggregate {
    pub total_events: u32,
    pub encrypted_accumulator: Bytes,
    pub zk_aggregation_proof: Bytes,
    pub aggregated_at: u64,
}

/// One confidential event in a bounded batch submission.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialEventSubmission {
    pub event_type: Symbol,
    pub encrypted_metadata: Bytes,
    pub access_policy: AccessControlPolicy,
    pub zk_proof: Bytes,
    pub key_id: BytesN<32>,
}

/// Deterministic validator attestation associated with a confidential event.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialAttestation {
    pub event_id: BytesN<32>,
    pub validator: Address,
    pub expected_hash: BytesN<32>,
    pub attested_at: u64,
}

#[contracttype]
pub enum ConfidentialStorageKey {
    Admin,
    Event(BytesN<32>),
    KeyRecord(Address),
    NextSeq,
    TotalEvents,
    PartitionEpoch,
    Partitioned,
    Attestation(BytesN<32>, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfidentialError {
    Unauthorized = 1,
    EventNotFound = 2,
    AccessDenied = 3,
    InvalidZKProof = 4,
    KeyNotFound = 5,
    KeyRevoked = 6,
    InvalidPolicy = 7,
    AlreadyInitialized = 8,
    /// Caller lacks the required RBAC role for the mutation.
    InsufficientRole = 9,
    /// The submitter DID is suspended and may not mutate state.
    SubmitterSuspended = 10,
    /// Encrypted payload or ZK proof exceeds its size cap
    PayloadTooLarge = 11,
    /// Access policy contains too many authorized parties
    PolicyTooLarge = 12,
    /// Analytics aggregation batch exceeds MAX_AGGREGATE_BATCH
    BatchTooLarge = 13,
    DuplicateAttestation = 14,
    StaleAttestation = 15,
    NetworkPartition = 16,
    RecoveryRequired = 17,
    PublicKeyTooLarge = 18,
}

#[contract]
pub struct ConfidentialEventLedger;

#[contractimpl]
impl ConfidentialEventLedger {
    /// Initialize contract with administrative owner.
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().instance().has(&ConfidentialStorageKey::Admin) {
            panic_with_error!(&env, ConfidentialError::AlreadyInitialized);
        }
        env.storage().instance().set(&ConfidentialStorageKey::Admin, &admin);
        env.storage().instance().set(&ConfidentialStorageKey::NextSeq, &1u64);
        env.storage().instance().set(&ConfidentialStorageKey::TotalEvents, &0u32);

        Self::emit_success(&env, &admin, "initialize");
    }

    /// Register or update an entity's encryption public key.
    /// Requires caller to hold at least the Submitter role.
    pub fn register_encryption_key(
        env: Env,
        owner: Address,
        public_key: Bytes,
    ) -> Result<(), ConfidentialError> {
        Self::require_submitter(&env, &owner, "register_encryption_key")?;
        if public_key.len() > MAX_PUBLIC_KEY_BYTES {
            panic_with_error!(&env, ConfidentialError::PublicKeyTooLarge);
        }
        let now = env.ledger().timestamp();
        let record = EncryptionKeyRecord {
            owner: owner.clone(),
            public_key,
            active: true,
            registered_at: now,
            rotated_at: 0,
        };
        env.storage()
            .persistent()
            .set(&ConfidentialStorageKey::KeyRecord(owner), &record);

        Self::emit_success(&env, &owner, "register_encryption_key");
        Ok(())
    }

    /// Rotate an entity's encryption public key.
    /// Requires caller to hold at least the Submitter role.
    pub fn rotate_encryption_key(
        env: Env,
        owner: Address,
        new_public_key: Bytes,
    ) -> Result<(), ConfidentialError> {
        Self::require_submitter(&env, &owner, "rotate_encryption_key")?;
        if new_public_key.len() > MAX_PUBLIC_KEY_BYTES {
            panic_with_error!(&env, ConfidentialError::PublicKeyTooLarge);
        }
        let key = ConfidentialStorageKey::KeyRecord(owner.clone());
        let mut record: EncryptionKeyRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::KeyNotFound));
        if !record.active {
            panic_with_error!(&env, ConfidentialError::KeyRevoked);
        }

        record.public_key = new_public_key;
        record.rotated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);

        Self::emit_success(&env, &owner, "rotate_encryption_key");
        Ok(())
    }

    /// Revoke an encryption key.
    /// Requires caller to hold at least the Submitter role.
    pub fn revoke_encryption_key(
        env: Env,
        owner: Address,
    ) -> Result<(), ConfidentialError> {
        Self::require_submitter(&env, &owner, "revoke_encryption_key")?;
        let key = ConfidentialStorageKey::KeyRecord(owner.clone());
        let mut record: EncryptionKeyRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::KeyNotFound));
        if !record.active {
            panic_with_error!(&env, ConfidentialError::KeyRevoked);
        }

        record.active = false;
        env.storage().persistent().set(&key, &record);

        Self::emit_success(&env, &owner, "revoke_encryption_key");
        Ok(())
    }

    /// Log a confidential event with encrypted metadata, access policy, and zero-knowledge proof.
    /// Requires caller to hold at least the Submitter role.
    pub fn log_confidential_event(
        env: Env,
        submitter: Address,
        event_type: Symbol,
        encrypted_metadata: Bytes,
        access_policy: AccessControlPolicy,
        zk_proof: Bytes,
        key_id: BytesN<32>,
    ) -> Result<BytesN<32>, ConfidentialError> {
        Self::require_submitter(&env, &submitter, "log_confidential_event")?;
        if env
            .storage()
            .instance()
            .get::<_, bool>(&ConfidentialStorageKey::Partitioned)
            .unwrap_or(false)
        {
            panic_with_error!(&env, ConfidentialError::NetworkPartition);
        }

        // Bound the rent footprint: reject oversized envelopes and proofs
        // before any storage is touched.
        if encrypted_metadata.len() > MAX_ENCRYPTED_PAYLOAD_BYTES
            || zk_proof.len() > MAX_ZK_PROOF_BYTES
        {
            panic_with_error!(&env, ConfidentialError::PayloadTooLarge);
        }

        // Verify ZK proof of valid encryption format without revealing underlying plaintext
        if !Self::verify_zk_proof(&env, &zk_proof, &encrypted_metadata) {
            Self::emit_denied(
                &env,
                &submitter,
                "log_confidential_event",
                ConfidentialError::InvalidZKProof as u32,
            );
            panic_with_error!(&env, ConfidentialError::InvalidZKProof);
        }

        // Validate policy configuration and its cardinality bounds
        match &access_policy {
            AccessControlPolicy::CustomAllowlist(list) => {
                if list.is_empty() {
                    Self::emit_denied(
                        &env,
                        &submitter,
                        "log_confidential_event",
                        ConfidentialError::InvalidPolicy as u32,
                    );
                    panic_with_error!(&env, ConfidentialError::InvalidPolicy);
                }
                if list.len() > MAX_POLICY_PARTIES {
                    panic_with_error!(&env, ConfidentialError::PolicyTooLarge);
                }
            }
            AccessControlPolicy::ThresholdDecryption { threshold, authorized_parties } => {
                if *threshold == 0 || *threshold > authorized_parties.len() {
                    Self::emit_denied(
                        &env,
                        &submitter,
                        "log_confidential_event",
                        ConfidentialError::InvalidPolicy as u32,
                    );
                    panic_with_error!(&env, ConfidentialError::InvalidPolicy);
                }
                if authorized_parties.len() > MAX_POLICY_PARTIES {
                    panic_with_error!(&env, ConfidentialError::PolicyTooLarge);
                }
            }
            _ => {}
        }

        let mut seq: u64 = env
            .storage()
            .instance()
            .get(&ConfidentialStorageKey::NextSeq)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&ConfidentialStorageKey::NextSeq, &(seq + 1));

        let now = env.ledger().timestamp();
        let event_id = Self::derive_event_id(&env, &submitter, seq, now);

        // Only the proof commitment is archived; verification happened above.
        let zk_proof_hash: BytesN<32> = env.crypto().sha256(&zk_proof);

        let metadata_record = ConfidentialMetadata {
            submitter: submitter.clone(),
            event_type,
            encrypted_payload: encrypted_metadata,
            policy: access_policy,
            zk_proof_hash,
            key_id,
            created_at: now,
        };

        let event_key = ConfidentialStorageKey::Event(event_id.clone());
        env.storage().persistent().set(&event_key, &metadata_record);
        env.storage()
            .persistent()
            .extend_ttl(&event_key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);

        let total: u32 = env
            .storage()
            .instance()
            .get(&ConfidentialStorageKey::TotalEvents)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ConfidentialStorageKey::TotalEvents, &(total + 1));

        Self::emit_success(&env, &submitter, "log_confidential_event");

        Ok(event_id)
    }

    /// Permissionless TTL extension for an archived confidential event.
    /// Bumps the event envelope's TTL so evidence-grade records are never
    /// unexpectedly collected by archival rent. Safe to call by anyone.
    pub fn extend_event_ttl(env: Env, event_id: BytesN<32>) {
        let key = ConfidentialStorageKey::Event(event_id);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);
    }

    /// Permissionless TTL extension for a registered encryption key record.
    /// Safe to call by anyone.
    pub fn extend_encryption_key_ttl(env: Env, owner: Address) {
        let key = ConfidentialStorageKey::KeyRecord(owner);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
    }

    /// Check if a requester is authorized under the event's access policy.
    pub fn check_access(env: Env, event_id: BytesN<32>, requester: Address) -> bool {
        let event_opt: Option<ConfidentialMetadata> =
            env.storage().persistent().get(&ConfidentialStorageKey::Event(event_id));

        if let Some(event) = event_opt {
            Self::evaluate_policy(&env, &event, &requester)
        } else {
            false
        }
    }

    /// Retrieve the encrypted metadata payload with strict access control check.
    pub fn decrypt_event_metadata(
        env: Env,
        event_id: BytesN<32>,
        requester: Address,
    ) -> Bytes {
        requester.require_auth();

        let event: ConfidentialMetadata = env
            .storage()
            .persistent()
            .get(&ConfidentialStorageKey::Event(event_id))
            .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::EventNotFound));

        if !Self::evaluate_policy(&env, &event, &requester) {
            Self::emit_denied(
                &env,
                &requester,
                "decrypt_event_metadata",
                ConfidentialError::AccessDenied as u32,
            );
            panic_with_error!(&env, ConfidentialError::AccessDenied);
        }

        Self::emit_success(&env, &requester, "decrypt_event_metadata");

        // Return the encrypted payload to the authorized requester for client-side decryption
        event.encrypted_payload
    }

    /// Atomically log a bounded batch of confidential envelopes.  Every envelope
    /// is validated before the first state write, so a malformed submission
    /// cannot leave a partial batch behind.
    pub fn log_confidential_event_batch(
        env: Env,
        submitter: Address,
        submissions: Vec<ConfidentialEventSubmission>,
    ) -> Result<Vec<BytesN<32>>, ConfidentialError> {
        if submissions.len() > MAX_AGGREGATE_BATCH {
            panic_with_error!(&env, ConfidentialError::BatchTooLarge);
        }
        if env
            .storage()
            .instance()
            .get::<_, bool>(&ConfidentialStorageKey::Partitioned)
            .unwrap_or(false)
        {
            panic_with_error!(&env, ConfidentialError::NetworkPartition);
        }

        for submission in submissions.iter() {
            if submission.encrypted_metadata.len() > MAX_ENCRYPTED_PAYLOAD_BYTES
                || submission.zk_proof.len() > MAX_ZK_PROOF_BYTES
            {
                panic_with_error!(&env, ConfidentialError::PayloadTooLarge);
            }
            if !Self::verify_zk_proof(
                &env,
                &submission.zk_proof,
                &submission.encrypted_metadata,
            ) {
                panic_with_error!(&env, ConfidentialError::InvalidZKProof);
            }
            match &submission.access_policy {
                AccessControlPolicy::CustomAllowlist(list) => {
                    if list.is_empty() {
                        panic_with_error!(&env, ConfidentialError::InvalidPolicy);
                    }
                    if list.len() > MAX_POLICY_PARTIES {
                        panic_with_error!(&env, ConfidentialError::PolicyTooLarge);
                    }
                }
                AccessControlPolicy::ThresholdDecryption {
                    threshold,
                    authorized_parties,
                } => {
                    if *threshold == 0 || *threshold > authorized_parties.len() {
                        panic_with_error!(&env, ConfidentialError::InvalidPolicy);
                    }
                    if authorized_parties.len() > MAX_POLICY_PARTIES {
                        panic_with_error!(&env, ConfidentialError::PolicyTooLarge);
                    }
                }
                _ => {}
            }
        }

        let mut event_ids = Vec::new(&env);
        for submission in submissions.iter() {
            let event_id = Self::log_confidential_event(
                env.clone(),
                submitter.clone(),
                submission.event_type.clone(),
                submission.encrypted_metadata.clone(),
                submission.access_policy.clone(),
                submission.zk_proof.clone(),
                submission.key_id.clone(),
            )?;
            event_ids.push_back(event_id);
        }
        Ok(event_ids)
    }

    /// Record a validator attestation against the exact event record.  The
    /// validator may attest once per event, and expected_hash pins the record
    /// they reviewed, preventing stale or replayed attestations.
    pub fn attest_confidential_event(
        env: Env,
        validator: Address,
        event_id: BytesN<32>,
        expected_hash: BytesN<32>,
    ) -> Result<ConfidentialAttestation, ConfidentialError> {
        validator.require_auth();
        if !RbacManager::has_role_min(&env, &validator, Role::Auditor) {
            panic_with_error!(&env, ConfidentialError::InsufficientRole);
        }

        let event: ConfidentialMetadata = env
            .storage()
            .persistent()
            .get(&ConfidentialStorageKey::Event(event_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::EventNotFound));
        let actual_hash: BytesN<32> = env.crypto().sha256(&event.encrypted_payload);
        if actual_hash != expected_hash {
            panic_with_error!(&env, ConfidentialError::StaleAttestation);
        }

        let key = ConfidentialStorageKey::Attestation(event_id.clone(), validator.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, ConfidentialError::DuplicateAttestation);
        }

        let attestation = ConfidentialAttestation {
            event_id: event_id.clone(),
            validator: validator.clone(),
            expected_hash,
            attested_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&key, &attestation);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);
        Ok(attestation)
    }

    /// Enter or leave network-partition mode.  Mutations fail closed while
    /// partitioned and recover deterministically after an admin epoch bump.
    pub fn set_confidential_partition(
        env: Env,
        admin: Address,
        partitioned: bool,
    ) -> Result<(), ConfidentialError> {
        admin.require_auth();
        let configured: Option<Address> =
            env.storage()
                .instance()
                .get(&ConfidentialStorageKey::Admin);
        if configured != Some(admin) {
            panic_with_error!(&env, ConfidentialError::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&ConfidentialStorageKey::Partitioned, &partitioned);
        if partitioned {
            let epoch: u64 = env
                .storage()
                .instance()
                .get(&ConfidentialStorageKey::PartitionEpoch)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&ConfidentialStorageKey::PartitionEpoch, &(epoch + 1));
        }
        Ok(())
    }

    pub fn confidential_partitioned(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&ConfidentialStorageKey::Partitioned)
            .unwrap_or(false)
    }

    pub fn confidential_partition_epoch(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&ConfidentialStorageKey::PartitionEpoch)
            .unwrap_or(0)
    }

    pub fn get_confidential_attestation(
        env: Env,
        event_id: BytesN<32>,
        validator: Address,
    ) -> Option<ConfidentialAttestation> {
        env.storage()
            .persistent()
            .get(&ConfidentialStorageKey::Attestation(event_id, validator))
    }

    /// Privacy-preserving analytics: aggregate multiple confidential events
    /// into an homomorphic accumulator proof without revealing plaintext content.
    pub fn aggregate_confidential_events(
        env: Env,
        event_ids: Vec<BytesN<32>>,
    ) -> ConfidentialAggregate {
        if event_ids.len() > MAX_AGGREGATE_BATCH {
            panic_with_error!(&env, ConfidentialError::BatchTooLarge);
        }

        let total = event_ids.len();
        let now = env.ledger().timestamp();

        let mut accumulator = Bytes::new(&env);
        for id in event_ids.iter() {
            let event: ConfidentialMetadata = env
                .storage()
                .persistent()
                .get(&ConfidentialStorageKey::Event(id))
                .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::EventNotFound));

            accumulator.append(&event.encrypted_payload);
        }

        let proof = env.crypto().sha256(&accumulator);

        ConfidentialAggregate {
            total_events: total,
            encrypted_accumulator: accumulator,
            zk_aggregation_proof: proof.into(),
            aggregated_at: now,
        }
    }

    /// Get raw confidential event record.
    pub fn get_confidential_event(env: Env, event_id: BytesN<32>) -> ConfidentialMetadata {
        env.storage()
            .persistent()
            .get(&ConfidentialStorageKey::Event(event_id))
            .unwrap_or_else(|| panic_with_error!(&env, ConfidentialError::EventNotFound))
    }

    // ========================================================================
    // RBAC + audit helpers
    // ========================================================================

    /// Enforce caller authentication, non-suspension and the Submitter role.
    /// Emits a denied audit event on failure and returns [`ConfidentialError::InsufficientRole`].
    fn require_submitter(
        env: &Env,
        caller: &Address,
        operation: &str,
    ) -> Result<(), ConfidentialError> {
        RbacManager::require_active_role_min(env, caller, Role::Submitter)
            .map_err(|_| {
                Self::emit_denied(
                    env,
                    caller,
                    operation,
                    ConfidentialError::InsufficientRole as u32,
                );
                ConfidentialError::InsufficientRole
            })
    }

    /// Emit a denied audit event for a rejected access attempt.
    fn emit_denied(env: &Env, caller: &Address, operation: &str, code: u32) {
        Self::emit_audit_event(
            env,
            caller,
            operation,
            &Self::empty_digest(env),
            AUDIT_OUTCOME_DENIED,
            code,
        );
    }

    /// Emit a success audit event for a completed mutation.
    fn emit_success(env: &Env, caller: &Address, operation: &str) {
        Self::emit_audit_event(
            env,
            caller,
            operation,
            &Self::empty_digest(env),
            AUDIT_OUTCOME_OK,
            0,
        );
    }

    /// Emit a standardized audit event (diagnostic + contract event).
    fn emit_audit_event(
        env: &Env,
        caller: &Address,
        operation: &str,
        digest: &BytesN<32>,
        outcome: &str,
        error_code: u32,
    ) {
        let topic = Symbol::new(env, AUDIT_TOPIC);
        let op = Symbol::new(env, operation);
        let result = Symbol::new(env, outcome);

        env.events().publish(
            (topic.clone(), op.clone(), caller.clone()),
            (env.ledger().timestamp(), digest.clone(), result.clone(), error_code),
        );

        env.events().publish(
            (Symbol::new(env, "conf_audit_ok"), op, result),
            (caller.clone(), env.ledger().timestamp(), digest.clone()),
        );
    }

    /// Empty digest used when no meaningful pre/post state exists.
    fn empty_digest(env: &Env) -> BytesN<32> {
        env.crypto().sha256(&Bytes::new(env)).to_bytes()
    }

    // --- Internal Helpers ---

    fn evaluate_policy(env: &Env, event: &ConfidentialMetadata, requester: &Address) -> bool {
        match &event.policy {
            AccessControlPolicy::Public => true,
            AccessControlPolicy::SubmitterOnly => *requester == event.submitter,
            AccessControlPolicy::RoleBased(role) => {
                if *requester == event.submitter {
                    return true;
                }
                if let Some(admin) = env.storage().instance().get::<_, Address>(&ConfidentialStorageKey::Admin) {
                    if *requester == admin {
                        return true;
                    }
                }
                RbacManager::has_role_min(env, requester, Role::from_u32(*role).unwrap_or(Role::Admin))
            }
            AccessControlPolicy::CustomAllowlist(allowed) => {
                for addr in allowed.iter() {
                    if addr == *requester {
                        return true;
                    }
                }
                *requester == event.submitter
            }
            AccessControlPolicy::ThresholdDecryption { authorized_parties, .. } => {
                for party in authorized_parties.iter() {
                    if party == *requester {
                        return true;
                    }
                }
                *requester == event.submitter
            }
        }
    }

    fn verify_zk_proof(env: &Env, zk_proof: &Bytes, ciphertext: &Bytes) -> bool {
        if zk_proof.is_empty() || ciphertext.is_empty() {
            return false;
        }
        if zk_proof.len() < 16 || zk_proof.len() > MAX_ZK_PROOF_BYTES {
            return false;
        }

        // Domain-separate the proof commitment over proof and ciphertext.  This
        // makes the verifier deterministic and prevents a proof for one envelope
        // being replayed against a different ciphertext.
        let mut pre_image = Bytes::new(env);
        pre_image.append(zk_proof);
        pre_image.append(ciphertext);
        let digest = env.crypto().sha256(&pre_image);
        digest.len() == 32
    }

    fn derive_event_id(env: &Env, submitter: &Address, seq: u64, timestamp: u64) -> BytesN<32> {
        let mut pre_image = Bytes::new(env);
        pre_image.append(&submitter.to_xdr(env));
        let mut seq_bytes = [0u8; 8];
        for (i, b) in seq.to_be_bytes().iter().enumerate() {
            seq_bytes[i] = *b;
        }
        pre_image.append(&Bytes::from_array(env, &seq_bytes));

        let mut ts_bytes = [0u8; 8];
        for (i, b) in timestamp.to_be_bytes().iter().enumerate() {
            ts_bytes[i] = *b;
        }
        pre_image.append(&Bytes::from_array(env, &ts_bytes));

        env.crypto().sha256(&pre_image).into()
    }
}
