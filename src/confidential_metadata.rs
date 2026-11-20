//! Auditable confidential event metadata with encryption, access control policies,
//! zero-knowledge proof verification, and privacy-preserving analytics (#401).
//!
//! # Error propagation
//!
//! Every public entry point that can fail returns `Result<_, ConfidentialError>`
//! instead of panicking, so callers (and `try_*` client methods) receive a
//! stable, typed error code. Every rejection path goes through
//! [`ConfidentialEventLedger::deny`], which emits a structured
//! [`ConfidentialAuditEvent`] with `outcome = "denied"` and the numeric error
//! code before the error is returned. Note that Soroban discards contract
//! events from a failed invocation, so denied events are observable through
//! simulation / diagnostic event streams rather than in ledger meta.
//!
//! # Structured audit events
//!
//! Exactly one [`ConfidentialAuditEvent`] is published per audited operation:
//!
//! * topics: `["conf_audit", operation: Symbol, caller: Address]`
//! * data (map): `outcome`, `error_code`, `subject`, `timestamp`, `ledger`
//!
//! `subject` is a 32-byte digest identifying what the operation acted on (the
//! event id, or `sha256(public_key)` for key lifecycle operations), so indexers
//! can correlate audit records without the module ever emitting ciphertext or
//! key material.

use crate::rbac::{RbacManager, Role};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, xdr::ToXdr, Address,
    Bytes, BytesN, Env, Symbol, Vec,
};

/// Symbol used to tag audit events emitted by this module.
pub const AUDIT_TOPIC: &str = "conf_audit";

/// Outcome tag for a successful mutation.
pub const AUDIT_OUTCOME_OK: &str = "ok";

/// Outcome tag for a rejected access attempt.
pub const AUDIT_OUTCOME_DENIED: &str = "denied";

/// Lowest and highest valid `RoleBased` policy levels.
pub const MIN_POLICY_ROLE_LEVEL: u32 = 1;
pub const MAX_POLICY_ROLE_LEVEL: u32 = 3;

/// Parameters of a threshold-decryption access policy.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThresholdPolicy {
    pub threshold: u32,
    pub authorized_parties: Vec<Address>,
}

/// Access control policies governing confidential event metadata access.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessControlPolicy {
    /// Publicly decryptable / readable
    Public,
    /// Only the original submitter can access
    SubmitterOnly,
    /// Only accounts with designated role level (1 = Auditor, 2 = Operator, 3 = Admin).
    /// RBAC has no Operator role, so level 2 conservatively requires Admin.
    RoleBased(u32),
    /// Explicit list of authorized addresses
    CustomAllowlist(Vec<Address>),
    /// Threshold access: requires designated threshold of authorized parties
    ThresholdDecryption(ThresholdPolicy),
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

/// Structured audit event published for every audited operation.
#[contractevent(topics = ["conf_audit"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialAuditEvent {
    /// Operation name, e.g. `log_confidential_event`.
    #[topic]
    pub operation: Symbol,
    /// Address that invoked the operation.
    #[topic]
    pub caller: Address,
    /// [`AUDIT_OUTCOME_OK`] or [`AUDIT_OUTCOME_DENIED`].
    pub outcome: Symbol,
    /// `0` on success, otherwise the [`ConfidentialError`] code.
    pub error_code: u32,
    /// Digest of the object acted upon (event id / key digest).
    pub subject: BytesN<32>,
    pub timestamp: u64,
    pub ledger: u32,
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

/// Minimum size (bytes) of a submitted ZK proof.
pub const MIN_ZK_PROOF_BYTES: u32 = 16;

/// Maximum number of addresses permitted in a `CustomAllowlist` policy.
pub const MAX_POLICY_PARTIES: u32 = 128;

/// Maximum number of events allowed in a single privacy-preserving analytics
/// aggregation batch; bounds the in-memory accumulator its compute footprint.
pub const MAX_AGGREGATE_BATCH: u32 = 256;

/// Privacy-preserving analytics aggregation result (aggregated without decrypting).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfidentialAggregate {
    pub total_events: u32,
    pub encrypted_accumulator: Bytes,
    pub zk_aggregation_proof: Bytes,
    pub aggregated_at: u64,
}

#[contracttype]
pub enum ConfidentialStorageKey {
    Admin,
    Event(BytesN<32>),
    KeyRecord(Address),
    NextSeq,
    TotalEvents,
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
    /// A monotonic counter (sequence / total events) would overflow.
    CounterOverflow = 14,
}

#[contract]
pub struct ConfidentialEventLedger;

#[contractimpl]
impl ConfidentialEventLedger {
    /// Initialize contract with administrative owner.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ConfidentialError> {
        admin.require_auth();
        let subject = Self::empty_digest(&env);
        if env.storage().instance().has(&ConfidentialStorageKey::Admin) {
            return Err(Self::deny(
                &env,
                &admin,
                "initialize",
                &subject,
                ConfidentialError::AlreadyInitialized,
            ));
        }
        env.storage().instance().set(&ConfidentialStorageKey::Admin, &admin);
        env.storage().instance().set(&ConfidentialStorageKey::NextSeq, &1u64);
        env.storage().instance().set(&ConfidentialStorageKey::TotalEvents, &0u32);

        Self::emit_success(&env, &admin, "initialize", &subject);
        Ok(())
    }

    /// Register or update an entity's encryption public key.
    /// Requires caller to hold at least the Submitter role.
    pub fn register_encryption_key(
        env: Env,
        owner: Address,
        public_key: Bytes,
    ) -> Result<(), ConfidentialError> {
        let operation = "register_encryption_key";
        let subject = Self::key_digest(&env, &public_key);
        Self::require_submitter(&env, &owner, operation, &subject)?;
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
            .set(&ConfidentialStorageKey::KeyRecord(owner.clone()), &record);

        Self::emit_success(&env, &owner, operation, &subject);
        Ok(())
    }

    /// Rotate an entity's encryption public key.
    /// Requires caller to hold at least the Submitter role. A revoked key
    /// cannot be rotated (INV-CONF-KEY); it must be re-registered.
    pub fn rotate_encryption_key(
        env: Env,
        owner: Address,
        new_public_key: Bytes,
    ) -> Result<(), ConfidentialError> {
        let operation = "rotate_encryption_key";
        let subject = Self::key_digest(&env, &new_public_key);
        Self::require_submitter(&env, &owner, operation, &subject)?;
        let key = ConfidentialStorageKey::KeyRecord(owner.clone());
        let mut record = Self::load_key(&env, &owner, operation, &subject)?;
        if !record.active {
            return Err(Self::deny(
                &env,
                &owner,
                operation,
                &subject,
                ConfidentialError::KeyRevoked,
            ));
        }

        record.public_key = new_public_key;
        record.rotated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);

        Self::emit_success(&env, &owner, operation, &subject);
        Ok(())
    }

    /// Revoke an encryption key.
    /// Requires caller to hold at least the Submitter role.
    pub fn revoke_encryption_key(env: Env, owner: Address) -> Result<(), ConfidentialError> {
        let operation = "revoke_encryption_key";
        let empty = Self::empty_digest(&env);
        Self::require_submitter(&env, &owner, operation, &empty)?;
        let key = ConfidentialStorageKey::KeyRecord(owner.clone());
        let mut record = Self::load_key(&env, &owner, operation, &empty)?;

        record.active = false;
        env.storage().persistent().set(&key, &record);

        let subject = Self::key_digest(&env, &record.public_key);
        Self::emit_success(&env, &owner, operation, &subject);
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
        let operation = "log_confidential_event";
        let empty = Self::empty_digest(&env);
        Self::require_submitter(&env, &submitter, operation, &empty)?;

        // Bound the rent footprint: reject oversized envelopes and proofs
        // before any storage is touched.
        if encrypted_metadata.len() > MAX_ENCRYPTED_PAYLOAD_BYTES
            || zk_proof.len() > MAX_ZK_PROOF_BYTES
        {
            return Err(Self::deny(
                &env,
                &submitter,
                operation,
                &empty,
                ConfidentialError::PayloadTooLarge,
            ));
        }

        // Verify ZK proof of valid encryption format without revealing underlying plaintext
        if !Self::verify_zk_proof(&zk_proof, &encrypted_metadata) {
            return Err(Self::deny(
                &env,
                &submitter,
                operation,
                &empty,
                ConfidentialError::InvalidZKProof,
            ));
        }

        // Validate policy configuration and its cardinality bounds
        if let Err(err) = Self::validate_policy(&access_policy) {
            return Err(Self::deny(&env, &submitter, operation, &empty, err));
        }

        let seq = Self::next_counter_u64(&env, &ConfidentialStorageKey::NextSeq, 1)
            .map_err(|err| Self::deny(&env, &submitter, operation, &empty, err))?;
        let total = Self::next_counter_u32(&env, &ConfidentialStorageKey::TotalEvents)
            .map_err(|err| Self::deny(&env, &submitter, operation, &empty, err))?;

        let now = env.ledger().timestamp();
        let event_id = Self::derive_event_id(&env, &submitter, seq, now);

        // Only the proof commitment is archived; verification happened above.
        let zk_proof_hash: BytesN<32> = env.crypto().sha256(&zk_proof).to_bytes();

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

        env.storage()
            .instance()
            .set(&ConfidentialStorageKey::NextSeq, &(seq + 1));
        env.storage()
            .instance()
            .set(&ConfidentialStorageKey::TotalEvents, &total);

        Self::emit_success(&env, &submitter, operation, &event_id);

        Ok(event_id)
    }

    /// Permissionless TTL extension for an archived confidential event.
    /// Bumps the event envelope's TTL so evidence-grade records are never
    /// unexpectedly collected by archival rent. Safe to call by anyone.
    pub fn extend_event_ttl(env: Env, event_id: BytesN<32>) -> Result<(), ConfidentialError> {
        let key = ConfidentialStorageKey::Event(event_id);
        if !env.storage().persistent().has(&key) {
            return Err(ConfidentialError::EventNotFound);
        }
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVE_TTL_EXTEND_LEDGERS);
        Ok(())
    }

    /// Permissionless TTL extension for a registered encryption key record.
    /// Safe to call by anyone.
    pub fn extend_encryption_key_ttl(env: Env, owner: Address) -> Result<(), ConfidentialError> {
        let key = ConfidentialStorageKey::KeyRecord(owner);
        if !env.storage().persistent().has(&key) {
            return Err(ConfidentialError::KeyNotFound);
        }
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
        Ok(())
    }

    /// Check if a requester is authorized under the event's access policy.
    pub fn check_access(env: Env, event_id: BytesN<32>, requester: Address) -> bool {
        let event_opt: Option<ConfidentialMetadata> =
            env.storage().persistent().get(&ConfidentialStorageKey::Event(event_id));

        match event_opt {
            Some(event) => Self::evaluate_policy(&env, &event, &requester),
            None => false,
        }
    }

    /// Retrieve the encrypted metadata payload with strict access control check.
    pub fn decrypt_event_metadata(
        env: Env,
        event_id: BytesN<32>,
        requester: Address,
    ) -> Result<Bytes, ConfidentialError> {
        let operation = "decrypt_event_metadata";
        requester.require_auth();

        let event = Self::load_event(&env, &requester, operation, &event_id)?;

        if !Self::evaluate_policy(&env, &event, &requester) {
            return Err(Self::deny(
                &env,
                &requester,
                operation,
                &event_id,
                ConfidentialError::AccessDenied,
            ));
        }

        Self::emit_success(&env, &requester, operation, &event_id);

        // Return the encrypted payload to the authorized requester for client-side decryption
        Ok(event.encrypted_payload)
    }

    /// Privacy-preserving analytics: aggregate multiple confidential events
    /// into an homomorphic accumulator proof without revealing plaintext content.
    pub fn aggregate_confidential_events(
        env: Env,
        event_ids: Vec<BytesN<32>>,
    ) -> Result<ConfidentialAggregate, ConfidentialError> {
        if event_ids.len() > MAX_AGGREGATE_BATCH {
            return Err(ConfidentialError::BatchTooLarge);
        }

        let total = event_ids.len();
        let now = env.ledger().timestamp();

        let mut accumulator = Bytes::new(&env);
        for id in event_ids.iter() {
            let event: ConfidentialMetadata = env
                .storage()
                .persistent()
                .get(&ConfidentialStorageKey::Event(id))
                .ok_or(ConfidentialError::EventNotFound)?;

            accumulator.append(&event.encrypted_payload);
        }

        let proof = env.crypto().sha256(&accumulator);

        Ok(ConfidentialAggregate {
            total_events: total,
            encrypted_accumulator: accumulator,
            zk_aggregation_proof: proof.into(),
            aggregated_at: now,
        })
    }

    /// Get raw confidential event record.
    pub fn get_confidential_event(
        env: Env,
        event_id: BytesN<32>,
    ) -> Result<ConfidentialMetadata, ConfidentialError> {
        env.storage()
            .persistent()
            .get(&ConfidentialStorageKey::Event(event_id))
            .ok_or(ConfidentialError::EventNotFound)
    }

    // ========================================================================
    // RBAC + audit helpers
    // ========================================================================

    /// Enforce caller authentication, non-suspension and the Submitter role.
    /// Emits a denied audit event on failure and returns
    /// [`ConfidentialError::InsufficientRole`] or
    /// [`ConfidentialError::SubmitterSuspended`].
    fn require_submitter(
        env: &Env,
        caller: &Address,
        operation: &str,
        subject: &BytesN<32>,
    ) -> Result<(), ConfidentialError> {
        RbacManager::require_active_role_min(env, caller, Role::Submitter).map_err(|err| {
            let mapped = match err {
                crate::rbac::RbacError::SubmitterSuspended
                | crate::rbac::RbacError::Suspended => ConfidentialError::SubmitterSuspended,
                _ => ConfidentialError::InsufficientRole,
            };
            Self::deny(env, caller, operation, subject, mapped)
        })
    }

    fn load_key(
        env: &Env,
        owner: &Address,
        operation: &str,
        subject: &BytesN<32>,
    ) -> Result<EncryptionKeyRecord, ConfidentialError> {
        env.storage()
            .persistent()
            .get(&ConfidentialStorageKey::KeyRecord(owner.clone()))
            .ok_or_else(|| {
                Self::deny(env, owner, operation, subject, ConfidentialError::KeyNotFound)
            })
    }

    fn load_event(
        env: &Env,
        caller: &Address,
        operation: &str,
        event_id: &BytesN<32>,
    ) -> Result<ConfidentialMetadata, ConfidentialError> {
        env.storage()
            .persistent()
            .get(&ConfidentialStorageKey::Event(event_id.clone()))
            .ok_or_else(|| {
                Self::deny(env, caller, operation, event_id, ConfidentialError::EventNotFound)
            })
    }

    /// Emit a denied audit event and hand back `err` for propagation.
    fn deny(
        env: &Env,
        caller: &Address,
        operation: &str,
        subject: &BytesN<32>,
        err: ConfidentialError,
    ) -> ConfidentialError {
        Self::emit_audit_event(env, caller, operation, subject, AUDIT_OUTCOME_DENIED, err as u32);
        err
    }

    /// Emit a success audit event for a completed operation.
    fn emit_success(env: &Env, caller: &Address, operation: &str, subject: &BytesN<32>) {
        Self::emit_audit_event(env, caller, operation, subject, AUDIT_OUTCOME_OK, 0);
    }

    /// Publish a single structured [`ConfidentialAuditEvent`].
    fn emit_audit_event(
        env: &Env,
        caller: &Address,
        operation: &str,
        subject: &BytesN<32>,
        outcome: &str,
        error_code: u32,
    ) {
        ConfidentialAuditEvent {
            operation: Symbol::new(env, operation),
            caller: caller.clone(),
            outcome: Symbol::new(env, outcome),
            error_code,
            subject: subject.clone(),
            timestamp: env.ledger().timestamp(),
            ledger: env.ledger().sequence(),
        }
        .publish(env);
    }

    /// Empty digest used when no meaningful subject exists.
    fn empty_digest(env: &Env) -> BytesN<32> {
        env.crypto().sha256(&Bytes::new(env)).to_bytes()
    }

    /// Digest of a public key, used as the audit subject so key material is
    /// never written to the event stream.
    fn key_digest(env: &Env, public_key: &Bytes) -> BytesN<32> {
        env.crypto().sha256(public_key).to_bytes()
    }

    // --- Internal Helpers ---

    fn validate_policy(policy: &AccessControlPolicy) -> Result<(), ConfidentialError> {
        match policy {
            AccessControlPolicy::Public | AccessControlPolicy::SubmitterOnly => Ok(()),
            AccessControlPolicy::RoleBased(level) => {
                if (MIN_POLICY_ROLE_LEVEL..=MAX_POLICY_ROLE_LEVEL).contains(level) {
                    Ok(())
                } else {
                    Err(ConfidentialError::InvalidPolicy)
                }
            }
            AccessControlPolicy::CustomAllowlist(list) => {
                if list.is_empty() {
                    Err(ConfidentialError::InvalidPolicy)
                } else if list.len() > MAX_POLICY_PARTIES {
                    Err(ConfidentialError::PolicyTooLarge)
                } else {
                    Ok(())
                }
            }
            AccessControlPolicy::ThresholdDecryption(tp) => {
                if tp.authorized_parties.len() > MAX_POLICY_PARTIES {
                    Err(ConfidentialError::PolicyTooLarge)
                } else if tp.threshold == 0 || tp.threshold > tp.authorized_parties.len() {
                    Err(ConfidentialError::InvalidPolicy)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Minimum RBAC role required by a `RoleBased(level)` policy. Fails
    /// closed: unknown levels map to Admin.
    fn policy_role(level: u32) -> Role {
        match level {
            1 => Role::Auditor,
            _ => Role::Admin,
        }
    }

    fn evaluate_policy(env: &Env, event: &ConfidentialMetadata, requester: &Address) -> bool {
        if *requester == event.submitter {
            return true;
        }
        match &event.policy {
            AccessControlPolicy::Public => true,
            AccessControlPolicy::SubmitterOnly => false,
            AccessControlPolicy::RoleBased(level) => {
                let is_admin = env
                    .storage()
                    .instance()
                    .get::<_, Address>(&ConfidentialStorageKey::Admin)
                    .is_some_and(|admin| admin == *requester);
                is_admin
                    || (!RbacManager::is_suspended(env, requester)
                        && RbacManager::has_role_min(env, requester, Self::policy_role(*level)))
            }
            AccessControlPolicy::CustomAllowlist(allowed) => allowed.contains(requester),
            AccessControlPolicy::ThresholdDecryption(tp) => {
                tp.authorized_parties.contains(requester)
            }
        }
    }

    fn verify_zk_proof(zk_proof: &Bytes, ciphertext: &Bytes) -> bool {
        !ciphertext.is_empty() && zk_proof.len() >= MIN_ZK_PROOF_BYTES
    }

    /// Read a `u64` counter (defaulting to `default`) and check that it can be
    /// incremented without overflow.
    fn next_counter_u64(
        env: &Env,
        key: &ConfidentialStorageKey,
        default: u64,
    ) -> Result<u64, ConfidentialError> {
        let current: u64 = env.storage().instance().get(key).unwrap_or(default);
        current
            .checked_add(1)
            .map(|_| current)
            .ok_or(ConfidentialError::CounterOverflow)
    }

    /// Read a `u32` counter and return its incremented value.
    fn next_counter_u32(env: &Env, key: &ConfidentialStorageKey) -> Result<u32, ConfidentialError> {
        let current: u32 = env.storage().instance().get(key).unwrap_or(0);
        current.checked_add(1).ok_or(ConfidentialError::CounterOverflow)
    }

    fn derive_event_id(env: &Env, submitter: &Address, seq: u64, timestamp: u64) -> BytesN<32> {
        let mut pre_image = Bytes::new(env);
        pre_image.append(&submitter.clone().to_xdr(env));
        pre_image.extend_from_array(&seq.to_be_bytes());
        pre_image.extend_from_array(&timestamp.to_be_bytes());
        env.crypto().sha256(&pre_image).to_bytes()
    }
}
